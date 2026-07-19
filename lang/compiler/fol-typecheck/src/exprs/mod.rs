pub mod access;
pub mod bindings;
pub mod calls;
pub mod controlflow;
pub mod helpers;
pub mod literals;
pub mod operators;
pub mod references;

use crate::{
    decls, CheckedType, CheckedTypeId, RecoverableCallEffect, RoutineType, TypecheckError,
    TypecheckErrorKind, TypecheckResult, TypedProgram,
};
use fol_intrinsics::{select_intrinsic, IntrinsicSurface};
use fol_parser::ast::{
    AstNode, CallSurface, ChannelEndpoint, FolType, OwnershipOption, ParsedSourceUnitKind,
    UnaryOperator,
};
use fol_resolver::{ReferenceKind, ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::borrow::Cow;
use std::collections::BTreeMap;

use helpers::{
    binding_kind_for, describe_type, ensure_assignable, ensure_assignable_target, internal_error,
    node_origin, origin_for, plain_value_expr, unsupported_node_surface, with_node_origin,
};
pub(crate) use helpers::{inline_body_block_scope, loop_body_scope};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorCallMode {
    Propagate,
    Observe,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TypeContext {
    pub(crate) source_unit_id: SourceUnitId,
    pub(crate) scope_id: ScopeId,
    pub(crate) routine_return_type: Option<CheckedTypeId>,
    pub(crate) routine_error_type: Option<CheckedTypeId>,
    pub(crate) error_call_mode: ErrorCallMode,
    /// Syntax id of the direct call whose arguments will cross a spawn/async
    /// task boundary. Keeping the exact id prevents nested calls inside task
    /// arguments from being mistaken for asynchronous calls themselves.
    pub(crate) processor_task_call: Option<fol_parser::ast::SyntaxNodeId>,
    /// True only while an argument is being passed from one `mux[T]`
    /// parameter to another. Every other whole-value use stays forbidden.
    pub(crate) allow_mutex_handle: bool,
    /// Exact body scope of the innermost repeating loop. Transfers from
    /// bindings declared outside this scope would execute more than once and
    /// therefore cannot consume a move-only value.
    pub(crate) repeating_loop_scope: Option<ScopeId>,
    /// True while typing a dfr/edf body. Mutex guard transitions are lexical
    /// today and cannot be replayed safely when deferred execution runs.
    pub(crate) inside_deferred_block: bool,
    /// True specifically inside error-only `edf` cleanup. Eventual ownership
    /// cannot be mutated there because the body does not run on normal exits.
    pub(crate) inside_error_deferred_block: bool,
    /// True only while typing the receiver of a field access, i.e. the `job`
    /// in `job.field`. A partially moved aggregate may still be projected for a
    /// surviving field, so the whole-value "partially moved" rejection is
    /// suppressed here and enforced only for genuine whole-value reads.
    pub(crate) field_projection_root: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TypedExpr {
    pub(crate) value_type: Option<CheckedTypeId>,
    pub(crate) recoverable_effect: Option<RecoverableCallEffect>,
}

impl TypedExpr {
    pub(crate) fn none() -> Self {
        Self {
            value_type: None,
            recoverable_effect: None,
        }
    }

    pub(crate) fn value(value_type: CheckedTypeId) -> Self {
        Self {
            value_type: Some(value_type),
            recoverable_effect: None,
        }
    }

    pub(crate) fn maybe_value(value_type: Option<CheckedTypeId>) -> Self {
        Self {
            value_type,
            recoverable_effect: None,
        }
    }

    pub(crate) fn with_optional_effect(mut self, effect: Option<RecoverableCallEffect>) -> Self {
        self.recoverable_effect = effect;
        self
    }

    pub(crate) fn is_never(self, typed: &TypedProgram) -> bool {
        self.value_type == Some(typed.builtin_types().never)
    }

    pub(crate) fn required_value(
        self,
        message: impl Into<String>,
    ) -> Result<CheckedTypeId, TypecheckError> {
        self.value_type
            .ok_or_else(|| TypecheckError::new(TypecheckErrorKind::InvalidInput, message))
    }
}

pub fn type_program(typed: &mut TypedProgram) -> TypecheckResult<()> {
    let resolved = typed.resolved().clone();
    let syntax = resolved.syntax().clone();
    let mut errors = Vec::new();

    for (source_unit_index, source_unit) in syntax.source_units.iter().enumerate() {
        if source_unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit_id = SourceUnitId(source_unit_index);
        let scope_id = match resolved
            .source_unit(source_unit_id)
            .map(|unit| unit.scope_id)
        {
            Some(scope_id) => scope_id,
            None => {
                return Err(vec![internal_error(
                    "resolved source unit disappeared",
                    None,
                )])
            }
        };
        let context = TypeContext {
            source_unit_id,
            scope_id,
            routine_return_type: None,
            routine_error_type: None,
            error_call_mode: ErrorCallMode::Propagate,
            processor_task_call: None,
            allow_mutex_handle: false,
            repeating_loop_scope: None,
            inside_deferred_block: false,
            inside_error_deferred_block: false,
            field_projection_root: false,
        };
        for item in &source_unit.items {
            if let Err(error) = type_node(typed, &resolved, context, &item.node) {
                errors.push(error);
            }
        }
    }

    if errors.is_empty() {
        if let Err(error) = crate::channel_analysis::validate_endpoint_lifecycles(typed) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub(crate) fn type_node(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
) -> Result<TypedExpr, TypecheckError> {
    type_node_with_expectation(typed, resolved, context, node, None)
}

/// Type a `channel[tx]` / `channel[rx]` endpoint access.
///
/// `channel[tx]` yields a first-class, clone-capable `chn[tx, T]` sender value.
/// `channel[rx]` is normally a blocking receive yielding `opt[T]`; but when the
/// expected type is a `chn[rx, T]` receiver value and the source is a full
/// channel, it instead transfers the channel's unique receiver as that
/// first-class move-only value (V3_MEM §8.2). Receiving from a moved receiver
/// value (`receiver[rx]`) is again an `opt[T]` blocking receive.
#[allow(clippy::too_many_arguments)]
fn type_channel_access(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    channel: &AstNode,
    endpoint: &ChannelEndpoint,
    expected_type: Option<CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    if !typed.capability_model().supports_processor() {
        return Err(unsupported_node_surface(
            resolved,
            node,
            "channel endpoint access requires hosted std support; declare the bundled internal standard dependency",
        ));
    }
    helpers::require_direct_channel_binding(resolved, context.scope_id, channel)?;
    if matches!(endpoint, ChannelEndpoint::Rx) {
        reject_sender_capture_receive(typed, resolved, channel)?;
    }
    let channel_raw = type_node(typed, resolved, context, channel)?;
    let channel_type = plain_value_expr(
        typed,
        context,
        channel_raw,
        node_origin(resolved, channel),
        "channel endpoint receiver",
    )?
    .required_value("channel endpoint receiver does not have a type")?;
    let element_type = match endpoint {
        ChannelEndpoint::Tx => helpers::channel_element_type(typed, channel_type)?,
        ChannelEndpoint::Rx => helpers::channel_receiver_element_type(typed, channel_type)?,
    };
    match endpoint {
        ChannelEndpoint::Tx => Ok(TypedExpr::value(
            typed
                .type_table_mut()
                .intern(CheckedType::ChannelSender { element_type }),
        )),
        ChannelEndpoint::Rx => {
            // A `chn[rx, T]`-typed context over a full channel transfers the
            // unique receiver as a first-class value; otherwise this is a
            // blocking receive shell.
            let source_is_full_channel = matches!(
                helpers::apparent_type_id(typed, channel_type)
                    .ok()
                    .and_then(|apparent| typed.type_table().get(apparent)),
                Some(CheckedType::Channel { .. })
            );
            if source_is_full_channel && expected_is_channel_receiver(typed, expected_type) {
                Ok(TypedExpr::value(
                    typed
                        .type_table_mut()
                        .intern(CheckedType::ChannelReceiver { element_type }),
                ))
            } else {
                // A blocking receive is a shell: `opt[T]` whose present branch
                // owns a fresh payload and whose `nil` means every sender has
                // closed (V3_MEM §8.2). It blocks, so a live guard value cannot
                // cross it (V3_MEM §8.3).
                helpers::reject_bound_guard_boundary(
                    typed,
                    "blocking receive",
                    node_origin(resolved, node),
                )?;
                Ok(TypedExpr::value(typed.type_table_mut().intern(
                    CheckedType::Optional {
                        inner: element_type,
                    },
                )))
            }
        }
    }
}

fn expected_is_channel_receiver(
    typed: &TypedProgram,
    expected_type: Option<CheckedTypeId>,
) -> bool {
    let Some(expected_type) = expected_type else {
        return false;
    };
    let Ok(apparent) = helpers::apparent_type_id(typed, expected_type) else {
        return false;
    };
    matches!(
        typed.type_table().get(apparent),
        Some(CheckedType::ChannelReceiver { .. })
    )
}

pub(crate) fn reject_sender_capture_receive(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    channel: &AstNode,
) -> Result<(), TypecheckError> {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        name,
    } = helpers::strip_comments(channel)
    else {
        return Ok(());
    };
    let sender_only = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
        .and_then(|symbol| typed.typed_symbol(symbol))
        .is_some_and(|symbol| symbol.is_channel_sender_capture);
    if !sender_only {
        return Ok(());
    }
    Err(unsupported_node_surface(
        resolved,
        channel,
        format!(
            "captured endpoint '{name}[tx]' is sender-only; keep '{name}[rx]' in the owning receiving routine"
        ),
    ))
}

pub(crate) fn reject_direct_spawn_channel_receiver(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    task: &AstNode,
) -> Result<(), TypecheckError> {
    let (target, name) = match helpers::strip_comments(task) {
        AstNode::FunctionCall {
            syntax_id: Some(syntax_id),
            name,
            ..
        } => (
            resolved
                .references
                .iter()
                .find(|reference| {
                    reference.syntax_id == Some(*syntax_id)
                        && reference.kind == ReferenceKind::FunctionCall
                })
                .and_then(|reference| reference.resolved),
            name.clone(),
        ),
        AstNode::QualifiedFunctionCall { path, .. } => {
            let Some(syntax_id) = path.syntax_id() else {
                return Ok(());
            };
            (
                resolved
                    .references
                    .iter()
                    .find(|reference| {
                        reference.syntax_id == Some(syntax_id)
                            && reference.kind == ReferenceKind::QualifiedFunctionCall
                    })
                    .and_then(|reference| reference.resolved),
                path.joined(),
            )
        }
        AstNode::MethodCall {
            syntax_id: Some(syntax_id),
            method,
            ..
        } => (typed.method_call_target(*syntax_id), method.clone()),
        _ => return Ok(()),
    };
    let receiver_params = target
        .and_then(|symbol| typed.typed_symbol(symbol))
        .map(|symbol| &symbol.channel_receiver_params);
    if receiver_params.is_none_or(|params| params.is_empty()) {
        return Ok(());
    }
    Err(unsupported_node_surface(
        resolved,
        task,
        format!(
            "routine '{name}' receives from a channel and cannot be spawned directly; keep the single receiver in the owning routine and spawn sender-only producers"
        ),
    ))
}

pub(crate) fn require_named_processor_call_target(
    resolved: &ResolvedProgram,
    task: &AstNode,
    surface: &str,
) -> Result<(), TypecheckError> {
    let (syntax_id, reference_kind) = match helpers::strip_comments(task) {
        AstNode::FunctionCall {
            syntax_id: Some(syntax_id),
            ..
        } => (*syntax_id, ReferenceKind::FunctionCall),
        AstNode::QualifiedFunctionCall { path, .. } => {
            let Some(syntax_id) = path.syntax_id() else {
                return Ok(());
            };
            (syntax_id, ReferenceKind::QualifiedFunctionCall)
        }
        _ => return Ok(()),
    };
    let target_kind = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(syntax_id) && reference.kind == reference_kind
        })
        .and_then(|reference| reference.resolved)
        .and_then(|symbol| resolved.symbol(symbol))
        .map(|symbol| symbol.kind);
    if target_kind.is_none() || target_kind == Some(SymbolKind::Routine) {
        return Ok(());
    }
    Err(unsupported_node_surface(
        resolved,
        task,
        format!(
            "{surface} requires a direct call to a named routine declaration in V3; indirect routine values, stored anonymous routines, and routine parameters are not supported"
        ),
    ))
}

fn processor_call_syntax_id(task: &AstNode) -> Option<fol_parser::ast::SyntaxNodeId> {
    match helpers::strip_comments(task) {
        AstNode::FunctionCall { syntax_id, .. } => *syntax_id,
        AstNode::QualifiedFunctionCall { path, .. } => path.syntax_id(),
        _ => None,
    }
}

pub(crate) fn apply_spawn_argument_boundary(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    task: &AstNode,
    detached: bool,
) -> Result<(), TypecheckError> {
    let task = helpers::strip_comments(task);

    // Ordinary call typing has already inferred and recorded the concrete
    // signature for direct calls. Re-bind the arguments here so omitted
    // defaults cross exactly the same processor boundary as explicit values.
    // Reading the instantiated signature also distinguishes a safe concrete
    // generic call from forwarding an unresolved generic value.
    let direct_call: Option<(_, Cow<'_, str>, &[AstNode])> = match task {
        AstNode::FunctionCall {
            syntax_id: Some(syntax_id),
            name,
            args,
            ..
        } => Some((*syntax_id, Cow::Borrowed(name.as_str()), args.as_slice())),
        AstNode::QualifiedFunctionCall { path, args } => path
            .syntax_id()
            .map(|syntax_id| (syntax_id, Cow::Owned(path.joined()), args.as_slice())),
        _ => None,
    };
    if let Some((syntax_id, name, args)) = direct_call {
        if let Some(signature) = typed.call_signature(syntax_id).cloned() {
            let bound_args = calls::bind_call_arguments(
                &signature,
                args,
                name.as_ref(),
                node_origin(resolved, task),
                true,
                true,
            )?;
            for (index, (parameter_type, bound_arg)) in
                signature.params.iter().zip(bound_args.iter()).enumerate()
            {
                let boundary_value = match bound_arg {
                    calls::BoundCallArg::Explicit(arg)
                    | calls::BoundCallArg::VariadicUnpack(arg) => Some(*arg),
                    calls::BoundCallArg::VariadicPack(args) => args.first().copied(),
                    calls::BoundCallArg::Default => {
                        signature.param_defaults.get(index).and_then(Option::as_ref)
                    }
                }
                .unwrap_or(task);
                validate_processor_boundary_type(
                    typed,
                    resolved,
                    *parameter_type,
                    boundary_value,
                    detached,
                )?;
            }
        }
    }

    let mut boundary_values = Vec::new();
    match task {
        AstNode::FunctionCall { args, .. } | AstNode::QualifiedFunctionCall { args, .. } => {
            boundary_values.extend(args);
        }
        AstNode::MethodCall { object, args, .. } => {
            boundary_values.push(object.as_ref());
            boundary_values.extend(args);
        }
        _ => return Ok(()),
    };
    for arg in boundary_values {
        let arg = match helpers::strip_comments(arg) {
            AstNode::NamedArgument { value, .. } => helpers::strip_comments(value),
            other => other,
        };
        let syntax_id = arg.syntax_id();
        let resolved_reference = syntax_id.and_then(|syntax_id| {
            resolved.references.iter().find(|reference| {
                reference.syntax_id == Some(syntax_id)
                    && reference.kind == ReferenceKind::Identifier
            })
        });
        let resolved_type = resolved_reference
            .and_then(|reference| typed.typed_reference(reference.id))
            .and_then(|reference| reference.resolved_type)
            .or_else(|| {
                syntax_id
                    .and_then(|syntax_id| typed.typed_node(syntax_id))
                    .and_then(|node| node.inferred_type)
            });
        let direct_borrow = matches!(
            helpers::strip_comments(arg),
            AstNode::UnaryOp {
                op: UnaryOperator::BorrowFrom,
                ..
            }
        );
        if direct_borrow
            || resolved_type.is_some_and(|type_id| helpers::type_contains_borrowed(typed, type_id))
        {
            return Err(node_origin(resolved, arg).map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::Ownership,
                        "borrowed values cannot cross a spawn or async thread boundary; pass a clonable stack value or move an owned value",
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Ownership,
                        "borrowed values cannot cross a spawn or async thread boundary; pass a clonable stack value or move an owned value",
                        origin,
                    )
                },
            ));
        }
        let Some(resolved_type) = resolved_type else {
            continue;
        };
        if helpers::type_contains_shared_pointer(typed, resolved_type) {
            return Err(node_origin(resolved, arg).map_or_else(
                || {
                    TypecheckError::new(
                        TypecheckErrorKind::Ownership,
                        "values containing shared Rc pointers cannot cross a spawn or async thread boundary; use mux[T] data that contains only thread-safe values",
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Ownership,
                        "values containing shared Rc pointers cannot cross a spawn or async thread boundary; use mux[T] data that contains only thread-safe values",
                        origin,
                    )
                },
            ));
        }
        if matches!(
            typed.type_table().get(resolved_type),
            Some(CheckedType::Owned { .. })
        ) {
            if let (Some(symbol), Some(origin)) = (
                resolved_reference.and_then(|reference| reference.resolved),
                node_origin(resolved, arg),
            ) {
                typed.mark_binding_moved(symbol, origin);
            }
        }
    }
    Ok(())
}

fn validate_processor_boundary_type(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    type_id: CheckedTypeId,
    value: &AstNode,
    detached: bool,
) -> Result<(), TypecheckError> {
    let message = if detached && helpers::type_is_eventual(typed, type_id) {
        Some(
            "an eventual handle cannot enter a detached task ('[spn, det]') because it is bound to its parent scope; await it first, or spawn a scoped '[spn]' task instead",
        )
    } else if helpers::type_contains_fin(typed, type_id) {
        Some(
            "a 'fin' value cannot cross a spawn or async task boundary because it is not 'send'; the foreign resource it finalizes is not proven thread-safe",
        )
    } else if decls::checked_type_contains_generic_param(typed, type_id) {
        Some(
            "unconstrained generic values cannot cross a spawn or async thread boundary because FOL does not yet define a thread-safety and lifetime contract; use a concrete thread-safe value",
        )
    } else if helpers::type_contains_borrowed(typed, type_id) {
        Some(
            "borrowed values cannot cross a spawn or async thread boundary; pass a clonable stack value or move an owned value",
        )
    } else if helpers::type_contains_shared_pointer(typed, type_id) {
        Some(
            "values containing shared Rc pointers cannot cross a spawn or async thread boundary; use mux[T] data that contains only thread-safe values",
        )
    } else {
        None
    };
    let Some(message) = message else {
        return Ok(());
    };
    Err(node_origin(resolved, value).map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Ownership, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin),
    ))
}

fn reject_unsupported_spawn_task_surface(
    resolved: &ResolvedProgram,
    task: &AstNode,
) -> Result<(), TypecheckError> {
    match helpers::strip_comments(task) {
        AstNode::FunctionCall {
            surface: CallSurface::Plain,
            ..
        }
        | AstNode::QualifiedFunctionCall { .. } => Ok(()),
        AstNode::AnonymousFun { params, .. }
        | AstNode::AnonymousPro { params, .. }
        | AstNode::AnonymousLog { params, .. }
            if params.is_empty() =>
        {
            Ok(())
        }
        AstNode::AnonymousFun { .. }
        | AstNode::AnonymousPro { .. }
        | AstNode::AnonymousLog { .. } => Err(unsupported_node_surface(
            resolved,
            task,
            "a directly spawned anonymous routine cannot declare call parameters",
        )),
        _ => Err(unsupported_node_surface(
            resolved,
            task,
            "spawn requires a direct named routine call or a zero-parameter anonymous routine in V3; method calls and other expressions are not supported",
        )),
    }
}

pub(crate) fn type_node_with_expectation(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    expected_type: Option<CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    // Expression typing recurses with the AST; debug frames here are large
    // enough that ~18 nesting levels exhaust a 2 MB worker-thread stack.
    // Grow the stack in segments (rustc's ensure_sufficient_stack pattern).
    let result = stacker::maybe_grow(256 * 1024, 4 * 1024 * 1024, || {
        type_node_with_expectation_inner(typed, resolved, context, node, expected_type)
    })?;
    if let Some(type_id) = result.value_type {
        helpers::reject_embedded_full_channel(typed, type_id, node_origin(resolved, node))?;
    }
    Ok(result)
}

fn type_node_with_expectation_inner(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    expected_type: Option<CheckedTypeId>,
) -> Result<TypedExpr, TypecheckError> {
    match node {
        AstNode::Comment { .. } => Ok(TypedExpr::none()),
        AstNode::Commented { node, .. } => {
            type_node_with_expectation(typed, resolved, context, node, expected_type)
        }
        AstNode::BinaryOp { op, left, right } => {
            operators::type_binary_op(typed, resolved, context, op, left, right)
        }
        AstNode::UnaryOp { op, operand } => {
            operators::type_unary_op(typed, resolved, context, node, op, operand, expected_type)
        }
        AstNode::OwnershipOp {
            options, operand, ..
        } => operators::type_ownership_op(
            typed,
            resolved,
            context,
            node,
            options,
            operand,
            expected_type,
        ),
        AstNode::ChannelAccess { channel, endpoint } => type_channel_access(
            typed,
            resolved,
            context,
            node,
            channel,
            endpoint,
            expected_type,
        ),
        AstNode::VarDecl {
            name,
            type_hint: _,
            value,
            options,
            ..
        }
        | AstNode::LabDecl {
            name,
            type_hint: _,
            value,
            options,
            ..
        } => {
            let _ = bindings::type_binding_initializer(
                typed,
                resolved,
                context,
                name,
                value.as_deref(),
                binding_kind_for(node),
                options
                    .iter()
                    .any(|option| matches!(option, fol_parser::ast::VarOption::New)),
                options
                    .iter()
                    .any(|option| matches!(option, fol_parser::ast::VarOption::Borrowing)),
                options
                    .iter()
                    .any(|option| matches!(option, fol_parser::ast::VarOption::Mutable)),
            )?;
            Ok(TypedExpr::none())
        }
        AstNode::Literal(literal) => Ok(TypedExpr::value(literals::type_literal(
            typed,
            resolved,
            node,
            literal,
            expected_type,
        )?)),
        AstNode::ContainerLiteral {
            container_type,
            elements,
        } => literals::type_container_literal(
            typed,
            resolved,
            context,
            container_type.clone(),
            elements,
            expected_type,
        ),
        AstNode::RecordInit {
            syntax_id: _,
            fields,
        } => bindings::type_record_init(typed, resolved, context, fields, expected_type),
        AstNode::Identifier { name, syntax_id } => {
            references::type_identifier_reference(typed, resolved, context, name, *syntax_id)
        }
        AstNode::QualifiedIdentifier { path } => {
            references::type_qualified_identifier_reference(typed, resolved, context, path)
        }
        AstNode::AsyncStage => Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            if typed.capability_model().supports_processor() {
                "async is a pipe stage and must appear as call() | async"
            } else {
                "async pipe stages require hosted std support; declare the bundled internal standard dependency"
            },
        )),
        AstNode::AwaitStage => Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            if typed.capability_model().supports_processor() {
                "await is a pipe stage and must appear as eventual | await"
            } else {
                "await pipe stages require hosted std support; declare the bundled internal standard dependency"
            },
        )),
        AstNode::Spawn { task, detached } => {
            if !typed.capability_model().supports_processor() {
                return Err(unsupported_node_surface(
                    resolved,
                    node,
                    "spawn requires hosted std support; declare the bundled internal standard dependency",
                ));
            }
            require_named_processor_call_target(resolved, task, "spawn")?;
            reject_direct_spawn_channel_receiver(typed, resolved, task)?;
            helpers::reject_bound_guard_boundary(typed, "spawn", node_origin(resolved, node))?;
            let observed = type_node_with_expectation(
                typed,
                resolved,
                TypeContext {
                    error_call_mode: ErrorCallMode::Observe,
                    processor_task_call: processor_call_syntax_id(task),
                    ..context
                },
                task,
                None,
            )?;
            // Method targets are selected by call typing, so their channel
            // receiver effect can only be checked after the task is typed.
            reject_direct_spawn_channel_receiver(typed, resolved, task)?;
            let anonymous_recoverable = matches!(
                helpers::strip_comments(task),
                AstNode::AnonymousFun { .. }
                    | AstNode::AnonymousPro { .. }
                    | AstNode::AnonymousLog { .. }
            ) && observed
                .value_type
                .and_then(|type_id| typed.type_table().get(type_id))
                .is_some_and(|typ| {
                    matches!(typ, CheckedType::Routine(signature) if signature.error_type.is_some())
                });
            if observed.recoverable_effect.is_some() || anonymous_recoverable {
                return Err(node_origin(resolved, node).map_or_else(
                    || TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "a spawn ('[spn]'/'[spn, det]'/'[>]') cannot spawn a recoverable routine because it discards the error; make the callee infallible, or drop the spawn marker and use 'call() | async', then await and handle it",
                    ),
                    |origin| TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        "a spawn ('[spn]'/'[spn, det]'/'[>]') cannot spawn a recoverable routine because it discards the error; make the callee infallible, or drop the spawn marker and use 'call() | async', then await and handle it",
                        origin,
                    ),
                ));
            }
            apply_spawn_argument_boundary(typed, resolved, task, *detached)?;
            reject_unsupported_spawn_task_surface(resolved, task)?;
            Ok(TypedExpr::none())
        }
        AstNode::FunDecl {
            name,
            syntax_id,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::ProDecl {
            name,
            syntax_id,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::LogDecl {
            name,
            syntax_id,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            if let Some(message) =
                decls::unsupported_routine_param_surface_message(params, typed.capability_model())
            {
                return Err(unsupported_node_surface(resolved, node, message));
            }
            let routine_scope = syntax_id
                .and_then(|syntax_id| resolved.scope_for_syntax(syntax_id))
                .ok_or_else(|| {
                    TypecheckError::new(
                        TypecheckErrorKind::ScopeResolutionFailed,
                        format!("routine '{name}' has no scope mapping in the resolved program"),
                    )
                })?;
            let expected_return_type = match return_type.as_ref() {
                None | Some(FolType::None) => None,
                Some(ty) => Some(decls::lower_type(typed, resolved, routine_scope, ty)?),
            };
            let expected_error_type = error_type
                .as_ref()
                .map(|ty| decls::lower_type(typed, resolved, routine_scope, ty))
                .transpose()?;
            let routine_context = TypeContext {
                source_unit_id: context.source_unit_id,
                scope_id: routine_scope,
                routine_return_type: expected_return_type,
                routine_error_type: expected_error_type,
                error_call_mode: ErrorCallMode::Propagate,
                processor_task_call: None,
                allow_mutex_handle: false,
                repeating_loop_scope: None,
                inside_deferred_block: false,
                inside_error_deferred_block: false,
                field_projection_root: false,
            };
            type_routine_param_defaults(typed, resolved, routine_context, params)?;
            let body_type = type_body(typed, resolved, routine_context, body)?;
            let _ = type_body(typed, resolved, routine_context, inquiries)?;
            // Functions with a declared return type require explicit 'return' on all paths
            let routine_origin = syntax_id.and_then(|id| origin_for(resolved, id));
            if expected_return_type.is_some() && !body_type.is_never(typed) {
                let err = TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    format!("routine '{name}' declares a return type but not all code paths use 'return'"),
                );
                return Err(match routine_origin.clone() {
                    Some(o) => err.with_fallback_origin(o),
                    None => err,
                });
            }
            // Functions with T/E must use both 'return' and 'report'
            if expected_error_type.is_some() {
                if !body_contains_return(body) {
                    let err = TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        format!("routine '{name}' declares an error type — both 'return' and 'report' are required"),
                    );
                    return Err(match routine_origin.clone() {
                        Some(o) => err.with_fallback_origin(o),
                        None => err,
                    });
                }
                if !body_contains_report(body) {
                    let err = TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        format!("routine '{name}' declares an error type — both 'return' and 'report' are required"),
                    );
                    return Err(match routine_origin {
                        Some(o) => err.with_fallback_origin(o),
                        None => err,
                    });
                }
            }
            if let (Some(syntax_id), Some(type_id)) =
                (syntax_id, expected_return_type.or(body_type.value_type))
            {
                typed.record_node_type(*syntax_id, context.source_unit_id, type_id)?;
            }
            Ok(body_type)
        }
        AstNode::AnonymousFun {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::AnonymousPro {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::AnonymousLog {
            syntax_id,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            let anonymous_channel_message = "anonymous routine chn[T] parameters are not supported in V3; use a named routine so channel effects can be refined, or capture an existing c[tx] sender explicitly";
            if let Some(message) =
                decls::unsupported_routine_param_surface_message(params, typed.capability_model())
            {
                return Err(unsupported_node_surface(resolved, node, message));
            }
            if params.iter().any(|param| param.is_mutex) {
                return Err(unsupported_node_surface(
                    resolved,
                    node,
                    "anonymous routines with mux[T] parameters are not supported in V3; use a named routine and call it directly so the mutex ABI remains explicit",
                ));
            }
            if params
                .iter()
                .any(|param| matches!(&param.param_type, FolType::Channel { .. }))
            {
                return Err(unsupported_node_surface(
                    resolved,
                    node,
                    anonymous_channel_message,
                ));
            }
            for param in params {
                if !anonymous_routine_type_is_lowerable(&param.param_type) {
                    return Err(unsupported_node_surface(
                        resolved,
                        node,
                        "complex type annotation in anonymous routine is not yet supported",
                    ));
                }
            }
            for typ in return_type.as_ref().into_iter().chain(error_type.as_ref()) {
                if !anonymous_routine_type_is_lowerable(typ) {
                    return Err(unsupported_node_surface(
                        resolved,
                        node,
                        "complex type annotation in anonymous routine is not yet supported",
                    ));
                }
            }
            let routine_scope = syntax_id
                .and_then(|id| resolved.scope_for_syntax(id))
                .unwrap_or(context.scope_id);
            let mut lowered_params = Vec::with_capacity(captures.len() + params.len());
            for capture in captures {
                if matches!(capture.endpoint, Some(ChannelEndpoint::Rx)) {
                    return Err(unsupported_node_surface(
                        resolved,
                        node,
                        "a channel receiver cannot be cloned into a spawned capture; capture c[tx] and keep c[rx] in the receiving routine",
                    ));
                }
                // Validate the capture *form* before resolving the outer binding
                // so an untagged capture reports the surface error rather than a
                // "lost its outer binding" internal error.
                if capture.endpoint.is_none() && capture.operation.is_none() {
                    return Err(unsupported_node_surface(
                        resolved,
                        node,
                        format!(
                            "anonymous capture '{}' must state a channel endpoint ('{}[tx]') or a value operation ('{}[mov]')",
                            capture.name, capture.name, capture.name
                        ),
                    ));
                }
                let outer_symbol = [
                    fol_resolver::SymbolKind::ValueBinding,
                    fol_resolver::SymbolKind::Parameter,
                    fol_resolver::SymbolKind::Capture,
                ]
                .into_iter()
                .find_map(|kind| {
                    helpers::find_symbol_in_scope_chain(
                        resolved,
                        context.source_unit_id,
                        context.scope_id,
                        &capture.name,
                        kind,
                    )
                })
                .ok_or_else(|| {
                    internal_error(
                        format!("capture '{}' lost its outer binding", capture.name),
                        node_origin(resolved, node),
                    )
                })?;
                let capture_type = typed
                    .typed_symbol(outer_symbol)
                    .and_then(|symbol| symbol.declared_type)
                    .ok_or_else(|| {
                        TypecheckError::new(
                            TypecheckErrorKind::InvalidInput,
                            format!("capture '{}' does not retain a type", capture.name),
                        )
                    })?;
                let capture_symbol = decls::find_symbol_id_in_scope(
                    resolved,
                    context.source_unit_id,
                    routine_scope,
                    &[fol_resolver::SymbolKind::Capture],
                    &capture.name,
                )?;
                // A capture states either a channel endpoint (`c[tx]`, cloned as
                // a sender) or a value operation (`data[mov]`, moved whole into
                // the task environment). The two are mutually exclusive by
                // construction (the parser accepts one bracket form). An
                // untagged value capture is rejected: §2.2 requires the capture
                // boundary to state its transfer.
                let lowered_capture_type = match (capture.endpoint.as_ref(), capture.operation) {
                    (Some(ChannelEndpoint::Tx), None) => {
                        let element_type = match typed.type_table().get(capture_type) {
                            Some(CheckedType::Channel { element_type })
                            | Some(CheckedType::ChannelSender { element_type }) => *element_type,
                            _ => {
                                return Err(unsupported_node_surface(
                                    resolved,
                                    node,
                                    format!(
                                        "capture '{}[tx]' requires a chn[T] binding",
                                        capture.name
                                    ),
                                ));
                            }
                        };
                        if helpers::type_contains_shared_pointer(typed, element_type) {
                            return Err(capture_spawn_send_error(
                                resolved,
                                node,
                                &format!("captured endpoint '{}[tx]'", capture.name),
                            ));
                        }
                        let sender_type = typed
                            .type_table_mut()
                            .intern(CheckedType::ChannelSender { element_type });
                        if let Some(symbol) = typed.typed_symbol_mut(capture_symbol) {
                            symbol.is_channel_sender_capture = true;
                        }
                        sender_type
                    }
                    (None, Some(OwnershipOption::Move)) => {
                        // Owned move capture: the whole value crosses the spawn
                        // boundary, so it must be thread-safe (V3_PROC "owned
                        // captures require send").
                        if helpers::type_contains_shared_pointer(typed, capture_type) {
                            return Err(capture_spawn_send_error(
                                resolved,
                                node,
                                &format!("moved capture '{}[mov]'", capture.name),
                            ));
                        }
                        // §2.2 capture transfer boundary: `[mov]` consumes the
                        // outer binding, so later use is a use-after-move.
                        if let Some(origin) = node_origin(resolved, node) {
                            typed.mark_binding_moved(outer_symbol, origin);
                        }
                        capture_type
                    }
                    (None, Some(OwnershipOption::Copy)) => {
                        // Copy capture: an independent copy crosses the spawn
                        // boundary and the outer binding stays usable. The value
                        // must be a `copy` type (§4.1) and thread-safe (V3_PROC).
                        if decls::type_lacks_copy(typed, capture_type)? {
                            return Err(with_node_origin(
                                resolved,
                                node,
                                TypecheckErrorKind::Ownership,
                                format!(
                                    "copied capture '{}[cpy]' requires a copy type; use '{}[mov]' or '{}[cln]' instead",
                                    capture.name, capture.name, capture.name
                                ),
                            ));
                        }
                        if helpers::type_contains_shared_pointer(typed, capture_type) {
                            return Err(capture_spawn_send_error(
                                resolved,
                                node,
                                &format!("copied capture '{}[cpy]'", capture.name),
                            ));
                        }
                        // A copy leaves the source live: no `mark_binding_moved`.
                        capture_type
                    }
                    (None, Some(OwnershipOption::Clone)) => {
                        // Clone capture: an independent clone crosses the spawn
                        // boundary and the outer binding stays usable. The value
                        // must be clonable (§4.1) and thread-safe (V3_PROC). The
                        // spawn arg lowers through the same LoadLocal materialization,
                        // which renders `.clone()` only for a non-move-only value —
                        // so a move-only value (which LoadLocal would consume via
                        // `std::mem::take`) is rejected here: it needs `[mov]`.
                        if bindings::ownership_moves_on_transfer(typed, capture_type) {
                            return Err(with_node_origin(
                                resolved,
                                node,
                                TypecheckErrorKind::Ownership,
                                format!(
                                    "cloned capture '{}[cln]' cannot clone a move-only value; use '{}[mov]' to transfer it",
                                    capture.name, capture.name
                                ),
                            ));
                        }
                        if decls::type_lacks_clone(typed, capture_type)? {
                            return Err(with_node_origin(
                                resolved,
                                node,
                                TypecheckErrorKind::Ownership,
                                format!(
                                    "cloned capture '{}[cln]' requires a clonable value",
                                    capture.name
                                ),
                            ));
                        }
                        if helpers::type_contains_shared_pointer(typed, capture_type) {
                            return Err(capture_spawn_send_error(
                                resolved,
                                node,
                                &format!("cloned capture '{}[cln]'", capture.name),
                            ));
                        }
                        // A clone leaves the source live: no `mark_binding_moved`.
                        capture_type
                    }
                    _ => {
                        // (None, None) is rejected above; any endpoint+operation
                        // mix is impossible (the parser accepts one bracket form).
                        return Err(internal_error(
                            format!(
                                "capture '{}' carried an unexpected endpoint/operation combination",
                                capture.name
                            ),
                            node_origin(resolved, node),
                        ));
                    }
                };
                decls::record_symbol_type(typed, capture_symbol, lowered_capture_type)?;
                lowered_params.push(lowered_capture_type);
            }
            for param in params {
                let param_type =
                    decls::lower_type(typed, resolved, routine_scope, &param.param_type)?;
                if matches!(
                    typed
                        .type_table()
                        .get(helpers::apparent_type_id(typed, param_type)?),
                    Some(CheckedType::Channel { .. })
                ) {
                    return Err(unsupported_node_surface(
                        resolved,
                        node,
                        anonymous_channel_message,
                    ));
                }
                if let Ok(param_symbol_id) = decls::find_symbol_id_in_scope(
                    resolved,
                    context.source_unit_id,
                    routine_scope,
                    &[fol_resolver::SymbolKind::Parameter],
                    &param.name,
                ) {
                    decls::record_symbol_type(typed, param_symbol_id, param_type)?;
                }
                lowered_params.push(param_type);
            }
            let expected_return_type = match return_type.as_ref() {
                None | Some(FolType::None) => None,
                Some(ty) => Some(decls::lower_type(typed, resolved, routine_scope, ty)?),
            };
            let expected_error_type = error_type
                .as_ref()
                .map(|ty| decls::lower_type(typed, resolved, routine_scope, ty))
                .transpose()?;
            let routine_context = TypeContext {
                source_unit_id: context.source_unit_id,
                scope_id: routine_scope,
                routine_return_type: expected_return_type,
                routine_error_type: expected_error_type,
                error_call_mode: ErrorCallMode::Propagate,
                processor_task_call: None,
                allow_mutex_handle: false,
                repeating_loop_scope: None,
                inside_deferred_block: false,
                inside_error_deferred_block: false,
                field_projection_root: false,
            };
            type_routine_param_defaults(typed, resolved, routine_context, params)?;
            let body_type = type_body(typed, resolved, routine_context, body)?;
            let _ = type_body(typed, resolved, routine_context, inquiries)?;
            // Anonymous routines with a declared return type require explicit 'return'
            let anon_origin = node_origin(resolved, node);
            if expected_return_type.is_some() && !body_type.is_never(typed) {
                let err = TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "anonymous routine declares a return type but not all code paths use 'return'",
                );
                return Err(match anon_origin.clone() {
                    Some(o) => err.with_fallback_origin(o),
                    None => err,
                });
            }
            // Anonymous routines with T/E must use both 'return' and 'report'
            if expected_error_type.is_some() {
                if !body_contains_return(body) {
                    let err = TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        "anonymous routine declares an error type — both 'return' and 'report' are required",
                    );
                    return Err(match anon_origin.clone() {
                        Some(o) => err.with_fallback_origin(o),
                        None => err,
                    });
                }
                if !body_contains_report(body) {
                    let err = TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        "anonymous routine declares an error type — both 'return' and 'report' are required",
                    );
                    return Err(match anon_origin {
                        Some(o) => err.with_fallback_origin(o),
                        None => err,
                    });
                }
            }
            let routine_type_id =
                typed
                    .type_table_mut()
                    .intern(CheckedType::Routine(RoutineType {
                        generic_params: Vec::new(),
                        generic_constraints: BTreeMap::new(),
                        param_names: vec![String::new(); lowered_params.len()],
                        param_defaults: vec![None; lowered_params.len()],
                        variadic_index: params.iter().position(|param| param.is_variadic),
                        mutex_params: params
                            .iter()
                            .enumerate()
                            .filter_map(|(index, param)| {
                                param.is_mutex.then_some(captures.len() + index)
                            })
                            .collect(),
                        params: lowered_params,
                        return_type: expected_return_type,
                        error_type: expected_error_type,
                    }));
            Ok(TypedExpr::value(routine_type_id))
        }
        AstNode::Block { statements, .. } => {
            let block_context = inline_body_block_scope(
                resolved,
                context.source_unit_id,
                context.scope_id,
                statements,
            )
            .map(|scope_id| TypeContext {
                scope_id,
                ..context
            })
            .unwrap_or(context);
            type_body(typed, resolved, block_context, statements)
        }
        AstNode::Program { declarations } => type_body(typed, resolved, context, declarations),
        AstNode::When {
            expr,
            cases,
            default,
        } => controlflow::type_when(typed, resolved, context, expr, cases, default.as_deref()),
        AstNode::Loop {
            syntax_id,
            condition,
            body,
        } => controlflow::type_loop(typed, resolved, context, *syntax_id, condition, body),
        AstNode::Assignment { target, value } => {
            ensure_assignable_target(
                typed,
                resolved,
                context.source_unit_id,
                context.scope_id,
                target,
            )?;
            let whole_target = whole_binding_assignment_symbol(resolved, target);
            if context.inside_deferred_block {
                if let Some(symbol) =
                    whole_target.filter(|symbol| typed.moved_binding_origin(*symbol).is_some())
                {
                    let name = resolved
                        .symbol(symbol)
                        .map(|symbol| symbol.name.as_str())
                        .unwrap_or("<unknown>");
                    let message = format!(
                        "moved binding '{name}' cannot be reinitialized inside dfr/edf because delayed ownership effects are not modeled; reinitialize it before registering the deferred block"
                    );
                    return Err(node_origin(resolved, target).map_or_else(
                        || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
                        |origin| {
                            TypecheckError::with_origin(
                                TypecheckErrorKind::Ownership,
                                message.clone(),
                                origin,
                            )
                        },
                    ));
                }
            }
            let expected = if let Some(symbol) =
                whole_target.filter(|symbol| typed.moved_binding_origin(*symbol).is_some())
            {
                if let Some(borrow) = typed.active_borrow_for_owner(symbol).cloned() {
                    let name = resolved
                        .symbol(symbol)
                        .map(|symbol| symbol.name.as_str())
                        .unwrap_or("<unknown>");
                    let message = format!(
                        "cannot reinitialize moved binding '{name}' while it remains borrowed"
                    );
                    let error = node_origin(resolved, target).map_or_else(
                        || TypecheckError::new(TypecheckErrorKind::OwnerBorrowed, message.clone()),
                        |origin| {
                            TypecheckError::with_origin(
                                TypecheckErrorKind::OwnerBorrowed,
                                message.clone(),
                                origin,
                            )
                        },
                    );
                    return Err(
                        error.with_related_origin(borrow.origin, "active borrow created here")
                    );
                }
                typed
                    .typed_symbol(symbol)
                    .and_then(|symbol| symbol.declared_type)
                    .ok_or_else(|| {
                        internal_error(
                            "moved assignment target lost its checked type",
                            node_origin(resolved, target),
                        )
                    })?
            } else {
                type_node(typed, resolved, context, target)?
                    .required_value("assignment target does not have a type")?
            };
            let actual =
                type_node_with_expectation(typed, resolved, context, value, Some(expected))?
                    .required_value("assignment value does not have a type")?;
            bindings::reject_nested_eventual_value(
                typed,
                actual,
                node_origin(resolved, value),
                "assignment",
            )?;
            ensure_assignable(typed, expected, actual, "assignment".to_string(), None)?;
            if let Some(symbol) = whole_target {
                bindings::reject_recoverable_eventual_overwrite(
                    typed,
                    resolved,
                    symbol,
                    node_origin(resolved, target),
                )?;
            }
            bindings::track_value_transfer(typed, resolved, context, Some(value), actual)?;
            if let Some(symbol) = whole_target {
                typed.mark_binding_reinitialized(symbol, node_origin(resolved, target));
                bindings::register_recoverable_eventual_binding(
                    typed,
                    symbol,
                    actual,
                    context.scope_id,
                    node_origin(resolved, target),
                );
            }
            // Assignment is a statement. Treating it as the assigned value is
            // especially unsound for move-only values: lowering stores the
            // value into the target, so a second expression use would move the
            // same temporary twice.
            Ok(TypedExpr::none())
        }
        AstNode::FunctionCall {
            surface: CallSurface::DotIntrinsic,
            name,
            args,
            syntax_id,
            ..
        } => calls::type_dot_intrinsic_call(
            typed,
            resolved,
            context,
            name,
            args,
            *syntax_id,
            expected_type,
        ),
        AstNode::FunctionCall {
            name,
            args,
            syntax_id,
            ..
        } if name == "report" => {
            if context.inside_deferred_block {
                return Err(with_node_origin(
                    resolved,
                    node,
                    TypecheckErrorKind::InvalidInput,
                    "report is not allowed inside dfr/edf blocks; deferred cleanup cannot initiate a recoverable error exit",
                ));
            }
            calls::type_report_call(typed, resolved, context, args, *syntax_id)
        }
        AstNode::FunctionCall {
            name,
            args,
            type_args,
            syntax_id,
            ..
        } => {
            if let Ok(entry) = select_intrinsic(IntrinsicSurface::KeywordCall, name) {
                calls::type_keyword_intrinsic_call(
                    typed, resolved, context, entry, args, *syntax_id,
                )
            } else {
                calls::type_function_call(
                    typed, resolved, context, name, type_args, args, *syntax_id,
                )
            }
        }
        AstNode::QualifiedFunctionCall { path, args } => {
            calls::type_qualified_function_call(typed, resolved, context, path, args)
        }
        AstNode::MethodCall {
            object,
            method,
            args,
            ..
        } => calls::type_method_call(typed, resolved, context, node, object, method, args),
        AstNode::FieldAccess { object, field } => {
            access::type_field_access(typed, resolved, context, object, field, expected_type)
        }
        AstNode::IndexAccess { container, index } => {
            if matches!(
                helpers::strip_comments(container),
                AstNode::ChannelAccess {
                    endpoint: ChannelEndpoint::Rx,
                    ..
                }
            ) {
                Err(unsupported_node_surface(
                    resolved,
                    node,
                    "channel receivers are blocking pull expressions and cannot be indexed; use 'var value = channel[rx]' or iterate 'for value in channel[rx]'",
                ))
            } else {
                access::type_index_access(typed, resolved, context, container, index)
            }
        }
        AstNode::SliceAccess {
            container,
            start,
            end,
            ..
        } => access::type_slice_access(
            typed,
            resolved,
            context,
            container,
            start.as_deref(),
            end.as_deref(),
        ),
        AstNode::PatternAccess {
            container,
            patterns,
        } => access::type_inner_place_access(typed, resolved, context, container, patterns),
        AstNode::Rolling { .. } => Err(unsupported_node_surface(
            resolved,
            node,
            "rolling/comprehension expressions are not yet supported",
        )),
        AstNode::Range { .. } => Err(unsupported_node_surface(
            resolved,
            node,
            "range expressions are not yet supported",
        )),
        AstNode::Select { arms, default, .. } => {
            controlflow::type_select(typed, resolved, context, node, arms, default.as_deref())
        }
        AstNode::Return { value } => controlflow::type_return(
            typed,
            resolved,
            context,
            value.as_deref(),
            node_origin(resolved, node),
        ),
        AstNode::Break => {
            if let Some(loop_scope) = context.repeating_loop_scope {
                helpers::reject_recoverable_eventuals_leaving_scope(
                    typed,
                    resolved,
                    loop_scope,
                    node_origin(resolved, node),
                    "leaving the loop with break",
                )?;
            }
            Ok(TypedExpr::value(typed.builtin_types().never))
        }
        AstNode::Dfr { syntax_id, body } | AstNode::Edf { syntax_id, body } => {
            if body_contains_return(body) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "return is not allowed inside dfr/edf blocks",
                ));
            }
            if body_contains_break(body) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "break is not allowed inside dfr/edf blocks",
                ));
            }
            // Deferred blocks replay at every exit; a diverging terminator
            // inside one cannot be lowered against the surrounding exit.
            if body_contains_panic(body) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "panic is not allowed inside dfr/edf blocks",
                ));
            }
            let deferred_scope = syntax_id
                .and_then(|syntax_id| resolved.scope_for_syntax(syntax_id))
                .ok_or_else(|| {
                    internal_error("dfr/edf block lost its resolved lexical scope", None)
                })?;
            let _ = type_body(
                typed,
                resolved,
                TypeContext {
                    scope_id: deferred_scope,
                    inside_deferred_block: true,
                    inside_error_deferred_block: context.inside_error_deferred_block
                        || matches!(node, AstNode::Edf { .. }),
                    ..context
                },
                body,
            )?;
            register_deferred_outer_binding_uses(typed, resolved, context.scope_id, body)?;
            Ok(TypedExpr::none())
        }
        AstNode::Yield { .. } => Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            "yield expressions are not yet supported",
        )),
        AstNode::Invoke { callee, args } => {
            let callee_expr = type_node(typed, resolved, context, callee)?;
            let callee_type_id = callee_expr.value_type.ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "invoke callee expression does not produce a value",
                )
            })?;
            let signature = match typed.type_table().get(callee_type_id) {
                Some(CheckedType::Routine(sig)) => sig.clone(),
                _ => {
                    return Err(TypecheckError::new(
                        TypecheckErrorKind::InvalidInput,
                        format!(
                            "invoke callee is not a callable routine type (found {})",
                            describe_type(typed, callee_type_id)
                        ),
                    ));
                }
            };
            let (signature, arg_effect) = calls::check_call_arguments(
                typed,
                resolved,
                context,
                &signature,
                args,
                "<invoke>",
                node_origin(resolved, node),
                false,
                false,
                false,
            )?;
            let call_effect = helpers::merge_recoverable_effects(
                typed,
                node_origin(resolved, node),
                "invoke",
                [
                    arg_effect,
                    signature
                        .error_type
                        .map(|error_type| RecoverableCallEffect { error_type }),
                ],
            )?;
            Ok(TypedExpr::maybe_value(signature.return_type).with_optional_effect(call_effect))
        }
        AstNode::TemplateCall { .. } => Err(unsupported_node_surface(
            resolved,
            node,
            "template instantiation is not yet supported",
        )),
        AstNode::AvailabilityAccess { .. } => Err(unsupported_node_surface(
            resolved,
            node,
            "availability access is not yet supported",
        )),
        AstNode::StdDecl { .. } => Ok(TypedExpr::none()),
        // Declaration-level constructs: type their children but produce no value.
        AstNode::UseDecl { .. }
        | AstNode::TypeDecl { .. }
        | AstNode::AliasDecl { .. }
        | AstNode::DefDecl { .. }
        | AstNode::SegDecl { .. }
        | AstNode::DestructureDecl { .. }
        | AstNode::NamedArgument { .. }
        | AstNode::Unpack { .. }
        | AstNode::PatternWildcard
        | AstNode::PatternCapture { .. }
        | AstNode::Inquiry { .. } => {
            for child in node.children() {
                let _ = type_node(typed, resolved, context, child)?;
            }
            Ok(TypedExpr::none())
        }
    }
}

/// Check whether an AST body contains at least one `return` statement (non-recursive into nested routines).
fn body_contains_return(nodes: &[AstNode]) -> bool {
    nodes.iter().any(node_contains_return)
}

fn whole_binding_assignment_symbol(
    resolved: &ResolvedProgram,
    target: &AstNode,
) -> Option<SymbolId> {
    let (syntax_id, kind) = match helpers::strip_comments(target) {
        AstNode::Identifier {
            syntax_id: Some(syntax_id),
            ..
        } => (*syntax_id, ReferenceKind::Identifier),
        AstNode::QualifiedIdentifier { path } => {
            (path.syntax_id()?, ReferenceKind::QualifiedIdentifier)
        }
        _ => return None,
    };
    resolved
        .references
        .iter()
        .find(|reference| reference.syntax_id == Some(syntax_id) && reference.kind == kind)
        .and_then(|reference| reference.resolved)
}

fn body_contains_panic(nodes: &[AstNode]) -> bool {
    nodes.iter().any(node_contains_panic)
}

fn node_contains_panic(node: &AstNode) -> bool {
    match node {
        AstNode::FunctionCall { name, .. } if name == "panic" => true,
        AstNode::FunDecl { .. }
        | AstNode::ProDecl { .. }
        | AstNode::LogDecl { .. }
        | AstNode::AnonymousFun { .. }
        | AstNode::AnonymousPro { .. }
        | AstNode::AnonymousLog { .. } => false,
        _ => node
            .children()
            .iter()
            .any(|child| node_contains_panic(child)),
    }
}

fn anonymous_routine_type_is_lowerable(typ: &FolType) -> bool {
    match typ {
        FolType::Int { .. }
        | FolType::Float { .. }
        | FolType::Bool
        | FolType::Char { .. }
        | FolType::Never
        | FolType::Named { .. }
        | FolType::QualifiedNamed { .. } => true,
        ty if ty.is_builtin_str() => true,
        _ => false,
    }
}

fn body_contains_break(nodes: &[AstNode]) -> bool {
    nodes.iter().any(|node| match node {
        AstNode::Break => true,
        AstNode::Commented { node, .. } => body_contains_break(std::slice::from_ref(node.as_ref())),
        AstNode::Block { statements, .. } => body_contains_break(statements),
        AstNode::When { cases, default, .. } => {
            cases.iter().any(|case| match case {
                fol_parser::ast::WhenCase::Case { body, .. }
                | fol_parser::ast::WhenCase::Is { body, .. }
                | fol_parser::ast::WhenCase::In { body, .. }
                | fol_parser::ast::WhenCase::Has { body, .. }
                | fol_parser::ast::WhenCase::On { body, .. }
                | fol_parser::ast::WhenCase::Of { body, .. } => body_contains_break(body),
            }) || default
                .as_ref()
                .is_some_and(|body| body_contains_break(body))
        }
        AstNode::Loop { body, .. } | AstNode::Dfr { body, .. } | AstNode::Edf { body, .. } => {
            body_contains_break(body)
        }
        AstNode::FunDecl { .. }
        | AstNode::ProDecl { .. }
        | AstNode::LogDecl { .. }
        | AstNode::AnonymousFun { .. }
        | AstNode::AnonymousPro { .. }
        | AstNode::AnonymousLog { .. } => false,
        _ => false,
    })
}

fn node_contains_return(node: &AstNode) -> bool {
    match node {
        AstNode::Return { .. } => true,
        AstNode::FunDecl { .. }
        | AstNode::ProDecl { .. }
        | AstNode::LogDecl { .. }
        | AstNode::AnonymousFun { .. }
        | AstNode::AnonymousPro { .. }
        | AstNode::AnonymousLog { .. } => false,
        _ => node
            .children()
            .iter()
            .any(|child| node_contains_return(child)),
    }
}

/// Check whether an AST body contains at least one `report(...)` call (non-recursive into nested routines).
fn body_contains_report(nodes: &[AstNode]) -> bool {
    nodes.iter().any(node_contains_report)
}

fn node_contains_report(node: &AstNode) -> bool {
    match node {
        AstNode::FunctionCall { name, .. } if name == "report" => true,
        AstNode::FunDecl { .. }
        | AstNode::ProDecl { .. }
        | AstNode::LogDecl { .. }
        | AstNode::AnonymousFun { .. }
        | AstNode::AnonymousPro { .. }
        | AstNode::AnonymousLog { .. } => false,
        _ => node
            .children()
            .iter()
            .any(|child| node_contains_report(child)),
    }
}

fn type_routine_param_defaults(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    params: &[fol_parser::ast::Parameter],
) -> Result<(), TypecheckError> {
    for param in params {
        let Some(default) = param.default.as_ref() else {
            continue;
        };
        let expected = decls::lower_type(typed, resolved, context.scope_id, &param.param_type)?;
        let typed_default =
            type_node_with_expectation(typed, resolved, context, default, Some(expected))?;
        let typed_default = plain_value_expr(
            typed,
            context,
            typed_default,
            node_origin(resolved, default),
            format!("default value for parameter '{}'", param.name),
        )?;
        let actual = typed_default.required_value(format!(
            "default value for parameter '{}' does not have a type",
            param.name
        ))?;
        ensure_assignable(
            typed,
            expected,
            actual,
            format!("default value for parameter '{}'", param.name),
            node_origin(resolved, default),
        )?;
    }
    Ok(())
}

pub(crate) fn type_body(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    nodes: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    type_body_inner(typed, resolved, context, nodes, false)
}

pub(crate) fn type_body_transferring_value(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    nodes: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    type_body_inner(typed, resolved, context, nodes, true)
}

fn type_body_inner(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    nodes: &[AstNode],
    transfer_final_value: bool,
) -> Result<TypedExpr, TypecheckError> {
    // Non-lexical borrow lifetimes (Slice C): a borrow ends at its last use, not
    // at the end of its lexical scope. Precompute, per borrow-eligible binding,
    // the index of the last top-level statement in this body that references it;
    // after that statement completes the loan is released so the owner is usable
    // again. Uses nested in branches/loops attribute to their enclosing
    // top-level statement, which keeps the release conservative (a statement is
    // never split), so this is sound without a full CFG.
    let last_use = compute_last_statement_use(resolved, nodes);
    let result = (|| {
        let mut final_expr = TypedExpr::none();
        let mut pending_value = None;
        for (stmt_index, node) in nodes.iter().enumerate() {
            let node_result = type_node(typed, resolved, context, node);
            if let Some(error) = take_deferred_transfer_error(typed, resolved) {
                return Err(error);
            }
            let node_expr = node_result?;
            if let Some(actual) = node_expr.value_type {
                if let Some((previous_node, previous_expr)) = pending_value.take() {
                    reject_discarded_body_expr(typed, resolved, previous_node, previous_expr)?;
                }
                final_expr = node_expr;
                if node_expr.is_never(typed) {
                    return Ok(final_expr);
                }
                let transfer_result =
                    bindings::track_value_transfer(typed, resolved, context, Some(node), actual);
                if let Some(error) = take_deferred_transfer_error(typed, resolved) {
                    return Err(error);
                }
                transfer_result?;
                pending_value = Some((node, node_expr));
            } else if node_expr.recoverable_effect.is_some() {
                helpers::reject_recoverable_plain_use(
                    node_origin(resolved, node),
                    "statement-position expression",
                )?;
            }
            typed.release_scope_borrows_dead_after(context.scope_id, stmt_index, &last_use);
        }
        if !transfer_final_value {
            if let Some((node, expr)) = pending_value {
                reject_discarded_body_expr(typed, resolved, node, expr)?;
            }
        }
        Ok(final_expr)
    })();
    let result = match result {
        Ok(expr) if !expr.is_never(typed) => {
            helpers::reject_recoverable_eventuals_in_scope(typed, resolved, context.scope_id)
                .map(|()| expr)
        }
        other => other,
    };
    typed.release_borrows_in_scope(context.scope_id);
    typed.release_mutex_guards_in_scope(context.scope_id);
    typed.release_deferred_binding_uses_in_scope(context.scope_id);
    typed.release_recoverable_eventual_obligations_in_scope(context.scope_id);
    result
}

/// For each symbol referenced anywhere in `nodes`, record the highest index of
/// a top-level statement that references it. This is the last-use frontier used
/// to release non-lexical borrows (Slice C); a use nested inside a branch or
/// loop is attributed to the enclosing top-level statement.
fn compute_last_statement_use(
    resolved: &ResolvedProgram,
    nodes: &[AstNode],
) -> BTreeMap<SymbolId, usize> {
    fn collect(
        node: &AstNode,
        resolved: &ResolvedProgram,
        out: &mut std::collections::BTreeSet<SymbolId>,
    ) {
        if let AstNode::Identifier {
            syntax_id: Some(syntax_id),
            ..
        } = node
        {
            if let Some(symbol) = resolved
                .references
                .iter()
                .find(|reference| {
                    reference.syntax_id == Some(*syntax_id)
                        && reference.kind == ReferenceKind::Identifier
                })
                .and_then(|reference| reference.resolved)
            {
                out.insert(symbol);
            }
        }
        for child in node.children() {
            collect(child, resolved, out);
        }
    }

    let mut last_use = BTreeMap::new();
    for (index, node) in nodes.iter().enumerate() {
        let mut symbols = std::collections::BTreeSet::new();
        collect(node, resolved, &mut symbols);
        for symbol in symbols {
            last_use.insert(symbol, index);
        }
    }
    last_use
}

fn reject_discarded_body_expr(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    node: &AstNode,
    expr: TypedExpr,
) -> Result<(), TypecheckError> {
    if expr.recoverable_effect.is_some() {
        helpers::reject_recoverable_plain_use(
            node_origin(resolved, node),
            "statement-position expression",
        )?;
    }
    // A channel send yields a must-handle `err[T]` (V3_MEM §8.2). A bare send in
    // statement position silently drops that result — and with it the unsent
    // payload on a closed receiver — so it is rejected. Bind it (`var sent:
    // err[T] = ...`), inspect it with `when ... on ... *`, or propagate it.
    if is_channel_send_node(node) {
        return Err(node_origin(resolved, node).map_or_else(
            || {
                TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "a channel send returns a must-handle 'err[T]'; bind it, inspect it with 'when ... on ... *', or propagate it instead of discarding the unsent payload",
                )
            },
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::Unsupported,
                    "a channel send returns a must-handle 'err[T]'; bind it, inspect it with 'when ... on ... *', or propagate it instead of discarding the unsent payload",
                    origin,
                )
            },
        ));
    }
    if let Some(actual) = expr.value_type {
        helpers::reject_discarded_recoverable_eventual(typed, actual, node_origin(resolved, node))?;
    }
    Ok(())
}

/// A `value | channel[tx]` channel-send expression (ignoring comment wrappers).
/// A capture crossing a spawn boundary must be thread-safe; a value that
/// contains shared `Rc` pointers cannot. `subject` names the offending capture
/// (e.g. `moved capture 'data[mov]'`).
fn capture_spawn_send_error(
    resolved: &ResolvedProgram,
    node: &AstNode,
    subject: &str,
) -> TypecheckError {
    let message = format!(
        "{subject} carries values containing shared Rc pointers and cannot cross a spawn boundary"
    );
    node_origin(resolved, node).map_or_else(
        || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
        |origin| {
            TypecheckError::with_origin(TypecheckErrorKind::Ownership, message.clone(), origin)
        },
    )
}

fn is_channel_send_node(node: &AstNode) -> bool {
    matches!(
        helpers::strip_comments(node),
        AstNode::BinaryOp {
            op: fol_parser::ast::BinaryOperator::Pipe,
            right,
            ..
        } if matches!(
            helpers::strip_comments(right),
            AstNode::ChannelAccess {
                endpoint: fol_parser::ast::ChannelEndpoint::Tx,
                ..
            }
        )
    )
}

fn take_deferred_transfer_error(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
) -> Option<TypecheckError> {
    let conflict = typed.take_deferred_transfer_conflict()?;
    let name = resolved
        .symbol(conflict.symbol)
        .map(|symbol| symbol.name.as_str())
        .unwrap_or("<unknown>");
    Some(
        TypecheckError::with_origin(
            TypecheckErrorKind::Ownership,
            format!(
                "move-only binding '{name}' cannot be transferred after it is referenced by a dfr/edf body in the same lexical scope"
            ),
            conflict.transfer_origin,
        )
        .with_related_origin(conflict.deferred_origin, "deferred use registered here"),
    )
}

fn register_deferred_outer_binding_uses(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    registration_scope: ScopeId,
    body: &[AstNode],
) -> Result<(), TypecheckError> {
    for node in body {
        register_deferred_outer_binding_uses_in_node(typed, resolved, registration_scope, node)?;
    }
    Ok(())
}

fn register_deferred_outer_binding_uses_in_node(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    registration_scope: ScopeId,
    node: &AstNode,
) -> Result<(), TypecheckError> {
    if let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = helpers::strip_comments(node)
    {
        let reference = resolved
            .references
            .iter()
            .find(|reference| {
                reference.syntax_id == Some(*syntax_id)
                    && reference.kind == ReferenceKind::Identifier
            })
            .ok_or_else(|| {
                internal_error(
                    "dfr/edf identifier use lost its resolved reference",
                    origin_for(resolved, *syntax_id),
                )
            })?;
        let symbol = reference.resolved.ok_or_else(|| {
            internal_error(
                "dfr/edf identifier use remained unresolved after successful typing",
                origin_for(resolved, *syntax_id),
            )
        })?;
        let declaration_scope = resolved
            .symbol(symbol)
            .map(|symbol| symbol.scope)
            .ok_or_else(|| {
                internal_error(
                    "dfr/edf identifier use lost its resolved symbol",
                    origin_for(resolved, *syntax_id),
                )
            })?;
        if scope_is_same_or_ancestor(resolved, declaration_scope, registration_scope) {
            let origin = origin_for(resolved, *syntax_id).ok_or_else(|| {
                internal_error("dfr/edf identifier use lost its syntax origin", None)
            })?;
            typed.register_deferred_binding_use(
                symbol,
                crate::model::DeferredBindingUse {
                    scope: registration_scope,
                    origin,
                },
            );
        }
    }

    for child in node.children() {
        register_deferred_outer_binding_uses_in_node(typed, resolved, registration_scope, child)?;
    }
    Ok(())
}

fn scope_is_same_or_ancestor(
    resolved: &ResolvedProgram,
    possible_ancestor: ScopeId,
    scope: ScopeId,
) -> bool {
    std::iter::successors(Some(scope), |scope_id| {
        resolved.scope(*scope_id).and_then(|scope| scope.parent)
    })
    .any(|scope_id| scope_id == possible_ancestor)
}

#[cfg(test)]
mod tests {
    use super::helpers::{expected_nil_shell_type, unwrap_shell_result_type};
    use super::literals::type_literal_simple;
    use crate::{BuiltinType, CheckedType, TypedProgram};
    use fol_parser::ast::{AstParser, Literal};
    use fol_resolver::resolve_package;
    use fol_stream::FileStream;

    fn typed_fixture_program() -> TypedProgram {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream =
            FileStream::from_file(fixture_path).expect("Should open expression fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Expression fixture should parse");
        let resolved = resolve_package(syntax).expect("Expression fixture should resolve");
        TypedProgram::from_resolved(resolved)
    }

    #[test]
    fn literal_typing_maps_v1_scalar_literals_to_builtin_types() {
        let mut typed = typed_fixture_program();

        let int_type = type_literal_simple(&mut typed, &Literal::Integer(1), None).unwrap();
        assert_eq!(
            typed.type_table().get(int_type),
            Some(&crate::CheckedType::Builtin(BuiltinType::Int))
        );
        let str_type =
            type_literal_simple(&mut typed, &Literal::String("ok".to_string()), None).unwrap();
        assert_eq!(
            typed.type_table().get(str_type),
            Some(&crate::CheckedType::Builtin(BuiltinType::Str))
        );
    }

    #[test]
    fn nil_contract_only_accepts_optional_and_error_expected_shells() {
        let mut typed = typed_fixture_program();
        let int_type = typed.builtin_types().int;
        let str_type = typed.builtin_types().str_;
        let optional_str = typed
            .type_table_mut()
            .intern(CheckedType::Optional { inner: str_type });
        let bare_error = typed
            .type_table_mut()
            .intern(CheckedType::Error { inner: None });
        let typed_error = typed.type_table_mut().intern(CheckedType::Error {
            inner: Some(str_type),
        });

        assert_eq!(expected_nil_shell_type(&typed, None).unwrap(), None);
        assert_eq!(
            expected_nil_shell_type(&typed, Some(optional_str)).unwrap(),
            Some(optional_str)
        );
        assert_eq!(
            expected_nil_shell_type(&typed, Some(bare_error)).unwrap(),
            Some(bare_error)
        );
        assert_eq!(
            expected_nil_shell_type(&typed, Some(typed_error)).unwrap(),
            Some(typed_error)
        );
        assert_eq!(
            expected_nil_shell_type(&typed, Some(int_type)).unwrap(),
            None
        );
    }

    #[test]
    fn unwrap_contract_only_accepts_optional_and_typed_error_shells() {
        let mut typed = typed_fixture_program();
        let str_type = typed.builtin_types().str_;
        let bool_type = typed.builtin_types().bool_;
        let optional_str = typed
            .type_table_mut()
            .intern(CheckedType::Optional { inner: str_type });
        let bare_error = typed
            .type_table_mut()
            .intern(CheckedType::Error { inner: None });
        let typed_error = typed.type_table_mut().intern(CheckedType::Error {
            inner: Some(bool_type),
        });

        assert_eq!(
            unwrap_shell_result_type(&typed, optional_str).unwrap(),
            Some(str_type)
        );
        assert_eq!(
            unwrap_shell_result_type(&typed, typed_error).unwrap(),
            Some(bool_type)
        );
        assert_eq!(unwrap_shell_result_type(&typed, bare_error).unwrap(), None);
        assert_eq!(
            unwrap_shell_result_type(&typed, typed.builtin_types().int).unwrap(),
            None
        );
    }
}

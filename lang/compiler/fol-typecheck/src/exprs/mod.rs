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
    AstNode, CallSurface, ChannelEndpoint, FolType, ParsedSourceUnitKind, UnaryOperator,
};
use fol_resolver::{ReferenceKind, ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::BTreeMap;

use helpers::{
    binding_kind_for, describe_type, ensure_assignable, ensure_assignable_target, internal_error,
    node_origin, origin_for, plain_value_expr, unsupported_node_surface,
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
    /// True only while an argument is being passed from one `[mux]`
    /// parameter to another. Every other whole-value use stays forbidden.
    pub(crate) allow_mutex_handle: bool,
    /// Exact body scope of the innermost repeating loop. Transfers from
    /// bindings declared outside this scope would execute more than once and
    /// therefore cannot consume a move-only value.
    pub(crate) repeating_loop_scope: Option<ScopeId>,
    /// True while typing a dfr/edf body. Mutex guard transitions are lexical
    /// today and cannot be replayed safely when deferred execution runs.
    pub(crate) inside_deferred_block: bool,
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
            allow_mutex_handle: false,
            repeating_loop_scope: None,
            inside_deferred_block: false,
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
    if !receiver_params.is_some_and(|params| !params.is_empty()) {
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
    let AstNode::FunctionCall {
        syntax_id: Some(syntax_id),
        ..
    } = helpers::strip_comments(task)
    else {
        return Ok(());
    };
    let target_kind = resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(*syntax_id) && reference.kind == ReferenceKind::FunctionCall
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

pub(crate) fn apply_spawn_argument_boundary(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    task: &AstNode,
) -> Result<(), TypecheckError> {
    let task = helpers::strip_comments(task);

    // Ordinary call typing has already inferred and recorded the concrete
    // signature for direct calls. Re-bind the arguments here so omitted
    // defaults cross exactly the same processor boundary as explicit values.
    // Reading the instantiated signature also distinguishes a safe concrete
    // generic call from forwarding an unresolved generic value.
    if let AstNode::FunctionCall {
        syntax_id: Some(syntax_id),
        name,
        args,
        ..
    } = task
    {
        if let Some(signature) = typed.call_signature(*syntax_id).cloned() {
            let bound_args = calls::bind_call_arguments(
                &signature,
                args,
                name,
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
                validate_processor_boundary_type(typed, resolved, *parameter_type, boundary_value)?;
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
                        "values containing shared Rc pointers cannot cross a spawn or async thread boundary; use [mux] data that contains only thread-safe values",
                    )
                },
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Ownership,
                        "values containing shared Rc pointers cannot cross a spawn or async thread boundary; use [mux] data that contains only thread-safe values",
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
) -> Result<(), TypecheckError> {
    let message = if decls::checked_type_contains_generic_param(typed, type_id) {
        Some(
            "unconstrained generic values cannot cross a spawn or async thread boundary because FOL does not yet define a thread-safety and lifetime contract; use a concrete thread-safe value",
        )
    } else if helpers::type_contains_borrowed(typed, type_id) {
        Some(
            "borrowed values cannot cross a spawn or async thread boundary; pass a clonable stack value or move an owned value",
        )
    } else if helpers::type_contains_shared_pointer(typed, type_id) {
        Some(
            "values containing shared Rc pointers cannot cross a spawn or async thread boundary; use [mux] data that contains only thread-safe values",
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
        } => Ok(()),
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
            "spawn requires a direct unqualified routine call or a zero-parameter anonymous routine in V3; qualified calls, method calls, and other expressions are not supported",
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
        AstNode::Spawn { task } => {
            if !typed.capability_model().supports_processor() {
                return Err(unsupported_node_surface(
                    resolved,
                    node,
                    "spawn requires hosted std support; declare the bundled internal standard dependency",
                ));
            }
            require_named_processor_call_target(resolved, task, "spawn")?;
            reject_direct_spawn_channel_receiver(typed, resolved, task)?;
            let observed = type_node_with_expectation(
                typed,
                resolved,
                TypeContext {
                    error_call_mode: ErrorCallMode::Observe,
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
                        "spawning a recoverable routine without await discards its error; make the callee infallible or pipe through async",
                    ),
                    |origin| TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        "spawning a recoverable routine without await discards its error; make the callee infallible or pipe through async",
                        origin,
                    ),
                ));
            }
            apply_spawn_argument_boundary(typed, resolved, task)?;
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
                allow_mutex_handle: false,
                repeating_loop_scope: None,
                inside_deferred_block: false,
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
                    "anonymous routines with [mux] parameters are not supported in V3; use a named routine and call it directly so the mutex ABI remains explicit",
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
                match capture.endpoint {
                    Some(ChannelEndpoint::Tx) => {}
                    Some(ChannelEndpoint::Rx) => {
                        return Err(unsupported_node_surface(
                            resolved,
                            node,
                            "a channel receiver cannot be cloned into a spawned capture; capture c[tx] and keep c[rx] in the receiving routine",
                        ));
                    }
                    None => {
                        return Err(unsupported_node_surface(
                            resolved,
                            node,
                            "V3 anonymous captures must name a channel endpoint such as c[tx]",
                        ));
                    }
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
                let element_type = match typed.type_table().get(capture_type) {
                    Some(CheckedType::Channel { element_type })
                    | Some(CheckedType::ChannelSender { element_type }) => *element_type,
                    _ => {
                        return Err(unsupported_node_surface(
                            resolved,
                            node,
                            format!("capture '{}[tx]' requires a chn[T] binding", capture.name),
                        ));
                    }
                };
                if helpers::type_contains_shared_pointer(typed, element_type) {
                    return Err(node_origin(resolved, node).map_or_else(
                        || {
                            TypecheckError::new(
                                TypecheckErrorKind::Ownership,
                                format!(
                                    "captured endpoint '{}[tx]' carries values containing shared Rc pointers and cannot cross a spawn boundary",
                                    capture.name
                                ),
                            )
                        },
                        |origin| {
                            TypecheckError::with_origin(
                                TypecheckErrorKind::Ownership,
                                format!(
                                    "captured endpoint '{}[tx]' carries values containing shared Rc pointers and cannot cross a spawn boundary",
                                    capture.name
                                ),
                                origin,
                            )
                        },
                    ));
                }
                let sender_type = typed
                    .type_table_mut()
                    .intern(CheckedType::ChannelSender { element_type });
                let capture_symbol = decls::find_symbol_id_in_scope(
                    resolved,
                    context.source_unit_id,
                    routine_scope,
                    &[fol_resolver::SymbolKind::Capture],
                    &capture.name,
                )?;
                decls::record_symbol_type(typed, capture_symbol, sender_type)?;
                if let Some(symbol) = typed.typed_symbol_mut(capture_symbol) {
                    symbol.is_channel_sender_capture = true;
                }
                lowered_params.push(sender_type);
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
                allow_mutex_handle: false,
                repeating_loop_scope: None,
                inside_deferred_block: false,
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
            bindings::track_value_transfer(typed, resolved, context, Some(value), actual)?;
            if let Some(symbol) = whole_target {
                typed.mark_binding_reinitialized(symbol, node_origin(resolved, target));
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
        AstNode::ChannelAccess { channel, endpoint } => {
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
            Ok(match endpoint {
                ChannelEndpoint::Tx => TypedExpr::value(
                    typed
                        .type_table_mut()
                        .intern(CheckedType::ChannelSender { element_type }),
                ),
                ChannelEndpoint::Rx => TypedExpr::value(element_type),
            })
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
        AstNode::PatternAccess { .. } => Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            "pattern access is not yet supported",
        )),
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
        AstNode::Return { value } => {
            controlflow::type_return(typed, resolved, context, value.as_deref())
        }
        AstNode::Break => Ok(TypedExpr::value(typed.builtin_types().never)),
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
    nodes.iter().any(|node| node_contains_return(node))
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
    nodes.iter().any(|node| node_contains_panic(node))
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
    nodes.iter().any(|node| node_contains_report(node))
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
    type_body_inner(typed, resolved, context, nodes)
}

pub(crate) fn type_body_transferring_value(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    nodes: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    type_body_inner(typed, resolved, context, nodes)
}

fn type_body_inner(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    nodes: &[AstNode],
) -> Result<TypedExpr, TypecheckError> {
    let result = (|| {
        let mut final_expr = TypedExpr::none();
        for node in nodes {
            let node_result = type_node(typed, resolved, context, node);
            if let Some(error) = take_deferred_transfer_error(typed, resolved) {
                return Err(error);
            }
            let node_expr = node_result?;
            if let Some(actual) = node_expr.value_type {
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
            }
        }
        Ok(final_expr)
    })();
    typed.release_borrows_in_scope(context.scope_id);
    typed.release_mutex_guards_in_scope(context.scope_id);
    typed.release_deferred_binding_uses_in_scope(context.scope_id);
    result
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

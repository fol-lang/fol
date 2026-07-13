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
    AstNode, CallSurface, FolType, ParsedSourceUnitKind,
};
use fol_resolver::{ResolvedProgram, ScopeId, SourceUnitId};
use std::collections::BTreeMap;

pub(crate) use helpers::{inline_body_block_scope, loop_binder_scope};
use helpers::{
    binding_kind_for, describe_type, ensure_assignable, ensure_assignable_target,
    internal_error, node_origin, origin_for, plain_value_expr,
    unsupported_node_surface,
};

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
        };
        for item in &source_unit.items {
            if let Err(error) = type_node(typed, &resolved, context, &item.node) {
                errors.push(error);
            }
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
    stacker::maybe_grow(256 * 1024, 4 * 1024 * 1024, || {
        type_node_with_expectation_inner(typed, resolved, context, node, expected_type)
    })
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
            operators::type_unary_op(typed, resolved, context, node, op, operand)
        }
        AstNode::VarDecl {
            name,
            type_hint: _,
            value,
            ..
        }
        | AstNode::LabDecl {
            name,
            type_hint: _,
            value,
            ..
        } => bindings::type_binding_initializer(
            typed,
            resolved,
            context,
            name,
            value.as_deref(),
            binding_kind_for(node),
        ),
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
        } => {
            bindings::type_record_init(typed, resolved, context, fields, expected_type)
        }
        AstNode::Identifier { name, syntax_id } => {
            references::type_identifier_reference(typed, resolved, context, name, *syntax_id)
        }
        AstNode::QualifiedIdentifier { path } => {
            references::type_qualified_identifier_reference(typed, resolved, context, path)
        }
        AstNode::AsyncStage => Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            "async pipe stages are planned for a future release",
        )),
        AstNode::AwaitStage => Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            "await pipe stages are planned for a future release",
        )),
        AstNode::Spawn { .. } => Err(unsupported_node_surface(
            resolved,
            node,
            "spawn expressions are planned for a future release",
        )),
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
            if let Some(message) = decls::unsupported_routine_param_surface_message(params) {
                return Err(unsupported_node_surface(resolved, node, message));
            }
            let routine_scope = syntax_id
                .and_then(|syntax_id| resolved.scope_for_syntax(syntax_id))
                .ok_or_else(|| {
                    TypecheckError::new(
                        TypecheckErrorKind::ScopeResolutionFailed,
                        format!(
                            "routine '{name}' has no scope mapping in the resolved program"
                        ),
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
            if !captures.is_empty() {
                return Err(unsupported_node_surface(
                    resolved,
                    node,
                    "anonymous routines with captures are not yet supported",
                ));
            }
            if let Some(message) = decls::unsupported_routine_param_surface_message(params) {
                return Err(unsupported_node_surface(resolved, node, message));
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
            let mut lowered_params = Vec::with_capacity(params.len());
            for param in params {
                let param_type = decls::lower_type(typed, resolved, routine_scope, &param.param_type)?;
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
            let routine_type_id = typed.type_table_mut().intern(CheckedType::Routine(RoutineType {
                generic_params: Vec::new(),
                generic_constraints: BTreeMap::new(),
                param_names: vec![String::new(); lowered_params.len()],
                param_defaults: vec![None; lowered_params.len()],
                variadic_index: params
                    .iter()
                    .position(|param| param.is_variadic),
                params: lowered_params,
                return_type: expected_return_type,
                error_type: expected_error_type,
            }));
            Ok(TypedExpr::value(routine_type_id))
        }
        AstNode::Block { statements, .. } => type_body(typed, resolved, context, statements),
        AstNode::Program { declarations } => type_body(typed, resolved, context, declarations),
        AstNode::When {
            expr,
            cases,
            default,
        } => controlflow::type_when(typed, resolved, context, expr, cases, default.as_deref()),
        AstNode::Loop { condition, body } => {
            controlflow::type_loop(typed, resolved, context, condition, body)
        }
        AstNode::Assignment { target, value } => {
            ensure_assignable_target(
                typed,
                resolved,
                context.source_unit_id,
                context.scope_id,
                target,
            )?;
            let expected = type_node(typed, resolved, context, target)?.required_value(
                "assignment target does not have a type",
            )?;
            let actual =
                type_node_with_expectation(typed, resolved, context, value, Some(expected))?
                    .required_value("assignment value does not have a type")?;
            ensure_assignable(
                typed,
                expected,
                actual,
                "assignment".to_string(),
                None,
            )?;
            Ok(TypedExpr::value(expected))
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
        } if name == "report" => calls::type_report_call(typed, resolved, context, args, *syntax_id),
        AstNode::FunctionCall {
            name,
            args,
            type_args,
            syntax_id,
            ..
        } => {
            if let Ok(entry) = select_intrinsic(IntrinsicSurface::KeywordCall, name) {
                calls::type_keyword_intrinsic_call(
                    typed,
                    resolved,
                    context,
                    entry,
                    args,
                    *syntax_id,
                )
            } else {
                calls::type_function_call(
                    typed,
                    resolved,
                    context,
                    name,
                    type_args,
                    args,
                    *syntax_id,
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
        } => {
            calls::type_method_call(typed, resolved, context, node, object, method, args)
        }
        AstNode::FieldAccess { object, field } => {
            access::type_field_access(typed, resolved, context, object, field, expected_type)
        }
        AstNode::ChannelAccess { .. } => Err(unsupported_node_surface(
            resolved,
            node,
            "channel endpoint access is planned for a future release",
        )),
        AstNode::IndexAccess { container, index } => {
            access::type_index_access(typed, resolved, context, container, index)
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
        AstNode::Select { .. } => Err(unsupported_node_surface(
            resolved,
            node,
            "select/channel semantics are planned for a future release",
        )),
        AstNode::Return { value } => {
            controlflow::type_return(typed, resolved, context, value.as_deref())
        }
        AstNode::Break => Ok(TypedExpr::value(typed.builtin_types().never)),
        AstNode::Dfr { body, .. } => {
            if body_contains_return(body) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "return is not allowed inside dfr blocks in V1",
                ));
            }
            if body_contains_break(body) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "break is not allowed inside dfr blocks in V1",
                ));
            }
            // Deferred blocks replay at every exit; a diverging terminator
            // inside one cannot be lowered against the surrounding exit.
            if body_contains_panic(body) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::InvalidInput,
                    "panic is not allowed inside dfr blocks in V1",
                ));
            }
            let _ = type_body(typed, resolved, context, body)?;
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
        _ => node.children().iter().any(|child| node_contains_panic(child)),
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
            }) || default.as_ref().is_some_and(|body| body_contains_break(body))
        }
        AstNode::Loop { body, .. } | AstNode::Dfr { body, .. } => body_contains_break(body),
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
        _ => node.children().iter().any(|child| node_contains_return(child)),
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
        _ => node.children().iter().any(|child| node_contains_report(child)),
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
    let mut final_expr = TypedExpr::none();
    for node in nodes {
        let node_expr = type_node(typed, resolved, context, node)?;
        if node_expr.value_type.is_some() {
            final_expr = node_expr;
            if node_expr.is_never(typed) {
                return Ok(final_expr);
            }
        }
    }
    Ok(final_expr)
}

#[cfg(test)]
mod tests {
    use super::literals::type_literal_simple;
    use super::helpers::{expected_nil_shell_type, unwrap_shell_result_type};
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

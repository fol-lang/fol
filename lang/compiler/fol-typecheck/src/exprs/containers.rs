//! Growable-container methods (`values.push(x)`).
//!
//! These are receiver methods on a builtin container type rather than on a user
//! record, so they resolve here instead of through `routine_signature_for_method`.
//! The receiver is a mutable PLACE, not a transferred value, which is why this
//! runs before the receiver is typed as a value.

use fol_parser::ast::AstNode;
use fol_resolver::ResolvedProgram;

use super::helpers::{
    apparent_type_id, find_symbol_in_scope_chain, node_origin, resolve_container_method_receiver,
    strip_comments,
};
use super::{plain_value_expr, type_node_with_expectation, TypeContext, TypedExpr};
use crate::errors::{TypecheckError, TypecheckErrorKind};
use crate::types::CheckedType;
use crate::TypedProgram;
use fol_resolver::SymbolKind;

/// Methods this module owns. On a container receiver they always win; on any
/// other receiver the call falls through to ordinary method resolution, so a
/// user routine named `push` on a record still works.
fn is_container_method(method: &str) -> bool {
    matches!(method, "push")
}

/// True when the receiver names a binding whose type is a container family, so
/// the container diagnostics are the right ones to produce. Anything else is
/// left to ordinary method resolution.
fn receiver_names_container(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    receiver: &AstNode,
) -> bool {
    let (binding_name, field) = match strip_comments(receiver) {
        AstNode::Identifier { name, .. } => (name.clone(), None),
        AstNode::QualifiedIdentifier { path } => (path.joined(), None),
        AstNode::FieldAccess { object, field } => match strip_comments(object) {
            AstNode::Identifier { name, .. } => (name.clone(), Some(field.clone())),
            AstNode::QualifiedIdentifier { path } => (path.joined(), Some(field.clone())),
            _ => return false,
        },
        _ => return false,
    };
    let declared = [SymbolKind::ValueBinding, SymbolKind::Parameter]
        .into_iter()
        .find_map(|kind| {
            find_symbol_in_scope_chain(
                resolved,
                context.source_unit_id,
                context.scope_id,
                &binding_name,
                kind,
            )
        })
        .and_then(|symbol| typed.typed_symbol(symbol))
        .and_then(|symbol| symbol.declared_type);
    let Some(binding_type) = declared.and_then(|type_id| apparent_type_id(typed, type_id).ok())
    else {
        return false;
    };
    let container_type = match field {
        None => binding_type,
        Some(field) => {
            let Some(CheckedType::Record { fields }) = typed.type_table().get(binding_type) else {
                return false;
            };
            let Some(field_type) = fields.get(&field).copied() else {
                return false;
            };
            match apparent_type_id(typed, field_type) {
                Ok(type_id) => type_id,
                Err(_) => return false,
            }
        }
    };
    matches!(
        typed.type_table().get(container_type),
        Some(
            CheckedType::Vector { .. }
                | CheckedType::Array { .. }
                | CheckedType::Sequence { .. }
                | CheckedType::Map { .. }
                | CheckedType::Set { .. }
        )
    )
}

/// Type `receiver.<method>(args)` when the receiver is a growable container.
/// `Ok(None)` means this is not a container method call and ordinary method
/// resolution should proceed.
pub(crate) fn type_container_method_call(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    context: TypeContext,
    node: &AstNode,
    object: &AstNode,
    method: &str,
    args: &[AstNode],
) -> Result<Option<TypedExpr>, TypecheckError> {
    if !is_container_method(method) || !receiver_names_container(typed, resolved, context, object) {
        return Ok(None);
    }
    if !typed.capability_model().supports_container_growth() {
        return Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            format!(
                "'.{method}' grows a container, which allocates; it requires the 'memo' capability \
                 model or higher, and this artifact declares 'core'"
            ),
        ));
    }
    let receiver = resolve_container_method_receiver(
        typed,
        resolved,
        context.source_unit_id,
        context.scope_id,
        object,
        method,
    )?;
    let [value] = args else {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!("'.{method}' expects exactly 1 argument, got {}", args.len()),
        ));
    };
    let origin = node_origin(resolved, node).or_else(|| node_origin(resolved, value));
    let element_type = receiver.element_type;
    let actual_expr =
        type_node_with_expectation(typed, resolved, context, value, Some(element_type))?;
    let actual_expr = plain_value_expr(
        typed,
        context,
        actual_expr,
        origin.clone(),
        format!("'.{method}' on '{}'", receiver.binding_name),
    )?;
    let actual =
        actual_expr.required_value(format!("'.{method}' argument does not have a type"))?;
    super::helpers::ensure_assignable(
        typed,
        element_type,
        apparent_type_id(typed, actual)?,
        format!("'.{method}' on '{}'", receiver.binding_name),
        origin,
    )?;
    Ok(Some(TypedExpr::none()))
}

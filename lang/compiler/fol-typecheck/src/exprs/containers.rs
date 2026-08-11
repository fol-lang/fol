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

/// What a container method takes and gives back. `Element` is the container's
/// own element type, resolved per call site.
#[derive(Clone, Copy)]
enum ArgShape {
    Element,
    Index,
}

/// Methods this module owns. On a container receiver they always win; on any
/// other receiver the call falls through to ordinary method resolution, so a
/// user routine named `push` on a record still works.
fn container_method(method: &str) -> Option<(&'static [ArgShape], bool)> {
    // (argument shapes, yields the displaced element as `opt[T]`)
    match method {
        "push" => Some((&[ArgShape::Element], false)),
        "pop" => Some((&[], true)),
        "insert_at" => Some((&[ArgShape::Index, ArgShape::Element], false)),
        "remove_at" => Some((&[ArgShape::Index], true)),
        "clear" => Some((&[], false)),
        "truncate" => Some((&[ArgShape::Index], false)),
        _ => None,
    }
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
    let Some((shapes, yields_element)) = container_method(method) else {
        return Ok(None);
    };
    if !receiver_names_container(typed, resolved, context, object) {
        return Ok(None);
    }
    if !typed.capability_model().supports_container_growth() {
        return Err(TypecheckError::new(
            TypecheckErrorKind::Unsupported,
            format!(
                "'.{method}' resizes a container, which allocates; it requires the 'memo' \
                 capability model or higher, and this artifact declares 'core'"
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
    if args.len() != shapes.len() {
        return Err(TypecheckError::new(
            TypecheckErrorKind::InvalidInput,
            format!(
                "'.{method}' expects exactly {} argument(s), got {}",
                shapes.len(),
                args.len()
            ),
        ));
    }
    let element_type = receiver.element_type;
    let int_type = typed.builtin_types().int;
    for (shape, arg) in shapes.iter().zip(args) {
        let expected = match shape {
            ArgShape::Element => element_type,
            ArgShape::Index => int_type,
        };
        let origin = node_origin(resolved, arg).or_else(|| node_origin(resolved, node));
        let actual_expr =
            type_node_with_expectation(typed, resolved, context, arg, Some(expected))?;
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
            expected,
            apparent_type_id(typed, actual)?,
            format!("'.{method}' on '{}'", receiver.binding_name),
            origin,
        )?;
    }
    if !yields_element {
        return Ok(Some(TypedExpr::none()));
    }
    // The displaced element comes back as `opt[T]`: an empty container is an
    // ordinary state to test for, not a fault.
    let optional = typed.type_table_mut().intern(CheckedType::Optional {
        inner: element_type,
    });
    if let Some(syntax_id) = node.syntax_id() {
        typed.record_node_type(syntax_id, context.source_unit_id, optional)?;
    }
    Ok(Some(TypedExpr::value(optional)))
}

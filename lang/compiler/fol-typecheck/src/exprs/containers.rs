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
    strip_comments, ContainerFamily,
};
use super::{plain_value_expr, type_node_with_expectation, TypeContext, TypedExpr};
use crate::errors::{TypecheckError, TypecheckErrorKind};
use crate::types::CheckedType;
use crate::TypedProgram;
use fol_resolver::SymbolKind;

/// What a container method takes. `Element`, `Key` and `Value` resolve against
/// the receiver's own type arguments at each call site.
#[derive(Clone, Copy)]
enum ArgShape {
    Element,
    Index,
    Key,
    Value,
}

/// What a container method gives back.
#[derive(Clone, Copy)]
enum ResultShape {
    /// A statement — no value.
    None,
    /// The displaced element or value, as `opt[T]`.
    OptionalElement,
    OptionalValue,
    Bool,
    /// Every key, as `vec[K]`; or every value, as `vec[V]`.
    KeyVector,
    ValueVector,
}

/// Methods this module owns, per container family. On a container receiver they
/// always win; on any other receiver the call falls through to ordinary method
/// resolution, so a user routine named `push` on a record still works.
fn vector_method(method: &str) -> Option<(&'static [ArgShape], ResultShape)> {
    match method {
        "push" => Some((&[ArgShape::Element], ResultShape::None)),
        "pop" => Some((&[], ResultShape::OptionalElement)),
        "insert_at" => Some((&[ArgShape::Index, ArgShape::Element], ResultShape::None)),
        "remove_at" => Some((&[ArgShape::Index], ResultShape::OptionalElement)),
        "clear" => Some((&[], ResultShape::None)),
        "truncate" => Some((&[ArgShape::Index], ResultShape::None)),
        _ => None,
    }
}

fn map_method(method: &str) -> Option<(&'static [ArgShape], ResultShape)> {
    match method {
        "insert" => Some((
            &[ArgShape::Key, ArgShape::Value],
            ResultShape::OptionalValue,
        )),
        "get" => Some((&[ArgShape::Key], ResultShape::OptionalValue)),
        "remove" => Some((&[ArgShape::Key], ResultShape::OptionalValue)),
        "contains" => Some((&[ArgShape::Key], ResultShape::Bool)),
        "clear" => Some((&[], ResultShape::None)),
        "keys" => Some((&[], ResultShape::KeyVector)),
        "values" => Some((&[], ResultShape::ValueVector)),
        _ => None,
    }
}

/// Every name either family owns, used to decide whether to engage at all.
fn is_container_method(method: &str) -> bool {
    vector_method(method).is_some() || map_method(method).is_some()
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
    // The receiver's family decides which method table applies, so `.push` on a
    // map and `.insert` on a vector are both named as wrong-family mistakes
    // rather than unknown methods.
    let (shapes, result) = match receiver.family {
        ContainerFamily::Vector { .. } => vector_method(method).ok_or_else(|| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!("'vec[T]' has no '.{method}'; it is a 'map[K,V]' method"),
            )
        })?,
        ContainerFamily::Map { .. } => map_method(method).ok_or_else(|| {
            TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!("'map[K,V]' has no '.{method}'; it is a 'vec[T]' method"),
            )
        })?,
    };
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
    let int_type = typed.builtin_types().int;
    let (element_type, key_type, value_type) = match receiver.family {
        ContainerFamily::Vector { element } => (element, int_type, element),
        ContainerFamily::Map { key, value } => (value, key, value),
    };
    for (shape, arg) in shapes.iter().zip(args) {
        let expected = match shape {
            ArgShape::Element => element_type,
            ArgShape::Index => int_type,
            ArgShape::Key => key_type,
            ArgShape::Value => value_type,
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
    // A displaced or looked-up value comes back as `opt[T]`: absence is an
    // ordinary state to test for, not a fault.
    let result_type = match result {
        ResultShape::None => return Ok(Some(TypedExpr::none())),
        ResultShape::OptionalElement => typed.type_table_mut().intern(CheckedType::Optional {
            inner: element_type,
        }),
        ResultShape::OptionalValue => typed
            .type_table_mut()
            .intern(CheckedType::Optional { inner: value_type }),
        ResultShape::Bool => typed.builtin_types().bool_,
        ResultShape::KeyVector => typed.type_table_mut().intern(CheckedType::Vector {
            element_type: key_type,
        }),
        ResultShape::ValueVector => typed.type_table_mut().intern(CheckedType::Vector {
            element_type: value_type,
        }),
    };
    if let Some(syntax_id) = node.syntax_id() {
        typed.record_node_type(syntax_id, context.source_unit_id, result_type)?;
    }
    Ok(Some(TypedExpr::value(result_type)))
}

//! Finding the `fin` values a record owns through its fields.
//!
//! FOL finalizes a value its owner can name. A `fin` value bound directly is
//! named by its local; one held in a record field is named by a field path from
//! that local, which is what lets a holder finalize what it contains at scope
//! exit. A container element has no such name -- there is no scope-exit
//! iteration in the IR -- so typecheck still refuses to store a `fin` value in
//! one rather than skip its finalizer silently.

use crate::FinalizeEachForm;
use fol_typecheck::{CheckedType, CheckedTypeId, TypedPackage};

/// Peel apparent-type overrides and declared aliases down to the structural
/// type, so a named record reads as the record it is.
fn structural_type(typed_package: &TypedPackage, type_id: CheckedTypeId) -> Option<CheckedType> {
    let program = &typed_package.program;
    let mut current = type_id;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        if !seen.insert(current) {
            return None;
        }
        if let Some(apparent) = program.apparent_type_override(current) {
            current = apparent;
            continue;
        }
        match program.type_table().get(current) {
            Some(CheckedType::Declared { symbol, .. }) => {
                match program
                    .typed_symbol(*symbol)
                    .and_then(|symbol| symbol.declared_type)
                {
                    Some(declared) => current = declared,
                    None => return None,
                }
            }
            other => return other.cloned(),
        }
    }
}

/// Every `fin` value reachable from `type_id` through record fields, as the
/// field path to it and its own type.
///
/// Descent stops at a `fin` field: that value's finalizer consumes it whole, so
/// anything inside is its own business, and finalizing an inner field first
/// would hand the outer finalizer a partially moved value.
pub(crate) fn fin_field_paths(
    typed_package: &TypedPackage,
    type_id: CheckedTypeId,
) -> Vec<FinPath> {
    let mut found = Vec::new();
    let mut visiting = std::collections::BTreeSet::new();
    collect(
        typed_package,
        type_id,
        &mut Vec::new(),
        &mut visiting,
        &mut found,
    );
    found
}

/// Whether the type owns any `fin` value through its fields.
pub(crate) fn has_fin_field(typed_package: &TypedPackage, type_id: CheckedTypeId) -> bool {
    !fin_field_paths(typed_package, type_id).is_empty()
}

/// A reachable `fin` value: the field path to it, its type, and -- when the
/// value at the path is a container -- how to iterate what it holds.
pub(crate) struct FinPath {
    pub path: Vec<String>,
    pub type_id: CheckedTypeId,
    pub form: Option<FinalizeEachForm>,
}

/// The container form for a type whose elements are themselves `fin`, if any.
fn container_form(
    typed_package: &TypedPackage,
    type_id: CheckedTypeId,
) -> Option<(FinalizeEachForm, CheckedTypeId)> {
    let resolves_to_fin = |candidate| typed_package.program.type_resolves_to_fin(candidate);
    match structural_type(typed_package, type_id)? {
        CheckedType::Vector { element_type } | CheckedType::Sequence { element_type } => {
            resolves_to_fin(element_type).then_some((FinalizeEachForm::Linear, element_type))
        }
        CheckedType::Array { element_type, .. } => {
            resolves_to_fin(element_type).then_some((FinalizeEachForm::Array, element_type))
        }
        CheckedType::Set { member_types } => member_types
            .iter()
            .find(|member| resolves_to_fin(**member))
            .map(|member| (FinalizeEachForm::Set, *member)),
        CheckedType::Map {
            key_type,
            value_type,
        } => {
            if resolves_to_fin(value_type) {
                Some((FinalizeEachForm::MapValue, value_type))
            } else {
                resolves_to_fin(key_type).then_some((FinalizeEachForm::MapKey, key_type))
            }
        }
        CheckedType::Optional { inner } => {
            resolves_to_fin(inner).then_some((FinalizeEachForm::OptionalPayload, inner))
        }
        CheckedType::Error { inner } => inner
            .filter(|inner| resolves_to_fin(*inner))
            .map(|inner| (FinalizeEachForm::ErrorPayload, inner)),
        _ => None,
    }
}

fn collect(
    typed_package: &TypedPackage,
    type_id: CheckedTypeId,
    path: &mut Vec<String>,
    visiting: &mut std::collections::BTreeSet<CheckedTypeId>,
    found: &mut Vec<FinPath>,
) {
    // A record can reach itself through a pointer; the cycle carries no owned
    // value to finalize here.
    if !visiting.insert(type_id) {
        return;
    }
    if let Some((form, element_type)) = container_form(typed_package, type_id) {
        found.push(FinPath {
            path: path.clone(),
            type_id: element_type,
            form: Some(form),
        });
    } else if let Some(CheckedType::Record { fields }) = structural_type(typed_package, type_id) {
        for (name, field_type) in fields {
            path.push(name);
            if typed_package.program.type_resolves_to_fin(field_type) {
                found.push(FinPath {
                    path: path.clone(),
                    type_id: field_type,
                    form: None,
                });
            } else {
                collect(typed_package, field_type, path, visiting, found);
            }
            path.pop();
        }
    }
    visiting.remove(&type_id);
}

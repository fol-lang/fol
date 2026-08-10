//! Finding the `fin` values a record owns through its fields.
//!
//! FOL finalizes a value its owner can name. A `fin` value bound directly is
//! named by its local; one held in a record field is named by a field path from
//! that local, which is what lets a holder finalize what it contains at scope
//! exit. A container element has no such name -- there is no scope-exit
//! iteration in the IR -- so typecheck still refuses to store a `fin` value in
//! one rather than skip its finalizer silently.

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
) -> Vec<(Vec<String>, CheckedTypeId)> {
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

fn collect(
    typed_package: &TypedPackage,
    type_id: CheckedTypeId,
    path: &mut Vec<String>,
    visiting: &mut std::collections::BTreeSet<CheckedTypeId>,
    found: &mut Vec<(Vec<String>, CheckedTypeId)>,
) {
    // A record can reach itself through a pointer; the cycle carries no owned
    // value to finalize here.
    if !visiting.insert(type_id) {
        return;
    }
    if let Some(CheckedType::Record { fields }) = structural_type(typed_package, type_id) {
        for (name, field_type) in fields {
            path.push(name);
            if typed_package.program.type_resolves_to_fin(field_type) {
                found.push((path.clone(), field_type));
            } else {
                collect(typed_package, field_type, path, visiting, found);
            }
            path.pop();
        }
    }
    visiting.remove(&type_id);
}

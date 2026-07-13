use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_select_bindings_are_scoped_to_select_bodies() {
    let temp_root = unique_temp_root("resolver_select_binding_scope");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "pro main(channel: int): int = {\n    select {\n        when channel as current { return current; }\n    };\n};\n",
    )
    .expect("Should write the select binding fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let select_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            (matches!(scope.kind, ScopeKind::Block)
                && resolved
                    .symbols_in_scope(scope_id)
                    .into_iter()
                    .any(|symbol| {
                        symbol.name == "current" && symbol.kind == SymbolKind::ValueBinding
                    }))
            .then_some(scope_id)
        })
        .expect("Resolver should create a dedicated scope for select bindings");
    let current_symbol = resolved
        .symbols_in_scope(select_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "current" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Select scope should keep the select binding symbol");

    assert!(
        resolved
            .references_in_scope(select_scope_id)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::Identifier
                    && reference.name == "current"
                    && reference.resolved == Some(current_symbol.id)
            }),
        "Identifier uses inside select bodies should resolve against the select binding"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_select_bindings_do_not_leak_outside_select_bodies() {
    let temp_root = unique_temp_root("resolver_select_binding_leak");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "pro main(channel: int): int = {\n    select {\n        when channel as current { return current; }\n    };\n    return current;\n};\n",
    )
    .expect("Should write the leaking select binding fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject select bindings used outside the select body");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error.to_string().contains("could not resolve name 'current'")
        }),
        "Resolver should keep select bindings local to the select body scope"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_audit_surfaces_resolve_type_hints_and_when_of_types() {
    let temp_root = unique_temp_root("resolver_audit_type_surfaces");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "typ Item: int;\nfun build(items: vec[int], value: any): int = {\n    for(var item: Item; item in items) {\n        item;\n    };\n    var built: vec[int] = { entry for var entry: Item in items };\n    when(value) {\n        of(Item) { return 1; };\n        { return 0; };\n    }\n};\n",
    )
    .expect("Should write the audited type-surface fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let item_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "Item" && symbol.kind == SymbolKind::Type)
        .expect("Program scope should keep the Item type symbol");
    let type_reference_count = resolved
        .references
        .iter()
        .filter(|reference| {
            reference.kind == ReferenceKind::TypeName
                && reference.name == "Item"
                && reference.resolved == Some(item_symbol.id)
        })
        .count();

    assert!(
        type_reference_count >= 3,
        "Loop binder type hints, rolling binder type hints, and when-of type matches should all resolve named types"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_audit_surfaces_reject_missing_types_in_when_of_cases() {
    let temp_root = unique_temp_root("resolver_audit_missing_when_of_type");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun build(value: any): int = {\n    when(value) {\n        of(Missing) { return 1; };\n        { return 0; };\n    }\n};\n",
    )
    .expect("Should write the missing when-of type fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unresolved when-of type matches");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error.to_string().contains("could not resolve type 'Missing'")
        }),
        "Resolver should no longer skip when-of type references during traversal"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

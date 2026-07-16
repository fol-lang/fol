use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_records_local_binding_declaration_origins() {
    let temp_root = unique_temp_root("local_binding_origins");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    var count = 7;\n    var label = count;\n    return label;\n};\n",
    )
    .expect("Should write the local binding origin fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );

    let count = resolved
        .all_symbols()
        .find(|symbol| symbol.name == "count" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Resolver should keep the `count` local binding");
    let origin = count
        .origin
        .as_ref()
        .expect("Local binding should now carry a declaration origin");
    // `var count` on line 2; the name starts at column 9 and spans "count".
    assert_eq!(origin.line, 2);
    assert_eq!(origin.column, 9);
    assert_eq!(origin.length, 5);

    let label = resolved
        .all_symbols()
        .find(|symbol| symbol.name == "label" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Resolver should keep the `label` local binding");
    assert_eq!(
        label
            .origin
            .as_ref()
            .expect("second local binding should also carry an origin")
            .line,
        3
    );
}

#[test]
fn test_resolver_records_parameter_declaration_origins() {
    let temp_root = unique_temp_root("parameter_origins");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] add(left: int, right: int): int = {\n    return left;\n};\n",
    )
    .expect("Should write the parameter origin fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );

    let left = resolved
        .all_symbols()
        .find(|symbol| symbol.name == "left" && symbol.kind == SymbolKind::Parameter)
        .expect("Resolver should keep the `left` parameter");
    let origin = left
        .origin
        .as_ref()
        .expect("Parameter should now carry its own declaration origin");
    // `fun[] add(left: int, ...` on line 1; the `left` NAME starts at column 11
    // and spans "left" (not the routine name span).
    assert_eq!(origin.line, 1);
    assert_eq!(origin.column, 11);
    assert_eq!(origin.length, 4);

    let right = resolved
        .all_symbols()
        .find(|symbol| symbol.name == "right" && symbol.kind == SymbolKind::Parameter)
        .expect("Resolver should keep the `right` parameter");
    let right_origin = right
        .origin
        .as_ref()
        .expect("Second parameter should also carry its own origin");
    // `right` NAME starts at column 22 and spans "right".
    assert_eq!(right_origin.line, 1);
    assert_eq!(right_origin.column, 22);
    assert_eq!(right_origin.length, 5);
}

#[test]
fn test_resolver_records_distinct_origins_for_shadowed_local_bindings() {
    let temp_root = unique_temp_root("shadowed_binding_origins");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    var value = 1;\n    {\n        var value = 2;\n        return value;\n    };\n};\n",
    )
    .expect("Should write the shadowed binding fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );

    let lines: Vec<usize> = resolved
        .all_symbols()
        .filter(|symbol| symbol.name == "value" && symbol.kind == SymbolKind::ValueBinding)
        .filter_map(|symbol| symbol.origin.as_ref().map(|origin| origin.line))
        .collect();
    // The outer binding (line 2) and the shadowing inner binding (line 4) keep
    // distinct declaration origins.
    assert!(lines.contains(&2), "outer binding origin, got: {lines:?}");
    assert!(lines.contains(&4), "shadowing binding origin, got: {lines:?}");
}

#[test]
fn test_resolver_builds_block_scopes_and_allows_shadowing() {
    let temp_root = unique_temp_root("block_scope_shadowing");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    var value = 1;\n    {\n        var value = 2;\n        return value;\n    };\n};\n",
    )
    .expect("Should write the block shadowing fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope for the fixture");
    let block_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            (matches!(scope.kind, ScopeKind::Block) && scope.parent == Some(routine_scope_id))
                .then_some(scope_id)
        })
        .expect("Resolver should create a nested block scope for the explicit block");

    assert!(
        resolved
            .symbols_in_scope(routine_scope_id)
            .into_iter()
            .any(|symbol| symbol.name == "value" && symbol.kind == SymbolKind::ValueBinding),
        "Routine scope should keep the outer local binding"
    );
    assert!(
        resolved
            .symbols_in_scope(block_scope_id)
            .into_iter()
            .any(|symbol| symbol.name == "value" && symbol.kind == SymbolKind::ValueBinding),
        "Nested block scope should keep the shadowing local binding"
    );
    assert!(
        resolved
            .references_in_scope(block_scope_id)
            .into_iter()
            .any(|reference| reference.name == "value" && reference.resolved.is_some()),
        "Identifier references inside the nested block should resolve against the shadowing binding"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_same_scope_duplicate_local_bindings() {
    let temp_root = unique_temp_root("block_scope_duplicates");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    var value = 1;\n    var value = 2;\n    return value;\n};\n",
    )
    .expect("Should write the duplicate local binding fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject duplicate same-scope local bindings");

    assert!(
        errors
            .iter()
            .any(|error| error.kind() == ResolverErrorKind::DuplicateSymbol),
        "Resolver should report duplicate local symbol errors for same-scope bindings"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_use_before_bind_in_local_initializers() {
    let temp_root = unique_temp_root("block_scope_use_before_bind");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    var first = second;\n    var second = 2;\n    return first;\n};\n",
    )
    .expect("Should write the use-before-bind fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject local use-before-bind references");

    assert!(
        errors
            .iter()
            .any(|error| error.kind() == ResolverErrorKind::UnresolvedName),
        "Resolver should report unresolved-name errors for local use-before-bind"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_binds_iteration_loop_names_inside_loop_scope() {
    let temp_root = unique_temp_root("loop_binder_visible");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(items: seq[int], limit: int): int = {\n    loop(item in items when item < limit) {\n        return item;\n    }\n    return limit;\n};\n",
    )
    .expect("Should write the loop-binder resolver fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let loop_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            matches!(scope.kind, ScopeKind::LoopBinder).then_some(scope_id)
        })
        .expect("Resolver should create a loop-binder scope");

    assert!(
        resolved
            .symbols_in_scope(loop_scope_id)
            .into_iter()
            .any(|symbol| symbol.name == "item" && symbol.kind == SymbolKind::LoopBinder),
        "Loop-binder scope should contain the iteration binder symbol"
    );
    assert!(
        resolved
            .references_in_scope(loop_scope_id)
            .into_iter()
            .filter(|reference| reference.name == "item")
            .count()
            >= 2,
        "Loop-binder scope should resolve binder references in both the guard and the body"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_iteration_loop_binders_outside_the_loop() {
    let temp_root = unique_temp_root("loop_binder_invisible");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(items: seq[int]): int = {\n    loop(item in items) {\n        return item;\n    }\n    return item;\n};\n",
    )
    .expect("Should write the loop-binder visibility fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject iteration binders outside their loop scope");

    assert!(
        errors
            .iter()
            .any(|error| error.kind() == ResolverErrorKind::UnresolvedName),
        "Resolver should report unresolved-name errors when a loop binder escapes its scope"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_condition_loop_locals_inside_the_loop_scope() {
    let temp_root = unique_temp_root("condition_loop_local_invisible");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    loop(false) {\n        var inside: int = 7;\n    };\n    return inside;\n};\n",
    )
    .expect("Should write the condition-loop visibility fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject condition-loop locals outside their lexical scope");

    assert!(
        errors
            .iter()
            .any(|error| error.kind() == ResolverErrorKind::UnresolvedName),
        "Resolver should report a loop-local escape as unresolved"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_maps_each_sibling_loop_to_its_exact_scope() {
    let temp_root = unique_temp_root("sibling_loop_scopes");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(first: seq[int], second: seq[int]): int = {\n    loop(left in first) { var a: int = left; };\n    loop(right in second) { var b: int = right; };\n    return 0;\n};\n",
    )
    .expect("Should write sibling-loop scope fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let loop_scopes = resolved
        .scopes
        .iter_with_ids()
        .filter_map(|(scope_id, scope)| {
            matches!(scope.kind, ScopeKind::LoopBinder).then_some(scope_id)
        })
        .collect::<Vec<_>>();
    assert_eq!(loop_scopes.len(), 2, "each sibling loop needs its own scope");
    assert_ne!(loop_scopes[0], loop_scopes[1]);
    for (scope_id, binder) in [(loop_scopes[0], "left"), (loop_scopes[1], "right")] {
        assert!(resolved.symbols_in_scope(scope_id).iter().any(|symbol| {
            symbol.name == binder && symbol.kind == SymbolKind::LoopBinder
        }));
    }

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

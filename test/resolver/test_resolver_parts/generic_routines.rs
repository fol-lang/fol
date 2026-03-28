use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_rejects_generic_parameter_references_outside_routine_scope() {
    let temp_root = unique_temp_root("generic_out_of_scope");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n    return value;\n};\nali Outside: T;\n",
    )
    .expect("Should write the out-of-scope generic fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject generic type names outside the routine scope");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::UnresolvedName
            && error.message().contains("could not resolve type 'T'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_binds_generic_parameters_into_nested_routine_scope() {
    let temp_root = unique_temp_root("nested_generic_scope");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n    fun copy(): T = {\n        return value;\n    };\n    return copy();\n};\n",
    )
    .expect("Should write the nested generic scope fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let routine_scopes = resolved
        .scopes
        .iter_with_ids()
        .filter_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .collect::<Vec<_>>();

    assert!(
        routine_scopes.iter().any(|scope_id| {
            resolved
                .symbols_in_scope(*scope_id)
                .into_iter()
                .any(|symbol| symbol.name == "T" && symbol.kind == SymbolKind::GenericParameter)
        }),
        "Expected at least one routine scope to retain the generic parameter symbol"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

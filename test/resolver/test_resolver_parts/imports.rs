use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_lowers_top_level_use_declarations_into_import_records() {
    let temp_root = unique_temp_root("imports_top_level");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/module.fol"), "var helper: int = 1;\n")
        .expect("Should write the imported namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the top-level import fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "math")
        .expect("Top-level use declarations should lower into resolver import records");
    let import_symbol = resolved
        .symbol(import.alias_symbol)
        .expect("Top-level import alias symbol should exist");

    assert_eq!(import_symbol.kind, SymbolKind::ImportAlias);
    assert_eq!(
        import.import_target,
        "math",
        "Resolver import records should preserve canonical import targets"
    );
    assert!(
        matches!(
            import
                .target_scope
                .and_then(|scope_id| resolved.scope(scope_id))
                .map(|scope| &scope.kind),
            Some(ScopeKind::NamespaceRoot { namespace }) if namespace.ends_with("math")
        ),
        "Location imports inside the package should resolve to the existing namespace scope instead of double-loading the directory"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_local_use_aliases_visible_in_routine_scopes() {
    let temp_root = unique_temp_root("imports_local_alias");
    fs::create_dir_all(temp_root.join("helper"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("helper/module.fol"), "var value: int = 1;\n")
        .expect("Should write the imported helper namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    use helper: loc = {\"helper\"};\n    return helper;\n};\n",
    )
    .expect("Should write the local import fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let local_import = resolved
        .imports_in_scope(routine_scope_id)
        .into_iter()
        .find(|import| import.alias_name == "helper")
        .expect("Local use declarations should lower into routine-scope import records");
    let helper_symbol = resolved
        .symbols_in_scope(routine_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "helper" && symbol.kind == SymbolKind::ImportAlias)
        .expect("Routine scope should keep the local import alias symbol");
    let helper_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| reference.name == "helper")
        .expect("Routine scope should record references to the local import alias");

    assert_eq!(local_import.alias_symbol, helper_symbol.id);
    assert_eq!(helper_reference.resolved, Some(helper_symbol.id));
    assert!(
        matches!(
            local_import
                .target_scope
                .and_then(|scope_id| resolved.scope(scope_id))
                .map(|scope| &scope.kind),
            Some(ScopeKind::NamespaceRoot { namespace }) if namespace.ends_with("helper")
        ),
        "Routine-local location imports inside the package should resolve to the existing namespace scope instead of double-loading the directory"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_duplicate_local_import_aliases() {
    let temp_root = unique_temp_root("imports_duplicate_alias");
    fs::create_dir_all(temp_root.join("core/helper"))
        .expect("Should create the first helper namespace fixture directory");
    fs::create_dir_all(temp_root.join("core/helper2"))
        .expect("Should create the second helper namespace fixture directory");
    fs::write(temp_root.join("core/helper/mod.fol"), "var[exp] value: int = 1;\n")
        .expect("Should write the first helper namespace fixture");
    fs::write(temp_root.join("core/helper2/mod.fol"), "var[exp] value: int = 2;\n")
        .expect("Should write the second helper namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    use helper: loc = {\"core::helper\"};\n    use helper: loc = {\"core::helper2\"};\n    return 0;\n};\n",
    )
    .expect("Should write the duplicate local import fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject duplicate local import aliases");

    assert!(
        errors
            .iter()
            .any(|error| error.kind() == ResolverErrorKind::DuplicateSymbol),
        "Resolver should report duplicate-symbol errors for conflicting local import aliases"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

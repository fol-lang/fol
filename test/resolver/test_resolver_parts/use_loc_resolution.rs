use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_resolves_use_loc_against_the_current_package_root() {
    let temp_root = unique_temp_root("use_loc_package_root");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("pkg"))
        .expect("Should create the imported package root fixture directory");
    fs::write(temp_root.join("pkg/lib.fol"), "var[exp] value: int = 1;\n")
        .expect("Should write the imported package-root fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use pkg: loc = {\"../pkg\"};\nfun[] main(): int = {\n    return value;\n};\n",
    )
    .expect("Should write the package-root import fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "pkg")
        .expect("Resolver should keep the package-root import record");

    assert!(
        matches!(
            resolved
                .scope(import.target_scope.expect("Import target should resolve"))
                .map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "pkg"
        ),
        "Package-root location imports should mount the exact imported directory as a root scope"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_loads_use_loc_from_external_directory_roots() {
    let temp_root = unique_temp_root("use_loc_external_root");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the app package root fixture directory");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create the dependency package root fixture directory");
    fs::write(temp_root.join("math/lib.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the dependency package fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use math: loc = {\"../math\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the external loc import fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary app fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "math")
        .expect("Resolver should keep the imported local package alias");
    let target_scope = import
        .target_scope
        .expect("External local directory imports should resolve to a mounted root scope");
    let answer_symbol = resolved
        .symbols_in_scope(target_scope)
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted external package roots should expose exported root symbols");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope for the importing package");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::Identifier && reference.name == "answer"
        })
        .expect("Importing routines should resolve plain identifiers against mounted external exports");

    assert!(
        matches!(
            resolved.scope(target_scope).map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "math"
        ),
        "External loc imports should mount the exact target directory as the imported root scope",
    );
    assert_eq!(answer_reference.resolved, Some(answer_symbol.id));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary external loc fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_use_loc_against_nested_namespaces() {
    let temp_root = unique_temp_root("use_loc_namespace");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared/net/http"))
        .expect("Should create the imported nested directory fixture");
    fs::write(
        temp_root.join("shared/net/http/route.fol"),
        "var[exp] handler: int = 1;\n",
    )
    .expect("Should write the imported namespace fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use http: loc = {\"../shared/net/http\"};\nfun[] main(): int = {\n    return handler;\n};\n",
    )
    .expect("Should write the namespace import fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "http")
        .expect("Resolver should keep the namespace import record");

    assert!(
        matches!(
            resolved
                .scope(import.target_scope.expect("Import target should resolve"))
                .map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "http"
        ),
        "Nested loc directory imports should mount the exact target directory as the imported root scope"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_imported_loc_directory_files_connected_under_one_root() {
    let temp_root = unique_temp_root("use_loc_connected_surface");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported package-root fixture directory");
    fs::write(
        temp_root.join("shared/values.fol"),
        "var[exp] root_value: int = 1;\n",
    )
    .expect("Should write the imported root export fixture");
    fs::write(
        temp_root.join("shared/helpers.fol"),
        "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the second imported root export fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return emit(root_value);\n};\n",
    )
    .expect("Should write the connected imported-root fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "shared")
        .expect("Resolver should keep the connected imported root alias");
    let target_scope = import
        .target_scope
        .expect("Connected loc directory imports should resolve to a mounted root scope");
    let root_value_symbol = resolved
        .symbols_in_scope(target_scope)
        .into_iter()
        .find(|symbol| symbol.name == "root_value" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted imported roots should expose exported root-level symbols");
    let emit_symbol = resolved
        .symbols_in_scope(target_scope)
        .into_iter()
        .find(|symbol| symbol.name == "emit" && symbol.kind == SymbolKind::Routine)
        .expect("Mounted imported roots should expose exported symbols from multiple root files");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope for the importing package");
    let root_value_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::Identifier && reference.name == "root_value"
        })
        .expect("Routine scope should record the plain imported root-member reference");
    let emit_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::FunctionCall && reference.name == "emit"
        })
        .expect("Routine scope should record the imported routine reference from the second root file");

    assert!(
        matches!(
            resolved.scope(target_scope).map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "shared"
        ),
        "Connected loc directory imports should still mount the exact imported directory as the root scope",
    );
    assert_eq!(root_value_reference.resolved, Some(root_value_symbol.id));
    assert_eq!(emit_reference.resolved, Some(emit_symbol.id));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary connected loc fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_missing_use_loc_targets() {
    let temp_root = unique_temp_root("use_loc_missing_target");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::write(
        temp_root.join("app/main.fol"),
        "use missing: loc = {\"../missing\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the missing import fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject missing local location imports");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::InvalidInput
                && error
                    .to_string()
                    .contains("resolver loc import target")
                && error.to_string().contains("does not exist")
        }),
        "Resolver should report missing directory targets explicitly"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_use_loc_file_targets_explicitly() {
    let temp_root = unique_temp_root("use_loc_file_target");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported fixture directory");
    fs::write(
        temp_root.join("shared/value.fol"),
        "var[exp] value: int = 1;\n",
    )
    .expect("Should write the imported file fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use value: loc = {\"../shared/value.fol\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the file-target import fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject direct file targets for loc imports");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::InvalidInput
                && error
                    .to_string()
                    .contains("must point to a directory, not a file")
        }),
        "Resolver should report direct file loc imports explicitly"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_use_loc_targets_that_define_build_fol() {
    let temp_root = unique_temp_root("use_loc_build_target");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported fixture directory");
    fs::write(
        temp_root.join("shared/value.fol"),
        "var[exp] value: int = 1;\n",
    )
    .expect("Should write the imported value fixture");
    fs::write(
        temp_root.join("shared/build.fol"),
        "pro[] build(): non = {\n    return;\n};\n",
    )
    .expect("Should write the formal package build marker");
    fs::write(
        temp_root.join("app/main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the build-root loc import fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject formal package roots through loc imports");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::InvalidInput
                && error
                    .to_string()
                    .contains("must be imported with pkg instead of loc")
        }),
        "Resolver should explain that build.fol roots belong to pkg imports"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_uses_exact_directory_targets_without_namespace_ambiguity() {
    let temp_root = unique_temp_root("use_loc_exact_directory_target");
    fs::create_dir_all(temp_root.join("app/shared"))
        .expect("Should create the importing package namespace fixture");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported sibling package root fixture");
    fs::write(temp_root.join("app/shared/local.fol"), "var local_only: int = 1;\n")
        .expect("Should write the current-package namespace fixture");
    fs::write(
        temp_root.join("shared/value.fol"),
        "var[exp] value: int = 1;\n",
    )
    .expect("Should write the imported sibling package fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use local: loc = {\"../shared\"};\nfun[] main(): int = {\n    return value;\n};\n",
    )
    .expect("Should write the exact-directory import fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "local")
        .expect("Resolver should keep the local directory import record");

    assert!(
        matches!(
            resolved
                .scope(import.target_scope.expect("Import target should resolve"))
                .map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "shared"
        ),
        "Directory-based loc imports should follow the exact filesystem target, not current-package namespace names"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

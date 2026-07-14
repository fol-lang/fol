use super::{
    resolve_package_from_folder_with_config, try_resolve_package_from_folder_with_config,
    unique_temp_root,
};
use fol_resolver::{ResolverConfig, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;
use std::path::Path;

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("copy target root should be creatable");
    for entry in fs::read_dir(from).expect("copy source root should be readable") {
        let entry = entry.expect("copy entry should be readable");
        let entry_type = entry.file_type().expect("copy entry type should be readable");
        let to_path = to.join(entry.file_name());
        if entry_type.is_dir() {
            copy_tree(&entry.path(), &to_path);
        } else {
            fs::copy(entry.path(), &to_path).expect("copy entry should succeed");
        }
    }
}

fn materialize_bundled_std_alias(store_root: &Path, alias: &str) {
    let bundled_std_root =
        fol_package::available_bundled_std_root().expect("bundled std root should exist");
    copy_tree(&bundled_std_root, &store_root.join(alias));
}

#[test]
fn test_resolver_resolves_bundled_std_from_declared_pkg_alias() {
    let temp_root = unique_temp_root("bundled_std_pkg_alias");
    let app_root = temp_root.join("app");
    let store_root = temp_root.join(".fol/pkg");
    fs::create_dir_all(&app_root)
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(&store_root).expect("Should create the package store root fixture directory");
    materialize_bundled_std_alias(&store_root, "std");
    fs::write(
        app_root.join("main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
    )
    .expect("Should write the bundled std pkg import fixture");

    let resolved = resolve_package_from_folder_with_config(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig {
            std_root: None,
            package_store_root: Some(store_root.to_string_lossy().into_owned()),
        },
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "std")
        .expect("Resolver should keep the bundled std pkg import record");
    let target_scope = import
        .target_scope
        .expect("Bundled std pkg imports should resolve to a mounted root scope");
    assert!(
        matches!(
            resolved.scope(target_scope).map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "std"
        ),
        "Bundled std pkg imports should mount the explicit std dependency alias root",
    );
    let fmt_scope = resolved
        .namespace_scope("std::fmt")
        .expect("Mounted bundled std packages should expose the std.fmt namespace");
    assert!(
        resolved
            .symbols_in_scope(fmt_scope)
            .into_iter()
            .any(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::Routine),
        "Mounted bundled std packages should expose public namespace symbols",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_nested_bundled_std_namespaces_from_pkg_alias_root() {
    let temp_root = unique_temp_root("bundled_std_pkg_namespace_root");
    let app_root = temp_root.join("app");
    let store_root = temp_root.join(".fol/pkg");
    fs::create_dir_all(&app_root)
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(&store_root).expect("Should create the package store root fixture directory");
    materialize_bundled_std_alias(&store_root, "std");
    fs::write(
        app_root.join("main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::math::answer();\n};\n",
    )
    .expect("Should write the bundled std namespace import fixture");

    let resolved = resolve_package_from_folder_with_config(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig {
            std_root: None,
            package_store_root: Some(store_root.to_string_lossy().into_owned()),
        },
    );
    assert!(
        resolved.namespace_scope("std::fmt::math").is_some(),
        "Bundled std pkg imports should expose nested namespace scopes",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_bundled_std_io_from_pkg_alias_root() {
    let temp_root = unique_temp_root("bundled_std_io_pkg_root");
    let app_root = temp_root.join("app");
    let store_root = temp_root.join(".fol/pkg");
    fs::create_dir_all(&app_root).expect("Should create the importing package root fixture directory");
    fs::create_dir_all(&store_root).expect("Should create the package store root fixture directory");
    materialize_bundled_std_alias(&store_root, "std");
    fs::write(
        app_root.join("main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::io::echo_int(7);\n};\n",
    )
    .expect("Should write the bundled std.io pkg import fixture");

    let resolved = resolve_package_from_folder_with_config(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig {
            std_root: None,
            package_store_root: Some(store_root.to_string_lossy().into_owned()),
        },
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "std")
        .expect("Resolver should keep the bundled std.io pkg import record");
    let target_scope = import
        .target_scope
        .expect("Bundled std.io pkg imports should resolve to a mounted root scope");
    assert!(
        matches!(
            resolved.scope(target_scope).map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "std"
        ),
        "Bundled std.io pkg imports should keep the mounted std package root visible",
    );
    let io_scope = resolved
        .namespace_scope("std::io")
        .expect("Mounted bundled std packages should expose the std.io namespace");
    assert!(
        resolved
            .symbols_in_scope(io_scope)
            .into_iter()
            .any(|symbol| symbol.name == "echo_int" && symbol.kind == SymbolKind::Routine),
        "Mounted bundled std packages should expose public std.io symbols",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_missing_bundled_std_dependency_alias_cleanly() {
    let temp_root = unique_temp_root("bundled_std_missing_pkg_alias");
    let app_root = temp_root.join("app");
    let store_root = temp_root.join(".fol/pkg");
    fs::create_dir_all(&app_root).expect("Should create the importing package root fixture directory");
    fs::create_dir_all(&store_root).expect("Should create the package store root fixture directory");
    fs::write(
        app_root.join("main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
    )
    .expect("Should write the missing std dependency alias fixture");

    let errors = try_resolve_package_from_folder_with_config(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig {
            std_root: None,
            package_store_root: Some(store_root.to_string_lossy().into_owned()),
        },
    )
    .expect_err("Resolver should reject missing bundled std dependency aliases");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::InvalidInput
                && error.to_string().contains("std")
        }),
        "Resolver should report missing bundled std dependency aliases through pkg import diagnostics",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_alias_mismatches_for_bundled_std_pkg_imports() {
    let temp_root = unique_temp_root("bundled_std_alias_mismatch");
    let app_root = temp_root.join("app");
    let store_root = temp_root.join(".fol/pkg");
    fs::create_dir_all(&app_root).expect("Should create the importing package root fixture directory");
    fs::create_dir_all(&store_root).expect("Should create the package store root fixture directory");
    materialize_bundled_std_alias(&store_root, "standard_lib");
    fs::write(
        app_root.join("main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
    )
    .expect("Should write the bundled std alias mismatch fixture");

    let errors = try_resolve_package_from_folder_with_config(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig {
            std_root: None,
            package_store_root: Some(store_root.to_string_lossy().into_owned()),
        },
    )
    .expect_err("Resolver should reject bundled std alias mismatches");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::InvalidInput
                && error.to_string().contains("std")
        }),
        "Resolver should report alias mismatches as missing declared std dependency aliases",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

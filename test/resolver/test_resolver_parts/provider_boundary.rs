use super::{resolve_package_from_folder, resolve_package_from_folder_with_config, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverConfig, ScopeKind, SymbolKind};
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

#[test]
fn test_resolver_keeps_loc_import_semantics_stable_through_package_provider() {
    let temp_root = unique_temp_root("provider_boundary_loc");
    let app_root = temp_root.join("app");
    let shared_root = temp_root.join("shared");
    fs::create_dir_all(&app_root).expect("Should create the importing package root fixture");
    fs::create_dir_all(&shared_root).expect("Should create the imported local directory fixture");
    fs::write(shared_root.join("values.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported local value fixture");
    fs::write(
        shared_root.join("helpers.fol"),
        "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the imported local routine fixture");
    fs::write(
        app_root.join("main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return emit(answer);\n};\n",
    )
    .expect("Should write the importing loc fixture");

    let resolved = resolve_package_from_folder(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "shared")
        .expect("Resolver should keep the loc import record");
    let target_scope = import
        .target_scope
        .expect("Loc imports should still resolve to a mounted root scope");
    let symbols = resolved.symbols_in_scope(target_scope);
    let answer_symbol = symbols
        .iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Loc imports should still expose exported value symbols");
    let emit_symbol = symbols
        .iter()
        .find(|symbol| symbol.name == "emit" && symbol.kind == SymbolKind::Routine)
        .expect("Loc imports should still expose exported routine symbols");
    let routine_scope = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope for the importing package");

    assert_eq!(
        resolved
            .references_in_scope(routine_scope)
            .into_iter()
            .find(|reference| reference.kind == ReferenceKind::Identifier && reference.name == "answer")
            .and_then(|reference| reference.resolved),
        Some(answer_symbol.id),
    );
    assert_eq!(
        resolved
            .references_in_scope(routine_scope)
            .into_iter()
            .find(|reference| reference.kind == ReferenceKind::FunctionCall && reference.name == "emit")
            .and_then(|reference| reference.resolved),
        Some(emit_symbol.id),
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_bundled_std_pkg_import_semantics_stable_through_package_provider() {
    let temp_root = unique_temp_root("provider_boundary_std");
    let store_root = temp_root.join(".fol/pkg");
    let app_root = temp_root.join("app");
    fs::create_dir_all(&store_root).expect("Should create the package-store fixture");
    fs::create_dir_all(&app_root).expect("Should create the importing package root fixture");
    let bundled_std_root =
        fol_package::available_bundled_std_root().expect("bundled std root should exist");
    copy_tree(&bundled_std_root, &store_root.join("std"));
    fs::write(
        app_root.join("main.fol"),
        "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
    )
    .expect("Should write the importing bundled std pkg fixture");

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
        .expect("Bundled std pkg imports should still resolve to a mounted root scope");
    assert!(
        matches!(
            resolved.scope(target_scope).map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "std"
        ),
        "Bundled std pkg imports should still mount the std package root through the provider boundary",
    );
    let fmt_scope = resolved
        .namespace_scope("std::fmt")
        .expect("Bundled std pkg imports should expose the std.fmt namespace");
    assert!(
        resolved
            .symbols_in_scope(fmt_scope)
            .into_iter()
            .any(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::Routine),
        "Bundled std pkg imports should expose public namespace symbols through the provider boundary",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_pkg_import_semantics_stable_through_package_provider() {
    let temp_root = unique_temp_root("provider_boundary_pkg");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the exported root source fixture");
    fs::create_dir_all(store_root.join("json/src/fmt"))
        .expect("Should create the exported namespace source fixture");
    fs::create_dir_all(&app_root).expect("Should create the importing package root fixture");
    fs::write(store_root.join("json/build.fol"), "name: json\nversion: 1.0.0\n")
        .expect("Should write the installed package metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"json\",\n        version = \"1.0.0\",\n    });\n};\n",
    )
    .expect("Should write the installed package build fixture");
    fs::write(store_root.join("json/src/root/value.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the exported root source fixture");
    fs::write(
        store_root.join("json/src/fmt/value.fol"),
        "var[exp] formatted: int = 9;\n",
    )
    .expect("Should write the exported namespace source fixture");
    fs::write(
        app_root.join("main.fol"),
        concat!(
            "use json: pkg = {\"json\"};\n",
            "fun[] main(): int = {\n",
            "    return json::src::root::answer + json::src::fmt::formatted;\n",
            "};\n",
        ),
    )
    .expect("Should write the importing pkg fixture");

    let resolved = resolve_package_from_folder_with_config(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig {
            std_root: None,
            package_store_root: Some(
                store_root
                    .to_str()
                    .expect("Temporary package-store fixture path should be valid UTF-8")
                    .to_string(),
            ),
        },
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "json")
        .expect("Resolver should keep the pkg import record");
    let _target_scope = import
        .target_scope
        .expect("Pkg imports should still resolve to a mounted root scope");
    let routine_scope = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope for the importing package");

    let answer_scope = resolved
        .namespace_scope("json::src::root")
        .expect("Pkg imports should expose semantic source namespaces through the provider boundary");
    assert!(resolved
        .symbols_in_scope(answer_scope)
        .into_iter()
        .any(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding));
    assert_eq!(
        resolved
            .references_in_scope(routine_scope)
            .into_iter()
            .find(|reference| {
                reference.kind == ReferenceKind::QualifiedIdentifier
                    && reference.name == "json::src::fmt::formatted"
            })
            .and_then(|reference| reference.resolved)
            .is_some(),
        true,
        "Pkg imports should still resolve qualified references through semantic source namespaces",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

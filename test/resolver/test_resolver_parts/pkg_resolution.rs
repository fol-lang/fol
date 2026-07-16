use super::{resolve_package_from_folder_with_config, try_resolve_package_from_folder_with_config, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverConfig, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

fn formal_pkg_build(name: &str, deps: &[(&str, &str, &str)]) -> String {
    let mut source = format!(
        concat!(
            "pro[] build(): non = {{\n",
            "    var build = .build();\n",
            "    build.meta({{\n",
            "        name = \"{name}\",\n",
            "        version = \"1.0.0\",\n",
            "    }});\n",
        ),
        name = name
    );
    for (alias, source_kind, target) in deps {
        source.push_str(&format!(
            concat!(
                "    build.add_dep({{\n",
                "        alias = \"{alias}\",\n",
                "        source = \"{source_kind}\",\n",
                "        target = \"{target}\",\n",
                "    }});\n",
            ),
            alias = alias,
            source_kind = source_kind,
            target = target
        ));
    }
    source.push_str("};\n");
    source
}

#[test]
fn test_resolver_resolves_pkg_imports_from_the_configured_package_store_root() {
    let temp_root = unique_temp_root("pkg_import_root");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create the package-store fixture directory");
    fs::create_dir_all(&app_root)
        .expect("Should create the importing package root fixture directory");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the installed package metadata fixture");
    fs::create_dir_all(store_root.join("json/src"))
        .expect("Should create the installed package export root fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_pkg_build("json", &[]),
    )
        .expect("Should write the installed package build fixture");
    fs::write(store_root.join("json/src/lib.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the installed package export fixture");
    fs::write(
        app_root.join("main.fol"),
        "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return json::src::answer;\n};\n",
    )
    .expect("Should write the pkg import fixture");

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
    let target_scope = import
        .target_scope
        .expect("Configured pkg imports should resolve to a mounted root scope");
    let answer_scope = resolved
        .namespace_scope("json::src")
        .expect("Mounted pkg roots should expose semantic source namespaces");
    let answer_symbol = resolved
        .symbols_in_scope(answer_scope)
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted pkg roots should expose semantic namespace symbols");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "json::src::answer"
        })
        .expect("Routine scope should record the qualified pkg-imported identifier reference");

    assert!(
        matches!(
            resolved.scope(target_scope).map(|scope| &scope.kind),
            Some(ScopeKind::ProgramRoot { package }) if package == "json"
        ),
        "Configured pkg imports should mount the installed package root as the imported root scope",
    );
    assert_eq!(answer_reference.resolved, Some(answer_symbol.id));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_pkg_imports_expose_semantic_internal_namespaces() {
    let temp_root = unique_temp_root("pkg_hidden_internal_root");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("json/src/public"))
        .expect("Should create the exported source root fixture");
    fs::create_dir_all(store_root.join("json/src/internal"))
        .expect("Should create the internal source root fixture");
    fs::create_dir_all(&app_root)
        .expect("Should create the importing package root fixture directory");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the installed package metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_pkg_build("json", &[]),
    )
        .expect("Should write the installed package build fixture");
    fs::write(
        store_root.join("json/src/public/value.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the exported source fixture");
    fs::write(
        store_root.join("json/src/internal/secret.fol"),
        "var[exp] secret: int = 7;\n",
    )
    .expect("Should write the internal source fixture");
    fs::write(
        app_root.join("main.fol"),
        "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return json::src::internal::secret;\n};\n",
    )
    .expect("Should write the internal pkg import fixture");

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
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    assert!(resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .any(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "json::src::internal::secret"
                && reference.resolved.is_some()
        }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_missing_package_store_roots_for_pkg_imports() {
    let temp_root = unique_temp_root("pkg_missing_store_root");
    fs::create_dir_all(&temp_root)
        .expect("Should create the importing package root fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the pkg import fixture");

    let errors = try_resolve_package_from_folder_with_config(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig::default(),
    )
    .expect_err("Resolver should reject pkg imports without an explicit package store root");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::InvalidInput
                && error
                    .to_string()
                    .contains("requires an explicit package store root")
        }),
        "Resolver should report missing package-store roots explicitly for pkg imports",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_pkg_names_through_declared_export_namespaces() {
    let temp_root = unique_temp_root("pkg_exported_namespace");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the exported root source fixture");
    fs::create_dir_all(store_root.join("json/src/fmt"))
        .expect("Should create the exported fmt source fixture");
    fs::create_dir_all(&app_root)
        .expect("Should create the importing package root fixture directory");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the installed package metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_pkg_build("json", &[]),
    )
    .expect("Should write the installed package build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the exported root source fixture");
    fs::write(
        store_root.join("json/src/fmt/value.fol"),
        "var[exp] formatted: int = 7;\n",
    )
    .expect("Should write the exported fmt source fixture");
    fs::write(
        app_root.join("main.fol"),
        concat!(
            "use json: pkg = {\"json\"};\n",
            "fun[] main(): int = {\n",
            "    return json::src::root::answer + json::src::fmt::formatted;\n",
            "};\n",
        ),
    )
    .expect("Should write the qualified pkg namespace lookup fixture");

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
        .expect("Configured pkg imports should resolve to a mounted root scope");
    let answer_scope = resolved
        .namespace_scope("json::src::root")
        .expect("Mounted pkg roots should create semantic root source namespaces");
    let answer_symbol = resolved
        .symbols_in_scope(answer_scope)
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted pkg roots should expose value symbols from semantic namespaces");
    let fmt_scope = resolved
        .namespace_scope("json::src::fmt")
        .expect("Mounted pkg roots should create semantic nested source namespaces");
    let formatted_symbol = resolved
        .symbols_in_scope(fmt_scope)
        .into_iter()
        .find(|symbol| symbol.name == "formatted" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted pkg export namespaces should expose exported value symbols");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "json::src::root::answer"
        })
        .expect("Routine scope should record the qualified semantic pkg reference");
    let formatted_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "json::src::fmt::formatted"
        })
        .expect("Routine scope should record the qualified pkg namespace reference");

    assert_eq!(answer_reference.resolved, Some(answer_symbol.id));
    assert_eq!(formatted_reference.resolved, Some(formatted_symbol.id));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_pkg_qualified_names_follow_semantic_internal_namespaces() {
    let temp_root = unique_temp_root("pkg_internal_namespace_hidden");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the exported root source fixture");
    fs::create_dir_all(store_root.join("json/src/internal"))
        .expect("Should create the internal source fixture");
    fs::create_dir_all(&app_root)
        .expect("Should create the importing package root fixture directory");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the installed package metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_pkg_build("json", &[]),
    )
        .expect("Should write the installed package build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the exported root source fixture");
    fs::write(
        store_root.join("json/src/internal/secret.fol"),
        "var[exp] secret: int = 7;\n",
    )
    .expect("Should write the internal source fixture");
    fs::write(
        app_root.join("main.fol"),
        concat!(
            "use json: pkg = {\"json\"};\n",
            "fun[] main(): int = {\n",
            "    return json::src::internal::secret;\n",
            "};\n",
        ),
    )
    .expect("Should write the hidden internal namespace lookup fixture");

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
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    assert!(resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .any(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "json::src::internal::secret"
                && reference.resolved.is_some()
        }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_pkg_transitive_dependencies_follow_build_definitions() {
    let temp_root = unique_temp_root("pkg_transitive_build_graph");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("core/src/root"))
        .expect("Should create the transitive dependency export root fixture");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the direct dependency export root fixture");
    fs::create_dir_all(&app_root)
        .expect("Should create the importing package root fixture directory");
    fs::write(
        store_root.join("core/build.fol"),
        "name: core\nversion: 1.0.0\n",
    )
    .expect("Should write the transitive dependency metadata fixture");
    fs::write(
        store_root.join("core/build.fol"),
        formal_pkg_build("core", &[]),
    )
        .expect("Should write the transitive dependency build fixture");
    fs::write(
        store_root.join("core/src/root/value.fol"),
        "var[exp] shared: int = 7;\n",
    )
    .expect("Should write the transitive dependency source fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\ndep.core: pkg:core\n",
    )
    .expect("Should write the direct dependency metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_pkg_build("json", &[("core", "pkg", "core")]),
    )
    .expect("Should write the direct dependency build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "use core: pkg = {\"core\"};\nvar[exp] answer: int = core::src::root::shared;\n",
    )
    .expect("Should write the direct dependency source fixture");
    fs::write(
        app_root.join("main.fol"),
        "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return json::src::root::answer;\n};\n",
    )
    .expect("Should write the transitive pkg import fixture");

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
        .expect("Configured pkg imports should resolve to a mounted root scope");
    let answer_scope = resolved
        .namespace_scope("json::src::root")
        .expect("Mounted pkg roots should expose semantic source namespaces");
    let answer_symbol = resolved
        .symbols_in_scope(answer_scope)
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted pkg roots should expose semantic namespace symbols");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "json::src::root::answer"
        })
        .expect("Routine scope should record the transitive qualified pkg reference");

    assert_eq!(
        answer_reference.resolved,
        Some(answer_symbol.id),
        "Pkg imports should resolve through build-declared transitive dependencies",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

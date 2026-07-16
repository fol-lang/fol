use super::{
    resolve_package_from_folder, resolve_workspace_from_folder_with_config, unique_temp_root,
};
use fol_resolver::{ReferenceKind, ResolverConfig};
use std::fs;

#[test]
fn test_resolver_workspace_keeps_direct_loaded_packages() {
    let temp_root = unique_temp_root("workspace_direct");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported package root fixture directory");
    fs::write(temp_root.join("shared/lib.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported package fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the importing package fixture");

    let workspace = resolve_workspace_from_folder_with_config(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig::default(),
    );

    assert_eq!(workspace.package_count(), 2);
    assert_eq!(workspace.entry_program().package_name(), "app");
    assert!(
        workspace
            .packages()
            .any(|package| package.identity.display_name == "shared"),
        "Workspace should retain the directly imported package"
    );
    assert!(
        workspace
            .entry_program()
            .references
            .iter()
            .any(|reference| {
                reference.kind == ReferenceKind::Identifier
                    && reference.name == "answer"
                    && reference.resolved.is_some()
            }),
        "Workspace handoff should preserve the existing resolved entry reference"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_workspace_dedupes_repeated_loaded_packages() {
    let temp_root = unique_temp_root("workspace_repeated");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported package root fixture directory");
    fs::write(temp_root.join("shared/lib.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported package fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        concat!(
            "use left: loc = {\"../shared\"};\n",
            "use right: loc = {\"../shared\"};\n",
            "fun[] main(): int = {\n",
            "    return answer;\n",
            "};\n",
        ),
    )
    .expect("Should write the repeated import fixture");

    let workspace = resolve_workspace_from_folder_with_config(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
        ResolverConfig::default(),
    );

    assert_eq!(workspace.package_count(), 2);
    assert_eq!(
        workspace
            .packages()
            .filter(|package| package.identity.display_name == "shared")
            .count(),
        1,
        "Workspace should retain one shared imported package entry even when resolution loads it repeatedly"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_workspace_keeps_transitive_loaded_packages() {
    let temp_root = unique_temp_root("workspace_transitive_pkg");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("json/src"))
        .expect("Should create the json package root fixture");
    fs::create_dir_all(store_root.join("core/src"))
        .expect("Should create the core package root fixture");
    fs::create_dir_all(&app_root).expect("Should create the importing package root fixture");

    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\ndep.core: pkg:core\n",
    )
    .expect("Should write json package metadata");
    fs::write(
        store_root.join("json/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"json\",\n        version = \"1.0.0\",\n    });\n    build.add_dep({\n        alias = \"core\",\n        source = \"pkg\",\n        target = \"core\",\n    });\n};\n",
    )
    .expect("Should write json build definition");
    fs::write(
        store_root.join("json/src/lib.fol"),
        "use core: pkg = {\"core\"};\nvar[exp] answer: int = core::src::shared;\n",
    )
    .expect("Should write json package sources");

    fs::write(
        store_root.join("core/build.fol"),
        "name: core\nversion: 1.0.0\n",
    )
    .expect("Should write core package metadata");
    fs::write(
        store_root.join("core/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"core\",\n        version = \"1.0.0\",\n    });\n};\n",
    )
        .expect("Should write core build definition");
    fs::write(store_root.join("core/src/lib.fol"), "var[exp] shared: int = 7;\n")
        .expect("Should write core package sources");

    fs::write(
        app_root.join("main.fol"),
        "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return json::src::answer;\n};\n",
    )
    .expect("Should write the transitive import fixture");

    let workspace = resolve_workspace_from_folder_with_config(
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

    assert_eq!(workspace.package_count(), 3);
    assert!(
        workspace
            .packages()
            .any(|package| package.identity.display_name == "json"),
        "Workspace should retain the directly imported package"
    );
    assert!(
        workspace
            .packages()
            .any(|package| package.identity.display_name == "core"),
        "Workspace should retain transitive imported packages as well"
    );
    assert!(
        workspace
            .entry_program()
            .references
            .iter()
            .any(|reference| {
                reference.kind == ReferenceKind::QualifiedIdentifier
                    && reference.name == "json::src::answer"
                    && reference.resolved.is_some()
            }),
        "Workspace handoff should preserve existing entry-package resolution"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_legacy_program_api_matches_workspace_entry_program() {
    let temp_root = unique_temp_root("workspace_legacy_program_api");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported package root fixture directory");
    fs::write(temp_root.join("shared/lib.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported package fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the importing package fixture");

    let app_root = temp_root
        .join("app")
        .to_str()
        .expect("Temporary resolver fixture path should be valid UTF-8")
        .to_string();
    let legacy = resolve_package_from_folder(&app_root);
    let workspace = resolve_workspace_from_folder_with_config(&app_root, ResolverConfig::default());
    let entry = workspace.entry_program();

    assert_eq!(legacy.package_name(), entry.package_name());
    assert_eq!(legacy.source_units.len(), entry.source_units.len());
    assert_eq!(legacy.references.len(), entry.references.len());
    assert_eq!(legacy.imports.len(), entry.imports.len());
    assert!(
        entry.references.iter().any(|reference| {
            reference.kind == ReferenceKind::Identifier
                && reference.name == "answer"
                && reference.resolved.is_some()
        }),
        "Workspace entry program should preserve the existing entry-package resolution contract"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

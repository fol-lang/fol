use super::{PackageIdentity, PackageSourceKind, ResolverConfig, ResolverSession};
use crate::ResolverErrorKind;
use fol_lexer::lexer::stage3::Elements;
use fol_package::{infer_package_root, PreparedPackage};
use fol_parser::ast::AstParser;
use fol_stream::FileStream;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn parse_package(path: &str) -> fol_parser::ast::ParsedPackage {
    let mut stream = FileStream::from_folder(path).expect("Should open parser fixture folder");
    let mut lexer = Elements::init(&mut stream);
    let mut parser = AstParser::new();
    parser
        .parse_package(&mut lexer)
        .expect("Fixture folder should parse as a package")
}

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_resolver_session_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

/// Builds a canonical modern `build.fol` body for a package `name`, optionally
/// declaring eager package-store dependencies (`pkg` source) for each alias.
fn modern_build_fol(name: &str, pkg_deps: &[&str]) -> String {
    let mut body = String::from("pro[] build(): non = {\n");
    body.push_str("    var build = .build();\n");
    body.push_str(&format!(
        "    build.meta({{ name = \"{name}\", version = \"1.0.0\" }});\n"
    ));
    for dep in pkg_deps {
        body.push_str(&format!(
            "    build.add_dep({{ alias = \"{dep}\", source = \"pkg\", target = \"{dep}\" }});\n"
        ));
    }
    body.push_str("    var graph = build.graph();\n");
    body.push_str(&format!(
        "    graph.add_exe({{ name = \"{name}\", root = \"lib.fol\", fol_model = \"memo\" }});\n"
    ));
    body.push_str("    return;\n};\n");
    body
}

/// Materializes a temporary multi-file package folder named `source_units` and
/// returns its path. The caller is responsible for removing the parent temp root.
fn write_source_units_fixture(label: &str) -> std::path::PathBuf {
    let root = unique_temp_root(label).join("source_units");
    fs::create_dir_all(&root).expect("Should create a temporary multi-unit package fixture");
    fs::write(root.join("00_first.fol"), "var[exp] first: int = 1;\n")
        .expect("Should write the first source unit fixture");
    fs::write(root.join("10_second.fol"), "var[exp] second: int = 2;\n")
        .expect("Should write the second source unit fixture");
    root
}

#[test]
fn session_config_can_be_provided_explicitly() {
    let session = ResolverSession::with_config(ResolverConfig {
        std_root: Some("/tmp/fol_std".to_string()),
        package_store_root: Some("/tmp/fol_pkg".to_string()),
    });

    assert_eq!(session.config().std_root.as_deref(), Some("/tmp/fol_std"));
    assert_eq!(
        session.config().package_store_root.as_deref(),
        Some("/tmp/fol_pkg")
    );
    assert_eq!(session.cached_package_count(), 0);
    assert_eq!(session.loading_depth(), 0);
}

#[test]
fn session_defaults_std_root_to_bundled_tree_when_unspecified() {
    let session = ResolverSession::with_config(ResolverConfig::default());

    assert_eq!(
        session.config().std_root,
        fol_package::available_bundled_std_root().map(|path| path.to_string_lossy().to_string())
    );
}

#[test]
fn inferred_package_root_uses_common_parent_of_parsed_source_units() {
    let fixture_root = write_source_units_fixture("inferred_root");
    let parsed = parse_package(
        fixture_root
            .to_str()
            .expect("Temporary fixture path should be valid UTF-8"),
    );
    let inferred = infer_package_root(&parsed).expect("Should infer a common package root");

    assert!(
        inferred.ends_with("source_units"),
        "Expected inferred package root to end with the parsed folder name, got {:?}",
        inferred
    );

    fs::remove_dir_all(
        fixture_root
            .parent()
            .expect("fixture root should have a parent"),
    )
    .expect("Temporary fixture directory should be removable after the test");
}

#[test]
fn session_cache_keys_track_source_kind_and_canonical_root() {
    let fixture_root = write_source_units_fixture("cache_keys");
    let fixture_path = fixture_root
        .to_str()
        .expect("Temporary fixture path should be valid UTF-8");
    let mut session = ResolverSession::new();
    let identity = PackageIdentity {
        source_kind: PackageSourceKind::Local,
        canonical_root: "/tmp/example".to_string(),
        display_name: "example".to_string(),
    };
    session.cache_package(super::LoadedPackage {
        identity: identity.clone(),
        prepared: PreparedPackage::new(
            fol_package::PackageIdentity {
                source_kind: fol_package::PackageSourceKind::Local,
                canonical_root: identity.canonical_root.clone(),
                display_name: identity.display_name.clone(),
            },
            parse_package(fixture_path),
        ),
        program: {
            let syntax = parse_package(fixture_path);
            let mut nested = ResolverSession::new();
            nested
                .resolve_parsed_package(syntax, None)
                .expect("Fixture package should resolve")
        },
    });

    assert!(session.cached_package(&identity).is_some());
    assert_eq!(session.cached_package_count(), 1);

    fs::remove_dir_all(
        fixture_root
            .parent()
            .expect("fixture root should have a parent"),
    )
    .expect("Temporary fixture directory should be removable after the test");
}

#[test]
fn session_can_load_additional_package_roots_from_directories() {
    let temp_root = unique_temp_root("load_package_root");
    fs::create_dir_all(temp_root.join("dep"))
        .expect("Should create a temporary package root fixture");
    fs::write(
        temp_root.join("dep/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependency package fixture");
    let mut session = ResolverSession::new();

    let loaded = session
        .load_package_from_directory(&temp_root.join("dep"), PackageSourceKind::Local)
        .expect("Session should load additional package roots from disk");

    assert_eq!(loaded.program.package_name(), "dep");
    assert_eq!(loaded.program.source_units.len(), 1);
    assert!(loaded.prepared.metadata.is_none());
    assert!(loaded.prepared.build.is_none());
    assert_eq!(session.cached_package_count(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary session fixture directory should be removable after the test");
}

#[test]
fn session_reuses_cached_packages_for_repeated_canonical_roots() {
    let temp_root = unique_temp_root("load_package_cache");
    fs::create_dir_all(temp_root.join("dep"))
        .expect("Should create a temporary package root fixture");
    fs::write(
        temp_root.join("dep/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependency package fixture");
    let mut session = ResolverSession::new();

    let first = session
        .load_package_from_directory(&temp_root.join("dep"), PackageSourceKind::Local)
        .expect("Session should load the package root the first time");
    let second = session
        .load_package_from_directory(&temp_root.join("dep"), PackageSourceKind::Local)
        .expect("Session should reuse the cached package root");

    assert_eq!(first.identity, second.identity);
    assert_eq!(session.cached_package_count(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary session fixture directory should be removable after the test");
}

#[test]
fn session_reports_explicit_import_cycles_with_participating_roots() {
    let temp_root = unique_temp_root("import_cycle");
    fs::create_dir_all(temp_root.join("dep"))
        .expect("Should create a temporary package root fixture");
    fs::write(
        temp_root.join("dep/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependency package fixture");
    let canonical_root = std::fs::canonicalize(temp_root.join("dep"))
        .expect("Temporary dependency root should canonicalize");
    let identity = PackageIdentity {
        source_kind: PackageSourceKind::Local,
        canonical_root: canonical_root.to_string_lossy().to_string(),
        display_name: "dep".to_string(),
    };
    let mut session = ResolverSession::new();
    session.loading_stack.push(identity.clone());

    let error = session
        .load_package_from_directory(canonical_root.as_path(), PackageSourceKind::Local)
        .expect_err("Session should reject canonical package roots already in the load stack");

    assert_eq!(error.kind(), ResolverErrorKind::ImportCycle);
    assert!(
        error
            .to_string()
            .contains("import cycle detected while loading package roots"),
        "Import cycle diagnostics should explain the active loading cycle",
    );
    assert!(
        error.to_string().contains(&identity.canonical_root),
        "Import cycle diagnostics should list the participating canonical roots",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary session fixture directory should be removable after the test");
}

#[test]
fn session_can_load_installed_pkg_roots_with_required_metadata_and_build_files() {
    let temp_root = unique_temp_root("load_pkg_root");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        modern_build_fol("json", &[]),
    )
    .expect("Should write the package build fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = ResolverSession::new();

    let loaded = session
        .load_package_from_store_target(&store_root, "json")
        .expect("Session should load installed package roots from the package store");

    assert_eq!(loaded.identity.source_kind, PackageSourceKind::Package);
    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(loaded.program.package_name(), "json");
    assert_eq!(loaded.program.ordinary_source_units().count(), 1);
    assert!(
        loaded
            .program
            .ordinary_source_units()
            .all(|unit| !unit.path.ends_with("build.fol")),
        "Installed package source loading should exclude package control files from the ordinary source set",
    );
    assert_eq!(
        loaded
            .prepared
            .metadata
            .as_ref()
            .expect("Installed package roots should retain parsed package metadata")
            .version,
        "1.0.0"
    );
    assert_eq!(
        loaded
            .prepared
            .build
            .as_ref()
            .expect("Installed package roots should retain parsed build definitions")
            .mode(),
        fol_package::PackageBuildMode::ModernOnly
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn session_rejects_pkg_roots_without_required_metadata() {
    let temp_root = unique_temp_root("missing_pkg_metadata");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    var graph = build.graph();\n    graph.add_exe({ name = \"json\", root = \"lib.fol\", fol_model = \"memo\" });\n    return;\n};\n",
    )
    .expect("Should write a build file without package metadata");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = ResolverSession::new();

    let error = session
        .load_package_from_store_target(&store_root, "json")
        .expect_err("Session should reject installed package roots without package metadata");

    assert_eq!(error.kind(), ResolverErrorKind::InvalidInput);
    assert!(error
        .to_string()
        .contains("is missing required field 'name'"));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn session_rejects_pkg_roots_without_required_build_files() {
    let temp_root = unique_temp_root("missing_pkg_build");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = ResolverSession::new();

    let error = session
        .load_package_from_store_target(&store_root, "json")
        .expect_err("Session should reject installed package roots without build files");

    assert_eq!(error.kind(), ResolverErrorKind::InvalidInput);
    assert!(error
        .to_string()
        .contains("missing required package build file"));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn session_ignores_package_fol_when_package_yaml_is_present() {
    let temp_root = unique_temp_root("ignored_package_fol");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        modern_build_fol("json", &[]),
    )
    .expect("Should write the package build fixture");
    fs::write(
        store_root.join("json/package.fol"),
        "var[exp] alias_name: str = \"json\";\n",
    )
    .expect("Should write the ordinary package.fol fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = ResolverSession::new();

    let loaded = session
        .load_package_from_store_target(&store_root, "json")
        .expect("Session should take metadata from build.fol regardless of package.fol");

    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(loaded.program.package_name(), "json");

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn session_package_fol_only_roots_still_fail_missing_metadata() {
    let temp_root = unique_temp_root("package_fol_only");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/package.fol"),
        "var[exp] alias_name: str = \"json\";\n",
    )
    .expect("Should write the ordinary package.fol fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    var graph = build.graph();\n    graph.add_exe({ name = \"json\", root = \"lib.fol\", fol_model = \"memo\" });\n    return;\n};\n",
    )
    .expect("Should write a build file without package metadata");
    let mut session = ResolverSession::new();

    let error = session
        .load_package_from_store_target(&store_root, "json")
        .expect_err("Session should still require build.fol metadata even if package.fol exists");

    assert_eq!(error.kind(), ResolverErrorKind::InvalidInput);
    assert!(error
        .to_string()
        .contains("is missing required field 'name'"));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn session_rejects_malformed_pkg_metadata_explicitly() {
    let temp_root = unique_temp_root("malformed_pkg_metadata");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"json\", version = \"1.0.0\", bogus = \"unsupported\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"json\", root = \"lib.fol\", fol_model = \"memo\" });\n    return;\n};\n",
    )
    .expect("Should write the malformed package metadata fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = ResolverSession::new();

    let error = session
        .load_package_from_store_target(&store_root, "json")
        .expect_err("Session should reject malformed package metadata");

    assert_eq!(error.kind(), ResolverErrorKind::InvalidInput);
    assert!(error
        .to_string()
        .contains("unsupported package metadata field"));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn session_loads_pkg_roots_with_only_control_files_and_no_ordinary_sources() {
    // A pkg root whose only file is the `build.fol` control file loads successfully as a
    // metadata-only package: `build.fol` is retained as a build source unit while the
    // ordinary (consumer-visible) source set is empty. The earlier legacy rejection of
    // control-only roots has been removed, so this pins the current accepting behavior.
    let temp_root = unique_temp_root("pkg_control_only");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        modern_build_fol("json", &[]),
    )
    .expect("Should write the package build fixture");
    let mut session = ResolverSession::new();

    let loaded = session
        .load_package_from_store_target(&store_root, "json")
        .expect("Session should load a metadata-only pkg root that contains only build.fol");

    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(
        loaded.program.ordinary_source_units().count(),
        0,
        "A control-only pkg root should expose no ordinary source units",
    );
    assert_eq!(
        loaded.program.build_source_units().count(),
        1,
        "A control-only pkg root should still retain build.fol as a build source unit",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn session_recursively_loads_transitive_pkg_dependencies_from_store() {
    let temp_root = unique_temp_root("transitive_pkg_graph");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("core"))
        .expect("Should create the transitive dependency root fixture");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create the direct dependency root fixture");
    fs::create_dir_all(&app_root).expect("Should create the importing app fixture directory");
    fs::write(
        store_root.join("core/build.fol"),
        modern_build_fol("core", &[]),
    )
    .expect("Should write the transitive dependency build fixture");
    fs::write(
        store_root.join("core/lib.fol"),
        "var[exp] shared: int = 7;\n",
    )
    .expect("Should write the transitive dependency export");
    fs::write(
        store_root.join("json/build.fol"),
        modern_build_fol("json", &["core"]),
    )
    .expect("Should write the direct dependency build fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "use core: pkg = {\"core\"};\nvar[exp] answer: int = core::shared;\n",
    )
    .expect("Should write the direct dependency source");
    fs::write(
        app_root.join("main.fol"),
        "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the importing app source");
    let parsed = parse_package(
        app_root
            .to_str()
            .expect("Temporary app fixture path should be valid UTF-8"),
    );
    let mut session = ResolverSession::with_config(ResolverConfig {
        std_root: None,
        package_store_root: Some(
            store_root
                .to_str()
                .expect("Temporary package-store fixture path should be valid UTF-8")
                .to_string(),
        ),
    });

    session
        .resolve_package(parsed)
        .expect("Transitive pkg dependencies should resolve through the shared session");

    assert_eq!(
        session.cached_package_count(),
        2,
        "Resolving one direct pkg import with one transitive pkg dependency should cache both package roots",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary transitive package graph fixture should be removable after the test");
}

#[test]
fn session_preloads_pkg_dependencies_from_metadata() {
    let temp_root = unique_temp_root("build_pkg_preload");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("core")).expect("Should create the dependency root fixture");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create the dependent package root fixture");
    fs::write(
        store_root.join("core/build.fol"),
        modern_build_fol("core", &[]),
    )
    .expect("Should write the dependency build fixture");
    fs::write(
        store_root.join("core/lib.fol"),
        "var[exp] shared: int = 7;\n",
    )
    .expect("Should write the dependency source fixture");
    fs::write(
        store_root.join("json/build.fol"),
        modern_build_fol("json", &["core"]),
    )
    .expect("Should write the dependent package build fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependent package source fixture");
    let mut session = ResolverSession::with_config(ResolverConfig {
        std_root: None,
        package_store_root: Some(
            store_root
                .to_str()
                .expect("Temporary package-store fixture path should be valid UTF-8")
                .to_string(),
        ),
    });

    let loaded = session
        .load_package_from_store_target(&store_root, "json")
        .expect("Session should load metadata-declared pkg dependencies eagerly");

    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(
        loaded.prepared.build.as_ref().map(|build| build.mode()),
        Some(fol_package::PackageBuildMode::ModernOnly)
    );
    // The eager `core` dependency is declared in json's build.fol metadata and is loaded
    // through the package-session layer while preparing json (a broken `core/build.fol`
    // would make this `load` fail). It is surfaced here as a parsed metadata dependency.
    let metadata = loaded
        .prepared
        .metadata
        .as_ref()
        .expect("Loaded pkg roots should retain parsed package metadata");
    assert!(
        metadata
            .dependencies
            .iter()
            .any(|dep| dep.alias == "core" && dep.target == "core"),
        "json build.fol metadata should declare an eager pkg dependency on core",
    );
    // The resolver session cache tracks only the packages the resolver itself mounts, so a
    // metadata-only eager dependency (not source-imported by json) leaves the resolver
    // cache holding just json.
    assert_eq!(
        session.cached_package_count(),
        1,
        "Loading json should cache json at the resolver session level",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary build-preload fixture directory should be removable after the test");
}

#[test]
fn session_reuses_cached_shared_pkg_dependencies_across_multiple_dependents() {
    let temp_root = unique_temp_root("shared_pkg_graph");
    let store_root = temp_root.join("store");
    let app_root = temp_root.join("app");
    fs::create_dir_all(store_root.join("core"))
        .expect("Should create the shared dependency root fixture");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create the first direct dependency root fixture");
    fs::create_dir_all(store_root.join("xml"))
        .expect("Should create the second direct dependency root fixture");
    fs::create_dir_all(&app_root).expect("Should create the importing app fixture directory");
    fs::write(
        store_root.join("core/build.fol"),
        modern_build_fol("core", &[]),
    )
    .expect("Should write the shared dependency build fixture");
    fs::write(
        store_root.join("core/lib.fol"),
        "var[exp] shared: int = 7;\n",
    )
    .expect("Should write the shared dependency export");
    fs::write(
        store_root.join("json/build.fol"),
        modern_build_fol("json", &["core"]),
    )
    .expect("Should write the first direct dependency build fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "use core: pkg = {\"core\"};\nvar[exp] left: int = core::shared;\n",
    )
    .expect("Should write the first direct dependency source");
    fs::write(
        store_root.join("xml/build.fol"),
        modern_build_fol("xml", &["core"]),
    )
    .expect("Should write the second direct dependency build fixture");
    fs::write(
        store_root.join("xml/lib.fol"),
        "use core: pkg = {\"core\"};\nvar[exp] right: int = core::shared;\n",
    )
    .expect("Should write the second direct dependency source");
    fs::write(
        app_root.join("main.fol"),
        concat!(
            "use json: pkg = {\"json\"};\n",
            "use xml: pkg = {\"xml\"};\n",
            "fun[] main(): int = {\n",
            "    return 0;\n",
            "};\n",
        ),
    )
    .expect("Should write the importing app source");
    let parsed = parse_package(
        app_root
            .to_str()
            .expect("Temporary app fixture path should be valid UTF-8"),
    );
    let mut session = ResolverSession::with_config(ResolverConfig {
        std_root: None,
        package_store_root: Some(
            store_root
                .to_str()
                .expect("Temporary package-store fixture path should be valid UTF-8")
                .to_string(),
        ),
    });

    session
        .resolve_package(parsed)
        .expect("Shared pkg dependencies should resolve through one cached session");

    assert_eq!(
        session.cached_package_count(),
        3,
        "Two direct pkg imports sharing one transitive dependency should cache json, xml, and core once each",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary shared package graph fixture should be removable after the test");
}

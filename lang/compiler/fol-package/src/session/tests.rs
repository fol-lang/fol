use super::{
    canonical_directory_root, infer_package_root, parse_directory_package_syntax,
    resolve_directory_path, PackageSession,
};
use crate::{PackageConfig, PackageIdentity, PackageSourceKind, PreparedPackage};
use fol_parser::ast::{AstParser, ParsedPackage, UsePathSegment};
use fol_stream::FileStream;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn parse_fixture_package() -> ParsedPackage {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../test/parser/simple_var.fol"
    );
    let mut stream = FileStream::from_file(fixture_path).expect("Should open package fixture");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    parser
        .parse_package(&mut lexer)
        .expect("Package fixture should parse as a package")
}

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_package_session_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

fn formal_build_fixture(name: &str, deps: &[(&str, &str, &str)]) -> String {
    let mut source = format!(
        "pro[] build(): non = {{\n    var build = .build();\n    build.meta({{ name = \"{name}\", version = \"1.0.0\" }});\n"
    );
    for (alias, source_kind, target) in deps {
        source.push_str(&format!(
            "    build.add_dep({{ alias = \"{alias}\", source = \"{source_kind}\", target = \"{target}\" }});\n"
        ));
    }
    source.push_str("};\n");
    source
}

fn formal_build_fixture_with_surface(name: &str) -> String {
    concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"",
    )
    .to_string()
        + name
        + "\", version = \"1.0.0\" });\n"
        + "    var graph = build.graph();\n"
        + "    var codec = graph.add_module({ name = \"codec\", root = \"src/root/codec.fol\" });\n"
        + "    var app = graph.add_exe({ name = \""
        + name
        + "\", root = \"src/root/main.fol\" });\n"
        + "    var schema = graph.write_file({ name = \"schema\", path = \"gen/schema.fol\", contents = \"ok\" });\n"
        + "    var docs = graph.step(\"docs\");\n"
        + "    graph.install(app);\n"
        + "    build.export_module({ name = \"api\", module = codec });\n"
        + "    build.export_artifact({ name = \"runtime\", artifact = app });\n"
        + "    build.export_step({ name = \"check\", step = docs });\n"
        + "    build.export_output({ name = \"schema-api\", output = schema });\n"
        + "    return;\n"
        + "};\n"
}

#[test]
fn package_session_config_can_be_provided_explicitly() {
    let session = PackageSession::with_config(PackageConfig {
        std_root: Some("/tmp/fol_std".to_string()),
        package_store_root: Some("/tmp/fol_pkg".to_string()),
        package_cache_root: Some("/tmp/fol_cache".to_string()),
        package_git_cache_root: Some("/tmp/fol_git_cache".to_string()),
    });

    assert_eq!(session.config().std_root.as_deref(), Some("/tmp/fol_std"));
    assert_eq!(
        session.config().package_store_root.as_deref(),
        Some("/tmp/fol_pkg")
    );
    assert_eq!(
        session.config().package_cache_root.as_deref(),
        Some("/tmp/fol_cache")
    );
    assert_eq!(
        session.config().package_git_cache_root.as_deref(),
        Some("/tmp/fol_git_cache")
    );
    assert_eq!(session.cached_package_count(), 0);
    assert_eq!(session.loading_depth(), 0);
}

#[test]
fn package_session_defaults_std_root_to_bundled_tree_when_unspecified() {
    let session = PackageSession::with_config(PackageConfig::default());

    assert_eq!(
        session.config().std_root,
        crate::available_bundled_std_root().map(|path| path.to_string_lossy().to_string())
    );
}

#[test]
fn package_session_can_load_internal_standard_package() {
    let mut session = PackageSession::with_config(PackageConfig::default());

    let loaded = session
        .load_internal_standard_package()
        .expect("package session should load bundled internal standard package");

    assert_eq!(loaded.source_kind(), PackageSourceKind::Standard);
    assert_eq!(loaded.identity.display_name, "std");
}

#[test]
fn package_session_caches_prepared_packages_by_identity() {
    let mut session = PackageSession::new();
    let identity = PackageIdentity {
        source_kind: PackageSourceKind::Local,
        canonical_root: "/tmp/example".to_string(),
        display_name: "example".to_string(),
    };
    session.cache_package(PreparedPackage::new(
        identity.clone(),
        parse_fixture_package(),
    ));

    assert!(session.cached_package(&identity).is_some());
    assert_eq!(session.cached_package_count(), 1);
}

#[test]
fn package_session_can_prepare_entry_packages_from_parsed_syntax() {
    let session = PackageSession::new();
    let prepared = session
        .prepare_entry_package(parse_fixture_package())
        .expect("Package session should prepare parsed entry packages");

    assert_eq!(prepared.identity.source_kind, PackageSourceKind::Entry);
    assert!(
        prepared.identity.canonical_root.ends_with("parser"),
        "Prepared entry packages should infer a canonical package root from parsed source units",
    );
    assert_eq!(prepared.package_name(), "parser");
    assert!(prepared.metadata.is_none());
    assert!(prepared.build.is_none());
}

#[test]
fn package_session_tracks_loading_stack_for_cycle_detection() {
    let identity = PackageIdentity {
        source_kind: PackageSourceKind::Package,
        canonical_root: "/tmp/example".to_string(),
        display_name: "example".to_string(),
    };
    let mut session = PackageSession::new();

    session
        .begin_loading(&identity)
        .expect("First load of a package root should succeed");
    let error = session
        .begin_loading(&identity)
        .expect_err("Repeated active loads of the same package root should report a cycle");

    assert_eq!(error.kind(), crate::PackageErrorKind::ImportCycle);
    assert!(error
        .to_string()
        .contains("package import cycle detected while loading package roots"));
    assert_eq!(session.loading_depth(), 1);

    session.finish_loading();
    assert_eq!(session.loading_depth(), 0);
}

#[test]
fn inferred_package_root_uses_common_parent_of_parsed_source_units() {
    let temp_root = unique_temp_root("infer_common_parent");
    let package_dir = temp_root.join("source_units");
    fs::create_dir_all(&package_dir).expect("Should create fixture package directory");
    fs::write(
        package_dir.join("main.fol"),
        "var[exp] main_value: int = 1;\n",
    )
    .expect("Should write the primary source unit");
    fs::write(
        package_dir.join("lib.fol"),
        "var[exp] lib_value: int = 2;\n",
    )
    .expect("Should write the secondary source unit");

    let parsed = {
        let mut stream = FileStream::from_folder(
            package_dir
                .to_str()
                .expect("Temporary fixture path should be valid UTF-8"),
        )
        .expect("Should open folder package fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        parser
            .parse_package(&mut lexer)
            .expect("Folder fixture should parse as a package")
    };

    let inferred = infer_package_root(&parsed).expect("Should infer a common package root");

    assert!(
        inferred.ends_with("source_units"),
        "Expected inferred package root to end with the parsed folder name, got {:?}",
        inferred
    );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn canonical_directory_root_rejects_file_targets() {
    let fixture_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../test/parser/simple_var.fol"
    );

    let error = canonical_directory_root(Path::new(fixture_path), PackageSourceKind::Local)
        .expect_err("Package directory loading should reject direct file targets");

    assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
    assert!(error
        .to_string()
        .contains("must point to a directory, not a file"));
}

#[test]
fn resolve_directory_path_joins_relative_segments() {
    let resolved = resolve_directory_path(
        Path::new("/tmp/root"),
        &[
            UsePathSegment {
                separator: None,
                spelling: "deps".to_string(),
            },
            UsePathSegment {
                separator: Some(fol_parser::ast::UsePathSeparator::Slash),
                spelling: "json".to_string(),
            },
        ],
    );

    assert_eq!(resolved, Path::new("/tmp/root/deps/json"));
}

#[test]
fn parse_directory_package_syntax_loads_folder_packages() {
    let temp_root = unique_temp_root("parse_directory_syntax");
    fs::create_dir_all(temp_root.join("dep"))
        .expect("Should create a temporary package root fixture");
    fs::write(
        temp_root.join("dep/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependency package fixture");

    let parsed = parse_directory_package_syntax(
        temp_root.join("dep").as_path(),
        "dep",
        PackageSourceKind::Local,
    )
    .expect("Package session helpers should parse directory packages");

    assert_eq!(parsed.package, "dep");
    assert_eq!(parsed.source_units.len(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-session fixture directory should be removable after the test");
}

#[test]
fn package_session_can_load_local_directory_packages() {
    let temp_root = unique_temp_root("load_local_directory");
    fs::create_dir_all(temp_root.join("dep"))
        .expect("Should create a temporary package root fixture");
    fs::write(
        temp_root.join("dep/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependency package fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_directory_package(temp_root.join("dep").as_path(), PackageSourceKind::Local)
        .expect("Package session should load local directory packages");

    assert_eq!(loaded.package_name(), "dep");
    assert_eq!(loaded.source_kind(), PackageSourceKind::Local);
    assert_eq!(session.cached_package_count(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-session fixture directory should be removable after the test");
}

#[test]
fn package_session_rejects_local_directory_targets_that_define_build_fol() {
    let temp_root = unique_temp_root("load_local_directory_with_build");
    fs::create_dir_all(temp_root.join("dep"))
        .expect("Should create a temporary package root fixture");
    fs::write(
        temp_root.join("dep/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependency package fixture");
    fs::write(
        temp_root.join("dep/build.fol"),
        "pro[] build(): non = {\n    return graph\n}\n",
    )
    .expect("Should write the formal package build marker");
    let mut session = PackageSession::new();

    let error = session
        .load_directory_package(temp_root.join("dep").as_path(), PackageSourceKind::Local)
        .expect_err("Local directory imports should reject formal package roots");

    assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
    assert!(
        error
            .to_string()
            .contains("must be imported with pkg instead of loc"),
        "Local directory import errors should explain that formal package roots belong to pkg",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-session fixture directory should be removable after the test");
}

#[test]
fn package_session_reuses_cached_local_directory_packages() {
    let temp_root = unique_temp_root("load_local_directory_cache");
    fs::create_dir_all(temp_root.join("dep"))
        .expect("Should create a temporary package root fixture");
    fs::write(
        temp_root.join("dep/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the dependency package fixture");
    let mut session = PackageSession::new();

    let first = session
        .load_directory_package(temp_root.join("dep").as_path(), PackageSourceKind::Local)
        .expect("Package session should load the local directory the first time");
    let second = session
        .load_directory_package(temp_root.join("dep").as_path(), PackageSourceKind::Local)
        .expect("Package session should reuse cached local directories");

    assert_eq!(first.identity, second.identity);
    assert_eq!(session.cached_package_count(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-session fixture directory should be removable after the test");
}

#[test]
fn package_session_can_load_standard_directory_packages() {
    let temp_root = unique_temp_root("load_standard_directory");
    fs::create_dir_all(temp_root.join("fmt"))
        .expect("Should create a temporary std package root fixture");
    fs::write(
        temp_root.join("fmt/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the standard package fixture");
    let mut session = PackageSession::with_config(PackageConfig {
        std_root: Some(
            temp_root
                .to_str()
                .expect("Temporary std fixture root should be valid UTF-8")
                .to_string(),
        ),
        package_store_root: None,
        package_cache_root: None,
        package_git_cache_root: None,
    });

    let loaded = session
        .load_directory_package(temp_root.join("fmt").as_path(), PackageSourceKind::Standard)
        .expect("Package session should load standard directory packages");

    assert_eq!(loaded.package_name(), "fmt");
    assert_eq!(loaded.source_kind(), PackageSourceKind::Standard);
    assert_eq!(session.cached_package_count(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-session fixture directory should be removable after the test");
}

#[test]
fn package_session_can_load_installed_pkg_roots_with_required_controls() {
    let temp_root = unique_temp_root("load_pkg_root");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        concat!("name: json\n", "version: 1.0.0\n", "kind: lib\n"),
    )
    .expect("Should write the package metadata fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_build_fixture("json", &[]),
    )
    .expect("Should write the package build fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect("Package session should load installed package roots from the package store");

    assert_eq!(loaded.identity.source_kind, PackageSourceKind::Package);
    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(loaded.package_name(), "json");
    assert_eq!(loaded.syntax.source_units.len(), 2);
    assert!(
        loaded
            .syntax
            .source_units
            .iter()
            .all(|unit| !unit.path.ends_with("package.fol")),
        "Installed package source loading should keep legacy control files out of the parsed source set",
    );
    assert!(
        loaded
            .syntax
            .source_units
            .iter()
            .any(|unit| unit.path.ends_with("build.fol")),
        "Installed package source loading should keep build.fol in the parsed source set",
    );
    assert_eq!(
        loaded
            .metadata
            .as_ref()
            .expect("Installed package roots should retain parsed package metadata")
            .version,
        "1.0.0"
    );
    assert!(
        loaded.build.is_some(),
        "Installed package roots should retain parsed build definitions"
    );
    assert!(loaded.exports.is_empty());

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn parse_directory_package_syntax_keeps_build_files_for_pkg_roots() {
    let temp_root = unique_temp_root("pkg_build_file_inclusion");
    fs::create_dir_all(temp_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        temp_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the package metadata fixture");
    fs::write(
        temp_root.join("json/build.fol"),
        formal_build_fixture("json", &[]),
    )
    .expect("Should write the package build fixture");
    fs::create_dir_all(temp_root.join("json/src"))
        .expect("Should create the exported source fixture");
    fs::write(
        temp_root.join("json/src/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");

    let parsed = parse_directory_package_syntax(
        temp_root.join("json").as_path(),
        "json",
        PackageSourceKind::Package,
    )
    .expect("Pkg source parsing should keep build files and ordinary source files");

    // build.fol and ordinary source files are both retained in the parsed set.
    assert_eq!(parsed.source_units.len(), 2);
    assert!(
        parsed
            .source_units
            .iter()
            .any(|unit| unit.path.ends_with("build.fol")),
        "Pkg source parsing should retain build.fol in the parsed source set",
    );
    assert!(
        parsed
            .source_units
            .iter()
            .any(|unit| unit.path.ends_with("lib.fol")),
        "Pkg source parsing should retain ordinary source files in the parsed source set",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn parse_directory_package_syntax_accepts_pkg_roots_with_only_build_files() {
    let temp_root = unique_temp_root("pkg_build_only");
    fs::create_dir_all(temp_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        temp_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the package metadata fixture");
    fs::write(
        temp_root.join("json/build.fol"),
        formal_build_fixture("json", &[]),
    )
    .expect("Should write the package build fixture");

    let parsed = parse_directory_package_syntax(
        temp_root.join("json").as_path(),
        "json",
        PackageSourceKind::Package,
    )
    .expect("Pkg roots with only build.fol should now parse");

    assert_eq!(parsed.source_units.len(), 1);
    assert!(
        parsed.source_units[0].path.ends_with("build.fol"),
        "sole source unit should be build.fol, got {}",
        parsed.source_units[0].path
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_no_longer_projects_declared_export_namespace_mounts() {
    let temp_root = unique_temp_root("pkg_no_export_projection");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the exported root fixture");
    fs::create_dir_all(store_root.join("json/src/fmt/nested"))
        .expect("Should create the nested export fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the package metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"json\", version = \"1.0.0\" });\n",
            "};\n",
        ),
    )
    .expect("Should write the package build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the exported root source fixture");
    fs::write(
        store_root.join("json/src/fmt/value.fol"),
        "var[exp] formatted: int = 7;\n",
    )
    .expect("Should write the exported namespace source fixture");
    fs::write(
        store_root.join("json/src/fmt/nested/value.fol"),
        "var[exp] nested_value: int = 9;\n",
    )
    .expect("Should write the nested exported namespace source fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect("Package session should load package roots without projected export mounts");

    assert!(loaded.exports.is_empty());

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_keeps_semantic_build_entries_for_formal_pkg_roots() {
    let temp_root = unique_temp_root("pkg_semantic_build_entry");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\n",
    )
    .expect("Should write the package metadata fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_build_fixture("json", &[]),
    )
    .expect("Should write the semantic build entry fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect("Package session should load pkg roots with semantic build entries");

    assert!(loaded.exports.is_empty());
    assert_eq!(
        loaded.build_mode(),
        crate::build::PackageBuildMode::ModernOnly
    );
    assert!(loaded.has_semantic_build_entry(
        &crate::build_entry::BuildEntrySignatureExpectation::canonical()
    ));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_projects_dependency_surfaces_for_formal_pkg_roots() {
    let temp_root = unique_temp_root("pkg_dependency_surface");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_build_fixture_with_surface("json"),
    )
    .expect("Should write the semantic build entry fixture");
    fs::write(
        store_root.join("json/src/root/main.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    fs::write(
        store_root.join("json/src/root/codec.fol"),
        "var[exp] codec: int = 7;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect("Package session should load pkg roots with projected dependency surfaces");

    let surfaces = loaded
        .dependency_surfaces
        .as_ref()
        .expect("formal pkg roots should now project dependency surfaces");
    let surface = surfaces
        .find("json")
        .expect("surface should be keyed by package name");
    assert!(surface
        .source_roots
        .iter()
        .any(|root| std::path::Path::new(root.relative_path.as_str()).ends_with("src/root")));
    assert!(surface.modules.iter().any(|module| module.name == "api"));
    assert!(surface
        .artifacts
        .iter()
        .any(|artifact| artifact.name == "runtime" && artifact.fol_model == "memo"));
    assert!(surface.steps.iter().any(|step| step.name == "check"));
    assert!(surface
        .generated_outputs
        .iter()
        .any(|output| output.name == "schema-api"));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_rejects_pkg_roots_without_required_build_file() {
    let temp_root = unique_temp_root("missing_pkg_build");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = PackageSession::new();

    let error = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect_err("Package session should reject installed package roots without build.fol");

    assert_eq!(error.kind(), crate::PackageErrorKind::InvalidInput);
    assert!(error
        .to_string()
        .contains("missing required package build file"));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_loads_formal_packages_from_build_fol_only() {
    let temp_root = unique_temp_root("build_fol_only_formal_package");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json"))
        .expect("Should create a temporary package-store fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_build_fixture("json", &[]),
    )
    .expect("Should write the package build fixture");
    fs::write(
        store_root.join("json/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the package source fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect("Package session should load formal packages from build.fol metadata alone");

    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(loaded.package_name(), "json");

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_preloads_transitive_pkg_dependencies() {
    let temp_root = unique_temp_root("transitive_pkg_graph");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("core/src/root"))
        .expect("Should create the transitive dependency export root fixture");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the direct dependency export root fixture");
    fs::write(
        store_root.join("core/build.fol"),
        "name: core\nversion: 1.0.0\n",
    )
    .expect("Should write the transitive dependency metadata fixture");
    fs::write(
        store_root.join("core/build.fol"),
        formal_build_fixture("core", &[]),
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
        formal_build_fixture("json", &[("core", "pkg", "core")]),
    )
    .expect("Should write the direct dependency build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "use core: pkg = {\"core\"};\nvar[exp] answer: int = core::src::root::shared;\n",
    )
    .expect("Should write the direct dependency source fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect("Package session should load direct package roots");

    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(session.cached_package_count(), 2);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_only_preloads_eager_pkg_dependencies() {
    let temp_root = unique_temp_root("eager_only_pkg_preload");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("core/src/root"))
        .expect("Should create eager dependency fixture");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create dependent package fixture");
    fs::write(
        store_root.join("core/build.fol"),
        formal_build_fixture("core", &[]),
    )
    .expect("Should write eager dependency build fixture");
    fs::write(
        store_root.join("core/src/root/value.fol"),
        "var[exp] shared: int = 7;\n",
    )
    .expect("Should write eager dependency source fixture");
    fs::write(
        store_root.join("json/build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"json\", version = \"1.0.0\" });\n",
            "    build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core\", mode = \"lazy\" });\n",
            "};\n",
        ),
    )
    .expect("Should write dependent package build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "var[exp] answer: int = 1;\n",
    )
    .expect("Should write dependent package source fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect("Package session should load the direct package");

    assert_eq!(loaded.identity.display_name, "json");
    assert_eq!(session.cached_package_count(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_reports_explicit_pkg_dependency_cycles() {
    let temp_root = unique_temp_root("cyclic_pkg_graph");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the first cyclic package fixture");
    fs::create_dir_all(store_root.join("core/src/root"))
        .expect("Should create the second cyclic package fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\ndep.core: pkg:core\n",
    )
    .expect("Should write the first package metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_build_fixture("json", &[("core", "pkg", "core")]),
    )
    .expect("Should write the first package build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "var[exp] answer: int = 1;\n",
    )
    .expect("Should write the first package source fixture");
    fs::write(
        store_root.join("core/build.fol"),
        "name: core\nversion: 1.0.0\ndep.json: pkg:json\n",
    )
    .expect("Should write the second package metadata fixture");
    fs::write(
        store_root.join("core/build.fol"),
        formal_build_fixture("core", &[("json", "pkg", "json")]),
    )
    .expect("Should write the second package build fixture");
    fs::write(
        store_root.join("core/src/root/value.fol"),
        "var[exp] shared: int = 2;\n",
    )
    .expect("Should write the second package source fixture");
    let mut session = PackageSession::new();

    let error = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "json".to_string(),
            }],
        )
        .expect_err("Package session should reject cyclic package dependency graphs");

    assert_eq!(error.kind(), crate::PackageErrorKind::ImportCycle);
    assert!(error
        .to_string()
        .contains("package import cycle detected while loading package roots"));
    assert!(
        error.to_string().contains("json") && error.to_string().contains("core"),
        "Cycle diagnostics should list the participating package roots",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

#[test]
fn package_session_dedupes_shared_transitive_pkg_dependencies() {
    let temp_root = unique_temp_root("shared_pkg_graph");
    let store_root = temp_root.join("store");
    fs::create_dir_all(store_root.join("core/src/root"))
        .expect("Should create the shared dependency export root fixture");
    fs::create_dir_all(store_root.join("json/src/root"))
        .expect("Should create the first direct dependency export root fixture");
    fs::create_dir_all(store_root.join("xml/src/root"))
        .expect("Should create the second direct dependency export root fixture");
    fs::create_dir_all(store_root.join("combo/src/root"))
        .expect("Should create the top-level package export root fixture");
    fs::write(
        store_root.join("core/build.fol"),
        "name: core\nversion: 1.0.0\n",
    )
    .expect("Should write the shared dependency metadata fixture");
    fs::write(
        store_root.join("core/build.fol"),
        formal_build_fixture("core", &[]),
    )
    .expect("Should write the shared dependency build fixture");
    fs::write(
        store_root.join("core/src/root/value.fol"),
        "var[exp] shared: int = 7;\n",
    )
    .expect("Should write the shared dependency source fixture");
    fs::write(
        store_root.join("json/build.fol"),
        "name: json\nversion: 1.0.0\ndep.core: pkg:core\n",
    )
    .expect("Should write the first direct dependency metadata fixture");
    fs::write(
        store_root.join("json/build.fol"),
        formal_build_fixture("json", &[("core", "pkg", "core")]),
    )
    .expect("Should write the first direct dependency build fixture");
    fs::write(
        store_root.join("json/src/root/value.fol"),
        "var[exp] answer: int = 1;\n",
    )
    .expect("Should write the first direct dependency source fixture");
    fs::write(
        store_root.join("xml/build.fol"),
        "name: xml\nversion: 1.0.0\ndep.core: pkg:core\n",
    )
    .expect("Should write the second direct dependency metadata fixture");
    fs::write(
        store_root.join("xml/build.fol"),
        formal_build_fixture("xml", &[("core", "pkg", "core")]),
    )
    .expect("Should write the second direct dependency build fixture");
    fs::write(
        store_root.join("xml/src/root/value.fol"),
        "var[exp] answer: int = 2;\n",
    )
    .expect("Should write the second direct dependency source fixture");
    fs::write(
        store_root.join("combo/build.fol"),
        "name: combo\nversion: 1.0.0\ndep.json: pkg:json\ndep.xml: pkg:xml\n",
    )
    .expect("Should write the top-level package metadata fixture");
    fs::write(
        store_root.join("combo/build.fol"),
        formal_build_fixture("combo", &[("json", "pkg", "json"), ("xml", "pkg", "xml")]),
    )
    .expect("Should write the top-level package build fixture");
    fs::write(
        store_root.join("combo/src/root/value.fol"),
        "var[exp] answer: int = 3;\n",
    )
    .expect("Should write the top-level package source fixture");
    let mut session = PackageSession::new();

    let loaded = session
        .load_package_from_store(
            &store_root,
            &[UsePathSegment {
                separator: None,
                spelling: "combo".to_string(),
            }],
        )
        .expect("Package session should load shared dependency graphs");

    assert_eq!(loaded.identity.display_name, "combo");
    assert_eq!(session.cached_package_count(), 4);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary package-store fixture should be removable after the test");
}

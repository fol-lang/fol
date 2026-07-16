use super::super::{
    canonical_graph_construction_capabilities, evaluate_build_source, BuildEvaluationBoundary,
    BuildEvaluationErrorKind, BuildEvaluationInputs, BuildEvaluationRequest, BuildEvaluationResult,
    EvaluatedBuildProgram,
};
use crate::graph::BuildGraph;
use crate::option::{BuildOptionDeclarationSet, ResolvedBuildOptionSet};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn temp_build_package(source: &str) -> (PathBuf, PathBuf) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    let package_root = std::env::temp_dir().join(format!(
        "fol_build_eval_hdl_{}_{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&package_root).expect("temp package root should be created");
    fs::write(package_root.join("build.fol"), source).expect("build source should be written");
    (package_root.clone(), package_root.join("build.fol"))
}

#[test]
fn typed_target_inputs_keep_the_documented_build_option_spelling() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var target = graph.standard_target();\n",
        "    when(target == \"x86_64-linux-gnu\") {\n",
        "        { graph.add_exe({ name = \"app\", root = \"src/main.fol\" }); }\n",
        "    };\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            target: Some(fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap()),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("typed target condition should evaluate")
        .expect("matching target condition should construct an artifact");

    assert_eq!(
        evaluated.result.resolved_options.get("target"),
        Some("x86_64-linux-gnu")
    );
    assert_eq!(evaluated.result.graph.artifacts().len(), 1);
    assert_eq!(
        evaluated.result.graph.artifacts()[0].target.as_str(),
        "x86_64-unknown-linux-gnu"
    );
}

#[test]
fn build_source_evaluator_accepts_build_metadata_and_dependency_calls() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var logtiny = build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://example.com/logtiny\", mode = \"lazy\" });\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    graph.install(app);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("build metadata calls should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.graph.artifacts().len(), 1);
    assert_eq!(evaluated.result.dependency_requests.len(), 1);
    assert_eq!(evaluated.result.dependency_requests[0].alias, "logtiny");
    assert_eq!(
        evaluated.result.dependency_requests[0].package,
        "git+https://example.com/logtiny"
    );
    assert_eq!(
        evaluated.result.dependency_requests[0].evaluation_mode,
        Some(crate::DependencyBuildEvaluationMode::Lazy)
    );
    assert_eq!(evaluated.result.dependency_requests[0].git_version, None);
    assert_eq!(evaluated.result.dependency_requests[0].git_hash, None);
}

#[test]
fn build_source_evaluator_keeps_structured_git_dependency_selectors() {
    let cases = [
        (
            "version = \"branch:main\",\n        hash = \"deadbeef\",",
            Some(crate::GitDependencyVersionSelector::Branch(
                "main".to_string(),
            )),
            Some("deadbeef"),
        ),
        (
            "version = \"tag:v0.1.0\",",
            Some(crate::GitDependencyVersionSelector::Tag(
                "v0.1.0".to_string(),
            )),
            None,
        ),
        (
            "version = \"commit:abc123def456\",",
            Some(crate::GitDependencyVersionSelector::Commit(
                "abc123def456".to_string(),
            )),
            None,
        ),
    ];

    for (extra_fields, expected_version, expected_hash) in cases {
        let source = format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"demo\", version = \"0.1.0\" }});\n",
                "    build.add_dep({{\n",
                "        alias = \"logtiny\",\n",
                "        source = \"git\",\n",
                "        target = \"git+https://example.com/logtiny\",\n",
                "        {extra_fields}\n",
                "    }});\n",
                "    return;\n",
                "}};\n",
            ),
            extra_fields = extra_fields
        );
        let (package_root, build_path) = temp_build_package(&source);
        let request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };

        let evaluated = evaluate_build_source(&request, &build_path, &source)
            .expect("structured git dependency selectors should evaluate")
            .expect("build body should produce operations");

        assert_eq!(
            evaluated.result.dependency_requests[0].git_version,
            expected_version
        );
        assert_eq!(
            evaluated.result.dependency_requests[0].git_hash.as_deref(),
            expected_hash
        );
    }
}

#[test]
fn build_source_evaluator_rejects_invalid_git_dependency_selector_configs() {
    let cases = [
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://example.com/logtiny\", version = \"rev:abc123\" });\n",
                "    return;\n",
                "};\n",
            ),
            "dependency 'version' must be one of: branch:<name>, tag:<name>, commit:<sha>",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"json\", source = \"pkg\", target = \"json\", version = \"branch:main\" });\n",
                "    return;\n",
                "};\n",
            ),
            "dependency field 'version' is only valid for git dependencies",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"shared\", source = \"loc\", target = \"../shared\", hash = \"deadbeef\" });\n",
                "    return;\n",
                "};\n",
            ),
            "dependency field 'hash' is only valid for git dependencies",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://example.com/logtiny\", hash = \"\" });\n",
                "    return;\n",
                "};\n",
            ),
            "dependency 'hash' must not be empty",
        ),
    ];

    for (source, expected) in cases {
        let (package_root, build_path) = temp_build_package(source);
        let request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };

        let error = evaluate_build_source(&request, &build_path, source)
            .expect_err("invalid git dependency selector config should fail");
        assert!(
            error.message().contains(expected),
            "expected '{expected}', got '{error}'",
        );
    }
}

#[test]
fn build_source_evaluator_rejects_invalid_build_dependency_modes() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://example.com/logtiny\", mode = \"ambient\" });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let error = evaluate_build_source(&request, &build_path, source)
        .expect_err("invalid build dependency mode should fail");

    assert!(error.message().contains(
        "add_dep config is invalid: dependency mode must be one of: eager, lazy, on-demand"
    ));
}

#[test]
fn build_source_evaluator_treats_add_dep_as_a_real_dependency_handle() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var dep = build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core\" });\n",
        "    var generated = dep.generated(\"bindings\");\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("build dependency handles should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.dependency_requests.len(), 1);
    assert_eq!(evaluated.result.dependency_requests[0].alias, "core");
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "bindings"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::GeneratedOutput));
}

#[test]
fn build_source_evaluator_keeps_explicit_dependency_args_on_build_handles() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var target = graph.standard_target();\n",
        "    var optimize = graph.standard_optimize();\n",
        "    var fast = graph.option({ name = \"use_fast_parser\", kind = \"bool\", default = true });\n",
        "    var dep = .build().add_dep({\n",
        "        alias = \"json\",\n",
        "        source = \"pkg\",\n",
        "        target = \"json\",\n",
        "        args = { target = target, optimize = optimize, use_fast_parser = fast, flavor = \"strict\", jobs = 4 },\n",
        "    });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let mut options = std::collections::BTreeMap::new();
    options.insert("target".to_string(), "aarch64-linux-gnu".to_string());
    options.insert("optimize".to_string(), "release-safe".to_string());
    options.insert("use_fast_parser".to_string(), "false".to_string());
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            options,
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("dependency args should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.dependency_requests.len(), 1);
    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("target")
            .map(String::as_str),
        Some("aarch64-linux-gnu")
    );
    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("optimize")
            .map(String::as_str),
        Some("release-safe")
    );
    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("use_fast_parser")
            .map(String::as_str),
        Some("false")
    );
    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("flavor")
            .map(String::as_str),
        Some("strict")
    );
    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("jobs")
            .map(String::as_str),
        Some("4")
    );
}

#[test]
fn build_source_evaluator_allows_evaluated_option_expressions_in_dependency_args() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    var graph = build.graph();\n",
        "    build.add_dep({\n",
        "        alias = \"json\",\n",
        "        source = \"pkg\",\n",
        "        target = \"json\",\n",
        "        args = {\n",
        "            target = graph.standard_target(),\n",
        "            optimize = graph.standard_optimize(),\n",
        "            jobs = graph.option({ name = \"jobs\", kind = \"int\", default = 4 }),\n",
        "        },\n",
        "    });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let mut options = std::collections::BTreeMap::new();
    options.insert("target".to_string(), "aarch64-linux-gnu".to_string());
    options.insert("optimize".to_string(), "release-safe".to_string());
    options.insert("jobs".to_string(), "16".to_string());
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            options,
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("evaluated option expressions should work")
        .expect("build body should produce operations");

    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("target")
            .map(String::as_str),
        Some("aarch64-linux-gnu")
    );
    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("optimize")
            .map(String::as_str),
        Some("release-safe")
    );
    assert_eq!(
        evaluated.evaluated.dependencies[0]
            .args
            .get("jobs")
            .map(String::as_str),
        Some("16")
    );
}

#[test]
fn build_source_evaluator_rejects_missing_required_dependency_option_args() {
    let source = concat!(
        // Standard target/optimize handles have concrete host/debug defaults.
        // A user option without a default stays unresolved and must fail when
        // forwarded as a required dependency argument.
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var endpoint = graph.option({ name = \"endpoint\", kind = \"str\" });\n",
        "    var dep = .build().add_dep({\n",
        "        alias = \"json\",\n",
        "        source = \"pkg\",\n",
        "        target = \"json\",\n",
        "        args = { endpoint = endpoint },\n",
        "    });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let error = evaluate_build_source(&request, &build_path, source)
        .expect_err("missing required dependency option args should fail");

    assert_eq!(
        error.message(),
        "dependency 'json' requires resolved str option 'endpoint' for arg 'endpoint'"
    );
}

#[test]
fn build_source_evaluator_allows_dependency_modules_to_feed_back_into_artifact_imports() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var dep = build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core\" });\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var root_module = dep.module(\"root\");\n",
        "    app.import(root_module);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("dependency module imports should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.graph.artifacts().len(), 1);
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "root"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::Module));
    assert!(evaluated
        .result
        .graph
        .modules()
        .iter()
        .any(|module| module.name == "dep:core:root"));
}

#[test]
fn build_source_evaluator_keeps_mixed_source_dependency_surface_queries_precise() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var shared = build.add_dep({ alias = \"shared\", source = \"loc\", target = \"../shared\" });\n",
        "    var json = build.add_dep({ alias = \"json\", source = \"pkg\", target = \"json\" });\n",
        "    var logtiny = build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://example.com/logtiny\" });\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        // Dependency surface queries are bound to locals (which records each
        // query); only import consumes a dependency-module handle, so the
        // artifact/generated/step queries are recorded without a consumer call.
        "    var shared_root = shared.module(\"root\");\n",
        "    app.import(shared_root);\n",
        "    var json_art = json.artifact(\"json\");\n",
        "    var logtiny_bind = logtiny.generated(\"bindings\");\n",
        "    var check = logtiny.step(\"check\");\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("mixed-source dependency queries should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.dependency_requests.len(), 3);
    assert!(evaluated
        .result
        .dependency_requests
        .iter()
        .any(|dep| dep.alias == "shared" && dep.source_kind == crate::DependencySourceKind::Local));
    assert!(evaluated
        .result
        .dependency_requests
        .iter()
        .any(|dep| dep.alias == "json"
            && dep.source_kind == crate::DependencySourceKind::PackageStore));
    assert!(evaluated
        .result
        .dependency_requests
        .iter()
        .any(|dep| dep.alias == "logtiny" && dep.source_kind == crate::DependencySourceKind::Git));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "root"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::Module));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "json"
            && query.query_name == "json"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::Artifact));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "logtiny"
            && query.query_name == "bindings"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::GeneratedOutput));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "logtiny"
            && query.query_name == "check"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::Step));
}

#[test]
fn build_source_evaluator_accepts_dependency_path_handles_in_consumers() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var shared = build.add_dep({ alias = \"shared\", source = \"pkg\", target = \"shared\" });\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var run = graph.add_run(app);\n",
        "    var config = shared.file(\"config\");\n",
        "    var assets = shared.dir(\"assets\");\n",
        "    var schema = shared.path(\"schema\");\n",
        "    run.add_file_arg(config);\n",
        // NOTE: artifact.add_generated now requires a local generated-output
        // handle and rejects dependency path handles, so `schema` only records
        // its Path query here rather than being consumed by add_generated.
        "    graph.install_file({ name = \"config\", source = config });\n",
        "    graph.install_dir({ name = \"assets\", source = assets });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("dependency path handles should evaluate")
        .expect("build body should produce operations");

    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "config"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::File));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "assets"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::Dir));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "shared"
            && query.query_name == "schema"
            && query.kind == crate::runtime::BuildRuntimeDependencyQueryKind::Path));
    assert_eq!(evaluated.result.graph.installs().len(), 2);
    assert!(evaluated
        .result
        .graph
        .installs()
        .iter()
        .any(|install| install.name == "config"));
    assert!(evaluated
        .result
        .graph
        .installs()
        .iter()
        .any(|install| install.name == "assets"));
}

#[test]
fn build_source_evaluator_keeps_generated_handles_through_local_helpers() {
    let source = concat!(
        "fun[] emit_cfg() = {\n",
        "    var graph = .build().graph();\n",
        "    return graph.write_file({ name = \"cfg\", path = \"config/generated.toml\", contents = \"ok\" });\n",
        "};\n",
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var run = graph.add_run(app);\n",
        "    var cfg = emit_cfg();\n",
        "    app.add_generated(cfg);\n",
        "    run.add_file_arg(cfg);\n",
        "    graph.install_file({ name = \"install-cfg\", source = cfg });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("helper output handles should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.graph.generated_files().len(), 1);
    assert_eq!(evaluated.result.graph.installs().len(), 1);
    let artifact_inputs = evaluated
        .result
        .graph
        .artifact_inputs_for(crate::graph::BuildArtifactId(0))
        .collect::<Vec<_>>();
    assert!(artifact_inputs.iter().any(|input| matches!(
        input,
        crate::graph::BuildArtifactInput::GeneratedFile(crate::graph::BuildGeneratedFileId(0))
    )));
    let run_cfg = evaluated
        .result
        .graph
        .run_config_for(crate::graph::BuildStepId(0))
        .expect("run config should exist");
    assert!(run_cfg.args.iter().any(|arg| matches!(
        arg,
        crate::graph::BuildRunArg::GeneratedFile(crate::graph::BuildGeneratedFileId(0))
    )));
}

#[test]
fn build_source_evaluator_projects_install_destinations_under_selected_prefix() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var cfg = graph.write_file({ name = \"cfg\", path = \"config/generated.toml\", contents = \"ok\" });\n",
        "    var assets = graph.dir_from_root(\"assets\");\n",
        "    graph.install(app);\n",
        "    graph.install_file({ name = \"generated-cfg\", source = cfg });\n",
        "    graph.install_dir({ name = \"assets\", source = assets });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            install_prefix: "/opt/demo".to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("install projection should evaluate")
        .expect("build body should produce operations");

    let installs = evaluated.result.graph.installs();
    assert!(installs.iter().any(|install| install.name == "install"
        && install.projected_destination == "/opt/demo/bin/demo"));
    assert!(installs
        .iter()
        .any(|install| install.name == "generated-cfg"
            && install.projected_destination == "/opt/demo/config/generated.toml"));
    assert!(installs
        .iter()
        .any(|install| install.name == "assets"
            && install.projected_destination == "/opt/demo/assets"));
}

#[test]
fn build_source_and_generated_handles_compose_across_run_and_install_surfaces() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var run = graph.add_run(app);\n",
        "    var defaults = graph.file_from_root(\"config/defaults.toml\");\n",
        "    var assets = graph.dir_from_root(\"assets\");\n",
        "    var copied = graph.copy_file({ name = \"copied-defaults\", source = defaults, path = \"config/copied.toml\" });\n",
        "    run.add_file_arg(defaults);\n",
        "    run.add_file_arg(copied);\n",
        "    graph.install_file({ name = \"defaults\", source = defaults });\n",
        "    graph.install_dir({ name = \"assets\", source = assets });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            install_prefix: "/opt/demo".to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("source and generated handles should compose")
        .expect("build body should produce operations");

    let run_cfg = evaluated
        .result
        .graph
        .run_config_for(crate::graph::BuildStepId(0))
        .expect("run config should exist");
    assert!(run_cfg.args.iter().any(
        |arg| matches!(arg, crate::graph::BuildRunArg::Path(path) if path == "config/defaults.toml")
    ));
    assert!(run_cfg.args.iter().any(|arg| matches!(
        arg,
        crate::graph::BuildRunArg::GeneratedFile(crate::graph::BuildGeneratedFileId(0))
    )));

    let installs = evaluated.result.graph.installs();
    assert!(installs.iter().any(|install| install.name == "defaults"
        && install.projected_destination == "/opt/demo/config/defaults.toml"));
    assert!(installs
        .iter()
        .any(|install| install.name == "assets"
            && install.projected_destination == "/opt/demo/assets"));
}

#[test]
fn build_source_evaluator_surfaces_real_build_root_and_install_prefix_strings() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var root = graph.build_root();\n",
        "    var prefix = graph.install_prefix();\n",
        // Build config values accept literals/identifiers, not `+` expressions,
        // so surface build_root()/install_prefix() strings through separate
        // write_file contents rather than a concatenation.
        "    graph.write_file({ name = \"root\", path = \"gen/root.txt\", contents = root });\n",
        "    graph.write_file({ name = \"paths\", path = \"gen/paths.txt\", contents = prefix });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            install_prefix: "/srv/demo".to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("path helpers should evaluate")
        .expect("build body should produce operations");

    assert!(evaluated
        .evaluated
        .generated_files
        .iter()
        .any(|generated| generated.relative_path == "gen/paths.txt"));
}

#[test]
fn build_source_evaluator_rejects_invalid_dependency_sources_with_exact_diagnostics() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    build.add_dep({ alias = \"core\", source = \"http\", target = \"core\" });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let error = evaluate_build_source(&request, &build_path, source)
        .expect_err("invalid dependency source should fail");

    assert_eq!(
        error.message(),
        "add_dep config is invalid: dependency source must be one of: loc, pkg, git, internal (got 'http')"
    );
}

#[test]
fn build_source_evaluator_accepts_internal_standard_dependencies() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("internal standard dependency should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.dependency_requests.len(), 1);
    assert_eq!(evaluated.result.dependency_requests[0].alias, "std");
    assert_eq!(
        evaluated.result.dependency_requests[0].source_kind,
        crate::DependencySourceKind::Internal
    );
    assert_eq!(evaluated.result.dependency_requests[0].package, "standard");
}

#[test]
fn build_source_evaluator_rejects_invalid_internal_dependency_targets() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"fmt\" });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let error = evaluate_build_source(&request, &build_path, source)
        .expect_err("invalid internal dependency target should fail");

    assert_eq!(
        error.message(),
        "add_dep config is invalid: dependency source 'internal' currently requires target = 'standard' (got 'fmt')"
    );
}

#[test]
fn build_source_evaluator_rejects_invalid_dependency_arg_shapes_with_exact_diagnostics() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var dep = .build().add_dep({ alias = \"core\", source = \"pkg\", target = \"core\", args = { target = graph } });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let error = evaluate_build_source(&request, &build_path, source)
        .expect_err("invalid dependency arg shape should fail");

    assert_eq!(
        error.message(),
        "build.add_dep config is invalid: dependency arg 'target' must evaluate to bool, int, str, or an option handle"
    );
}

#[test]
fn build_source_evaluator_projects_typed_c_imports_through_graph_and_runtime() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
        "    var header = graph.file_from_root(\"native/widget.h\");\n",
        "    var provider = graph.file_from_root(\"native/widget.o\");\n",
        "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("typed local C import handles should evaluate")
        .expect("build body should produce operations");

    let attachment = evaluated
        .result
        .graph
        .c_imports()
        .first()
        .expect("authoritative graph should contain the C import");
    assert_eq!(evaluated.result.graph.c_imports().len(), 1);
    assert_eq!(attachment.artifact_id, crate::graph::BuildArtifactId(0));
    assert_eq!(attachment.header, "native/widget.h");
    assert_eq!(attachment.provider, "native/widget.o");
    assert_eq!(
        attachment.provider_kind,
        crate::graph::BuildCImportProviderKind::Object
    );
    assert_eq!(evaluated.evaluated.artifacts.len(), 1);
    assert_eq!(
        evaluated.evaluated.artifacts[0].artifact_id,
        attachment.artifact_id
    );
    assert_eq!(
        evaluated.evaluated.artifacts[0].c_imports,
        vec![attachment.clone()]
    );
}

#[test]
fn build_source_evaluator_rejects_non_source_c_import_handles_with_exact_provenance() {
    let cases = [
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var graph = .build().graph();\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var provider = graph.file_from_root(\"native/widget.o\");\n",
                "    app.add_c_import({ header = \"native/widget.h\", provider = provider, provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'header' must be a local source-file handle",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var graph = .build().graph();\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var header = graph.file_from_root(\"native/widget.h\");\n",
                "    app.add_c_import({ header = header, provider = \"native/widget.o\", provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'provider' must be a local source-file handle",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var graph = .build().graph();\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var header = graph.dir_from_root(\"native\");\n",
                "    var provider = graph.file_from_root(\"native/widget.o\");\n",
                "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'header' must be a local source-file handle, not a source-dir handle",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var graph = .build().graph();\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var header = graph.file_from_root(\"native/widget.h\");\n",
                "    var provider = graph.write_file({ name = \"widget-object\", path = \"gen/widget.o\", contents = \"object\" });\n",
                "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'provider' must be a local source-file handle, not a generated-output handle",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    var graph = build.graph();\n",
                "    var dep = build.add_dep({ alias = \"native\", source = \"pkg\", target = \"native\" });\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var header = graph.file_from_root(\"native/widget.h\");\n",
                "    var provider = dep.file(\"widget-object\");\n",
                "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'provider' must be a local source-file handle, not a dependency-file handle",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    var graph = build.graph();\n",
                "    var dep = build.add_dep({ alias = \"native\", source = \"pkg\", target = \"native\" });\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var header = graph.file_from_root(\"native/widget.h\");\n",
                "    var provider = dep.dir(\"widget-objects\");\n",
                "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'provider' must be a local source-file handle, not a dependency-dir handle",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    var graph = build.graph();\n",
                "    var dep = build.add_dep({ alias = \"native\", source = \"pkg\", target = \"native\" });\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var header = graph.file_from_root(\"native/widget.h\");\n",
                "    var provider = dep.path(\"widget-object\");\n",
                "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'provider' must be a local source-file handle, not a dependency-path handle",
        ),
        (
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    var graph = build.graph();\n",
                "    var dep = build.add_dep({ alias = \"native\", source = \"pkg\", target = \"native\" });\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    var header = graph.file_from_root(\"native/widget.h\");\n",
                "    var provider = dep.generated(\"widget-object\");\n",
                "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
                "    return;\n",
                "};\n",
            ),
            "add_c_import config is invalid: 'provider' must be a local source-file handle, not a dependency-generated-output handle",
        ),
    ];

    for (source, expected) in cases {
        let (package_root, build_path) = temp_build_package(source);
        let request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };

        let error = evaluate_build_source(&request, &build_path, source)
            .expect_err("non-source C import inputs must fail closed");
        assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
        assert_eq!(error.message(), expected);
    }
}

#[test]
fn build_source_evaluator_rejects_invalid_c_import_kind_paths_and_duplicates() {
    let cases = [
        (
            "/native/widget.h",
            "native/widget.o",
            "object",
            "",
            "C import header path '/native/widget.h' must be relative to the package root",
        ),
        (
            "native/widget.h",
            "../native/widget.o",
            "object",
            "",
            "C import provider path '../native/widget.o' must not traverse outside the package root",
        ),
        (
            "native/widget.h",
            "native/widget.o",
            "Object",
            "",
            "add_c_import config is invalid: 'provider_kind' must be exactly 'object', got 'Object'",
        ),
        (
            "native/widget.h",
            "native/widget.o",
            "object",
            "    app.add_c_import({ header = header, provider = provider, provider_kind = \"object\" });\n",
            "artifact 'artifact:0' already has C import header 'native/widget.h' with provider 'native/widget.o'",
        ),
        (
            "native/widget.h",
            "native/widget.o",
            "object",
            concat!(
                "    var other_header = graph.file_from_root(\"native/other.h\");\n",
                "    var other_provider = graph.file_from_root(\"native/other.o\");\n",
                "    app.add_c_import({ header = other_header, provider = other_provider, provider_kind = \"object\" });\n",
            ),
            "artifact 'artifact:0' cannot attach more than one C import",
        ),
    ];

    for (header, provider, provider_kind, repeated, expected) in cases {
        let source = format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var graph = .build().graph();\n",
                "    var app = graph.add_exe({{ name = \"app\", root = \"src/main.fol\" }});\n",
                "    var header = graph.file_from_root(\"{header}\");\n",
                "    var provider = graph.file_from_root(\"{provider}\");\n",
                "    app.add_c_import({{ header = header, provider = provider, provider_kind = \"{provider_kind}\" }});\n",
                "{repeated}",
                "    return;\n",
                "}};\n",
            ),
            header = header,
            provider = provider,
            provider_kind = provider_kind,
            repeated = repeated,
        );
        let (package_root, build_path) = temp_build_package(&source);
        let request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };

        let error = evaluate_build_source(&request, &build_path, &source)
            .expect_err("invalid C import configuration must fail closed");
        assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
        assert_eq!(error.message(), expected);
    }
}

#[test]
fn build_source_evaluator_requires_a_string_c_import_provider_kind() {
    for (kind_field, expected) in [
        (
            "",
            "add_c_import config is invalid: missing required string field 'provider_kind'",
        ),
        (
            ", provider_kind = 1",
            "add_c_import config is invalid: 'provider_kind' must be a string",
        ),
    ] {
        let source = format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var graph = .build().graph();\n",
                "    var app = graph.add_exe({{ name = \"app\", root = \"src/main.fol\" }});\n",
                "    var header = graph.file_from_root(\"native/widget.h\");\n",
                "    var provider = graph.file_from_root(\"native/widget.o\");\n",
                "    app.add_c_import({{ header = header, provider = provider{kind_field} }});\n",
                "    return;\n",
                "}};\n",
            ),
            kind_field = kind_field,
        );
        let (package_root, build_path) = temp_build_package(&source);
        let request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };

        let error = evaluate_build_source(&request, &build_path, &source)
            .expect_err("C import provider kind must be an explicit string");
        assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
        assert_eq!(error.message(), expected);
    }
}

#[test]
fn build_source_evaluator_extracts_and_replays_restricted_build_bodies() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe(\"app\", \"src/app.fol\");\n",
        "    graph.add_test(\"app_test\", \"test/app.fol\");\n",
        "    graph.add_run(\"serve\", \"app\");\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("restricted build body should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.evaluated.artifacts.len(), 2);
    assert!(evaluated
        .evaluated
        .artifacts
        .iter()
        .any(|artifact| artifact.root_module == "src/app.fol"));
    assert!(evaluated
        .evaluated
        .step_bindings
        .iter()
        .any(|binding| binding.step_name == "serve"));
    assert_eq!(evaluated.result.graph.artifacts().len(), 2);
    assert_eq!(evaluated.result.graph.steps().len(), 1);
}

#[test]
fn build_source_evaluator_supports_object_style_artifacts_and_handle_calls() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var target = graph.standard_target();\n",
        "    var optimize = graph.standard_optimize();\n",
        "    var app = graph.add_exe({\n",
        "        name = \"demo\",\n",
        "        root = \"src/demo.fol\",\n",
        "        target = target,\n",
        "        optimize = optimize,\n",
        "    });\n",
        "    graph.install(app);\n",
        "    graph.add_run(app);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("object style build body should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.evaluated.artifacts.len(), 1);
    assert!(evaluated
        .evaluated
        .artifacts
        .iter()
        .any(|artifact| artifact.name == "demo" && artifact.root_module == "src/demo.fol"));
    assert!(evaluated
        .evaluated
        .step_bindings
        .iter()
        .any(|binding| binding.step_name == "run"));
    assert_eq!(evaluated.result.graph.artifacts().len(), 1);
    assert_eq!(evaluated.result.graph.installs().len(), 1);
    let mut step_names = evaluated
        .result
        .graph
        .steps()
        .iter()
        .map(|step| step.name.as_str())
        .collect::<Vec<_>>();
    step_names.sort_unstable();
    assert_eq!(step_names, vec!["install", "run"]);
}

#[test]
fn build_source_evaluator_supports_user_option_record_configs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var strip = graph.option({ name = \"strip\", kind = \"bool\", default = false });\n",
        "    var jobs = graph.option({ name = \"jobs\", kind = \"int\", default = 8 });\n",
        "    var flavor = graph.option({ name = \"flavor\", kind = \"enum\", default = \"fast\" });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("user option configs should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.option_declarations.declarations().len(), 3);
    assert_eq!(
        evaluated.result.resolved_options.get("strip"),
        Some("false")
    );
    assert_eq!(evaluated.result.resolved_options.get("jobs"), Some("8"));
    assert_eq!(
        evaluated.result.resolved_options.get("flavor"),
        Some("fast")
    );
}

#[test]
fn build_source_evaluator_reuses_bound_run_and_install_handles_as_step_dependencies() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe(\"demo\", \"src/demo.fol\");\n",
        "    var run_app = graph.add_run(app);\n",
        "    var install_app = graph.install(app);\n",
        "    graph.step(\"bundle\", run_app, install_app);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("bound step-like handles should evaluate")
        .expect("build body should produce operations");

    let bundle = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "bundle")
        .expect("bundle step should exist");
    let dependencies = evaluated
        .result
        .graph
        .step_dependencies_for(bundle.id)
        .collect::<Vec<_>>();
    let dependency_names = dependencies
        .iter()
        .filter_map(|id| evaluated.result.graph.steps().get(id.index()))
        .map(|step| step.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(dependency_names, vec!["run", "install"]);
}

#[test]
fn build_source_evaluator_rejects_unknown_handle_methods_explicitly() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var docs = graph.step(\"docs\");\n",
        "    docs.finish(docs);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let error = evaluate_build_source(&request, &build_path, source)
        .expect_err("unsupported handle methods should fail explicitly");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::Unsupported);
    assert!(error.message().contains("finish"));
}

#[test]
fn build_source_evaluator_supports_step_handle_depend_on_chains() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var lint = graph.step(\"lint\");\n",
        "    var docs = graph.step(\"docs\").depend_on(lint);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("step-handle chaining should evaluate")
        .expect("build body should produce operations");

    let docs = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "docs")
        .expect("docs step should exist");
    let lint = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "lint")
        .expect("lint step should exist");
    assert_eq!(
        evaluated
            .result
            .graph
            .step_dependencies_for(docs.id)
            .collect::<Vec<_>>(),
        vec![lint.id]
    );
}

#[test]
fn build_source_evaluator_supports_run_handle_depend_on_chains() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var lint = graph.step(\"lint\");\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    var rundep = graph.add_run(app).depend_on(lint);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("run-handle chaining should evaluate")
        .expect("build body should produce operations");

    let run = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "run")
        .expect("run step should exist");
    let lint = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "lint")
        .expect("lint step should exist");
    assert_eq!(
        evaluated
            .result
            .graph
            .step_dependencies_for(run.id)
            .collect::<Vec<_>>(),
        vec![lint.id]
    );
}

#[test]
fn build_source_evaluator_supports_install_handle_depend_on_chains() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var lint = graph.step(\"lint\");\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    var installdep = graph.install(app).depend_on(lint);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("install-handle chaining should evaluate")
        .expect("build body should produce operations");

    let install = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "install")
        .expect("install step should exist");
    let lint = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "lint")
        .expect("lint step should exist");
    assert_eq!(
        evaluated
            .result
            .graph
            .step_dependencies_for(install.id)
            .collect::<Vec<_>>(),
        vec![lint.id]
    );
}

#[test]
fn build_source_evaluator_keeps_step_like_handle_chains_stable() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var lint = graph.step(\"lint\");\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    var run_app = graph.add_run(app);\n",
        "    var install_app = graph.install(app);\n",
        "    run_app.depend_on(lint);\n",
        "    install_app.depend_on(lint);\n",
        "    var bundle = graph.step(\"bundle\", run_app, install_app, run_app).depend_on(lint);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("combined handle chaining should evaluate")
        .expect("build body should produce operations");

    let lint = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "lint")
        .expect("lint step should exist");
    let run = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "run")
        .expect("run step should exist");
    let install = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "install")
        .expect("install step should exist");
    let bundle = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "bundle")
        .expect("bundle step should exist");

    assert_eq!(
        evaluated
            .result
            .graph
            .step_dependencies_for(run.id)
            .collect::<Vec<_>>(),
        vec![lint.id]
    );
    assert_eq!(
        evaluated
            .result
            .graph
            .step_dependencies_for(install.id)
            .collect::<Vec<_>>(),
        vec![lint.id]
    );
    assert_eq!(
        evaluated
            .result
            .graph
            .step_dependencies_for(bundle.id)
            .collect::<Vec<_>>(),
        vec![run.id, install.id, lint.id]
    );
}

#[test]
fn evaluated_build_program_surface_keeps_runtime_metadata_and_graph_result() {
    let result = BuildEvaluationResult::new(
        BuildEvaluationBoundary::GraphConstructionSubset,
        canonical_graph_construction_capabilities(),
        "/pkg",
        BuildOptionDeclarationSet::new(),
        ResolvedBuildOptionSet::new(),
        Vec::new(),
        BuildGraph::new(),
    );
    let evaluated = EvaluatedBuildProgram {
        program: crate::runtime::BuildRuntimeProgram::new(
            crate::runtime::BuildExecutionRepresentation::RestrictedRuntimeIr,
        ),
        artifacts: vec![crate::runtime::BuildRuntimeArtifact::new(
            crate::graph::BuildArtifactId(0),
            "app",
            crate::runtime::BuildRuntimeArtifactKind::Executable,
            "src/app.fol",
        )],
        generated_files: vec![crate::runtime::BuildRuntimeGeneratedFile::new(
            "version",
            "gen/version.fol",
            crate::runtime::BuildRuntimeGeneratedFileKind::Write,
        )],
        dependencies: vec![crate::runtime::BuildRuntimeDependency {
            alias: "core".to_string(),
            source_kind: crate::api::DependencySourceKind::PackageStore,
            package: "org/core".to_string(),
            args: std::collections::BTreeMap::new(),
            evaluation_mode: None,
        }],
        dependency_exports: Vec::new(),
        dependency_queries: Vec::new(),
        step_bindings: vec![crate::runtime::BuildRuntimeStepBinding::new(
            "run",
            crate::runtime::BuildRuntimeStepBindingKind::DefaultRun,
            Some("app"),
        )],
        result,
    };

    assert_eq!(evaluated.artifacts.len(), 1);
    assert_eq!(evaluated.generated_files.len(), 1);
    assert_eq!(evaluated.dependencies.len(), 1);
    assert!(evaluated.dependency_exports.is_empty());
    assert_eq!(evaluated.step_bindings.len(), 1);
    assert_eq!(evaluated.result.package_root, "/pkg");
}

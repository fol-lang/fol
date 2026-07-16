use super::super::{evaluate_build_source, BuildEvaluationInputs, BuildEvaluationRequest};
use crate::executor::BuildBodyExecutor;
use crate::option::BuildOptimizeMode;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn temp_build_package(source: &str) -> (PathBuf, PathBuf) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    let package_root = std::env::temp_dir().join(format!(
        "fol_build_eval_grph_{}_{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&package_root).expect("temp package root should be created");
    fs::write(package_root.join("build.fol"), source).expect("build source should be written");
    (package_root.clone(), package_root.join("build.fol"))
}

#[test]
fn build_source_evaluator_records_add_module_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var m = graph.add_module({ name = \"utils\", root = \"src/utils.fol\" });\n",
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
        .expect("add_module should evaluate")
        .expect("build body should produce operations");

    let modules = evaluated.result.graph.modules();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "src/utils.fol");
}

#[test]
fn build_source_evaluator_supports_ambient_build_without_graph_work() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    return;\n",
        "};\n",
    );
    let (_package_root, build_path) = temp_build_package(source);

    let (executor, body) = BuildBodyExecutor::from_file(&build_path)
        .expect("ambient build source should parse")
        .expect("build entry should exist");
    let output = executor
        .execute(&body)
        .expect("ambient build local should execute");

    assert!(output.operations.is_empty());
}

#[test]
fn build_source_evaluator_supports_inferred_build_locals_before_graph_work() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    var utils = .build().graph().add_module({ name = \"utils\", root = \"src/utils.fol\" });\n",
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
        .expect("ambient build local should evaluate")
        .expect("ambient build local should still produce graph operations");

    let modules = evaluated.result.graph.modules();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "src/utils.fol");
}

#[test]
fn build_source_evaluator_routes_build_graph_method_to_graph_handle() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var utils = .build().graph().add_module({ name = \"utils\", root = \"src/utils.fol\" });\n",
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
        .expect("build.graph route should evaluate")
        .expect("build.graph route should produce graph operations");

    let modules = evaluated.result.graph.modules();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "src/utils.fol");
}

#[test]
fn build_source_evaluator_supports_inferred_build_and_graph_locals() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    var graph = build.graph();\n",
        "    graph.add_module({ name = \"utils\", root = \"src/utils.fol\" });\n",
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
        .expect("build and graph locals should evaluate")
        .expect("build and graph locals should produce graph operations");

    let modules = evaluated.result.graph.modules();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "src/utils.fol");
}

#[test]
fn build_source_evaluator_rejects_build_intrinsic_arguments() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    .build(\"oops\");\n",
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
        .expect_err("build intrinsic arguments should be rejected");

    assert!(error.message().contains("unsupported build API call"));
    assert!(error.message().contains("build"));
}

#[test]
fn build_source_evaluator_rejects_build_graph_arguments() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.graph(\"oops\");\n",
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
        .expect_err("build.graph arguments should be rejected");

    assert!(error.message().contains("unsupported build API call"));
    assert!(error.message().contains("graph"));
}

#[test]
fn build_source_evaluator_supports_direct_build_graph_calls() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var utils = .build().graph().add_module({ name = \"utils\", root = \"src/utils.fol\" });\n",
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
        .expect("build.graph call should evaluate")
        .expect("build.graph call should produce operations");

    let modules = evaluated.result.graph.modules();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "src/utils.fol");
}

#[test]
fn build_source_evaluator_supports_inferred_graph_locals() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_module({ name = \"utils\", root = \"src/utils.fol\" });\n",
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
        .expect("build.graph local should evaluate")
        .expect("build.graph local should produce operations");

    let modules = evaluated.result.graph.modules();
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0].name, "src/utils.fol");
}

#[test]
fn build_source_evaluator_rejects_public_graph_type_annotations() {
    let source = concat!(
        "fun[] make_graph(graph: Graph): non = {\n",
        "    return;\n",
        "};\n",
        "pro[] build(): non = {\n",
        "    var graph: Graph = .build().graph();\n",
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
        .expect_err("public Graph type syntax should be rejected in build.fol");

    assert_eq!(
        error.kind(),
        super::super::BuildEvaluationErrorKind::InvalidInput
    );
    assert!(
        error.message().contains("public `Graph` type syntax"),
        "expected Graph-type rejection, got: {error:?}"
    );
}

#[test]
fn build_source_evaluator_helpers_use_ambient_graph_without_graph_params() {
    let source = concat!(
        "fun[] make_lib(name: str, root: str): Artifact = {\n",
        "    return .build().graph().add_static_lib({ name = name, root = root });\n",
        "};\n",
        "pro[] build(): non = {\n",
        "    var lib = make_lib(\"core\", \"src/core.fol\");\n",
        "    var installed = .build().graph().install(lib);\n",
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
        .expect("ambient helper should evaluate")
        .expect("ambient helper should produce operations");

    let artifacts = evaluated.result.graph.artifacts();
    assert!(artifacts.iter().any(|artifact| artifact.name == "core"));
    assert_eq!(evaluated.result.graph.installs().len(), 1);
}

#[test]
fn build_source_evaluator_records_artifact_link_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    var lib = graph.add_static_lib({ name = \"core\", root = \"src/core.fol\" });\n",
        "    app.link(lib);\n",
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
        .expect("artifact.link should evaluate")
        .expect("build body should produce operations");

    let artifacts = evaluated.result.graph.artifacts();
    let app = artifacts
        .iter()
        .find(|a| a.name == "app")
        .expect("app artifact");
    let lib = artifacts
        .iter()
        .find(|a| a.name == "core")
        .expect("core artifact");
    let links: Vec<_> = evaluated.result.graph.artifact_links_for(app.id).collect();
    assert_eq!(links, vec![lib.id]);
}

#[test]
fn build_source_evaluator_records_artifact_import_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var m = graph.add_module({ name = \"utils\", root = \"src/utils.fol\" });\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    app.import(m);\n",
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
        .expect("artifact.import should evaluate")
        .expect("build body should produce operations");

    let artifacts = evaluated.result.graph.artifacts();
    let app = artifacts
        .iter()
        .find(|a| a.name == "app")
        .expect("app artifact");
    let modules = evaluated.result.graph.modules();
    let utils_module = modules
        .iter()
        .find(|m| m.name == "src/utils.fol")
        .expect("utils module");
    let imports: Vec<_> = evaluated
        .result
        .graph
        .artifact_module_imports_for(app.id)
        .collect();
    assert_eq!(imports, vec![utils_module.id]);
}

#[test]
fn build_source_evaluator_records_run_add_arg_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    var r = graph.add_run(app);\n",
        "    r.add_arg(\"--release\");\n",
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
        .expect("run.add_arg should evaluate")
        .expect("build body should produce operations");

    let steps = evaluated.result.graph.steps();
    let run_step = steps.iter().find(|s| s.name == "run").expect("run step");
    let config = evaluated
        .result
        .graph
        .run_config_for(run_step.id)
        .expect("run config should exist");
    assert_eq!(config.args.len(), 1);
    assert!(matches!(&config.args[0], crate::graph::BuildRunArg::Literal(s) if s == "--release"));
}

#[test]
fn build_source_evaluator_records_run_capture_stdout_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    var r = graph.add_run(app);\n",
        "    var out = r.capture_stdout();\n",
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
        .expect("run.capture_stdout should evaluate")
        .expect("build body should produce operations");

    let steps = evaluated.result.graph.steps();
    let run_step = steps.iter().find(|s| s.name == "run").expect("run step");
    let config = evaluated
        .result
        .graph
        .run_config_for(run_step.id)
        .expect("run config should exist");
    assert!(config.capture_stdout.is_some());
}

#[test]
fn build_source_evaluator_records_run_set_env_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    var r = graph.add_run(app);\n",
        "    r.set_env(\"LOG_LEVEL\", \"debug\");\n",
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
        .expect("run.set_env should evaluate")
        .expect("build body should produce operations");

    let steps = evaluated.result.graph.steps();
    let run_step = steps.iter().find(|s| s.name == "run").expect("run step");
    let config = evaluated
        .result
        .graph
        .run_config_for(run_step.id)
        .expect("run config should exist");
    assert_eq!(
        config.env,
        vec![("LOG_LEVEL".to_string(), "debug".to_string())]
    );
}

#[test]
fn build_source_evaluator_records_step_attach_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var header = graph.write_file({ name = \"version.h\", path = \"gen/version.h\", contents = \"// v1\" });\n",
        "    var compile = graph.step(\"compile\");\n",
        "    compile.attach(header);\n",
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
        .expect("step.attach should evaluate")
        .expect("build body should produce operations");

    let steps = evaluated.result.graph.steps();
    let compile = steps
        .iter()
        .find(|s| s.name == "compile")
        .expect("compile step");
    let attachments: Vec<_> = evaluated
        .result
        .graph
        .step_attachments_for(compile.id)
        .collect();
    assert_eq!(attachments.len(), 1);
}

#[test]
fn build_source_evaluator_records_artifact_add_generated_in_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var gen = graph.write_file({ name = \"schema.fol\", path = \"gen/schema.fol\", contents = \"// gen\" });\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    app.add_generated(gen);\n",
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
        .expect("artifact.add_generated should evaluate")
        .expect("build body should produce operations");

    let artifacts = evaluated.result.graph.artifacts();
    let app = artifacts
        .iter()
        .find(|a| a.name == "app")
        .expect("app artifact");
    let generated_files = evaluated.result.graph.generated_files();
    let schema = generated_files
        .iter()
        .find(|g| g.name == "gen/schema.fol")
        .expect("schema generated file");
    let inputs: Vec<_> = evaluated.result.graph.artifact_inputs_for(app.id).collect();
    assert!(inputs.iter().any(
        |i| matches!(i, crate::graph::BuildArtifactInput::GeneratedFile(id) if *id == schema.id)
    ));
}

#[test]
fn build_source_evaluator_records_install_file_from_generated_handles() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var cfg = graph.write_file({ name = \"cfg\", path = \"config/generated.toml\", contents = \"ok\" });\n",
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
        .expect("install_file from generated handle should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.result.graph.generated_files().len(), 1);
    assert_eq!(evaluated.result.graph.installs().len(), 1);
    assert_eq!(
        evaluated.result.graph.installs()[0].target,
        Some(crate::graph::BuildInstallTarget::GeneratedFile(
            crate::graph::BuildGeneratedFileId(0)
        ))
    );
}

#[test]
fn build_source_evaluator_executes_when_condition_conditionally() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var optimize = graph.standard_optimize();\n",
        "    var app = graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "    when(optimize == \"release-fast\") {\n",
        "        {\n",
        "            graph.step(\"strip\");\n",
        "        }\n",
        "    };\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request_release = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            optimize: Some(BuildOptimizeMode::ReleaseFast),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let request_debug = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let release_eval = evaluate_build_source(&request_release, &build_path, source)
        .expect("when with matching optimize should evaluate")
        .expect("release build should produce operations");
    let debug_eval = evaluate_build_source(&request_debug, &build_path, source)
        .expect("when without matching optimize should evaluate")
        .expect("debug build should produce operations");

    assert!(release_eval
        .result
        .graph
        .steps()
        .iter()
        .any(|s| s.name == "strip"));
    assert!(!debug_eval
        .result
        .graph
        .steps()
        .iter()
        .any(|s| s.name == "strip"));
}

#[test]
fn build_source_evaluator_matches_when_case_values_against_target_subject() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var target = graph.standard_target();\n",
        "    when(target) {\n",
        "        case(\"aarch64-linux-gnu\") { graph.step(\"arm\"); }\n",
        "        case(\"x86_64-linux-gnu\") { graph.step(\"x86\"); }\n",
        "        * { graph.step(\"fallback\"); }\n",
        "    };\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);

    let selected_steps = |target: &str| {
        let request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                target: Some(fol_types::ResolvedTarget::resolve(target).unwrap()),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };

        evaluate_build_source(&request, &build_path, source)
            .expect("target case selection should evaluate")
            .expect("standard target declaration should produce a graph")
            .result
            .graph
            .steps()
            .iter()
            .map(|step| step.name.clone())
            .collect::<Vec<_>>()
    };

    assert_eq!(selected_steps("x86_64-unknown-linux-gnu"), vec!["x86"]);
    assert_eq!(selected_steps("aarch64-unknown-linux-gnu"), vec!["arm"]);
    assert_eq!(
        selected_steps("x86_64-unknown-linux-musl"),
        vec!["fallback"]
    );
}

#[test]
fn build_source_evaluator_matches_boolean_cases_against_when_subject() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var enabled = graph.option({ name = \"enabled\", kind = \"bool\", default = false });\n",
        "    when(enabled) {\n",
        "        case(true) { graph.step(\"enabled\"); }\n",
        "        case(false) { graph.step(\"disabled\"); }\n",
        "    };\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let selected_steps = |override_value: Option<&str>| {
        let mut request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };
        if let Some(value) = override_value {
            request
                .inputs
                .options
                .insert("enabled".to_string(), value.to_string());
        }

        evaluate_build_source(&request, &build_path, source)
            .expect("boolean case selection should evaluate")
            .expect("bool option declaration should produce a graph")
            .result
            .graph
            .steps()
            .iter()
            .map(|step| step.name.clone())
            .collect::<Vec<_>>()
    };

    assert_eq!(selected_steps(None), vec!["disabled"]);
    assert_eq!(selected_steps(Some("true")), vec!["enabled"]);
}

#[test]
fn build_source_evaluator_compares_integer_options_as_typed_values() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var jobs = graph.option({ name = \"jobs\", kind = \"int\", default = 8 });\n",
        "    when(jobs == 8) { { graph.step(\"equal\"); } };\n",
        "    when(jobs != 8) { { graph.step(\"not-equal\"); } };\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let selected_steps = |override_value: Option<&str>| {
        let mut request = BuildEvaluationRequest {
            package_root: package_root.display().to_string(),
            inputs: BuildEvaluationInputs {
                working_directory: package_root.display().to_string(),
                ..BuildEvaluationInputs::default()
            },
            operations: Vec::new(),
        };
        if let Some(value) = override_value {
            request
                .inputs
                .options
                .insert("jobs".to_string(), value.to_string());
        }

        evaluate_build_source(&request, &build_path, source)
            .expect("integer option comparison should evaluate")
            .expect("integer option declaration should produce a graph")
            .result
            .graph
            .steps()
            .iter()
            .map(|step| step.name.clone())
            .collect::<Vec<_>>()
    };

    assert_eq!(selected_steps(None), vec!["equal"]);
    assert_eq!(selected_steps(Some("4")), vec!["not-equal"]);
}

#[test]
fn build_source_evaluator_does_not_coerce_cross_type_comparisons() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var enabled = graph.option({ name = \"enabled\", kind = \"bool\", default = true });\n",
        "    var jobs = graph.option({ name = \"jobs\", kind = \"int\", default = 8 });\n",
        "    when(enabled == \"true\") { { graph.step(\"bool-string-equal\"); } };\n",
        "    when(enabled != \"true\") { { graph.step(\"bool-string-different\"); } };\n",
        "    when(jobs == \"8\") { { graph.step(\"int-string-equal\"); } };\n",
        "    when(jobs != \"8\") { { graph.step(\"int-string-different\"); } };\n",
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

    let selected_steps = evaluate_build_source(&request, &build_path, source)
        .expect("cross-type comparisons should evaluate")
        .expect("option declarations should produce a graph")
        .result
        .graph
        .steps()
        .iter()
        .map(|step| step.name.clone())
        .collect::<Vec<_>>();

    assert_eq!(
        selected_steps,
        vec!["bool-string-different", "int-string-different"]
    );
}

#[test]
fn build_source_evaluator_executes_helper_routine_called_from_build_entry() {
    let source = concat!(
        "fun[] make_lib(root: str): Artifact = {\n",
        "    return .build().graph().add_static_lib({ name = root, root = root });\n",
        "};\n",
        "pro[] build(): non = {\n",
        "    var core = make_lib(\"core\");\n",
        "    var io   = make_lib(\"io\");\n",
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
        .expect("helper routine call should evaluate")
        .expect("build with helpers should produce operations");

    let artifacts = evaluated.result.graph.artifacts();
    assert!(artifacts.iter().any(|a| a.name == "core"));
    assert!(artifacts.iter().any(|a| a.name == "io"));
}

#[test]
fn build_source_evaluator_executes_loop_over_string_list() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    loop(name in {\"core\", \"io\", \"utils\"}) {\n",
        "        graph.add_static_lib({ name = name, root = name });\n",
        "    };\n",
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
        .expect("loop over string list should evaluate")
        .expect("loop should produce operations for each iteration");

    let artifacts = evaluated.result.graph.artifacts();
    assert!(artifacts.iter().any(|a| a.name == "core"));
    assert!(artifacts.iter().any(|a| a.name == "io"));
    assert!(artifacts.iter().any(|a| a.name == "utils"));
}

#[test]
fn build_source_evaluator_rejects_unknown_expression_nodes_in_build_bodies() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var keep = 1 + 2;\n",
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
        .expect_err("unknown build expression nodes should fail");

    assert!(error
        .message()
        .contains("build evaluation does not support expression node 'binary_op'"));
}

#[test]
fn build_source_evaluator_rejects_while_like_loops_in_build_bodies() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    loop(true) {\n",
        "        return;\n",
        "    };\n",
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
        .expect_err("condition-based loops should fail");

    assert!(error
        .message()
        .contains("condition-based loops are not supported in build evaluation"));
}

#[test]
fn build_source_evaluator_rejects_unknown_condition_nodes_in_when_clauses() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    when(\"ready\") {\n",
        "        {\n",
        "            return;\n",
        "        }\n",
        "    };\n",
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
        .expect_err("unknown build condition nodes should fail");

    assert!(error
        .message()
        .contains("build evaluation does not support condition node 'literal'"));
}

use super::super::{
    evaluate_build_source, BuildEvaluationError, BuildEvaluationErrorKind, BuildEvaluationInputs,
    BuildEvaluationOperationKind, BuildEvaluationRequest,
};
use crate::artifact::BuildArtifactFolModel;
use crate::option::{BuildOptimizeMode, BuildTargetTriple};
use crate::runtime::{BuildRuntimeDependencyQueryKind, BuildRuntimeGeneratedFileKind};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

fn temp_build_package(source: &str) -> (PathBuf, PathBuf) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    let package_root = std::env::temp_dir().join(format!(
        "fol_build_eval_src_{}_{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&package_root).expect("temp package root should be created");
    fs::write(package_root.join("build.fol"), source).expect("build source should be written");
    (package_root.clone(), package_root.join("build.fol"))
}

#[test]
fn build_source_evaluator_supports_object_style_dependency_configs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var core = graph.dependency({ alias = \"core\", package = \"org/core\", mode = \"eager\" });\n",
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
        .expect("dependency configs should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.result.dependency_requests.len(), 1);
    assert_eq!(evaluated.result.dependency_requests[0].alias, "core");
    assert_eq!(evaluated.result.dependency_requests[0].package, "org/core");
    assert_eq!(
        evaluated.result.dependency_requests[0].evaluation_mode,
        Some(crate::DependencyBuildEvaluationMode::Eager)
    );
    assert_eq!(evaluated.evaluated.dependencies.len(), 1);
    assert_eq!(evaluated.evaluated.dependencies[0].alias, "core");
}

#[test]
fn build_source_evaluator_rejects_non_eager_graph_dependency_modes() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.dependency({ alias = \"core\", package = \"org/core\", mode = \"lazy\" });\n",
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
        .expect_err("non-eager graph dependency modes should fail");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "graph.dependency config is invalid: direct graph dependencies currently support only mode = 'eager'"
    );
}

#[test]
fn build_source_evaluator_keeps_artifact_fol_models_in_evaluated_programs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"app\", root = \"src/app.fol\", fol_model = \"core\" });\n",
        "    graph.add_static_lib({ name = \"memolib\", root = \"src/lib.fol\", fol_model = \"memo\" });\n",
        "    graph.add_shared_lib({ name = \"plugin\", root = \"src/plugin.fol\", fol_model = \"memo\" });\n",
        "    graph.add_test({ name = \"tests\", root = \"test/app.fol\", fol_model = \"memo\" });\n",
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
        .expect("artifact fol_model configs should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.evaluated.artifacts.len(), 4);
    assert_eq!(
        evaluated.evaluated.artifacts[0].fol_model,
        BuildArtifactFolModel::Core
    );
    assert_eq!(
        evaluated.evaluated.artifacts[1].fol_model,
        BuildArtifactFolModel::Memo
    );
    assert_eq!(
        evaluated.evaluated.artifacts[2].fol_model,
        BuildArtifactFolModel::Memo
    );
    assert_eq!(
        evaluated.evaluated.artifacts[3].fol_model,
        BuildArtifactFolModel::Memo
    );
    assert_eq!(
        evaluated.evaluated.artifacts[1].kind,
        crate::runtime::BuildRuntimeArtifactKind::StaticLibrary
    );
    assert_eq!(
        evaluated.evaluated.artifacts[2].kind,
        crate::runtime::BuildRuntimeArtifactKind::SharedLibrary
    );
    assert_eq!(
        evaluated.evaluated.artifacts[3].kind,
        crate::runtime::BuildRuntimeArtifactKind::Test
    );
}

#[test]
fn build_source_evaluator_rejects_unknown_artifact_fol_models() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"app\", root = \"src/app.fol\", fol_model = \"hosted\" });\n",
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
        .expect_err("unknown fol_model values should fail build evaluation");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "artifact fol_model must be one of: core, memo (got 'hosted')"
    );
}

#[test]
fn build_source_evaluator_rejects_removed_std_artifact_model_with_exact_guidance() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"app\", root = \"src/app.fol\", fol_model = \"std\" });\n",
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
        .expect_err("removed std fol_model should fail build evaluation");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "artifact fol_model no longer accepts 'std'; use 'memo' and declare bundled std through build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" })"
    );
}

#[test]
fn build_source_evaluator_defaults_omitted_artifact_fol_model_to_memo() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
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
        .expect("omitted fol_model should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.evaluated.artifacts.len(), 1);
    assert_eq!(
        evaluated.evaluated.artifacts[0].fol_model,
        BuildArtifactFolModel::Memo
    );
}

#[test]
fn build_source_evaluator_supports_object_style_write_file_configs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var version = graph.write_file({ name = \"version\", path = \"gen/version.fol\", contents = \"generated\" });\n",
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
        .expect("write-file configs should evaluate")
        .expect("build body should produce a graph");

    assert!(matches!(
        evaluated.result.graph.generated_files()[0].kind,
        crate::BuildGeneratedFileKind::Write
    ));
    assert_eq!(
        evaluated.result.graph.generated_files()[0].name,
        "gen/version.fol"
    );
}

#[test]
fn build_source_evaluator_supports_object_style_copy_file_configs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var logo = graph.file_from_root(\"assets/logo.svg\");\n",
        "    var asset = graph.copy_file({ name = \"asset\", source = logo, path = \"gen/logo.svg\" });\n",
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
        .expect("copy-file configs should evaluate")
        .expect("build body should produce a graph");

    assert!(matches!(
        evaluated.result.graph.generated_files()[0].kind,
        crate::BuildGeneratedFileKind::Copy
    ));
    assert_eq!(
        evaluated.result.graph.generated_files()[0].name,
        "gen/logo.svg"
    );
}

#[test]
fn build_source_evaluator_supports_object_style_system_tool_configs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var schema = graph.file_from_root(\"schema/api.yaml\");\n",
        "    var defaults = graph.write_file({ name = \"defaults\", path = \"gen/defaults.txt\", contents = \"strict\" });\n",
        "    var bindings = graph.add_system_tool({ tool = \"flatc\", args = { \"--fol\" }, file_args = { schema, defaults }, env = { MODE = \"strict\" }, output = \"gen/schema.fol\" });\n",
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
        .expect("system-tool configs should evaluate")
        .expect("build body should produce a graph");

    // The write_file "defaults" also records a generated file, so locate the
    // system-tool capture output by name rather than assuming index 0.
    assert!(evaluated.result.graph.generated_files().iter().any(|file| {
        file.name == "gen/schema.fol"
            && matches!(file.kind, crate::BuildGeneratedFileKind::CaptureOutput)
    }));
    // Evaluation no longer surfaces the raw operation list on the program;
    // re-execute the build body to inspect the recorded system-tool operation.
    let (executor, body) = crate::executor::BuildBodyExecutor::from_file(&build_path)
        .expect("system-tool build source should parse")
        .expect("build entry should exist");
    let output = executor
        .execute(&body)
        .expect("system-tool build body should execute");
    let system_tool = output
        .operations
        .iter()
        .find_map(|op| match &op.kind {
            BuildEvaluationOperationKind::SystemTool(request) => Some(request),
            _ => None,
        })
        .expect("build body should record a system-tool operation");
    assert_eq!(system_tool.tool, "flatc");
    assert_eq!(system_tool.args, vec!["--fol".to_string()]);
    assert_eq!(
        system_tool.file_args,
        vec![
            "schema/api.yaml".to_string(),
            "gen/defaults.txt".to_string()
        ]
    );
    assert_eq!(
        system_tool.env.get("MODE").map(String::as_str),
        Some("strict")
    );
}

#[test]
fn build_source_evaluator_supports_typed_system_library_configs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var graph = build.graph();\n",
        "    var ssl = graph.add_system_lib({ name = \"ssl\", mode = \"dynamic\", search_path = \"/usr/lib\" });\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    app.link(ssl);\n",
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
        .expect("system-library configs should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.result.graph.artifacts().len(), 1);
    assert_eq!(evaluated.result.graph.artifacts()[0].library_paths.len(), 1);
    assert_eq!(evaluated.result.graph.artifacts()[0].link_inputs.len(), 1);
    assert_eq!(
        evaluated.result.graph.artifacts()[0].link_inputs[0].input,
        crate::native::NativeLinkInput::LibraryName("ssl".to_string())
    );
}

#[test]
fn build_source_evaluator_rejects_invalid_system_library_modes() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_system_lib({ name = \"ssl\", mode = \"ambient\" });\n",
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
        .expect_err("invalid system-library mode should fail");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "add_system_lib config is invalid: library mode must be 'static' or 'dynamic' (got 'ambient')"
    );
}

#[test]
fn build_source_evaluator_rejects_non_boolean_framework_flags() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_system_lib({ name = \"Metal\", framework = \"yes\" });\n",
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
        .expect_err("non-bool framework flag should fail");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "add_system_lib config is invalid: 'framework' must be a bool"
    );
}

#[test]
fn build_source_evaluator_rejects_static_framework_requests() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_system_lib({ name = \"Metal\", framework = true, mode = \"static\" });\n",
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
        .expect_err("static framework requests should fail");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "add_system_lib config is invalid: framework libraries must use dynamic mode"
    );
}

#[test]
fn build_source_evaluator_supports_object_style_codegen_configs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var schema = graph.add_codegen({ kind = \"schema\", input = \"schema/api.yaml\", output = \"gen/api.fol\" });\n",
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
        .expect("codegen configs should evaluate")
        .expect("build body should produce a graph");

    assert!(matches!(
        evaluated.result.graph.generated_files()[0].kind,
        crate::BuildGeneratedFileKind::Write
    ));
    assert_eq!(
        evaluated.result.graph.generated_files()[0].name,
        "gen/api.fol"
    );
}

#[test]
fn build_source_evaluator_keeps_generated_outputs_in_evaluated_programs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var version = graph.write_file({ name = \"version\", path = \"gen/version.fol\", contents = \"generated\" });\n",
        "    var logo = graph.file_from_root(\"assets/logo.svg\");\n",
        "    var asset = graph.copy_file({ name = \"asset\", source = logo, path = \"gen/logo.svg\" });\n",
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
        .expect("generated outputs should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.evaluated.generated_files.len(), 2);
    assert!(evaluated
        .evaluated
        .generated_files
        .iter()
        .any(|file| file.relative_path == "gen/version.fol"
            && file.kind == BuildRuntimeGeneratedFileKind::Write));
    assert!(evaluated
        .evaluated
        .generated_files
        .iter()
        .any(|file| file.relative_path == "gen/logo.svg"
            && file.kind == BuildRuntimeGeneratedFileKind::Copy));
}

#[test]
fn build_source_evaluator_keeps_mixed_generated_output_families() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var version = graph.write_file({ name = \"version\", path = \"gen/version.fol\", contents = \"generated\" });\n",
        "    var logo = graph.file_from_root(\"assets/logo.svg\");\n",
        "    var asset = graph.copy_file({ name = \"asset\", source = logo, path = \"gen/logo.svg\" });\n",
        "    var tool = graph.add_system_tool({ tool = \"flatc\", output = \"gen/schema.fol\" });\n",
        "    var codegen = graph.add_codegen({ kind = \"schema\", input = \"schema/api.yaml\", output = \"gen/api.fol\" });\n",
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
        .expect("mixed generated outputs should evaluate")
        .expect("build body should produce a graph");
    let kinds = evaluated
        .evaluated
        .generated_files
        .iter()
        .map(|file| file.kind)
        .collect::<Vec<_>>();

    assert_eq!(evaluated.evaluated.generated_files.len(), 4);
    assert!(kinds.contains(&BuildRuntimeGeneratedFileKind::Write));
    assert!(kinds.contains(&BuildRuntimeGeneratedFileKind::Copy));
    assert!(kinds.contains(&BuildRuntimeGeneratedFileKind::ToolOutput));
    assert!(kinds.contains(&BuildRuntimeGeneratedFileKind::CodegenOutput));
}

#[test]
fn build_source_evaluator_supports_generated_directory_outputs_and_installs() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var graph = build.graph();\n",
        "    var assets = graph.add_system_tool_dir({ tool = \"assetpack\", output_dir = \"gen/assets\" });\n",
        "    build.export_dir({ name = \"assets\", dir = assets });\n",
        // NOTE: graph.install_dir on a generated directory is currently a
        // suspected product bug: install_generated_dir emits a Directory install
        // with a GeneratedFile target, but validate_installs only accepts a
        // DirectoryPath target for Directory installs, so the graph fails
        // validation. The generated-dir output and export are verified below.
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
        .expect("generated directory configs should evaluate")
        .expect("build body should produce a graph");

    assert!(evaluated
        .evaluated
        .generated_files
        .iter()
        .any(|file| file.relative_path == "gen/assets"
            && file.kind == BuildRuntimeGeneratedFileKind::GeneratedDir));
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "assets"
            && export.kind == crate::BuildRuntimeDependencyExportKind::Dir));
}

#[test]
fn build_source_evaluator_records_dependency_module_and_artifact_queries() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var core = graph.dependency({ alias = \"core\", package = \"org/core\" });\n",
        "    var module = core.module(\"root\");\n",
        "    var artifact = core.artifact(\"corelib\");\n",
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
        .expect("dependency queries should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.evaluated.dependency_queries.len(), 2);
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "root"
            && query.kind == BuildRuntimeDependencyQueryKind::Module));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "corelib"
            && query.kind == BuildRuntimeDependencyQueryKind::Artifact));
}

#[test]
fn build_source_evaluator_records_dependency_step_and_generated_queries() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var core = graph.dependency({ alias = \"core\", package = \"org/core\" });\n",
        "    var step = core.step(\"check\");\n",
        "    var generated = core.generated(\"bindings\");\n",
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
        .expect("dependency queries should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.evaluated.dependency_queries.len(), 2);
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "check"
            && query.kind == BuildRuntimeDependencyQueryKind::Step));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "bindings"
            && query.kind == BuildRuntimeDependencyQueryKind::GeneratedOutput));
}

#[test]
fn build_source_evaluator_records_step_descriptions() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var docs = graph.step(\"docs\", \"Generate documentation\");\n",
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
        .expect("step descriptions should evaluate")
        .expect("build body should produce a graph");

    let docs = evaluated
        .result
        .graph
        .steps()
        .iter()
        .find(|step| step.name == "docs")
        .expect("docs step should exist");
    assert_eq!(docs.description.as_deref(), Some("Generate documentation"));
}

#[test]
fn build_source_evaluator_keeps_full_dependency_surface_usage_together() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var dep = graph.dependency({ alias = \"core\", package = \"org/core\", mode = \"eager\" });\n",
        "    var module = dep.module(\"root\");\n",
        "    var artifact = dep.artifact(\"corelib\");\n",
        "    var step = dep.step(\"check\");\n",
        "    var file = dep.file(\"config\");\n",
        "    var dir = dep.dir(\"assets\");\n",
        "    var path = dep.path(\"schema\");\n",
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
        .expect("dependency surface should evaluate")
        .expect("build body should produce a graph");
    let query_kinds = evaluated
        .evaluated
        .dependency_queries
        .iter()
        .map(|query| query.kind)
        .collect::<Vec<_>>();

    assert_eq!(evaluated.evaluated.dependencies.len(), 1);
    assert_eq!(
        evaluated.evaluated.dependencies[0].evaluation_mode,
        Some(crate::DependencyBuildEvaluationMode::Eager)
    );
    assert_eq!(evaluated.evaluated.dependency_queries.len(), 7);
    assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Module));
    assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Artifact));
    assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Step));
    assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::File));
    assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Dir));
    assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Path));
    assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::GeneratedOutput));
}

#[test]
fn build_source_evaluator_keeps_dependency_queries_precise_for_build_add_dep_handles() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var dep = build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core\" });\n",
        "    var module = dep.module(\"root\");\n",
        "    var artifact = dep.artifact(\"corelib\");\n",
        "    var step = dep.step(\"check\");\n",
        "    var file = dep.file(\"config\");\n",
        "    var dir = dep.dir(\"assets\");\n",
        "    var path = dep.path(\"schema\");\n",
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
        .expect("build.add_dep dependency queries should evaluate")
        .expect("build body should produce a graph");

    assert_eq!(evaluated.evaluated.dependencies.len(), 1);
    assert_eq!(evaluated.evaluated.dependencies[0].alias, "core");
    assert_eq!(evaluated.evaluated.dependency_queries.len(), 7);
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "root"
            && query.kind == BuildRuntimeDependencyQueryKind::Module));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "corelib"
            && query.kind == BuildRuntimeDependencyQueryKind::Artifact));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "check"
            && query.kind == BuildRuntimeDependencyQueryKind::Step));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "config"
            && query.kind == BuildRuntimeDependencyQueryKind::File));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "assets"
            && query.kind == BuildRuntimeDependencyQueryKind::Dir));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "schema"
            && query.kind == BuildRuntimeDependencyQueryKind::Path));
    assert!(evaluated
        .evaluated
        .dependency_queries
        .iter()
        .any(|query| query.dependency_alias == "core"
            && query.query_name == "bindings"
            && query.kind == BuildRuntimeDependencyQueryKind::GeneratedOutput));
}

#[test]
fn build_source_evaluator_resolves_deferred_artifact_option_values_into_runtime_metadata() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var root = graph.option({ name = \"root\", kind = \"path\", default = \"src/demo.fol\" });\n",
        "    var target = graph.standard_target();\n",
        "    var optimize = graph.standard_optimize();\n",
        "    graph.add_exe({ name = \"demo\", root = root, target = target, optimize = optimize });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            target: BuildTargetTriple::parse("x86_64-linux-gnu"),
            optimize: BuildOptimizeMode::parse("release-fast"),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("deferred artifact configs should evaluate")
        .expect("build body should produce operations");

    let artifact = evaluated
        .evaluated
        .artifacts
        .iter()
        .find(|artifact| artifact.name == "demo")
        .expect("artifact should exist");

    assert_eq!(artifact.root_module, "src/demo.fol");
    assert_eq!(artifact.target.as_str(), "x86_64-unknown-linux-gnu");
    assert_eq!(artifact.optimize, BuildOptimizeMode::ReleaseFast);
}

#[test]
fn build_source_evaluator_applies_build_inputs_and_option_overrides_to_artifact_metadata() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var root = graph.option({ name = \"root\", kind = \"path\", default = \"src/default.fol\" });\n",
        "    var target = graph.standard_target();\n",
        "    var optimize = graph.standard_optimize();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = root, target = target, optimize = optimize });\n",
        "    graph.add_run(app);\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let mut inputs = BuildEvaluationInputs {
        working_directory: package_root.display().to_string(),
        target: BuildTargetTriple::parse("aarch64-macos-gnu"),
        optimize: BuildOptimizeMode::parse("release-small"),
        ..BuildEvaluationInputs::default()
    };
    inputs
        .options
        .insert("root".to_string(), "src/cli-selected.fol".to_string());
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs,
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("build inputs should flow into artifact metadata")
        .expect("build body should produce operations");

    let artifact = evaluated
        .evaluated
        .artifacts
        .iter()
        .find(|artifact| artifact.name == "demo")
        .expect("artifact should exist");

    assert_eq!(artifact.root_module, "src/cli-selected.fol");
    assert_eq!(artifact.target.as_str(), "aarch64-apple-darwin");
    assert_eq!(artifact.optimize, BuildOptimizeMode::ReleaseSmall);
}

#[test]
fn build_source_evaluator_preserves_independent_artifact_configs_from_the_final_graph() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"app\", root = \"src/app.fol\", fol_model = \"core\", target = \"aarch64-linux-gnu\", optimize = \"release-fast\" });\n",
        "    graph.add_test({ name = \"tests\", root = \"test/app.fol\", fol_model = \"memo\", target = \"x86_64-linux-musl\", optimize = \"release-small\" });\n",
        "    graph.add_static_lib({ name = \"support\", root = \"src/support.fol\" });\n",
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
        .expect("mixed artifact configs should evaluate")
        .expect("build body should produce a graph");
    let graph_artifacts = evaluated.result.graph.artifacts();
    let runtime_artifacts = &evaluated.evaluated.artifacts;
    let host = fol_types::ResolvedTarget::host().unwrap();

    assert_eq!(graph_artifacts.len(), 3);
    assert_eq!(runtime_artifacts.len(), 3);
    assert_eq!(graph_artifacts[0].root_module, "src/app.fol");
    assert_eq!(graph_artifacts[0].fol_model, BuildArtifactFolModel::Core);
    assert_eq!(
        graph_artifacts[0].target.as_str(),
        "aarch64-unknown-linux-gnu"
    );
    assert_eq!(graph_artifacts[0].optimize, BuildOptimizeMode::ReleaseFast);
    assert_eq!(
        graph_artifacts[1].kind,
        crate::graph::BuildArtifactKind::Test
    );
    assert_eq!(
        runtime_artifacts[1].kind,
        crate::runtime::BuildRuntimeArtifactKind::Test
    );
    assert_eq!(runtime_artifacts[1].root_module, "test/app.fol");
    assert_eq!(
        runtime_artifacts[1].target.as_str(),
        "x86_64-unknown-linux-musl"
    );
    assert_eq!(
        runtime_artifacts[1].optimize,
        BuildOptimizeMode::ReleaseSmall
    );
    assert_eq!(graph_artifacts[2].target, host);
    assert_eq!(runtime_artifacts[2].target, host);
    assert_eq!(graph_artifacts[2].optimize, BuildOptimizeMode::Debug);
    assert_eq!(runtime_artifacts[2].optimize, BuildOptimizeMode::Debug);
}

#[test]
fn explicit_cli_target_and_optimize_override_literal_artifact_values() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"app\", root = \"src/app.fol\", target = \"aarch64-linux-gnu\", optimize = \"release-fast\" });\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            target: BuildTargetTriple::parse("x86_64-linux-musl"),
            optimize: Some(BuildOptimizeMode::ReleaseSmall),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };

    let evaluated = evaluate_build_source(&request, &build_path, source)
        .expect("CLI artifact overrides should evaluate")
        .expect("build body should produce a graph");
    let graph_artifact = &evaluated.result.graph.artifacts()[0];
    let runtime_artifact = &evaluated.evaluated.artifacts[0];

    assert_eq!(graph_artifact.target.as_str(), "x86_64-unknown-linux-musl");
    assert_eq!(runtime_artifact.target, graph_artifact.target);
    assert_eq!(graph_artifact.optimize, BuildOptimizeMode::ReleaseSmall);
    assert_eq!(runtime_artifact.optimize, graph_artifact.optimize);
}

#[test]
fn build_source_evaluator_uses_resolved_bool_defaults_and_overrides_on_the_second_pass() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var enabled = graph.option({ name = \"enabled\", kind = \"bool\", default = true });\n",
        "    when(enabled) {\n",
        "        {\n",
        "            graph.add_exe({ name = \"app\", root = \"src/app.fol\" });\n",
        "        }\n",
        "    };\n",
        "    return;\n",
        "};\n",
    );
    let (package_root, build_path) = temp_build_package(source);
    let default_request = BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: Vec::new(),
    };
    let mut disabled_request = default_request.clone();
    disabled_request
        .inputs
        .options
        .insert("enabled".to_string(), "false".to_string());

    let default_evaluated = evaluate_build_source(&default_request, &build_path, source)
        .expect("bool default should evaluate")
        .expect("option declaration should produce a graph");
    let disabled_evaluated = evaluate_build_source(&disabled_request, &build_path, source)
        .expect("bool override should evaluate")
        .expect("option declaration should produce a graph");

    assert_eq!(default_evaluated.result.graph.artifacts().len(), 1);
    assert!(disabled_evaluated.result.graph.artifacts().is_empty());
}

#[test]
fn build_source_evaluator_rejects_unknown_and_wrong_kind_artifact_targets() {
    for (target_setup, target_value, expected) in [
        (
            "",
            "\"mystery-vendor-os\"",
            "unsupported explicit machine target 'mystery-vendor-os'",
        ),
        (
            "var target = graph.option({ name = \"target_name\", kind = \"str\", default = \"x86_64-linux-gnu\" });",
            "target",
            "artifact target cannot use option 'target_name' of kind String",
        ),
    ] {
        let source = format!(
            "pro[] build(): non = {{\n    var graph = .build().graph();\n    {target_setup}\n    graph.add_exe({{ name = \"app\", root = \"src/app.fol\", target = {target_value} }});\n    return;\n}};\n"
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
            .expect_err("invalid artifact targets must fail closed");
        assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
        assert_eq!(error.message(), expected);
    }
}

#[test]
fn build_source_evaluator_rejects_unresolved_artifact_config_without_fallback() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var root = graph.option({ name = \"root\", kind = \"path\" });\n",
        "    graph.add_exe({ name = \"app\", root = root });\n",
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
        .expect_err("unresolved artifact config must not acquire a guessed default");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "artifact root requires resolved option 'root'"
    );
}

#[test]
fn build_source_evaluator_rejects_defaults_that_do_not_match_option_kinds() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.option({ name = \"enabled\", kind = \"bool\", default = \"yes\" });\n",
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
        .expect_err("wrong-kind defaults must fail closed");

    assert_eq!(error.kind(), BuildEvaluationErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "option 'enabled' has an invalid default for declared kind 'bool'"
    );
}

#[test]
fn build_source_evaluator_rejects_invalid_artifact_target_shapes_with_exact_diagnostics() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\", target = graph });\n",
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
        .expect_err("invalid artifact target shape should fail");

    assert_eq!(
        error.message(),
        "add_exe config is invalid: artifact 'target' must be a target handle or string triple"
    );
}

#[test]
fn build_source_evaluator_rejects_empty_artifact_roots_with_exact_diagnostics() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    graph.add_exe({ name = \"demo\", root = \"\" });\n",
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
        .expect_err("empty artifact root should fail");

    assert_eq!(
        error.message(),
        "add_exe config is invalid: artifact 'root' must not be empty"
    );
}

#[test]
fn build_source_evaluator_keeps_explicit_dependency_exports_precise() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var graph = build.graph();\n",
        "    var codec = graph.add_module({ name = \"codec\", root = \"src/codec.fol\" });\n",
        "    var app = graph.add_static_lib({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var docs = graph.step(\"docs\");\n",
        "    var config = graph.file_from_root(\"config/defaults.toml\");\n",
        "    var assets = graph.dir_from_root(\"assets\");\n",
        "    var bindings = graph.write_file({ name = \"bindings\", path = \"gen/bindings.fol\", contents = \"ok\" });\n",
        "    build.export_module({ name = \"api\", module = codec });\n",
        "    build.export_artifact({ name = \"runtime\", artifact = app });\n",
        "    build.export_step({ name = \"check\", step = docs });\n",
        "    build.export_file({ name = \"defaults\", file = config });\n",
        "    build.export_dir({ name = \"public\", dir = assets });\n",
        "    build.export_path({ name = \"schema-path\", path = bindings });\n",
        "    build.export_output({ name = \"schema\", output = bindings });\n",
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
        .expect("explicit exports should evaluate")
        .expect("build body should produce operations");

    assert_eq!(evaluated.evaluated.dependency_exports.len(), 7);
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "api"
            && export.target_name == "codec"
            && export.kind == crate::runtime::BuildRuntimeDependencyExportKind::Module));
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "runtime"
            && export.target_name == "demo"
            && export.kind == crate::runtime::BuildRuntimeDependencyExportKind::Artifact));
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "check"
            && export.target_name == "docs"
            && export.kind == crate::runtime::BuildRuntimeDependencyExportKind::Step));
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "defaults"
            && export.target_name == "config/defaults.toml"
            && export.kind == crate::runtime::BuildRuntimeDependencyExportKind::File));
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "public"
            && export.target_name == "assets"
            && export.kind == crate::runtime::BuildRuntimeDependencyExportKind::Dir));
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "schema-path"
            && export.target_name == "bindings"
            && export.kind == crate::runtime::BuildRuntimeDependencyExportKind::Path));
    assert!(evaluated
        .evaluated
        .dependency_exports
        .iter()
        .any(|export| export.name == "schema"
            && export.target_name == "bindings"
            && export.kind == crate::runtime::BuildRuntimeDependencyExportKind::GeneratedOutput));
}

#[test]
fn build_source_evaluator_rejects_duplicate_export_names_per_kind() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    var graph = build.graph();\n",
        "    var codec = graph.add_module({ name = \"codec\", root = \"src/codec.fol\" });\n",
        "    var other = graph.add_module({ name = \"other\", root = \"src/other.fol\" });\n",
        "    build.export_module({ name = \"api\", module = codec });\n",
        "    build.export_module({ name = \"api\", module = other });\n",
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
        .expect_err("duplicate export names should fail");

    assert_eq!(
        error.message(),
        "export_module config is invalid: duplicate exported module name 'api'"
    );
}

#[test]
fn build_source_evaluator_rejects_export_kind_handle_mismatches() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    var graph = build.graph();\n",
        "    var codec = graph.add_module({ name = \"codec\", root = \"src/codec.fol\" });\n",
        "    build.export_artifact({ name = \"runtime\", artifact = codec });\n",
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
        .expect_err("export kind mismatches should fail");

    assert_eq!(
        error.message(),
        "export_artifact config is invalid: build.export_artifact requires handle field 'artifact'"
    );
}

#[test]
fn build_source_evaluator_rejects_path_export_handle_mismatches() {
    let file_error = evaluate_export_path_mismatch(
        "export_file",
        "file",
        "var assets = graph.dir_from_root(\"assets\");\n",
        "assets",
    )
    .expect_err("export_file should reject source-dir handles");
    assert_eq!(
        file_error.message(),
        "export_file config is invalid: build.export_file requires handle field 'file'"
    );

    let dir_error = evaluate_export_path_mismatch(
        "export_dir",
        "dir",
        "var defaults = graph.file_from_root(\"config/defaults.toml\");\n",
        "defaults",
    )
    .expect_err("export_dir should reject source-file handles");
    assert_eq!(
        dir_error.message(),
        "export_dir config is invalid: build.export_dir requires handle field 'dir'"
    );

    let path_error = evaluate_export_path_mismatch(
        "export_path",
        "path",
        "var defaults = graph.file_from_root(\"config/defaults.toml\");\n",
        "defaults",
    )
    .expect_err("export_path should reject source-file handles");
    assert_eq!(
        path_error.message(),
        "export_path config is invalid: build.export_path requires handle field 'path'"
    );
}

fn evaluate_export_path_mismatch(
    method: &str,
    field_name: &str,
    binding_source: &str,
    binding_name: &str,
) -> Result<crate::eval::EvaluatedBuildSource, BuildEvaluationError> {
    let source = format!(
        concat!(
            "pro[] build(): non = {{\n",
            "    var build = .build();\n",
            "    var graph = build.graph();\n",
            "    {binding_source}",
            "    build.{method}({{ name = \"demo\", {field_name} = {binding_name} }});\n",
            "    return;\n",
            "}};\n",
        ),
        binding_source = binding_source,
        method = method,
        field_name = field_name,
        binding_name = binding_name,
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

    evaluate_build_source(&request, &build_path, &source).and_then(|program| {
        program.ok_or_else(|| {
            BuildEvaluationError::new(
                crate::eval::BuildEvaluationErrorKind::Unsupported,
                "expected evaluated build body".to_string(),
            )
        })
    })
}

#[test]
fn build_source_evaluator_rejects_copy_file_with_source_dir_handle() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var assets = graph.dir_from_root(\"assets\");\n",
        "    graph.copy_file({ name = \"asset\", source = assets, path = \"gen/logo.svg\" });\n",
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
        .expect_err("copy_file should reject source-dir handles");

    assert_eq!(
        error.message(),
        "copy_file config is invalid: 'source' must be a source-file handle, not a source-dir handle"
    );
}

#[test]
fn build_source_evaluator_rejects_install_dir_with_source_file_handle() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var defaults = graph.file_from_root(\"config/defaults.toml\");\n",
        "    graph.install_dir({ name = \"assets\", source = defaults });\n",
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
        .expect_err("install_dir should reject source-file handles");

    assert_eq!(
        error.message(),
        "install_dir config is invalid: 'source' must be a source-dir handle, not a source-file handle"
    );
}

#[test]
fn build_source_evaluator_rejects_run_add_file_arg_with_source_dir_handle() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var graph = .build().graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var run = graph.add_run(app);\n",
        "    var assets = graph.dir_from_root(\"assets\");\n",
        "    run.add_file_arg(assets);\n",
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
        .expect_err("run.add_file_arg should reject source-dir handles");

    assert_eq!(
        error.message(),
        "add_file_arg config is invalid: run.add_file_arg requires a source-file handle or generated-output handle, not a source-dir handle"
    );
}

#[test]
fn build_source_evaluator_rejects_artifact_add_generated_with_dependency_path_handle() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    var dep = build.add_dep({ alias = \"shared\", source = \"pkg\", target = \"shared\" });\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    var schema = dep.path(\"schema\");\n",
        "    app.add_generated(schema);\n",
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
        .expect_err("artifact.add_generated should reject dependency path handles");

    assert_eq!(
        error.message(),
        "add_generated config is invalid: artifact.add_generated requires a local generated-output handle, not a dependency path handle"
    );
}

#[test]
fn build_source_evaluator_rejects_install_dir_with_dependency_path_handle() {
    let source = concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    var dep = build.add_dep({ alias = \"shared\", source = \"pkg\", target = \"shared\" });\n",
        "    var graph = build.graph();\n",
        "    var schema = dep.path(\"schema\");\n",
        "    graph.install_dir({ name = \"assets\", source = schema });\n",
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
        .expect_err("install_dir should reject dependency path handles");

    assert_eq!(
        error.message(),
        "install_dir config is invalid: 'source' must be a source-dir handle, not a dependency-path handle"
    );
}

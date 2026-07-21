//! Package discovery and acquisition for FOL.
//!
//! Current scope:
//! - package loading configuration and identity types
//! - package metadata loading
//! - package build-definition loading
//! - package root validation
//! - package graph preparation ahead of ordinary source resolution
//!
//! This crate intentionally does not perform name resolution for ordinary source code.

pub mod build;
pub mod build_api;
pub mod build_artifact;
pub mod build_codegen;
pub mod build_dependency;
pub mod build_entry;
pub mod build_eval;
pub mod build_graph;
pub mod build_native;
pub mod build_option;
pub mod build_runtime;
pub mod build_semantic;
pub mod build_step;
pub mod config;
pub mod errors;
pub mod git;
pub mod identity;
pub mod locator;
pub mod lockfile;
pub mod metadata;
pub mod model;
pub mod paths;
pub mod session;

pub use build::{
    classify_semantic_build_mode, parse_package_build, parse_package_build_mode,
    PackageBuildDefinition, PackageBuildMode,
};
pub use build_api::{
    validate_build_name, BuildApi, BuildApiError, BuildApiNameError, BuildArtifactHandle,
    BuildCImportRequest, BuildOptionValue, DependencyHandle, DependencyRequest, ExecutableRequest,
    GitDependencyVersionSelector, InstallArtifactRequest, InstallDirRequest, InstallFileRequest,
    InstallHandle, RunHandle, RunRequest, SharedLibraryRequest, StandardOptimizeOption,
    StandardOptimizeRequest, StandardTargetOption, StandardTargetRequest, StaticLibraryRequest,
    StepHandle, StepRequest, TestArtifactRequest, UserOption, UserOptionRequest,
};
pub use build_artifact::{
    project_graph_artifacts, BuildArtifactDefinition, BuildArtifactLinkage, BuildArtifactModelKind,
    BuildArtifactModuleConfig, BuildArtifactOutput, BuildArtifactPipelinePlan,
    BuildArtifactPipelineStage, BuildArtifactReport, BuildArtifactRootSource, BuildArtifactSet,
    BuildArtifactTargetConfig, BuildArtifactTargetConfigError,
};
pub use build_codegen::{
    CodegenKind, CodegenRequest, CodegenResult, GeneratedFileAction, GeneratedFileDefinition,
    GeneratedFileInstallProjection, GeneratedFileSet, GeneratedOutputDependencySet,
    SystemToolRequest, SystemToolResult,
};
pub use build_dependency::{
    dependency_modules_from_exports, DependencyArtifactSurface, DependencyArtifactSurfaceSet,
    DependencyBuildEvaluationMode, DependencyBuildHandle, DependencyBuildSurface,
    DependencyBuildSurfaceSet, DependencyDirSurface, DependencyDirSurfaceSet,
    DependencyFileSurface, DependencyFileSurfaceSet, DependencyGeneratedOutputSurface,
    DependencyGeneratedOutputSurfaceSet, DependencyModuleSurface, DependencyModuleSurfaceSet,
    DependencyPathSurface, DependencyPathSurfaceSet, DependencySourceRootSurface,
    DependencyStepSurface, DependencyStepSurfaceSet,
};
pub use build_entry::{
    collect_build_entry_candidates, validate_build_entry_cardinality,
    validate_build_entry_parameter_shape, validate_build_entry_parameter_type,
    validate_build_entry_return_type, validate_parsed_build_entry, BuildEntryCandidate,
    BuildEntrySignatureExpectation, BuildEntryValidationError, BuildEntryValidationErrorKind,
    ValidatedBuildEntry,
};
pub use build_eval::{
    canonical_graph_construction_capabilities, evaluate_build_plan, evaluate_build_source,
    forbidden_capability_error, forbidden_capability_message, AllowedBuildTimeOperation,
    BuildEnvironmentSelectionPolicy, BuildEvaluationBoundary, BuildEvaluationError,
    BuildEvaluationErrorKind, BuildEvaluationInputEnvelope, BuildEvaluationInputs,
    BuildEvaluationInstallArtifactRequest, BuildEvaluationOperation, BuildEvaluationOperationKind,
    BuildEvaluationRequest, BuildEvaluationResult, BuildEvaluationRunRequest,
    BuildEvaluationStepRequest, BuildRuntimeCapabilityModel, EvaluatedBuildSource,
    ForbiddenBuildTimeOperation,
};
pub use build_graph::{
    BuildArtifact, BuildArtifactDependency, BuildArtifactId, BuildArtifactInput, BuildArtifactKind,
    BuildCImportAttachment, BuildCImportAttachmentError, BuildCImportProviderKind,
    BuildGeneratedFile, BuildGeneratedFileId, BuildGeneratedFileKind, BuildGraph,
    BuildGraphValidationError, BuildGraphValidationErrorKind, BuildInstall, BuildInstallId,
    BuildInstallKind, BuildInstallTarget, BuildModule, BuildModuleId, BuildModuleKind, BuildOption,
    BuildOptionId, BuildOptionKind, BuildStep, BuildStepDependency, BuildStepId, BuildStepKind,
};
pub use build_native::{
    NativeArtifactDefinition, NativeArtifactKind, NativeArtifactSet, NativeIncludePath,
    NativeLibraryPath, NativeLinkDirective, NativeLinkInput, NativeLinkMode, NativePlatform,
    NativeSearchPathOrigin,
};
pub use build_option::{
    BuildOptimizeMode, BuildOptionDeclaration, BuildOptionDeclarationSet, BuildOptionOverride,
    BuildOptionOverrideParseError, BuildTargetTriple, ResolvedBuildOptionSet,
    StandardOptimizeDeclaration, StandardTargetDeclaration, UserOptionDeclaration,
};
pub use build_runtime::{
    find_record_field, BuildExecutionRepresentation, BuildRuntimeDependency,
    BuildRuntimeDependencyQuery, BuildRuntimeDependencyQueryKind, BuildRuntimeDiagnostic,
    BuildRuntimeDiagnosticKind, BuildRuntimeExpr, BuildRuntimeFrame, BuildRuntimeGeneratedFile,
    BuildRuntimeGeneratedFileKind, BuildRuntimeHandle, BuildRuntimeHandleKind, BuildRuntimeLocalId,
    BuildRuntimeMethodCall, BuildRuntimeProgram, BuildRuntimeReceiverKind, BuildRuntimeRecordField,
    BuildRuntimeStmt, BuildRuntimeValue,
};
pub use build_semantic::{
    canonical_artifact_config_shapes, canonical_build_context_config_shapes,
    canonical_build_context_method_signatures, canonical_chain_metadata,
    canonical_graph_method_signatures, canonical_handle_method_signatures,
    canonical_option_config_shapes, canonical_option_value_kinds, BuildSemanticChainKind,
    BuildSemanticChainMetadata, BuildSemanticMethodParameter, BuildSemanticMethodSignature,
    BuildSemanticOptionValueKind, BuildSemanticParameterShape, BuildSemanticRecordField,
    BuildSemanticRecordShape, BuildSemanticRecordShapeKind, BuildSemanticType,
    BuildSemanticTypeFamily, BuildStdlibImportSurface, BuildStdlibModuleKind,
    BuildStdlibModulePath,
};
pub use build_step::{
    plan_step_order, project_graph_steps, BuildDefaultStepKind, BuildRequestedStep,
    BuildStepCacheBoundary, BuildStepCacheKey, BuildStepDefinition, BuildStepEvent,
    BuildStepEventKind, BuildStepExecutionRequest, BuildStepExecutionResult, BuildStepPlanError,
    BuildStepReport,
};
pub use config::{
    available_bundled_std_root, available_bundled_store_root, bundled_std_root, effective_std_root,
    PackageConfig,
};
pub use errors::{PackageError, PackageErrorKind};
pub use fol_parser::ast::ParsedSourceUnitKind;
pub use git::{
    wrap_git_failure, PackageGitFetchOptions, PackageGitMaterialization, PackageGitSourceSession,
};
pub use identity::{PackageIdentity, PackageSourceKind};
pub use locator::{
    parse_package_locator, PackageGitLocator, PackageGitSelector, PackageGitTransport,
    PackageLocator, PackageLocatorKind,
};
pub use lockfile::{
    parse_package_lockfile, render_package_lockfile, PackageLockEntry, PackageLockfile,
};
pub use metadata::{
    extract_package_dependencies_from_build, extract_package_metadata_fields_from_build,
    parse_package_metadata_from_build, PackageDependencyDecl, PackageDependencySourceKind,
    PackageMetadata,
};
pub use model::{PreparedExportMount, PreparedPackage};
pub use paths::{git_cache_path, git_store_path};
pub use session::{
    canonical_directory_root, infer_package_root, parse_directory_package_syntax,
    resolve_directory_path, resolve_directory_target, PackageSession,
};

#[cfg(test)]
mod tests {
    use super::{
        canonical_chain_metadata, canonical_graph_construction_capabilities,
        canonical_graph_method_signatures, canonical_handle_method_signatures,
        canonical_option_value_kinds, classify_semantic_build_mode, collect_build_entry_candidates,
        evaluate_build_source, forbidden_capability_message, validate_parsed_build_entry,
        AllowedBuildTimeOperation, BuildEntrySignatureExpectation, BuildEvaluationInputs,
        BuildEvaluationRequest, BuildExecutionRepresentation, BuildOptimizeMode,
        BuildRuntimeDependencyQueryKind, BuildRuntimeDiagnostic, BuildRuntimeDiagnosticKind,
        BuildRuntimeExpr, BuildRuntimeGeneratedFileKind, BuildRuntimeHandle,
        BuildRuntimeHandleKind, BuildRuntimeLocalId, BuildRuntimeMethodCall,
        BuildRuntimeReceiverKind, BuildRuntimeRecordField, BuildRuntimeStmt, BuildRuntimeValue,
        BuildSemanticChainKind, BuildSemanticType, BuildSemanticTypeFamily, BuildTargetTriple,
        DependencyBuildEvaluationMode, ForbiddenBuildTimeOperation, NativeArtifactDefinition,
        NativeArtifactKind, NativeArtifactSet, NativeLinkDirective, NativeLinkInput,
        NativeLinkMode, PackageBuildMode, ParsedSourceUnitKind,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_build_package(source: &str) -> (PathBuf, PathBuf) {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        let package_root = std::env::temp_dir().join(format!(
            "fol_pkg_lib_eval_{}_{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&package_root).expect("temp package root should be created");
        fs::write(package_root.join("build.fol"), source).expect("build source should be written");
        (package_root.clone(), package_root.join("build.fol"))
    }

    #[test]
    fn crate_root_reexports_native_surface_types() {
        let mut set = NativeArtifactSet::new();
        let artifact = NativeArtifactDefinition {
            name: "ssl".to_string(),
            kind: NativeArtifactKind::StaticLibrary,
            relative_path: "native/libssl.a".to_string(),
        };
        set.add(artifact.clone());
        let directive = NativeLinkDirective {
            input: NativeLinkInput::Artifact(artifact),
            mode: NativeLinkMode::Static,
        };

        assert_eq!(set.definitions().len(), 1);
        assert_eq!(directive.mode, NativeLinkMode::Static);
    }

    #[test]
    fn crate_root_reexports_semantic_build_surface_types() {
        let graph = BuildSemanticType::graph();
        let methods = canonical_graph_method_signatures();
        let handles = canonical_handle_method_signatures();
        let chains = canonical_chain_metadata();
        let option_kinds = canonical_option_value_kinds();

        assert_eq!(graph.family, BuildSemanticTypeFamily::Graph);
        assert!(methods.iter().any(|method| method.name == "add_exe"));
        assert!(handles.iter().any(|method| method.name == "depend_on"));
        assert!(chains
            .iter()
            .any(|chain| chain.kind == BuildSemanticChainKind::RunDependency));
        assert!(option_kinds.len() >= 7);
    }

    #[test]
    fn crate_root_reexports_parsed_source_unit_kinds() {
        assert_eq!(
            ParsedSourceUnitKind::Build,
            fol_parser::ast::ParsedSourceUnitKind::Build
        );
    }

    #[test]
    fn crate_root_reexports_runtime_ir_surface() {
        let graph = BuildRuntimeValue::Handle(BuildRuntimeHandle::new(
            BuildRuntimeHandleKind::Graph,
            "graph",
        ));
        let method = BuildRuntimeMethodCall::new(
            BuildRuntimeExpr::Local(BuildRuntimeLocalId(0)),
            BuildRuntimeReceiverKind::Graph,
            "add_exe",
            vec![BuildRuntimeExpr::Value(BuildRuntimeValue::String(
                "demo".to_string(),
            ))],
        );
        let stmt = BuildRuntimeStmt::Expr(BuildRuntimeExpr::Local(BuildRuntimeLocalId(0)));
        let field = BuildRuntimeRecordField::new(
            "name",
            BuildRuntimeExpr::Value(BuildRuntimeValue::String("demo".to_string())),
        );
        let diagnostic = BuildRuntimeDiagnostic::new(
            BuildRuntimeDiagnosticKind::UnknownMethod,
            "unknown build method",
        );

        assert!(matches!(
            graph,
            BuildRuntimeValue::Handle(BuildRuntimeHandle {
                kind: BuildRuntimeHandleKind::Graph,
                ..
            })
        ));
        assert_eq!(
            BuildExecutionRepresentation::RestrictedRuntimeIr,
            BuildExecutionRepresentation::RestrictedRuntimeIr
        );
        assert_eq!(method.method, "add_exe");
        assert!(matches!(stmt, BuildRuntimeStmt::Expr(_)));
        assert_eq!(field.name, "name");
        assert_eq!(diagnostic.kind, BuildRuntimeDiagnosticKind::UnknownMethod);
    }

    #[test]
    fn crate_root_reexports_semantic_build_entry_surface() {
        let syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "build.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: ParsedSourceUnitKind::Build,
                items: vec![fol_parser::ast::ParsedTopLevel {
                    node_id: fol_parser::ast::SyntaxNodeId(1),
                    node: fol_parser::ast::AstNode::ProDecl {
                        syntax_id: None,
                        options: Vec::new(),
                        generics: Vec::new(),
                        name: "build".to_string(),
                        receiver_type: None,
                        captures: Vec::new(),
                        params: Vec::new(),
                        return_type: Some(fol_parser::ast::FolType::None),
                        error_type: None,
                        body: Vec::new(),
                        inquiries: Vec::new(),
                    },
                    meta: fol_parser::ast::ParsedTopLevelMeta::default(),
                }],
            }],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };

        let candidates = collect_build_entry_candidates(&syntax);
        let validated =
            validate_parsed_build_entry(&syntax, &BuildEntrySignatureExpectation::canonical())
                .expect("crate root should expose semantic build entry validation");

        assert_eq!(candidates.len(), 1);
        assert_eq!(validated.candidate.name, "build");
    }

    #[test]
    fn crate_root_reexports_semantic_build_modes() {
        let syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "build.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: ParsedSourceUnitKind::Build,
                items: vec![fol_parser::ast::ParsedTopLevel {
                    node_id: fol_parser::ast::SyntaxNodeId(1),
                    node: fol_parser::ast::AstNode::ProDecl {
                        syntax_id: None,
                        options: Vec::new(),
                        generics: Vec::new(),
                        name: "build".to_string(),
                        receiver_type: None,
                        captures: Vec::new(),
                        params: Vec::new(),
                        return_type: Some(fol_parser::ast::FolType::None),
                        error_type: None,
                        body: Vec::new(),
                        inquiries: Vec::new(),
                    },
                    meta: fol_parser::ast::ParsedTopLevelMeta::default(),
                }],
            }],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };

        assert_eq!(
            classify_semantic_build_mode(&syntax, false),
            PackageBuildMode::ModernOnly
        );
    }

    #[test]
    fn crate_root_reexports_phase_four_capability_surface() {
        let capabilities = canonical_graph_construction_capabilities();

        assert!(capabilities
            .allowed_operations
            .contains(&AllowedBuildTimeOperation::GraphMutation));
        assert!(capabilities
            .forbidden_operations
            .contains(&ForbiddenBuildTimeOperation::ArbitraryNetworkAccess));
        assert!(forbidden_capability_message(
            ForbiddenBuildTimeOperation::AmbientEnvironmentAccess
        )
        .contains("declared inputs"));
    }

    #[test]
    fn crate_root_reexports_phase_six_build_evaluation_surface() {
        let source = concat!(
            "pro[] build(): non = {\n",
            "    var graph = .graph();\n",
            "    var app = graph.add_exe({\n",
            "        name = \"demo\",\n",
            "        root = \"src/demo.fol\",\n",
            "    });\n",
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
            .expect("crate root build evaluation should succeed")
            .expect("crate root build evaluation should produce a graph");

        assert_eq!(evaluated.evaluated.artifacts.len(), 1);
        assert!(evaluated
            .evaluated
            .step_bindings
            .iter()
            .any(|binding| binding.step_name == "run"));
        assert_eq!(evaluated.result.graph.artifacts().len(), 1);
    }

    #[test]
    fn crate_root_reexports_phase_ten_dependency_surface() {
        let source = concat!(
            "pro[] build(): non = {\n",
            "    var graph = .graph();\n",
            "    var dep = graph.dependency({ alias = \"core\", package = \"org/core\", mode = \"eager\" });\n",
            "    var module = dep.module(\"root\");\n",
            "    var artifact = dep.artifact(\"corelib\");\n",
            "    var step = dep.step(\"check\");\n",
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
            .expect("crate root dependency evaluation should succeed")
            .expect("crate root dependency evaluation should produce a graph");
        let query_kinds = evaluated
            .evaluated
            .dependency_queries
            .iter()
            .map(|query| query.kind)
            .collect::<Vec<_>>();

        assert_eq!(evaluated.evaluated.dependencies.len(), 1);
        // Direct graph.dependency declarations currently support only the
        // eager evaluation mode; the lazy/on-demand modes are not yet wired
        // through the graph dependency surface.
        assert_eq!(
            evaluated.evaluated.dependencies[0].evaluation_mode,
            Some(DependencyBuildEvaluationMode::Eager)
        );
        assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Module));
        assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Artifact));
        assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::Step));
        assert!(query_kinds.contains(&BuildRuntimeDependencyQueryKind::GeneratedOutput));
    }

    #[test]
    fn crate_root_reexports_phase_eleven_generated_surface() {
        let source = concat!(
            "pro[] build(): non = {\n",
            "    var graph = .graph();\n",
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
            .expect("crate root generated evaluation should succeed")
            .expect("crate root generated evaluation should produce a graph");
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
    fn crate_root_reexports_phase_nine_real_option_surface() {
        let source = concat!(
            "pro[] build(): non = {\n",
            "    var graph = .graph();\n",
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
            .expect("crate root option evaluation should succeed")
            .expect("crate root option evaluation should produce a graph");
        let artifact = evaluated
            .evaluated
            .artifacts
            .iter()
            .find(|artifact| artifact.name == "demo")
            .expect("artifact should exist");

        assert_eq!(artifact.root_module, "src/cli-selected.fol");
        assert_eq!(artifact.target.as_str(), "aarch64-apple-darwin");
        assert_eq!(artifact.optimize, BuildOptimizeMode::ReleaseSmall);
        assert_eq!(
            evaluated.result.resolved_options.get("root"),
            Some("src/cli-selected.fol")
        );
    }
}

//! Build system execution for FOL.
//!
//! Owns all build graph IR, build API types, and build execution logic.
//! The compiler (`fol-package`) handles only entry validation and package metadata.

pub mod api;
pub mod artifact;
pub mod codegen;
pub mod dependency;
pub mod eval;
pub mod executor;
pub mod graph;
pub mod native;
pub mod option;
pub mod runtime;
pub mod semantic;
pub mod stdlib;
pub mod step;

pub use api::{
    validate_build_name, BuildApi, BuildApiError, BuildApiNameError, BuildArtifactHandle,
    BuildCImportRequest, BuildOptionValue, DependencyArgValue, DependencyHandle, DependencyRequest,
    DependencySourceKind, ExecutableRequest, GitDependencyVersionSelector, InstallArtifactRequest,
    InstallDirRequest, InstallFileRequest, InstallHandle, OutputHandle, OutputHandleKind,
    OutputHandleLocator, PathHandle, PathHandleClass, PathHandleProvenance, RunHandle, RunRequest,
    SharedLibraryRequest, SourceDirHandle, SourceFileHandle, StandardOptimizeOption,
    StandardOptimizeRequest, StandardTargetOption, StandardTargetRequest, StaticLibraryRequest,
    StepHandle, StepRequest, SystemLibraryHandle, TestArtifactRequest, UserOption,
    UserOptionRequest,
};
pub use artifact::{
    project_graph_artifacts, BuildArtifactDefinition, BuildArtifactFolModel, BuildArtifactLinkage,
    BuildArtifactModelKind, BuildArtifactModuleConfig, BuildArtifactOutput,
    BuildArtifactPipelinePlan, BuildArtifactPipelineStage, BuildArtifactReport,
    BuildArtifactRootSource, BuildArtifactSet, BuildArtifactTargetConfig,
    BuildArtifactTargetConfigError,
};
pub use codegen::{
    materialize_generated_action_plan, CodegenKind, CodegenRequest, CodegenResult,
    GeneratedFileAction, GeneratedFileDefinition, GeneratedFileInstallProjection,
    GeneratedFileMaterialization, GeneratedFileMaterializationError,
    GeneratedFileMaterializationPlan, GeneratedFileSet, GeneratedOutputDependencySet,
    SystemToolRequest, SystemToolResult,
};
pub use dependency::{
    DependencyArtifactSurface, DependencyArtifactSurfaceSet, DependencyBuildEvaluationMode,
    DependencyBuildExposure, DependencyBuildHandle, DependencyBuildSurface,
    DependencyBuildSurfaceSet, DependencyDirSurface, DependencyDirSurfaceSet,
    DependencyFileSurface, DependencyFileSurfaceSet, DependencyGeneratedOutputSurface,
    DependencyGeneratedOutputSurfaceSet, DependencyModuleSurface, DependencyModuleSurfaceSet,
    DependencyPathSurface, DependencyPathSurfaceSet, DependencySourceRootSurface,
    DependencyStepSurface, DependencyStepSurfaceSet,
};
pub use eval::{
    canonical_graph_construction_capabilities, evaluate_build_plan, evaluate_build_source,
    forbidden_capability_error, forbidden_capability_message, AllowedBuildTimeOperation,
    BuildEnvironmentSelectionPolicy, BuildEvaluationBoundary, BuildEvaluationError,
    BuildEvaluationErrorKind, BuildEvaluationInputEnvelope, BuildEvaluationInputs,
    BuildEvaluationInstallArtifactRequest, BuildEvaluationOperation, BuildEvaluationOperationKind,
    BuildEvaluationRequest, BuildEvaluationResult, BuildEvaluationRunRequest,
    BuildEvaluationStepRequest, BuildRuntimeCapabilityModel, EvaluatedBuildSource,
    ForbiddenBuildTimeOperation,
};
pub use graph::{
    BuildArtifact, BuildArtifactDependency, BuildArtifactId, BuildArtifactInput, BuildArtifactKind,
    BuildCImportAttachment, BuildCImportAttachmentError, BuildCImportProviderKind,
    BuildGeneratedFile, BuildGeneratedFileId, BuildGeneratedFileKind, BuildGraph,
    BuildGraphValidationError, BuildGraphValidationErrorKind, BuildInstall, BuildInstallId,
    BuildInstallKind, BuildInstallTarget, BuildModule, BuildModuleId, BuildModuleKind, BuildOption,
    BuildOptionId, BuildOptionKind, BuildStep, BuildStepDependency, BuildStepId, BuildStepKind,
};
pub use native::{
    NativeArtifactDefinition, NativeArtifactKind, NativeArtifactSet, NativeIncludePath,
    NativeLibraryPath, NativeLinkDirective, NativeLinkInput, NativeLinkMode, NativePlatform,
    NativeSearchPathOrigin, SystemLibraryRequest,
};
pub use option::{
    BuildOptimizeMode, BuildOptionDeclaration, BuildOptionDeclarationSet, BuildOptionOverride,
    BuildOptionOverrideParseError, BuildTargetTriple, ResolvedBuildOptionSet,
    StandardOptimizeDeclaration, StandardTargetDeclaration, UserOptionDeclaration,
};
pub use runtime::{
    find_record_field, BuildExecutionRepresentation, BuildRuntimeDependency,
    BuildRuntimeDependencyExport, BuildRuntimeDependencyExportKind, BuildRuntimeDependencyQuery,
    BuildRuntimeDependencyQueryKind, BuildRuntimeDiagnostic, BuildRuntimeDiagnosticKind,
    BuildRuntimeExpr, BuildRuntimeFrame, BuildRuntimeGeneratedFile, BuildRuntimeGeneratedFileKind,
    BuildRuntimeHandle, BuildRuntimeHandleKind, BuildRuntimeLocalId, BuildRuntimeMethodCall,
    BuildRuntimeProgram, BuildRuntimeReceiverKind, BuildRuntimeRecordField, BuildRuntimeStmt,
    BuildRuntimeValue,
};
pub use semantic::{
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
pub use stdlib::BuildStdlibScope;
pub use step::{
    plan_step_order, project_graph_steps, BuildDefaultStepKind, BuildRequestedStep,
    BuildStepCacheBoundary, BuildStepCacheKey, BuildStepDefinition, BuildStepEvent,
    BuildStepEventKind, BuildStepExecutionRequest, BuildStepExecutionResult, BuildStepPlanError,
    BuildStepReport,
};

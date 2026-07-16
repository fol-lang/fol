use super::capabilities::{BuildEvaluationBoundary, BuildRuntimeCapabilityModel};
use crate::api::{
    CopyFileRequest, DependencyRequest, ExecutableRequest, InstallDirRequest, InstallFileRequest,
    SharedLibraryRequest, StandardOptimizeRequest, StandardTargetRequest, StaticLibraryRequest,
    TestArtifactRequest, UserOptionRequest, WriteFileRequest,
};
use crate::codegen::{CodegenRequest, SystemToolRequest};
use crate::graph::BuildGraph;
use crate::option::{
    BuildOptimizeMode, BuildOptionDeclarationSet, BuildTargetTriple, ResolvedBuildOptionSet,
};
use crate::runtime::{
    BuildRuntimeArtifact, BuildRuntimeDependency, BuildRuntimeDependencyExport,
    BuildRuntimeDependencyQuery, BuildRuntimeGeneratedFile, BuildRuntimeProgram,
    BuildRuntimeStepBinding,
};
use fol_parser::ast::SyntaxOrigin;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildEvaluationRequest {
    pub package_root: String,
    pub inputs: BuildEvaluationInputs,
    pub operations: Vec<BuildEvaluationOperation>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildEvaluationInputs {
    pub working_directory: String,
    pub install_prefix: String,
    pub target: Option<BuildTargetTriple>,
    pub optimize: Option<BuildOptimizeMode>,
    pub options: BTreeMap<String, String>,
    pub environment_policy: BuildEnvironmentSelectionPolicy,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildEvaluationInputEnvelope {
    pub working_directory: String,
    pub install_prefix: String,
    pub target: Option<BuildTargetTriple>,
    pub optimize: Option<BuildOptimizeMode>,
    pub options: BTreeMap<String, String>,
    pub declared_environment: Vec<String>,
    pub selected_environment: BTreeMap<String, String>,
}

impl BuildEvaluationInputEnvelope {
    pub fn determinism_key(&self) -> String {
        let options = self
            .options
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        let declared_environment = self.declared_environment.join(",");
        let target = self
            .target
            .as_ref()
            .map(BuildTargetTriple::render)
            .unwrap_or_default();
        let optimize = self
            .optimize
            .as_ref()
            .map(|mode| mode.as_str().to_string())
            .unwrap_or_default();
        let selected_environment = self
            .selected_environment
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "cwd={};prefix={};target={};optimize={};options=[{}];declared_env=[{}];env=[{}]",
            self.working_directory,
            self.install_prefix,
            target,
            optimize,
            options,
            declared_environment,
            selected_environment
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildEnvironmentSelectionPolicy {
    declared_names: Vec<String>,
}

impl BuildEnvironmentSelectionPolicy {
    pub fn new(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut declared_names = names.into_iter().map(Into::into).collect::<Vec<_>>();
        declared_names.sort();
        declared_names.dedup();
        Self { declared_names }
    }

    pub fn declared_names(&self) -> &[String] {
        &self.declared_names
    }

    pub fn allows(&self, name: &str) -> bool {
        self.declared_names.iter().any(|declared| declared == name)
    }

    pub fn select<'a>(
        &self,
        environment: impl IntoIterator<Item = (&'a String, &'a String)>,
    ) -> BTreeMap<String, String> {
        environment
            .into_iter()
            .filter(|(name, _)| self.allows(name))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect()
    }

    pub fn determinism_key(&self) -> String {
        self.declared_names.join(",")
    }
}

impl BuildEvaluationInputs {
    pub fn explicit_envelope(&self) -> BuildEvaluationInputEnvelope {
        BuildEvaluationInputEnvelope {
            working_directory: self.working_directory.clone(),
            install_prefix: self.install_prefix.clone(),
            target: self.target.clone(),
            optimize: self.optimize,
            options: self.options.clone(),
            declared_environment: self.environment_policy.declared_names().to_vec(),
            selected_environment: self.environment_policy.select(self.environment.iter()),
        }
    }

    pub fn determinism_key(&self) -> String {
        self.explicit_envelope().determinism_key()
    }
}

impl BuildEvaluationRequest {
    pub fn determinism_key(&self) -> String {
        format!(
            "root={};{};ops={}",
            self.package_root,
            self.inputs.determinism_key(),
            self.operations.len()
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvaluationOperation {
    pub origin: Option<SyntaxOrigin>,
    pub kind: BuildEvaluationOperationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildEvaluationOperationKind {
    StandardTarget(StandardTargetRequest),
    StandardOptimize(StandardOptimizeRequest),
    Option(UserOptionRequest),
    AddExe(ExecutableRequest),
    AddStaticLib(StaticLibraryRequest),
    AddSharedLib(SharedLibraryRequest),
    AddTest(TestArtifactRequest),
    AddModule(crate::api::AddModuleRequest),
    Step(BuildEvaluationStepRequest),
    AddRun(BuildEvaluationRunRequest),
    InstallArtifact(BuildEvaluationInstallArtifactRequest),
    InstallFile(InstallFileRequest),
    InstallGeneratedFile {
        name: String,
        generated_name: String,
    },
    InstallGeneratedDir {
        name: String,
        generated_name: String,
    },
    InstallDir(InstallDirRequest),
    WriteFile(WriteFileRequest),
    CopyFile(CopyFileRequest),
    SystemTool(SystemToolRequest),
    SystemToolDir(SystemToolRequest),
    Codegen(CodegenRequest),
    CodegenDir(CodegenRequest),
    Dependency(DependencyRequest),
    ArtifactLink {
        artifact: String,
        linked: String,
    },
    ArtifactLinkSystemLibrary {
        artifact: String,
        request: crate::native::SystemLibraryRequest,
    },
    ArtifactImport {
        artifact: String,
        module_name: String,
    },
    ArtifactAddGenerated {
        artifact: String,
        generated_name: String,
    },
    RunAddArg {
        run_name: String,
        kind: BuildEvaluationRunArgKind,
        value: String,
    },
    RunCapture {
        run_name: String,
        output_name: String,
    },
    RunSetEnv {
        run_name: String,
        key: String,
        value: String,
    },
    StepAttach {
        step_name: String,
        generated_name: String,
    },
    Unsupported {
        label: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildEvaluationRunArgKind {
    Literal,
    GeneratedFile,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvaluationStepRequest {
    pub name: String,
    pub description: Option<String>,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvaluationRunRequest {
    pub name: String,
    pub artifact: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvaluationInstallArtifactRequest {
    pub name: String,
    pub artifact: String,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvaluationResult {
    pub boundary: BuildEvaluationBoundary,
    pub capabilities: BuildRuntimeCapabilityModel,
    pub package_root: String,
    pub option_declarations: BuildOptionDeclarationSet,
    pub resolved_options: ResolvedBuildOptionSet,
    pub dependency_requests: Vec<DependencyRequest>,
    pub graph: BuildGraph,
}

impl BuildEvaluationResult {
    pub fn new(
        boundary: BuildEvaluationBoundary,
        capabilities: BuildRuntimeCapabilityModel,
        package_root: impl Into<String>,
        option_declarations: BuildOptionDeclarationSet,
        resolved_options: ResolvedBuildOptionSet,
        dependency_requests: Vec<DependencyRequest>,
        graph: BuildGraph,
    ) -> Self {
        Self {
            boundary,
            capabilities,
            package_root: package_root.into(),
            option_declarations,
            resolved_options,
            dependency_requests,
            graph,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBuildSource {
    pub evaluated: EvaluatedBuildProgram,
    pub result: BuildEvaluationResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBuildProgram {
    pub program: BuildRuntimeProgram,
    pub artifacts: Vec<BuildRuntimeArtifact>,
    pub generated_files: Vec<BuildRuntimeGeneratedFile>,
    pub dependencies: Vec<BuildRuntimeDependency>,
    pub dependency_exports: Vec<BuildRuntimeDependencyExport>,
    pub dependency_queries: Vec<BuildRuntimeDependencyQuery>,
    pub step_bindings: Vec<BuildRuntimeStepBinding>,
    pub result: BuildEvaluationResult,
}

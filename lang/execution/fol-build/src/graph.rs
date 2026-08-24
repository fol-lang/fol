const MAX_GRAPH_DEPTH: usize = 256;

#[macro_export]
macro_rules! define_graph_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub usize);

        impl $name {
            pub fn index(self) -> usize {
                self.0
            }

            pub fn from_index(index: usize) -> Self {
                Self(index)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}{}", $label, self.0)
            }
        }
    };
}

define_graph_id!(BuildStepId, "step:");
define_graph_id!(BuildArtifactId, "artifact:");
define_graph_id!(BuildModuleId, "module:");
define_graph_id!(BuildGeneratedFileId, "generated:");
define_graph_id!(BuildOptionId, "option:");
define_graph_id!(BuildInstallId, "install:");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStepKind {
    Default,
    Install,
    Run,
    Test,
    Check,
    CustomCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildArtifactKind {
    Executable,
    StaticLibrary,
    SharedLibrary,
    Test,
    Object,
}

/// How a C provider is linked. `import_library` and `framework` stay
/// unspelled until their lane is certified, per section 4.5.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuildCImportProviderKind {
    #[default]
    Object,
    Static,
    Shared,
}

impl BuildCImportProviderKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "object" => Some(Self::Object),
            "static" => Some(Self::Static),
            "shared" => Some(Self::Shared),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Static => "static",
            Self::Shared => "shared",
        }
    }

    /// The spellings this build accepts, for a diagnostic that lists them.
    pub const ACCEPTED: &'static [&'static str] = &["object", "static", "shared"];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildModuleKind {
    Source,
    Generated,
    Imported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildGeneratedFileKind {
    Write,
    Copy,
    CaptureOutput,
    GeneratedDir,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOptionKind {
    Target,
    Optimize,
    Bool,
    Int,
    String,
    Enum,
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildInstallKind {
    Artifact,
    File,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStep {
    pub id: BuildStepId,
    pub kind: BuildStepKind,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifact {
    pub id: BuildArtifactId,
    pub kind: BuildArtifactKind,
    pub name: String,
    pub root_module: String,
    pub fol_model: crate::artifact::BuildArtifactFolModel,
    pub target: fol_types::ResolvedTarget,
    pub optimize: crate::option::BuildOptimizeMode,
    pub library_paths: Vec<NativeLibraryPath>,
    pub link_inputs: Vec<NativeLinkDirective>,
    /// The declared ABI major/minor, when the artifact declares one.
    pub abi_version: Option<(u32, u32)>,
    /// The export allowlist, in declaration order.
    pub abi_exports: Vec<BuildAbiExport>,
}

/// One checked C import attached to one artifact.
///
/// Every field the section 4.13 pipeline needs lives here, because the
/// attachment is the single object that binds the synthesized namespace to its
/// provider plan. Splitting the target or the include roots into unrelated
/// options would let an artifact link one provider while using an interface
/// derived from another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildCImportAttachment {
    pub artifact_id: BuildArtifactId,
    /// The FOL namespace the synthesized foreign package is mounted under.
    pub alias: String,
    pub header: String,
    pub provider: String,
    pub provider_kind: BuildCImportProviderKind,
    /// The versioned annotation overlay, when the import declares one.
    pub annotations: Option<String>,
    /// The target this provider is for; `None` means every target.
    pub target: Option<String>,
    pub dialect: Option<String>,
    pub compiler: Option<String>,
    pub sysroot: Option<String>,
    pub include_roots: Vec<String>,
    pub system_include_roots: Vec<String>,
    pub defines: Vec<String>,
}

/// One `add_c_import` record before the graph binds it to an artifact.
///
/// `Default` exists so a caller naming three fields does not have to spell the
/// eight optional ones, which is what keeps the frozen record cheap to extend.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildCImportDeclaration {
    pub alias: String,
    pub header: String,
    pub provider: String,
    pub provider_kind: Option<BuildCImportProviderKind>,
    pub annotations: Option<String>,
    pub target: Option<String>,
    pub dialect: Option<String>,
    pub compiler: Option<String>,
    pub sysroot: Option<String>,
    pub include_roots: Vec<String>,
    pub system_include_roots: Vec<String>,
    pub defines: Vec<String>,
}

impl BuildCImportDeclaration {
    /// Bind this declaration to an artifact without going through the graph.
    ///
    /// The graph's `add_c_import` is the checked path; this is the projection
    /// on its own, for callers that already hold a validated declaration.
    pub fn into_attachment(self, artifact_id: BuildArtifactId) -> BuildCImportAttachment {
        BuildCImportAttachment {
            artifact_id,
            alias: self.alias,
            header: self.header,
            provider: self.provider,
            provider_kind: self
                .provider_kind
                .unwrap_or(BuildCImportProviderKind::Object),
            annotations: self.annotations,
            target: self.target,
            dialect: self.dialect,
            compiler: self.compiler,
            sysroot: self.sysroot,
            include_roots: self.include_roots,
            system_include_roots: self.system_include_roots,
            defines: self.defines,
        }
    }
}

/// The alias becomes a FOL namespace, so it has to be spellable as one.
fn is_valid_c_import_alias(alias: &str) -> bool {
    let mut chars = alias.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildCImportAttachmentError {
    UnknownArtifact(BuildArtifactId),
    Duplicate {
        artifact_id: BuildArtifactId,
        header: String,
        provider: String,
    },
    DuplicateAlias {
        artifact_id: BuildArtifactId,
        alias: String,
    },
    InvalidAlias(String),
}

impl std::fmt::Display for BuildCImportAttachmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownArtifact(artifact_id) => {
                write!(f, "cannot attach C import to unknown artifact '{artifact_id}'")
            }
            Self::Duplicate {
                artifact_id,
                header,
                provider,
            } => write!(
                f,
                "artifact '{artifact_id}' already has C import header '{header}' with provider '{provider}'"
            ),
            Self::DuplicateAlias { artifact_id, alias } => write!(
                f,
                "artifact '{artifact_id}' already imports a C namespace aliased '{alias}'"
            ),
            Self::InvalidAlias(alias) => write!(
                f,
                "C import alias '{alias}' must match [a-z][a-z0-9_]*"
            ),
        }
    }
}

impl std::error::Error for BuildCImportAttachmentError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildModule {
    pub id: BuildModuleId,
    pub kind: BuildModuleKind,
    pub name: String,
}

/// One entry of a library artifact's ABI export allowlist.
///
/// `[exp]` makes a routine selectable and never exports a native symbol on its
/// own (section 4.10), so this is the second half: the artifact naming exactly
/// which routines become C symbols, and under exactly which names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildAbiExport {
    /// Fully qualified FOL routine, e.g. `api::add`.
    pub routine: String,
    /// The exact external C symbol. Never mangled, never inferred.
    pub symbol: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildGeneratedFile {
    pub id: BuildGeneratedFileId,
    pub kind: BuildGeneratedFileKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOption {
    pub id: BuildOptionId,
    pub kind: BuildOptionKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInstall {
    pub id: BuildInstallId,
    pub kind: BuildInstallKind,
    pub name: String,
    pub target: Option<BuildInstallTarget>,
    pub projected_destination: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildStepDependency {
    pub step: BuildStepId,
    pub depends_on: BuildStepId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildArtifactInput {
    Module(BuildModuleId),
    GeneratedFile(BuildGeneratedFileId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildArtifactDependency {
    pub artifact: BuildArtifactId,
    pub input: BuildArtifactInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildInstallTarget {
    Artifact(BuildArtifactId),
    GeneratedFile(BuildGeneratedFileId),
    DirectoryPath(String),
}

// --- New Slice 7 graph IR types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildRunArg {
    Literal(String),
    GeneratedFile(BuildGeneratedFileId),
    Path(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildRunConfig {
    pub args: Vec<BuildRunArg>,
    pub env: Vec<(String, String)>,
    pub capture_stdout: Option<BuildGeneratedFileId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildArtifactLink {
    pub artifact: BuildArtifactId,
    pub linked: BuildArtifactId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildArtifactModuleImport {
    pub artifact: BuildArtifactId,
    pub module: BuildModuleId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildStepAttachment {
    pub step: BuildStepId,
    pub generated_file: BuildGeneratedFileId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildGraphValidationErrorKind {
    StepDependencyCycle,
    MissingArtifactInput,
    InvalidInstallTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildGraphValidationError {
    pub kind: BuildGraphValidationErrorKind,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildGraph {
    steps: Vec<BuildStep>,
    artifacts: Vec<BuildArtifact>,
    modules: Vec<BuildModule>,
    generated_files: Vec<BuildGeneratedFile>,
    options: Vec<BuildOption>,
    installs: Vec<BuildInstall>,
    step_dependencies: Vec<BuildStepDependency>,
    artifact_dependencies: Vec<BuildArtifactDependency>,
    run_configs: std::collections::BTreeMap<BuildStepId, BuildRunConfig>,
    artifact_links: Vec<BuildArtifactLink>,
    artifact_module_imports: Vec<BuildArtifactModuleImport>,
    c_imports: Vec<BuildCImportAttachment>,
    step_attachments: Vec<BuildStepAttachment>,
}

impl BuildGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn steps(&self) -> &[BuildStep] {
        &self.steps
    }

    pub fn artifacts(&self) -> &[BuildArtifact] {
        &self.artifacts
    }

    pub fn modules(&self) -> &[BuildModule] {
        &self.modules
    }

    pub fn generated_files(&self) -> &[BuildGeneratedFile] {
        &self.generated_files
    }

    pub fn options(&self) -> &[BuildOption] {
        &self.options
    }

    pub fn installs(&self) -> &[BuildInstall] {
        &self.installs
    }

    pub fn step_dependencies(&self) -> &[BuildStepDependency] {
        &self.step_dependencies
    }

    pub fn artifact_dependencies(&self) -> &[BuildArtifactDependency] {
        &self.artifact_dependencies
    }

    pub fn c_imports(&self) -> &[BuildCImportAttachment] {
        &self.c_imports
    }

    pub fn c_imports_for(
        &self,
        artifact_id: BuildArtifactId,
    ) -> impl Iterator<Item = &BuildCImportAttachment> {
        self.c_imports
            .iter()
            .filter(move |attachment| attachment.artifact_id == artifact_id)
    }

    pub fn add_step(
        &mut self,
        kind: BuildStepKind,
        name: impl Into<String>,
        description: Option<String>,
    ) -> BuildStepId {
        let id = BuildStepId::from_index(self.steps.len());
        self.steps.push(BuildStep {
            id,
            kind,
            name: name.into(),
            description,
        });
        id
    }

    pub fn add_artifact(
        &mut self,
        kind: BuildArtifactKind,
        name: impl Into<String>,
    ) -> BuildArtifactId {
        self.add_configured_artifact(
            kind,
            name,
            String::new(),
            crate::artifact::BuildArtifactFolModel::Memo,
            fol_types::ResolvedTarget::host()
                .expect("fol-build requires a supported concrete host target"),
            crate::option::BuildOptimizeMode::Debug,
        )
    }

    pub fn add_configured_artifact(
        &mut self,
        kind: BuildArtifactKind,
        name: impl Into<String>,
        root_module: impl Into<String>,
        fol_model: crate::artifact::BuildArtifactFolModel,
        target: fol_types::ResolvedTarget,
        optimize: crate::option::BuildOptimizeMode,
    ) -> BuildArtifactId {
        let id = BuildArtifactId::from_index(self.artifacts.len());
        self.artifacts.push(BuildArtifact {
            id,
            kind,
            name: name.into(),
            root_module: root_module.into(),
            fol_model,
            target,
            optimize,
            library_paths: Vec::new(),
            link_inputs: Vec::new(),
            abi_version: None,
            abi_exports: Vec::new(),
        });
        id
    }

    pub fn artifact(&self, artifact_id: BuildArtifactId) -> Option<&BuildArtifact> {
        self.artifacts
            .get(artifact_id.index())
            .filter(|artifact| artifact.id == artifact_id)
    }

    pub fn artifact_mut(&mut self, artifact_id: BuildArtifactId) -> Option<&mut BuildArtifact> {
        self.artifacts
            .get_mut(artifact_id.index())
            .filter(|artifact| artifact.id == artifact_id)
    }

    pub fn add_c_import(
        &mut self,
        artifact_id: BuildArtifactId,
        declaration: BuildCImportDeclaration,
    ) -> Result<BuildCImportAttachment, BuildCImportAttachmentError> {
        if self.artifact(artifact_id).is_none() {
            return Err(BuildCImportAttachmentError::UnknownArtifact(artifact_id));
        }
        if !is_valid_c_import_alias(&declaration.alias) {
            return Err(BuildCImportAttachmentError::InvalidAlias(declaration.alias));
        }
        let attachment = declaration.into_attachment(artifact_id);
        if self.c_imports.contains(&attachment) {
            return Err(BuildCImportAttachmentError::Duplicate {
                artifact_id,
                header: attachment.header,
                provider: attachment.provider,
            });
        }
        if self.c_imports.iter().any(|existing| {
            existing.artifact_id == artifact_id && existing.alias == attachment.alias
        }) {
            return Err(BuildCImportAttachmentError::DuplicateAlias {
                artifact_id,
                alias: attachment.alias,
            });
        }
        self.c_imports.push(attachment.clone());
        Ok(attachment)
    }

    pub fn add_module(&mut self, kind: BuildModuleKind, name: impl Into<String>) -> BuildModuleId {
        let id = BuildModuleId::from_index(self.modules.len());
        self.modules.push(BuildModule {
            id,
            kind,
            name: name.into(),
        });
        id
    }

    pub fn add_generated_file(
        &mut self,
        kind: BuildGeneratedFileKind,
        name: impl Into<String>,
    ) -> BuildGeneratedFileId {
        let id = BuildGeneratedFileId::from_index(self.generated_files.len());
        self.generated_files.push(BuildGeneratedFile {
            id,
            kind,
            name: name.into(),
        });
        id
    }

    pub fn add_option(&mut self, kind: BuildOptionKind, name: impl Into<String>) -> BuildOptionId {
        let id = BuildOptionId::from_index(self.options.len());
        self.options.push(BuildOption {
            id,
            kind,
            name: name.into(),
        });
        id
    }

    pub fn add_install(
        &mut self,
        kind: BuildInstallKind,
        name: impl Into<String>,
    ) -> BuildInstallId {
        self.add_install_with_target(kind, name, None, String::new())
    }

    pub fn add_install_with_target(
        &mut self,
        kind: BuildInstallKind,
        name: impl Into<String>,
        target: Option<BuildInstallTarget>,
        projected_destination: impl Into<String>,
    ) -> BuildInstallId {
        let id = BuildInstallId::from_index(self.installs.len());
        self.installs.push(BuildInstall {
            id,
            kind,
            name: name.into(),
            target,
            projected_destination: projected_destination.into(),
        });
        id
    }

    pub fn add_step_dependency(&mut self, step: BuildStepId, depends_on: BuildStepId) {
        if self
            .step_dependencies
            .iter()
            .any(|edge| edge.step == step && edge.depends_on == depends_on)
        {
            return;
        }
        self.step_dependencies
            .push(BuildStepDependency { step, depends_on });
    }

    pub fn step_dependencies_for(
        &self,
        step: BuildStepId,
    ) -> impl Iterator<Item = BuildStepId> + '_ {
        self.step_dependencies
            .iter()
            .filter(move |edge| edge.step == step)
            .map(|edge| edge.depends_on)
    }

    pub fn add_artifact_module_input(&mut self, artifact: BuildArtifactId, module: BuildModuleId) {
        self.artifact_dependencies.push(BuildArtifactDependency {
            artifact,
            input: BuildArtifactInput::Module(module),
        });
    }

    pub fn add_artifact_generated_file_input(
        &mut self,
        artifact: BuildArtifactId,
        generated_file: BuildGeneratedFileId,
    ) {
        self.artifact_dependencies.push(BuildArtifactDependency {
            artifact,
            input: BuildArtifactInput::GeneratedFile(generated_file),
        });
    }

    pub fn artifact_inputs_for(
        &self,
        artifact: BuildArtifactId,
    ) -> impl Iterator<Item = BuildArtifactInput> + '_ {
        self.artifact_dependencies
            .iter()
            .filter(move |edge| edge.artifact == artifact)
            .map(|edge| edge.input)
    }

    pub fn run_config_for(&self, step: BuildStepId) -> Option<&BuildRunConfig> {
        self.run_configs.get(&step)
    }

    pub fn set_run_config(&mut self, step: BuildStepId, config: BuildRunConfig) {
        self.run_configs.insert(step, config);
    }

    pub fn run_config_mut(&mut self, step: BuildStepId) -> &mut BuildRunConfig {
        self.run_configs.entry(step).or_default()
    }

    pub fn add_artifact_link(&mut self, artifact: BuildArtifactId, linked: BuildArtifactId) {
        if !self
            .artifact_links
            .iter()
            .any(|l| l.artifact == artifact && l.linked == linked)
        {
            self.artifact_links
                .push(BuildArtifactLink { artifact, linked });
        }
    }

    /// Declare the artifact's ABI major/minor.
    pub fn set_artifact_abi_version(&mut self, artifact: BuildArtifactId, major: u32, minor: u32) {
        if let Some(artifact) = self.artifact_mut(artifact) {
            artifact.abi_version = Some((major, minor));
        }
    }

    /// Add one entry to the artifact's export allowlist.
    pub fn add_artifact_abi_export(&mut self, artifact: BuildArtifactId, export: BuildAbiExport) {
        if let Some(artifact) = self.artifact_mut(artifact) {
            artifact.abi_exports.push(export);
        }
    }

    pub fn add_artifact_system_library(
        &mut self,
        artifact: BuildArtifactId,
        request: &crate::native::SystemLibraryRequest,
    ) {
        let Some(artifact) = self.artifact_mut(artifact) else {
            return;
        };
        if let Some(search_path) = request.search_path.as_ref() {
            let path = NativeLibraryPath {
                origin: NativeSearchPathOrigin::System,
                relative_path: search_path.clone(),
            };
            if !artifact.library_paths.contains(&path) {
                artifact.library_paths.push(path);
            }
        }
        let directive = NativeLinkDirective {
            input: request.link_input(),
            mode: request.mode,
        };
        if !artifact.link_inputs.contains(&directive) {
            artifact.link_inputs.push(directive);
        }
    }

    pub fn artifact_links_for(
        &self,
        artifact: BuildArtifactId,
    ) -> impl Iterator<Item = BuildArtifactId> + '_ {
        self.artifact_links
            .iter()
            .filter(move |l| l.artifact == artifact)
            .map(|l| l.linked)
    }

    pub fn add_artifact_module_import(&mut self, artifact: BuildArtifactId, module: BuildModuleId) {
        if !self
            .artifact_module_imports
            .iter()
            .any(|i| i.artifact == artifact && i.module == module)
        {
            self.artifact_module_imports
                .push(BuildArtifactModuleImport { artifact, module });
        }
    }

    pub fn artifact_module_imports_for(
        &self,
        artifact: BuildArtifactId,
    ) -> impl Iterator<Item = BuildModuleId> + '_ {
        self.artifact_module_imports
            .iter()
            .filter(move |i| i.artifact == artifact)
            .map(|i| i.module)
    }

    pub fn add_step_attachment(&mut self, step: BuildStepId, generated_file: BuildGeneratedFileId) {
        if !self
            .step_attachments
            .iter()
            .any(|a| a.step == step && a.generated_file == generated_file)
        {
            self.step_attachments.push(BuildStepAttachment {
                step,
                generated_file,
            });
        }
    }

    pub fn step_attachments_for(
        &self,
        step: BuildStepId,
    ) -> impl Iterator<Item = BuildGeneratedFileId> + '_ {
        self.step_attachments
            .iter()
            .filter(move |a| a.step == step)
            .map(|a| a.generated_file)
    }

    pub fn validate(&self) -> Vec<BuildGraphValidationError> {
        let mut errors = Vec::new();
        self.validate_step_dependencies(&mut errors);
        self.validate_artifact_inputs(&mut errors);
        self.validate_installs(&mut errors);
        errors
    }

    fn validate_step_dependencies(&self, errors: &mut Vec<BuildGraphValidationError>) {
        let mut visiting = vec![false; self.steps.len()];
        let mut visited = vec![false; self.steps.len()];
        let mut stack = Vec::new();

        for step in &self.steps {
            self.visit_step_dependencies(step.id, &mut visiting, &mut visited, &mut stack, errors);
        }
    }

    fn validate_artifact_inputs(&self, errors: &mut Vec<BuildGraphValidationError>) {
        for edge in &self.artifact_dependencies {
            if edge.artifact.index() >= self.artifacts.len() {
                errors.push(BuildGraphValidationError {
                    kind: BuildGraphValidationErrorKind::MissingArtifactInput,
                    message: format!(
                        "artifact input edge references unknown artifact {}",
                        edge.artifact
                    ),
                });
                continue;
            }

            match edge.input {
                BuildArtifactInput::Module(module) if module.index() >= self.modules.len() => {
                    errors.push(BuildGraphValidationError {
                        kind: BuildGraphValidationErrorKind::MissingArtifactInput,
                        message: format!(
                            "artifact input edge references unknown module {} for {}",
                            module, edge.artifact
                        ),
                    });
                }
                BuildArtifactInput::GeneratedFile(generated)
                    if generated.index() >= self.generated_files.len() =>
                {
                    errors.push(BuildGraphValidationError {
                        kind: BuildGraphValidationErrorKind::MissingArtifactInput,
                        message: format!(
                            "artifact input edge references unknown generated file {} for {}",
                            generated, edge.artifact
                        ),
                    });
                }
                _ => {}
            }
        }
    }

    fn validate_installs(&self, errors: &mut Vec<BuildGraphValidationError>) {
        for install in &self.installs {
            match (&install.kind, install.target.as_ref()) {
                (BuildInstallKind::Artifact, Some(BuildInstallTarget::Artifact(artifact))) => {
                    if artifact.index() >= self.artifacts.len() {
                        errors.push(BuildGraphValidationError {
                            kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                            message: format!(
                                "install {} references unknown artifact {}",
                                install.id, artifact
                            ),
                        });
                    }
                }
                (BuildInstallKind::File, Some(BuildInstallTarget::GeneratedFile(generated))) => {
                    if generated.index() >= self.generated_files.len() {
                        errors.push(BuildGraphValidationError {
                            kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                            message: format!(
                                "install {} references unknown generated file {}",
                                install.id, generated
                            ),
                        });
                    }
                }
                (
                    BuildInstallKind::Directory,
                    Some(BuildInstallTarget::GeneratedFile(generated)),
                ) => match self.generated_files.get(generated.index()) {
                    None => {
                        errors.push(BuildGraphValidationError {
                            kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                            message: format!(
                                "install {} references unknown generated file {}",
                                install.id, generated
                            ),
                        });
                    }
                    Some(file) if file.kind != BuildGeneratedFileKind::GeneratedDir => {
                        errors.push(BuildGraphValidationError {
                            kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                            message: format!(
                                "install {} directory target {} is not a generated directory",
                                install.id, generated
                            ),
                        });
                    }
                    Some(_) => {}
                },
                (BuildInstallKind::Directory, Some(BuildInstallTarget::DirectoryPath(path))) => {
                    if path.is_empty() {
                        errors.push(BuildGraphValidationError {
                            kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                            message: format!(
                                "install {} directory target must not be empty",
                                install.id
                            ),
                        });
                    }
                }
                (_, None) => {
                    errors.push(BuildGraphValidationError {
                        kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                        message: format!("install {} is missing a target", install.id),
                    });
                }
                _ => {
                    errors.push(BuildGraphValidationError {
                        kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                        message: format!(
                            "install {} target shape does not match {:?}",
                            install.id, install.kind
                        ),
                    });
                }
            }
        }
    }

    fn visit_step_dependencies(
        &self,
        step: BuildStepId,
        visiting: &mut [bool],
        visited: &mut [bool],
        stack: &mut Vec<BuildStepId>,
        errors: &mut Vec<BuildGraphValidationError>,
    ) {
        let index = step.index();
        if index >= self.steps.len() || visited[index] {
            return;
        }
        if stack.len() >= MAX_GRAPH_DEPTH {
            errors.push(BuildGraphValidationError {
                kind: BuildGraphValidationErrorKind::StepDependencyCycle,
                message: format!(
                    "step dependency graph exceeded maximum depth ({MAX_GRAPH_DEPTH})"
                ),
            });
            return;
        }
        if visiting[index] {
            let cycle_start = stack
                .iter()
                .position(|candidate| *candidate == step)
                .unwrap_or(0);
            let mut cycle = stack[cycle_start..]
                .iter()
                .map(|entry| entry.to_string())
                .collect::<Vec<_>>();
            cycle.push(step.to_string());
            errors.push(BuildGraphValidationError {
                kind: BuildGraphValidationErrorKind::StepDependencyCycle,
                message: format!("step dependency cycle detected: {}", cycle.join(" -> ")),
            });
            return;
        }

        visiting[index] = true;
        stack.push(step);
        for dependency in self.step_dependencies_for(step) {
            self.visit_step_dependencies(dependency, visiting, visited, stack, errors);
        }
        stack.pop();
        visiting[index] = false;
        visited[index] = true;
    }
}

use crate::native::{NativeLibraryPath, NativeLinkDirective, NativeSearchPathOrigin};

#[cfg(test)]
mod tests {
    use super::{
        BuildArtifactDependency, BuildArtifactId, BuildArtifactInput, BuildArtifactKind,
        BuildCImportAttachment, BuildCImportAttachmentError, BuildCImportDeclaration,
        BuildCImportProviderKind, BuildGeneratedFileId, BuildGeneratedFileKind, BuildGraph,
        BuildGraphValidationError, BuildGraphValidationErrorKind, BuildInstallId, BuildInstallKind,
        BuildInstallTarget, BuildModuleId, BuildModuleKind, BuildOptionId, BuildOptionKind,
        BuildStepDependency, BuildStepId, BuildStepKind,
    };

    #[test]
    fn build_graph_ids_round_trip_their_raw_indexes() {
        assert_eq!(BuildStepId::from_index(3).index(), 3);
        assert_eq!(BuildArtifactId::from_index(5).index(), 5);
        assert_eq!(BuildModuleId::from_index(7).index(), 7);
        assert_eq!(BuildGeneratedFileId::from_index(11).index(), 11);
        assert_eq!(BuildOptionId::from_index(13).index(), 13);
        assert_eq!(BuildInstallId::from_index(17).index(), 17);
    }

    #[test]
    fn build_graph_ids_render_with_stable_family_prefixes() {
        assert_eq!(BuildStepId(0).to_string(), "step:0");
        assert_eq!(BuildArtifactId(1).to_string(), "artifact:1");
        assert_eq!(BuildModuleId(2).to_string(), "module:2");
        assert_eq!(BuildGeneratedFileId(3).to_string(), "generated:3");
        assert_eq!(BuildOptionId(4).to_string(), "option:4");
        assert_eq!(BuildInstallId(5).to_string(), "install:5");
    }

    #[test]
    fn build_graph_kind_enums_cover_the_round_two_ir_vocab() {
        assert_eq!(BuildStepKind::Run, BuildStepKind::Run);
        assert_eq!(BuildArtifactKind::Executable, BuildArtifactKind::Executable);
        assert_eq!(BuildModuleKind::Generated, BuildModuleKind::Generated);
        assert_eq!(
            BuildGeneratedFileKind::CaptureOutput,
            BuildGeneratedFileKind::CaptureOutput
        );
        assert_eq!(BuildOptionKind::Optimize, BuildOptionKind::Optimize);
        assert_eq!(BuildOptionKind::Int, BuildOptionKind::Int);
        assert_eq!(BuildOptionKind::Path, BuildOptionKind::Path);
        assert_eq!(BuildInstallKind::Directory, BuildInstallKind::Directory);
    }

    #[test]
    fn build_graph_allocators_assign_dense_ids_per_node_family() {
        let mut graph = BuildGraph::new();

        let compile_step = graph.add_step(BuildStepKind::Default, "compile", None);
        let run_step = graph.add_step(BuildStepKind::Run, "run", None);
        let exe = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let module = graph.add_module(BuildModuleKind::Source, "app.main");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, "version.rs");
        let option = graph.add_option(BuildOptionKind::Target, "target");
        let install = graph.add_install(BuildInstallKind::Artifact, "install-app");

        assert_eq!(compile_step, BuildStepId(0));
        assert_eq!(run_step, BuildStepId(1));
        assert_eq!(exe, BuildArtifactId(0));
        assert_eq!(module, BuildModuleId(0));
        assert_eq!(generated, BuildGeneratedFileId(0));
        assert_eq!(option, BuildOptionId(0));
        assert_eq!(install, BuildInstallId(0));
    }

    #[test]
    fn build_graph_storage_tables_preserve_inserted_records() {
        let mut graph = BuildGraph::new();

        graph.add_step(BuildStepKind::Test, "test", None);
        graph.add_artifact(BuildArtifactKind::StaticLibrary, "support");
        graph.add_module(BuildModuleKind::Imported, "dep.math");
        graph.add_generated_file(BuildGeneratedFileKind::Copy, "config.json");
        graph.add_option(BuildOptionKind::Bool, "enable-logs");
        graph.add_install(BuildInstallKind::Directory, "install-assets");

        assert_eq!(graph.steps()[0].name, "test");
        assert_eq!(graph.artifacts()[0].kind, BuildArtifactKind::StaticLibrary);
        assert_eq!(graph.modules()[0].kind, BuildModuleKind::Imported);
        assert_eq!(
            graph.generated_files()[0].kind,
            BuildGeneratedFileKind::Copy
        );
        assert_eq!(graph.options()[0].kind, BuildOptionKind::Bool);
        assert_eq!(graph.installs()[0].kind, BuildInstallKind::Directory);
        assert_eq!(graph.installs()[0].target, None);
    }

    #[test]
    fn c_import_attachments_are_typed_and_scoped_to_their_artifact() {
        let mut graph = BuildGraph::new();
        let app = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let helper = graph.add_artifact(BuildArtifactKind::StaticLibrary, "helper");

        let attachment = graph
            .add_c_import(app, widget_declaration())
            .expect("known artifacts should accept exact object providers");

        assert_eq!(
            attachment,
            BuildCImportAttachment {
                artifact_id: app,
                alias: "widget".to_string(),
                header: "native/widget.h".to_string(),
                provider: "native/widget.o".to_string(),
                provider_kind: BuildCImportProviderKind::Object,
                annotations: None,
                target: None,
                dialect: None,
                compiler: None,
                sysroot: None,
                include_roots: Vec::new(),
                system_include_roots: Vec::new(),
                defines: Vec::new(),
            }
        );
        assert_eq!(graph.c_imports(), std::slice::from_ref(&attachment));
        assert_eq!(
            graph.c_imports_for(app).collect::<Vec<_>>(),
            vec![&attachment]
        );
        assert!(graph.c_imports_for(helper).next().is_none());
        assert_eq!(graph.artifact(app).map(|artifact| artifact.id), Some(app));
        assert!(graph.artifact(BuildArtifactId(99)).is_none());
    }

    #[test]
    fn c_import_attachments_reject_duplicates_and_unknown_artifacts() {
        let mut graph = BuildGraph::new();
        let app = graph.add_artifact(BuildArtifactKind::Executable, "app");
        graph
            .add_c_import(app, widget_declaration())
            .expect("first attachment should succeed");

        assert!(matches!(
            graph.add_c_import(app, widget_declaration()),
            Err(BuildCImportAttachmentError::Duplicate {
                artifact_id,
                ref header,
                ref provider,
            }) if artifact_id == app
                && header == "native/widget.h"
                && provider == "native/widget.o"
        ));
        assert_eq!(graph.c_imports().len(), 1);

        // A second import under the same alias would give one artifact two
        // namespaces with one name; a distinct alias is the supported form.
        assert_eq!(
            graph.add_c_import(
                app,
                BuildCImportDeclaration {
                    header: "native/other.h".to_string(),
                    provider: "native/other.o".to_string(),
                    ..widget_declaration()
                },
            ),
            Err(BuildCImportAttachmentError::DuplicateAlias {
                artifact_id: app,
                alias: "widget".to_string(),
            })
        );
        assert_eq!(graph.c_imports().len(), 1);

        graph
            .add_c_import(
                app,
                BuildCImportDeclaration {
                    alias: "other".to_string(),
                    header: "native/other.h".to_string(),
                    provider: "native/other.o".to_string(),
                    ..widget_declaration()
                },
            )
            .expect("a distinct alias should attach a second namespace");
        assert_eq!(graph.c_imports().len(), 2);

        let unknown = BuildArtifactId(99);
        assert_eq!(
            graph.add_c_import(unknown, widget_declaration()),
            Err(BuildCImportAttachmentError::UnknownArtifact(unknown))
        );
        assert_eq!(graph.c_imports().len(), 2);
    }

    #[test]
    fn c_import_aliases_must_be_spellable_as_fol_namespaces() {
        let mut graph = BuildGraph::new();
        let app = graph.add_artifact(BuildArtifactKind::Executable, "app");

        for alias in ["Widget", "9widget", "wid-get", "", "widget::inner"] {
            assert_eq!(
                graph.add_c_import(
                    app,
                    BuildCImportDeclaration {
                        alias: alias.to_string(),
                        ..widget_declaration()
                    },
                ),
                Err(BuildCImportAttachmentError::InvalidAlias(alias.to_string())),
                "alias '{alias}' should be refused"
            );
        }
        assert!(graph.c_imports().is_empty());
    }

    fn widget_declaration() -> BuildCImportDeclaration {
        BuildCImportDeclaration {
            alias: "widget".to_string(),
            header: "native/widget.h".to_string(),
            provider: "native/widget.o".to_string(),
            provider_kind: Some(BuildCImportProviderKind::Object),
            ..BuildCImportDeclaration::default()
        }
    }

    #[test]
    fn c_import_provider_kind_accepts_only_exact_object_spelling() {
        assert_eq!(
            BuildCImportProviderKind::parse("object"),
            Some(BuildCImportProviderKind::Object)
        );
        assert_eq!(BuildCImportProviderKind::Object.as_str(), "object");
        for invalid in ["Object", "OBJECT", "obj", "archive", ""] {
            assert_eq!(BuildCImportProviderKind::parse(invalid), None);
        }
    }

    #[test]
    fn build_graph_records_explicit_step_dependencies() {
        let mut graph = BuildGraph::new();
        let compile = graph.add_step(BuildStepKind::Default, "compile", None);
        let test = graph.add_step(BuildStepKind::Test, "test", None);
        let run = graph.add_step(BuildStepKind::Run, "run", None);

        graph.add_step_dependency(test, compile);
        graph.add_step_dependency(run, compile);

        assert_eq!(
            graph.step_dependencies(),
            &[
                BuildStepDependency {
                    step: test,
                    depends_on: compile,
                },
                BuildStepDependency {
                    step: run,
                    depends_on: compile,
                },
            ]
        );
    }

    #[test]
    fn build_graph_can_query_dependencies_for_one_step() {
        let mut graph = BuildGraph::new();
        let compile = graph.add_step(BuildStepKind::Default, "compile", None);
        let install = graph.add_step(BuildStepKind::Install, "install", None);
        let run = graph.add_step(BuildStepKind::Run, "run", None);

        graph.add_step_dependency(install, compile);
        graph.add_step_dependency(run, compile);

        let install_dependencies = graph.step_dependencies_for(install).collect::<Vec<_>>();
        let run_dependencies = graph.step_dependencies_for(run).collect::<Vec<_>>();

        assert_eq!(install_dependencies, vec![compile]);
        assert_eq!(run_dependencies, vec![compile]);
    }

    #[test]
    fn build_graph_dedupes_repeated_step_dependencies() {
        let mut graph = BuildGraph::new();
        let compile = graph.add_step(BuildStepKind::Default, "compile", None);
        let run = graph.add_step(BuildStepKind::Run, "run", None);

        graph.add_step_dependency(run, compile);
        graph.add_step_dependency(run, compile);

        assert_eq!(
            graph.step_dependencies(),
            &[BuildStepDependency {
                step: run,
                depends_on: compile,
            }]
        );
    }

    #[test]
    fn build_graph_records_module_and_generated_file_artifact_inputs() {
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let module = graph.add_module(BuildModuleKind::Source, "app.main");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Write, "version.txt");

        graph.add_artifact_module_input(artifact, module);
        graph.add_artifact_generated_file_input(artifact, generated);

        assert_eq!(
            graph.artifact_dependencies(),
            &[
                BuildArtifactDependency {
                    artifact,
                    input: BuildArtifactInput::Module(module),
                },
                BuildArtifactDependency {
                    artifact,
                    input: BuildArtifactInput::GeneratedFile(generated),
                },
            ]
        );
    }

    #[test]
    fn build_graph_can_query_inputs_for_one_artifact() {
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::StaticLibrary, "support");
        let module = graph.add_module(BuildModuleKind::Imported, "dep.math");
        let generated = graph.add_generated_file(BuildGeneratedFileKind::Copy, "config.json");

        graph.add_artifact_module_input(artifact, module);
        graph.add_artifact_generated_file_input(artifact, generated);

        let inputs = graph.artifact_inputs_for(artifact).collect::<Vec<_>>();

        assert_eq!(
            inputs,
            vec![
                BuildArtifactInput::Module(module),
                BuildArtifactInput::GeneratedFile(generated),
            ]
        );
    }

    #[test]
    fn empty_build_graph_validation_is_clean() {
        let graph = BuildGraph::new();

        assert!(graph.validate().is_empty());
    }

    #[test]
    fn build_graph_validation_errors_keep_kind_and_message() {
        let error = BuildGraphValidationError {
            kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
            message: "install target must resolve to a known artifact".to_string(),
        };

        assert_eq!(
            error,
            BuildGraphValidationError {
                kind: BuildGraphValidationErrorKind::InvalidInstallTarget,
                message: "install target must resolve to a known artifact".to_string(),
            }
        );
    }

    #[test]
    fn build_graph_validation_rejects_self_cycles() {
        let mut graph = BuildGraph::new();
        let build = graph.add_step(BuildStepKind::Default, "build", None);

        graph.add_step_dependency(build, build);

        let errors = graph.validate();

        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].kind,
            BuildGraphValidationErrorKind::StepDependencyCycle
        );
        assert!(errors[0].message.contains("step:0 -> step:0"));
    }

    #[test]
    fn build_graph_validation_rejects_multi_step_cycles() {
        let mut graph = BuildGraph::new();
        let build = graph.add_step(BuildStepKind::Default, "build", None);
        let test = graph.add_step(BuildStepKind::Test, "test", None);
        let run = graph.add_step(BuildStepKind::Run, "run", None);

        graph.add_step_dependency(build, test);
        graph.add_step_dependency(test, run);
        graph.add_step_dependency(run, build);

        let errors = graph.validate();

        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].kind,
            BuildGraphValidationErrorKind::StepDependencyCycle
        );
        assert!(errors[0].message.contains("step:0"));
        assert!(errors[0].message.contains("step:1"));
        assert!(errors[0].message.contains("step:2"));
    }

    #[test]
    fn build_graph_validation_rejects_unknown_artifact_inputs() {
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");

        graph.add_artifact_module_input(artifact, BuildModuleId(99));
        graph.add_artifact_generated_file_input(artifact, BuildGeneratedFileId(77));

        let errors = graph.validate();

        assert_eq!(errors.len(), 2);
        assert!(errors
            .iter()
            .all(|error| error.kind == BuildGraphValidationErrorKind::MissingArtifactInput));
    }

    #[test]
    fn build_graph_validation_rejects_invalid_install_targets() {
        let mut graph = BuildGraph::new();
        graph.add_install(BuildInstallKind::Artifact, "install-missing");
        graph.add_install_with_target(
            BuildInstallKind::Artifact,
            "install-wrong-shape",
            Some(BuildInstallTarget::DirectoryPath("bin".to_string())),
            String::new(),
        );
        graph.add_install_with_target(
            BuildInstallKind::File,
            "install-unknown-generated",
            Some(BuildInstallTarget::GeneratedFile(BuildGeneratedFileId(44))),
            String::new(),
        );

        let errors = graph.validate();

        assert_eq!(errors.len(), 3);
        assert!(errors
            .iter()
            .all(|error| error.kind == BuildGraphValidationErrorKind::InvalidInstallTarget));
    }

    #[test]
    fn build_graph_validation_accepts_generated_directory_installs() {
        let mut graph = BuildGraph::new();
        let generated_dir =
            graph.add_generated_file(BuildGeneratedFileKind::GeneratedDir, "assets");
        graph.add_install_with_target(
            BuildInstallKind::Directory,
            "install-generated-dir",
            Some(BuildInstallTarget::GeneratedFile(generated_dir)),
            "assets".to_string(),
        );

        assert!(graph.validate().is_empty());

        // A directory install pointing at a plain generated FILE is invalid.
        let generated_file = graph.add_generated_file(BuildGeneratedFileKind::Write, "notes.txt");
        graph.add_install_with_target(
            BuildInstallKind::Directory,
            "install-generated-file-as-dir",
            Some(BuildInstallTarget::GeneratedFile(generated_file)),
            "notes.txt".to_string(),
        );

        let errors = graph.validate();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].kind,
            BuildGraphValidationErrorKind::InvalidInstallTarget
        );
    }
    // --- V4 M0 characterization ---------------------------------------
    //
    // Both tests below record hazards rather than guarantees. They exist so the
    // milestone that fixes each one has to edit a test instead of silently
    // changing behaviour.

    /// Graph identity is the insertion index. Two generated files that declare
    /// the same public name are both accepted and are distinguished only by
    /// that index, so declaration order is part of the contract without ever
    /// being stated as one. M2 gives actions real cache identity.
    #[test]
    fn generated_file_names_are_not_unique_and_identity_is_positional() {
        let mut graph = BuildGraph::new();
        let first = graph.add_generated_file(BuildGeneratedFileKind::Write, "config.fol");
        let second = graph.add_generated_file(BuildGeneratedFileKind::Write, "config.fol");

        assert_ne!(first, second);
        assert_eq!(first.index(), 0);
        assert_eq!(second.index(), 1);
        assert_eq!(first.to_string(), "generated:0");
        // The name cannot tell them apart, so nothing downstream can either.
        assert_eq!(
            graph.generated_files[first.index()].name,
            graph.generated_files[second.index()].name
        );
    }

    /// Declaring the same artifact name twice is accepted at the graph layer.
    /// The integration characterization proves it survives all the way to a
    /// successful `run`.
    #[test]
    fn artifact_names_are_not_unique_within_one_graph() {
        let mut graph = BuildGraph::new();
        let first = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let second = graph.add_artifact(BuildArtifactKind::Executable, "app");

        assert_ne!(first, second);
        assert_eq!(graph.artifacts.len(), 2);
        assert!(graph.validate().is_empty());
    }
    /// Include paths have a type, a plan field, and no producer. The graph
    /// cannot store one and no `build.fol` method adds one, so a C header can
    /// never become a tracked input. Still a characterization after M1: the
    /// resolved plan carries the field honestly empty rather than dropping it.
    /// M2 and the C-import milestones give includes a route.
    #[test]
    fn include_paths_have_no_route_from_the_graph_into_a_plan() {
        let mut graph = BuildGraph::new();
        let artifact = graph.add_artifact(BuildArtifactKind::Executable, "app");
        graph.add_artifact_system_library(
            artifact,
            &crate::native::SystemLibraryRequest {
                name: "ssl".to_string(),
                mode: crate::native::NativeLinkMode::Dynamic,
                framework: false,
                search_path: Some("/usr/include".to_string()),
            },
        );

        let plans = crate::plan::resolve_graph_artifacts(
            &graph,
            &crate::plan::ResolvedProvenance::default(),
        );
        // The library search path reaches the plan; the include set cannot,
        // because nothing can populate it.
        assert!(!plans[0].native_attachments.library_paths.is_empty());
        assert!(plans[0].native_attachments.include_paths.is_empty());
    }
}

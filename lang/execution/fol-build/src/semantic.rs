#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStdlibModuleKind {
    Root,
    Types,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStdlibModulePath {
    pub package: String,
    pub module: String,
    pub kind: BuildStdlibModuleKind,
}

impl BuildStdlibModulePath {
    pub fn root() -> Self {
        Self {
            package: "fol/build".to_string(),
            module: "build".to_string(),
            kind: BuildStdlibModuleKind::Root,
        }
    }

    pub fn types() -> Self {
        Self {
            package: "fol/build".to_string(),
            module: "build/types".to_string(),
            kind: BuildStdlibModuleKind::Types,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStdlibImportSurface {
    pub canonical_import_alias: String,
    pub root_module: BuildStdlibModulePath,
}

impl BuildStdlibImportSurface {
    pub fn canonical() -> Self {
        Self {
            canonical_import_alias: "build".to_string(),
            root_module: BuildStdlibModulePath::root(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSemanticTypeFamily {
    BuildContext,
    Graph,
    ArtifactHandle,
    SystemLibraryHandle,
    ModuleHandle,
    StepHandle,
    RunHandle,
    InstallHandle,
    DependencyHandle,
    DependencyModuleHandle,
    DependencyArtifactHandle,
    DependencyStepHandle,
    DependencyGeneratedOutputHandle,
    GeneratedFileHandle,
    SourceFileHandle,
    SourceDirHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSemanticType {
    pub module: BuildStdlibModulePath,
    pub name: String,
    pub family: BuildSemanticTypeFamily,
}

impl BuildSemanticType {
    pub fn build_context() -> Self {
        Self {
            module: BuildStdlibModulePath::root(),
            name: "BuildContext".to_string(),
            family: BuildSemanticTypeFamily::BuildContext,
        }
    }

    pub fn graph() -> Self {
        Self {
            module: BuildStdlibModulePath::root(),
            name: "Graph".to_string(),
            family: BuildSemanticTypeFamily::Graph,
        }
    }

    pub fn artifact_handle() -> Self {
        Self::types_named("Artifact", BuildSemanticTypeFamily::ArtifactHandle)
    }

    pub fn system_library_handle() -> Self {
        Self::types_named(
            "SystemLibrary",
            BuildSemanticTypeFamily::SystemLibraryHandle,
        )
    }

    pub fn step_handle() -> Self {
        Self::types_named("Step", BuildSemanticTypeFamily::StepHandle)
    }

    pub fn run_handle() -> Self {
        Self::types_named("Run", BuildSemanticTypeFamily::RunHandle)
    }

    pub fn install_handle() -> Self {
        Self::types_named("Install", BuildSemanticTypeFamily::InstallHandle)
    }

    pub fn dependency_handle() -> Self {
        Self::types_named("Dependency", BuildSemanticTypeFamily::DependencyHandle)
    }

    pub fn dependency_module_handle() -> Self {
        Self::types_named(
            "DependencyModule",
            BuildSemanticTypeFamily::DependencyModuleHandle,
        )
    }

    pub fn dependency_artifact_handle() -> Self {
        Self::types_named(
            "DependencyArtifact",
            BuildSemanticTypeFamily::DependencyArtifactHandle,
        )
    }

    pub fn dependency_step_handle() -> Self {
        Self::types_named(
            "DependencyStep",
            BuildSemanticTypeFamily::DependencyStepHandle,
        )
    }

    pub fn dependency_generated_output_handle() -> Self {
        Self::types_named(
            "DependencyGeneratedOutput",
            BuildSemanticTypeFamily::DependencyGeneratedOutputHandle,
        )
    }

    pub fn generated_file_handle() -> Self {
        Self::types_named(
            "GeneratedFile",
            BuildSemanticTypeFamily::GeneratedFileHandle,
        )
    }

    pub fn source_file_handle() -> Self {
        Self::types_named("SourceFile", BuildSemanticTypeFamily::SourceFileHandle)
    }

    pub fn source_dir_handle() -> Self {
        Self::types_named("SourceDir", BuildSemanticTypeFamily::SourceDirHandle)
    }

    pub fn module_handle() -> Self {
        Self::types_named("Module", BuildSemanticTypeFamily::ModuleHandle)
    }

    fn types_named(name: &str, family: BuildSemanticTypeFamily) -> Self {
        Self {
            module: BuildStdlibModulePath::types(),
            name: name.to_string(),
            family,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSemanticParameterShape {
    Scalar,
    Record,
    Handle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSemanticMethodParameter {
    pub name: String,
    pub shape: BuildSemanticParameterShape,
    pub value_type: Option<BuildSemanticTypeFamily>,
    pub optional: bool,
    pub variadic: bool,
}

impl BuildSemanticMethodParameter {
    pub fn scalar(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            shape: BuildSemanticParameterShape::Scalar,
            value_type: None,
            optional: false,
            variadic: false,
        }
    }

    pub fn record(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            shape: BuildSemanticParameterShape::Record,
            value_type: None,
            optional: false,
            variadic: false,
        }
    }

    pub fn handle(name: impl Into<String>, family: BuildSemanticTypeFamily) -> Self {
        Self {
            name: name.into(),
            shape: BuildSemanticParameterShape::Handle,
            value_type: Some(family),
            optional: false,
            variadic: false,
        }
    }

    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub fn variadic(mut self) -> Self {
        self.variadic = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSemanticMethodSignature {
    pub receiver: BuildSemanticTypeFamily,
    pub name: String,
    pub params: Vec<BuildSemanticMethodParameter>,
    pub returns: Option<BuildSemanticTypeFamily>,
    pub chainable: bool,
}

impl BuildSemanticMethodSignature {
    pub fn new(receiver: BuildSemanticTypeFamily, name: impl Into<String>) -> Self {
        Self {
            receiver,
            name: name.into(),
            params: Vec::new(),
            returns: None,
            chainable: false,
        }
    }

    pub fn with_param(mut self, param: BuildSemanticMethodParameter) -> Self {
        self.params.push(param);
        self
    }

    pub fn returning(mut self, family: BuildSemanticTypeFamily) -> Self {
        self.returns = Some(family);
        self
    }

    pub fn chainable(mut self) -> Self {
        self.chainable = true;
        self
    }
}

pub fn canonical_graph_method_signatures() -> Vec<BuildSemanticMethodSignature> {
    vec![
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "standard_target")
            .with_param(BuildSemanticMethodParameter::record("config").optional()),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "standard_optimize")
            .with_param(BuildSemanticMethodParameter::record("config").optional()),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "option")
            .with_param(BuildSemanticMethodParameter::record("config")),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_exe")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::ArtifactHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_static_lib")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::ArtifactHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_shared_lib")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::ArtifactHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_test")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::ArtifactHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "step")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .with_param(BuildSemanticMethodParameter::scalar("description").optional())
            .with_param(
                BuildSemanticMethodParameter::handle(
                    "depends_on",
                    BuildSemanticTypeFamily::StepHandle,
                )
                .variadic(),
            )
            .returning(BuildSemanticTypeFamily::StepHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_run")
            .with_param(BuildSemanticMethodParameter::handle(
                "artifact",
                BuildSemanticTypeFamily::ArtifactHandle,
            ))
            .returning(BuildSemanticTypeFamily::RunHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "install")
            .with_param(BuildSemanticMethodParameter::handle(
                "artifact",
                BuildSemanticTypeFamily::ArtifactHandle,
            ))
            .returning(BuildSemanticTypeFamily::InstallHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "install_file")
            .with_param(BuildSemanticMethodParameter::scalar("path"))
            .returning(BuildSemanticTypeFamily::InstallHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "install_dir")
            .with_param(BuildSemanticMethodParameter::scalar("path"))
            .returning(BuildSemanticTypeFamily::InstallHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "write_file")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "copy_file")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_system_tool")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_system_tool_dir")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_system_lib")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::SystemLibraryHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_codegen")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_codegen_dir")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "dependency")
            .with_param(BuildSemanticMethodParameter::scalar("alias"))
            .with_param(BuildSemanticMethodParameter::scalar("package"))
            .returning(BuildSemanticTypeFamily::DependencyHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_module")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::ModuleHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "file_from_root")
            .with_param(BuildSemanticMethodParameter::scalar("subpath"))
            .returning(BuildSemanticTypeFamily::SourceFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "dir_from_root")
            .with_param(BuildSemanticMethodParameter::scalar("subpath"))
            .returning(BuildSemanticTypeFamily::SourceDirHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "build_root"),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "install_prefix"),
    ]
}

pub fn canonical_build_context_method_signatures() -> Vec<BuildSemanticMethodSignature> {
    vec![
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "meta")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "add_dep")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::DependencyHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "export_module")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "export_artifact")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "export_step")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "export_file")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "export_dir")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "export_path")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "export_output")
            .with_param(BuildSemanticMethodParameter::record("config"))
            .returning(BuildSemanticTypeFamily::BuildContext)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::BuildContext, "graph")
            .returning(BuildSemanticTypeFamily::Graph),
    ]
}

pub fn canonical_handle_method_signatures() -> Vec<BuildSemanticMethodSignature> {
    vec![
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::StepHandle, "depend_on")
            .with_param(BuildSemanticMethodParameter::handle(
                "step",
                BuildSemanticTypeFamily::StepHandle,
            ))
            .returning(BuildSemanticTypeFamily::StepHandle)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::RunHandle, "depend_on")
            .with_param(BuildSemanticMethodParameter::handle(
                "step",
                BuildSemanticTypeFamily::StepHandle,
            ))
            .returning(BuildSemanticTypeFamily::RunHandle)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::InstallHandle, "depend_on")
            .with_param(BuildSemanticMethodParameter::handle(
                "step",
                BuildSemanticTypeFamily::StepHandle,
            ))
            .returning(BuildSemanticTypeFamily::InstallHandle)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::DependencyHandle, "module")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .returning(BuildSemanticTypeFamily::DependencyModuleHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::DependencyHandle, "artifact")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .returning(BuildSemanticTypeFamily::DependencyArtifactHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::DependencyHandle, "step")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .returning(BuildSemanticTypeFamily::DependencyStepHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::DependencyHandle, "file")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .returning(BuildSemanticTypeFamily::SourceFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::DependencyHandle, "dir")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .returning(BuildSemanticTypeFamily::SourceDirHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::DependencyHandle, "path")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::DependencyHandle, "generated")
            .with_param(BuildSemanticMethodParameter::scalar("name"))
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        // Artifact handle methods
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::ArtifactHandle, "link")
            .with_param(BuildSemanticMethodParameter::handle(
                "dep_artifact",
                BuildSemanticTypeFamily::ArtifactHandle,
            )),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::ArtifactHandle, "link")
            .with_param(BuildSemanticMethodParameter::handle(
                "system_lib",
                BuildSemanticTypeFamily::SystemLibraryHandle,
            )),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::ArtifactHandle, "import")
            .with_param(BuildSemanticMethodParameter::handle(
                "dep_module",
                BuildSemanticTypeFamily::ModuleHandle,
            )),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::ArtifactHandle, "add_generated")
            .with_param(BuildSemanticMethodParameter::handle(
                "gen_file",
                BuildSemanticTypeFamily::GeneratedFileHandle,
            )),
        // Run handle methods
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::RunHandle, "add_arg")
            .with_param(BuildSemanticMethodParameter::scalar("value"))
            .returning(BuildSemanticTypeFamily::RunHandle)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::RunHandle, "add_file_arg")
            .with_param(BuildSemanticMethodParameter::handle(
                "gen_file",
                BuildSemanticTypeFamily::GeneratedFileHandle,
            ))
            .returning(BuildSemanticTypeFamily::RunHandle)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::RunHandle, "add_dir_arg")
            .with_param(BuildSemanticMethodParameter::scalar("path"))
            .returning(BuildSemanticTypeFamily::RunHandle)
            .chainable(),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::RunHandle, "capture_stdout")
            .returning(BuildSemanticTypeFamily::GeneratedFileHandle),
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::RunHandle, "set_env")
            .with_param(BuildSemanticMethodParameter::scalar("key"))
            .with_param(BuildSemanticMethodParameter::scalar("value"))
            .returning(BuildSemanticTypeFamily::RunHandle)
            .chainable(),
        // Step handle methods
        BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::StepHandle, "attach")
            .with_param(BuildSemanticMethodParameter::handle(
                "gen_file",
                BuildSemanticTypeFamily::GeneratedFileHandle,
            ))
            .returning(BuildSemanticTypeFamily::StepHandle)
            .chainable(),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSemanticRecordShapeKind {
    BuildContextConfig,
    ArtifactConfig,
    OptionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSemanticRecordField {
    pub name: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSemanticRecordShape {
    pub name: String,
    pub kind: BuildSemanticRecordShapeKind,
    pub fields: Vec<BuildSemanticRecordField>,
}

impl BuildSemanticRecordShape {
    pub fn build_context(
        name: impl Into<String>,
        fields: impl IntoIterator<Item = BuildSemanticRecordField>,
    ) -> Self {
        Self {
            name: name.into(),
            kind: BuildSemanticRecordShapeKind::BuildContextConfig,
            fields: fields.into_iter().collect(),
        }
    }

    pub fn artifact(
        name: impl Into<String>,
        fields: impl IntoIterator<Item = BuildSemanticRecordField>,
    ) -> Self {
        Self {
            name: name.into(),
            kind: BuildSemanticRecordShapeKind::ArtifactConfig,
            fields: fields.into_iter().collect(),
        }
    }

    pub fn option(
        name: impl Into<String>,
        fields: impl IntoIterator<Item = BuildSemanticRecordField>,
    ) -> Self {
        Self {
            name: name.into(),
            kind: BuildSemanticRecordShapeKind::OptionConfig,
            fields: fields.into_iter().collect(),
        }
    }
}

impl BuildSemanticRecordField {
    pub fn required(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: true,
        }
    }

    pub fn optional(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required: false,
        }
    }
}

pub fn canonical_artifact_config_shapes() -> Vec<BuildSemanticRecordShape> {
    let base_fields = vec![
        BuildSemanticRecordField::required("name"),
        BuildSemanticRecordField::required("root"),
        BuildSemanticRecordField::optional("fol_model"),
        BuildSemanticRecordField::optional("target"),
        BuildSemanticRecordField::optional("optimize"),
    ];

    vec![
        BuildSemanticRecordShape::artifact("ExeConfig", base_fields.clone()),
        BuildSemanticRecordShape::artifact("StaticLibConfig", base_fields.clone()),
        BuildSemanticRecordShape::artifact("SharedLibConfig", base_fields.clone()),
        BuildSemanticRecordShape::artifact("TestConfig", base_fields),
    ]
}

pub fn canonical_build_context_config_shapes() -> Vec<BuildSemanticRecordShape> {
    vec![
        BuildSemanticRecordShape::build_context(
            "BuildMetaConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("version"),
                BuildSemanticRecordField::optional("kind"),
                BuildSemanticRecordField::optional("description"),
                BuildSemanticRecordField::optional("license"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildDependencyConfig",
            [
                BuildSemanticRecordField::required("alias"),
                BuildSemanticRecordField::required("source"),
                BuildSemanticRecordField::required("target"),
                BuildSemanticRecordField::optional("version"),
                BuildSemanticRecordField::optional("hash"),
                BuildSemanticRecordField::optional("mode"),
                BuildSemanticRecordField::optional("args"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildExportModuleConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("module"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildExportArtifactConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("artifact"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildExportStepConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("step"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildExportFileConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("file"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildExportDirConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("dir"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildExportPathConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("path"),
            ],
        ),
        BuildSemanticRecordShape::build_context(
            "BuildExportOutputConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("output"),
            ],
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSemanticOptionValueKind {
    Target,
    Optimize,
    Bool,
    Int,
    String,
    Enum,
    Path,
}

pub fn canonical_option_config_shapes() -> Vec<BuildSemanticRecordShape> {
    vec![
        BuildSemanticRecordShape::option(
            "StandardTargetConfig",
            [
                BuildSemanticRecordField::optional("name"),
                BuildSemanticRecordField::optional("default"),
            ],
        ),
        BuildSemanticRecordShape::option(
            "StandardOptimizeConfig",
            [
                BuildSemanticRecordField::optional("name"),
                BuildSemanticRecordField::optional("default"),
            ],
        ),
        BuildSemanticRecordShape::option(
            "UserOptionConfig",
            [
                BuildSemanticRecordField::required("name"),
                BuildSemanticRecordField::required("kind"),
                BuildSemanticRecordField::optional("default"),
            ],
        ),
    ]
}

pub fn canonical_option_value_kinds() -> Vec<BuildSemanticOptionValueKind> {
    vec![
        BuildSemanticOptionValueKind::Target,
        BuildSemanticOptionValueKind::Optimize,
        BuildSemanticOptionValueKind::Bool,
        BuildSemanticOptionValueKind::Int,
        BuildSemanticOptionValueKind::String,
        BuildSemanticOptionValueKind::Enum,
        BuildSemanticOptionValueKind::Path,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSemanticChainKind {
    StepDependency,
    RunDependency,
    InstallDependency,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSemanticChainMetadata {
    pub receiver: BuildSemanticTypeFamily,
    pub method: String,
    pub kind: BuildSemanticChainKind,
    pub carries_step_handle: bool,
}

pub fn canonical_chain_metadata() -> Vec<BuildSemanticChainMetadata> {
    vec![
        BuildSemanticChainMetadata {
            receiver: BuildSemanticTypeFamily::StepHandle,
            method: "depend_on".to_string(),
            kind: BuildSemanticChainKind::StepDependency,
            carries_step_handle: true,
        },
        BuildSemanticChainMetadata {
            receiver: BuildSemanticTypeFamily::RunHandle,
            method: "depend_on".to_string(),
            kind: BuildSemanticChainKind::RunDependency,
            carries_step_handle: true,
        },
        BuildSemanticChainMetadata {
            receiver: BuildSemanticTypeFamily::InstallHandle,
            method: "depend_on".to_string(),
            kind: BuildSemanticChainKind::InstallDependency,
            carries_step_handle: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_artifact_config_shapes, canonical_build_context_config_shapes,
        canonical_build_context_method_signatures, canonical_chain_metadata,
        canonical_graph_method_signatures, canonical_handle_method_signatures,
        canonical_option_config_shapes, canonical_option_value_kinds, BuildSemanticChainKind,
        BuildSemanticMethodParameter, BuildSemanticMethodSignature, BuildSemanticOptionValueKind,
        BuildSemanticParameterShape, BuildSemanticRecordShapeKind, BuildSemanticType,
        BuildSemanticTypeFamily, BuildStdlibImportSurface, BuildStdlibModuleKind,
        BuildStdlibModulePath,
    };

    #[test]
    fn build_stdlib_module_paths_keep_canonical_package_and_module_names() {
        let root = BuildStdlibModulePath::root();
        let types = BuildStdlibModulePath::types();

        assert_eq!(root.package, "fol/build");
        assert_eq!(root.module, "build");
        assert_eq!(root.kind, BuildStdlibModuleKind::Root);

        assert_eq!(types.package, "fol/build");
        assert_eq!(types.module, "build/types");
        assert_eq!(types.kind, BuildStdlibModuleKind::Types);
    }

    #[test]
    fn build_stdlib_import_surface_keeps_the_canonical_build_alias() {
        let surface = BuildStdlibImportSurface::canonical();

        assert_eq!(surface.canonical_import_alias, "build");
        assert_eq!(surface.root_module, BuildStdlibModulePath::root());
    }

    #[test]
    fn semantic_build_surface_types_keep_canonical_modules() {
        let build = BuildSemanticType::build_context();
        let graph = BuildSemanticType::graph();
        let artifact = BuildSemanticType::artifact_handle();
        let step = BuildSemanticType::step_handle();

        assert_eq!(build.module, BuildStdlibModulePath::root());
        assert_eq!(build.family, BuildSemanticTypeFamily::BuildContext);
        assert_eq!(graph.module, BuildStdlibModulePath::root());
        assert_eq!(graph.family, BuildSemanticTypeFamily::Graph);
        assert_eq!(artifact.module, BuildStdlibModulePath::types());
        assert_eq!(artifact.family, BuildSemanticTypeFamily::ArtifactHandle);
        assert_eq!(step.module, BuildStdlibModulePath::types());
        assert_eq!(step.family, BuildSemanticTypeFamily::StepHandle);
    }

    #[test]
    fn semantic_build_surface_handle_names_stay_stable() {
        assert_eq!(BuildSemanticType::build_context().name, "BuildContext");
        assert_eq!(BuildSemanticType::run_handle().name, "Run");
        assert_eq!(BuildSemanticType::install_handle().name, "Install");
        assert_eq!(BuildSemanticType::dependency_handle().name, "Dependency");
        assert_eq!(
            BuildSemanticType::dependency_module_handle().name,
            "DependencyModule"
        );
        assert_eq!(
            BuildSemanticType::dependency_artifact_handle().name,
            "DependencyArtifact"
        );
        assert_eq!(
            BuildSemanticType::dependency_step_handle().name,
            "DependencyStep"
        );
        assert_eq!(
            BuildSemanticType::dependency_generated_output_handle().name,
            "DependencyGeneratedOutput"
        );
        assert_eq!(
            BuildSemanticType::generated_file_handle().name,
            "GeneratedFile"
        );
        assert_eq!(BuildSemanticType::source_file_handle().name, "SourceFile");
        assert_eq!(BuildSemanticType::source_dir_handle().name, "SourceDir");
    }

    #[test]
    fn semantic_method_parameters_capture_scalar_record_and_handle_shapes() {
        let scalar = BuildSemanticMethodParameter::scalar("name");
        let record = BuildSemanticMethodParameter::record("config").optional();
        let handle = BuildSemanticMethodParameter::handle(
            "artifact",
            BuildSemanticTypeFamily::ArtifactHandle,
        )
        .variadic();

        assert_eq!(scalar.shape, BuildSemanticParameterShape::Scalar);
        assert_eq!(record.shape, BuildSemanticParameterShape::Record);
        assert!(record.optional);
        assert_eq!(handle.shape, BuildSemanticParameterShape::Handle);
        assert_eq!(
            handle.value_type,
            Some(BuildSemanticTypeFamily::ArtifactHandle)
        );
        assert!(handle.variadic);
    }

    #[test]
    fn semantic_method_signatures_keep_receiver_return_and_chainability() {
        let signature =
            BuildSemanticMethodSignature::new(BuildSemanticTypeFamily::Graph, "add_exe")
                .with_param(BuildSemanticMethodParameter::record("config"))
                .returning(BuildSemanticTypeFamily::ArtifactHandle)
                .chainable();

        assert_eq!(signature.receiver, BuildSemanticTypeFamily::Graph);
        assert_eq!(signature.name, "add_exe");
        assert_eq!(signature.params.len(), 1);
        assert_eq!(
            signature.returns,
            Some(BuildSemanticTypeFamily::ArtifactHandle)
        );
        assert!(signature.chainable);
    }

    #[test]
    fn canonical_graph_methods_cover_build_graph_entrypoints() {
        let signatures = canonical_graph_method_signatures();
        let names = signatures
            .iter()
            .map(|signature| signature.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"standard_target"));
        assert!(names.contains(&"standard_optimize"));
        assert!(names.contains(&"add_exe"));
        assert!(names.contains(&"step"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"copy_file"));
        assert!(names.contains(&"add_system_tool"));
        assert!(names.contains(&"add_system_tool_dir"));
        assert!(names.contains(&"add_system_lib"));
        assert!(names.contains(&"add_codegen"));
        assert!(names.contains(&"add_codegen_dir"));
        assert!(names.contains(&"dependency"));
        assert!(names.contains(&"file_from_root"));
        assert!(names.contains(&"dir_from_root"));
    }

    #[test]
    fn canonical_build_context_methods_cover_metadata_dependency_and_graph_access() {
        let signatures = canonical_build_context_method_signatures();
        let names = signatures
            .iter()
            .map(|signature| signature.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(signatures.len(), 10);
        assert!(names.contains(&"meta"));
        assert!(names.contains(&"add_dep"));
        assert!(names.contains(&"export_module"));
        assert!(names.contains(&"export_artifact"));
        assert!(names.contains(&"export_step"));
        assert!(names.contains(&"export_file"));
        assert!(names.contains(&"export_dir"));
        assert!(names.contains(&"export_path"));
        assert!(names.contains(&"export_output"));
        assert!(names.contains(&"graph"));
        assert_eq!(
            signatures
                .iter()
                .find(|signature| signature.name == "add_dep")
                .and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::DependencyHandle)
        );
        assert!(signatures
            .iter()
            .filter(|signature| signature.name.starts_with("export_") || signature.name == "meta")
            .all(|signature| signature.chainable));
    }

    #[test]
    fn canonical_build_context_graph_method_returns_graph_family() {
        let signatures = canonical_build_context_method_signatures();
        let graph = signatures
            .iter()
            .find(|signature| signature.name == "graph");

        assert_eq!(
            graph.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::Graph)
        );
    }

    #[test]
    fn canonical_build_context_config_shapes_cover_meta_and_dependency_records() {
        let shapes = canonical_build_context_config_shapes();
        let names = shapes
            .iter()
            .map(|shape| shape.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(shapes.len(), 9);
        assert!(names.contains(&"BuildMetaConfig"));
        assert!(names.contains(&"BuildDependencyConfig"));
        assert!(names.contains(&"BuildExportModuleConfig"));
        assert!(names.contains(&"BuildExportArtifactConfig"));
        assert!(names.contains(&"BuildExportStepConfig"));
        assert!(names.contains(&"BuildExportFileConfig"));
        assert!(names.contains(&"BuildExportDirConfig"));
        assert!(names.contains(&"BuildExportPathConfig"));
        assert!(names.contains(&"BuildExportOutputConfig"));
        assert!(shapes
            .iter()
            .all(|shape| shape.kind == BuildSemanticRecordShapeKind::BuildContextConfig));
    }

    #[test]
    fn canonical_meta_config_requires_only_name_and_version() {
        let shapes = canonical_build_context_config_shapes();
        let meta = shapes
            .iter()
            .find(|shape| shape.name == "BuildMetaConfig")
            .expect("meta config should exist");

        let required = meta
            .fields
            .iter()
            .filter(|field| field.required)
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(required, vec!["name", "version"]);
        assert!(meta
            .fields
            .iter()
            .any(|field| field.name == "kind" && !field.required));
        assert!(meta
            .fields
            .iter()
            .any(|field| field.name == "description" && !field.required));
        assert!(meta
            .fields
            .iter()
            .any(|field| field.name == "license" && !field.required));
    }

    #[test]
    fn canonical_build_context_surface_keeps_receiver_param_and_shape_contracts() {
        let methods = canonical_build_context_method_signatures();
        let shapes = canonical_build_context_config_shapes();
        let meta = methods
            .iter()
            .find(|signature| signature.name == "meta")
            .unwrap();
        let add_dep = methods
            .iter()
            .find(|signature| signature.name == "add_dep")
            .unwrap();
        let export_module = methods
            .iter()
            .find(|signature| signature.name == "export_module")
            .unwrap();
        let graph = methods
            .iter()
            .find(|signature| signature.name == "graph")
            .unwrap();

        assert!(methods
            .iter()
            .all(|signature| signature.receiver == BuildSemanticTypeFamily::BuildContext));
        assert_eq!(meta.params.len(), 1);
        assert_eq!(meta.params[0].shape, BuildSemanticParameterShape::Record);
        assert_eq!(meta.returns, Some(BuildSemanticTypeFamily::BuildContext));
        assert_eq!(add_dep.params.len(), 1);
        assert_eq!(add_dep.params[0].shape, BuildSemanticParameterShape::Record);
        assert_eq!(export_module.params.len(), 1);
        assert_eq!(
            export_module.params[0].shape,
            BuildSemanticParameterShape::Record
        );
        assert!(graph.params.is_empty());
        assert_eq!(graph.returns, Some(BuildSemanticTypeFamily::Graph));
        assert!(shapes.iter().any(|shape| shape.name == "BuildMetaConfig"));
        assert!(shapes
            .iter()
            .any(|shape| shape.name == "BuildDependencyConfig"));
        assert!(shapes
            .iter()
            .any(|shape| shape.name == "BuildExportModuleConfig"));
        assert!(shapes
            .iter()
            .any(|shape| shape.name == "BuildExportFileConfig"));
    }

    #[test]
    fn canonical_graph_methods_return_expected_handle_families() {
        let signatures = canonical_graph_method_signatures();
        let add_exe = signatures
            .iter()
            .find(|signature| signature.name == "add_exe");
        let add_run = signatures
            .iter()
            .find(|signature| signature.name == "add_run");
        let install = signatures
            .iter()
            .find(|signature| signature.name == "install");
        let write_file = signatures
            .iter()
            .find(|signature| signature.name == "write_file");
        let add_codegen = signatures
            .iter()
            .find(|signature| signature.name == "add_codegen");
        let file_from_root = signatures
            .iter()
            .find(|signature| signature.name == "file_from_root");
        let dir_from_root = signatures
            .iter()
            .find(|signature| signature.name == "dir_from_root");

        assert_eq!(
            add_exe.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::ArtifactHandle)
        );
        assert_eq!(
            add_run.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::RunHandle)
        );
        assert_eq!(
            install.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::InstallHandle)
        );
        assert_eq!(
            write_file.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::GeneratedFileHandle)
        );
        assert_eq!(
            add_codegen.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::GeneratedFileHandle)
        );
        assert_eq!(
            file_from_root.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::SourceFileHandle)
        );
        assert_eq!(
            dir_from_root.and_then(|signature| signature.returns),
            Some(BuildSemanticTypeFamily::SourceDirHandle)
        );
    }

    #[test]
    fn canonical_handle_methods_cover_depend_on_chains() {
        let signatures = canonical_handle_method_signatures();
        let depend_on = signatures
            .iter()
            .filter(|signature| signature.name == "depend_on")
            .collect::<Vec<_>>();

        assert_eq!(depend_on.len(), 3);
        assert!(depend_on.iter().all(|signature| signature.chainable));
        assert!(signatures
            .iter()
            .any(|signature| signature.name == "module"));
        assert!(signatures
            .iter()
            .any(|signature| signature.name == "artifact"));
        assert!(signatures.iter().any(|signature| signature.name == "step"));
        assert!(signatures
            .iter()
            .any(|signature| signature.name == "generated"));
    }

    #[test]
    fn canonical_handle_methods_preserve_receiver_specific_returns() {
        let signatures = canonical_handle_method_signatures();
        let step = signatures
            .iter()
            .find(|signature| signature.receiver == BuildSemanticTypeFamily::StepHandle)
            .expect("step handle signature should exist");
        let run = signatures
            .iter()
            .find(|signature| signature.receiver == BuildSemanticTypeFamily::RunHandle)
            .expect("run handle signature should exist");
        let install = signatures
            .iter()
            .find(|signature| signature.receiver == BuildSemanticTypeFamily::InstallHandle)
            .expect("install handle signature should exist");
        let dependency_module = signatures
            .iter()
            .find(|signature| {
                signature.receiver == BuildSemanticTypeFamily::DependencyHandle
                    && signature.name == "module"
            })
            .expect("dependency module signature should exist");
        let dependency_generated = signatures
            .iter()
            .find(|signature| {
                signature.receiver == BuildSemanticTypeFamily::DependencyHandle
                    && signature.name == "generated"
            })
            .expect("dependency generated signature should exist");
        let dependency_file = signatures
            .iter()
            .find(|signature| {
                signature.receiver == BuildSemanticTypeFamily::DependencyHandle
                    && signature.name == "file"
            })
            .expect("dependency file signature should exist");
        let dependency_dir = signatures
            .iter()
            .find(|signature| {
                signature.receiver == BuildSemanticTypeFamily::DependencyHandle
                    && signature.name == "dir"
            })
            .expect("dependency dir signature should exist");
        let dependency_path = signatures
            .iter()
            .find(|signature| {
                signature.receiver == BuildSemanticTypeFamily::DependencyHandle
                    && signature.name == "path"
            })
            .expect("dependency path signature should exist");

        assert_eq!(step.returns, Some(BuildSemanticTypeFamily::StepHandle));
        assert_eq!(run.returns, Some(BuildSemanticTypeFamily::RunHandle));
        assert_eq!(
            install.returns,
            Some(BuildSemanticTypeFamily::InstallHandle)
        );
        assert_eq!(
            dependency_module.returns,
            Some(BuildSemanticTypeFamily::DependencyModuleHandle)
        );
        assert_eq!(
            dependency_generated.returns,
            Some(BuildSemanticTypeFamily::GeneratedFileHandle)
        );
        assert_eq!(
            dependency_file.returns,
            Some(BuildSemanticTypeFamily::SourceFileHandle)
        );
        assert_eq!(
            dependency_dir.returns,
            Some(BuildSemanticTypeFamily::SourceDirHandle)
        );
        assert_eq!(
            dependency_path.returns,
            Some(BuildSemanticTypeFamily::GeneratedFileHandle)
        );
    }

    #[test]
    fn canonical_artifact_config_shapes_cover_all_primary_artifact_kinds() {
        let shapes = canonical_artifact_config_shapes();
        let names = shapes
            .iter()
            .map(|shape| shape.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"ExeConfig"));
        assert!(names.contains(&"StaticLibConfig"));
        assert!(names.contains(&"SharedLibConfig"));
        assert!(names.contains(&"TestConfig"));
        assert!(shapes
            .iter()
            .all(|shape| shape.kind == BuildSemanticRecordShapeKind::ArtifactConfig));
    }

    #[test]
    fn canonical_artifact_config_shapes_keep_required_name_and_root_fields() {
        let shapes = canonical_artifact_config_shapes();

        for shape in shapes {
            assert!(shape
                .fields
                .iter()
                .any(|field| field.name == "name" && field.required));
            assert!(shape
                .fields
                .iter()
                .any(|field| field.name == "root" && field.required));
        }
    }

    #[test]
    fn canonical_artifact_config_shapes_allow_optional_fol_model_field() {
        let shapes = canonical_artifact_config_shapes();

        for shape in shapes {
            assert!(shape
                .fields
                .iter()
                .any(|field| field.name == "fol_model" && !field.required));
        }
    }

    #[test]
    fn canonical_option_config_shapes_cover_standard_and_user_option_forms() {
        let shapes = canonical_option_config_shapes();
        let names = shapes
            .iter()
            .map(|shape| shape.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"StandardTargetConfig"));
        assert!(names.contains(&"StandardOptimizeConfig"));
        assert!(names.contains(&"UserOptionConfig"));
        assert!(shapes
            .iter()
            .all(|shape| shape.kind == BuildSemanticRecordShapeKind::OptionConfig));
    }

    #[test]
    fn canonical_option_value_kinds_cover_all_current_build_option_families() {
        let kinds = canonical_option_value_kinds();

        assert!(kinds.contains(&BuildSemanticOptionValueKind::Target));
        assert!(kinds.contains(&BuildSemanticOptionValueKind::Optimize));
        assert!(kinds.contains(&BuildSemanticOptionValueKind::Bool));
        assert!(kinds.contains(&BuildSemanticOptionValueKind::Int));
        assert!(kinds.contains(&BuildSemanticOptionValueKind::String));
        assert!(kinds.contains(&BuildSemanticOptionValueKind::Enum));
        assert!(kinds.contains(&BuildSemanticOptionValueKind::Path));
    }

    #[test]
    fn canonical_chain_metadata_covers_depend_on_receivers() {
        let metadata = canonical_chain_metadata();

        assert_eq!(metadata.len(), 3);
        assert!(metadata.iter().all(|entry| entry.method == "depend_on"));
        assert!(metadata.iter().all(|entry| entry.carries_step_handle));
    }

    #[test]
    fn canonical_chain_metadata_distinguishes_chain_kinds_per_receiver() {
        let metadata = canonical_chain_metadata();

        assert_eq!(metadata[0].receiver, BuildSemanticTypeFamily::StepHandle);
        assert_eq!(metadata[0].kind, BuildSemanticChainKind::StepDependency);
        assert_eq!(metadata[1].receiver, BuildSemanticTypeFamily::RunHandle);
        assert_eq!(metadata[1].kind, BuildSemanticChainKind::RunDependency);
        assert_eq!(metadata[2].receiver, BuildSemanticTypeFamily::InstallHandle);
        assert_eq!(metadata[2].kind, BuildSemanticChainKind::InstallDependency);
    }
}

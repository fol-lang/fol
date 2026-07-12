use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFileDefinition {
    pub name: String,
    pub relative_path: String,
    pub action: GeneratedFileAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedFileAction {
    Write {
        contents: String,
    },
    Copy {
        source_path: String,
    },
    CaptureToolOutput {
        tool: String,
        args: Vec<String>,
        file_args: Vec<String>,
        env: BTreeMap<String, String>,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedFileSet {
    definitions: Vec<GeneratedFileDefinition>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GeneratedOutputDependencySet {
    outputs: Vec<GeneratedFileDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFileInstallProjection {
    pub generated_file_name: String,
    pub install_name: String,
    pub install_path: String,
}

impl GeneratedFileInstallProjection {
    pub fn new(
        generated_file_name: impl Into<String>,
        install_name: impl Into<String>,
        install_path: impl Into<String>,
    ) -> Self {
        Self {
            generated_file_name: generated_file_name.into(),
            install_name: install_name.into(),
            install_path: install_path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemToolRequest {
    pub tool: String,
    pub args: Vec<String>,
    pub file_args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemToolResult {
    pub tool: String,
    pub exit_status: i32,
    pub generated_outputs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenKind {
    FolToFol,
    Schema,
    AssetPreprocess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenRequest {
    pub kind: CodegenKind,
    pub input: String,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenResult {
    pub kind: CodegenKind,
    pub output: String,
}

impl GeneratedFileSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn definitions(&self) -> &[GeneratedFileDefinition] {
        &self.definitions
    }

    pub fn add(&mut self, definition: GeneratedFileDefinition) {
        self.definitions.push(definition);
    }
}

impl GeneratedOutputDependencySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, definition: GeneratedFileDefinition) {
        self.outputs.push(definition);
    }

    pub fn get(&self, name: &str) -> Option<&GeneratedFileDefinition> {
        self.outputs
            .iter()
            .find(|definition| definition.name == name)
    }

    pub fn outputs(&self) -> &[GeneratedFileDefinition] {
        &self.outputs
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CodegenKind, CodegenRequest, CodegenResult, GeneratedFileAction, GeneratedFileDefinition,
        GeneratedFileInstallProjection, GeneratedFileSet, GeneratedOutputDependencySet,
        SystemToolRequest, SystemToolResult,
    };
    use std::collections::BTreeMap;

    #[test]
    fn generated_file_set_starts_empty() {
        let set = GeneratedFileSet::new();

        assert!(set.definitions().is_empty());
    }

    #[test]
    fn generated_file_set_preserves_inserted_shell_definitions() {
        let mut set = GeneratedFileSet::new();
        set.add(GeneratedFileDefinition {
            name: "version".to_string(),
            relative_path: "gen/version.fol".to_string(),
            action: GeneratedFileAction::Write {
                contents: "let version = \"0.1.0\"".to_string(),
            },
        });

        assert_eq!(set.definitions().len(), 1);
        assert_eq!(set.definitions()[0].name, "version");
        assert_eq!(set.definitions()[0].relative_path, "gen/version.fol");
        assert!(matches!(
            set.definitions()[0].action,
            GeneratedFileAction::Write { .. }
        ));
    }

    #[test]
    fn generated_file_actions_cover_write_copy_and_captured_outputs() {
        let write = GeneratedFileAction::Write {
            contents: "hello".to_string(),
        };
        let copy = GeneratedFileAction::Copy {
            source_path: "assets/logo.svg".to_string(),
        };
        let capture = GeneratedFileAction::CaptureToolOutput {
            tool: "schema-gen".to_string(),
            args: vec!["api.yaml".to_string()],
            file_args: vec!["schema/api.yaml".to_string()],
            env: BTreeMap::from([("MODE".to_string(), "strict".to_string())]),
        };

        assert!(matches!(write, GeneratedFileAction::Write { .. }));
        assert!(matches!(copy, GeneratedFileAction::Copy { .. }));
        assert!(matches!(
            capture,
            GeneratedFileAction::CaptureToolOutput { .. }
        ));
    }

    #[test]
    fn generated_file_install_projection_keeps_install_helper_metadata() {
        let projection =
            GeneratedFileInstallProjection::new("config", "install-config", "share/config.json");

        assert_eq!(projection.generated_file_name, "config");
        assert_eq!(projection.install_name, "install-config");
        assert_eq!(projection.install_path, "share/config.json");
    }

    #[test]
    fn system_tool_models_keep_requests_and_results_stable() {
        let request = SystemToolRequest {
            tool: "flatc".to_string(),
            args: vec!["--fol".to_string(), "schema.fbs".to_string()],
            file_args: vec!["schema/api.fbs".to_string()],
            env: BTreeMap::from([("FLAVOR".to_string(), "strict".to_string())]),
            outputs: vec!["gen/schema.fol".to_string()],
        };
        let result = SystemToolResult {
            tool: "flatc".to_string(),
            exit_status: 0,
            generated_outputs: vec!["gen/schema.fol".to_string()],
        };

        assert_eq!(request.tool, "flatc");
        assert_eq!(request.file_args, vec!["schema/api.fbs".to_string()]);
        assert_eq!(
            request.env.get("FLAVOR").map(String::as_str),
            Some("strict")
        );
        assert_eq!(request.outputs, vec!["gen/schema.fol".to_string()]);
        assert_eq!(result.exit_status, 0);
        assert_eq!(result.generated_outputs.len(), 1);
    }

    #[test]
    fn codegen_models_cover_fol_schema_and_asset_flows() {
        let fol = CodegenRequest {
            kind: CodegenKind::FolToFol,
            input: "schema/source.fol".to_string(),
            output: "gen/source.fol".to_string(),
        };
        let schema = CodegenRequest {
            kind: CodegenKind::Schema,
            input: "schema/api.yaml".to_string(),
            output: "gen/api.fol".to_string(),
        };
        let asset = CodegenResult {
            kind: CodegenKind::AssetPreprocess,
            output: "gen/logo.bin".to_string(),
        };

        assert!(matches!(fol.kind, CodegenKind::FolToFol));
        assert!(matches!(schema.kind, CodegenKind::Schema));
        assert!(matches!(asset.kind, CodegenKind::AssetPreprocess));
        assert_eq!(asset.output, "gen/logo.bin");
    }

    #[test]
    fn generated_output_dependency_set_supports_named_lookup() {
        let mut outputs = GeneratedOutputDependencySet::new();
        outputs.add(GeneratedFileDefinition {
            name: "bindings".to_string(),
            relative_path: "gen/bindings.fol".to_string(),
            action: GeneratedFileAction::Write {
                contents: "generated".to_string(),
            },
        });

        assert_eq!(outputs.outputs().len(), 1);
        assert_eq!(
            outputs
                .get("bindings")
                .map(|definition| definition.relative_path.as_str()),
            Some("gen/bindings.fol")
        );
        assert!(outputs.get("missing").is_none());
    }
}

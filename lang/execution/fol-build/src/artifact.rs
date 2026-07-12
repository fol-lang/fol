#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildArtifactModelKind {
    Executable,
    StaticLibrary,
    SharedLibrary,
    TestBundle,
    GeneratedSourceBundle,
    DocsBundle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildArtifactLinkage {
    Executable,
    Static,
    Shared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuildArtifactFolModel {
    Core,
    #[default]
    Memo,
}

impl BuildArtifactFolModel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Memo => "memo",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "core" => Some(Self::Core),
            "memo" => Some(Self::Memo),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifactRootSource {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifactModuleConfig {
    pub roots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifactTargetConfig {
    pub fol_model: BuildArtifactFolModel,
    pub target: Option<String>,
    pub optimize: Option<String>,
}

impl BuildArtifactTargetConfig {
    pub fn apply_resolved_options(&self, resolved: &ResolvedBuildOptionSet) -> Self {
        Self {
            fol_model: self.fol_model,
            target: resolved
                .get("target")
                .map(str::to_string)
                .or_else(|| self.target.clone()),
            optimize: resolved
                .get("optimize")
                .map(str::to_string)
                .or_else(|| self.optimize.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildArtifactOutput {
    EmittedRustCrate { crate_root: String },
    Binary { binary_path: String },
    GeneratedSourceBundle { root: String },
    DocsBundle { root: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifactReport {
    pub artifact_name: String,
    pub output: BuildArtifactOutput,
}

impl BuildArtifactReport {
    pub fn summary(&self) -> String {
        match &self.output {
            BuildArtifactOutput::EmittedRustCrate { crate_root } => {
                format!("emitted-rust:{} root={crate_root}", self.artifact_name)
            }
            BuildArtifactOutput::Binary { binary_path } => {
                format!("binary:{} path={binary_path}", self.artifact_name)
            }
            BuildArtifactOutput::GeneratedSourceBundle { root } => {
                format!("generated:{} root={root}", self.artifact_name)
            }
            BuildArtifactOutput::DocsBundle { root } => {
                format!("docs:{} root={root}", self.artifact_name)
            }
        }
    }

    pub fn primary_path(&self) -> &str {
        match &self.output {
            BuildArtifactOutput::EmittedRustCrate { crate_root } => crate_root,
            BuildArtifactOutput::Binary { binary_path } => binary_path,
            BuildArtifactOutput::GeneratedSourceBundle { root } => root,
            BuildArtifactOutput::DocsBundle { root } => root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifactPipelinePlan {
    pub definition: BuildArtifactDefinition,
    pub stages: Vec<BuildArtifactPipelineStage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildArtifactPipelineStage {
    Package,
    Resolver,
    Typecheck,
    Lower,
    Backend,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildArtifactNativeAttachmentSet {
    pub include_paths: Vec<crate::native::NativeIncludePath>,
    pub library_paths: Vec<crate::native::NativeLibraryPath>,
    pub link_inputs: Vec<crate::native::NativeLinkDirective>,
}

pub fn project_graph_artifacts(graph: &BuildGraph) -> Vec<BuildArtifactDefinition> {
    graph
        .artifacts()
        .iter()
        .map(|artifact| BuildArtifactDefinition {
            name: artifact.name.clone(),
            kind: match artifact.kind {
                BuildArtifactKind::Executable => BuildArtifactModelKind::Executable,
                BuildArtifactKind::StaticLibrary => BuildArtifactModelKind::StaticLibrary,
                BuildArtifactKind::SharedLibrary => BuildArtifactModelKind::SharedLibrary,
                BuildArtifactKind::Object => BuildArtifactModelKind::TestBundle,
            },
            root_source: BuildArtifactRootSource {
                path: graph
                    .artifact_inputs_for(artifact.id)
                    .find_map(|input| match input {
                        crate::graph::BuildArtifactInput::Module(module_id) => graph
                            .modules()
                            .get(module_id.index())
                            .map(|module| module.name.clone()),
                        crate::graph::BuildArtifactInput::GeneratedFile(_) => None,
                    })
                    .unwrap_or_default(),
            },
            modules: BuildArtifactModuleConfig {
                roots: graph
                    .artifact_inputs_for(artifact.id)
                    .filter_map(|input| match input {
                        crate::graph::BuildArtifactInput::Module(module_id) => graph
                            .modules()
                            .get(module_id.index())
                            .map(|module| module.name.clone()),
                        crate::graph::BuildArtifactInput::GeneratedFile(_) => None,
                    })
                    .collect(),
            },
            output_name: artifact.name.clone(),
            linkage: match artifact.kind {
                BuildArtifactKind::Executable => BuildArtifactLinkage::Executable,
                BuildArtifactKind::StaticLibrary => BuildArtifactLinkage::Static,
                BuildArtifactKind::SharedLibrary | BuildArtifactKind::Object => {
                    BuildArtifactLinkage::Shared
                }
            },
            target: BuildArtifactTargetConfig {
                fol_model: BuildArtifactFolModel::default(),
                target: None,
                optimize: None,
            },
            native_attachments: BuildArtifactNativeAttachmentSet::default(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildArtifactDefinition {
    pub name: String,
    pub kind: BuildArtifactModelKind,
    pub root_source: BuildArtifactRootSource,
    pub modules: BuildArtifactModuleConfig,
    pub output_name: String,
    pub linkage: BuildArtifactLinkage,
    pub target: BuildArtifactTargetConfig,
    pub native_attachments: BuildArtifactNativeAttachmentSet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildArtifactSet {
    definitions: Vec<BuildArtifactDefinition>,
}

impl BuildArtifactSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn definitions(&self) -> &[BuildArtifactDefinition] {
        &self.definitions
    }

    pub fn add_definition(&mut self, definition: BuildArtifactDefinition) {
        self.definitions.push(definition);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        project_graph_artifacts, BuildArtifactDefinition, BuildArtifactFolModel,
        BuildArtifactLinkage,
        BuildArtifactModelKind, BuildArtifactModuleConfig, BuildArtifactNativeAttachmentSet,
        BuildArtifactOutput, BuildArtifactPipelinePlan, BuildArtifactPipelineStage,
        BuildArtifactReport, BuildArtifactRootSource, BuildArtifactSet, BuildArtifactTargetConfig,
    };
    use crate::graph::{BuildArtifactKind, BuildGraph, BuildModuleKind};
    use crate::native::{
        NativeArtifactDefinition, NativeArtifactKind, NativeIncludePath, NativeLibraryPath,
        NativeLinkDirective, NativeLinkInput, NativeLinkMode, NativeSearchPathOrigin,
    };
    use crate::option::ResolvedBuildOptionSet;

    #[test]
    fn build_artifact_set_starts_empty() {
        let set = BuildArtifactSet::new();

        assert!(set.definitions().is_empty());
    }

    #[test]
    fn build_artifact_set_preserves_inserted_definitions() {
        let mut set = BuildArtifactSet::new();
        set.add_definition(BuildArtifactDefinition {
            name: "app".to_string(),
            kind: BuildArtifactModelKind::Executable,
            root_source: BuildArtifactRootSource {
                path: "src/main.fol".to_string(),
            },
            modules: BuildArtifactModuleConfig {
                roots: vec!["src".to_string()],
            },
            output_name: "app".to_string(),
            linkage: BuildArtifactLinkage::Executable,
            target: BuildArtifactTargetConfig {
                fol_model: BuildArtifactFolModel::Memo,
                target: None,
                optimize: None,
            },
            native_attachments: BuildArtifactNativeAttachmentSet::default(),
        });

        assert_eq!(set.definitions()[0].name, "app");
    }

    #[test]
    fn artifact_model_kinds_cover_phase_five_bundle_shapes() {
        assert_eq!(
            BuildArtifactModelKind::GeneratedSourceBundle,
            BuildArtifactModelKind::GeneratedSourceBundle
        );
        assert_eq!(
            BuildArtifactModelKind::DocsBundle,
            BuildArtifactModelKind::DocsBundle
        );
        assert_eq!(
            BuildArtifactModelKind::TestBundle,
            BuildArtifactModelKind::TestBundle
        );
    }

    #[test]
    fn artifact_definitions_keep_root_module_output_and_linkage_config() {
        let definition = BuildArtifactDefinition {
            name: "plugin".to_string(),
            kind: BuildArtifactModelKind::SharedLibrary,
            root_source: BuildArtifactRootSource {
                path: "src/plugin.fol".to_string(),
            },
            modules: BuildArtifactModuleConfig {
                roots: vec!["src".to_string(), "generated".to_string()],
            },
            output_name: "fol_plugin".to_string(),
            linkage: BuildArtifactLinkage::Shared,
            target: BuildArtifactTargetConfig {
                fol_model: BuildArtifactFolModel::Memo,
                target: Some("x86_64-linux-gnu".to_string()),
                optimize: Some("release".to_string()),
            },
            native_attachments: BuildArtifactNativeAttachmentSet {
                include_paths: vec![NativeIncludePath {
                    origin: NativeSearchPathOrigin::PackageRoot,
                    relative_path: "include".to_string(),
                }],
                library_paths: vec![NativeLibraryPath {
                    origin: NativeSearchPathOrigin::BuildRoot,
                    relative_path: ".fol/build/native".to_string(),
                }],
                link_inputs: vec![
                    NativeLinkDirective {
                        input: NativeLinkInput::LibraryName("ssl".to_string()),
                        mode: NativeLinkMode::Dynamic,
                    },
                    NativeLinkDirective {
                        input: NativeLinkInput::Artifact(NativeArtifactDefinition {
                            name: "zlib".to_string(),
                            kind: NativeArtifactKind::StaticLibrary,
                            relative_path: "native/libz.a".to_string(),
                        }),
                        mode: NativeLinkMode::Static,
                    },
                ],
            },
        };

        assert_eq!(definition.root_source.path, "src/plugin.fol");
        assert_eq!(definition.modules.roots.len(), 2);
        assert_eq!(definition.output_name, "fol_plugin");
        assert_eq!(definition.linkage, BuildArtifactLinkage::Shared);
        assert_eq!(definition.target.fol_model, BuildArtifactFolModel::Memo);
        assert_eq!(
            definition.target.target.as_deref(),
            Some("x86_64-linux-gnu")
        );
        assert_eq!(definition.target.optimize.as_deref(), Some("release"));
        assert_eq!(definition.native_attachments.include_paths.len(), 1);
        assert_eq!(definition.native_attachments.library_paths.len(), 1);
        assert_eq!(definition.native_attachments.link_inputs.len(), 2);
    }

    #[test]
    fn artifact_target_config_applies_resolved_target_and_optimize_overrides() {
        let mut resolved = ResolvedBuildOptionSet::new();
        resolved.insert("target", "aarch64-macos-gnu");
        resolved.insert("optimize", "release-fast");

        let config = BuildArtifactTargetConfig {
            fol_model: BuildArtifactFolModel::Core,
            target: Some("x86_64-linux-gnu".to_string()),
            optimize: Some("debug".to_string()),
        }
        .apply_resolved_options(&resolved);

        assert_eq!(config.fol_model, BuildArtifactFolModel::Core);
        assert_eq!(config.target.as_deref(), Some("aarch64-macos-gnu"));
        assert_eq!(config.optimize.as_deref(), Some("release-fast"));
    }

    #[test]
    fn artifact_fol_models_parse_and_render_canonically() {
        assert_eq!(BuildArtifactFolModel::parse("core"), Some(BuildArtifactFolModel::Core));
        assert_eq!(
            BuildArtifactFolModel::parse("memo"),
            Some(BuildArtifactFolModel::Memo)
        );
        assert_eq!(BuildArtifactFolModel::parse("std"), None);
        assert_eq!(BuildArtifactFolModel::parse("alloc"), None);
        assert_eq!(BuildArtifactFolModel::parse("mem"), None);
        assert_eq!(BuildArtifactFolModel::parse("hosted"), None);
        assert_eq!(BuildArtifactFolModel::Core.as_str(), "core");
        assert_eq!(BuildArtifactFolModel::Memo.as_str(), "memo");
    }

    #[test]
    fn artifact_target_config_defaults_to_memo_model() {
        let config = BuildArtifactTargetConfig {
            fol_model: BuildArtifactFolModel::default(),
            target: None,
            optimize: None,
        };

        assert_eq!(config.fol_model, BuildArtifactFolModel::Memo);
    }

    #[test]
    fn artifact_reports_cover_backend_and_bundle_outputs() {
        let emitted = BuildArtifactReport {
            artifact_name: "app".to_string(),
            output: BuildArtifactOutput::EmittedRustCrate {
                crate_root: ".fol/build/emit/rust/app".to_string(),
            },
        };
        let binary = BuildArtifactReport {
            artifact_name: "app".to_string(),
            output: BuildArtifactOutput::Binary {
                binary_path: ".fol/build/debug/app".to_string(),
            },
        };
        let docs = BuildArtifactReport {
            artifact_name: "docs".to_string(),
            output: BuildArtifactOutput::DocsBundle {
                root: ".fol/build/docs".to_string(),
            },
        };

        match emitted.output {
            BuildArtifactOutput::EmittedRustCrate { crate_root } => {
                assert!(crate_root.contains("emit/rust"));
            }
            other => panic!("unexpected emitted output: {other:?}"),
        }
        match binary.output {
            BuildArtifactOutput::Binary { binary_path } => {
                assert!(binary_path.ends_with("/app"));
            }
            other => panic!("unexpected binary output: {other:?}"),
        }
        match docs.output {
            BuildArtifactOutput::DocsBundle { root } => {
                assert!(root.ends_with("docs"));
            }
            other => panic!("unexpected docs output: {other:?}"),
        }
    }

    #[test]
    fn artifact_report_summaries_keep_frontend_facing_words_and_paths() {
        let emitted = BuildArtifactReport {
            artifact_name: "app".to_string(),
            output: BuildArtifactOutput::EmittedRustCrate {
                crate_root: ".fol/build/emit/rust/app".to_string(),
            },
        };
        let binary = BuildArtifactReport {
            artifact_name: "app".to_string(),
            output: BuildArtifactOutput::Binary {
                binary_path: ".fol/build/debug/app".to_string(),
            },
        };

        assert!(emitted.summary().contains("emitted-rust:app"));
        assert!(emitted.summary().contains(".fol/build/emit/rust/app"));
        assert_eq!(emitted.primary_path(), ".fol/build/emit/rust/app");
        assert!(binary.summary().contains("binary:app"));
        assert_eq!(binary.primary_path(), ".fol/build/debug/app");
    }

    #[test]
    fn artifact_pipeline_plan_tracks_all_compiler_and_backend_stages() {
        let plan = BuildArtifactPipelinePlan {
            definition: BuildArtifactDefinition {
                name: "app".to_string(),
                kind: BuildArtifactModelKind::Executable,
                root_source: BuildArtifactRootSource {
                    path: "src/main.fol".to_string(),
                },
                modules: BuildArtifactModuleConfig {
                    roots: vec!["src".to_string()],
                },
                output_name: "app".to_string(),
                linkage: BuildArtifactLinkage::Executable,
                target: BuildArtifactTargetConfig {
                    fol_model: BuildArtifactFolModel::Memo,
                    target: Some("native".to_string()),
                    optimize: Some("debug".to_string()),
                },
                native_attachments: BuildArtifactNativeAttachmentSet::default(),
            },
            stages: vec![
                BuildArtifactPipelineStage::Package,
                BuildArtifactPipelineStage::Resolver,
                BuildArtifactPipelineStage::Typecheck,
                BuildArtifactPipelineStage::Lower,
                BuildArtifactPipelineStage::Backend,
            ],
        };

        assert_eq!(plan.definition.name, "app");
        assert_eq!(plan.stages.len(), 5);
        assert_eq!(plan.stages[0], BuildArtifactPipelineStage::Package);
        assert_eq!(plan.stages[4], BuildArtifactPipelineStage::Backend);
    }

    #[test]
    fn graph_artifact_projection_maps_build_graph_nodes_into_artifact_definitions() {
        let mut graph = BuildGraph::new();
        let main = graph.add_module(BuildModuleKind::Source, "src/main.fol");
        let exe = graph.add_artifact(BuildArtifactKind::Executable, "app");
        let lib = graph.add_artifact(BuildArtifactKind::StaticLibrary, "support");
        graph.add_artifact_module_input(exe, main);
        graph.add_artifact_module_input(lib, main);

        let projected = project_graph_artifacts(&graph);

        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].kind, BuildArtifactModelKind::Executable);
        assert_eq!(projected[0].root_source.path, "src/main.fol");
        assert_eq!(projected[1].kind, BuildArtifactModelKind::StaticLibrary);
        assert_eq!(projected[1].modules.roots, vec!["src/main.fol".to_string()]);
    }
}
use crate::graph::{BuildArtifactKind, BuildGraph};
use crate::option::ResolvedBuildOptionSet;

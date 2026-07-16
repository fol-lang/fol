#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyBuildSurface {
    pub alias: String,
    pub exposure: DependencyBuildExposure,
    pub modules: Vec<DependencyModuleSurface>,
    pub source_roots: Vec<DependencySourceRootSurface>,
    pub artifacts: Vec<DependencyArtifactSurface>,
    pub steps: Vec<DependencyStepSurface>,
    pub files: Vec<DependencyFileSurface>,
    pub dirs: Vec<DependencyDirSurface>,
    pub paths: Vec<DependencyPathSurface>,
    pub generated_outputs: Vec<DependencyGeneratedOutputSurface>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DependencyBuildExposure {
    pub modules_explicit: bool,
    pub artifacts_explicit: bool,
    pub steps_explicit: bool,
    pub files_explicit: bool,
    pub dirs_explicit: bool,
    pub paths_explicit: bool,
    pub generated_outputs_explicit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyBuildHandle {
    pub alias: String,
    pub package: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyBuildEvaluationMode {
    Eager,
    Lazy,
    OnDemand,
}

impl DependencyBuildEvaluationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eager => "eager",
            Self::Lazy => "lazy",
            Self::OnDemand => "on-demand",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "eager" => Some(Self::Eager),
            "lazy" => Some(Self::Lazy),
            "on-demand" => Some(Self::OnDemand),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyModuleSurfaceSet {
    pub modules: Vec<DependencyModuleSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyArtifactSurfaceSet {
    pub artifacts: Vec<DependencyArtifactSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyStepSurfaceSet {
    pub steps: Vec<DependencyStepSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyGeneratedOutputSurfaceSet {
    pub generated_outputs: Vec<DependencyGeneratedOutputSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyFileSurfaceSet {
    pub files: Vec<DependencyFileSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyDirSurfaceSet {
    pub dirs: Vec<DependencyDirSurface>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyPathSurfaceSet {
    pub paths: Vec<DependencyPathSurface>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyModuleSurface {
    pub name: String,
    pub source_namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencySourceRootSurface {
    pub relative_path: String,
    pub namespace_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyArtifactSurface {
    pub name: String,
    pub artifact_kind: String,
    pub fol_model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyStepSurface {
    pub name: String,
    pub step_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyGeneratedOutputSurface {
    pub name: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyFileSurface {
    pub name: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyDirSurface {
    pub name: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyPathSurface {
    pub name: String,
    pub relative_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyBuildSurfaceSet {
    surfaces: Vec<DependencyBuildSurface>,
}

impl DependencyBuildSurfaceSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn surfaces(&self) -> &[DependencyBuildSurface] {
        &self.surfaces
    }

    pub fn add(&mut self, surface: DependencyBuildSurface) {
        self.surfaces.push(surface);
    }

    pub fn find(&self, alias: &str) -> Option<&DependencyBuildSurface> {
        self.surfaces.iter().find(|surface| surface.alias == alias)
    }
}

impl DependencyBuildSurface {
    pub fn projected(alias: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            exposure: DependencyBuildExposure::default(),
            modules: Vec::new(),
            source_roots: Vec::new(),
            artifacts: Vec::new(),
            steps: Vec::new(),
            files: Vec::new(),
            dirs: Vec::new(),
            paths: Vec::new(),
            generated_outputs: Vec::new(),
        }
    }

    pub fn find_module(&self, name: &str) -> Option<&DependencyModuleSurface> {
        self.modules.iter().find(|module| module.name == name)
    }

    pub fn find_artifact(&self, name: &str) -> Option<&DependencyArtifactSurface> {
        self.artifacts.iter().find(|artifact| artifact.name == name)
    }

    pub fn find_step(&self, name: &str) -> Option<&DependencyStepSurface> {
        self.steps.iter().find(|step| step.name == name)
    }

    pub fn find_file(&self, name: &str) -> Option<&DependencyFileSurface> {
        self.files.iter().find(|file| file.name == name)
    }

    pub fn find_dir(&self, name: &str) -> Option<&DependencyDirSurface> {
        self.dirs.iter().find(|dir| dir.name == name)
    }

    pub fn find_path(&self, name: &str) -> Option<&DependencyPathSurface> {
        self.paths.iter().find(|path| path.name == name)
    }

    pub fn find_generated_output(&self, name: &str) -> Option<&DependencyGeneratedOutputSurface> {
        self.generated_outputs
            .iter()
            .find(|output| output.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DependencyArtifactSurface, DependencyArtifactSurfaceSet, DependencyBuildEvaluationMode,
        DependencyBuildExposure, DependencyBuildHandle, DependencyBuildSurface,
        DependencyBuildSurfaceSet, DependencyDirSurface, DependencyDirSurfaceSet,
        DependencyFileSurface, DependencyFileSurfaceSet, DependencyGeneratedOutputSurface,
        DependencyGeneratedOutputSurfaceSet, DependencyModuleSurface, DependencyModuleSurfaceSet,
        DependencyPathSurface, DependencyPathSurfaceSet, DependencySourceRootSurface,
        DependencyStepSurface, DependencyStepSurfaceSet,
    };

    #[test]
    fn dependency_build_surface_set_starts_empty() {
        let set = DependencyBuildSurfaceSet::new();

        assert!(set.surfaces().is_empty());
    }

    #[test]
    fn dependency_build_surface_set_preserves_inserted_shell_surfaces() {
        let mut set = DependencyBuildSurfaceSet::new();
        set.add(DependencyBuildSurface {
            alias: "logtiny".to_string(),
            exposure: DependencyBuildExposure::default(),
            modules: vec![DependencyModuleSurface {
                name: "logtiny".to_string(),
                source_namespace: "logtiny::src".to_string(),
            }],
            source_roots: vec![DependencySourceRootSurface {
                relative_path: "src".to_string(),
                namespace_prefix: "logtiny::src".to_string(),
            }],
            artifacts: vec![DependencyArtifactSurface {
                name: "logtiny".to_string(),
                artifact_kind: "static-lib".to_string(),
                fol_model: "memo".to_string(),
            }],
            steps: vec![DependencyStepSurface {
                name: "test".to_string(),
                step_kind: "test".to_string(),
            }],
            files: vec![DependencyFileSurface {
                name: "config".to_string(),
                relative_path: "config/default.toml".to_string(),
            }],
            dirs: vec![DependencyDirSurface {
                name: "assets".to_string(),
                relative_path: "assets".to_string(),
            }],
            paths: vec![DependencyPathSurface {
                name: "schema".to_string(),
                relative_path: "gen/schema.fol".to_string(),
            }],
            generated_outputs: vec![DependencyGeneratedOutputSurface {
                name: "bindings".to_string(),
                relative_path: "gen/bindings.fol".to_string(),
            }],
        });

        assert_eq!(set.surfaces().len(), 1);
        assert_eq!(set.surfaces()[0].alias, "logtiny");
        assert_eq!(set.surfaces()[0].modules.len(), 1);
        assert_eq!(set.surfaces()[0].source_roots.len(), 1);
        assert_eq!(set.surfaces()[0].artifacts.len(), 1);
        assert_eq!(set.surfaces()[0].steps.len(), 1);
        assert_eq!(set.surfaces()[0].files.len(), 1);
        assert_eq!(set.surfaces()[0].dirs.len(), 1);
        assert_eq!(set.surfaces()[0].paths.len(), 1);
        assert_eq!(set.surfaces()[0].generated_outputs.len(), 1);
    }

    #[test]
    fn dependency_build_handle_keeps_alias_and_package_identity() {
        let handle = DependencyBuildHandle {
            alias: "logtiny".to_string(),
            package: "org/logtiny".to_string(),
        };

        assert_eq!(handle.alias, "logtiny");
        assert_eq!(handle.package, "org/logtiny");
    }

    #[test]
    fn dependency_build_evaluation_modes_cover_phase_eight_loading_strategies() {
        assert_eq!(DependencyBuildEvaluationMode::Eager.as_str(), "eager");
        assert_eq!(DependencyBuildEvaluationMode::Lazy.as_str(), "lazy");
        assert_eq!(
            DependencyBuildEvaluationMode::OnDemand.as_str(),
            "on-demand"
        );
        assert_eq!(
            DependencyBuildEvaluationMode::parse("eager"),
            Some(DependencyBuildEvaluationMode::Eager)
        );
        assert_eq!(
            DependencyBuildEvaluationMode::parse("lazy"),
            Some(DependencyBuildEvaluationMode::Lazy)
        );
        assert_eq!(
            DependencyBuildEvaluationMode::parse("on-demand"),
            Some(DependencyBuildEvaluationMode::OnDemand)
        );
        assert_eq!(DependencyBuildEvaluationMode::parse("ambient"), None);
    }

    #[test]
    fn dependency_surface_collection_types_preserve_inserted_items() {
        let modules = DependencyModuleSurfaceSet {
            modules: vec![DependencyModuleSurface {
                name: "logtiny".to_string(),
                source_namespace: "logtiny::src".to_string(),
            }],
        };
        let artifacts = DependencyArtifactSurfaceSet {
            artifacts: vec![DependencyArtifactSurface {
                name: "logtiny".to_string(),
                artifact_kind: "static-lib".to_string(),
                fol_model: "memo".to_string(),
            }],
        };
        let steps = DependencyStepSurfaceSet {
            steps: vec![DependencyStepSurface {
                name: "test".to_string(),
                step_kind: "test".to_string(),
            }],
        };
        let outputs = DependencyGeneratedOutputSurfaceSet {
            generated_outputs: vec![DependencyGeneratedOutputSurface {
                name: "bindings".to_string(),
                relative_path: "gen/bindings.fol".to_string(),
            }],
        };
        let files = DependencyFileSurfaceSet {
            files: vec![DependencyFileSurface {
                name: "config".to_string(),
                relative_path: "config/default.toml".to_string(),
            }],
        };
        let dirs = DependencyDirSurfaceSet {
            dirs: vec![DependencyDirSurface {
                name: "assets".to_string(),
                relative_path: "assets".to_string(),
            }],
        };
        let paths = DependencyPathSurfaceSet {
            paths: vec![DependencyPathSurface {
                name: "schema".to_string(),
                relative_path: "gen/schema.fol".to_string(),
            }],
        };

        assert_eq!(modules.modules.len(), 1);
        assert_eq!(artifacts.artifacts.len(), 1);
        assert_eq!(steps.steps.len(), 1);
        assert_eq!(files.files.len(), 1);
        assert_eq!(dirs.dirs.len(), 1);
        assert_eq!(paths.paths.len(), 1);
        assert_eq!(outputs.generated_outputs.len(), 1);
    }

    #[test]
    fn dependency_surface_sets_can_find_surfaces_by_alias() {
        let mut set = DependencyBuildSurfaceSet::new();
        set.add(DependencyBuildSurface {
            alias: "core".to_string(),
            exposure: DependencyBuildExposure::default(),
            modules: Vec::new(),
            source_roots: Vec::new(),
            artifacts: Vec::new(),
            steps: Vec::new(),
            files: Vec::new(),
            dirs: Vec::new(),
            paths: Vec::new(),
            generated_outputs: Vec::new(),
        });

        assert_eq!(
            set.find("core").map(|surface| surface.alias.as_str()),
            Some("core")
        );
        assert!(set.find("json").is_none());
    }

    #[test]
    fn dependency_surface_lookups_find_named_members() {
        let surface = DependencyBuildSurface {
            alias: "core".to_string(),
            exposure: DependencyBuildExposure {
                modules_explicit: true,
                artifacts_explicit: false,
                steps_explicit: false,
                files_explicit: true,
                dirs_explicit: true,
                paths_explicit: true,
                generated_outputs_explicit: true,
            },
            modules: vec![DependencyModuleSurface {
                name: "root".to_string(),
                source_namespace: "core::src".to_string(),
            }],
            source_roots: vec![DependencySourceRootSurface {
                relative_path: "src".to_string(),
                namespace_prefix: "core::src".to_string(),
            }],
            artifacts: vec![DependencyArtifactSurface {
                name: "corelib".to_string(),
                artifact_kind: "static-lib".to_string(),
                fol_model: "core".to_string(),
            }],
            steps: vec![DependencyStepSurface {
                name: "check".to_string(),
                step_kind: "check".to_string(),
            }],
            files: vec![DependencyFileSurface {
                name: "config".to_string(),
                relative_path: "config/default.toml".to_string(),
            }],
            dirs: vec![DependencyDirSurface {
                name: "assets".to_string(),
                relative_path: "assets".to_string(),
            }],
            paths: vec![DependencyPathSurface {
                name: "schema".to_string(),
                relative_path: "gen/schema.fol".to_string(),
            }],
            generated_outputs: vec![DependencyGeneratedOutputSurface {
                name: "bindings".to_string(),
                relative_path: "gen/bindings.fol".to_string(),
            }],
        };

        assert_eq!(
            surface
                .find_module("root")
                .map(|module| module.source_namespace.as_str()),
            Some("core::src")
        );
        assert_eq!(
            surface
                .find_artifact("corelib")
                .map(|artifact| artifact.artifact_kind.as_str()),
            Some("static-lib")
        );
        assert_eq!(
            surface
                .find_step("check")
                .map(|step| step.step_kind.as_str()),
            Some("check")
        );
        assert_eq!(
            surface
                .find_generated_output("bindings")
                .map(|output| output.relative_path.as_str()),
            Some("gen/bindings.fol")
        );
        assert_eq!(
            surface
                .find_file("config")
                .map(|file| file.relative_path.as_str()),
            Some("config/default.toml")
        );
        assert_eq!(
            surface
                .find_dir("assets")
                .map(|dir| dir.relative_path.as_str()),
            Some("assets")
        );
        assert_eq!(
            surface
                .find_path("schema")
                .map(|path| path.relative_path.as_str()),
            Some("gen/schema.fol")
        );
        assert!(surface.exposure.modules_explicit);
        assert!(surface.exposure.files_explicit);
        assert!(surface.exposure.dirs_explicit);
        assert!(surface.exposure.paths_explicit);
        assert!(surface.exposure.generated_outputs_explicit);
        assert!(!surface.exposure.artifacts_explicit);
        assert!(surface.find_module("missing").is_none());
    }

    #[test]
    fn projected_dependency_surface_starts_without_explicit_exposure_flags() {
        let surface = DependencyBuildSurface::projected("demo");

        assert_eq!(surface.alias, "demo");
        assert_eq!(surface.exposure, DependencyBuildExposure::default());
        assert!(surface.modules.is_empty());
        assert!(surface.source_roots.is_empty());
        assert!(surface.files.is_empty());
        assert!(surface.dirs.is_empty());
        assert!(surface.paths.is_empty());
    }
}

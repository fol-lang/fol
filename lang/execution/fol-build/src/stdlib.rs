use crate::semantic::{
    canonical_artifact_config_shapes, canonical_build_context_config_shapes,
    canonical_build_context_method_signatures, canonical_chain_metadata,
    canonical_graph_method_signatures, canonical_handle_method_signatures,
    canonical_option_config_shapes, BuildSemanticMethodSignature, BuildSemanticRecordShape,
    BuildSemanticType, BuildSemanticTypeFamily,
};

/// The complete build stdlib scope injected into `build.fol` during resolution.
///
/// When the resolver encounters a file flagged as `ParsedSourceUnitKind::Build` it uses
/// this scope instead of walking the sibling `.fol` files in the package folder.
/// Every ambient graph-handle method, every handle method, and every config record shape is listed here.
#[derive(Debug, Clone)]
pub struct BuildStdlibScope {
    /// All public handle types available in `build.fol` (Artifact, Step, Run, Install, …).
    pub types: Vec<BuildSemanticType>,
    /// Methods callable on the ambient build context handle (`.build()`).
    pub build_methods: Vec<BuildSemanticMethodSignature>,
    /// Methods callable on the ambient graph handle (add_exe, install, step, dependency, …).
    pub graph_methods: Vec<BuildSemanticMethodSignature>,
    /// Methods callable on artifact/step/run/install/dependency/generated-file handles.
    pub handle_methods: Vec<BuildSemanticMethodSignature>,
    /// Record shapes accepted by build-context metadata and dependency methods.
    pub build_config_shapes: Vec<BuildSemanticRecordShape>,
    /// Record shapes accepted by graph methods that take a config record argument.
    pub artifact_config_shapes: Vec<BuildSemanticRecordShape>,
    /// Record shapes accepted by option-related graph methods.
    pub option_config_shapes: Vec<BuildSemanticRecordShape>,
}

impl BuildStdlibScope {
    /// The canonical scope for all `build.fol` files.
    pub fn canonical() -> Self {
        Self {
            types: canonical_build_types(),
            build_methods: canonical_build_context_method_signatures(),
            graph_methods: canonical_graph_method_signatures(),
            handle_methods: canonical_handle_method_signatures(),
            build_config_shapes: canonical_build_context_config_shapes(),
            artifact_config_shapes: canonical_artifact_config_shapes(),
            option_config_shapes: canonical_option_config_shapes(),
        }
    }

    /// Returns the method signature for a given build-context method name, if it exists.
    pub fn find_build_method(&self, name: &str) -> Option<&BuildSemanticMethodSignature> {
        self.build_methods.iter().find(|m| m.name == name)
    }

    /// Returns the method signature for a given receiver family and method name, if it exists.
    pub fn find_graph_method(&self, name: &str) -> Option<&BuildSemanticMethodSignature> {
        self.graph_methods.iter().find(|m| m.name == name)
    }

    /// Returns the method signature for a given handle family and method name, if it exists.
    pub fn find_handle_method(
        &self,
        family: BuildSemanticTypeFamily,
        name: &str,
    ) -> Option<&BuildSemanticMethodSignature> {
        self.handle_methods
            .iter()
            .find(|m| m.receiver == family && m.name == name)
    }

    /// Returns the method signature for the exact receiver family and method name.
    pub fn find_method_for_receiver(
        &self,
        family: BuildSemanticTypeFamily,
        name: &str,
    ) -> Option<&BuildSemanticMethodSignature> {
        match family {
            BuildSemanticTypeFamily::BuildContext => self.find_build_method(name),
            BuildSemanticTypeFamily::Graph => self.find_graph_method(name),
            _ => self.find_handle_method(family, name),
        }
    }

    /// Returns all methods available on a given handle family.
    pub fn methods_for_family(
        &self,
        family: BuildSemanticTypeFamily,
    ) -> Vec<&BuildSemanticMethodSignature> {
        match family {
            BuildSemanticTypeFamily::BuildContext => self.build_methods.iter().collect(),
            BuildSemanticTypeFamily::Graph => self.graph_methods.iter().collect(),
            _ => self
                .handle_methods
                .iter()
                .filter(|m| m.receiver == family)
                .collect(),
        }
    }

    /// Returns the chain metadata for depend_on calls, used to validate step dependencies.
    pub fn chain_metadata(&self) -> Vec<crate::semantic::BuildSemanticChainMetadata> {
        canonical_chain_metadata()
    }
}

fn canonical_build_types() -> Vec<BuildSemanticType> {
    vec![
        BuildSemanticType::artifact_handle(),
        BuildSemanticType::module_handle(),
        BuildSemanticType::step_handle(),
        BuildSemanticType::run_handle(),
        BuildSemanticType::install_handle(),
        BuildSemanticType::dependency_handle(),
        BuildSemanticType::dependency_module_handle(),
        BuildSemanticType::dependency_artifact_handle(),
        BuildSemanticType::dependency_step_handle(),
        BuildSemanticType::dependency_generated_output_handle(),
        BuildSemanticType::generated_file_handle(),
    ]
}

#[cfg(test)]
mod tests {
    use super::BuildStdlibScope;
    use crate::semantic::BuildSemanticTypeFamily;

    #[test]
    fn stdlib_scope_canonical_covers_all_build_types() {
        let scope = BuildStdlibScope::canonical();

        let families: Vec<BuildSemanticTypeFamily> = scope.types.iter().map(|t| t.family).collect();

        assert!(!families.contains(&BuildSemanticTypeFamily::BuildContext));
        assert!(families.contains(&BuildSemanticTypeFamily::ArtifactHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::StepHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::RunHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::InstallHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::DependencyHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::DependencyModuleHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::DependencyArtifactHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::DependencyStepHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::DependencyGeneratedOutputHandle));
        assert!(families.contains(&BuildSemanticTypeFamily::GeneratedFileHandle));
        // NOTE: SourceFileHandle/SourceDirHandle are transient handle families
        // and are no longer enumerated in the canonical stdlib scope types.
        assert!(families.contains(&BuildSemanticTypeFamily::ModuleHandle));
    }

    #[test]
    fn stdlib_scope_canonical_covers_core_graph_methods() {
        let scope = BuildStdlibScope::canonical();

        let build_names: Vec<&str> = scope
            .build_methods
            .iter()
            .map(|m| m.name.as_str())
            .collect();
        let graph_names: Vec<&str> = scope
            .graph_methods
            .iter()
            .map(|m| m.name.as_str())
            .collect();

        assert!(build_names.contains(&"meta"));
        assert!(build_names.contains(&"add_dep"));
        assert!(build_names.contains(&"graph"));
        assert!(graph_names.contains(&"add_exe"));
        assert!(graph_names.contains(&"add_static_lib"));
        assert!(graph_names.contains(&"add_shared_lib"));
        assert!(graph_names.contains(&"add_test"));
        assert!(graph_names.contains(&"step"));
        assert!(graph_names.contains(&"add_run"));
        assert!(graph_names.contains(&"install"));
        assert!(graph_names.contains(&"install_file"));
        assert!(graph_names.contains(&"install_dir"));
        assert!(graph_names.contains(&"write_file"));
        assert!(graph_names.contains(&"copy_file"));
        assert!(graph_names.contains(&"add_system_tool"));
        assert!(graph_names.contains(&"add_system_tool_dir"));
        assert!(graph_names.contains(&"add_system_lib"));
        assert!(graph_names.contains(&"add_codegen"));
        assert!(graph_names.contains(&"add_codegen_dir"));
        assert!(graph_names.contains(&"dependency"));
        assert!(graph_names.contains(&"file_from_root"));
        assert!(graph_names.contains(&"dir_from_root"));
        assert!(graph_names.contains(&"standard_target"));
        assert!(graph_names.contains(&"standard_optimize"));
        assert!(graph_names.contains(&"option"));
    }

    #[test]
    fn stdlib_scope_find_graph_method_returns_matching_signature() {
        let scope = BuildStdlibScope::canonical();

        let meta = scope.find_build_method("meta");
        let add_exe = scope.find_graph_method("add_exe");
        let missing = scope.find_graph_method("no_such_method");

        assert!(meta.is_some());
        assert_eq!(meta.unwrap().name, "meta");
        assert!(add_exe.is_some());
        assert_eq!(add_exe.unwrap().name, "add_exe");
        assert!(missing.is_none());
    }

    #[test]
    fn stdlib_scope_find_handle_method_is_receiver_specific() {
        let scope = BuildStdlibScope::canonical();

        let step_depend =
            scope.find_handle_method(BuildSemanticTypeFamily::StepHandle, "depend_on");
        let run_depend = scope.find_handle_method(BuildSemanticTypeFamily::RunHandle, "depend_on");
        let wrong_family =
            scope.find_handle_method(BuildSemanticTypeFamily::ArtifactHandle, "depend_on");

        assert!(step_depend.is_some());
        assert!(run_depend.is_some());
        assert!(wrong_family.is_none());
    }

    #[test]
    fn stdlib_scope_methods_for_family_returns_all_methods_for_receiver() {
        let scope = BuildStdlibScope::canonical();

        let build_methods = scope.methods_for_family(BuildSemanticTypeFamily::BuildContext);
        let build_names: Vec<&str> = build_methods.iter().map(|m| m.name.as_str()).collect();
        let graph_methods = scope.methods_for_family(BuildSemanticTypeFamily::Graph);
        let graph_names: Vec<&str> = graph_methods.iter().map(|m| m.name.as_str()).collect();
        let dep_methods = scope.methods_for_family(BuildSemanticTypeFamily::DependencyHandle);
        let dep_names: Vec<&str> = dep_methods.iter().map(|m| m.name.as_str()).collect();

        assert!(build_names.contains(&"meta"));
        assert!(build_names.contains(&"add_dep"));
        assert!(build_names.contains(&"graph"));
        assert!(graph_names.contains(&"add_exe"));
        assert!(graph_names.contains(&"install"));
        assert!(dep_names.contains(&"module"));
        assert!(dep_names.contains(&"artifact"));
        assert!(dep_names.contains(&"step"));
        assert!(dep_names.contains(&"generated"));
    }

    #[test]
    fn stdlib_scope_keeps_build_and_graph_method_surfaces_disjoint() {
        let scope = BuildStdlibScope::canonical();

        assert!(scope
            .find_method_for_receiver(BuildSemanticTypeFamily::BuildContext, "meta")
            .is_some());
        assert!(scope
            .find_method_for_receiver(BuildSemanticTypeFamily::BuildContext, "add_exe")
            .is_none());
        assert!(scope
            .find_method_for_receiver(BuildSemanticTypeFamily::Graph, "add_exe")
            .is_some());
        assert!(scope
            .find_method_for_receiver(BuildSemanticTypeFamily::Graph, "meta")
            .is_none());
    }

    #[test]
    fn stdlib_scope_build_surface_contract_stays_coherent() {
        let scope = BuildStdlibScope::canonical();
        let meta = scope.find_build_method("meta").unwrap();
        let add_dep = scope.find_build_method("add_dep").unwrap();
        let graph = scope.find_build_method("graph").unwrap();
        let build_shape_names = scope
            .build_config_shapes
            .iter()
            .map(|shape| shape.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(meta.receiver, BuildSemanticTypeFamily::BuildContext);
        assert_eq!(add_dep.receiver, BuildSemanticTypeFamily::BuildContext);
        assert_eq!(graph.receiver, BuildSemanticTypeFamily::BuildContext);
        assert_eq!(meta.params.len(), 1);
        assert_eq!(add_dep.params.len(), 1);
        assert!(graph.params.is_empty());
        assert_eq!(graph.returns, Some(BuildSemanticTypeFamily::Graph));
        assert!(build_shape_names.contains(&"BuildMetaConfig"));
        assert!(build_shape_names.contains(&"BuildDependencyConfig"));
        let meta_shape = scope
            .build_config_shapes
            .iter()
            .find(|shape| shape.name == "BuildMetaConfig")
            .expect("meta config shape should exist");
        let required = meta_shape
            .fields
            .iter()
            .filter(|field| field.required)
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(required, vec!["name", "version"]);
    }

    #[test]
    fn stdlib_scope_chain_metadata_covers_depend_on_receivers() {
        let scope = BuildStdlibScope::canonical();
        let chains = scope.chain_metadata();

        assert_eq!(chains.len(), 3);
        assert!(chains.iter().all(|c| c.method == "depend_on"));
    }

    #[test]
    fn stdlib_scope_artifact_config_shapes_cover_all_artifact_kinds() {
        let scope = BuildStdlibScope::canonical();
        let build_names: Vec<&str> = scope
            .build_config_shapes
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        let names: Vec<&str> = scope
            .artifact_config_shapes
            .iter()
            .map(|s| s.name.as_str())
            .collect();

        assert!(build_names.contains(&"BuildMetaConfig"));
        assert!(build_names.contains(&"BuildDependencyConfig"));
        assert!(names.contains(&"ExeConfig"));
        assert!(names.contains(&"StaticLibConfig"));
        assert!(names.contains(&"SharedLibConfig"));
        assert!(names.contains(&"TestConfig"));
        assert!(names.contains(&"CImportConfig"));
    }

    #[test]
    fn stdlib_scope_exposes_c_import_only_on_artifact_handles() {
        let scope = BuildStdlibScope::canonical();
        let method = scope
            .find_handle_method(BuildSemanticTypeFamily::ArtifactHandle, "add_c_import")
            .expect("artifact.add_c_import should be in the build stdlib");

        assert_eq!(method.params.len(), 1);
        assert_eq!(
            method.params[0].shape,
            crate::semantic::BuildSemanticParameterShape::Record
        );
        assert!(scope
            .find_handle_method(BuildSemanticTypeFamily::Graph, "add_c_import")
            .is_none());
    }

    #[test]
    fn stdlib_scope_option_config_shapes_cover_standard_and_user_options() {
        let scope = BuildStdlibScope::canonical();
        let names: Vec<&str> = scope
            .option_config_shapes
            .iter()
            .map(|s| s.name.as_str())
            .collect();

        assert!(names.contains(&"StandardTargetConfig"));
        assert!(names.contains(&"StandardOptimizeConfig"));
        assert!(names.contains(&"UserOptionConfig"));
    }

    #[test]
    fn stdlib_scope_keeps_internal_build_context_type_hidden() {
        let scope = BuildStdlibScope::canonical();

        assert!(scope
            .types
            .iter()
            .all(|surface_type| surface_type.family != BuildSemanticTypeFamily::BuildContext));
    }
}

use crate::{
    build_dependency::DependencyBuildSurfaceSet,
    build_entry::{BuildEntrySignatureExpectation, BuildEntryValidationError, ValidatedBuildEntry},
    build_native::NativeArtifactSet,
    PackageBuildDefinition, PackageBuildMode, PackageIdentity, PackageMetadata, PackageSourceKind,
};
use fol_parser::ast::{ParsedPackage, ParsedSourceUnit, ParsedSourceUnitKind};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PreparedExportMount {
    pub source_namespace: String,
    pub mounted_namespace_suffix: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedPackage {
    pub identity: PackageIdentity,
    pub metadata: Option<PackageMetadata>,
    pub build: Option<PackageBuildDefinition>,
    pub exports: Vec<PreparedExportMount>,
    pub dependency_surfaces: Option<DependencyBuildSurfaceSet>,
    pub native_surfaces: Option<NativeArtifactSet>,
    pub syntax: ParsedPackage,
}

impl PreparedPackage {
    pub fn new(identity: PackageIdentity, syntax: ParsedPackage) -> Self {
        Self {
            identity,
            metadata: None,
            build: None,
            exports: Vec::new(),
            dependency_surfaces: None,
            native_surfaces: None,
            syntax,
        }
    }

    pub fn with_controls(
        identity: PackageIdentity,
        metadata: PackageMetadata,
        build: PackageBuildDefinition,
        exports: Vec<PreparedExportMount>,
        dependency_surfaces: Option<DependencyBuildSurfaceSet>,
        native_surfaces: Option<NativeArtifactSet>,
        syntax: ParsedPackage,
    ) -> Self {
        Self {
            identity,
            metadata: Some(metadata),
            build: Some(build),
            exports,
            dependency_surfaces,
            native_surfaces,
            syntax,
        }
    }

    pub fn package_name(&self) -> &str {
        &self.syntax.package
    }

    pub fn source_kind(&self) -> PackageSourceKind {
        self.identity.source_kind
    }

    pub fn build_mode(&self) -> PackageBuildMode {
        self.build
            .as_ref()
            .map(PackageBuildDefinition::mode)
            .unwrap_or(PackageBuildMode::Empty)
    }

    pub fn source_units(&self) -> &[ParsedSourceUnit] {
        &self.syntax.source_units
    }

    pub fn build_source_units(&self) -> impl Iterator<Item = &ParsedSourceUnit> {
        self.syntax
            .source_units
            .iter()
            .filter(|unit| unit.kind == ParsedSourceUnitKind::Build)
    }

    pub fn ordinary_source_units(&self) -> impl Iterator<Item = &ParsedSourceUnit> {
        self.syntax
            .source_units
            .iter()
            .filter(|unit| unit.kind == ParsedSourceUnitKind::Ordinary)
    }

    pub fn validate_semantic_build_entry(
        &self,
        expectation: &BuildEntrySignatureExpectation,
    ) -> Result<ValidatedBuildEntry, Vec<BuildEntryValidationError>> {
        crate::build_entry::validate_parsed_build_entry(&self.syntax, expectation)
    }

    pub fn has_semantic_build_entry(&self, expectation: &BuildEntrySignatureExpectation) -> bool {
        self.validate_semantic_build_entry(expectation).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::PreparedPackage;
    use crate::{
        build_dependency::DependencyBuildSurfaceSet,
        build_entry::BuildEntrySignatureExpectation,
        build_native::{NativeArtifactDefinition, NativeArtifactKind, NativeArtifactSet},
        PackageBuildDefinition, PackageBuildMode, PackageConfig, PackageDependencyDecl,
        PackageDependencySourceKind, PackageIdentity, PackageMetadata, PackageSourceKind,
        PreparedExportMount,
    };
    use fol_parser::ast::{AstParser, ParsedPackage, ParsedSourceUnitKind};
    use fol_stream::FileStream;

    fn parse_fixture_package() -> ParsedPackage {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream = FileStream::from_file(fixture_path).expect("Should open package fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        parser
            .parse_package(&mut lexer)
            .expect("Package fixture should parse as a package")
    }

    #[test]
    fn package_config_defaults_to_no_external_roots() {
        let config = PackageConfig::default();

        assert_eq!(config.std_root, None);
        assert_eq!(config.package_store_root, None);
        assert_eq!(config.package_cache_root, None);
    }

    #[test]
    fn prepared_package_retains_identity_and_parsed_package_name() {
        let syntax = parse_fixture_package();
        let prepared = PreparedPackage::new(
            PackageIdentity {
                source_kind: PackageSourceKind::Entry,
                canonical_root: "/tmp/fixture".to_string(),
                display_name: "fixture".to_string(),
            },
            syntax,
        );

        assert_eq!(prepared.package_name(), "parser");
        assert_eq!(prepared.source_kind(), PackageSourceKind::Entry);
        assert_eq!(prepared.identity.display_name, "fixture");
        assert!(prepared.metadata.is_none());
        assert!(prepared.build.is_none());
        assert!(prepared.exports.is_empty());
        assert!(prepared.dependency_surfaces.is_none());
        assert!(prepared.native_surfaces.is_none());
        assert_eq!(prepared.syntax.source_units.len(), 1);
        assert_eq!(
            prepared.source_units()[0].kind,
            ParsedSourceUnitKind::Ordinary
        );
    }

    #[test]
    fn prepared_package_can_carry_metadata_and_build_controls() {
        let syntax = parse_fixture_package();
        let mut native_surfaces = NativeArtifactSet::new();
        native_surfaces.add(NativeArtifactDefinition {
            name: "api".to_string(),
            kind: NativeArtifactKind::Header,
            relative_path: "include/api.h".to_string(),
        });
        let prepared = PreparedPackage::with_controls(
            PackageIdentity {
                source_kind: PackageSourceKind::Package,
                canonical_root: "/tmp/pkg/json".to_string(),
                display_name: "json".to_string(),
            },
            PackageMetadata {
                name: "json".to_string(),
                version: "1.0.0".to_string(),
                kind: Some("lib".to_string()),
                description: None,
                license: None,
                dependencies: vec![PackageDependencyDecl {
                    alias: "core".to_string(),
                    source_kind: PackageDependencySourceKind::PackageStore,
                    target: "core".to_string(),
                    evaluation_mode: fol_build::DependencyBuildEvaluationMode::Eager,
                    git_version: None,
                    git_hash: None,
                }],
            },
            PackageBuildDefinition {
                mode: PackageBuildMode::ModernOnly,
            },
            vec![PreparedExportMount {
                source_namespace: "json::src".to_string(),
                mounted_namespace_suffix: None,
            }],
            Some(DependencyBuildSurfaceSet::new()),
            Some(native_surfaces),
            syntax,
        );

        assert_eq!(
            prepared.metadata.as_ref().map(|meta| meta.name.as_str()),
            Some("json")
        );
        assert_eq!(
            prepared.build.as_ref().map(|build| build.mode()),
            Some(PackageBuildMode::ModernOnly)
        );
        assert_eq!(prepared.exports.len(), 1);
        assert!(prepared.dependency_surfaces.is_some());
        assert_eq!(
            prepared
                .native_surfaces
                .as_ref()
                .map(|set| set.definitions().len()),
            Some(1)
        );
        assert_eq!(prepared.build_mode(), PackageBuildMode::ModernOnly);
    }

    #[test]
    fn prepared_package_helpers_split_build_and_ordinary_units() {
        let mut syntax = parse_fixture_package();
        syntax.source_units.push(fol_parser::ast::ParsedSourceUnit {
            path: "build.fol".to_string(),
            package: syntax.package.clone(),
            namespace: syntax.package.clone(),
            kind: ParsedSourceUnitKind::Build,
            items: Vec::new(),
        });
        let prepared = PreparedPackage::new(
            PackageIdentity {
                source_kind: PackageSourceKind::Entry,
                canonical_root: "/tmp/fixture".to_string(),
                display_name: "fixture".to_string(),
            },
            syntax,
        );

        assert_eq!(prepared.build_source_units().count(), 1);
        assert_eq!(prepared.ordinary_source_units().count(), 1);
        assert_eq!(
            prepared
                .build_source_units()
                .next()
                .map(|unit| unit.path.as_str()),
            Some("build.fol")
        );
    }

    #[test]
    fn prepared_package_can_validate_semantic_build_entries() {
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
        let prepared = PreparedPackage::new(
            PackageIdentity {
                source_kind: PackageSourceKind::Entry,
                canonical_root: "/tmp/demo".to_string(),
                display_name: "demo".to_string(),
            },
            syntax,
        );

        let validated = prepared
            .validate_semantic_build_entry(&BuildEntrySignatureExpectation::canonical())
            .expect("prepared packages should validate semantic build entries");
        assert_eq!(validated.candidate.name, "build");
        assert!(prepared.has_semantic_build_entry(&BuildEntrySignatureExpectation::canonical()));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardTargetDeclaration {
    pub name: String,
    pub default: Option<BuildTargetTriple>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StandardOptimizeDeclaration {
    pub name: String,
    pub default: Option<BuildOptimizeMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOptionDeclaration {
    StandardTarget(StandardTargetDeclaration),
    StandardOptimize(StandardOptimizeDeclaration),
    User(UserOptionDeclaration),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserOptionDeclaration {
    pub name: String,
    pub kind: BuildOptionKind,
    pub default: Option<BuildOptionValue>,
    pub help: Option<String>,
}

/// Compatibility name for the now-central concrete target type.
pub type BuildTargetTriple = fol_types::ResolvedTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOptimizeMode {
    Debug,
    ReleaseSafe,
    ReleaseFast,
    ReleaseSmall,
}

impl Default for BuildOptimizeMode {
    fn default() -> Self {
        Self::Debug
    }
}

impl BuildOptimizeMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::ReleaseSafe => "release-safe",
            Self::ReleaseFast => "release-fast",
            Self::ReleaseSmall => "release-small",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "debug" => Some(Self::Debug),
            "release-safe" => Some(Self::ReleaseSafe),
            "release-fast" => Some(Self::ReleaseFast),
            "release-small" => Some(Self::ReleaseSmall),
            _ => None,
        }
    }

    pub fn from_frontend_profile_name(raw: &str) -> Option<Self> {
        match raw {
            "debug" => Some(Self::Debug),
            "release" => Some(Self::ReleaseSafe),
            _ => None,
        }
    }
}

impl BuildOptionDeclaration {
    pub fn name(&self) -> &str {
        match self {
            Self::StandardTarget(declaration) => declaration.name.as_str(),
            Self::StandardOptimize(declaration) => declaration.name.as_str(),
            Self::User(declaration) => declaration.name.as_str(),
        }
    }

    pub fn coerce_raw_value(&self, raw: &str) -> Option<String> {
        match self {
            Self::StandardTarget(_) => BuildTargetTriple::parse(raw).map(|value| value.render()),
            Self::StandardOptimize(_) => {
                BuildOptimizeMode::parse(raw).map(|value| value.as_str().to_string())
            }
            Self::User(declaration) => {
                BuildOptionValue::parse_for_kind(declaration.kind, raw).map(|value| value.render())
            }
        }
    }

    pub fn default_raw_value(&self) -> Option<String> {
        match self {
            Self::StandardTarget(declaration) => {
                declaration.default.as_ref().map(|value| value.render())
            }
            Self::StandardOptimize(declaration) => {
                declaration.default.map(|value| value.as_str().to_string())
            }
            Self::User(declaration) => declaration.default.as_ref().map(BuildOptionValue::render),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildOptionDeclarationSet {
    declarations: Vec<BuildOptionDeclaration>,
}

impl BuildOptionDeclarationSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn declarations(&self) -> &[BuildOptionDeclaration] {
        &self.declarations
    }

    pub fn add(&mut self, declaration: BuildOptionDeclaration) {
        self.declarations.push(declaration);
    }

    pub fn find(&self, name: &str) -> Option<&BuildOptionDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildOptionOverrideParseError {
    MissingName,
    MissingValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildOptionOverride {
    pub name: String,
    pub raw_value: String,
}

impl BuildOptionOverride {
    pub fn parse(raw: &str) -> Result<Self, BuildOptionOverrideParseError> {
        let (name, raw_value) = raw
            .split_once('=')
            .ok_or(BuildOptionOverrideParseError::MissingValue)?;
        if name.is_empty() {
            return Err(BuildOptionOverrideParseError::MissingName);
        }
        if raw_value.is_empty() {
            return Err(BuildOptionOverrideParseError::MissingValue);
        }
        Ok(Self {
            name: name.to_string(),
            raw_value: raw_value.to_string(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedBuildOptionSet {
    values: BTreeMap<String, String>,
}

impl ResolvedBuildOptionSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<String>, raw_value: impl Into<String>) {
        self.values.insert(name.into(), raw_value.into());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BuildOptimizeMode, BuildOptionDeclaration, BuildOptionDeclarationSet, BuildOptionOverride,
        BuildOptionOverrideParseError, BuildTargetTriple, ResolvedBuildOptionSet,
        StandardOptimizeDeclaration, StandardTargetDeclaration, UserOptionDeclaration,
    };
    use crate::api::BuildOptionValue;
    use crate::graph::BuildOptionKind;

    #[test]
    fn build_option_declaration_set_starts_empty() {
        let set = BuildOptionDeclarationSet::new();

        assert!(set.declarations().is_empty());
    }

    #[test]
    fn build_option_declaration_set_preserves_inserted_shell_declarations() {
        let mut set = BuildOptionDeclarationSet::new();
        set.add(BuildOptionDeclaration::StandardTarget(
            StandardTargetDeclaration {
                name: "target".to_string(),
                default: BuildTargetTriple::parse("x86_64-linux-gnu"),
            },
        ));
        set.add(BuildOptionDeclaration::StandardOptimize(
            StandardOptimizeDeclaration {
                name: "optimize".to_string(),
                default: Some(BuildOptimizeMode::Debug),
            },
        ));
        set.add(BuildOptionDeclaration::User(UserOptionDeclaration {
            name: "docs".to_string(),
            kind: BuildOptionKind::Bool,
            default: Some(BuildOptionValue::Bool(false)),
            help: Some("enable docs generation".to_string()),
        }));

        assert_eq!(set.declarations().len(), 3);
        assert!(matches!(
            &set.declarations()[0],
            BuildOptionDeclaration::StandardTarget(StandardTargetDeclaration { name, .. })
            if name == "target"
        ));
        assert!(matches!(
            &set.declarations()[1],
            BuildOptionDeclaration::StandardOptimize(StandardOptimizeDeclaration { name, .. })
            if name == "optimize"
        ));
        assert!(matches!(
            &set.declarations()[2],
            BuildOptionDeclaration::User(UserOptionDeclaration { name, kind, default, .. })
            if name == "docs"
                && *kind == BuildOptionKind::Bool
                && *default == Some(BuildOptionValue::Bool(false))
        ));
    }

    #[test]
    fn build_target_triple_keeps_option_and_rust_spellings_distinct() {
        let triple =
            BuildTargetTriple::parse("x86_64-linux-gnu").expect("canonical triples should parse");

        assert_eq!(triple.render(), "x86_64-linux-gnu");
        assert_eq!(triple.rust_target_triple(), "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn build_target_triple_rejects_unknown_or_malformed_triplets() {
        assert!(BuildTargetTriple::parse("x86_64").is_none());
        assert!(BuildTargetTriple::parse("sparc-linux-gnu").is_none());
        assert!(BuildTargetTriple::parse("x86_64-linux-gnu-extra").is_none());
    }

    #[test]
    fn build_optimize_mode_parses_and_renders_canonical_modes() {
        assert_eq!(
            BuildOptimizeMode::parse("release-fast"),
            Some(BuildOptimizeMode::ReleaseFast)
        );
        assert_eq!(BuildOptimizeMode::Debug.as_str(), "debug");
        assert_eq!(BuildOptimizeMode::ReleaseSmall.as_str(), "release-small");
    }

    #[test]
    fn build_optimize_mode_maps_frontend_profiles_onto_canonical_modes() {
        assert_eq!(
            BuildOptimizeMode::from_frontend_profile_name("debug"),
            Some(BuildOptimizeMode::Debug)
        );
        assert_eq!(
            BuildOptimizeMode::from_frontend_profile_name("release"),
            Some(BuildOptimizeMode::ReleaseSafe)
        );
        assert_eq!(BuildOptimizeMode::from_frontend_profile_name("bench"), None);
    }

    #[test]
    fn build_option_override_parses_named_equals_value_pairs() {
        let override_value =
            BuildOptionOverride::parse("jobs=16").expect("named overrides should parse");

        assert_eq!(override_value.name, "jobs");
        assert_eq!(override_value.raw_value, "16");
    }

    #[test]
    fn build_option_override_rejects_missing_names_or_values() {
        assert_eq!(
            BuildOptionOverride::parse("=fast"),
            Err(BuildOptionOverrideParseError::MissingName)
        );
        assert_eq!(
            BuildOptionOverride::parse("jobs"),
            Err(BuildOptionOverrideParseError::MissingValue)
        );
        assert_eq!(
            BuildOptionOverride::parse("jobs="),
            Err(BuildOptionOverrideParseError::MissingValue)
        );
    }

    #[test]
    fn resolved_build_option_set_supports_named_lookup() {
        let mut resolved = ResolvedBuildOptionSet::new();
        resolved.insert("target", "aarch64-macos-gnu");
        resolved.insert("optimize", "release-fast");

        assert_eq!(resolved.get("target"), Some("aarch64-macos-gnu"));
        assert_eq!(resolved.get("optimize"), Some("release-fast"));
        assert_eq!(
            resolved.iter().collect::<Vec<_>>(),
            vec![
                ("optimize", "release-fast"),
                ("target", "aarch64-macos-gnu")
            ]
        );
    }

    #[test]
    fn build_option_values_render_with_stable_raw_spelling() {
        assert_eq!(BuildOptionValue::Bool(true).render(), "true");
        assert_eq!(BuildOptionValue::Int(8).render(), "8");
        assert_eq!(
            BuildOptionValue::String("dist".to_string()).render(),
            "dist"
        );
        assert_eq!(
            BuildOptionValue::Enum("release".to_string()).render(),
            "release"
        );
        assert_eq!(
            BuildOptionValue::Path("src/app.fol".to_string()).render(),
            "src/app.fol"
        );
    }

    #[test]
    fn build_option_values_parse_against_user_kinds() {
        assert_eq!(
            BuildOptionValue::parse_for_kind(BuildOptionKind::Bool, "false"),
            Some(BuildOptionValue::Bool(false))
        );
        assert_eq!(
            BuildOptionValue::parse_for_kind(BuildOptionKind::Int, "16"),
            Some(BuildOptionValue::Int(16))
        );
        assert_eq!(
            BuildOptionValue::parse_for_kind(BuildOptionKind::String, "dist"),
            Some(BuildOptionValue::String("dist".to_string()))
        );
        assert_eq!(
            BuildOptionValue::parse_for_kind(BuildOptionKind::Enum, "fast"),
            Some(BuildOptionValue::Enum("fast".to_string()))
        );
        assert_eq!(
            BuildOptionValue::parse_for_kind(BuildOptionKind::Path, "src/app.fol"),
            Some(BuildOptionValue::Path("src/app.fol".to_string()))
        );
        assert_eq!(
            BuildOptionValue::parse_for_kind(BuildOptionKind::Bool, "yes"),
            None
        );
        assert_eq!(
            BuildOptionValue::parse_for_kind(BuildOptionKind::Target, "native"),
            None
        );
    }

    #[test]
    fn build_option_declarations_coerce_raw_values_and_defaults() {
        let target = BuildOptionDeclaration::StandardTarget(StandardTargetDeclaration {
            name: "target".to_string(),
            default: BuildTargetTriple::parse("x86_64-linux-gnu"),
        });
        let user = BuildOptionDeclaration::User(UserOptionDeclaration {
            name: "jobs".to_string(),
            kind: BuildOptionKind::Int,
            default: Some(BuildOptionValue::Int(8)),
            help: None,
        });

        assert_eq!(
            target.default_raw_value(),
            Some("x86_64-linux-gnu".to_string())
        );
        assert_eq!(user.default_raw_value(), Some("8".to_string()));
        assert_eq!(user.coerce_raw_value("16"), Some("16".to_string()));
        assert_eq!(user.coerce_raw_value("fast"), None);
    }
}
use crate::api::BuildOptionValue;
use crate::graph::BuildOptionKind;
use std::collections::BTreeMap;

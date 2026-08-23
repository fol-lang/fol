//! The one resolved artifact plan.
//!
//! Section 4.3 of `plan/V4_PLAN.md` requires that a single value carry every
//! ABI-affecting fact about an artifact from build evaluation through to the
//! backend. Before this, the same artifact was described several times over --
//! graph artifact, projected definition, executor artifact, frontend selection,
//! backend config -- and each description dropped something the next one then
//! reconstructed from a default or guessed from an output filename.
//!
//! A plan is produced once and is not re-derived. Where a field cannot be
//! populated yet, it is an explicit empty collection whose owning milestone is
//! named in a comment, never a value invented downstream.

use crate::artifact::{BuildArtifactFolModel, BuildArtifactNativeAttachmentSet};
use crate::native::NativeLinkDirective;
use crate::option::BuildOptimizeMode;

/// The exact kind of thing an artifact is.
///
/// Section 4.3 separates an executable from a test executable: they build the
/// same way and are selected, installed, and reported differently, so a single
/// `Executable` variant loses the distinction that decides whether `fol code
/// test` should run something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResolvedArtifactKind {
    Executable,
    TestExecutable,
    StaticLibrary,
    SharedLibrary,
    Object,
}

impl ResolvedArtifactKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::TestExecutable => "test-executable",
            Self::StaticLibrary => "static-library",
            Self::SharedLibrary => "shared-library",
            Self::Object => "object",
        }
    }

    /// Whether the artifact produces something the host can execute.
    pub const fn is_executable(self) -> bool {
        matches!(self, Self::Executable | Self::TestExecutable)
    }
}

/// What a produced file is for.
///
/// An artifact is a set of role-tagged outputs, not one path. A consumer asks
/// for the role it needs rather than pattern-matching a filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum OutputRole {
    Executable,
    TestExecutable,
    StaticArchive,
    SharedLibrary,
    Object,
    CHeader,
    AbiManifest,
    NativeLinkInterface,
    DebugSymbols,
}

impl OutputRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::TestExecutable => "test-executable",
            Self::StaticArchive => "static-archive",
            Self::SharedLibrary => "shared-library",
            Self::Object => "object",
            Self::CHeader => "c-header",
            Self::AbiManifest => "abi-manifest",
            Self::NativeLinkInterface => "native-link-interface",
            Self::DebugSymbols => "debug-symbols",
        }
    }
}

/// One produced file and what it is for.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedOutput {
    pub role: OutputRole,
    /// Relative to the artifact's build root.
    pub relative_path: String,
}

/// Where one produced output is installed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedInstall {
    pub role: OutputRole,
    /// Relative to the install prefix, per section 4.16.
    pub destination: String,
}

/// A source or generated file the artifact is built from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResolvedInput {
    /// A FOL source module.
    Module(String),
    /// A file produced by a build action before compilation.
    Generated(String),
}

impl ResolvedInput {
    pub fn path(&self) -> &str {
        match self {
            Self::Module(path) | Self::Generated(path) => path,
        }
    }
}

/// One entry of a library artifact's ABI export allowlist, per section 4.10.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedAbiExport {
    /// Fully qualified FOL routine, e.g. `api::add`.
    pub routine: String,
    /// Exact external C symbol. Never mangled, never inferred.
    pub symbol: String,
}

/// The ABI major/minor a library artifact declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct ResolvedAbiVersion {
    pub major: u32,
    pub minor: u32,
}

/// The artifact's public C surface.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedAbiSurface {
    pub version: ResolvedAbiVersion,
    pub exports: Vec<ResolvedAbiExport>,
    /// C imports attached to this artifact, by header path.
    pub imports: Vec<String>,
}

/// The ordered native link plan.
///
/// Order is significant and repetitions are meaningful: section 4.5 forbids
/// silently deduplicating link atoms, because a static link can legitimately
/// need the same archive twice.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedLinkPlan {
    pub directives: Vec<NativeLinkDirective>,
    /// Artifacts this one links against, in declaration order.
    pub linked_artifacts: Vec<String>,
    /// Native requirements this artifact propagates to its dependents.
    pub propagated_interface: Vec<NativeLinkDirective>,
}

/// Where a plan came from and what it was built with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedProvenance {
    pub package_name: String,
    pub package_version: String,
    /// Absolute path of the `build.fol` that declared the artifact.
    pub build_source: String,
}

/// One artifact, resolved once, carrying every fact downstream layers need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedArtifactPlan {
    pub name: String,
    pub kind: ResolvedArtifactKind,
    pub provenance: ResolvedProvenance,
    pub root_source: String,
    pub inputs: Vec<ResolvedInput>,
    pub fol_model: BuildArtifactFolModel,
    pub target: fol_types::ResolvedTarget,
    pub optimize: BuildOptimizeMode,
    pub abi: ResolvedAbiSurface,
    pub link_plan: ResolvedLinkPlan,
    pub native_attachments: BuildArtifactNativeAttachmentSet,
    pub outputs: Vec<ResolvedOutput>,
    pub installs: Vec<ResolvedInstall>,
}

impl ResolvedArtifactPlan {
    /// The output for one role, if the artifact produces it.
    pub fn output_for(&self, role: OutputRole) -> Option<&ResolvedOutput> {
        self.outputs.iter().find(|output| output.role == role)
    }

    /// The role a link step produces as this artifact's primary result.
    pub const fn primary_output_role(&self) -> OutputRole {
        match self.kind {
            ResolvedArtifactKind::Executable => OutputRole::Executable,
            ResolvedArtifactKind::TestExecutable => OutputRole::TestExecutable,
            ResolvedArtifactKind::StaticLibrary => OutputRole::StaticArchive,
            ResolvedArtifactKind::SharedLibrary => OutputRole::SharedLibrary,
            ResolvedArtifactKind::Object => OutputRole::Object,
        }
    }

    /// The file name the primary output takes on this plan's target.
    ///
    /// Naming is a target fact, not a guess from the artifact kind: MinGW keeps
    /// the ELF-style `libcore.a` while using the PE-style `core.dll`.
    pub fn primary_output_file_name(&self) -> String {
        match self.kind {
            ResolvedArtifactKind::Executable | ResolvedArtifactKind::TestExecutable => {
                self.target.executable_file_name(&self.name)
            }
            ResolvedArtifactKind::StaticLibrary => self.target.archive_file_name(&self.name),
            ResolvedArtifactKind::SharedLibrary => self.target.shared_library_file_name(&self.name),
            ResolvedArtifactKind::Object => format!("{}.o", self.name),
        }
    }
}

/// Deterministic identity for a resolved plan.
///
/// Two plans that differ in any ABI-affecting way must hash differently, and
/// two runs of the same plan must hash identically. Workspace identity already
/// exists and covers only the lowered source, which is why a debug and a
/// release build of one package share a crate directory name; this covers the
/// facts that one leaves out.
///
/// The digest is built from a canonical rendering rather than from `Hash`,
/// because `Hash` is not stable across Rust versions or platforms and a cache
/// key that changes with the compiler is not a cache key.
pub mod identity {
    use super::{
        OutputRole, ResolvedAbiExport, ResolvedArtifactKind, ResolvedArtifactPlan, ResolvedInput,
    };
    use std::collections::BTreeMap;

    /// A value that is part of determinism data but must not be persisted
    /// verbatim.
    ///
    /// An environment value can be a token, a signing key, or an absolute path
    /// naming a user. Recording it raw puts a secret in a build manifest that
    /// travels with the artifact; dropping it entirely makes two different
    /// builds look identical. Hashing keeps the distinction and loses the
    /// secret.
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub enum EnvironmentValue {
        /// Safe to record as-is: a known, non-secret selector.
        Public(String),
        /// Recorded as a digest of the value, never the value.
        Redacted(String),
    }

    impl EnvironmentValue {
        /// Render for inclusion in determinism data.
        pub fn rendered(&self) -> String {
            match self {
                Self::Public(value) => value.clone(),
                Self::Redacted(digest) => format!("redacted:{digest}"),
            }
        }

        /// Hash `value` rather than keeping it.
        pub fn redact(value: &str) -> Self {
            Self::Redacted(digest(value.as_bytes()))
        }
    }

    /// Environment names whose values are safe to record verbatim.
    ///
    /// The list is an allowlist rather than a denylist of secret-looking names:
    /// a denylist fails open, and the failure is a leaked credential.
    pub const PUBLIC_ENVIRONMENT_NAMES: &[&str] = &[
        "FOL_MODEL",
        "FOL_PROFILE",
        "FOL_TARGET",
        "CARGO_BUILD_TARGET",
        "RUSTC_BOOTSTRAP",
    ];

    /// Classify one environment entry for determinism data.
    pub fn classify_environment(name: &str, value: &str) -> EnvironmentValue {
        if PUBLIC_ENVIRONMENT_NAMES.contains(&name) {
            EnvironmentValue::Public(value.to_string())
        } else {
            EnvironmentValue::redact(value)
        }
    }

    /// FNV-1a over the canonical rendering. Stable across platforms and Rust
    /// versions, which `std`'s hasher is not.
    fn digest(bytes: &[u8]) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("{hash:016x}")
    }

    fn render_kind(kind: ResolvedArtifactKind) -> &'static str {
        kind.as_str()
    }

    fn render_inputs(inputs: &[ResolvedInput]) -> String {
        // Sorted, because two graphs that declare the same inputs in a
        // different order build the same artifact. Input *order* is not
        // ABI-affecting; link order is, and is rendered separately.
        let mut rendered: Vec<String> = inputs
            .iter()
            .map(|input| match input {
                ResolvedInput::Module(path) => format!("module:{path}"),
                ResolvedInput::Generated(path) => format!("generated:{path}"),
            })
            .collect();
        rendered.sort();
        rendered.join(",")
    }

    fn render_exports(exports: &[ResolvedAbiExport]) -> String {
        let mut rendered: Vec<String> = exports
            .iter()
            .map(|export| format!("{}={}", export.routine, export.symbol))
            .collect();
        rendered.sort();
        rendered.join(",")
    }

    fn render_output_roles(plan: &ResolvedArtifactPlan) -> String {
        let mut roles: Vec<&'static str> = plan
            .outputs
            .iter()
            .map(|output| OutputRole::as_str(output.role))
            .collect();
        roles.sort_unstable();
        roles.join(",")
    }

    fn render_link_plan(plan: &ResolvedArtifactPlan) -> String {
        // Order is preserved here, unlike inputs: a static link is
        // order-sensitive and a repeated archive is meaningful.
        let directives: Vec<String> = plan
            .link_plan
            .directives
            .iter()
            .map(|directive| format!("{:?}/{:?}", directive.input, directive.mode))
            .collect();
        format!(
            "{}|{}",
            directives.join(","),
            plan.link_plan.linked_artifacts.join(",")
        )
    }

    /// The canonical rendering a plan's identity is computed over.
    ///
    /// Exposed so a failing identity comparison can show what differed instead
    /// of two opaque hashes.
    pub fn canonical_rendering(
        plan: &ResolvedArtifactPlan,
        environment: &BTreeMap<String, EnvironmentValue>,
    ) -> String {
        let mut rendered = String::new();
        rendered.push_str(&format!("name={}\n", plan.name));
        rendered.push_str(&format!("kind={}\n", render_kind(plan.kind)));
        rendered.push_str(&format!("target={}\n", plan.target.rust_target_triple()));
        rendered.push_str(&format!("tier={}\n", plan.target.tier().as_str()));
        rendered.push_str(&format!("model={:?}\n", plan.fol_model));
        rendered.push_str(&format!("profile={:?}\n", plan.optimize));
        rendered.push_str(&format!("inputs={}\n", render_inputs(&plan.inputs)));
        rendered.push_str(&format!(
            "abi={}.{}\n",
            plan.abi.version.major, plan.abi.version.minor
        ));
        rendered.push_str(&format!("exports={}\n", render_exports(&plan.abi.exports)));
        rendered.push_str(&format!("link={}\n", render_link_plan(plan)));
        rendered.push_str(&format!("outputs={}\n", render_output_roles(plan)));
        for (name, value) in environment {
            rendered.push_str(&format!("env:{name}={}\n", value.rendered()));
        }
        rendered
    }

    /// The deterministic identity of a plan.
    pub fn plan_identity(
        plan: &ResolvedArtifactPlan,
        environment: &BTreeMap<String, EnvironmentValue>,
    ) -> String {
        digest(canonical_rendering(plan, environment).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::identity::{
        classify_environment, plan_identity, EnvironmentValue, PUBLIC_ENVIRONMENT_NAMES,
    };
    use super::*;
    use std::collections::BTreeMap;

    fn sample_plan() -> ResolvedArtifactPlan {
        ResolvedArtifactPlan {
            name: "app".to_string(),
            kind: ResolvedArtifactKind::Executable,
            provenance: ResolvedProvenance {
                package_name: "demo".to_string(),
                package_version: "0.1.0".to_string(),
                build_source: "/demo/build.fol".to_string(),
            },
            root_source: "src/main.fol".to_string(),
            inputs: vec![ResolvedInput::Module("src/main.fol".to_string())],
            fol_model: BuildArtifactFolModel::Memo,
            target: fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap(),
            optimize: BuildOptimizeMode::Debug,
            abi: ResolvedAbiSurface::default(),
            link_plan: ResolvedLinkPlan::default(),
            native_attachments: BuildArtifactNativeAttachmentSet::default(),
            outputs: vec![ResolvedOutput {
                role: OutputRole::Executable,
                relative_path: "bin/app".to_string(),
            }],
            installs: vec![ResolvedInstall {
                role: OutputRole::Executable,
                destination: "bin/app".to_string(),
            }],
        }
    }

    fn identity_of(plan: &ResolvedArtifactPlan) -> String {
        plan_identity(plan, &BTreeMap::new())
    }

    #[test]
    fn identity_is_stable_for_an_unchanged_plan() {
        let plan = sample_plan();
        assert_eq!(identity_of(&plan), identity_of(&plan.clone()));
    }

    /// Every fact section 4.3 calls ABI-affecting must move the identity. This
    /// is the test that would have caught workspace identity ignoring the build
    /// profile.
    #[test]
    fn every_abi_affecting_field_changes_the_identity() {
        let base = sample_plan();
        let baseline = identity_of(&base);

        let mut kind = base.clone();
        kind.kind = ResolvedArtifactKind::TestExecutable;
        assert_ne!(identity_of(&kind), baseline, "artifact kind");

        let mut target = base.clone();
        target.target = fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-musl").unwrap();
        assert_ne!(identity_of(&target), baseline, "target");

        let mut model = base.clone();
        model.fol_model = BuildArtifactFolModel::Core;
        assert_ne!(identity_of(&model), baseline, "fol model");

        let mut profile = base.clone();
        profile.optimize = BuildOptimizeMode::ReleaseFast;
        assert_ne!(identity_of(&profile), baseline, "profile");

        let mut inputs = base.clone();
        inputs
            .inputs
            .push(ResolvedInput::Generated("src/gen.fol".to_string()));
        assert_ne!(identity_of(&inputs), baseline, "inputs");

        let mut exports = base.clone();
        exports.abi.exports.push(ResolvedAbiExport {
            routine: "api::add".to_string(),
            symbol: "fol_demo_add".to_string(),
        });
        assert_ne!(identity_of(&exports), baseline, "abi exports");

        let mut abi_version = base.clone();
        abi_version.abi.version = ResolvedAbiVersion { major: 2, minor: 0 };
        assert_ne!(identity_of(&abi_version), baseline, "abi version");

        let mut link = base.clone();
        link.link_plan.linked_artifacts.push("core".to_string());
        assert_ne!(identity_of(&link), baseline, "link plan");

        let mut outputs = base.clone();
        outputs.outputs.push(ResolvedOutput {
            role: OutputRole::DebugSymbols,
            relative_path: "bin/app.debug".to_string(),
        });
        assert_ne!(identity_of(&outputs), baseline, "output roles");
    }

    /// Declaring the same inputs in a different order builds the same artifact,
    /// so it must not build a different identity. Link order is the opposite
    /// case and is asserted below.
    #[test]
    fn input_order_does_not_change_identity_but_link_order_does() {
        let mut first = sample_plan();
        first.inputs = vec![
            ResolvedInput::Module("a.fol".to_string()),
            ResolvedInput::Module("b.fol".to_string()),
        ];
        let mut second = first.clone();
        second.inputs.reverse();
        assert_eq!(identity_of(&first), identity_of(&second));

        let mut linked = first.clone();
        linked.link_plan.linked_artifacts = vec!["alpha".to_string(), "beta".to_string()];
        let mut reordered = linked.clone();
        reordered.link_plan.linked_artifacts.reverse();
        assert_ne!(
            identity_of(&linked),
            identity_of(&reordered),
            "static link order is significant and must reach the identity"
        );
    }

    #[test]
    fn environment_values_are_redacted_unless_explicitly_public() {
        let secret = classify_environment("GITHUB_TOKEN", "ghp_realsecretvalue");
        assert!(
            matches!(secret, EnvironmentValue::Redacted(_)),
            "an unlisted name must be redacted"
        );
        assert!(
            !secret.rendered().contains("ghp_realsecretvalue"),
            "the raw value reached the determinism data: {}",
            secret.rendered()
        );

        for name in PUBLIC_ENVIRONMENT_NAMES {
            assert_eq!(
                classify_environment(name, "value"),
                EnvironmentValue::Public("value".to_string()),
                "{name} is allowlisted and should be recorded verbatim"
            );
        }
    }

    #[test]
    fn redaction_still_distinguishes_two_different_secrets() {
        // Dropping the value entirely would make these two builds look
        // identical, which is the failure redaction exists to avoid.
        let first = EnvironmentValue::redact("secret-one");
        let second = EnvironmentValue::redact("secret-two");
        assert_ne!(first, second);
        assert_eq!(first, EnvironmentValue::redact("secret-one"));
    }

    #[test]
    fn environment_reaches_the_identity() {
        let plan = sample_plan();
        let mut environment = BTreeMap::new();
        environment.insert(
            "GITHUB_TOKEN".to_string(),
            EnvironmentValue::redact("first"),
        );
        let with_first = plan_identity(&plan, &environment);

        environment.insert(
            "GITHUB_TOKEN".to_string(),
            EnvironmentValue::redact("second"),
        );
        assert_ne!(with_first, plan_identity(&plan, &environment));
    }

    #[test]
    fn primary_output_names_follow_the_target() {
        let mut plan = sample_plan();
        assert_eq!(plan.primary_output_file_name(), "app");
        assert_eq!(plan.primary_output_role(), OutputRole::Executable);

        plan.kind = ResolvedArtifactKind::StaticLibrary;
        assert_eq!(plan.primary_output_file_name(), "libapp.a");
        assert_eq!(plan.primary_output_role(), OutputRole::StaticArchive);

        plan.kind = ResolvedArtifactKind::SharedLibrary;
        assert_eq!(plan.primary_output_file_name(), "libapp.so");

        plan.target = fol_types::ResolvedTarget::resolve("x86_64-pc-windows-msvc").unwrap();
        plan.kind = ResolvedArtifactKind::StaticLibrary;
        assert_eq!(plan.primary_output_file_name(), "app.lib");
    }

    /// A test executable is not an executable. Collapsing them is what makes a
    /// test bundle indistinguishable from a binary downstream.
    #[test]
    fn executable_and_test_executable_are_distinct_kinds() {
        assert_ne!(
            ResolvedArtifactKind::Executable,
            ResolvedArtifactKind::TestExecutable
        );
        assert!(ResolvedArtifactKind::Executable.is_executable());
        assert!(ResolvedArtifactKind::TestExecutable.is_executable());
        assert!(!ResolvedArtifactKind::Object.is_executable());
        assert_ne!(
            ResolvedArtifactKind::Executable.as_str(),
            ResolvedArtifactKind::TestExecutable.as_str()
        );
    }
}

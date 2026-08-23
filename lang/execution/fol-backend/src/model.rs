#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedRustFile {
    pub path: String,
    pub module_name: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendBuildPaths {
    pub output_root: String,
    pub build_root: String,
    pub bin_root: String,
    pub runtime_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendArtifact {
    RustSourceCrate {
        root: String,
        files: Vec<EmittedRustFile>,
    },
    CompiledBinary {
        crate_root: String,
        binary_path: String,
    },
}

/// What the backend is asked to produce.
///
/// Distinct from `fol_build::plan::ResolvedArtifactKind`, which is the *build
/// graph's* view. This one decides emitter and `rustc` behaviour: which entry
/// file to write, which `--crate-type` to pass, and what the output is called.
/// They are deliberately separate types so a build-graph kind cannot leak into
/// a rustc flag without a translation that has to be written down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BackendProductKind {
    Executable,
    TestExecutable,
    StaticLibrary,
    SharedLibrary,
    Object,
}

impl BackendProductKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::TestExecutable => "test-executable",
            Self::StaticLibrary => "static-library",
            Self::SharedLibrary => "shared-library",
            Self::Object => "object",
        }
    }

    /// Whether the product has a `main` and can be executed.
    ///
    /// A library must not be searched for an entry routine: an artifact with no
    /// `main` is correct, not broken.
    pub const fn has_entry_point(self) -> bool {
        matches!(self, Self::Executable | Self::TestExecutable)
    }

    /// The crate root file name the emitter writes.
    pub const fn crate_root_file_name(self) -> &'static str {
        if self.has_entry_point() {
            "main.rs"
        } else {
            "lib.rs"
        }
    }

    /// The `--crate-type` argument, or `None` when the product is not produced
    /// by a single `rustc` crate-type.
    ///
    /// `Object` returns `None`: emitting a relocatable object is only correct
    /// alongside a complete link-interface sidecar enumerating what the final
    /// link still needs, and V4 refuses to fake that.
    pub const fn rustc_crate_type(self) -> Option<&'static str> {
        match self {
            Self::Executable | Self::TestExecutable => Some("bin"),
            Self::StaticLibrary => Some("staticlib"),
            Self::SharedLibrary => Some("cdylib"),
            Self::Object => None,
        }
    }

    /// The file name this product takes on `target`.
    pub fn output_file_name(self, artifact: &str, target: &fol_types::ResolvedTarget) -> String {
        match self {
            Self::Executable | Self::TestExecutable => target.executable_file_name(artifact),
            Self::StaticLibrary => target.archive_file_name(artifact),
            Self::SharedLibrary => target.shared_library_file_name(artifact),
            Self::Object => format!("{artifact}.o"),
        }
    }
}

/// A role-tagged file the backend produced.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProducedOutput {
    pub role: fol_build::plan::OutputRole,
    pub path: String,
}

/// Everything one backend invocation produced.
///
/// An artifact is a set of role-tagged outputs, not one path. The generated
/// Rust crate is deliberately **not** in `outputs`: it is a private
/// materializer input, and section 4.3 forbids it becoming an installable or
/// released role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducedArtifact {
    pub kind: BackendProductKind,
    pub name: String,
    pub target: fol_types::ResolvedTarget,
    /// Private build directory holding the generated Rust crate.
    pub crate_root: String,
    pub outputs: Vec<ProducedOutput>,
}

impl ProducedArtifact {
    pub fn output_for(&self, role: fol_build::plan::OutputRole) -> Option<&ProducedOutput> {
        self.outputs.iter().find(|output| output.role == role)
    }

    /// The role this product's main file carries.
    pub const fn primary_role(&self) -> fol_build::plan::OutputRole {
        use fol_build::plan::OutputRole;
        match self.kind {
            BackendProductKind::Executable => OutputRole::Executable,
            BackendProductKind::TestExecutable => OutputRole::TestExecutable,
            BackendProductKind::StaticLibrary => OutputRole::StaticArchive,
            BackendProductKind::SharedLibrary => OutputRole::SharedLibrary,
            BackendProductKind::Object => OutputRole::Object,
        }
    }
}

#[cfg(test)]
mod product_kind_tests {
    use super::{BackendProductKind, ProducedArtifact, ProducedOutput};
    use fol_build::plan::OutputRole;

    #[test]
    fn only_executables_have_an_entry_point() {
        assert!(BackendProductKind::Executable.has_entry_point());
        assert!(BackendProductKind::TestExecutable.has_entry_point());
        for kind in [
            BackendProductKind::StaticLibrary,
            BackendProductKind::SharedLibrary,
            BackendProductKind::Object,
        ] {
            assert!(
                !kind.has_entry_point(),
                "{kind:?} must not be searched for a main routine"
            );
            assert_eq!(kind.crate_root_file_name(), "lib.rs");
        }
        assert_eq!(
            BackendProductKind::Executable.crate_root_file_name(),
            "main.rs"
        );
    }

    #[test]
    fn crate_types_match_the_product() {
        assert_eq!(
            BackendProductKind::Executable.rustc_crate_type(),
            Some("bin")
        );
        assert_eq!(
            BackendProductKind::TestExecutable.rustc_crate_type(),
            Some("bin")
        );
        assert_eq!(
            BackendProductKind::StaticLibrary.rustc_crate_type(),
            Some("staticlib")
        );
        assert_eq!(
            BackendProductKind::SharedLibrary.rustc_crate_type(),
            Some("cdylib")
        );
        // An object needs a link-interface sidecar; there is no single
        // crate-type that produces a correct one.
        assert_eq!(BackendProductKind::Object.rustc_crate_type(), None);
    }

    #[test]
    fn output_names_follow_the_target() {
        let linux = fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(
            BackendProductKind::Executable.output_file_name("app", &linux),
            "app"
        );
        assert_eq!(
            BackendProductKind::StaticLibrary.output_file_name("core", &linux),
            "libcore.a"
        );
        assert_eq!(
            BackendProductKind::SharedLibrary.output_file_name("core", &linux),
            "libcore.so"
        );

        let msvc = fol_types::ResolvedTarget::resolve("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(
            BackendProductKind::Executable.output_file_name("app", &msvc),
            "app.exe"
        );
        assert_eq!(
            BackendProductKind::StaticLibrary.output_file_name("core", &msvc),
            "core.lib"
        );
    }

    /// The generated Rust crate is a private input, never a produced role.
    #[test]
    fn a_produced_artifact_never_lists_its_rust_crate_as_an_output() {
        let artifact = ProducedArtifact {
            kind: BackendProductKind::StaticLibrary,
            name: "core".to_string(),
            target: fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap(),
            crate_root: "/build/fol-build-core-abc".to_string(),
            outputs: vec![ProducedOutput {
                role: OutputRole::StaticArchive,
                path: "/build/lib/libcore.a".to_string(),
            }],
        };

        assert_eq!(artifact.primary_role(), OutputRole::StaticArchive);
        assert!(artifact.output_for(OutputRole::StaticArchive).is_some());
        assert!(
            !artifact
                .outputs
                .iter()
                .any(|output| output.path.contains("fol-build-core-abc")),
            "the private crate directory leaked into the produced roles"
        );
    }
}

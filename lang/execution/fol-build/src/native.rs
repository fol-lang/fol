#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeArtifactKind {
    Header,
    Object,
    StaticLibrary,
    SharedLibrary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSearchPathOrigin {
    PackageRoot,
    BuildRoot,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeIncludePath {
    pub origin: NativeSearchPathOrigin,
    pub relative_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLibraryPath {
    pub origin: NativeSearchPathOrigin,
    pub relative_path: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeLinkMode {
    Static,
    Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemLibraryRequest {
    pub name: String,
    pub mode: NativeLinkMode,
    pub framework: bool,
    pub search_path: Option<String>,
}

impl SystemLibraryRequest {
    pub fn link_input(&self) -> NativeLinkInput {
        if self.framework {
            NativeLinkInput::LibraryName(format!("framework:{}", self.name))
        } else {
            NativeLinkInput::LibraryName(self.name.clone())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeLinkInput {
    Artifact(NativeArtifactDefinition),
    LibraryName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLinkDirective {
    pub input: NativeLinkInput,
    pub mode: NativeLinkMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeArtifactDefinition {
    pub name: String,
    pub kind: NativeArtifactKind,
    pub relative_path: String,
}

impl NativeArtifactDefinition {
    /// The file name this native artifact takes on `target`.
    ///
    /// Reads the target's own naming rules rather than a platform enum of its
    /// own. The enum this replaces mapped every Windows target to `{name}.lib`,
    /// which is wrong for MinGW: it uses the ELF-style `lib{name}.a` with the
    /// PE-style `{name}.dll`. Section 4.4 forbids a second target model for
    /// exactly this reason.
    pub fn canonical_file_name(&self, target: &fol_types::ResolvedTarget) -> String {
        match self.kind {
            NativeArtifactKind::Header => self.name.clone(),
            NativeArtifactKind::Object => match target.object_format() {
                fol_types::ObjectFormat::Pe => format!("{}.obj", self.name),
                fol_types::ObjectFormat::Elf | fol_types::ObjectFormat::MachO => {
                    format!("{}.o", self.name)
                }
            },
            NativeArtifactKind::StaticLibrary => target.archive_file_name(&self.name),
            NativeArtifactKind::SharedLibrary => target.shared_library_file_name(&self.name),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeArtifactSet {
    definitions: Vec<NativeArtifactDefinition>,
}

impl NativeArtifactSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn definitions(&self) -> &[NativeArtifactDefinition] {
        &self.definitions
    }

    pub fn add(&mut self, definition: NativeArtifactDefinition) {
        self.definitions.push(definition);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NativeArtifactDefinition, NativeArtifactKind, NativeArtifactSet, NativeIncludePath,
        NativeLibraryPath, NativeLinkDirective, NativeLinkInput, NativeLinkMode,
        NativeSearchPathOrigin,
    };

    #[test]
    fn native_artifact_set_starts_empty() {
        let set = NativeArtifactSet::new();

        assert!(set.definitions().is_empty());
    }

    #[test]
    fn native_artifact_set_preserves_inserted_shell_definitions() {
        let mut set = NativeArtifactSet::new();
        set.add(NativeArtifactDefinition {
            name: "api".to_string(),
            kind: NativeArtifactKind::Header,
            relative_path: "include/api.h".to_string(),
        });

        assert_eq!(set.definitions().len(), 1);
        assert_eq!(set.definitions()[0].name, "api");
        assert_eq!(set.definitions()[0].kind, NativeArtifactKind::Header);
        assert_eq!(set.definitions()[0].relative_path, "include/api.h");
    }

    #[test]
    fn native_artifact_kinds_cover_phase_ten_shapes() {
        assert_eq!(NativeArtifactKind::Header, NativeArtifactKind::Header);
        assert_eq!(NativeArtifactKind::Object, NativeArtifactKind::Object);
        assert_eq!(
            NativeArtifactKind::StaticLibrary,
            NativeArtifactKind::StaticLibrary
        );
        assert_eq!(
            NativeArtifactKind::SharedLibrary,
            NativeArtifactKind::SharedLibrary
        );
    }

    #[test]
    fn native_include_paths_keep_origin_and_relative_path() {
        let path = NativeIncludePath {
            origin: NativeSearchPathOrigin::PackageRoot,
            relative_path: "include".to_string(),
        };

        assert_eq!(path.origin, NativeSearchPathOrigin::PackageRoot);
        assert_eq!(path.relative_path, "include");
    }

    #[test]
    fn native_library_paths_cover_package_build_and_system_origins() {
        let package = NativeLibraryPath {
            origin: NativeSearchPathOrigin::PackageRoot,
            relative_path: "native/lib".to_string(),
        };
        let build = NativeLibraryPath {
            origin: NativeSearchPathOrigin::BuildRoot,
            relative_path: "out/lib".to_string(),
        };
        let system = NativeLibraryPath {
            origin: NativeSearchPathOrigin::System,
            relative_path: "/usr/lib".to_string(),
        };

        assert_eq!(package.origin, NativeSearchPathOrigin::PackageRoot);
        assert_eq!(build.origin, NativeSearchPathOrigin::BuildRoot);
        assert_eq!(system.origin, NativeSearchPathOrigin::System);
    }

    #[test]
    fn native_link_directives_keep_mode_and_library_inputs() {
        let directive = NativeLinkDirective {
            input: NativeLinkInput::LibraryName("ssl".to_string()),
            mode: NativeLinkMode::Dynamic,
        };

        assert_eq!(directive.mode, NativeLinkMode::Dynamic);
        assert_eq!(
            directive.input,
            NativeLinkInput::LibraryName("ssl".to_string())
        );
    }

    #[test]
    fn native_link_directives_can_reference_declared_native_artifacts() {
        let artifact = NativeArtifactDefinition {
            name: "crypto".to_string(),
            kind: NativeArtifactKind::StaticLibrary,
            relative_path: "native/libcrypto.a".to_string(),
        };
        let directive = NativeLinkDirective {
            input: NativeLinkInput::Artifact(artifact.clone()),
            mode: NativeLinkMode::Static,
        };

        assert_eq!(directive.mode, NativeLinkMode::Static);
        assert_eq!(directive.input, NativeLinkInput::Artifact(artifact));
    }

    fn target(spelling: &str) -> fol_types::ResolvedTarget {
        fol_types::ResolvedTarget::resolve(spelling).expect("a table target should resolve")
    }

    #[test]
    fn native_header_names_stay_plain_across_targets() {
        let header = NativeArtifactDefinition {
            name: "api.h".to_string(),
            kind: NativeArtifactKind::Header,
            relative_path: "include/api.h".to_string(),
        };

        for spelling in [
            "x86_64-unknown-linux-gnu",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ] {
            assert_eq!(header.canonical_file_name(&target(spelling)), "api.h");
        }
    }

    #[test]
    fn native_library_names_follow_the_target_convention() {
        let static_lib = NativeArtifactDefinition {
            name: "ssl".to_string(),
            kind: NativeArtifactKind::StaticLibrary,
            relative_path: "native/libssl.a".to_string(),
        };
        let shared_lib = NativeArtifactDefinition {
            name: "crypto".to_string(),
            kind: NativeArtifactKind::SharedLibrary,
            relative_path: "native/libcrypto.so".to_string(),
        };
        let object = NativeArtifactDefinition {
            name: "part".to_string(),
            kind: NativeArtifactKind::Object,
            relative_path: "native/part.o".to_string(),
        };

        let linux = target("x86_64-unknown-linux-gnu");
        assert_eq!(static_lib.canonical_file_name(&linux), "libssl.a");
        assert_eq!(shared_lib.canonical_file_name(&linux), "libcrypto.so");
        assert_eq!(object.canonical_file_name(&linux), "part.o");

        let darwin = target("aarch64-apple-darwin");
        assert_eq!(shared_lib.canonical_file_name(&darwin), "libcrypto.dylib");

        let msvc = target("x86_64-pc-windows-msvc");
        assert_eq!(static_lib.canonical_file_name(&msvc), "ssl.lib");
        assert_eq!(shared_lib.canonical_file_name(&msvc), "crypto.dll");
        assert_eq!(object.canonical_file_name(&msvc), "part.obj");

        // MinGW mixes the two conventions. The platform enum this replaced
        // mapped every Windows target to `ssl.lib`, which is wrong here.
        let mingw = target("x86_64-pc-windows-gnu");
        assert_eq!(static_lib.canonical_file_name(&mingw), "libssl.a");
        assert_eq!(shared_lib.canonical_file_name(&mingw), "crypto.dll");
    }
}

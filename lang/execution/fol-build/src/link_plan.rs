//! One ordered native link plan.
//!
//! Before this, native link inputs lived as loose declarations on a graph
//! artifact -- library paths and link directives that nothing outside
//! `fol-build` ever read. This module resolves every kind of link handle into
//! a single ordered sequence, validates it, and renders it as structured
//! compiler arguments.
//!
//! Two rules shape the whole module. Order is significant and repetition is
//! meaningful, so nothing here deduplicates or sorts atoms: a static link
//! genuinely needs an archive twice sometimes, and dependents must precede
//! providers. And arguments are emitted as separate argv items, never as a
//! concatenated flag string -- a path containing a space or a comma must not
//! be able to split itself into two arguments.

use crate::native::{NativeArtifactKind, NativeLinkMode};
use std::collections::BTreeMap;
use std::ffi::OsString;

/// Where a link input came from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LinkAtomOrigin {
    /// An artifact produced by this build graph.
    LocalArtifact { artifact: String },
    /// An artifact produced by a dependency package.
    DependencyArtifact { package: String, artifact: String },
    /// A file named exactly by the build program.
    ExactFile,
    /// A library resolved by name through the system linker.
    SystemLibrary,
}

/// One resolved link input.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NativeLinkAtom {
    pub origin: LinkAtomOrigin,
    /// The library or file name, without target decoration.
    pub name: String,
    /// The exact path, for everything except a by-name system library.
    pub path: Option<String>,
    pub kind: NativeArtifactKind,
    pub mode: NativeLinkMode,
    /// The target this input was produced for. `None` for a by-name system
    /// library, which the linker resolves against the target sysroot.
    pub target: Option<fol_types::ResolvedTarget>,
    /// Content digest, when the producer supplied one.
    pub digest: Option<String>,
}

impl NativeLinkAtom {
    /// A system library resolved by name.
    pub fn system_library(name: impl Into<String>, mode: NativeLinkMode) -> Self {
        Self {
            origin: LinkAtomOrigin::SystemLibrary,
            name: name.into(),
            path: None,
            kind: match mode {
                NativeLinkMode::Static => NativeArtifactKind::StaticLibrary,
                NativeLinkMode::Dynamic => NativeArtifactKind::SharedLibrary,
            },
            mode,
            target: None,
            digest: None,
        }
    }

    /// An artifact this graph produced.
    pub fn local_artifact(
        artifact: impl Into<String>,
        path: impl Into<String>,
        kind: NativeArtifactKind,
        target: fol_types::ResolvedTarget,
    ) -> Self {
        let artifact = artifact.into();
        Self {
            origin: LinkAtomOrigin::LocalArtifact {
                artifact: artifact.clone(),
            },
            name: artifact,
            path: Some(path.into()),
            kind,
            mode: match kind {
                NativeArtifactKind::SharedLibrary => NativeLinkMode::Dynamic,
                _ => NativeLinkMode::Static,
            },
            target: Some(target),
            digest: None,
        }
    }
}

/// What a dependency package exports for linking.
///
/// Section 4.5 requires a dependency handle to carry more than a name: without
/// the exact role path, the target it was built for, and a content digest, a
/// consumer cannot tell whether the archive it is about to link is the right
/// one or even the right architecture.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyArtifactExport {
    pub package: String,
    pub artifact: String,
    pub kind: NativeArtifactKind,
    /// Exact path of the produced role.
    pub role_path: String,
    pub target: fol_types::ResolvedTarget,
    pub content_digest: String,
    /// How the dependency was obtained: a registry name, a path, a revision.
    pub provenance: String,
    /// Native requirements this export passes on to whoever links it.
    pub transitive_interface: Vec<NativeLinkAtom>,
}

impl DependencyArtifactExport {
    pub fn as_atom(&self) -> NativeLinkAtom {
        NativeLinkAtom {
            origin: LinkAtomOrigin::DependencyArtifact {
                package: self.package.clone(),
                artifact: self.artifact.clone(),
            },
            name: self.artifact.clone(),
            path: Some(self.role_path.clone()),
            kind: self.kind,
            mode: match self.kind {
                NativeArtifactKind::SharedLibrary => NativeLinkMode::Dynamic,
                _ => NativeLinkMode::Static,
            },
            target: Some(self.target.clone()),
            digest: Some(self.content_digest.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkPlanErrorKind {
    /// An artifact links itself.
    SelfLink,
    /// Artifacts link in a cycle.
    LinkCycle,
    /// An executable cannot be a link input.
    IncompatibleArtifactKind,
    /// An input was built for a different target.
    TargetMismatch,
    /// An input's object format does not match the target's.
    ObjectFormatMismatch,
    /// An input names a role its producer does not have.
    MissingRole,
    /// Two inputs provide the same library name.
    DuplicateProvider,
    /// An Apple framework outside a promoted Apple lane.
    FrameworkNotSupported,
    /// A Windows import library outside a promoted MSVC or MinGW lane.
    ImportLibraryNotSupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPlanError {
    pub kind: LinkPlanErrorKind,
    pub message: String,
}

impl LinkPlanError {
    fn new(kind: LinkPlanErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for LinkPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for LinkPlanError {}

/// The ordered native inputs for one artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeLinkPlan {
    /// The artifact this plan links.
    pub artifact: String,
    pub target: fol_types::ResolvedTarget,
    /// Ordered: dependents precede the providers they need, and declaration
    /// order is preserved among siblings.
    pub atoms: Vec<NativeLinkAtom>,
    /// Directories to search, in declaration order.
    pub search_paths: Vec<String>,
    /// The installed file name a shared library records as its own identity.
    ///
    /// `None` for every other product kind. Without it a consumer records
    /// whatever path it happened to link through, so linking by absolute path
    /// bakes a build-machine path into the consumer.
    pub soname: Option<String>,
}

impl NativeLinkPlan {
    pub fn new(artifact: impl Into<String>, target: fol_types::ResolvedTarget) -> Self {
        Self {
            artifact: artifact.into(),
            target,
            atoms: Vec::new(),
            search_paths: Vec::new(),
            soname: None,
        }
    }

    /// Record the installed name a shared library carries as its identity.
    pub fn set_soname(&mut self, name: impl Into<String>) {
        self.soname = Some(name.into());
    }

    pub fn push(&mut self, atom: NativeLinkAtom) {
        self.atoms.push(atom);
    }

    pub fn push_search_path(&mut self, path: impl Into<String>) {
        self.search_paths.push(path.into());
    }

    /// Every check that must pass before a linker runs.
    pub fn validate(&self) -> Vec<LinkPlanError> {
        let mut errors = Vec::new();
        errors.extend(self.validate_no_self_link());
        errors.extend(self.validate_kinds());
        errors.extend(self.validate_targets());
        errors.extend(self.validate_promoted_lanes());
        errors.extend(self.validate_unique_providers());
        errors
    }

    fn validate_no_self_link(&self) -> Vec<LinkPlanError> {
        self.atoms
            .iter()
            .filter(|atom| {
                matches!(
                    &atom.origin,
                    LinkAtomOrigin::LocalArtifact { artifact } if *artifact == self.artifact
                )
            })
            .map(|_| {
                LinkPlanError::new(
                    LinkPlanErrorKind::SelfLink,
                    format!("artifact '{}' links against itself", self.artifact),
                )
            })
            .collect()
    }

    fn validate_kinds(&self) -> Vec<LinkPlanError> {
        self.atoms
            .iter()
            .filter(|atom| atom.kind == NativeArtifactKind::Header)
            .map(|atom| {
                LinkPlanError::new(
                    LinkPlanErrorKind::IncompatibleArtifactKind,
                    format!(
                        "'{}' is a header and cannot be a link input to '{}'",
                        atom.name, self.artifact
                    ),
                )
            })
            .collect()
    }

    fn validate_targets(&self) -> Vec<LinkPlanError> {
        let mut errors = Vec::new();
        for atom in &self.atoms {
            let Some(target) = &atom.target else {
                // A by-name system library is resolved by the linker against
                // the target sysroot, so it carries no target of its own.
                continue;
            };
            if target != &self.target {
                errors.push(LinkPlanError::new(
                    LinkPlanErrorKind::TargetMismatch,
                    format!(
                        "'{}' was built for {} and cannot link into '{}', which targets {}",
                        atom.name,
                        target.rust_target_triple(),
                        self.artifact,
                        self.target.rust_target_triple()
                    ),
                ));
                continue;
            }
        }
        errors
    }

    /// What the declared checks cannot see: whether each input file actually is
    /// what the graph says it is.
    ///
    /// `validate_targets` rejects an atom whose target differs, so by the time
    /// two atoms agree on a target they agree on an object format by
    /// construction -- comparing the declared formats there could never fail.
    /// Finding a mislabelled archive means reading it, which needs I/O and so
    /// lives here rather than in the pure `validate`.
    ///
    /// A file that cannot be identified is not an error: the plan carries
    /// inputs this is not the judge of, and a linker that disagrees will say
    /// so with more context than a guess here would.
    pub fn validate_native_inputs(&self) -> Vec<LinkPlanError> {
        let mut errors = Vec::new();
        for atom in &self.atoms {
            let Some(path) = &atom.path else { continue };
            let Some(facts) = inspect_native_file(std::path::Path::new(path)) else {
                continue;
            };
            if facts.format != self.target.object_format() {
                errors.push(LinkPlanError::new(
                    LinkPlanErrorKind::ObjectFormatMismatch,
                    format!(
                        "'{}' is {:?} on disk and '{}' needs {:?}",
                        atom.name,
                        facts.format,
                        self.artifact,
                        self.target.object_format()
                    ),
                ));
                continue;
            }
            if let Some(arch) = facts.arch {
                if arch != self.target.arch() {
                    errors.push(LinkPlanError::new(
                        LinkPlanErrorKind::TargetMismatch,
                        format!(
                            "'{}' is built for {:?} on disk and '{}' targets {:?}",
                            atom.name,
                            arch,
                            self.artifact,
                            self.target.arch()
                        ),
                    ));
                }
            }
        }
        errors
    }

    /// Framework and import-library inputs stay rejected until their sibling
    /// lanes are promoted, per section 16.3.
    fn validate_promoted_lanes(&self) -> Vec<LinkPlanError> {
        let mut errors = Vec::new();
        for atom in &self.atoms {
            if atom.name.starts_with("framework:") {
                errors.push(LinkPlanError::new(
                    LinkPlanErrorKind::FrameworkNotSupported,
                    format!(
                        "'{}' is an Apple framework, and no Apple lane is promoted; \
                         frameworks are rejected on the certified targets",
                        atom.name
                    ),
                ));
            }
            let is_import_library = atom
                .path
                .as_deref()
                .is_some_and(|path| path.ends_with(".lib") || path.ends_with(".imp.lib"));
            if is_import_library && self.target.tier() != fol_types::TargetTier::Certified {
                errors.push(LinkPlanError::new(
                    LinkPlanErrorKind::ImportLibraryNotSupported,
                    format!(
                        "'{}' is a Windows import library, and no MSVC or MinGW lane is \
                         promoted",
                        atom.name
                    ),
                ));
            }
        }
        errors
    }

    /// Two inputs providing the same library name is ambiguous: which one wins
    /// depends on link order, and the answer is not what the author wrote down.
    fn validate_unique_providers(&self) -> Vec<LinkPlanError> {
        let mut providers: BTreeMap<&str, &LinkAtomOrigin> = BTreeMap::new();
        let mut errors = Vec::new();
        for atom in &self.atoms {
            // A repeated *identical* atom is meaningful in a static link and is
            // not a duplicate provider; two different origins for one name are.
            if let Some(first) = providers.get(atom.name.as_str()) {
                if *first != &atom.origin {
                    errors.push(LinkPlanError::new(
                        LinkPlanErrorKind::DuplicateProvider,
                        format!(
                            "'{}' is provided by two different sources ({:?} and {:?}); \
                             which one wins would depend on link order",
                            atom.name, first, atom.origin
                        ),
                    ));
                }
                continue;
            }
            providers.insert(atom.name.as_str(), &atom.origin);
        }
        errors
    }

    /// The plan as structured compiler arguments.
    ///
    /// Every element is its own argv item. Nothing is concatenated into a flag
    /// string, so a path containing a space or a comma cannot split itself into
    /// two arguments -- which is how raw-flag concatenation goes wrong.
    pub fn to_rustc_args(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = Vec::new();
        // The flag and its value stay two `-Wl` items rather than one comma
        // list, so a name is never split on a comma it contains.
        if let Some(soname) = &self.soname {
            let flag = match self.target.object_format() {
                fol_types::ObjectFormat::Elf => Some("-Wl,-soname"),
                fol_types::ObjectFormat::MachO => Some("-Wl,-install_name"),
                // PE has no equivalent: a DLL's identity is its file name.
                fol_types::ObjectFormat::Pe => None,
            };
            if let Some(flag) = flag {
                args.push(OsString::from("-C"));
                args.push(OsString::from(format!("link-arg={flag}")));
                args.push(OsString::from("-C"));
                args.push(OsString::from(format!("link-arg=-Wl,{soname}")));
            }
        }
        for path in &self.search_paths {
            args.push(OsString::from("-L"));
            args.push(OsString::from(format!("native={path}")));
        }
        for atom in &self.atoms {
            match (&atom.origin, &atom.path) {
                // An exact file is passed to the linker as a path, because
                // `-l` would resolve by name and could find a different file.
                (LinkAtomOrigin::SystemLibrary, _) => {
                    let kind = match atom.mode {
                        NativeLinkMode::Static => "static",
                        NativeLinkMode::Dynamic => "dylib",
                    };
                    args.push(OsString::from("-l"));
                    args.push(OsString::from(format!("{kind}={}", atom.name)));
                }
                (_, Some(path)) => {
                    args.push(OsString::from("-C"));
                    let mut arg = OsString::from("link-arg=");
                    arg.push(path);
                    args.push(arg);
                }
                (_, None) => {
                    args.push(OsString::from("-l"));
                    args.push(OsString::from(atom.name.clone()));
                }
            }
        }
        args
    }

    /// The fingerprint contribution of this plan.
    ///
    /// Link order is part of it, because two plans with the same atoms in a
    /// different order can produce different binaries.
    pub fn fingerprint_rendering(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str(self.target.rust_target_triple());
        rendered.push('\n');
        if let Some(soname) = &self.soname {
            rendered.push_str("soname:");
            rendered.push_str(soname);
            rendered.push('\n');
        }
        for path in &self.search_paths {
            rendered.push_str("L:");
            rendered.push_str(path);
            rendered.push('\n');
        }
        for atom in &self.atoms {
            rendered.push_str(&format!(
                "{:?}|{}|{:?}|{:?}|{}\n",
                atom.origin,
                atom.name,
                atom.kind,
                atom.mode,
                atom.digest.as_deref().unwrap_or("-")
            ));
        }
        rendered
    }
}

/// Order a set of artifacts so every dependent precedes its providers.
///
/// A static link resolves left to right, so an archive must appear after
/// whatever needs its symbols. Declaration order is preserved among artifacts
/// that do not depend on one another, so the plan is reproducible.
pub fn order_dependents_before_providers(
    root: &str,
    links: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<String>, LinkPlanError> {
    let mut ordered: Vec<String> = Vec::new();
    let mut visiting: Vec<String> = Vec::new();

    fn visit(
        name: &str,
        links: &BTreeMap<String, Vec<String>>,
        ordered: &mut Vec<String>,
        visiting: &mut Vec<String>,
    ) -> Result<(), LinkPlanError> {
        // The in-progress check comes first. Checking `ordered` first would
        // return early on a back-edge, because a node is pushed there before
        // its providers are visited -- so a cycle would look like a node
        // already handled.
        if visiting.iter().any(|seen| seen == name) {
            return Err(LinkPlanError::new(
                LinkPlanErrorKind::LinkCycle,
                format!(
                    "artifacts link in a cycle: {} -> {name}",
                    visiting.join(" -> ")
                ),
            ));
        }
        if ordered.iter().any(|seen| seen == name) {
            return Ok(());
        }
        visiting.push(name.to_string());
        ordered.push(name.to_string());
        for provider in links.get(name).map(Vec::as_slice).unwrap_or_default() {
            visit(provider, links, ordered, visiting)?;
        }
        visiting.pop();
        Ok(())
    }

    visit(root, links, &mut ordered, &mut visiting)?;
    Ok(ordered)
}

/// What a native file's own bytes say it is.
///
/// The declared-metadata checks cannot catch a mislabelled input: an atom
/// whose target differs is already rejected, so by the time two atoms agree on
/// a target they agree on an object format by construction. The only way to
/// find an archive that is not what the graph says it is, is to look at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeFileFacts {
    pub format: fol_types::ObjectFormat,
    /// `None` when the format carries no architecture FOL models, or when the
    /// architecture is one FOL has no target for.
    pub arch: Option<fol_types::TargetArch>,
}

/// Read the leading bytes of a native input and say what they are.
///
/// `None` for a file that is not a native object, archive, or shared library,
/// and for one that cannot be read. Neither is an error here: this reports what
/// it can identify, and the caller decides what an unidentifiable input means.
pub fn inspect_native_file(path: &std::path::Path) -> Option<NativeFileFacts> {
    inspect_bytes(&read_prefix(path, 1024)?)
}

fn inspect_bytes(bytes: &[u8]) -> Option<NativeFileFacts> {
    // A static archive: an 8-byte magic, a 60-byte member header, then the
    // first member. Its format is the members' format, so identify one. A
    // mixed-architecture archive is not something a linker accepts either, so
    // the first member is enough.
    if bytes.starts_with(b"!<arch>\n") {
        return bytes.get(68..).and_then(inspect_bytes);
    }
    if bytes.starts_with(b"\x7fELF") {
        return Some(NativeFileFacts {
            format: fol_types::ObjectFormat::Elf,
            arch: elf_arch(bytes),
        });
    }
    if bytes.starts_with(b"MZ") {
        return Some(NativeFileFacts {
            format: fol_types::ObjectFormat::Pe,
            arch: None,
        });
    }
    // Mach-O, in the four byte orders and widths it is written in.
    let macho = [
        [0xfeu8, 0xed, 0xfa, 0xce],
        [0xce, 0xfa, 0xed, 0xfe],
        [0xfe, 0xed, 0xfa, 0xcf],
        [0xcf, 0xfa, 0xed, 0xfe],
    ];
    if macho.iter().any(|magic| bytes.starts_with(magic)) {
        return Some(NativeFileFacts {
            format: fol_types::ObjectFormat::MachO,
            arch: None,
        });
    }
    None
}

/// `e_machine` from an ELF header, for the architectures FOL has targets for.
fn elf_arch(bytes: &[u8]) -> Option<fol_types::TargetArch> {
    // Byte 5 is EI_DATA: 1 little-endian, 2 big-endian. `e_machine` is a
    // two-byte field at offset 18 written in that order.
    let little = *bytes.get(5)? == 1;
    let low = *bytes.get(18)?;
    let high = *bytes.get(19)?;
    let machine = if little {
        u16::from(low) | (u16::from(high) << 8)
    } else {
        u16::from(high) | (u16::from(low) << 8)
    };
    match machine {
        0x3e => Some(fol_types::TargetArch::X86_64),
        0xb7 => Some(fol_types::TargetArch::Aarch64),
        _ => None,
    }
}

fn read_prefix(path: &std::path::Path, limit: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut buffer = vec![0u8; limit];
    let read = file.read(&mut buffer).ok()?;
    buffer.truncate(read);
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(spelling: &str) -> fol_types::ResolvedTarget {
        fol_types::ResolvedTarget::resolve(spelling).expect("a table target should resolve")
    }

    fn plan() -> NativeLinkPlan {
        NativeLinkPlan::new("app", target("x86_64-unknown-linux-gnu"))
    }

    /// A package-owned search path that leaves the package is refused.
    ///
    /// The origin is why this can be decided at all: by the time a path
    /// becomes `-L native=<path>` it is a bare string with no owner attached.
    #[test]
    fn a_package_search_path_may_not_leave_the_package() {
        use crate::native::{NativeLibraryPath, NativeSearchPathOrigin};

        for escaping in ["../outside/lib", "lib/../../outside", "/absolute/lib"] {
            let error = super::validate_search_path(
                "app",
                &NativeLibraryPath {
                    origin: NativeSearchPathOrigin::PackageRoot,
                    relative_path: escaping.to_string(),
                },
            )
            .unwrap_or_else(|| panic!("{escaping} should be refused"));
            assert!(
                error.message.contains(escaping),
                "the refusal should quote the path: {}",
                error.message
            );
        }
    }

    /// An ordinary package-relative path is accepted.
    #[test]
    fn a_contained_package_search_path_is_accepted() {
        use crate::native::{NativeLibraryPath, NativeSearchPathOrigin};

        for contained in ["lib", "native/lib", "build/x86_64/lib"] {
            assert!(
                super::validate_search_path(
                    "app",
                    &NativeLibraryPath {
                        origin: NativeSearchPathOrigin::PackageRoot,
                        relative_path: contained.to_string(),
                    },
                )
                .is_none(),
                "{contained} should be accepted"
            );
        }
    }

    /// A system search path is exempt: an SDK is not inside the package, which
    /// is the same reason angled include roots are exempt on the header side.
    #[test]
    fn a_system_search_path_may_live_anywhere() {
        use crate::native::{NativeLibraryPath, NativeSearchPathOrigin};

        for outside in ["/opt/sdk/lib", "../elsewhere"] {
            assert!(
                super::validate_search_path(
                    "app",
                    &NativeLibraryPath {
                        origin: NativeSearchPathOrigin::System,
                        relative_path: outside.to_string(),
                    },
                )
                .is_none(),
                "{outside} should be accepted as a system path"
            );
        }
    }

    /// The magic bytes of each format FOL has a target for.
    #[test]
    fn native_files_are_identified_by_their_bytes() {
        use super::inspect_bytes;
        use fol_types::{ObjectFormat, TargetArch};

        // A little-endian 64-bit x86-64 ELF header prefix: magic, EI_CLASS,
        // EI_DATA, then e_machine 0x3e at offset 18.
        let mut elf = vec![0u8; 32];
        elf[..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2;
        elf[5] = 1;
        elf[18] = 0x3e;
        let facts = inspect_bytes(&elf).expect("an ELF header is identifiable");
        assert_eq!(facts.format, ObjectFormat::Elf);
        assert_eq!(facts.arch, Some(TargetArch::X86_64));

        // The same header claiming aarch64.
        elf[18] = 0xb7;
        assert_eq!(
            inspect_bytes(&elf).expect("still ELF").arch,
            Some(TargetArch::Aarch64)
        );

        assert_eq!(
            inspect_bytes(b"MZ\x90\x00").expect("PE").format,
            ObjectFormat::Pe
        );
        assert_eq!(
            inspect_bytes(&[0xcf, 0xfa, 0xed, 0xfe])
                .expect("Mach-O")
                .format,
            ObjectFormat::MachO
        );
        assert!(inspect_bytes(b"not an object file").is_none());
    }

    /// An archive is identified by its first member, not by its own magic.
    #[test]
    fn an_archive_is_identified_through_its_first_member() {
        use super::inspect_bytes;
        use fol_types::{ObjectFormat, TargetArch};

        let mut archive = Vec::new();
        archive.extend_from_slice(b"!<arch>\n");
        archive.extend_from_slice(&[b' '; 60]); // the member header
        let mut member = vec![0u8; 32];
        member[..4].copy_from_slice(b"\x7fELF");
        member[4] = 2;
        member[5] = 1;
        member[18] = 0x3e;
        archive.extend_from_slice(&member);

        let facts = inspect_bytes(&archive).expect("an archive is identifiable");
        assert_eq!(facts.format, ObjectFormat::Elf);
        assert_eq!(facts.arch, Some(TargetArch::X86_64));
    }

    /// The check that could not fire before: an input whose bytes disagree with
    /// the target it is being linked into.
    #[test]
    fn an_input_whose_bytes_are_the_wrong_format_is_rejected() {
        let scratch = std::env::temp_dir().join(format!(
            "fol-link-inspect-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&scratch).expect("scratch");

        // A PE object handed to an ELF link.
        let intruder = scratch.join("libwrong.a");
        std::fs::write(&intruder, b"MZ\x90\x00padding").expect("write");

        let mut plan = plan();
        plan.push(NativeLinkAtom::local_artifact(
            "wrong".to_string(),
            intruder.display().to_string(),
            NativeArtifactKind::StaticLibrary,
            target("x86_64-unknown-linux-gnu"),
        ));

        let errors = plan.validate_native_inputs();
        let _ = std::fs::remove_dir_all(&scratch);

        assert_eq!(errors.len(), 1, "expected one rejection: {errors:?}");
        assert_eq!(errors[0].kind, LinkPlanErrorKind::ObjectFormatMismatch);
        assert!(
            errors[0].message.contains("on disk"),
            "the message should say the evidence came from the file: {}",
            errors[0].message
        );
    }

    /// An input the inspector cannot identify is not an error: the plan carries
    /// inputs this is not the judge of.
    #[test]
    fn an_unidentifiable_input_is_left_alone() {
        let scratch = std::env::temp_dir().join(format!(
            "fol-link-unknown-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&scratch).expect("scratch");
        let opaque = scratch.join("libopaque.a");
        std::fs::write(&opaque, b"something else entirely").expect("write");

        let mut plan = plan();
        plan.push(NativeLinkAtom::local_artifact(
            "opaque".to_string(),
            opaque.display().to_string(),
            NativeArtifactKind::StaticLibrary,
            target("x86_64-unknown-linux-gnu"),
        ));
        let errors = plan.validate_native_inputs();
        let _ = std::fs::remove_dir_all(&scratch);
        assert!(errors.is_empty(), "{errors:?}");
    }

    /// A shared library names itself; nothing else does.
    ///
    /// Without this a consumer records whatever spelling it linked through, so
    /// linking by absolute path bakes a build-machine path into the consumer.
    #[test]
    fn a_shared_library_plan_sets_its_soname() {
        let mut plan = plan();
        plan.set_soname("libapp.so");
        let args = plan.to_rustc_args();

        let position = args
            .iter()
            .position(|arg| arg == &OsString::from("link-arg=-Wl,-soname"))
            .expect("the soname flag should be present");
        assert_eq!(args[position - 1], OsString::from("-C"));
        assert_eq!(args[position + 1], OsString::from("-C"));
        assert_eq!(args[position + 2], OsString::from("link-arg=-Wl,libapp.so"));
    }

    /// Runtime lookup is not bought with an rpath nobody was told about.
    #[test]
    fn no_plan_injects_an_rpath() {
        let mut plan = plan();
        plan.set_soname("libapp.so");
        plan.push_search_path("/build/lib");
        plan.push(NativeLinkAtom::local_artifact(
            "core".to_string(),
            "libcore.a".to_string(),
            NativeArtifactKind::StaticLibrary,
            target("x86_64-unknown-linux-gnu"),
        ));

        for arg in plan.to_rustc_args() {
            let rendered = arg.to_string_lossy().into_owned();
            assert!(
                !rendered.contains("rpath"),
                "the link plan injected an rpath: {rendered}"
            );
        }
    }

    /// An executable or a static archive has no self-identity to record.
    #[test]
    fn only_a_shared_library_gets_a_soname() {
        assert!(plan()
            .to_rustc_args()
            .iter()
            .all(|arg| !arg.to_string_lossy().contains("soname")));
    }

    /// PE has no SONAME equivalent: a DLL's identity is its file name, so
    /// asking the linker for one would be an error rather than a no-op.
    #[test]
    fn a_windows_target_gets_no_soname_flag() {
        let mut plan = NativeLinkPlan::new("app", target("x86_64-pc-windows-gnu"));
        plan.set_soname("app.dll");
        assert!(plan
            .to_rustc_args()
            .iter()
            .all(|arg| !arg.to_string_lossy().contains("soname")));
    }

    /// The soname changes the produced file, so it must change the fingerprint.
    #[test]
    fn the_soname_is_part_of_the_fingerprint() {
        let bare = plan();
        let mut named = plan();
        named.set_soname("libapp.so");
        assert_ne!(bare.fingerprint_rendering(), named.fingerprint_rendering());
    }

    #[test]
    fn a_well_formed_plan_reports_nothing() {
        let mut plan = plan();
        plan.push(NativeLinkAtom::local_artifact(
            "core",
            "build/libcore.a",
            NativeArtifactKind::StaticLibrary,
            target("x86_64-unknown-linux-gnu"),
        ));
        plan.push(NativeLinkAtom::system_library(
            "ssl",
            NativeLinkMode::Dynamic,
        ));
        assert_eq!(plan.validate(), Vec::new());
    }

    #[test]
    fn an_artifact_may_not_link_itself() {
        let mut plan = plan();
        plan.push(NativeLinkAtom::local_artifact(
            "app",
            "build/libapp.a",
            NativeArtifactKind::StaticLibrary,
            target("x86_64-unknown-linux-gnu"),
        ));
        assert!(plan
            .validate()
            .iter()
            .any(|error| error.kind == LinkPlanErrorKind::SelfLink));
    }

    /// A wrong-target archive must fail before the linker, where the error is
    /// about FOL's own plan rather than a confusing `ld` message.
    #[test]
    fn a_wrong_target_input_fails_before_the_linker() {
        let mut plan = plan();
        plan.push(NativeLinkAtom::local_artifact(
            "core",
            "build/libcore.a",
            NativeArtifactKind::StaticLibrary,
            target("aarch64-unknown-linux-gnu"),
        ));
        let errors = plan.validate();
        assert!(errors
            .iter()
            .any(|error| error.kind == LinkPlanErrorKind::TargetMismatch));
        assert!(errors[0].message.contains("aarch64-unknown-linux-gnu"));
    }

    #[test]
    fn a_header_is_not_a_link_input() {
        let mut plan = plan();
        plan.push(NativeLinkAtom::local_artifact(
            "api",
            "include/api.h",
            NativeArtifactKind::Header,
            target("x86_64-unknown-linux-gnu"),
        ));
        assert!(plan
            .validate()
            .iter()
            .any(|error| error.kind == LinkPlanErrorKind::IncompatibleArtifactKind));
    }

    #[test]
    fn an_apple_framework_fails_on_the_certified_lane() {
        let mut plan = plan();
        plan.push(NativeLinkAtom::system_library(
            "framework:Metal",
            NativeLinkMode::Dynamic,
        ));
        let errors = plan.validate();
        assert!(errors
            .iter()
            .any(|error| error.kind == LinkPlanErrorKind::FrameworkNotSupported));
        assert!(errors[0].message.contains("no Apple lane is promoted"));
    }

    #[test]
    fn a_windows_import_library_fails_until_its_lane_is_promoted() {
        let mut plan = NativeLinkPlan::new("app", target("x86_64-pc-windows-msvc"));
        plan.push(NativeLinkAtom::local_artifact(
            "core",
            "build/core.lib",
            NativeArtifactKind::StaticLibrary,
            target("x86_64-pc-windows-msvc"),
        ));
        assert!(plan
            .validate()
            .iter()
            .any(|error| error.kind == LinkPlanErrorKind::ImportLibraryNotSupported));
    }

    /// Two sources for one library name is ambiguous; a repeated identical atom
    /// is not, because a static link legitimately needs one twice.
    #[test]
    fn two_providers_of_one_name_conflict_but_a_repeat_does_not() {
        let mut plan = plan();
        plan.push(NativeLinkAtom::system_library(
            "ssl",
            NativeLinkMode::Dynamic,
        ));
        plan.push(NativeLinkAtom::system_library(
            "ssl",
            NativeLinkMode::Dynamic,
        ));
        assert!(
            !plan
                .validate()
                .iter()
                .any(|error| error.kind == LinkPlanErrorKind::DuplicateProvider),
            "a repeated identical atom is meaningful, not a duplicate provider"
        );

        plan.push(NativeLinkAtom::local_artifact(
            "ssl",
            "build/libssl.a",
            NativeArtifactKind::StaticLibrary,
            target("x86_64-unknown-linux-gnu"),
        ));
        assert!(plan
            .validate()
            .iter()
            .any(|error| error.kind == LinkPlanErrorKind::DuplicateProvider));
    }

    #[test]
    fn dependents_precede_providers_in_a_multi_level_closure() {
        let mut links = BTreeMap::new();
        links.insert("app".to_string(), vec!["mid".to_string()]);
        links.insert("mid".to_string(), vec!["base".to_string()]);

        let order = order_dependents_before_providers("app", &links).unwrap();
        assert_eq!(order, vec!["app", "mid", "base"]);
    }

    #[test]
    fn a_link_cycle_is_rejected() {
        let mut links = BTreeMap::new();
        links.insert("a".to_string(), vec!["b".to_string()]);
        links.insert("b".to_string(), vec!["a".to_string()]);

        let error =
            order_dependents_before_providers("a", &links).expect_err("a cycle should be rejected");
        assert_eq!(error.kind, LinkPlanErrorKind::LinkCycle);
    }

    /// Ordering is stable across runs, or the build is not reproducible.
    #[test]
    fn link_order_is_deterministic() {
        let mut links = BTreeMap::new();
        links.insert(
            "app".to_string(),
            vec!["alpha".to_string(), "beta".to_string()],
        );
        let first = order_dependents_before_providers("app", &links).unwrap();
        let second = order_dependents_before_providers("app", &links).unwrap();
        assert_eq!(first, second);
        assert_eq!(first, vec!["app", "alpha", "beta"]);
    }

    /// Arguments are separate argv items. A concatenated flag string would let
    /// a path containing a space split itself into two arguments.
    #[test]
    fn arguments_are_structured_never_concatenated() {
        let mut plan = plan();
        plan.push_search_path("/opt/lib with space");
        plan.push(NativeLinkAtom::system_library(
            "ssl",
            NativeLinkMode::Dynamic,
        ));
        plan.push(NativeLinkAtom::local_artifact(
            "core",
            "/build/lib with space/libcore.a",
            NativeArtifactKind::StaticLibrary,
            target("x86_64-unknown-linux-gnu"),
        ));

        let args = plan.to_rustc_args();
        assert_eq!(args[0], OsString::from("-L"));
        assert_eq!(args[1], OsString::from("native=/opt/lib with space"));
        assert_eq!(args[2], OsString::from("-l"));
        assert_eq!(args[3], OsString::from("dylib=ssl"));
        assert_eq!(args[4], OsString::from("-C"));
        assert_eq!(
            args[5],
            OsString::from("link-arg=/build/lib with space/libcore.a")
        );
        // The space stayed inside one argument rather than splitting it.
        assert_eq!(args.len(), 6);
    }

    #[test]
    fn a_static_system_library_asks_for_static_linkage() {
        let mut plan = plan();
        plan.push(NativeLinkAtom::system_library("z", NativeLinkMode::Static));
        assert_eq!(
            plan.to_rustc_args(),
            vec![OsString::from("-l"), OsString::from("static=z")]
        );
    }

    /// Link order reaches the fingerprint: two plans with the same atoms in a
    /// different order can produce different binaries.
    #[test]
    fn link_order_changes_the_fingerprint() {
        let alpha = NativeLinkAtom::system_library("alpha", NativeLinkMode::Dynamic);
        let beta = NativeLinkAtom::system_library("beta", NativeLinkMode::Dynamic);

        let mut first = plan();
        first.push(alpha.clone());
        first.push(beta.clone());

        let mut second = plan();
        second.push(beta);
        second.push(alpha);

        assert_ne!(
            first.fingerprint_rendering(),
            second.fingerprint_rendering()
        );
    }

    /// A dependency export carries what a consumer needs to trust it.
    #[test]
    fn a_dependency_export_carries_path_target_digest_and_provenance() {
        let export = DependencyArtifactExport {
            package: "logtiny".to_string(),
            artifact: "logtiny".to_string(),
            kind: NativeArtifactKind::StaticLibrary,
            role_path: "/store/logtiny/lib/liblogtiny.a".to_string(),
            target: target("x86_64-unknown-linux-gnu"),
            content_digest: "0123456789abcdef".to_string(),
            provenance: "git+https://example.invalid/logtiny#abc123".to_string(),
            transitive_interface: vec![NativeLinkAtom::system_library(
                "m",
                NativeLinkMode::Dynamic,
            )],
        };

        let atom = export.as_atom();
        assert_eq!(
            atom.path.as_deref(),
            Some("/store/logtiny/lib/liblogtiny.a")
        );
        assert_eq!(atom.digest.as_deref(), Some("0123456789abcdef"));
        assert_eq!(
            atom.target.as_ref(),
            Some(&target("x86_64-unknown-linux-gnu"))
        );
        assert_eq!(
            atom.origin,
            LinkAtomOrigin::DependencyArtifact {
                package: "logtiny".to_string(),
                artifact: "logtiny".to_string(),
            }
        );
        // The interface it passes on is what its own consumers must also link.
        assert_eq!(export.transitive_interface.len(), 1);
    }

    /// A dependency-exported archive reaches the link command as its exact
    /// path, not as a `-l` name that could resolve to a different file.
    #[test]
    fn a_dependency_export_reaches_the_link_command_by_exact_path() {
        let export = DependencyArtifactExport {
            package: "logtiny".to_string(),
            artifact: "logtiny".to_string(),
            kind: NativeArtifactKind::StaticLibrary,
            role_path: "/store/logtiny/lib/liblogtiny.a".to_string(),
            target: target("x86_64-unknown-linux-gnu"),
            content_digest: "0123456789abcdef".to_string(),
            provenance: "path+deps/logtiny".to_string(),
            transitive_interface: Vec::new(),
        };
        let mut plan = plan();
        plan.push(export.as_atom());

        assert!(plan.validate().is_empty());
        assert!(plan
            .to_rustc_args()
            .contains(&OsString::from("link-arg=/store/logtiny/lib/liblogtiny.a")));
    }
}

/// Reject a package-owned search path that leaves the package.
///
/// The origin is the whole point and it is lost downstream: by the time a path
/// becomes `-L native=<path>` it is a bare string, so containment has to be
/// decided here while `NativeSearchPathOrigin` still says who owns it. A
/// `System` root may be anywhere -- an SDK is not in the package -- but a
/// `PackageRoot` or `BuildRoot` path that escapes would let the accepted link
/// inputs change without the package's own fingerprint moving.
///
/// A pure string check rather than `canonicalize`: these paths are declared
/// relative to a root that may not exist yet when the plan is built, and a
/// directory that is absent is not the same failure as one that escapes.
fn validate_search_path(
    artifact: &str,
    path: &crate::native::NativeLibraryPath,
) -> Option<LinkPlanError> {
    use crate::native::NativeSearchPathOrigin;

    if matches!(path.origin, NativeSearchPathOrigin::System) {
        return None;
    }
    let candidate = std::path::Path::new(&path.relative_path);
    if candidate.is_absolute() {
        return Some(LinkPlanError::new(
            LinkPlanErrorKind::MissingRole,
            format!(
                "artifact '{artifact}' declares the absolute library search path '{}'; a \
                 package-owned path is relative to the package, and an absolute one describes \
                 this machine rather than this package",
                path.relative_path
            ),
        ));
    }
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Some(LinkPlanError::new(
            LinkPlanErrorKind::MissingRole,
            format!(
                "artifact '{artifact}' declares the library search path '{}', which leaves the \
                 package; declare it as a system search path if it is meant to",
                path.relative_path
            ),
        ));
    }
    None
}

/// Build the ordered link plan for one graph artifact.
///
/// Resolves every handle kind section 4.5 names: locally produced artifacts,
/// dependency exports, exact files, objects, and system libraries. Local
/// providers are ordered dependents-first, then the artifact's own directives
/// follow in declaration order.
pub fn resolve_link_plan(
    graph: &crate::graph::BuildGraph,
    artifact_id: crate::graph::BuildArtifactId,
    dependency_exports: &[DependencyArtifactExport],
) -> Result<NativeLinkPlan, LinkPlanError> {
    let Some(artifact) = graph.artifacts().get(artifact_id.index()) else {
        return Err(LinkPlanError::new(
            LinkPlanErrorKind::MissingRole,
            format!("artifact {artifact_id} is not in the graph"),
        ));
    };
    let mut plan = NativeLinkPlan::new(artifact.name.clone(), artifact.target.clone());
    if matches!(
        artifact.kind,
        crate::graph::BuildArtifactKind::SharedLibrary
    ) {
        plan.set_soname(artifact.target.shared_library_file_name(&artifact.name));
    }

    // Local providers, dependents before providers.
    let mut links: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for candidate in graph.artifacts() {
        let providers: Vec<String> = graph
            .artifact_links_for(candidate.id)
            .filter_map(|id| graph.artifacts().get(id.index()).map(|a| a.name.clone()))
            .collect();
        links.insert(candidate.name.clone(), providers);
    }
    for name in order_dependents_before_providers(&artifact.name, &links)? {
        if name == artifact.name {
            continue;
        }
        let Some(provider) = graph.artifacts().iter().find(|a| a.name == name) else {
            continue;
        };
        let kind = match provider.kind {
            crate::graph::BuildArtifactKind::StaticLibrary => NativeArtifactKind::StaticLibrary,
            crate::graph::BuildArtifactKind::SharedLibrary => NativeArtifactKind::SharedLibrary,
            crate::graph::BuildArtifactKind::Object => NativeArtifactKind::Object,
            // An executable or a test bundle is not linkable. Recorded as a
            // header so `validate_kinds` reports it by name rather than the
            // plan silently dropping it.
            _ => NativeArtifactKind::Header,
        };
        let file_name = match kind {
            NativeArtifactKind::StaticLibrary => provider.target.archive_file_name(&provider.name),
            NativeArtifactKind::SharedLibrary => {
                provider.target.shared_library_file_name(&provider.name)
            }
            _ => provider.name.clone(),
        };
        plan.push(NativeLinkAtom::local_artifact(
            provider.name.clone(),
            file_name,
            kind,
            provider.target.clone(),
        ));
    }

    for export in dependency_exports {
        plan.push(export.as_atom());
        for inherited in &export.transitive_interface {
            plan.push(inherited.clone());
        }
    }

    for path in &artifact.library_paths {
        if let Some(error) = validate_search_path(&artifact.name, path) {
            return Err(error);
        }
        plan.push_search_path(path.relative_path.clone());
    }
    for directive in &artifact.link_inputs {
        match &directive.input {
            crate::native::NativeLinkInput::LibraryName(name) => {
                plan.push(NativeLinkAtom::system_library(name.clone(), directive.mode));
            }
            crate::native::NativeLinkInput::Artifact(definition) => {
                plan.push(NativeLinkAtom {
                    origin: LinkAtomOrigin::ExactFile,
                    name: definition.name.clone(),
                    path: Some(definition.relative_path.clone()),
                    kind: definition.kind,
                    mode: directive.mode,
                    target: Some(artifact.target.clone()),
                    digest: None,
                });
            }
        }
    }
    Ok(plan)
}

#[cfg(test)]
mod resolve_tests {
    use super::*;
    use crate::graph::{BuildArtifactKind, BuildGraph};

    fn target() -> fol_types::ResolvedTarget {
        fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap()
    }

    fn configured(
        graph: &mut BuildGraph,
        kind: BuildArtifactKind,
        name: &str,
    ) -> crate::graph::BuildArtifactId {
        graph.add_configured_artifact(
            kind,
            name,
            "src/lib.fol",
            crate::artifact::BuildArtifactFolModel::Memo,
            target(),
            crate::option::BuildOptimizeMode::Debug,
        )
    }

    /// An executable links a local static library, and the archive reaches the
    /// plan by its target-correct file name.
    #[test]
    fn an_executable_links_a_local_static_library() {
        let mut graph = BuildGraph::new();
        let app = configured(&mut graph, BuildArtifactKind::Executable, "app");
        let core = configured(&mut graph, BuildArtifactKind::StaticLibrary, "core");
        graph.add_artifact_link(app, core);

        let plan = resolve_link_plan(&graph, app, &[]).expect("the plan should resolve");
        assert!(plan.validate().is_empty());
        assert_eq!(plan.atoms.len(), 1);
        assert_eq!(plan.atoms[0].path.as_deref(), Some("libcore.a"));
    }

    /// A multi-level closure keeps dependents before providers.
    #[test]
    fn a_multi_level_static_closure_is_ordered() {
        let mut graph = BuildGraph::new();
        let app = configured(&mut graph, BuildArtifactKind::Executable, "app");
        let mid = configured(&mut graph, BuildArtifactKind::StaticLibrary, "mid");
        let base = configured(&mut graph, BuildArtifactKind::StaticLibrary, "base");
        graph.add_artifact_link(app, mid);
        graph.add_artifact_link(mid, base);

        let plan = resolve_link_plan(&graph, app, &[]).expect("the plan should resolve");
        let names: Vec<&str> = plan.atoms.iter().map(|atom| atom.name.as_str()).collect();
        assert_eq!(names, vec!["mid", "base"]);
    }

    /// A shared library consumes its direct dependencies.
    #[test]
    fn a_shared_library_consumes_its_direct_dependencies() {
        let mut graph = BuildGraph::new();
        let sdk = configured(&mut graph, BuildArtifactKind::SharedLibrary, "sdk");
        let core = configured(&mut graph, BuildArtifactKind::StaticLibrary, "core");
        graph.add_artifact_link(sdk, core);

        let plan = resolve_link_plan(&graph, sdk, &[]).expect("the plan should resolve");
        assert_eq!(plan.atoms[0].name, "core");
        assert!(plan.validate().is_empty());
    }

    /// Linking an executable is rejected rather than silently dropped.
    #[test]
    fn linking_an_executable_is_rejected() {
        let mut graph = BuildGraph::new();
        let app = configured(&mut graph, BuildArtifactKind::Executable, "app");
        let tool = configured(&mut graph, BuildArtifactKind::Executable, "tool");
        graph.add_artifact_link(app, tool);

        let plan = resolve_link_plan(&graph, app, &[]).expect("the plan should resolve");
        assert!(plan
            .validate()
            .iter()
            .any(|error| error.kind == LinkPlanErrorKind::IncompatibleArtifactKind));
    }

    #[test]
    fn a_dependency_export_and_its_interface_both_reach_the_plan() {
        let mut graph = BuildGraph::new();
        let app = configured(&mut graph, BuildArtifactKind::Executable, "app");
        let export = DependencyArtifactExport {
            package: "logtiny".to_string(),
            artifact: "logtiny".to_string(),
            kind: NativeArtifactKind::StaticLibrary,
            role_path: "/store/liblogtiny.a".to_string(),
            target: target(),
            content_digest: "abc".to_string(),
            provenance: "path+deps/logtiny".to_string(),
            transitive_interface: vec![NativeLinkAtom::system_library(
                "m",
                NativeLinkMode::Dynamic,
            )],
        };

        let plan = resolve_link_plan(&graph, app, std::slice::from_ref(&export))
            .expect("the plan should resolve");
        let names: Vec<&str> = plan.atoms.iter().map(|atom| atom.name.as_str()).collect();
        assert_eq!(names, vec!["logtiny", "m"]);
        assert!(plan
            .to_rustc_args()
            .contains(&OsString::from("link-arg=/store/liblogtiny.a")));
    }
}

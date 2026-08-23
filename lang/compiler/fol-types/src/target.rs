//! Canonical, concrete compilation targets shared by the build, frontend, and
//! backend layers.
//!
//! One table, [`TARGETS`], is the authority. Every fact about a target — its
//! canonical triple, its FOL spelling, its object format, how it names an
//! executable or an archive — is a column there rather than a `match` arm
//! somewhere downstream. Before this the triple was the only stored fact and
//! three separate string matches re-derived the rest, which is how a target
//! model grows a second, disagreeing copy of itself.

/// Instruction-set architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetArch {
    X86_64,
    Aarch64,
}

/// The vendor field of the canonical triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetVendor {
    Unknown,
    Pc,
    Apple,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetOs {
    Linux,
    Windows,
    Darwin,
}

/// The environment/ABI field. Darwin encodes none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetEnv {
    Gnu,
    Musl,
    Msvc,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjectFormat {
    Elf,
    Pe,
    MachO,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Endianness {
    Little,
    Big,
}

/// How much of the pipeline is proven for a target.
///
/// The tiers are `plan/V4_PLAN.md` section 16.3. A tier is not a promise that
/// `rustc` accepts the triple — it records whether FOL's own gate builds,
/// links, and runs there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TargetTier {
    /// Release-blocking: the interop gate refuses to run without it.
    Certified,
    /// Resolvable and buildable, not gate-blocking.
    Candidate,
    /// Resolvable so a diagnostic can name it; not advertised for V4.
    Experimental,
}

impl TargetTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Certified => "certified",
            Self::Candidate => "candidate",
            Self::Experimental => "experimental",
        }
    }
}

/// How a target names the files a link step produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetNaming {
    pub executable_suffix: &'static str,
    pub archive_prefix: &'static str,
    pub archive_suffix: &'static str,
    pub shared_prefix: &'static str,
    pub shared_suffix: &'static str,
}

/// Every fact FOL knows about one target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetFacts {
    pub rust_triple: &'static str,
    /// The stable FOL build-option spelling. `build.fol` compares against this,
    /// so it is a compatibility surface, not a cosmetic rendering.
    pub fol_spelling: &'static str,
    pub arch: TargetArch,
    pub vendor: TargetVendor,
    pub os: TargetOs,
    pub env: TargetEnv,
    pub object_format: ObjectFormat,
    pub pointer_width: u16,
    pub endianness: Endianness,
    pub tier: TargetTier,
    pub naming: TargetNaming,
}

const ELF_NAMING: TargetNaming = TargetNaming {
    executable_suffix: "",
    archive_prefix: "lib",
    archive_suffix: ".a",
    shared_prefix: "lib",
    shared_suffix: ".so",
};

const MACHO_NAMING: TargetNaming = TargetNaming {
    executable_suffix: "",
    archive_prefix: "lib",
    archive_suffix: ".a",
    shared_prefix: "lib",
    shared_suffix: ".dylib",
};

const MINGW_NAMING: TargetNaming = TargetNaming {
    executable_suffix: ".exe",
    archive_prefix: "lib",
    archive_suffix: ".a",
    shared_prefix: "",
    shared_suffix: ".dll",
};

const MSVC_NAMING: TargetNaming = TargetNaming {
    executable_suffix: ".exe",
    archive_prefix: "",
    archive_suffix: ".lib",
    shared_prefix: "",
    shared_suffix: ".dll",
};

/// The one target table. Adding a target means adding a row here and nothing
/// else; every accessor below reads from it.
pub const TARGETS: &[TargetFacts] = &[
    TargetFacts {
        rust_triple: "x86_64-unknown-linux-gnu",
        fol_spelling: "x86_64-linux-gnu",
        arch: TargetArch::X86_64,
        vendor: TargetVendor::Unknown,
        os: TargetOs::Linux,
        env: TargetEnv::Gnu,
        object_format: ObjectFormat::Elf,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Certified,
        naming: ELF_NAMING,
    },
    TargetFacts {
        rust_triple: "x86_64-unknown-linux-musl",
        fol_spelling: "x86_64-linux-musl",
        arch: TargetArch::X86_64,
        vendor: TargetVendor::Unknown,
        os: TargetOs::Linux,
        env: TargetEnv::Musl,
        object_format: ObjectFormat::Elf,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Certified,
        naming: ELF_NAMING,
    },
    TargetFacts {
        rust_triple: "aarch64-unknown-linux-gnu",
        fol_spelling: "aarch64-linux-gnu",
        arch: TargetArch::Aarch64,
        vendor: TargetVendor::Unknown,
        os: TargetOs::Linux,
        env: TargetEnv::Gnu,
        object_format: ObjectFormat::Elf,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Candidate,
        naming: ELF_NAMING,
    },
    TargetFacts {
        rust_triple: "aarch64-unknown-linux-musl",
        fol_spelling: "aarch64-linux-musl",
        arch: TargetArch::Aarch64,
        vendor: TargetVendor::Unknown,
        os: TargetOs::Linux,
        env: TargetEnv::Musl,
        object_format: ObjectFormat::Elf,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Candidate,
        naming: ELF_NAMING,
    },
    TargetFacts {
        rust_triple: "x86_64-pc-windows-gnu",
        fol_spelling: "x86_64-windows-gnu",
        arch: TargetArch::X86_64,
        vendor: TargetVendor::Pc,
        os: TargetOs::Windows,
        env: TargetEnv::Gnu,
        object_format: ObjectFormat::Pe,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Experimental,
        naming: MINGW_NAMING,
    },
    TargetFacts {
        rust_triple: "x86_64-pc-windows-msvc",
        fol_spelling: "x86_64-windows-msvc",
        arch: TargetArch::X86_64,
        vendor: TargetVendor::Pc,
        os: TargetOs::Windows,
        env: TargetEnv::Msvc,
        object_format: ObjectFormat::Pe,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Experimental,
        naming: MSVC_NAMING,
    },
    TargetFacts {
        rust_triple: "aarch64-pc-windows-msvc",
        fol_spelling: "aarch64-windows-msvc",
        arch: TargetArch::Aarch64,
        vendor: TargetVendor::Pc,
        os: TargetOs::Windows,
        env: TargetEnv::Msvc,
        object_format: ObjectFormat::Pe,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Experimental,
        naming: MSVC_NAMING,
    },
    TargetFacts {
        rust_triple: "x86_64-apple-darwin",
        fol_spelling: "x86_64-macos-gnu",
        arch: TargetArch::X86_64,
        vendor: TargetVendor::Apple,
        os: TargetOs::Darwin,
        env: TargetEnv::None,
        object_format: ObjectFormat::MachO,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Experimental,
        naming: MACHO_NAMING,
    },
    TargetFacts {
        rust_triple: "aarch64-apple-darwin",
        fol_spelling: "aarch64-macos-gnu",
        arch: TargetArch::Aarch64,
        vendor: TargetVendor::Apple,
        os: TargetOs::Darwin,
        env: TargetEnv::None,
        object_format: ObjectFormat::MachO,
        pointer_width: 64,
        endianness: Endianness::Little,
        tier: TargetTier::Experimental,
        naming: MACHO_NAMING,
    },
];

fn facts_for(rust_triple: &str) -> Option<&'static TargetFacts> {
    TARGETS
        .iter()
        .find(|facts| facts.rust_triple == rust_triple)
}

/// A supported machine target resolved to the exact Rust target triple used by
/// the backend and by interop fingerprints.
///
/// This type never represents `host`, `native`, an unknown spelling, or an
/// omitted target. Those inputs are resolved at the boundary before a value is
/// constructed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedTarget {
    facts: &'static TargetFacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveTargetError {
    UnsupportedTarget(String),
    UnsupportedHost,
    /// The target resolves, and V4 does not build for it. Kept distinct from
    /// `UnsupportedTarget` so the diagnostic can say the name was understood.
    ExperimentalTarget(String),
}

impl std::fmt::Display for ResolveTargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget(target) => {
                write!(f, "unsupported explicit machine target '{target}'")
            }
            Self::UnsupportedHost => write!(
                f,
                "the current host does not map to a supported concrete machine target"
            ),
            Self::ExperimentalTarget(target) => write!(
                f,
                "machine target '{target}' is experimental and is not built by V4; \
                 the certified targets are {}",
                super::target::certified_target_spellings().join(", ")
            ),
        }
    }
}

impl std::error::Error for ResolveTargetError {}

impl ResolvedTarget {
    /// Resolve a host alias, FOL target spelling, or supported Rust target
    /// triple. Unknown inputs fail instead of falling back to the host.
    pub fn resolve(raw: &str) -> Result<Self, ResolveTargetError> {
        let raw = raw.trim();
        if matches!(raw, "host" | "native") {
            return Self::host();
        }
        if raw.is_empty() {
            return Err(ResolveTargetError::UnsupportedTarget(String::new()));
        }

        TARGETS
            .iter()
            .find(|facts| facts.rust_triple == raw || facts.fol_spelling == raw)
            .map(|facts| Self { facts })
            .ok_or_else(|| ResolveTargetError::UnsupportedTarget(raw.to_string()))
    }

    /// Compatibility parser for existing option APIs. New boundary code
    /// should use [`Self::resolve`] so it can preserve the failure reason.
    pub fn parse(raw: &str) -> Option<Self> {
        Self::resolve(raw).ok()
    }

    pub fn host() -> Result<Self, ResolveTargetError> {
        let rust_triple = Self::host_rust_triple()?;
        facts_for(rust_triple)
            .map(|facts| Self { facts })
            .ok_or(ResolveTargetError::UnsupportedHost)
    }

    pub fn host_rust_triple() -> Result<&'static str, ResolveTargetError> {
        let arch = if cfg!(target_arch = "x86_64") {
            TargetArch::X86_64
        } else if cfg!(target_arch = "aarch64") {
            TargetArch::Aarch64
        } else {
            return Err(ResolveTargetError::UnsupportedHost);
        };
        let os = if cfg!(target_os = "linux") {
            TargetOs::Linux
        } else if cfg!(target_os = "windows") {
            TargetOs::Windows
        } else if cfg!(target_os = "macos") {
            TargetOs::Darwin
        } else {
            return Err(ResolveTargetError::UnsupportedHost);
        };
        // Darwin carries no environment field, so it matches on arch and OS
        // alone; every other host must agree on all three.
        let env = if matches!(os, TargetOs::Darwin) {
            TargetEnv::None
        } else if cfg!(target_env = "gnu") {
            TargetEnv::Gnu
        } else if cfg!(target_env = "musl") {
            TargetEnv::Musl
        } else if cfg!(target_env = "msvc") {
            TargetEnv::Msvc
        } else {
            return Err(ResolveTargetError::UnsupportedHost);
        };

        TARGETS
            .iter()
            .find(|facts| facts.arch == arch && facts.os == os && facts.env == env)
            .map(|facts| facts.rust_triple)
            .ok_or(ResolveTargetError::UnsupportedHost)
    }

    pub fn as_str(&self) -> &str {
        self.facts.rust_triple
    }

    /// Render the stable FOL build-option spelling.
    ///
    /// Build programs compare `standard_target()` values with compact
    /// spellings such as `x86_64-linux-gnu`. Backend and interop consumers must
    /// use [`Self::rust_target_triple`] instead; changing the build-language
    /// value to a Rust vendor triple would silently change `when(target == ...)`
    /// and `case(...)` behavior.
    pub fn render(&self) -> String {
        self.facts.fol_spelling.to_string()
    }

    pub fn rust_target_triple(&self) -> &str {
        self.as_str()
    }

    pub fn rust_target_directory_name(&self) -> &str {
        self.as_str()
    }

    pub fn runs_on_host(&self) -> Result<bool, ResolveTargetError> {
        Ok(*self == Self::host()?)
    }

    /// Every fact this target carries, for a consumer that needs several.
    pub fn facts(&self) -> &'static TargetFacts {
        self.facts
    }

    pub fn arch(&self) -> TargetArch {
        self.facts.arch
    }

    pub fn vendor(&self) -> TargetVendor {
        self.facts.vendor
    }

    pub fn os(&self) -> TargetOs {
        self.facts.os
    }

    pub fn env(&self) -> TargetEnv {
        self.facts.env
    }

    pub fn object_format(&self) -> ObjectFormat {
        self.facts.object_format
    }

    pub fn pointer_width(&self) -> u16 {
        self.facts.pointer_width
    }

    pub fn endianness(&self) -> Endianness {
        self.facts.endianness
    }

    pub fn tier(&self) -> TargetTier {
        self.facts.tier
    }

    pub fn naming(&self) -> TargetNaming {
        self.facts.naming
    }

    /// Whether V4 builds for this target at all.
    ///
    /// An experimental target resolves so that a diagnostic can name it, and
    /// stops there. Letting one through means creating output directories and
    /// launching `rustc`, which then fails with its own error about a missing
    /// standard library and advice to run `rustup` -- advice this project does
    /// not follow.
    pub fn is_buildable(&self) -> bool {
        !matches!(self.facts.tier, TargetTier::Experimental)
    }

    /// Fail unless V4 builds for this target. Call at the boundary, before any
    /// output directory or tool process exists.
    pub fn ensure_buildable(&self) -> Result<(), ResolveTargetError> {
        if self.is_buildable() {
            Ok(())
        } else {
            Err(ResolveTargetError::ExperimentalTarget(
                self.as_str().to_string(),
            ))
        }
    }

    /// The file name a link step produces for an executable artifact.
    pub fn executable_file_name(&self, artifact: &str) -> String {
        format!("{artifact}{}", self.facts.naming.executable_suffix)
    }

    /// The file name a link step produces for a static library artifact.
    pub fn archive_file_name(&self, artifact: &str) -> String {
        format!(
            "{}{artifact}{}",
            self.facts.naming.archive_prefix, self.facts.naming.archive_suffix
        )
    }

    /// The file name a link step produces for a shared library artifact.
    pub fn shared_library_file_name(&self, artifact: &str) -> String {
        format!(
            "{}{artifact}{}",
            self.facts.naming.shared_prefix, self.facts.naming.shared_suffix
        )
    }
}

impl std::fmt::Display for ResolvedTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
#[cfg(test)]
mod tests {
    use super::{ResolveTargetError, ResolvedTarget};

    #[test]
    fn resolves_all_supported_spellings_to_rust_triples() {
        assert_eq!(
            ResolvedTarget::resolve("x86_64-linux-gnu")
                .unwrap()
                .as_str(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            ResolvedTarget::resolve(" aarch64-apple-darwin ")
                .unwrap()
                .as_str(),
            "aarch64-apple-darwin"
        );
    }

    #[test]
    fn renders_stable_fol_option_values_without_losing_rust_identity() {
        let target = ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap();

        assert_eq!(target.render(), "x86_64-linux-gnu");
        assert_eq!(target.rust_target_triple(), "x86_64-unknown-linux-gnu");
        assert_eq!(ResolvedTarget::resolve(&target.render()).unwrap(), target);
    }

    #[test]
    fn host_aliases_resolve_to_one_concrete_value() {
        let host = ResolvedTarget::host().unwrap();
        assert_eq!(ResolvedTarget::resolve("host").unwrap(), host);
        assert_eq!(ResolvedTarget::resolve("native").unwrap(), host);
        assert!(!host.as_str().is_empty());
    }

    #[test]
    fn unknown_targets_do_not_fall_back() {
        assert_eq!(
            ResolvedTarget::resolve("mystery-vendor-os"),
            Err(ResolveTargetError::UnsupportedTarget(
                "mystery-vendor-os".to_string()
            ))
        );
        assert_eq!(
            ResolvedTarget::resolve("   "),
            Err(ResolveTargetError::UnsupportedTarget(String::new()))
        );
    }
}

#[cfg(test)]
mod fact_tests {
    use super::{
        Endianness, ObjectFormat, ResolvedTarget, TargetArch, TargetEnv, TargetOs, TargetTier,
        TargetVendor, TARGETS,
    };

    #[test]
    fn every_target_row_is_internally_consistent() {
        for facts in TARGETS {
            // The triple is the authority; the fields must agree with it.
            let (arch, rest) = facts
                .rust_triple
                .split_once('-')
                .expect("a triple has at least two fields");
            assert_eq!(
                arch,
                match facts.arch {
                    TargetArch::X86_64 => "x86_64",
                    TargetArch::Aarch64 => "aarch64",
                },
                "{} disagrees with its arch field",
                facts.rust_triple
            );

            let vendor = rest.split('-').next().expect("a triple has a vendor");
            assert_eq!(
                vendor,
                match facts.vendor {
                    TargetVendor::Unknown => "unknown",
                    TargetVendor::Pc => "pc",
                    TargetVendor::Apple => "apple",
                },
                "{} disagrees with its vendor field",
                facts.rust_triple
            );

            // Object format follows from the OS, and naming follows from the
            // object format, so a mismatched row is a typo rather than a
            // legitimate variation.
            let expected_format = match facts.os {
                TargetOs::Linux => ObjectFormat::Elf,
                TargetOs::Windows => ObjectFormat::Pe,
                TargetOs::Darwin => ObjectFormat::MachO,
            };
            assert_eq!(
                facts.object_format, expected_format,
                "{} has the wrong object format for its OS",
                facts.rust_triple
            );

            assert_eq!(
                facts.pointer_width, 64,
                "{} is not 64-bit",
                facts.rust_triple
            );
            assert_eq!(facts.endianness, Endianness::Little);
            assert_eq!(
                matches!(facts.os, TargetOs::Darwin),
                matches!(facts.env, TargetEnv::None),
                "{} must encode no environment exactly when it is Darwin",
                facts.rust_triple
            );

            // Both spellings must resolve, and to this same row.
            for spelling in [facts.rust_triple, facts.fol_spelling] {
                let resolved =
                    ResolvedTarget::resolve(spelling).expect("a table spelling should resolve");
                assert_eq!(resolved.as_str(), facts.rust_triple);
            }
        }
    }

    #[test]
    fn table_spellings_are_unique() {
        let mut seen = Vec::new();
        for facts in TARGETS {
            for spelling in [facts.rust_triple, facts.fol_spelling] {
                assert!(
                    !seen.contains(&spelling),
                    "'{spelling}' appears twice; resolution would depend on table order"
                );
                seen.push(spelling);
            }
        }
    }

    #[test]
    fn certified_tier_matches_the_gated_lanes() {
        // plan/V4_PLAN.md section 16.3: the gnu and musl x86_64 Linux lanes are
        // release-blocking. A second Linux architecture is a candidate, and
        // Windows and Darwin are experimental.
        let certified: Vec<&str> = TARGETS
            .iter()
            .filter(|facts| facts.tier == TargetTier::Certified)
            .map(|facts| facts.rust_triple)
            .collect();
        assert_eq!(
            certified,
            vec!["x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"]
        );

        for triple in ["x86_64-pc-windows-msvc", "aarch64-apple-darwin"] {
            let target = ResolvedTarget::resolve(triple).expect("should resolve");
            assert_eq!(
                target.tier(),
                TargetTier::Experimental,
                "{triple} must stay experimental until it is promoted"
            );
        }
    }

    #[test]
    fn link_output_names_follow_the_target_convention() {
        let linux = ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap();
        assert_eq!(linux.executable_file_name("app"), "app");
        assert_eq!(linux.archive_file_name("core"), "libcore.a");
        assert_eq!(linux.shared_library_file_name("core"), "libcore.so");

        let darwin = ResolvedTarget::resolve("aarch64-apple-darwin").unwrap();
        assert_eq!(darwin.shared_library_file_name("core"), "libcore.dylib");

        // MSVC drops the `lib` prefix and uses `.lib` for the archive, which is
        // why naming cannot be derived from the object format alone.
        let msvc = ResolvedTarget::resolve("x86_64-pc-windows-msvc").unwrap();
        assert_eq!(msvc.executable_file_name("app"), "app.exe");
        assert_eq!(msvc.archive_file_name("core"), "core.lib");
        assert_eq!(msvc.shared_library_file_name("core"), "core.dll");

        // MinGW keeps the ELF-style archive but the PE-style shared library.
        let mingw = ResolvedTarget::resolve("x86_64-pc-windows-gnu").unwrap();
        assert_eq!(mingw.archive_file_name("core"), "libcore.a");
        assert_eq!(mingw.shared_library_file_name("core"), "core.dll");
    }
}

/// The canonical spellings of every certified target, for diagnostics.
pub fn certified_target_spellings() -> Vec<&'static str> {
    TARGETS
        .iter()
        .filter(|facts| facts.tier == TargetTier::Certified)
        .map(|facts| facts.rust_triple)
        .collect()
}

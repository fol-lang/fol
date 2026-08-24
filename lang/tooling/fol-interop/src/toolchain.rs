use std::path::{Component, PathBuf};

use fol_types::ResolvedTarget;
use linc::native::{CertificationToolchain, NativeError};
use parc::contract::{
    Architecture, CDataModel, CDataModelClass, CharSignedness, CompilerFamily, CompilerIdentity,
    Endian, Environment, ExtensionFamily, ExtensionProfile, FloatingFormat, FloatingLayout,
    IntegerLayout, LanguageStandard, NormalizedCompilerArg, ObjectFormat, OperatingSystem,
    ScalarLayout, SignedIntegerRepresentation, Signedness, TargetSpec, TargetSpecParts, Vendor,
};

use crate::analysis::certification_resource_limits;

/// Exact operational C compiler identity paired with PARC's canonical target
/// value. GCC and clang are both accepted; LINC observes and fingerprints which.
///
/// Construction invokes the supplied compiler directly. No shell, ambient
/// compiler lookup, target fallback, or caller-provided fingerprint is used.
#[derive(Debug, Clone)]
pub(crate) struct CertifiedCToolchain {
    certification: CertificationToolchain,
    target: TargetSpec,
}

impl CertifiedCToolchain {
    /// Observe an explicit C compiler for the selected concrete FOL target.
    ///
    /// Compiler identity, target, sysroot, and executable bytes are observed by
    /// LINC's bounded production API. FOL does not run its own identity probes.
    pub fn observe(
        selected_target: &ResolvedTarget,
        compiler_executable: impl Into<PathBuf>,
        dialect: Option<&str>,
    ) -> Result<Self, InteropToolchainError> {
        if !crate::is_certified_interop_target(selected_target.as_str()) {
            return Err(InteropToolchainError::UnsupportedTarget(
                selected_target.as_str().to_owned(),
            ));
        }

        let supplied = compiler_executable.into();
        if !supplied.is_absolute()
            || supplied
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(InteropToolchainError::InvalidCompilerPath(supplied));
        }
        let compiler_executable =
            std::fs::canonicalize(&supplied).map_err(|source| InteropToolchainError::Io {
                operation: "canonicalize compiler executable",
                path: supplied.clone(),
                source,
            })?;
        if !compiler_executable.is_file() {
            return Err(InteropToolchainError::InvalidCompilerPath(
                compiler_executable,
            ));
        }
        let certification = CertificationToolchain::observe(
            compiler_executable,
            Vec::new(),
            certification_resource_limits()?,
        )?;
        // LINC observes and fingerprints the compiler either way, and its
        // certification profile already accepts both families.
        if !matches!(
            certification.compiler_identity().family(),
            CompilerFamily::Gcc | CompilerFamily::Clang
        ) {
            return Err(InteropToolchainError::CompilerFamilyMismatch(
                certification.compiler_identity().family(),
            ));
        }
        if let Some(sysroot) = certification.compiler_sysroot() {
            return Err(InteropToolchainError::CompilerSysrootUnsupported(
                sysroot.to_owned(),
            ));
        }
        let target = certified_target(
            selected_target.as_str(),
            certification.compiler_identity().clone(),
            dialect,
        )?;

        Ok(Self {
            certification,
            target,
        })
    }

    pub fn target(&self) -> &TargetSpec {
        &self.target
    }

    pub(crate) const fn certification(&self) -> &CertificationToolchain {
        &self.certification
    }
}

#[derive(Debug)]
pub enum InteropToolchainError {
    UnsupportedTarget(String),
    InvalidCompilerPath(PathBuf),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    CompilerFamilyMismatch(CompilerFamily),
    CompilerSysrootUnsupported(PathBuf),
    InvalidTarget(String),
    /// A `dialect` the build program declared that is not a C standard.
    UnknownDialect(String),
    Native(NativeError),
    Contract(linc::contract::ContractError),
}

impl std::fmt::Display for InteropToolchainError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget(target) => write!(
                formatter,
                "FOL interop is not certified for target '{target}'; expected one of {}",
                crate::CERTIFIED_INTEROP_TARGETS.join(", ")
            ),
            Self::InvalidCompilerPath(path) => write!(
                formatter,
                "interop compiler must be an absolute regular file: {}",
                path.display()
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "could not {operation} {}: {source}", path.display()),
            Self::CompilerFamilyMismatch(family) => write!(
                formatter,
                "certified FOL interop requires GCC, but LINC observed {family:?}"
            ),
            Self::CompilerSysrootUnsupported(path) => write!(
                formatter,
                "certified FOL interop requires the compiler's default empty sysroot identity, not {}",
                path.display()
            ),
            Self::InvalidTarget(detail) => write!(formatter, "invalid interop target: {detail}"),
            Self::UnknownDialect(dialect) => write!(
                formatter,
                "'{dialect}' is not a C standard; expected one of {}",
                SUPPORTED_DIALECTS.join(", ")
            ),
            Self::Native(error) => write!(formatter, "LINC compiler observation failed: {error}"),
            Self::Contract(error) => write!(formatter, "invalid LINC probe limits: {error}"),
        }
    }
}

impl std::error::Error for InteropToolchainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Native(error) => Some(error),
            Self::Contract(error) => Some(error),
            _ => None,
        }
    }
}

impl From<NativeError> for InteropToolchainError {
    fn from(error: NativeError) -> Self {
        Self::Native(error)
    }
}

impl From<linc::contract::ContractError> for InteropToolchainError {
    fn from(error: linc::contract::ContractError) -> Self {
        Self::Contract(error)
    }
}

/// The C standards a build program may name, in the spelling it writes.
pub(crate) const SUPPORTED_DIALECTS: &[&str] = &["c89", "c95", "c99", "c11", "c17", "c23"];

/// The declared dialect, or C17 when the build program named none.
fn language_standard(dialect: Option<&str>) -> Result<LanguageStandard, InteropToolchainError> {
    let Some(dialect) = dialect else {
        return Ok(LanguageStandard::C17);
    };
    match dialect {
        "c89" => Ok(LanguageStandard::C89),
        "c95" => Ok(LanguageStandard::C95),
        "c99" => Ok(LanguageStandard::C99),
        "c11" => Ok(LanguageStandard::C11),
        "c17" => Ok(LanguageStandard::C17),
        "c23" => Ok(LanguageStandard::C23),
        other => Err(InteropToolchainError::UnknownDialect(other.to_owned())),
    }
}

/// The certified `TargetSpec` for the selected triple.
///
/// The shared columns -- architecture, vendor, OS, environment, object format,
/// endianness, pointer width -- are read from `fol_types::ResolvedTarget`
/// rather than restated here, so FOL and PARC cannot disagree about the same
/// target. What stays explicit is the part PARC has and FOL does not: the C
/// data model, language standard, extension profile, and ABI flags. Section 4.4
/// of plan/V4_PLAN.md requires exactly this split.
fn certified_target(
    triple: &str,
    compiler: CompilerIdentity,
    dialect: Option<&str>,
) -> Result<TargetSpec, InteropToolchainError> {
    let resolved = fol_types::ResolvedTarget::resolve(triple)
        .map_err(|_| InteropToolchainError::UnsupportedTarget(triple.to_owned()))?;
    if !crate::is_certified_interop_target(resolved.as_str()) {
        return Err(InteropToolchainError::UnsupportedTarget(triple.to_owned()));
    }

    let architecture = match resolved.arch() {
        fol_types::TargetArch::X86_64 => Architecture::X86_64,
        fol_types::TargetArch::Aarch64 => Architecture::Aarch64,
    };
    let operating_system = match resolved.os() {
        fol_types::TargetOs::Linux => OperatingSystem::Linux,
        other => {
            return Err(InteropToolchainError::UnsupportedTarget(format!(
                "{triple}: {other:?} is not a certified interop OS"
            )))
        }
    };
    let environment = match resolved.env() {
        fol_types::TargetEnv::Gnu => Environment::Gnu,
        fol_types::TargetEnv::Musl => Environment::Musl,
        other => {
            return Err(InteropToolchainError::UnsupportedTarget(format!(
                "{triple}: {other:?} is not a certified interop environment"
            )))
        }
    };
    let object_format = match resolved.object_format() {
        fol_types::ObjectFormat::Elf => ObjectFormat::Elf,
        other => {
            return Err(InteropToolchainError::UnsupportedTarget(format!(
                "{triple}: {other:?} is not a certified interop object format"
            )))
        }
    };
    let endian = match resolved.endianness() {
        fol_types::Endianness::Little => Endian::Little,
        fol_types::Endianness::Big => Endian::Big,
    };

    TargetSpec::try_new(TargetSpecParts {
        triple: resolved.as_str().to_owned(),
        architecture,
        vendor: Vendor::try_new(match resolved.vendor() {
            fol_types::TargetVendor::Unknown => "unknown",
            fol_types::TargetVendor::Pc => "pc",
            fol_types::TargetVendor::Apple => "apple",
        })
        .map_err(|error| InteropToolchainError::InvalidTarget(error.to_string()))?,
        operating_system,
        environment,
        object_format,
        endian,
        pointer_width: resolved.pointer_width(),
        c_data_model: lp64_data_model(),
        language_standard: language_standard(dialect)?,
        extension_profile: ExtensionProfile::new(ExtensionFamily::Gnu, []),
        compiler,
        sysroot: None,
        abi_flags: vec![NormalizedCompilerArg::try_new("-m64")
            .map_err(|error| InteropToolchainError::InvalidTarget(error.to_string()))?],
    })
    .map_err(|error| InteropToolchainError::InvalidTarget(error.to_string()))
}

fn scalar(storage_bits: u16, alignment_bits: u16) -> ScalarLayout {
    ScalarLayout {
        storage_bits,
        alignment_bits,
    }
}

fn integer(storage_bits: u16, alignment_bits: u16, signedness: Signedness) -> IntegerLayout {
    IntegerLayout {
        scalar: scalar(storage_bits, alignment_bits),
        signedness,
        representation: SignedIntegerRepresentation::TwosComplement,
    }
}

fn lp64_data_model() -> CDataModel {
    CDataModel {
        class: CDataModelClass::LP64,
        char_bit: 8,
        char_signedness: CharSignedness::Signed,
        signed_integer_representation: SignedIntegerRepresentation::TwosComplement,
        bool_layout: scalar(8, 8),
        char_layout: scalar(8, 8),
        short_layout: scalar(16, 16),
        int_layout: scalar(32, 32),
        long_layout: scalar(64, 64),
        long_long_layout: scalar(64, 64),
        int128_layout: Some(scalar(128, 128)),
        pointer_layout: scalar(64, 64),
        float_layout: FloatingLayout {
            scalar: scalar(32, 32),
            format: FloatingFormat::IeeeBinary32,
        },
        double_layout: FloatingLayout {
            scalar: scalar(64, 64),
            format: FloatingFormat::IeeeBinary64,
        },
        long_double_layout: FloatingLayout {
            scalar: scalar(128, 128),
            format: FloatingFormat::X87Extended80,
        },
        wchar_layout: integer(32, 32, Signedness::Signed),
        size_t_layout: integer(64, 64, Signedness::Unsigned),
        ptrdiff_t_layout: integer(64, 64, Signedness::Signed),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use fol_types::ResolvedTarget;

    use super::{CertifiedCToolchain, InteropToolchainError, LanguageStandard};
    use crate::CERTIFIED_INTEROP_TARGETS;

    pub(crate) fn synthetic_target() -> parc::contract::TargetSpec {
        let compiler = parc::contract::CompilerIdentity::try_new(
            parc::contract::CompilerFamily::Gcc,
            "toolchains/gcc/bin/gcc",
            parc::contract::ContentFingerprint::from_content(b"test compiler"),
            parc::contract::ContentFingerprint::from_content(b"test compiler version"),
            CERTIFIED_INTEROP_TARGETS[0],
            "test compiler",
        )
        .unwrap();
        super::certified_target(CERTIFIED_INTEROP_TARGETS[0], compiler, None).unwrap()
    }

    #[test]
    fn rejects_uncertified_target_before_compiler_io() {
        let target = ResolvedTarget::resolve("aarch64-unknown-linux-gnu").unwrap();
        let error =
            CertifiedCToolchain::observe(&target, PathBuf::from("not-absolute"), None).unwrap_err();
        assert!(matches!(error, InteropToolchainError::UnsupportedTarget(_)));
    }

    #[test]
    fn rejects_relative_compiler_before_invocation() {
        let target = ResolvedTarget::resolve(CERTIFIED_INTEROP_TARGETS[0]).unwrap();
        let error = CertifiedCToolchain::observe(&target, PathBuf::from("gcc"), None).unwrap_err();
        assert!(matches!(
            error,
            InteropToolchainError::InvalidCompilerPath(_)
        ));
    }

    /// A declared `dialect` reaches PARC's target rather than being stored and
    /// ignored, which is what it was: every scan ran as C17 whatever the build
    /// program said.
    #[test]
    fn the_declared_dialect_reaches_the_target() {
        let compiler = || {
            parc::contract::CompilerIdentity::try_new(
                parc::contract::CompilerFamily::Gcc,
                "toolchains/gcc/bin/gcc",
                parc::contract::ContentFingerprint::from_content(b"test compiler"),
                parc::contract::ContentFingerprint::from_content(b"test compiler version"),
                CERTIFIED_INTEROP_TARGETS[0],
                "test compiler",
            )
            .unwrap()
        };

        for (spelling, expected) in [
            ("c89", LanguageStandard::C89),
            ("c99", LanguageStandard::C99),
            ("c11", LanguageStandard::C11),
            ("c23", LanguageStandard::C23),
        ] {
            let target =
                super::certified_target(CERTIFIED_INTEROP_TARGETS[0], compiler(), Some(spelling))
                    .unwrap();
            assert_eq!(target.language_standard(), expected, "for {spelling}");
        }

        // No declaration is C17, and the target must say so rather than
        // carrying an absent value.
        let target =
            super::certified_target(CERTIFIED_INTEROP_TARGETS[0], compiler(), None).unwrap();
        assert_eq!(target.language_standard(), LanguageStandard::C17);
    }

    /// An unrecognized dialect is refused by name instead of falling back.
    #[test]
    fn an_unknown_dialect_is_refused_by_name() {
        let compiler = parc::contract::CompilerIdentity::try_new(
            parc::contract::CompilerFamily::Gcc,
            "toolchains/gcc/bin/gcc",
            parc::contract::ContentFingerprint::from_content(b"test compiler"),
            parc::contract::ContentFingerprint::from_content(b"test compiler version"),
            CERTIFIED_INTEROP_TARGETS[0],
            "test compiler",
        )
        .unwrap();
        let error = super::certified_target(CERTIFIED_INTEROP_TARGETS[0], compiler, Some("gnu17"))
            .unwrap_err();
        assert!(matches!(error, InteropToolchainError::UnknownDialect(ref d) if d == "gnu17"));
        assert!(error.to_string().contains("c17"), "{error}");
    }
}

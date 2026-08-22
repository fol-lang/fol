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

/// The certified `TargetSpec` for the selected triple. The libc decides the
/// environment; everything else is identical because glibc and musl share the
/// SysV AMD64 ABI.
fn certified_target(
    triple: &str,
    compiler: CompilerIdentity,
) -> Result<TargetSpec, InteropToolchainError> {
    let environment = match triple {
        "x86_64-unknown-linux-gnu" => Environment::Gnu,
        "x86_64-unknown-linux-musl" => Environment::Musl,
        other => return Err(InteropToolchainError::UnsupportedTarget(other.to_owned())),
    };
    TargetSpec::try_new(TargetSpecParts {
        triple: triple.to_owned(),
        architecture: Architecture::X86_64,
        vendor: Vendor::try_new("unknown")
            .map_err(|error| InteropToolchainError::InvalidTarget(error.to_string()))?,
        operating_system: OperatingSystem::Linux,
        environment,
        object_format: ObjectFormat::Elf,
        endian: Endian::Little,
        pointer_width: 64,
        c_data_model: lp64_data_model(),
        language_standard: LanguageStandard::C17,
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

    use super::{CertifiedCToolchain, InteropToolchainError};
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
        super::certified_target(CERTIFIED_INTEROP_TARGETS[0], compiler).unwrap()
    }

    #[test]
    fn rejects_uncertified_target_before_compiler_io() {
        let target = ResolvedTarget::resolve("aarch64-unknown-linux-gnu").unwrap();
        let error =
            CertifiedCToolchain::observe(&target, PathBuf::from("not-absolute")).unwrap_err();
        assert!(matches!(error, InteropToolchainError::UnsupportedTarget(_)));
    }

    #[test]
    fn rejects_relative_compiler_before_invocation() {
        let target = ResolvedTarget::resolve(CERTIFIED_INTEROP_TARGETS[0]).unwrap();
        let error = CertifiedCToolchain::observe(&target, PathBuf::from("gcc")).unwrap_err();
        assert!(matches!(
            error,
            InteropToolchainError::InvalidCompilerPath(_)
        ));
    }
}

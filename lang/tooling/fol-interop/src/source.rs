use std::path::{Path, PathBuf};

use parc::{
    contract::{CompleteSourcePackage, IncompleteSource, Selection, TargetSpec},
    scan::{
        scan_headers, PathMapping, PathMappingError, PathMappingRule, PreprocessorMode, ScanConfig,
        ScanConfigError, ScanError,
    },
};

/// Scan one package-local header through PARC and require a complete supported
/// declaration closure before any native inspection or generated-file work.
pub(crate) fn scan_complete_header(
    package_root: &Path,
    header: &Path,
    target: &TargetSpec,
) -> Result<CompleteSourcePackage, InteropSourceError> {
    let root = canonical_directory(package_root)?;
    let header = canonical_file(header)?;
    if !header.starts_with(&root) {
        return Err(InteropSourceError::HeaderOutsidePackage { root, header });
    }
    let mapping = PathMapping::try_new([PathMappingRule::try_new(&root, "package")?])?;
    let config =
        ScanConfig::new(target.clone(), mapping, PreprocessorMode::Builtin)?.entry_header(header);
    let report = scan_headers(&config)?;
    let unsupported_declarations = report
        .package()
        .declarations()
        .iter()
        .filter(|declaration| !declaration.support.is_supported())
        .count();
    let unsupported_macros = report
        .package()
        .macros()
        .iter()
        .filter(|macro_item| !macro_item.support.is_supported())
        .count();
    if unsupported_declarations != 0 || unsupported_macros != 0 {
        return Err(InteropSourceError::UnsupportedSource {
            declarations: unsupported_declarations,
            macros: unsupported_macros,
        });
    }
    report
        .into_complete(&Selection::all_supported())
        .map_err(InteropSourceError::Incomplete)
}

#[derive(Debug)]
pub enum InteropSourceError {
    InvalidPackageRoot(PathBuf),
    InvalidHeader(PathBuf),
    HeaderOutsidePackage {
        root: PathBuf,
        header: PathBuf,
    },
    UnsupportedSource {
        declarations: usize,
        macros: usize,
    },
    Io {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    PathMapping(PathMappingError),
    Configuration(ScanConfigError),
    Scan(ScanError),
    Incomplete(IncompleteSource),
}

impl std::fmt::Display for InteropSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPackageRoot(path) => write!(
                formatter,
                "interop package root must be an absolute directory: {}",
                path.display()
            ),
            Self::InvalidHeader(path) => write!(
                formatter,
                "interop header must be an absolute regular file: {}",
                path.display()
            ),
            Self::HeaderOutsidePackage { root, header } => write!(
                formatter,
                "interop header {} escapes package root {}",
                header.display(),
                root.display()
            ),
            Self::UnsupportedSource {
                declarations,
                macros,
            } => write!(
                formatter,
                "PARC source contains {declarations} unsupported declaration(s) and {macros} unsupported macro(s)"
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "could not {operation} {}: {source}", path.display()),
            Self::PathMapping(error) => write!(formatter, "invalid PARC path mapping: {error}"),
            Self::Configuration(error) => write!(formatter, "invalid PARC scan config: {error}"),
            Self::Scan(error) => write!(formatter, "PARC source scan failed: {error}"),
            Self::Incomplete(error) => write!(formatter, "PARC source is incomplete: {error}"),
        }
    }
}

impl std::error::Error for InteropSourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::PathMapping(error) => Some(error),
            Self::Configuration(error) => Some(error),
            Self::Scan(error) => Some(error),
            Self::Incomplete(error) => Some(error),
            Self::InvalidPackageRoot(_)
            | Self::InvalidHeader(_)
            | Self::HeaderOutsidePackage { .. }
            | Self::UnsupportedSource { .. } => None,
        }
    }
}

impl From<PathMappingError> for InteropSourceError {
    fn from(error: PathMappingError) -> Self {
        Self::PathMapping(error)
    }
}

impl From<ScanConfigError> for InteropSourceError {
    fn from(error: ScanConfigError) -> Self {
        Self::Configuration(error)
    }
}

impl From<ScanError> for InteropSourceError {
    fn from(error: ScanError) -> Self {
        Self::Scan(error)
    }
}

fn canonical_directory(path: &Path) -> Result<PathBuf, InteropSourceError> {
    if !path.is_absolute() {
        return Err(InteropSourceError::InvalidPackageRoot(path.to_owned()));
    }
    let canonical = std::fs::canonicalize(path).map_err(|source| InteropSourceError::Io {
        operation: "canonicalize package root",
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_dir() {
        return Err(InteropSourceError::InvalidPackageRoot(canonical));
    }
    Ok(canonical)
}

fn canonical_file(path: &Path) -> Result<PathBuf, InteropSourceError> {
    if !path.is_absolute() {
        return Err(InteropSourceError::InvalidHeader(path.to_owned()));
    }
    let canonical = std::fs::canonicalize(path).map_err(|source| InteropSourceError::Io {
        operation: "canonicalize header",
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_file() {
        return Err(InteropSourceError::InvalidHeader(canonical));
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{scan_complete_header, InteropSourceError};

    #[test]
    fn rejects_relative_paths_before_parc() {
        let error = scan_complete_header(
            Path::new("package"),
            Path::new("package/header.h"),
            &crate::toolchain::tests::synthetic_target(),
        )
        .unwrap_err();
        assert!(matches!(error, InteropSourceError::InvalidPackageRoot(_)));
    }

    #[test]
    fn rejects_header_outside_package_before_parc() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "fol-interop-source-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let package = scratch.join("package");
        let outside = scratch.join("outside.h");
        fs::create_dir_all(&package).unwrap();
        fs::write(&outside, b"int outside(void);\n").unwrap();

        let error = scan_complete_header(
            &package,
            &outside,
            &crate::toolchain::tests::synthetic_target(),
        )
        .unwrap_err();
        fs::remove_dir_all(&scratch).unwrap();

        assert!(matches!(
            error,
            InteropSourceError::HeaderOutsidePackage { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_header_escape_before_parc() {
        use std::os::unix::fs::symlink;

        static NEXT: AtomicU64 = AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "fol-interop-source-link-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let package = scratch.join("package");
        let outside = scratch.join("outside.h");
        let linked = package.join("linked.h");
        fs::create_dir_all(&package).unwrap();
        fs::write(&outside, b"int outside(void);\n").unwrap();
        symlink(&outside, &linked).unwrap();

        let error = scan_complete_header(
            &package,
            &linked,
            &crate::toolchain::tests::synthetic_target(),
        )
        .unwrap_err();
        fs::remove_dir_all(&scratch).unwrap();

        assert!(matches!(
            error,
            InteropSourceError::HeaderOutsidePackage { .. }
        ));
    }
}

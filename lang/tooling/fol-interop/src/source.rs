use std::collections::BTreeSet;
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
///
/// `wanted` names the symbols the annotation overlay selected. Only those and
/// their type closure enter the source package, which is what section 4.13
/// means by "unsupported declarations may remain in the header": a real C
/// header almost always declares a variadic somewhere, and selecting the whole
/// file would make every such header unusable over a declaration nobody asked
/// to call. Passing `None` selects everything supported, which is what the
/// fixed H7 smoke still wants.
pub(crate) fn scan_complete_header(
    package_root: &Path,
    header: &Path,
    target: &TargetSpec,
    wanted: Option<&BTreeSet<String>>,
    search: &HeaderSearch,
) -> Result<CompleteSourcePackage, InteropSourceError> {
    let root = canonical_directory(package_root)?;
    let header = canonical_file(header)?;
    if !header.starts_with(&root) {
        return Err(InteropSourceError::HeaderOutsidePackage { root, header });
    }
    let mapping = PathMapping::try_new([PathMappingRule::try_new(&root, "package")?])?;
    let mut config =
        ScanConfig::new(target.clone(), mapping, PreprocessorMode::Builtin)?.entry_header(header);

    // Include roots are canonicalized before they reach PARC, and a
    // package-relative one must stay inside the package. A `../..` root or a
    // symlink pointing out of the tree would make the accepted surface depend
    // on files outside the package -- which the fingerprint does not cover, so
    // two machines would silently bind different headers.
    for include in &search.include_roots {
        config = config.include_dir(canonical_include_root(&root, include)?);
    }
    // System roots are deliberately *not* required to be inside the package:
    // an SDK lives elsewhere by definition. They are still canonicalized, so
    // the recorded identity is a real path rather than whatever was typed.
    for include in &search.system_include_roots {
        config = config.system_include_dir(canonical_directory(Path::new(include))?);
    }
    for define in &search.defines {
        // `NAME=VALUE` or a bare `NAME`. Splitting here rather than in the
        // build surface keeps the C spelling the author already knows.
        match define.split_once('=') {
            Some((name, value)) => config = config.define(name, Some(value.to_string())),
            None => config = config.define(define.as_str(), None),
        }
    }
    if let Some(sysroot) = &search.sysroot {
        config = config.with_external_sysroot(canonical_directory(Path::new(sysroot))?);
    }

    let report = scan_headers(&config)?;

    let selection = match wanted {
        Some(wanted) => {
            let mut roots = Vec::new();
            let mut unsupported = Vec::new();
            for declaration in report.package().declarations() {
                let Some(name) = declaration.name.as_ref() else {
                    continue;
                };
                if !wanted.contains(&name.original) {
                    continue;
                }
                // A *selected* declaration that PARC could not model is a hard
                // rejection: the author asked to call this one.
                if declaration.support.is_supported() {
                    roots.push(declaration.id);
                } else {
                    unsupported.push(name.original.clone());
                }
            }
            if !unsupported.is_empty() {
                unsupported.sort();
                return Err(InteropSourceError::UnsupportedSelection(unsupported));
            }
            if roots.is_empty() {
                return Err(InteropSourceError::NothingSelected);
            }
            Selection::only(roots).map_err(|error| {
                InteropSourceError::UnsupportedSelection(vec![error.to_string()])
            })?
        }
        None => {
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
            Selection::all_supported()
        }
    };

    report
        .into_complete(&selection)
        .map_err(InteropSourceError::Incomplete)
}

/// Where the C preprocessor is allowed to look, and what it starts with.
///
/// Every field is stated by the build declaration; none is discovered from the
/// environment. Section 4.13 disables ambient `CPATH`, SDK, and sysroot
/// discovery in reproducible mode, so an include root that is not written down
/// is one the scan does not have.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderSearch {
    /// Quoted-include roots, package-relative or absolute. Must resolve inside
    /// the package.
    pub include_roots: Vec<String>,
    /// Angled-include roots. May live outside the package: an SDK does.
    pub system_include_roots: Vec<String>,
    /// `NAME` or `NAME=VALUE`, in declaration order.
    pub defines: Vec<String>,
    /// An external sysroot, when the provider was built against one.
    pub sysroot: Option<String>,
}

#[derive(Debug)]
pub enum InteropSourceError {
    InvalidPackageRoot(PathBuf),
    InvalidHeader(PathBuf),
    HeaderOutsidePackage {
        root: PathBuf,
        header: PathBuf,
    },
    /// An include root that resolves outside the package.
    ///
    /// Rejected rather than followed: the build fingerprint covers the package,
    /// so a root reaching outside it would let the accepted surface change
    /// without the fingerprint moving.
    IncludeRootOutsidePackage {
        root: PathBuf,
        include: PathBuf,
    },
    UnsupportedSource {
        declarations: usize,
        macros: usize,
    },
    /// A declaration the overlay *asked for* that PARC could not model.
    UnsupportedSelection(Vec<String>),
    /// The overlay named symbols, but the header declares none of them.
    NothingSelected,
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
            Self::IncludeRootOutsidePackage { root, include } => write!(
                formatter,
                "include root {} resolves outside package root {}; a root reaching outside the \
                 package would let the accepted C surface change without the build fingerprint \
                 moving, so it is refused rather than followed",
                include.display(),
                root.display()
            ),
            Self::UnsupportedSource {
                declarations,
                macros,
            } => write!(
                formatter,
                "PARC source contains {declarations} unsupported declaration(s) and {macros} unsupported macro(s)"
            ),
            Self::UnsupportedSelection(names) => write!(
                formatter,
                "the annotation overlay selects {} declaration(s) the C front end could not \
                 model: {}",
                names.len(),
                names.join(", ")
            ),
            Self::NothingSelected => formatter.write_str(
                "the entry header declares none of the symbols the annotation overlay selects",
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
            | Self::IncludeRootOutsidePackage { .. }
            | Self::UnsupportedSource { .. }
            | Self::UnsupportedSelection(_)
            | Self::NothingSelected => None,
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

/// Canonicalize one quoted-include root and prove it stays in the package.
///
/// A relative root is resolved against the package, which is what an author
/// writing `native/include` means. Canonicalization happens before the
/// containment check on purpose: `native/../..` and a symlink out of the tree
/// both look package-relative until they are resolved.
fn canonical_include_root(root: &Path, include: &str) -> Result<PathBuf, InteropSourceError> {
    let candidate = Path::new(include);
    let absolute = if candidate.is_absolute() {
        candidate.to_owned()
    } else {
        root.join(candidate)
    };
    let canonical = canonical_directory(&absolute)?;
    if !canonical.starts_with(root) {
        return Err(InteropSourceError::IncludeRootOutsidePackage {
            root: root.to_owned(),
            include: canonical,
        });
    }
    Ok(canonical)
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

    use super::{scan_complete_header, HeaderSearch, InteropSourceError};

    #[test]
    fn rejects_relative_paths_before_parc() {
        let error = scan_complete_header(
            Path::new("package"),
            Path::new("package/header.h"),
            &crate::toolchain::tests::synthetic_target(),
            None,
            &HeaderSearch::default(),
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
            None,
            &HeaderSearch::default(),
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
            None,
            &HeaderSearch::default(),
        )
        .unwrap_err();
        fs::remove_dir_all(&scratch).unwrap();

        assert!(matches!(
            error,
            InteropSourceError::HeaderOutsidePackage { .. }
        ));
    }

    /// A quoted-include root is canonicalized before the containment check, so
    /// `..` and symlinks cannot walk out of the package.
    #[test]
    fn rejects_include_root_that_escapes_the_package() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "fol-interop-include-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let package = scratch.join("package");
        let header = package.join("header.h");
        fs::create_dir_all(package.join("include")).unwrap();
        fs::create_dir_all(scratch.join("outside")).unwrap();
        fs::write(&header, b"int inside(void);\n").unwrap();

        let search = HeaderSearch {
            include_roots: vec!["include/../../outside".to_string()],
            ..HeaderSearch::default()
        };
        let error = scan_complete_header(
            &package,
            &header,
            &crate::toolchain::tests::synthetic_target(),
            None,
            &search,
        )
        .unwrap_err();
        fs::remove_dir_all(&scratch).unwrap();

        assert!(matches!(
            error,
            InteropSourceError::IncludeRootOutsidePackage { .. }
        ));
    }

    /// An angled-include root is exempt: an SDK legitimately lives outside the
    /// package, so containment must not be applied to it.
    #[test]
    fn accepts_system_include_root_outside_the_package() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let scratch = std::env::temp_dir().join(format!(
            "fol-interop-system-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let package = scratch.join("package");
        let sdk = scratch.join("sdk");
        fs::create_dir_all(&package).unwrap();
        fs::create_dir_all(&sdk).unwrap();
        fs::write(package.join("header.h"), b"int inside(void);\n").unwrap();

        let search = HeaderSearch {
            system_include_roots: vec![sdk.display().to_string()],
            ..HeaderSearch::default()
        };
        let error = scan_complete_header(
            &package,
            &package.join("header.h"),
            &crate::toolchain::tests::synthetic_target(),
            None,
            &search,
        )
        .unwrap_err();
        fs::remove_dir_all(&scratch).unwrap();

        assert!(!matches!(
            error,
            InteropSourceError::IncludeRootOutsidePackage { .. }
        ));
    }
}

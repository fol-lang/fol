//! `fol tool bind c`: run the checked C pipeline once and write down what it
//! accepted.
//!
//! Section 4.13 splits the work in two. This is the half that costs a C
//! preprocessor, a native inspection, and a generation pass; it runs when the
//! author asks for it, and its output is checked in. Ordinary compilation
//! reads that file. The build action re-runs this and compares, so the file is
//! evidence rather than a cache.
//!
//! Nothing here is discovered from the environment. Every path, the target,
//! and the compiler are explicit arguments, because a manifest that depended
//! on an ambient `CPATH` would describe a surface nobody else can reproduce.

use std::path::{Path, PathBuf};

use fol_abi::{
    AnnotationError, AnnotationOverlay, CapabilityModel, ImportManifest, ImportProvenance,
    ImportRejection,
};

use crate::{
    analysis::{preflight_temporary_parent, strict_compile_only_policy},
    generation::generate_raw_bindings,
    identity::compiled_component_revisions,
    interface::project_imported_interface,
    source::scan_complete_header,
    toolchain::CertifiedCToolchain,
};

use linc::{
    contract::AnalysisRequest,
    native::{NativeAnalyzer, NativeInspector, NativeResolver, ResolverConfiguration},
};

/// Everything one bind needs, all of it explicit.
#[derive(Debug, Clone)]
pub struct BindCRequest<'a> {
    pub alias: &'a str,
    pub target: fol_types::ResolvedTarget,
    pub package_root: &'a Path,
    pub header: &'a Path,
    pub provider: &'a Path,
    pub provider_kind: fol_build::BuildCImportProviderKind,
    pub annotations: Option<&'a Path>,
    pub compiler: &'a Path,
    pub temporary_parent: &'a Path,
    pub model: CapabilityModel,
}

/// Run the pipeline and return the manifest it accepted.
pub fn bind_c(request: BindCRequest<'_>) -> Result<ImportManifest, BindCError> {
    if !crate::is_certified_interop_target(request.target.rust_target_triple()) {
        return Err(BindCError::UncertifiedTarget {
            triple: request.target.rust_target_triple().to_string(),
        });
    }

    // The overlay is read before any external process runs: a typo in it is
    // cheap to report and should not cost a preprocessor invocation.
    let overlay = match request.annotations {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|source| BindCError::Io {
                path: path.to_owned(),
                source,
            })?;
            AnnotationOverlay::parse(&text).map_err(|error| BindCError::Annotation {
                path: path.to_owned(),
                error,
            })?
        }
        None => AnnotationOverlay::default(),
    };
    if overlay.is_empty() {
        return Err(BindCError::EmptySelection);
    }

    let package_root = canonical_directory(request.package_root)?;
    let header = canonical_file(request.header)?;
    let provider = canonical_file(request.provider)?;
    let compiler = canonical_file(request.compiler)?;
    let temporary_parent = preflight_temporary_parent(request.temporary_parent)
        .map_err(|error| BindCError::Policy(error.to_string()))?;

    let policy = strict_compile_only_policy(temporary_parent)
        .map_err(|error| BindCError::Policy(error.to_string()))?;
    let toolchain = CertifiedCToolchain::observe(&request.target, &compiler)
        .map_err(|error| BindCError::Toolchain(error.to_string()))?;
    // Only the overlay's chosen symbols enter the closure, so an unrelated
    // variadic elsewhere in the header does not make the header unusable.
    let wanted: std::collections::BTreeSet<String> = overlay
        .routines()
        .map(|routine| routine.symbol.clone())
        .collect();
    let source = scan_complete_header(&package_root, &header, toolchain.target(), Some(&wanted))
        .map_err(|error| BindCError::Source(error.to_string()))?;

    let native_inputs = [crate::pipeline::native_input_for(
        request.provider_kind,
        provider.clone(),
    )];
    let analysis_request = AnalysisRequest::try_new(&source, &native_inputs, policy)
        .map_err(|error| BindCError::Analysis(error.to_string()))?;
    let resolver = NativeResolver::new(
        NativeInspector::default(),
        ResolverConfiguration::new(
            Vec::new(),
            crate::pipeline::library_preference(request.provider_kind),
            crate::pipeline::MAX_TRANSITIVE_NATIVE_DEPENDENCIES,
        )
        .map_err(|error| BindCError::Analysis(error.to_string()))?,
    )
    .map_err(|error| BindCError::Analysis(error.to_string()))?;
    // LINC resolves every declared symbol to exactly one provider here. FOL
    // carries that result; it never runs a second resolver of its own.
    let evidence = NativeAnalyzer::new(resolver)
        .certify(&analysis_request, toolchain.certification())
        .map_err(|error| BindCError::Analysis(error.to_string()))?;
    let bundle = generate_raw_bindings(&source, &evidence)
        .map_err(|error| BindCError::Generation(error.to_string()))?;

    // The three stages must agree about what they looked at, or the interface
    // describes one thing and the provider another.
    let source_fingerprint = source.source().fingerprint();
    if evidence.package().source_fingerprint() != source_fingerprint
        || bundle.manifest().source_fingerprint() != source_fingerprint
    {
        return Err(BindCError::FingerprintMismatch("source"));
    }
    let target_fingerprint = source.source().target_fingerprint();
    if evidence.package().target_fingerprint() != target_fingerprint
        || bundle.projection().target_fingerprint() != target_fingerprint
    {
        return Err(BindCError::FingerprintMismatch("target"));
    }

    let interface = project_imported_interface(
        request.alias,
        request.target,
        &source,
        &bundle,
        &overlay,
        request.model,
    )
    .map_err(BindCError::Rejected)?;

    let revisions = compiled_component_revisions();
    Ok(ImportManifest {
        interface,
        provenance: ImportProvenance {
            header: relative_to(&package_root, &header),
            provider: relative_to(&package_root, &provider),
            provider_kind: request.provider_kind.as_str().to_string(),
            annotations: request
                .annotations
                .and_then(|path| path.canonicalize().ok())
                .map(|path| relative_to(&package_root, &path)),
            compiler: compiler.display().to_string(),
            components: vec![
                format!("parc={}", revisions.parc),
                format!("linc={}", revisions.linc),
                format!("gerc={}", revisions.gerc),
            ],
        },
    })
}

/// Paths in the manifest are package-relative, so the same package binds to the
/// same bytes wherever it is checked out.
fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn canonical_directory(path: &Path) -> Result<PathBuf, BindCError> {
    let canonical = path.canonicalize().map_err(|source| BindCError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_dir() {
        return Err(BindCError::NotADirectory(canonical));
    }
    Ok(canonical)
}

fn canonical_file(path: &Path) -> Result<PathBuf, BindCError> {
    let canonical = path.canonicalize().map_err(|source| BindCError::Io {
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_file() {
        return Err(BindCError::NotAFile(canonical));
    }
    Ok(canonical)
}

#[derive(Debug)]
pub enum BindCError {
    UncertifiedTarget {
        triple: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    NotADirectory(PathBuf),
    NotAFile(PathBuf),
    Annotation {
        path: PathBuf,
        error: AnnotationError,
    },
    EmptySelection,
    Policy(String),
    Toolchain(String),
    Source(String),
    Analysis(String),
    Generation(String),
    FingerprintMismatch(&'static str),
    Rejected(Vec<ImportRejection>),
}

impl std::fmt::Display for BindCError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UncertifiedTarget { triple } => write!(
                f,
                "C import target '{triple}' is not certified; expected one of {}",
                crate::CERTIFIED_INTEROP_TARGETS.join(", ")
            ),
            Self::Io { path, source } => write!(f, "could not read {}: {source}", path.display()),
            Self::NotADirectory(path) => write!(f, "{} is not a directory", path.display()),
            Self::NotAFile(path) => write!(f, "{} is not a file", path.display()),
            Self::Annotation { path, error } => {
                write!(f, "{}: {error}", path.display())
            }
            Self::EmptySelection => write!(
                f,
                "the annotation overlay selects no routines, so the import would define nothing; \
                 add a [routine.<symbol>] table for each declaration FOL should call"
            ),
            Self::Policy(detail) => write!(f, "invalid analysis policy: {detail}"),
            Self::Toolchain(detail) => write!(f, "could not certify the C toolchain: {detail}"),
            Self::Source(detail) => write!(f, "the entry header was not accepted: {detail}"),
            Self::Analysis(detail) => write!(f, "native analysis failed: {detail}"),
            Self::Generation(detail) => write!(f, "raw binding generation failed: {detail}"),
            Self::FingerprintMismatch(stage) => write!(
                f,
                "the interop stages disagree about the {stage} they examined"
            ),
            Self::Rejected(rejections) => {
                writeln!(
                    f,
                    "{} selected declaration(s) cannot be imported:",
                    rejections.len()
                )?;
                for rejection in rejections {
                    writeln!(f, "  [{}] {rejection}", rejection.diagnostic_code())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for BindCError {}

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
    /// Where the preprocessor may look, and what it starts with. Stated by the
    /// build declaration; nothing here is discovered from the environment.
    pub search: crate::source::HeaderSearch,
    /// The C standard the header is read as. `None` means C17.
    pub dialect: Option<&'a str>,
    /// Directories the resolver searches for the provider's own dependencies.
    ///
    /// Declared, never discovered: a shared provider carries a `DT_NEEDED` on
    /// its libc, and the policy resolves exact paths only.
    pub library_paths: &'a [String],
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
    let toolchain = CertifiedCToolchain::observe(&request.target, &compiler, request.dialect)
        .map_err(|error| BindCError::Toolchain(error.to_string()))?;
    // Only the overlay's chosen symbols enter the closure, so an unrelated
    // variadic elsewhere in the header does not make the header unusable.
    let wanted: std::collections::BTreeSet<String> = overlay
        .routines()
        .map(|routine| routine.symbol.clone())
        .collect();
    let source = scan_complete_header(
        &package_root,
        &header,
        toolchain.target(),
        Some(&wanted),
        &request.search,
    )
    .map_err(|error| BindCError::Source(error.to_string()))?;

    // Declared library search paths are validated here and then refused,
    // which is worth the apparent contradiction: the record carries them so a
    // build program can state what a shared provider needs, and the pinned
    // LINC cannot act on them. Its certification profile requires
    // exact-path resolution, and exact-path resolution rejects a search path
    // as an input outright -- so passing them through would surface an
    // internal policy error instead of the real reason.
    if let Some(first) = request.library_paths.first() {
        for path in request.library_paths {
            crate::source::canonical_directory(&crate::source::resolve_against(
                &package_root,
                path,
            ))
            .map_err(|error| BindCError::Source(error.to_string()))?;
        }
        return Err(BindCError::LibrarySearchPathsUnsupported {
            first: first.clone(),
        });
    }

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
        .map_err(|error| {
            BindCError::Analysis(name_the_declaration(
                &source,
                explain_provider_resolution(&provider, error.to_string()),
            ))
        })?;
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

    let target_triple = request.target.as_str().to_string();
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
            header_digest: content_digest(&header)?,
            provider: relative_to(&package_root, &provider),
            provider_digest: content_digest(&provider)?,
            provider_kind: request.provider_kind.as_str().to_string(),
            annotations: request
                .annotations
                .and_then(|path| path.canonicalize().ok())
                .map(|path| relative_to(&package_root, &path)),
            annotations_digest: match request.annotations {
                Some(path) => Some(content_digest(path)?),
                None => None,
            },
            compiler: compiler.display().to_string(),
            target: target_triple,
            dialect: request.dialect.unwrap_or("c17").to_string(),
            include_roots: request.search.include_roots.clone(),
            system_include_roots: request.search.system_include_roots.clone(),
            defines: request.search.defines.clone(),
            sysroot: request.search.sysroot.clone(),
            components: vec![
                format!("parc={}", revisions.parc),
                format!("linc={}", revisions.linc),
                format!("gerc={}", revisions.gerc),
            ],
        },
    })
}

/// The digest of a file's exact bytes.
///
/// Recorded beside the path so a reader can tell whether the file still says
/// what it said when the manifest was written. SHA-256 rather than the FNV-64
/// used for FOL's internal identity comparisons: these cover files a build
/// consumes from outside the compiler, so a collision has to be infeasible to
/// arrange rather than merely unlikely to occur.
fn content_digest(path: &Path) -> Result<String, BindCError> {
    let bytes = std::fs::read(path).map_err(|source| BindCError::Io {
        path: path.to_owned(),
        source,
    })?;
    Ok(fol_abi::sha256_hex(&bytes))
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
    /// The import declares library search paths the certified analysis profile
    /// cannot use.
    LibrarySearchPathsUnsupported {
        first: String,
    },
    Policy(String),
    Toolchain(String),
    Source(String),
    Analysis(String),
    Generation(String),
    FingerprintMismatch(&'static str),
    Rejected(Vec<ImportRejection>),
}

/// Rewrite a native-analysis failure to name a place in the header.
///
/// LINC reports against its own declaration ids -- `pdecl1_<hash>` -- which
/// say nothing to whoever wrote the header, and it carries no range because
/// the rejection is about a symbol rather than a span. The symbol is in the
/// message though, and the scanned package knows where each declaration was
/// written, so the id is replaced with the place. A message naming no symbol
/// FOL can find is left exactly as it was: a worse guess is not an
/// improvement.
fn name_the_declaration(source: &parc::contract::CompleteSourcePackage, detail: String) -> String {
    let Some(symbol) = detail.split('"').nth(1).map(str::to_string) else {
        return detail;
    };
    let package = source.source();
    let Some(declaration) = package.declarations().iter().find(|declaration| {
        declaration
            .name
            .as_ref()
            .is_some_and(|name| name.original == symbol)
    }) else {
        return detail;
    };
    let Some(range) = declaration.occurrences.first().map(|entry| entry.range) else {
        return detail;
    };
    let Some(file) = package.files().iter().find(|file| file.id == range.file) else {
        return detail;
    };
    let line = file
        .line_starts
        .partition_point(|start| *start <= range.start);
    // The id is what a reader cannot act on; everything else LINC said is
    // kept, because it is the reason.
    let without_id = detail
        .split_whitespace()
        .filter(|word| !word.starts_with("pdecl"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{without_id}\n  = declared at {}:{line}", file.logical_path)
}

/// Explain a provider that resolution could not find but the author did supply.
///
/// A shared library carries its own dependencies, and the certified analysis
/// profile resolves exact paths only -- it will not search for `libc.so.6`,
/// and refuses a search path as an input. What surfaces is the *dependency*
/// reported as a missing provider, which reads as though the file the author
/// passed were absent. Naming the difference is the whole of the fix: the
/// constraint is real and FOL cannot lift it here.
fn explain_provider_resolution(requested: &Path, detail: String) -> String {
    let Some(named) = detail.split('"').nth(1) else {
        return detail;
    };
    // Only when the missing provider is *not* what the author supplied. A
    // genuinely absent file should keep saying so.
    let supplied = requested
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if named == supplied || supplied.is_empty() {
        return detail;
    }
    format!(
        "{detail}\n  = note: '{named}' is a dependency of the provider you supplied, not the \
         provider itself. The certified analysis profile resolves exact paths only and refuses \
         a search path as an input, so a shared provider that carries dependencies of its own \
         cannot be imported yet; supply a static or object provider instead"
    )
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
            Self::LibrarySearchPathsUnsupported { first } => write!(
                f,
                "this import declares the library search path '{first}', which the certified \
                 analysis profile cannot use: it resolves exact paths only, and exact-path \
                 resolution refuses a search path as an input. A shared provider that carries \
                 dependencies of its own therefore cannot be imported yet; supply a static or \
                 object provider, or one whose dependencies are already resolved"
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

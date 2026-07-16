use std::{
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

use fol_build::{
    materialize_generated_action_plan, BuildArtifactId, BuildArtifactInput, BuildArtifactKind,
    BuildCImportProviderKind, BuildGraph, BuildOptimizeMode, GeneratedFileMaterializationError,
};
use gerc::{GenerationError, GenerationFingerprint, RustLinkArtifactKind, RustLinkAtom};
use linc::{
    contract::{
        AnalysisRequest, ArtifactFingerprint, ContractError, LinkAnalysisFingerprint, NativeInput,
    },
    native::{
        LibraryPreference, NativeAnalyzer, NativeError, NativeInspector, NativeResolver,
        ResolverConfiguration,
    },
};
use parc::contract::{SourceFingerprint, TargetFingerprint};

use crate::{
    analysis::{
        preflight_temporary_parent, strict_compile_only_policy, InteropAnalysisPolicyError,
    },
    anchor::{
        h7_c_int_function_anchor, H7InteropAnchorError, H7_ANCHOR_CRATE_NAME,
        H7_ANCHOR_FUNCTION_NAME, H7_RAW_CRATE_NAME,
    },
    generation::generate_raw_bindings,
    identity::{verify_locked_siblings, InteropIdentityError},
    materialization::{
        attach_generated_bindings, attach_h7_anchor, InteropMaterializationPlanError,
    },
    source::{scan_complete_header, InteropSourceError},
    toolchain::{CertifiedGnuToolchain, InteropToolchainError},
    CERTIFIED_INTEROP_TARGET,
};

const MAX_TRANSITIVE_NATIVE_DEPENDENCIES: usize = 128;

/// Caller-owned paths and the authoritative evaluated graph for one H7 smoke
/// import. No source, ABI, provider, or generation evidence can be supplied.
#[derive(Debug, Clone)]
pub struct H7InteropRequest<'a> {
    graph: &'a BuildGraph,
    artifact: BuildArtifactId,
    package_root: &'a Path,
    compiler: &'a Path,
    temporary_parent: &'a Path,
    generated_output_root: &'a Path,
}

impl<'a> H7InteropRequest<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        graph: &'a BuildGraph,
        artifact: BuildArtifactId,
        package_root: &'a Path,
        compiler: &'a Path,
        temporary_parent: &'a Path,
        generated_output_root: &'a Path,
    ) -> Self {
        Self {
            graph,
            artifact,
            package_root,
            compiler,
            temporary_parent,
            generated_output_root,
        }
    }
}

/// Exact identities retained across the one promoted FOL interop handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct H7InteropReport {
    parc_revision: &'static str,
    linc_revision: &'static str,
    gerc_revision: &'static str,
    source: SourceFingerprint,
    target: TargetFingerprint,
    evidence: LinkAnalysisFingerprint,
    generation: GenerationFingerprint,
    provider: ArtifactFingerprint,
}

impl H7InteropReport {
    pub const fn source_fingerprint(self) -> SourceFingerprint {
        self.source
    }

    pub const fn target_fingerprint(self) -> TargetFingerprint {
        self.target
    }

    pub const fn evidence_fingerprint(self) -> LinkAnalysisFingerprint {
        self.evidence
    }

    pub const fn generation_fingerprint(self) -> GenerationFingerprint {
        self.generation
    }

    pub const fn provider_fingerprint(self) -> ArtifactFingerprint {
        self.provider
    }

    pub const fn parc_revision(self) -> &'static str {
        self.parc_revision
    }

    pub const fn linc_revision(self) -> &'static str {
        self.linc_revision
    }

    pub const fn gerc_revision(self) -> &'static str {
        self.gerc_revision
    }

    pub fn summary(self) -> String {
        format!(
            "target={} parc={} linc={} gerc={} source={} target_fingerprint={} evidence={} generation={} provider={}",
            CERTIFIED_INTEROP_TARGET,
            self.parc_revision(),
            self.linc_revision(),
            self.gerc_revision(),
            self.source,
            self.target,
            self.evidence,
            self.generation,
            self.provider,
        )
    }
}

/// Fully checked and already materialized auxiliary Rust inputs for the
/// backend. Arguments remain exact `OsString` process arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct H7InteropBuild {
    graph: BuildGraph,
    artifact: BuildArtifactId,
    raw_crate_root: PathBuf,
    anchor_crate_root: PathBuf,
    rustc_link_arguments: Vec<OsString>,
    report: H7InteropReport,
}

impl H7InteropBuild {
    pub fn graph(&self) -> &BuildGraph {
        &self.graph
    }

    pub const fn artifact(&self) -> BuildArtifactId {
        self.artifact
    }

    pub fn raw_crate_root(&self) -> &Path {
        &self.raw_crate_root
    }

    pub fn anchor_crate_root(&self) -> &Path {
        &self.anchor_crate_root
    }

    pub fn rustc_link_arguments(&self) -> &[OsString] {
        &self.rustc_link_arguments
    }

    pub const fn report(&self) -> H7InteropReport {
        self.report
    }

    pub const fn raw_crate_name(&self) -> &'static str {
        H7_RAW_CRATE_NAME
    }

    pub const fn anchor_crate_name(&self) -> &'static str {
        H7_ANCHOR_CRATE_NAME
    }

    pub const fn anchor_function_name(&self) -> &'static str {
        H7_ANCHOR_FUNCTION_NAME
    }
}

/// Run the sole FOL production path from concrete build-graph inputs through
/// PARC, LINC, and GERC, then attach and materialize exact generated actions.
///
/// All target, graph, path, profile, and policy checks happen before any
/// external process. The locked sibling identities are observed before their
/// APIs execute. Generated output is written only after all sibling stages and
/// fingerprint checks succeed.
pub fn prepare_h7_interop(request: H7InteropRequest<'_>) -> Result<H7InteropBuild, H7InteropError> {
    let artifact = request
        .graph
        .artifact(request.artifact)
        .ok_or(H7InteropError::UnknownArtifact(request.artifact))?;
    if artifact.kind != BuildArtifactKind::Executable {
        return Err(H7InteropError::UnsupportedArtifactKind(artifact.kind));
    }
    if artifact.target.as_str() != CERTIFIED_INTEROP_TARGET {
        return Err(H7InteropError::UnsupportedTarget(
            artifact.target.as_str().to_owned(),
        ));
    }
    if !matches!(
        artifact.optimize,
        BuildOptimizeMode::Debug | BuildOptimizeMode::ReleaseSafe
    ) {
        return Err(H7InteropError::UnsupportedOptimize(artifact.optimize));
    }
    let graph_errors = request.graph.validate();
    if !graph_errors.is_empty() {
        return Err(H7InteropError::InvalidGraph(graph_errors.len()));
    }
    let mut c_imports = request.graph.c_imports_for(request.artifact);
    let c_import = c_imports.next().ok_or(H7InteropError::MissingCImport)?;
    if c_imports.next().is_some() {
        return Err(H7InteropError::MultipleCImports);
    }
    match c_import.provider_kind {
        BuildCImportProviderKind::Object => {}
    }
    if request
        .graph
        .artifact_inputs_for(request.artifact)
        .any(|input| matches!(input, BuildArtifactInput::GeneratedFile(_)))
    {
        return Err(H7InteropError::ExistingGeneratedInputs);
    }

    let package_root = canonical_directory(request.package_root, "package root")?;
    let header = canonical_package_file(
        &package_root,
        &package_root.join(&c_import.header),
        "header",
    )?;
    let provider = canonical_package_file(
        &package_root,
        &package_root.join(&c_import.provider),
        "provider",
    )?;
    let generated_output_root = preflight_generated_output_root(request.generated_output_root)?;
    let temporary_parent = preflight_temporary_parent(request.temporary_parent)?;
    let compiler = canonical_regular_file(request.compiler, "compiler")?;
    let revisions = verify_locked_siblings()?;

    let policy = strict_compile_only_policy(temporary_parent)?;
    let toolchain = CertifiedGnuToolchain::observe(&artifact.target, compiler)?;
    let source = scan_complete_header(&package_root, &header, toolchain.target())?;
    let native_inputs = [NativeInput::ObjectPath(provider.clone())];
    let analysis_request = AnalysisRequest::try_new(&source, &native_inputs, policy)?;
    let resolver = NativeResolver::new(
        NativeInspector::default(),
        ResolverConfiguration::new(
            Vec::new(),
            LibraryPreference::DynamicOnly,
            MAX_TRANSITIVE_NATIVE_DEPENDENCIES,
        )?,
    )?;
    let evidence =
        NativeAnalyzer::new(resolver).certify(&analysis_request, toolchain.certification())?;
    let bundle = generate_raw_bindings(&source, &evidence)?;
    let anchor = h7_c_int_function_anchor(&bundle)?;

    let source_fingerprint = source.source().fingerprint();
    let target_fingerprint = source.source().target_fingerprint();
    let evidence_fingerprint = evidence.package().fingerprint();
    let manifest = bundle.manifest();
    if evidence.package().source_fingerprint() != source_fingerprint
        || manifest.source_fingerprint() != source_fingerprint
    {
        return Err(H7InteropError::FingerprintMismatch("source"));
    }
    if evidence.package().target_fingerprint() != target_fingerprint
        || manifest.target_fingerprint() != target_fingerprint
        || bundle.projection().target_fingerprint() != target_fingerprint
        || bundle.link_plan().target_fingerprint() != target_fingerprint
    {
        return Err(H7InteropError::FingerprintMismatch("target"));
    }
    if manifest.evidence_fingerprint() != evidence_fingerprint {
        return Err(H7InteropError::FingerprintMismatch("evidence"));
    }
    let [RustLinkAtom::Artifact(linked_provider)] = bundle.link_plan().atoms() else {
        return Err(H7InteropError::UnexpectedLinkPlan);
    };
    if linked_provider.kind() != RustLinkArtifactKind::Object
        || linked_provider.canonical_path() != provider
    {
        return Err(H7InteropError::UnexpectedLinkPlan);
    }
    let rustc_link_arguments = bundle.link_plan().rustc_arguments()?.into_arguments();

    let mut graph = request.graph.clone();
    let mut generated = attach_generated_bindings(&mut graph, request.artifact, &bundle)?;
    attach_h7_anchor(&mut graph, request.artifact, &mut generated, &anchor)?;
    let raw_crate_root = generated_output_root.join(generated.raw_crate_root());
    let anchor_crate_root = generated_output_root.join(
        generated
            .anchor_crate_root()
            .ok_or(H7InteropError::MissingAnchor)?,
    );
    materialize_with_output_root(&generated_output_root, |output_root| {
        materialize_generated_action_plan(
            &graph,
            request.artifact,
            output_root,
            generated.materialization(),
        )
    })?;

    Ok(H7InteropBuild {
        graph,
        artifact: request.artifact,
        raw_crate_root,
        anchor_crate_root,
        rustc_link_arguments,
        report: H7InteropReport {
            parc_revision: revisions.parc,
            linc_revision: revisions.linc,
            gerc_revision: revisions.gerc,
            source: source_fingerprint,
            target: target_fingerprint,
            evidence: evidence_fingerprint,
            generation: manifest.generation_fingerprint(),
            provider: linked_provider.artifact_fingerprint(),
        },
    })
}

/// Validate the caller-selected output without creating it. Existing roots
/// must be real directories rather than symlinks. A missing root is accepted
/// only as one normalized leaf below an existing directory; its parent is
/// canonicalized now so later creation cannot follow the caller's unresolved
/// parent spelling.
fn preflight_generated_output_root(path: &Path) -> Result<PathBuf, H7InteropError> {
    if !normalized_absolute(path) {
        return Err(H7InteropError::InvalidPath {
            role: "generated output root",
            path: path.to_owned(),
        });
    }

    match std::fs::symlink_metadata(path) {
        Ok(metadata) => canonical_existing_output_root(path, metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let Some(parent) = path.parent() else {
                return Err(H7InteropError::InvalidPath {
                    role: "generated output root",
                    path: path.to_owned(),
                });
            };
            let Some(leaf) = path.file_name() else {
                return Err(H7InteropError::InvalidPath {
                    role: "generated output root",
                    path: path.to_owned(),
                });
            };
            let parent = canonical_directory(parent, "generated output parent")?;
            let canonical_candidate = parent.join(leaf);
            match std::fs::symlink_metadata(&canonical_candidate) {
                Ok(metadata) => canonical_existing_output_root(&canonical_candidate, metadata),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(canonical_candidate)
                }
                Err(source) => Err(H7InteropError::OutputIo {
                    operation: "inspect",
                    path: canonical_candidate,
                    source,
                }),
            }
        }
        Err(source) => Err(H7InteropError::OutputIo {
            operation: "inspect",
            path: path.to_owned(),
            source,
        }),
    }
}

fn canonical_existing_output_root(
    path: &Path,
    metadata: std::fs::Metadata,
) -> Result<PathBuf, H7InteropError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(H7InteropError::InvalidPath {
            role: "generated output root",
            path: path.to_owned(),
        });
    }
    let canonical = std::fs::canonicalize(path).map_err(|source| H7InteropError::OutputIo {
        operation: "canonicalize",
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_dir() {
        return Err(H7InteropError::InvalidPath {
            role: "generated output root",
            path: canonical,
        });
    }
    Ok(canonical)
}

/// Create a previously preflighted leaf immediately before materialization.
/// If this call owns that leaf, any materialization failure removes the whole
/// partial tree. Pre-existing caller-owned roots are never removed.
fn materialize_with_output_root<T>(
    output_root: &Path,
    materialize: impl FnOnce(&Path) -> Result<T, GeneratedFileMaterializationError>,
) -> Result<T, H7InteropError> {
    let created = match std::fs::create_dir(output_root) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(output_root).map_err(|source| {
                H7InteropError::OutputIo {
                    operation: "inspect",
                    path: output_root.to_owned(),
                    source,
                }
            })?;
            let canonical = canonical_existing_output_root(output_root, metadata)?;
            if canonical != output_root {
                return Err(H7InteropError::InvalidPath {
                    role: "generated output root",
                    path: output_root.to_owned(),
                });
            }
            false
        }
        Err(source) => {
            return Err(H7InteropError::OutputIo {
                operation: "create",
                path: output_root.to_owned(),
                source,
            })
        }
    };

    match materialize(output_root) {
        Ok(value) => Ok(value),
        Err(materialization) if created => {
            if let Err(source) = std::fs::remove_dir_all(output_root) {
                return Err(H7InteropError::MaterializationCleanup {
                    path: output_root.to_owned(),
                    materialization: Box::new(materialization),
                    source,
                });
            }
            Err(H7InteropError::Materialization(materialization))
        }
        Err(materialization) => Err(H7InteropError::Materialization(materialization)),
    }
}

fn canonical_directory(path: &Path, role: &'static str) -> Result<PathBuf, H7InteropError> {
    if !normalized_absolute(path) {
        return Err(H7InteropError::InvalidPath {
            role,
            path: path.to_owned(),
        });
    }
    let canonical = std::fs::canonicalize(path).map_err(|source| H7InteropError::Io {
        role,
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_dir() {
        return Err(H7InteropError::InvalidPath {
            role,
            path: canonical,
        });
    }
    Ok(canonical)
}

fn canonical_package_file(
    package_root: &Path,
    path: &Path,
    role: &'static str,
) -> Result<PathBuf, H7InteropError> {
    if !normalized_absolute(path) {
        return Err(H7InteropError::InvalidPath {
            role,
            path: path.to_owned(),
        });
    }
    let canonical = std::fs::canonicalize(path).map_err(|source| H7InteropError::Io {
        role,
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_file() || !canonical.starts_with(package_root) {
        return Err(H7InteropError::InvalidPath {
            role,
            path: canonical,
        });
    }
    Ok(canonical)
}

fn canonical_regular_file(path: &Path, role: &'static str) -> Result<PathBuf, H7InteropError> {
    if !normalized_absolute(path) {
        return Err(H7InteropError::InvalidPath {
            role,
            path: path.to_owned(),
        });
    }
    let canonical = std::fs::canonicalize(path).map_err(|source| H7InteropError::Io {
        role,
        path: path.to_owned(),
        source,
    })?;
    if !canonical.is_file() {
        return Err(H7InteropError::InvalidPath {
            role,
            path: canonical,
        });
    }
    Ok(canonical)
}

fn normalized_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
        && path.as_os_str() == path.components().collect::<PathBuf>().as_os_str()
}

#[derive(Debug)]
pub enum H7InteropError {
    UnknownArtifact(BuildArtifactId),
    UnsupportedArtifactKind(BuildArtifactKind),
    UnsupportedTarget(String),
    UnsupportedOptimize(BuildOptimizeMode),
    InvalidGraph(usize),
    MissingCImport,
    MultipleCImports,
    ExistingGeneratedInputs,
    InvalidPath {
        role: &'static str,
        path: PathBuf,
    },
    Io {
        role: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    OutputIo {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    FingerprintMismatch(&'static str),
    UnexpectedLinkPlan,
    MissingAnchor,
    Identity(InteropIdentityError),
    Toolchain(InteropToolchainError),
    Source(InteropSourceError),
    Policy(InteropAnalysisPolicyError),
    Contract(ContractError),
    Native(NativeError),
    Generation(GenerationError),
    Anchor(H7InteropAnchorError),
    Attachment(InteropMaterializationPlanError),
    Materialization(GeneratedFileMaterializationError),
    MaterializationCleanup {
        path: PathBuf,
        materialization: Box<GeneratedFileMaterializationError>,
        source: std::io::Error,
    },
}

impl std::fmt::Display for H7InteropError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownArtifact(artifact) => write!(formatter, "unknown interop {artifact}"),
            Self::UnsupportedArtifactKind(kind) => {
                write!(formatter, "H7 interop requires an executable artifact, not {kind:?}")
            }
            Self::UnsupportedTarget(target) => write!(
                formatter,
                "H7 interop target is '{target}', expected {CERTIFIED_INTEROP_TARGET}"
            ),
            Self::UnsupportedOptimize(optimize) => write!(
                formatter,
                "H7 interop supports debug or release-safe, not {}",
                optimize.as_str()
            ),
            Self::InvalidGraph(count) => {
                write!(formatter, "H7 interop graph has {count} validation error(s)")
            }
            Self::MissingCImport => formatter.write_str(
                "H7 interop artifact has no authoritative C import attachment",
            ),
            Self::MultipleCImports => formatter.write_str(
                "H7 interop artifact has more than one C import attachment",
            ),
            Self::ExistingGeneratedInputs => formatter.write_str(
                "H7 interop artifact already has generated inputs outside the exact smoke plan",
            ),
            Self::InvalidPath { role, path } => write!(
                formatter,
                "H7 interop {role} is not a normalized absolute path with the required kind and containment: {}",
                path.display()
            ),
            Self::Io { role, path, source } => write!(
                formatter,
                "could not canonicalize H7 interop {role} {}: {source}",
                path.display()
            ),
            Self::OutputIo {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "could not {operation} H7 interop generated output root {}: {source}",
                path.display()
            ),
            Self::FingerprintMismatch(layer) => {
                write!(formatter, "H7 interop {layer} fingerprint did not survive the pipeline")
            }
            Self::UnexpectedLinkPlan => formatter.write_str(
                "H7 interop expected exactly the selected object provider in the GERC link plan",
            ),
            Self::MissingAnchor => {
                formatter.write_str("H7 interop materialization omitted its mandatory anchor")
            }
            Self::Identity(error) => write!(formatter, "{error}"),
            Self::Toolchain(error) => write!(formatter, "{error}"),
            Self::Source(error) => write!(formatter, "{error}"),
            Self::Policy(error) => write!(formatter, "{error}"),
            Self::Contract(error) => write!(formatter, "invalid LINC request: {error}"),
            Self::Native(error) => write!(formatter, "LINC certification failed: {error}"),
            Self::Generation(error) => write!(formatter, "GERC generation failed: {error}"),
            Self::Anchor(error) => write!(formatter, "{error}"),
            Self::Attachment(error) => write!(formatter, "{error}"),
            Self::Materialization(error) => write!(formatter, "{error}"),
            Self::MaterializationCleanup {
                path,
                materialization,
                source,
            } => write!(
                formatter,
                "{materialization}; could not remove newly created H7 interop generated output root {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for H7InteropError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::OutputIo { source, .. } => Some(source),
            Self::Identity(error) => Some(error),
            Self::Toolchain(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Policy(error) => Some(error),
            Self::Contract(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Generation(error) => Some(error),
            Self::Anchor(error) => Some(error),
            Self::Attachment(error) => Some(error),
            Self::Materialization(error) => Some(error),
            Self::MaterializationCleanup { source, .. } => Some(source),
            _ => None,
        }
    }
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for H7InteropError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}

from_error!(InteropToolchainError, Toolchain);
from_error!(InteropIdentityError, Identity);
from_error!(InteropSourceError, Source);
from_error!(InteropAnalysisPolicyError, Policy);
from_error!(ContractError, Contract);
from_error!(NativeError, Native);
from_error!(GenerationError, Generation);
from_error!(H7InteropAnchorError, Anchor);
from_error!(InteropMaterializationPlanError, Attachment);
from_error!(GeneratedFileMaterializationError, Materialization);

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    use fol_build::{
        BuildArtifactFolModel, BuildArtifactId, BuildArtifactKind, BuildCImportProviderKind,
        BuildGraph, BuildOptimizeMode, GeneratedFileMaterializationError,
    };

    use super::{
        materialize_with_output_root, preflight_generated_output_root, prepare_h7_interop,
        H7InteropError, H7InteropRequest,
    };

    static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "fol-interop-pipeline-{label}-{}-{}",
                std::process::id(),
                NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("create interop pipeline scratch root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct H7Fixture {
        _scratch: Scratch,
        package: PathBuf,
        temporary_parent: PathBuf,
        output_root: PathBuf,
    }

    impl H7Fixture {
        fn new(label: &str, header: &str, provider_source: &str) -> Self {
            let scratch = Scratch::new(label);
            let package = scratch.path().join("package");
            let native = package.join("native");
            let temporary_parent = scratch.path().join("probes");
            let output_root = scratch.path().join("interop-generated");
            fs::create_dir_all(&native).expect("create interop native fixture directory");
            fs::create_dir_all(&temporary_parent).expect("create LINC temporary parent");
            fs::write(native.join("provider.h"), header).expect("write interop fixture header");
            fs::write(native.join("provider.c"), provider_source)
                .expect("write interop fixture provider source");
            Self {
                _scratch: scratch,
                package,
                temporary_parent,
                output_root,
            }
        }

        fn compile_provider(&self, gcc: &Path) {
            let output = Command::new(gcc)
                .env_clear()
                .current_dir(&self.package)
                .args(["-std=gnu17", "-m64", "-fPIC", "-c"])
                .arg("native/provider.c")
                .arg("-o")
                .arg("native/provider.o")
                .output()
                .expect("launch explicit GCC for interop provider fixture");
            assert!(
                output.status.success(),
                "GCC provider fixture compilation failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            assert!(self.package.join("native/provider.o").is_file());
        }
    }

    fn h7_graph(target: &str) -> (BuildGraph, BuildArtifactId) {
        let mut graph = BuildGraph::new();
        let artifact = graph.add_configured_artifact(
            BuildArtifactKind::Executable,
            "app",
            "src/main.fol",
            BuildArtifactFolModel::Core,
            fol_types::ResolvedTarget::resolve(target).unwrap(),
            BuildOptimizeMode::Debug,
        );
        graph
            .add_c_import(
                artifact,
                "native/provider.h",
                "native/provider.o",
                BuildCImportProviderKind::Object,
            )
            .unwrap();
        (graph, artifact)
    }

    fn explicit_gcc() -> Option<PathBuf> {
        let required = std::env::var_os("FOL_H7_REQUIRED").is_some();
        let Some(supplied) = std::env::var_os("FOL_H7_GCC") else {
            if required {
                panic!("required fol-interop pipeline tests need an explicit FOL_H7_GCC");
            }
            eprintln!("interop pipeline stage test skipped; set FOL_H7_GCC to require it");
            return None;
        };
        let supplied = PathBuf::from(supplied);
        let canonical = fs::canonicalize(&supplied).unwrap_or_else(|error| {
            panic!(
                "could not canonicalize FOL_H7_GCC '{}': {error}",
                supplied.display()
            )
        });
        assert!(canonical.is_file(), "FOL_H7_GCC must be a regular file");
        Some(canonical)
    }

    #[test]
    fn unsupported_target_fails_before_any_path_or_tool_io() {
        let mut graph = BuildGraph::new();
        let artifact = graph.add_configured_artifact(
            BuildArtifactKind::Executable,
            "app",
            "src/main.fol",
            BuildArtifactFolModel::Core,
            fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-musl").unwrap(),
            BuildOptimizeMode::Debug,
        );
        let missing = Path::new("/definitely/missing/fol-h7-input");
        let request = H7InteropRequest::new(&graph, artifact, missing, missing, missing, missing);

        let error = prepare_h7_interop(request).unwrap_err();

        assert!(matches!(error, H7InteropError::UnsupportedTarget(_)));
    }

    #[test]
    fn unsupported_optimization_fails_before_any_path_or_tool_io() {
        let mut graph = BuildGraph::new();
        let artifact = graph.add_configured_artifact(
            BuildArtifactKind::Executable,
            "app",
            "src/main.fol",
            BuildArtifactFolModel::Core,
            fol_types::ResolvedTarget::resolve(crate::CERTIFIED_INTEROP_TARGET).unwrap(),
            BuildOptimizeMode::ReleaseFast,
        );
        let missing = Path::new("/definitely/missing/fol-h7-input");
        let request = H7InteropRequest::new(&graph, artifact, missing, missing, missing, missing);

        let error = prepare_h7_interop(request).unwrap_err();

        assert!(matches!(error, H7InteropError::UnsupportedOptimize(_)));
    }

    #[test]
    fn missing_package_leaves_output_root_uncreated() {
        let scratch = Scratch::new("missing-package");
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);
        let missing_package = scratch.path().join("missing-package");
        let missing_compiler = scratch.path().join("missing-gcc");
        let missing_temporary_parent = scratch.path().join("missing-probes");
        let output_root = scratch.path().join("interop-generated");

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &missing_package,
            &missing_compiler,
            &missing_temporary_parent,
            &output_root,
        ))
        .unwrap_err();

        assert!(matches!(
            error,
            H7InteropError::Io {
                role: "package root",
                ..
            }
        ));
        assert!(!output_root.exists());
    }

    #[test]
    fn missing_compiler_leaves_output_root_uncreated() {
        let fixture = H7Fixture::new(
            "missing-compiler",
            "int fol_h7_value(void);\n",
            "int fol_h7_value(void) { return 42; }\n",
        );
        fs::write(fixture.package.join("native/provider.o"), b"not reached")
            .expect("create canonical provider path");
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);
        let missing_compiler = fixture.package.join("missing-gcc");

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &fixture.package,
            &missing_compiler,
            &fixture.temporary_parent,
            &fixture.output_root,
        ))
        .unwrap_err();

        assert!(matches!(
            error,
            H7InteropError::Io {
                role: "compiler",
                ..
            }
        ));
        assert!(!fixture.output_root.exists());
    }

    #[test]
    fn missing_temporary_parent_leaves_output_root_uncreated() {
        let fixture = H7Fixture::new(
            "missing-temp",
            "int fol_h7_value(void);\n",
            "int fol_h7_value(void) { return 42; }\n",
        );
        fs::write(fixture.package.join("native/provider.o"), b"not reached")
            .expect("create canonical provider path");
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);
        let missing_compiler = fixture.package.join("missing-gcc");
        let missing_temporary_parent = fixture._scratch.path().join("missing-probes");

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &fixture.package,
            &missing_compiler,
            &missing_temporary_parent,
            &fixture.output_root,
        ))
        .unwrap_err();

        assert!(matches!(error, H7InteropError::Policy(_)));
        assert!(!fixture.output_root.exists());
    }

    #[test]
    fn target_failure_leaves_output_root_uncreated() {
        let scratch = Scratch::new("target");
        let (graph, artifact) = h7_graph("x86_64-unknown-linux-musl");
        let missing = scratch.path().join("not-reached");
        let output_root = scratch.path().join("interop-generated");

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &missing,
            &missing,
            &missing,
            &output_root,
        ))
        .unwrap_err();

        assert!(matches!(error, H7InteropError::UnsupportedTarget(_)));
        assert!(!output_root.exists());
    }

    #[test]
    fn parc_failure_leaves_output_root_uncreated() {
        let Some(gcc) = explicit_gcc() else {
            return;
        };
        let fixture = H7Fixture::new(
            "parc",
            "_BitInt(17) fol_h7_value(_BitInt(17) value);\n",
            "int unrelated_provider(void) { return 1; }\n",
        );
        fixture.compile_provider(&gcc);
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &fixture.package,
            &gcc,
            &fixture.temporary_parent,
            &fixture.output_root,
        ))
        .unwrap_err();

        assert!(matches!(error, H7InteropError::Source(_)), "{error}");
        assert!(!fixture.output_root.exists());
    }

    #[test]
    fn linc_failure_leaves_output_root_uncreated() {
        let Some(gcc) = explicit_gcc() else {
            return;
        };
        let fixture = H7Fixture::new(
            "linc",
            "int fol_h7_value(void);\n",
            "int unrelated_provider(void) { return 1; }\n",
        );
        fixture.compile_provider(&gcc);
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &fixture.package,
            &gcc,
            &fixture.temporary_parent,
            &fixture.output_root,
        ))
        .unwrap_err();

        assert!(matches!(error, H7InteropError::Native(_)), "{error}");
        assert!(!fixture.output_root.exists());
    }

    #[test]
    fn gerc_failure_leaves_output_root_uncreated() {
        let Some(gcc) = explicit_gcc() else {
            return;
        };
        let fixture = H7Fixture::new(
            "gerc",
            "extern _Thread_local int fol_h7_tls;\nint fol_h7_value(void);\n",
            "_Thread_local int fol_h7_tls = 42;\nint fol_h7_value(void) { return fol_h7_tls; }\n",
        );
        fixture.compile_provider(&gcc);
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &fixture.package,
            &gcc,
            &fixture.temporary_parent,
            &fixture.output_root,
        ))
        .unwrap_err();

        assert!(matches!(error, H7InteropError::Generation(_)), "{error}");
        assert!(!fixture.output_root.exists());
    }

    #[test]
    fn successful_pipeline_creates_missing_output_leaf_only_at_materialization() {
        let Some(gcc) = explicit_gcc() else {
            return;
        };
        let fixture = H7Fixture::new(
            "success",
            "int fol_h7_value(void);\n",
            "int fol_h7_value(void) { return 42; }\n",
        );
        fixture.compile_provider(&gcc);
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);
        assert!(!fixture.output_root.exists());

        let build = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &fixture.package,
            &gcc,
            &fixture.temporary_parent,
            &fixture.output_root,
        ))
        .expect("checked pipeline should create and materialize the missing leaf");

        assert!(fixture.output_root.is_dir());
        assert!(build.raw_crate_root().join("src/lib.rs").is_file());
        assert!(build.anchor_crate_root().join("src/lib.rs").is_file());
    }

    #[test]
    fn materialization_failure_removes_only_a_root_created_by_this_call() {
        let scratch = Scratch::new("cleanup-created");
        let requested = scratch.path().join("interop-generated");
        let output_root = preflight_generated_output_root(&requested).unwrap();

        let error = materialize_with_output_root(&output_root, |created_root| {
            fs::write(created_root.join("partial"), b"partial")
                .expect("simulate a partial materialization");
            Err::<(), _>(GeneratedFileMaterializationError::Io {
                path: created_root.join("failed"),
                source: std::io::Error::other("injected materialization failure"),
            })
        })
        .unwrap_err();

        assert!(matches!(error, H7InteropError::Materialization(_)));
        assert!(!requested.exists());
    }

    #[test]
    fn materialization_failure_preserves_a_preexisting_output_root() {
        let scratch = Scratch::new("cleanup-existing");
        let requested = scratch.path().join("interop-generated");
        fs::create_dir(&requested).unwrap();
        fs::write(requested.join("caller-owned"), b"keep").unwrap();
        let output_root = preflight_generated_output_root(&requested).unwrap();

        let error = materialize_with_output_root(&output_root, |existing_root| {
            Err::<(), _>(GeneratedFileMaterializationError::Io {
                path: existing_root.join("failed"),
                source: std::io::Error::other("injected materialization failure"),
            })
        })
        .unwrap_err();

        assert!(matches!(error, H7InteropError::Materialization(_)));
        assert_eq!(fs::read(requested.join("caller-owned")).unwrap(), b"keep");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_output_root_is_rejected_before_creation_or_tool_io() {
        use std::os::unix::fs::symlink;

        let fixture = H7Fixture::new(
            "output-symlink",
            "int fol_h7_value(void);\n",
            "int fol_h7_value(void) { return 42; }\n",
        );
        fs::write(fixture.package.join("native/provider.o"), b"not reached").unwrap();
        let target = fixture._scratch.path().join("output-target");
        fs::create_dir(&target).unwrap();
        symlink(&target, &fixture.output_root).unwrap();
        let missing_compiler = fixture.package.join("missing-gcc");
        let (graph, artifact) = h7_graph(crate::CERTIFIED_INTEROP_TARGET);

        let error = prepare_h7_interop(H7InteropRequest::new(
            &graph,
            artifact,
            &fixture.package,
            &missing_compiler,
            &fixture.temporary_parent,
            &fixture.output_root,
        ))
        .unwrap_err();

        assert!(matches!(
            error,
            H7InteropError::InvalidPath {
                role: "generated output root",
                ..
            }
        ));
        assert!(fs::read_dir(&target).unwrap().next().is_none());
    }
}

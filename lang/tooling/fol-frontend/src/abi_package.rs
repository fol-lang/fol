//! `fol tool abi package`: turn an installed prefix into a release archive.
//!
//! An installed prefix is what a consumer on this machine needs. A release
//! archive is what a consumer on a different machine needs, and the difference
//! is not just compression: it has to carry what the prefix leaves implicit --
//! which target it is for, what produced it, and a checksum a stranger can
//! verify -- and it must not carry what the prefix happens to have lying
//! around.
//!
//! The exclusion is the load-bearing part. FOL compiles through generated
//! Rust, so a build tree contains `Cargo.toml` files, a `.rs` facade, and the
//! GERC raw module. None of that is part of the C surface, and shipping it
//! would publish an implementation detail as though it were an interface.
//! Section 14's STOP names a public backend-Rust artifact as a reason V4
//! cannot close, so this refuses rather than filters: a prefix that contains
//! backend source is a bug upstream of packaging.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{
    cli::args::AbiPackageCommand, FrontendCommandResult, FrontendConfig, FrontendError,
    FrontendErrorKind, FrontendResult,
};

/// What a release archive is allowed to contain, as top-level directories.
const PUBLISHED_ROOTS: &[&str] = &["include", "lib", "share"];

/// File names and extensions that mean backend source escaped into the prefix.
const REFUSED_EXTENSIONS: &[&str] = &["rs"];
const REFUSED_NAMES: &[&str] = &["Cargo.toml", "Cargo.lock"];

pub fn abi_package_command(
    command: &AbiPackageCommand,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let prefix = resolve(&config.working_directory, &command.prefix);
    let prefix = prefix.canonicalize().map_err(|error| {
        invalid(format!(
            "could not read the prefix {}: {error}",
            prefix.display()
        ))
    })?;

    let mut files = Vec::new();
    for root in PUBLISHED_ROOTS {
        let directory = prefix.join(root);
        if directory.is_dir() {
            collect(&directory, &prefix, &mut files)?;
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(invalid(format!(
            "{} contains none of {}; it is not an installed prefix",
            prefix.display(),
            PUBLISHED_ROOTS.join(", ")
        )));
    }
    refuse_backend_source(&files)?;

    // The manifest is what makes the archive self-describing: the target and
    // the two fingerprints come from it rather than from the packaging
    // command's own idea of what was built.
    let manifest_path = sole_manifest(&prefix)?;
    let manifest = read_manifest(&manifest_path)?;

    let license = command
        .license
        .as_ref()
        .map(|path| resolve(&config.working_directory, path));
    let staged = stage(&prefix, &files, &manifest, license.as_deref())?;
    let out = resolve(&config.working_directory, &command.out);
    pack(&staged.root, &out)?;

    let mut result = FrontendCommandResult::new(
        "tool abi package",
        format!(
            "packaged {} for {} into {}",
            manifest.surface.artifact,
            manifest.surface.interface.target.rust_target_triple(),
            out.display()
        ),
    );
    result.artifacts.push(crate::FrontendArtifactSummary::new(
        crate::FrontendArtifactKind::ReleaseArchive,
        manifest.surface.artifact.clone(),
        Some(out),
    ));
    result.payload = Some(staged.report.join("\n"));
    Ok(result)
}

struct Staged {
    root: PathBuf,
    report: Vec<String>,
    /// Kept so the staging directory outlives the tar invocation.
    _fixture: TempDirectory,
}

/// Build the archive tree: the published roots verbatim, plus the three files
/// that make it verifiable.
fn stage(
    prefix: &Path,
    files: &[PathBuf],
    manifest: &fol_abi::AbiManifest,
    license: Option<&Path>,
) -> FrontendResult<Staged> {
    let fixture = TempDirectory::new("fol-abi-package")?;
    let root = fixture.path().join(&manifest.surface.artifact);
    std::fs::create_dir_all(&root)
        .map_err(|error| failed(format!("could not stage the archive: {error}")))?;

    for relative in files {
        let destination = root.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                failed(format!("could not stage {}: {error}", relative.display()))
            })?;
        }
        std::fs::copy(prefix.join(relative), &destination)
            .map_err(|error| failed(format!("could not stage {}: {error}", relative.display())))?;
    }

    if let Some(license) = license {
        std::fs::copy(license, root.join("LICENSE")).map_err(|error| {
            invalid(format!(
                "could not read the license {}: {error}",
                license.display()
            ))
        })?;
    }

    // Checksums over the staged bytes, so what is verified is what shipped.
    let mut checksums = String::new();
    let mut report = Vec::new();
    for relative in files {
        let bytes = std::fs::read(root.join(relative)).map_err(|error| {
            failed(format!("could not re-read {}: {error}", relative.display()))
        })?;
        checksums.push_str(&fol_abi::sha256_hex(&bytes));
        checksums.push_str("  ");
        checksums.push_str(&relative.display().to_string());
        checksums.push('\n');
    }
    write(&root.join("CHECKSUMS.sha256"), &checksums)?;

    let provenance = provenance_document(manifest, files.len());
    write(&root.join("PROVENANCE"), &provenance)?;
    write(&root.join("SBOM"), &sbom_document(manifest))?;

    report.push(format!("archive root: {}", manifest.surface.artifact));
    report.push(format!(
        "target: {}",
        manifest.surface.interface.target.rust_target_triple()
    ));
    report.push(format!(
        "abi: {}.{}",
        manifest.surface.major, manifest.surface.minor
    ));
    report.push(format!("files: {}", files.len()));
    for relative in files {
        report.push(format!("  {}", relative.display()));
    }
    report.push("  CHECKSUMS.sha256".to_string());
    report.push("  PROVENANCE".to_string());
    report.push("  SBOM".to_string());
    if license.is_some() {
        report.push("  LICENSE".to_string());
    }

    Ok(Staged {
        root,
        report,
        _fixture: fixture,
    })
}

fn provenance_document(manifest: &fol_abi::AbiManifest, files: usize) -> String {
    let surface = &manifest.surface;
    format!(
        "artifact: {}\ntarget: {}\nabi: {}.{}\nsymbols: {}\npublished-files: {files}\n\
         interface-fingerprint: {}\nbuild-fingerprint: {}\nfol: {}\n",
        surface.artifact,
        surface.interface.target.rust_target_triple(),
        surface.major,
        surface.minor,
        surface.interface.routines.len(),
        manifest.interface_fingerprint(),
        manifest.build_fingerprint(),
        env!("CARGO_PKG_VERSION"),
    )
}

/// The components that produced this surface, by pinned revision.
///
/// Deliberately not a generic dependency dump: what a consumer of a C library
/// can act on is which toolchain measured the layouts, because that is what
/// they would have to match to reproduce the archive.
fn sbom_document(manifest: &fol_abi::AbiManifest) -> String {
    let mut out = format!(
        "component: {}\nversion: {}.{}\ntype: c-library\ntarget: {}\n",
        manifest.surface.artifact,
        manifest.surface.major,
        manifest.surface.minor,
        manifest.surface.interface.target.rust_target_triple(),
    );
    out.push_str(&format!("producer: fol {}\n", env!("CARGO_PKG_VERSION")));
    for (name, revision) in [
        ("parc", fol_interop::LOCKED_PARC_REVISION),
        ("linc", fol_interop::LOCKED_LINC_REVISION),
        ("gerc", fol_interop::LOCKED_GERC_REVISION),
    ] {
        out.push_str(&format!("producer-component: {name}@{revision}\n"));
    }
    out
}

/// Every regular file under `directory`, as paths relative to `prefix`.
fn collect(directory: &Path, prefix: &Path, into: &mut Vec<PathBuf>) -> FrontendResult<()> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| invalid(format!("could not read {}: {error}", directory.display())))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| invalid(format!("could not read a directory entry: {error}")))?;
        let path = entry.path();
        if path.is_dir() {
            collect(&path, prefix, into)?;
        } else {
            let relative = path
                .strip_prefix(prefix)
                .map_err(|_| failed(format!("{} escaped the prefix", path.display())))?;
            into.push(relative.to_path_buf());
        }
    }
    Ok(())
}

/// Refuse a prefix carrying backend source.
fn refuse_backend_source(files: &[PathBuf]) -> FrontendResult<()> {
    let offenders: Vec<String> = files
        .iter()
        .filter(|relative| {
            let name = relative
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let extension = relative
                .extension()
                .map(|ext| ext.to_string_lossy().into_owned())
                .unwrap_or_default();
            REFUSED_NAMES.contains(&name.as_str())
                || REFUSED_EXTENSIONS.contains(&extension.as_str())
        })
        .map(|relative| relative.display().to_string())
        .collect();
    if offenders.is_empty() {
        return Ok(());
    }
    Err(invalid(format!(
        "the prefix contains backend source, which a release archive must not publish: {}",
        offenders.join(", ")
    ))
    .with_note(
        "a C release archive carries headers, libraries, and manifests; the generated Rust \
         facade and its Cargo files are implementation, not interface"
            .to_string(),
    ))
}

/// The one export manifest in the prefix.
fn sole_manifest(prefix: &Path) -> FrontendResult<PathBuf> {
    let directory = prefix.join("share/fol/abi");
    let entries = std::fs::read_dir(&directory).map_err(|error| {
        invalid(format!(
            "could not read {}: {error}; an installed prefix records its ABI there",
            directory.display()
        ))
    })?;
    let mut manifests: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".folabi.json"))
        .collect();
    manifests.sort();
    match manifests.len() {
        1 => Ok(manifests.remove(0)),
        0 => Err(invalid(format!(
            "{} has no .folabi.json; there is nothing to describe the archive",
            directory.display()
        ))),
        count => Err(invalid(format!(
            "{} holds {count} manifests; package one artifact at a time",
            directory.display()
        ))),
    }
}

fn read_manifest(path: &Path) -> FrontendResult<fol_abi::AbiManifest> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| invalid(format!("could not read {}: {error}", path.display())))?;
    fol_abi::AbiManifest::parse(&text)
        .map_err(|error| invalid(format!("{}: {error}", path.display())))
}

/// Pack the staged tree with `tar`.
///
/// Shelled out rather than implemented: an archive format is not something to
/// hand-roll when every platform FOL certifies ships a `tar` that produces the
/// format consumers already have a tool for.
fn pack(root: &Path, out: &Path) -> FrontendResult<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| failed(format!("could not create {}: {error}", parent.display())))?;
    }
    let parent = root
        .parent()
        .ok_or_else(|| failed("the staged archive has no parent".to_string()))?;
    let name = root
        .file_name()
        .ok_or_else(|| failed("the staged archive has no name".to_string()))?;

    let output = Command::new("tar")
        .arg("-czf")
        .arg(out)
        .arg("-C")
        .arg(parent)
        .arg(name)
        .output()
        .map_err(|error| failed(format!("could not run tar: {error}")))?;
    if !output.status.success() {
        return Err(failed(format!(
            "tar failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

fn write(path: &Path, contents: &str) -> FrontendResult<()> {
    std::fs::write(path, contents)
        .map_err(|error| failed(format!("could not write {}: {error}", path.display())))
}

fn resolve(working_directory: &Path, path: &str) -> PathBuf {
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        working_directory.join(candidate)
    }
}

fn invalid(message: String) -> FrontendError {
    FrontendError::new(FrontendErrorKind::InvalidInput, message)
}

fn failed(message: String) -> FrontendError {
    FrontendError::new(FrontendErrorKind::CommandFailed, message)
}

/// A staging directory removed when packaging finishes.
struct TempDirectory {
    path: PathBuf,
}

impl TempDirectory {
    fn new(label: &str) -> FrontendResult<Self> {
        let path = std::env::temp_dir().join(format!(
            "{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&path)
            .map_err(|error| failed(format!("could not create a staging directory: {error}")))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

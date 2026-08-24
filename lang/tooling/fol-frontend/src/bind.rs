//! `fol tool bind c`.
//!
//! The one place a FOL author asks for the C pipeline by hand. Everything it
//! needs is on the command line, and its whole output is one checked manifest
//! written where the author said to put it.

use std::path::{Path, PathBuf};

use crate::{
    cli::BindCCommand, FrontendCommandResult, FrontendConfig, FrontendError, FrontendErrorKind,
    FrontendResult,
};

pub fn bind_c_command(
    command: &BindCCommand,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let package_root = &config.working_directory;
    let target = resolve_target(command.target.as_deref(), config)?;
    let provider_kind = fol_package::BuildCImportProviderKind::parse(&command.provider_kind)
        .ok_or_else(|| {
            invalid(format!(
                "--provider-kind must be one of {}, got '{}'",
                fol_package::BuildCImportProviderKind::ACCEPTED.join(", "),
                command.provider_kind
            ))
        })?;
    let model = match command.fol_model.as_deref().unwrap_or("memo") {
        "core" => fol_abi::CapabilityModel::Core,
        "memo" => fol_abi::CapabilityModel::Memo,
        "std" => fol_abi::CapabilityModel::Std,
        other => {
            return Err(invalid(format!(
                "--fol-model must be core, memo, or std, got '{other}'"
            )))
        }
    };

    // The compiler and the probe directory are the two things the pipeline
    // cannot invent. Section 4.13 disables ambient discovery, so an unset
    // variable is a refusal rather than a search.
    let compiler = required_path(
        config.interop_compiler_override.as_deref(),
        "FOL_INTEROP_GCC",
    )?;
    let temporary_parent = required_path(
        config.interop_temporary_parent_override.as_deref(),
        "FOL_INTEROP_TEMP",
    )?;

    let manifest = fol_interop::bind_c(fol_interop::BindCRequest {
        alias: &command.alias,
        target,
        package_root,
        header: &package_root.join(&command.header),
        provider: &package_root.join(&command.provider),
        provider_kind,
        annotations: command
            .annotations
            .as_ref()
            .map(|path| package_root.join(path))
            .as_deref(),
        compiler,
        temporary_parent,
        model,
    })
    .map_err(|error| FrontendError::new(FrontendErrorKind::CommandFailed, format!("{error}")))?;

    let out = package_root.join(&command.out);
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            FrontendError::new(
                FrontendErrorKind::CommandFailed,
                format!("could not create {}: {error}", parent.display()),
            )
        })?;
    }
    let document = manifest.canonical_json();
    std::fs::write(&out, &document).map_err(|error| {
        FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!("could not write {}: {error}", out.display()),
        )
    })?;

    let mut result = FrontendCommandResult::new(
        "tool bind c",
        format!(
            "bound {} routine(s) from '{}' as namespace '{}' ({})",
            manifest.interface.routines.len(),
            command.header,
            manifest.interface.alias,
            manifest.interface_fingerprint(),
        ),
    );
    result.artifacts.push(crate::FrontendArtifactSummary::new(
        crate::FrontendArtifactKind::AbiManifest,
        manifest.interface.alias.clone(),
        Some(out),
    ));
    Ok(result)
}

fn resolve_target(
    explicit: Option<&str>,
    config: &FrontendConfig,
) -> FrontendResult<fol_types::ResolvedTarget> {
    match explicit.or(config.build_target_override.as_deref()) {
        Some(triple) => fol_types::ResolvedTarget::resolve(triple)
            .map_err(|error| invalid(format!("unknown target '{triple}': {error}"))),
        None => fol_types::ResolvedTarget::host()
            .map_err(|error| invalid(format!("could not resolve the host target: {error}"))),
    }
}

fn required_path<'a>(path: Option<&'a Path>, variable: &'static str) -> FrontendResult<&'a Path> {
    path.ok_or_else(|| {
        invalid(format!(
            "`fol tool bind c` requires explicit {variable} configuration; nothing is discovered \
             from the environment"
        ))
    })
}

fn invalid(message: impl Into<String>) -> FrontendError {
    FrontendError::new(FrontendErrorKind::InvalidInput, message)
}

/// Where a package's checked manifests live by convention.
///
/// Not enforced -- `--out` is explicit -- but the default the documentation and
/// the example both use, so a reader sees one location rather than three.
pub fn default_manifest_path(package_root: &Path, alias: &str) -> PathBuf {
    package_root
        .join("build/interop")
        .join(format!("{alias}.folabi.json"))
}

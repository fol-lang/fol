//! Toolchain checks that run before any output directory or build command.
//!
//! A target FOL supports is still unbuildable when its Rust standard library is
//! not installed. Discovering that from `rustc` mid-build leaves a half-written
//! output tree and reports `E0463` together with advice to run `rustup target
//! add` -- advice this project does not follow, since `flake.nix` is the only
//! toolchain source. Checking first turns that into one FOL diagnostic and no
//! files.

use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetToolchainError {
    /// `rustc` knows the target and its standard library is not installed.
    StandardLibraryMissing { target: String, libdir: String },
    /// `rustc` could not be run or did not report a library directory.
    ProbeFailed { target: String, detail: String },
}

impl std::fmt::Display for TargetToolchainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StandardLibraryMissing { target, libdir } => write!(
                f,
                "the Rust standard library for '{target}' is not installed, so nothing \
                 can be built for it; rustc expects it at {libdir}. Add the target to \
                 the toolchain in flake.nix and re-enter 'nix develop'"
            ),
            Self::ProbeFailed { target, detail } => write!(
                f,
                "could not determine whether the Rust standard library for '{target}' \
                 is installed: {detail}"
            ),
        }
    }
}

impl std::error::Error for TargetToolchainError {}

/// Verify that the selected target can actually be compiled for.
///
/// A host build is always fine and skips the probe, so the cost falls only on
/// cross builds.
pub fn ensure_target_toolchain_available(
    target: &fol_types::ResolvedTarget,
) -> Result<(), TargetToolchainError> {
    if target.runs_on_host().unwrap_or(false) {
        return Ok(());
    }
    let triple = target.rust_target_triple();

    let output = Command::new("rustc")
        .args(["--print", "target-libdir", "--target", triple])
        .output()
        .map_err(|error| TargetToolchainError::ProbeFailed {
            target: triple.to_string(),
            detail: error.to_string(),
        })?;
    if !output.status.success() {
        return Err(TargetToolchainError::ProbeFailed {
            target: triple.to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    let libdir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if libdir.is_empty() {
        return Err(TargetToolchainError::ProbeFailed {
            target: triple.to_string(),
            detail: "rustc reported no target library directory".to_string(),
        });
    }
    if !PathBuf::from(&libdir).is_dir() {
        return Err(TargetToolchainError::StandardLibraryMissing {
            target: triple.to_string(),
            libdir,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_target_toolchain_available, TargetToolchainError};

    #[test]
    fn the_host_target_is_always_available_without_probing() {
        let host = fol_types::ResolvedTarget::host().expect("host should resolve");
        assert_eq!(ensure_target_toolchain_available(&host), Ok(()));
    }

    /// A cross target either has its standard library or produces a diagnostic
    /// that names the target and the expected path. Which one depends on the
    /// toolchain the suite runs under, so both are accepted -- what is asserted
    /// is that the failure is FOL's own and never leaks rustup advice.
    #[test]
    fn a_cross_target_either_resolves_or_explains_itself() {
        let cross = fol_types::ResolvedTarget::resolve("aarch64-unknown-linux-gnu")
            .expect("a candidate target should resolve");
        match ensure_target_toolchain_available(&cross) {
            Ok(()) => {}
            Err(error) => {
                let text = error.to_string();
                assert!(text.contains("aarch64-unknown-linux-gnu"));
                assert!(
                    !text.contains("rustup"),
                    "the diagnostic must not repeat rustc's rustup advice: {text}"
                );
                assert!(
                    matches!(error, TargetToolchainError::StandardLibraryMissing { .. }),
                    "expected a missing-stdlib diagnostic, got: {error:?}"
                );
            }
        }
    }
}

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

/// A native tool the link step needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeTool {
    Linker,
    Archiver,
    CCompiler,
    SymbolReader,
}

impl NativeTool {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linker => "linker",
            Self::Archiver => "archiver",
            Self::CCompiler => "C compiler",
            Self::SymbolReader => "symbol reader",
        }
    }

    /// Candidate program names, most specific first.
    pub const fn candidates(self) -> &'static [&'static str] {
        match self {
            Self::Linker => &["ld", "ld.lld", "lld"],
            Self::Archiver => &["ar", "llvm-ar"],
            Self::CCompiler => &["cc", "clang", "gcc"],
            Self::SymbolReader => &["nm", "llvm-nm"],
        }
    }
}

/// Verify the native tools a link step needs are present.
///
/// Runs before compilation for the same reason the standard-library probe
/// does: discovering a missing archiver from `rustc`'s own error leaves a
/// half-built tree and reports someone else's diagnostic.
///
/// Only checked for products that actually reach a native link. An executable
/// built entirely by `rustc` needs no separate archiver.
pub fn ensure_native_tools_available(
    tools: &[NativeTool],
) -> Result<Vec<(NativeTool, String)>, TargetToolchainError> {
    let mut found = Vec::new();
    for tool in tools {
        let resolved = tool.candidates().iter().find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        });
        match resolved {
            Some(name) => found.push((*tool, (*name).to_string())),
            None => {
                return Err(TargetToolchainError::ProbeFailed {
                    target: tool.as_str().to_string(),
                    detail: format!(
                        "no {} found; tried {}",
                        tool.as_str(),
                        tool.candidates().join(", ")
                    ),
                })
            }
        }
    }
    Ok(found)
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

    /// The tools a native link step needs resolve on the certified lane.
    #[test]
    fn native_link_tools_resolve_or_say_what_they_tried() {
        use super::{ensure_native_tools_available, NativeTool};
        match ensure_native_tools_available(&[NativeTool::Archiver, NativeTool::CCompiler]) {
            Ok(found) => assert_eq!(found.len(), 2),
            Err(error) => {
                // Outside `nix develop` a tool can genuinely be absent. What
                // must hold is that the diagnostic names the tool and the
                // candidates tried, rather than surfacing someone else's error.
                let text = error.to_string();
                assert!(text.contains("tried"), "unhelpful diagnostic: {text}");
            }
        }
    }

    #[test]
    fn every_native_tool_has_candidates_and_a_name() {
        use super::NativeTool;
        for tool in [
            NativeTool::Linker,
            NativeTool::Archiver,
            NativeTool::CCompiler,
            NativeTool::SymbolReader,
        ] {
            assert!(!tool.candidates().is_empty(), "{tool:?} has no candidates");
            assert!(!tool.as_str().is_empty());
        }
        assert!(NativeTool::Archiver.candidates().contains(&"ar"));
        assert_eq!(NativeTool::CCompiler.as_str(), "C compiler");
    }
}

//! Running a built artifact. Both the workspace route and the direct
//! single-file route go through here, so `run` cannot end up transparent to a
//! child's output on one path and swallow it on the other.

use crate::{FrontendError, FrontendErrorKind, FrontendResult};
use std::path::Path;

/// Write an executed program's captured stdout/stderr through to the
/// frontend's own streams so `run` stays transparent to child output.
pub(crate) fn forward_child_output(stdout: &[u8], stderr: &[u8]) {
    use std::io::Write;
    if !stdout.is_empty() {
        let mut out = std::io::stdout();
        let _ = out.write_all(stdout);
        let _ = out.flush();
    }
    if !stderr.is_empty() {
        let mut err = std::io::stderr();
        let _ = err.write_all(stderr);
        let _ = err.flush();
    }
}

/// Run a built binary, forwarding whatever it printed **before** deciding
/// whether it failed: a program's output belongs to the user either way.
pub(crate) fn run_child_transparently(binary: &Path, args: &[String]) -> FrontendResult<()> {
    let output = std::process::Command::new(binary)
        .args(args)
        .output()
        .map_err(|error| FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string()))?;
    forward_child_output(&output.stdout, &output.stderr);
    if !output.status.success() {
        return Err(FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!(
                "run command failed for '{}': status {}",
                binary.display(),
                output.status
            ),
        ));
    }
    Ok(())
}

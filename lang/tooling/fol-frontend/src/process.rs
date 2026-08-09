//! Running a built artifact. Both the workspace route and the direct
//! single-file route go through here, so `run` cannot end up transparent to a
//! child's output on one path and swallow it on the other.

use crate::{FrontendError, FrontendErrorKind, FrontendResult};
use std::path::Path;

/// Push the frontend's own buffered output out before handing the terminal to
/// someone else.
pub(crate) fn flush_frontend_streams() {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
}

/// Run a built binary on the frontend's own streams.
///
/// `status()` leaves stdin, stdout, and stderr inherited, which is the whole
/// point: capturing them instead would mean the program never receives typed
/// input, its output arrives in one lump at exit rather than as it is
/// produced, and a terminal query answers about a pipe. Interactive and
/// full-screen programs need all three to be true.
pub(crate) fn run_child_transparently(binary: &Path, args: &[String]) -> FrontendResult<()> {
    // Whatever the frontend has already printed has to reach the terminal
    // before the child starts writing to the same one.
    flush_frontend_streams();

    let status = std::process::Command::new(binary)
        .args(args)
        .status()
        .map_err(|error| FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string()))?;
    if !status.success() {
        return Err(FrontendError::new(
            FrontendErrorKind::CommandFailed,
            format!(
                "run command failed for '{}': status {}",
                binary.display(),
                status
            ),
        ));
    }
    Ok(())
}

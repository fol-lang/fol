//! Running a built artifact. Both the workspace route and the direct
//! single-file route go through here, so `run` cannot end up transparent to a
//! child's output on one path and swallow it on the other.

use crate::{FrontendError, FrontendErrorKind, FrontendResult};
use std::path::Path;
use std::process::ExitStatus;

/// Status the frontend reports for a child that died on a signal, following
/// the shell's `128 + signo` convention.
#[cfg(unix)]
fn exit_code_for_status(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(1)
}

#[cfg(not(unix))]
fn exit_code_for_status(status: ExitStatus) -> i32 {
    status.code().unwrap_or(1)
}

/// Turn a launched program's own failing status into a frontend error that
/// says so. The program ran; nothing about the build or the package graph is
/// wrong, so this is never `CommandFailed`/F1004, and the child's status is
/// carried through instead of being flattened to 1.
pub(crate) fn artifact_status_error(
    verb: &str,
    binary: &Path,
    status: ExitStatus,
) -> FrontendError {
    FrontendError::new(
        FrontendErrorKind::ArtifactFailed,
        format!(
            "{verb} artifact '{}' exited with {status}",
            binary.display()
        ),
    )
    .with_note("the program itself reported this status; the build succeeded")
    .with_process_exit_code(exit_code_for_status(status))
}

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
        return Err(artifact_status_error("run", binary, status));
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::artifact_status_error;
    use crate::FrontendErrorKind;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;
    use std::process::ExitStatus;

    #[test]
    fn a_failing_child_keeps_its_own_status_and_is_not_a_build_error() {
        // Every child status used to collapse into F1004 ("a build or
        // configuration problem") plus a blanket exit 1.
        let error =
            artifact_status_error("run", Path::new("/tmp/demo"), ExitStatus::from_raw(3 << 8));

        assert_eq!(error.kind(), FrontendErrorKind::ArtifactFailed);
        assert_eq!(error.kind().diagnostic_code(), "F1005");
        assert_eq!(error.process_exit_code(), Some(3));
    }

    #[test]
    fn a_signalled_child_reports_the_shells_128_plus_signal_status() {
        let error = artifact_status_error("run", Path::new("/tmp/demo"), ExitStatus::from_raw(9));

        assert_eq!(error.process_exit_code(), Some(128 + 9));
    }
}

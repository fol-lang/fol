//! Scratch directories for tests, removed whether the test passes or not.
//!
//! Cleaning up on the last line of a test only cleans up when the test reaches
//! its last line: a failing assertion unwinds straight past it. A green suite
//! used to leave ~1000 directories and 2 GB behind, and a full `/tmp` does not
//! fail honestly -- writes start returning `Disk quota exceeded` from whatever
//! test happens to be running, and a fixture copy truncated mid-write surfaces
//! as parse errors pointing at lines that do not exist in the file.
//!
//! So ownership is a guard: [`TempFixture`] removes its directory on drop, and
//! drop runs on the unwind path too.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Every fixture of one test process lives under a single parent directory.
///
/// Drop cannot run when the process is killed outright -- a SIGKILL, an abort,
/// or the harness dying because the filesystem it was writing to filled up.
/// Grouping per process is what lets the *next* run identify those leftovers:
/// a bare `fol_*` directory could belong to a run that is still using it, but
/// `fol_test_run_<pid>` whose pid is gone cannot.
fn run_root() -> &'static Path {
    static RUN_ROOT: OnceLock<PathBuf> = OnceLock::new();
    RUN_ROOT.get_or_init(|| {
        reap_dead_runs();
        let root = std::env::temp_dir().join(format!("fol_test_run_{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("fixture run root should be creatable");
        root
    })
}

/// Delete the scratch space of test processes that are no longer running.
///
/// Linux-only, like the rest of the project: a pid is alive exactly when its
/// `/proc` entry exists. A pid that has been recycled by an unrelated process
/// only means we skip a directory this time and reap it later, so the check
/// errs toward keeping data rather than deleting a live run's fixtures.
fn reap_dead_runs() {
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|name| name.strip_prefix("fol_test_run_"))
        else {
            continue;
        };
        if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        if Path::new(&format!("/proc/{pid}")).exists() {
            continue;
        }
        std::fs::remove_dir_all(entry.path()).ok();
    }
}

fn unique_suffix() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the unix epoch")
        .as_nanos();
    format!("{stamp}_{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

/// A scratch directory that deletes itself when it goes out of scope.
///
/// Deref makes it usable anywhere a `&Path` is expected, so call sites read the
/// same as they did with a bare `PathBuf`. It deliberately does **not** create
/// the directory: tests that hand the path to code expected to create it (or to
/// fail because it is missing) depend on it not existing yet.
pub struct TempFixture {
    /// Removed on drop. Stays the top directory even when `path` points inside
    /// it, so a fixture nested in a subdirectory still cleans up completely.
    root: PathBuf,
    path: PathBuf,
}

impl TempFixture {
    pub fn new(label: &str) -> Self {
        let root = run_root().join(format!("{label}_{}", unique_suffix()));
        Self {
            path: root.clone(),
            root,
        }
    }

    /// Point the fixture at a subdirectory while still owning the whole tree.
    ///
    /// Used where a test needs a package inside a workspace: the package path is
    /// what the test works with, but deleting only the package would leave the
    /// workspace behind.
    #[must_use]
    pub fn child(mut self, relative: impl AsRef<Path>) -> Self {
        self.path = self.root.join(relative);
        self
    }

    /// Point the fixture at a file inside a freshly created directory.
    ///
    /// The directory is what makes the name unique, so the file itself can keep
    /// a fixed, readable name. Tests that need a single source file get one
    /// without hand-rolling a parent directory each time.
    #[must_use]
    pub fn with_file(self, name: impl AsRef<Path>) -> Self {
        std::fs::create_dir_all(&self.root).expect("fixture root should be creatable");
        self.child(name)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Keep the directory after the test ends.
    ///
    /// Nothing in the tree needs this today; it exists so that a test which
    /// genuinely must outlive its fixture says so out loud instead of quietly
    /// going back to a leaking `PathBuf`.
    #[allow(dead_code)]
    pub fn keep(self) -> PathBuf {
        let path = self.path.clone();
        std::mem::forget(self);
        path
    }
}

impl std::ops::Deref for TempFixture {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for TempFixture {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

/// Lets a fixture be passed straight to `Command` arguments and to the
/// `impl Into<PathBuf>` APIs the frontend uses, the same as a `Path` can.
impl AsRef<std::ffi::OsStr> for TempFixture {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.path.as_os_str()
    }
}

impl std::fmt::Debug for TempFixture {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(formatter)
    }
}

impl Drop for TempFixture {
    fn drop(&mut self) {
        // A test that already removed the tree itself, or never created it, is
        // the normal case rather than an error worth reporting.
        std::fs::remove_dir_all(&self.root).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `body`, swallowing the panic and keeping it out of the test log.
    fn capture_panic(body: impl FnOnce() + std::panic::UnwindSafe) {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(body);
        std::panic::set_hook(previous);
        assert!(result.is_err(), "the body should have panicked");
    }

    #[test]
    fn fixture_is_removed_when_a_test_panics() {
        // The whole point: cleanup written as the last statement of a test never
        // runs for the tests that actually leak, because those are the ones that
        // unwind before reaching it.
        let observed = std::sync::Mutex::new(PathBuf::new());
        capture_panic(|| {
            let fixture = TempFixture::new("panic_cleanup");
            std::fs::create_dir_all(fixture.join("src")).expect("fixture should be creatable");
            std::fs::write(fixture.join("src/main.fol"), "fun[] main(): int = {};")
                .expect("fixture file should be writable");
            *observed.lock().expect("lock should be held") = fixture.path().to_path_buf();
            panic!("simulated test failure");
        });

        let path = observed.lock().expect("lock should be held").clone();
        assert!(
            !path.exists(),
            "fixture '{}' should be removed on the unwind path",
            path.display()
        );
    }

    #[test]
    fn child_fixture_removes_the_whole_tree_not_just_the_subdirectory() {
        let (root, child) = {
            let fixture = TempFixture::new("child_cleanup").child("workspace");
            std::fs::create_dir_all(fixture.path()).expect("child should be creatable");
            (fixture.root.clone(), fixture.path().to_path_buf())
        };

        assert!(!child.exists(), "the child directory should be removed");
        assert!(
            !root.exists(),
            "the owning root '{}' should be removed too, not left as a husk",
            root.display()
        );
    }

    #[test]
    fn reaper_removes_scratch_space_of_processes_that_are_gone() {
        // Pid 0 is never a live process, so this stands in for a run that was
        // killed before it could unwind.
        let dead = std::env::temp_dir().join("fol_test_run_0");
        std::fs::create_dir_all(dead.join("leftover")).expect("dead run root should be creatable");

        // A live run's scratch space must survive the sweep.
        let live = run_root().to_path_buf();
        std::fs::create_dir_all(&live).expect("live run root should exist");

        reap_dead_runs();

        assert!(!dead.exists(), "a dead run's fixtures should be reaped");
        assert!(live.exists(), "the running process's own root must survive");
    }
}

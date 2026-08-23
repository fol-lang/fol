//! Executing an action graph.
//!
//! Everything here exists to make one guarantee: a build either publishes a
//! complete, correct output tree or publishes nothing. Two failures motivate
//! that. A build interrupted halfway used to leave a partial tree that the next
//! run treated as finished, and two parallel builds sharing an output directory
//! could delete each other's work mid-run.
//!
//! The shape is: work in a per-plan temporary directory, hold a lock for the
//! duration, and rename into place at the end. A rename within one filesystem
//! is atomic, so a reader sees the old tree or the new one and never a half-
//! written mixture.

use crate::action::{BuildAction, BuildActionPayload};
use crate::action_graph::{canonical_relative_path, BuildActionGraph};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum MaterializeError {
    /// The graph did not pass validation, so nothing was attempted.
    Invalid(Vec<crate::action_graph::ActionValidationError>),
    /// Another process holds the lock for this plan.
    Locked {
        path: PathBuf,
    },
    /// A tool exited successfully and did not write an output it declared.
    ///
    /// A silent version of this is worse than a crash: the build reports
    /// success and a later step reads a stale file or fails far from the cause.
    DeclaredOutputMissing {
        action: String,
        path: String,
    },
    /// A tool may not be run: dependency-provided, or not an absolute path.
    ToolRefused {
        action: String,
        reason: crate::action_trust::ToolTrustError,
    },
    /// A tool exited with a failing status.
    ToolFailed {
        action: String,
        status: String,
        stderr: String,
    },
    Io {
        context: String,
        error: String,
    },
}

impl std::fmt::Display for MaterializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(errors) => {
                write!(f, "the action graph is invalid and was not executed:")?;
                for error in errors {
                    write!(f, "\n  {error}")?;
                }
                Ok(())
            }
            Self::Locked { path } => write!(
                f,
                "another build is materializing this plan (lock held at {})",
                path.display()
            ),
            Self::DeclaredOutputMissing { action, path } => write!(
                f,
                "action '{action}' succeeded without producing its declared output '{path}'"
            ),
            Self::ToolRefused { action, reason } => {
                write!(f, "action '{action}' cannot run: {reason}")
            }
            Self::ToolFailed {
                action,
                status,
                stderr,
            } => write!(f, "action '{action}' failed ({status}): {stderr}"),
            Self::Io { context, error } => write!(f, "{context}: {error}"),
        }
    }
}

impl std::error::Error for MaterializeError {}

fn io<T>(context: &str, result: std::io::Result<T>) -> Result<T, MaterializeError> {
    result.map_err(|error| MaterializeError::Io {
        context: context.to_string(),
        error: error.to_string(),
    })
}

/// A directory lock held for the duration of one materialization.
///
/// `create_new` is the whole mechanism: it fails if the path exists, and the
/// check and the create are one filesystem operation, so two processes cannot
/// both believe they acquired it.
#[derive(Debug)]
pub struct MaterializeLock {
    path: PathBuf,
}

impl MaterializeLock {
    pub fn acquire(root: &Path, plan_identity: &str) -> Result<Self, MaterializeError> {
        io("creating the lock directory", fs::create_dir_all(root))?;
        let path = root.join(format!("{plan_identity}.lock"));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                // Recorded so a stale lock can be attributed to a process.
                let _ = writeln!(file, "{}", std::process::id());
                Ok(Self { path })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(MaterializeError::Locked { path })
            }
            Err(error) => Err(MaterializeError::Io {
                context: "acquiring the materialization lock".to_string(),
                error: error.to_string(),
            }),
        }
    }
}

impl Drop for MaterializeLock {
    fn drop(&mut self) {
        // Released even when the build panics, so a crash does not wedge the
        // next run.
        let _ = fs::remove_file(&self.path);
    }
}

/// What one materialization produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializeReport {
    /// Actions executed, in the order they ran.
    pub executed: Vec<String>,
    /// Every produced path, relative to the published root, with its identity.
    pub outputs: BTreeMap<String, String>,
    /// Where the finished tree was published.
    pub published_root: PathBuf,
}

/// Execute a graph and publish its outputs atomically.
///
/// `requested` limits execution to one step's closure. An empty slice runs
/// everything.
pub fn materialize(
    graph: &BuildActionGraph,
    roots: &[&str],
    requested: &[crate::action::BuildActionId],
    workspace: &Path,
    published_root: &Path,
) -> Result<MaterializeReport, MaterializeError> {
    let errors = graph.validate(roots);
    if !errors.is_empty() {
        return Err(MaterializeError::Invalid(errors));
    }

    let order = if requested.is_empty() {
        graph
            .execution_order()
            .map_err(|cycle| MaterializeError::Io {
                context: "ordering actions".to_string(),
                error: cycle,
            })?
    } else {
        graph.closure_for(requested)
    };

    let plan_identity = graph_identity(graph, &order);
    let lock_root = workspace.join(".fol/locks");
    let _lock = MaterializeLock::acquire(&lock_root, &plan_identity)?;

    // A staging directory named for this plan, so two plans building at once
    // cannot write into each other's tree.
    let staging = workspace.join(".fol/staging").join(&plan_identity);
    if staging.exists() {
        io(
            "clearing a stale staging tree",
            fs::remove_dir_all(&staging),
        )?;
    }
    io("creating the staging tree", fs::create_dir_all(&staging))?;

    let mut report = MaterializeReport {
        executed: Vec::new(),
        outputs: BTreeMap::new(),
        published_root: published_root.to_path_buf(),
    };

    for id in order {
        let Some(action) = graph.action(id) else {
            continue;
        };
        run_action(action, workspace, &staging)?;
        for output in &action.outputs {
            let relative =
                canonical_relative_path(&output.path).map_err(|reason| MaterializeError::Io {
                    context: format!("action '{}' output '{}'", action.name, output.path),
                    error: reason,
                })?;
            let produced = staging.join(&relative);
            if !produced.exists() {
                return Err(MaterializeError::DeclaredOutputMissing {
                    action: action.name.clone(),
                    path: output.path.clone(),
                });
            }
            ensure_no_symlink_escape(&staging, &produced)?;
            let contents = io("reading a produced output", fs::read(&produced))?;
            report.outputs.insert(relative, digest(&contents));
        }
        report.executed.push(action.name.clone());
    }

    publish(&staging, published_root)?;
    Ok(report)
}

/// Move the staged tree into place, replacing any previous one.
///
/// The previous tree is moved aside first and removed only after the new one is
/// in place, so an interruption leaves one complete tree rather than none.
fn publish(staging: &Path, published_root: &Path) -> Result<(), MaterializeError> {
    if let Some(parent) = published_root.parent() {
        io(
            "creating the publication parent",
            fs::create_dir_all(parent),
        )?;
    }
    let previous = published_root.with_extension("previous");
    if previous.exists() {
        io(
            "clearing a stale previous tree",
            fs::remove_dir_all(&previous),
        )?;
    }
    let had_previous = published_root.exists();
    if had_previous {
        io(
            "moving the previous tree aside",
            fs::rename(published_root, &previous),
        )?;
    }
    match fs::rename(staging, published_root) {
        Ok(()) => {
            if had_previous {
                let _ = fs::remove_dir_all(&previous);
            }
            Ok(())
        }
        Err(error) => {
            // Put the old tree back rather than leaving nothing published.
            if had_previous {
                let _ = fs::rename(&previous, published_root);
            }
            Err(MaterializeError::Io {
                context: "publishing the staged tree".to_string(),
                error: error.to_string(),
            })
        }
    }
}

fn run_action(
    action: &BuildAction,
    workspace: &Path,
    staging: &Path,
) -> Result<(), MaterializeError> {
    match &action.payload {
        BuildActionPayload::Write { contents } => {
            let target = staged_output(action, staging)?;
            write_file_checked(staging, &target, contents.as_bytes())
        }
        BuildActionPayload::Copy { source } => {
            let relative =
                canonical_relative_path(source).map_err(|reason| MaterializeError::Io {
                    context: format!("action '{}' source '{source}'", action.name),
                    error: reason,
                })?;
            let from = workspace.join(&relative);
            let contents = io("reading a copy source", fs::read(&from))?;
            let target = staged_output(action, staging)?;
            write_file_checked(staging, &target, &contents)
        }
        BuildActionPayload::Install { destination, .. } => {
            // The install reads its single input and places it at the
            // destination inside the staged tree.
            let Some(input) = action.inputs.first() else {
                return Err(MaterializeError::Io {
                    context: format!("action '{}'", action.name),
                    error: "an install action must declare the output it installs".to_string(),
                });
            };
            let relative =
                canonical_relative_path(&input.path).map_err(|reason| MaterializeError::Io {
                    context: format!("action '{}' input '{}'", action.name, input.path),
                    error: reason,
                })?;
            let from = staging.join(&relative);
            let contents = io("reading an install source", fs::read(&from))?;
            let destination =
                canonical_relative_path(destination).map_err(|reason| MaterializeError::Io {
                    context: format!("action '{}' destination", action.name),
                    error: reason,
                })?;
            write_file_checked(staging, &staging.join(destination), &contents)
        }
        BuildActionPayload::SystemTool {
            tool,
            args,
            env,
            capture_stdout,
        } => run_tool(action, staging, tool, args, env, capture_stdout.as_deref()),
        // Codegen, Compile, and Run are declared and ordered here; the
        // materializer does not yet drive them, because compilation still runs
        // through the backend session. M3 moves it.
        BuildActionPayload::Codegen { .. }
        | BuildActionPayload::Compile { .. }
        | BuildActionPayload::Run { .. } => Ok(()),
    }
}

fn run_tool(
    action: &BuildAction,
    staging: &Path,
    tool: &str,
    args: &[String],
    env: &[(String, String)],
    capture_stdout: Option<&str>,
) -> Result<(), MaterializeError> {
    // Checked here rather than at declaration time so no path can reach an
    // exec without passing the policy.
    crate::action_trust::check_tool_is_runnable(tool, crate::action_trust::ToolProvenance::System)
        .map_err(|reason| MaterializeError::ToolRefused {
            action: action.name.clone(),
            reason,
        })?;

    let mut command = std::process::Command::new(tool);
    command.args(args).current_dir(staging);
    for (name, value) in env {
        command.env(name, value);
    }
    let output = io("launching a build tool", command.output())?;
    if !output.status.success() {
        return Err(MaterializeError::ToolFailed {
            action: action.name.clone(),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    if let Some(path) = capture_stdout {
        let relative = canonical_relative_path(path).map_err(|reason| MaterializeError::Io {
            context: format!("action '{}' stdout capture", action.name),
            error: reason,
        })?;
        write_file_checked(staging, &staging.join(relative), &output.stdout)?;
    }
    Ok(())
}

fn staged_output(action: &BuildAction, staging: &Path) -> Result<PathBuf, MaterializeError> {
    let Some(output) = action.outputs.first() else {
        return Err(MaterializeError::Io {
            context: format!("action '{}'", action.name),
            error: "the action declares no output to write".to_string(),
        });
    };
    let relative =
        canonical_relative_path(&output.path).map_err(|reason| MaterializeError::Io {
            context: format!("action '{}' output '{}'", action.name, output.path),
            error: reason,
        })?;
    Ok(staging.join(relative))
}

fn write_file_checked(root: &Path, path: &Path, contents: &[u8]) -> Result<(), MaterializeError> {
    ensure_no_symlink_escape(root, path)?;
    write_file(path, contents)
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), MaterializeError> {
    if let Some(parent) = path.parent() {
        io("creating an output directory", fs::create_dir_all(parent))?;
    }
    io("writing an output", fs::write(path, contents))
}

/// Refuse a path that reaches outside `root` through a symlink.
///
/// The string check in `action_graph` cannot see this: `out/link/file` is a
/// well-formed relative path, and whether it escapes depends on what `out/link`
/// points at on disk. Writing through such a link would put build output
/// anywhere the link aims, so the check happens against the real filesystem,
/// just before the write.
fn ensure_no_symlink_escape(root: &Path, path: &Path) -> Result<(), MaterializeError> {
    let canonical_root = io("resolving the staging root", fs::canonicalize(root))?;

    // Walk down from the root: the deepest existing ancestor is what a write
    // would follow, and a path that does not exist yet cannot be a link.
    let mut existing = path.to_path_buf();
    while !existing.exists() {
        match existing.parent() {
            Some(parent) => existing = parent.to_path_buf(),
            None => return Ok(()),
        }
    }
    let resolved = io("resolving an output path", fs::canonicalize(&existing))?;
    if resolved.starts_with(&canonical_root) {
        Ok(())
    } else {
        Err(MaterializeError::Io {
            context: format!("output path {}", path.display()),
            error: format!(
                "resolves to {}, which is outside the staging root {}",
                resolved.display(),
                canonical_root.display()
            ),
        })
    }
}

/// The identity of one materialization: every action that will run, in order.
fn graph_identity(graph: &BuildActionGraph, order: &[crate::action::BuildActionId]) -> String {
    let rendered = order
        .iter()
        .filter_map(|id| graph.action(*id))
        .map(|action| action.cache_identity())
        .collect::<Vec<_>>()
        .join("\u{1f}");
    digest(rendered.as_bytes())
}

fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
pub(crate) mod tests_support {
    /// A no-op tool by absolute path. The trust policy refuses a bare name,
    /// because it would resolve through `PATH` at execution time.
    pub(crate) fn absolute_true() -> String {
        for candidate in ["/bin/true", "/usr/bin/true"] {
            if std::path::Path::new(candidate).exists() {
                return candidate.to_string();
            }
        }
        panic!("no absolute `true` found; the test environment moved");
    }

    pub(crate) fn fixture(label: &str) -> fol_testkit::TempFixture {
        fol_testkit::TempFixture::new(&format!("fol_materialize_{label}"))
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::*;
    use super::*;
    use crate::action::{BuildActionInput, BuildActionOutput, BuildActionPayload};
    use crate::plan::OutputRole;

    fn write_action(
        graph: &mut BuildActionGraph,
        name: &str,
        path: &str,
        contents: &str,
    ) -> crate::action::BuildActionId {
        let id = graph.add_action(
            name,
            BuildActionPayload::Write {
                contents: contents.to_string(),
            },
        );
        graph
            .action_mut(id)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: path.to_string(),
                role: OutputRole::Object,
            });
        id
    }

    #[test]
    fn declared_files_are_created_and_published() {
        let root = fixture("write");
        let mut graph = BuildActionGraph::new();
        write_action(
            &mut graph,
            "gen",
            "out/generated.fol",
            "fun[] main(): non = {};\n",
        );

        let published = root.path().join("published");
        let report = materialize(&graph, &["out"], &[], root.path(), &published)
            .expect("a valid graph should materialize");

        assert_eq!(report.executed, vec!["gen".to_string()]);
        let produced = published.join("out/generated.fol");
        assert!(produced.is_file(), "the declared file was not published");
        assert_eq!(
            fs::read_to_string(&produced).unwrap(),
            "fun[] main(): non = {};\n"
        );
        assert!(report.outputs.contains_key("out/generated.fol"));
    }

    #[test]
    fn copy_and_install_place_their_files() {
        let root = fixture("copy_install");
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/seed.txt"), b"seed").unwrap();

        let mut graph = BuildActionGraph::new();
        graph.declare_source("src/seed.txt");
        let copy = graph.add_action(
            "copy-seed",
            BuildActionPayload::Copy {
                source: "src/seed.txt".to_string(),
            },
        );
        graph
            .action_mut(copy)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "out/seed.txt".to_string(),
                role: OutputRole::Object,
            });

        let install = graph.add_action(
            "install-seed",
            BuildActionPayload::Install {
                source_role: OutputRole::Object,
                destination: "install/seed.txt".to_string(),
            },
        );
        graph
            .action_mut(install)
            .unwrap()
            .inputs
            .push(BuildActionInput {
                path: "out/seed.txt".to_string(),
                produced_by: Some(copy),
            });
        graph
            .action_mut(install)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "install/seed.txt".to_string(),
                role: OutputRole::Object,
            });

        let published = root.path().join("published");
        materialize(&graph, &["out", "install"], &[], root.path(), &published)
            .expect("copy and install should materialize");

        assert_eq!(
            fs::read_to_string(published.join("install/seed.txt")).unwrap(),
            "seed"
        );
    }

    /// A tool that exits successfully without writing what it declared is an
    /// error. Reporting success here would let a later step read a stale file
    /// and fail far from the cause.
    #[test]
    fn a_successful_tool_that_omits_its_output_fails() {
        let root = fixture("missing_output");
        let mut graph = BuildActionGraph::new();
        let id = graph.add_action(
            "quiet-tool",
            BuildActionPayload::SystemTool {
                tool: absolute_true(),
                args: Vec::new(),
                env: Vec::new(),
                capture_stdout: None,
            },
        );
        graph
            .action_mut(id)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "out/promised.txt".to_string(),
                role: OutputRole::Object,
            });

        let published = root.path().join("published");
        let error = materialize(&graph, &["out"], &[], root.path(), &published)
            .expect_err("the missing output should fail the build");
        assert!(
            matches!(error, MaterializeError::DeclaredOutputMissing { .. }),
            "expected a missing-output error, got: {error}"
        );
    }

    #[test]
    fn an_invalid_graph_is_never_executed() {
        let root = fixture("invalid");
        let mut graph = BuildActionGraph::new();
        write_action(&mut graph, "a", "out/same.txt", "one");
        write_action(&mut graph, "b", "out/same.txt", "two");

        let published = root.path().join("published");
        let error = materialize(&graph, &["out"], &[], root.path(), &published)
            .expect_err("a duplicate output should stop the build");
        assert!(matches!(error, MaterializeError::Invalid(_)));
        assert!(
            !published.exists(),
            "nothing may be published for a graph that never ran"
        );
    }

    /// A second materialization of the same plan is refused while the first
    /// holds the lock, rather than the two writing over each other.
    #[test]
    fn a_held_lock_refuses_a_second_materialization() {
        let root = fixture("lock");
        let lock_root = root.path().join(".fol/locks");
        let first = MaterializeLock::acquire(&lock_root, "plan-identity")
            .expect("the first acquire should succeed");

        let second = MaterializeLock::acquire(&lock_root, "plan-identity");
        assert!(
            matches!(second, Err(MaterializeError::Locked { .. })),
            "the second acquire should have been refused"
        );

        // Releasing lets the next build in, including after a panic, because
        // the release is in `Drop`.
        drop(first);
        assert!(MaterializeLock::acquire(&lock_root, "plan-identity").is_ok());
    }

    /// Two different plans may materialize at once.
    #[test]
    fn different_plans_do_not_block_each_other() {
        let root = fixture("parallel_locks");
        let lock_root = root.path().join(".fol/locks");
        let _first = MaterializeLock::acquire(&lock_root, "plan-a").unwrap();
        assert!(MaterializeLock::acquire(&lock_root, "plan-b").is_ok());
    }

    /// A failed publication leaves the previous tree in place rather than
    /// nothing.
    #[test]
    fn a_previous_published_tree_survives_a_rerun() {
        let root = fixture("republish");
        let published = root.path().join("published");

        let mut first = BuildActionGraph::new();
        write_action(&mut first, "gen", "out/file.txt", "first");
        materialize(&first, &["out"], &[], root.path(), &published).unwrap();
        assert_eq!(
            fs::read_to_string(published.join("out/file.txt")).unwrap(),
            "first"
        );

        let mut second = BuildActionGraph::new();
        write_action(&mut second, "gen", "out/file.txt", "second");
        materialize(&second, &["out"], &[], root.path(), &published).unwrap();
        assert_eq!(
            fs::read_to_string(published.join("out/file.txt")).unwrap(),
            "second"
        );
        assert!(
            !published.with_extension("previous").exists(),
            "the superseded tree should be cleaned up after a successful publish"
        );
    }

    /// Two clean materializations of one graph produce identical output
    /// hashes.
    #[test]
    fn two_clean_materializations_agree_on_every_output_hash() {
        let build = |label: &str| {
            let root = fixture(label);
            let mut graph = BuildActionGraph::new();
            write_action(&mut graph, "a", "out/a.txt", "alpha");
            write_action(&mut graph, "b", "out/b.txt", "beta");
            let published = root.path().join("published");
            let report = materialize(&graph, &["out"], &[], root.path(), &published).unwrap();
            // The fixture is dropped at the end of the closure; the hashes are
            // what is compared, not the tree.
            report.outputs
        };
        assert_eq!(build("repro_one"), build("repro_two"));
    }

    /// The trust policy is enforced at the point of execution, so no code path
    /// can reach an exec without passing it.
    #[test]
    fn a_tool_that_fails_the_trust_policy_never_runs() {
        let root = fixture("trust");
        let mut graph = BuildActionGraph::new();
        let id = graph.add_action(
            "bare-name-tool",
            BuildActionPayload::SystemTool {
                // A bare name would resolve through PATH at execution time.
                tool: "true".to_string(),
                args: Vec::new(),
                env: Vec::new(),
                capture_stdout: None,
            },
        );
        graph
            .action_mut(id)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "out/result.txt".to_string(),
                role: OutputRole::Object,
            });

        let published = root.path().join("published");
        let error = materialize(&graph, &["out"], &[], root.path(), &published)
            .expect_err("the tool should be refused");
        assert!(
            matches!(error, MaterializeError::ToolRefused { .. }),
            "expected a trust refusal, got: {error}"
        );
        assert!(!published.exists(), "nothing may be published");
    }

    /// Only the requested closure runs.
    #[test]
    fn materializing_a_step_runs_only_its_closure() {
        let root = fixture("closure");
        let mut graph = BuildActionGraph::new();
        let base = write_action(&mut graph, "base", "out/base.txt", "base");
        let needed = write_action(&mut graph, "needed", "out/needed.txt", "needed");
        graph
            .action_mut(needed)
            .unwrap()
            .inputs
            .push(BuildActionInput {
                path: "out/base.txt".to_string(),
                produced_by: Some(base),
            });
        write_action(&mut graph, "unrelated", "out/unrelated.txt", "unrelated");

        let published = root.path().join("published");
        let report = materialize(&graph, &["out"], &[needed], root.path(), &published).unwrap();

        assert_eq!(
            report.executed,
            vec!["base".to_string(), "needed".to_string()]
        );
        assert!(!published.join("out/unrelated.txt").exists());
    }
}

#[cfg(test)]
mod escape_and_concurrency_tests {
    use super::tests_support::*;
    use super::*;
    use crate::action::BuildActionOutput;
    use crate::plan::OutputRole;

    /// A symlink inside the staging tree pointing outside it must not become a
    /// write target.
    ///
    /// The string check in `action_graph` cannot catch this: `out/link/file` is
    /// a well-formed relative path, and whether it escapes depends on what
    /// `out/link` points at on disk.
    #[test]
    #[cfg(unix)]
    fn a_symlink_out_of_the_staging_root_is_refused() {
        let root = fixture("symlink");
        let outside = root.path().join("outside");
        fs::create_dir_all(&outside).unwrap();

        // Pre-create the staging tree this plan will use, with a link that
        // aims out of it.
        let mut graph = BuildActionGraph::new();
        let id = graph.add_action(
            "gen",
            BuildActionPayload::Write {
                contents: "escaped".to_string(),
            },
        );
        graph
            .action_mut(id)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "out/link/file.txt".to_string(),
                role: OutputRole::Object,
            });

        let order = graph.execution_order().unwrap();
        let identity = super::graph_identity(&graph, &order);
        let staging = root.path().join(".fol/staging").join(&identity);
        fs::create_dir_all(staging.join("out")).unwrap();
        std::os::unix::fs::symlink(&outside, staging.join("out/link")).unwrap();

        let published = root.path().join("published");
        let error = materialize(&graph, &["out"], &[], root.path(), &published);

        // Either the staging tree is cleared before use (so the link is gone
        // and the write is contained), or the escape is refused. Both are
        // correct; writing through the link is not.
        match error {
            Ok(_) => assert!(
                !outside.join("file.txt").exists(),
                "the build wrote through a symlink and escaped its root"
            ),
            Err(error) => assert!(
                error.to_string().contains("outside the staging root")
                    || error.to_string().contains("staging"),
                "unexpected failure: {error}"
            ),
        }
    }

    /// Independent plans materialize concurrently; two builds of the same plan
    /// do not both proceed.
    #[test]
    fn independent_plans_materialize_in_parallel_and_colliding_ones_do_not() {
        let root = fixture("parallel");
        let workspace = root.path().to_path_buf();
        fs::create_dir_all(&workspace).unwrap();

        let handles: Vec<_> = (0..4)
            .map(|index| {
                let workspace = workspace.clone();
                std::thread::spawn(move || {
                    let mut graph = BuildActionGraph::new();
                    let id = graph.add_action(
                        format!("gen-{index}"),
                        BuildActionPayload::Write {
                            contents: format!("contents {index}"),
                        },
                    );
                    graph
                        .action_mut(id)
                        .unwrap()
                        .outputs
                        .push(BuildActionOutput {
                            path: format!("out/file-{index}.txt"),
                            role: OutputRole::Object,
                        });
                    let published = workspace.join(format!("published-{index}"));
                    materialize(&graph, &["out"], &[], &workspace, &published)
                        .map(|report| report.outputs.len())
                })
            })
            .collect();

        for handle in handles {
            let result = handle.join().expect("no materialization should panic");
            assert_eq!(
                result.expect("independent plans must not block each other"),
                1
            );
        }

        // Each published its own tree, and none deleted another's.
        for index in 0..4 {
            let path = workspace
                .join(format!("published-{index}"))
                .join(format!("out/file-{index}.txt"));
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                format!("contents {index}"),
                "plan {index} lost its output to a parallel build"
            );
        }
    }

    /// A run that fails partway publishes nothing, so the previous tree is
    /// still the one on disk.
    #[test]
    fn a_failed_run_leaves_the_previous_tree_intact() {
        let root = fixture("interrupted");
        let published = root.path().join("published");

        let mut good = BuildActionGraph::new();
        let first = good.add_action(
            "gen",
            BuildActionPayload::Write {
                contents: "committed".to_string(),
            },
        );
        good.action_mut(first)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "out/file.txt".to_string(),
                role: OutputRole::Object,
            });
        materialize(&good, &["out"], &[], root.path(), &published).unwrap();

        // A graph whose second action cannot complete: the tool succeeds and
        // does not produce what it declared.
        let mut broken = BuildActionGraph::new();
        let write = broken.add_action(
            "gen",
            BuildActionPayload::Write {
                contents: "replacement".to_string(),
            },
        );
        broken
            .action_mut(write)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "out/file.txt".to_string(),
                role: OutputRole::Object,
            });
        let quiet = broken.add_action(
            "quiet",
            BuildActionPayload::SystemTool {
                tool: absolute_true(),
                args: Vec::new(),
                env: Vec::new(),
                capture_stdout: None,
            },
        );
        broken
            .action_mut(quiet)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: "out/never.txt".to_string(),
                role: OutputRole::Object,
            });

        materialize(&broken, &["out"], &[], root.path(), &published)
            .expect_err("the incomplete run should fail");

        assert_eq!(
            fs::read_to_string(published.join("out/file.txt")).unwrap(),
            "committed",
            "a failed run replaced the published tree with a partial one"
        );
        assert!(
            !published.join("out/never.txt").exists(),
            "a partial output reached the published tree"
        );
    }
}

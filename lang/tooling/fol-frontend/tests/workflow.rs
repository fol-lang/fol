use fol_frontend::{run_command_from_args_in_dir, FrontendArtifactKind};
use std::fs;
use std::path::PathBuf;

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fol_frontend_workflow_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
}

#[test]
fn package_workflow_walkthrough_feels_like_one_canonical_tool_flow() {
    let root = temp_root("package");
    fs::create_dir_all(&root).expect("should create workflow root");

    let (_, init) = run_command_from_args_in_dir(["fol", "work", "init", "--bin"], &root)
        .expect("init should succeed");
    let (_, fetch) = run_command_from_args_in_dir(["fol", "pack", "fetch"], &root)
        .expect("fetch should succeed");
    let (_, check) = run_command_from_args_in_dir(["fol", "code", "check"], &root)
        .expect("check should succeed");
    let (_, build) = run_command_from_args_in_dir(["fol", "code", "build"], &root)
        .expect("build should succeed");
    let (_, run) =
        run_command_from_args_in_dir(["fol", "code", "run"], &root).expect("run should succeed");

    assert_eq!(init.command, "init");
    assert_eq!(fetch.command, "fetch");
    assert_eq!(check.command, "check");
    assert_eq!(build.command, "build");
    assert_eq!(run.command, "run");

    assert!(build
        .summary
        .contains(&root.join(".fol/build/debug").display().to_string()));
    assert!(fetch
        .summary
        .contains(&root.join(".fol/pkg").display().to_string()));
    assert_eq!(build.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
    let binary = build
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == FrontendArtifactKind::Binary)
        .expect("build should report a binary artifact");
    assert!(binary
        .path
        .as_ref()
        .expect("binary path should exist")
        .is_file());
    assert_eq!(run.artifacts[0].kind, FrontendArtifactKind::Binary);

    fs::remove_dir_all(root).ok();
}

#[test]
fn workspace_workflow_walkthrough_reports_member_and_artifact_roots() {
    let root = temp_root("workspace");
    let app = root.join("app");
    let lib = root.join("lib");
    fs::create_dir_all(&app).expect("should create app root");
    fs::create_dir_all(&lib).expect("should create lib root");

    run_command_from_args_in_dir(["fol", "work", "init", "--workspace"], &root)
        .expect("workspace init should succeed");
    run_command_from_args_in_dir(["fol", "work", "init", "--bin"], &app)
        .expect("app init should succeed");
    run_command_from_args_in_dir(["fol", "work", "init", "--lib"], &lib)
        .expect("lib init should succeed");
    fs::write(root.join("fol.work.yaml"), "members:\n  - app\n  - lib\n")
        .expect("should write workspace members");

    let (_, info) = run_command_from_args_in_dir(["fol", "work", "info"], &root)
        .expect("work info should succeed");
    let (_, list) = run_command_from_args_in_dir(["fol", "work", "list"], &root)
        .expect("work list should succeed");
    let (_, deps) = run_command_from_args_in_dir(["fol", "work", "deps"], &root)
        .expect("work deps should succeed");
    let (_, status) = run_command_from_args_in_dir(["fol", "work", "status"], &root)
        .expect("work status should succeed");
    let (_, build) = run_command_from_args_in_dir(["fol", "code", "build"], &root)
        .expect("build should succeed");

    assert!(info.summary.contains(&format!("root={}", root.display())));
    assert!(info.summary.contains("members=2"));
    assert!(list.summary.contains(&app.display().to_string()));
    assert!(list.summary.contains(&lib.display().to_string()));
    assert!(deps.summary.contains("app:"));
    assert!(deps.summary.contains("(no dependencies)"));
    assert!(status.summary.contains("lockfile=missing"));
    assert!(status
        .summary
        .contains(&root.join(".fol/cache/git").display().to_string()));
    assert!(build
        .summary
        .contains(&root.join(".fol/build/debug").display().to_string()));
    assert_eq!(
        build
            .artifacts
            .iter()
            .filter(|artifact| artifact.kind == FrontendArtifactKind::Binary)
            .count(),
        1
    );

    fs::remove_dir_all(root).ok();
}

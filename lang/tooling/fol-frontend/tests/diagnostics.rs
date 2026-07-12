use fol_frontend::{
    run_command_from_args_in_dir, FrontendOutput, FrontendOutputConfig, OutputMode,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn semantic_lib_build(name: &str) -> String {
    format!(
        concat!(
            "pro[] build(): non = {{\n",
            "    var build = .build();\n",
            "    build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n",
            "    var graph = build.graph();\n",
            "    var lib = graph.add_static_lib({{ name = \"{name}\", root = \"src/lib.fol\" }});\n",
            "    graph.install(lib);\n",
            "}};\n",
        ),
        name = name
    )
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fol_frontend_diagnostics_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
}

#[test]
fn frontend_workspace_discovery_failures_render_consistently_across_output_modes() {
    let root = temp_root("missing_root");
    fs::create_dir_all(&root).expect("should create empty root");

    let error = run_command_from_args_in_dir(["fol", "work", "info"], &root).unwrap_err();

    let human = FrontendOutput::new(FrontendOutputConfig::default())
        .render_error(&error)
        .expect("human render should succeed");
    let plain = FrontendOutput::new(FrontendOutputConfig {
        mode: OutputMode::Plain,
        ..FrontendOutputConfig::default()
    })
    .render_error(&error)
    .expect("plain render should succeed");
    let json = FrontendOutput::new(FrontendOutputConfig {
        mode: OutputMode::Json,
        ..FrontendOutputConfig::default()
    })
    .render_error(&error)
    .expect("json render should succeed");

    assert!(human.contains("F1002"));
    assert!(human.contains("fol work init --bin"));
    assert!(plain.contains("note: run `fol work init --workspace`"));
    assert!(json.contains("\"code\": \"F1002\""));
    assert!(json.contains("\"notes\": ["));

    fs::remove_dir_all(root).ok();
}

#[test]
fn frontend_parse_failures_keep_structured_help_notes() {
    let error = run_command_from_args_in_dir(["fol", "code", "emit", "wat"], std::env::temp_dir())
        .unwrap_err();
    let json = FrontendOutput::new(FrontendOutputConfig {
        mode: OutputMode::Json,
        ..FrontendOutputConfig::default()
    })
    .render_error(&error)
    .expect("json render should succeed");

    assert!(error.message().contains("unknown emit subcommand"));
    assert!(json.contains("fol --help"));
}

#[test]
fn locked_fetch_mismatch_failures_render_consistently_across_output_modes() {
    let root = temp_root("locked_mismatch");
    let app = root.join("app");
    let remote_a = root.join("remote-a");
    let remote_b = root.join("remote-b");
    create_git_package_repo(&remote_a, "logtiny", "0.1.0");
    create_git_package_repo(&remote_b, "logtiny", "0.1.1");
    create_app_with_git_dep(&app, &remote_a);

    run_command_from_args_in_dir(["fol", "pack", "fetch"], &app)
        .expect("initial fetch should succeed");
    fs::write(
        app.join("build.fol"),
        format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"app\", version = \"0.1.0\" }});\n",
                "    build.add_dep({{ alias = \"logtiny\", source = \"git\", target = \"git+file://{}\" }});\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({{ name = \"app\", root = \"src/main.fol\" }});\n",
                "    graph.install(app);\n",
                "    graph.add_run(app);\n",
                "}};\n",
            ),
            remote_b.display()
        ),
    )
    .expect("should rewrite manifest");

    let error = run_command_from_args_in_dir(["fol", "pack", "fetch", "--locked"], &app)
        .expect_err("locked fetch should fail when the manifest changes");

    let human = FrontendOutput::new(FrontendOutputConfig::default())
        .render_error(&error)
        .expect("human render should succeed");
    let plain = FrontendOutput::new(FrontendOutputConfig {
        mode: OutputMode::Plain,
        ..FrontendOutputConfig::default()
    })
    .render_error(&error)
    .expect("plain render should succeed");
    let json = FrontendOutput::new(FrontendOutputConfig {
        mode: OutputMode::Json,
        ..FrontendOutputConfig::default()
    })
    .render_error(&error)
    .expect("json render should succeed");

    assert!(human.contains("fol.lock"));
    assert!(human.contains("build.fol"));
    assert!(human.contains(
        "use `fol pack fetch --locked` only when build.fol and fol.lock are intentionally in sync"
    ));
    assert!(plain.contains("note: run `fol pack fetch` or `fol pack update` to refresh fol.lock"));
    assert!(json.contains("\"code\": \"F1001\""));
    assert!(json.contains("\"notes\": ["));

    fs::remove_dir_all(root).ok();
}

fn create_app_with_git_dep(app: &std::path::Path, remote: &std::path::Path) {
    fs::create_dir_all(app.join("src")).expect("should create app package");
    fs::write(
        app.join("build.fol"),
        format!(
            concat!(
                "pro[] build(): non = {{\n",
                "    var build = .build();\n",
                "    build.meta({{ name = \"app\", version = \"0.1.0\" }});\n",
                "    build.add_dep({{ alias = \"logtiny\", source = \"git\", target = \"git+file://{}\" }});\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({{ name = \"app\", root = \"src/main.fol\" }});\n",
                "    graph.install(app);\n",
                "    graph.add_run(app);\n",
                "}};\n",
            ),
            remote.display()
        ),
    )
    .expect("should write app manifest");
    fs::write(
        app.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .expect("should write app source");
}

fn create_git_package_repo(root: &std::path::Path, name: &str, version: &str) {
    fs::create_dir_all(root.join("src")).expect("package repo should be creatable");
    fs::write(root.join("build.fol"), semantic_lib_build(name).replace("0.1.0", version))
        .expect("package build should be writable");
    fs::write(root.join("src/lib.fol"), "var[exp] level: int = 1;\n")
        .expect("package source should be writable");
    git(root, &["init"]);
    git(root, &["config", "user.name", "FOL"]);
    git(root, &["config", "user.email", "fol@example.com"]);
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "init"]);
}

fn git(root: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .expect("git command should run");
    assert!(status.success(), "git {:?} should succeed", args);
}

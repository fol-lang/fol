use fol_frontend::{run_command_from_args_in_dir, FrontendArtifactKind};
use std::fs;
use std::path::PathBuf;

fn semantic_bin_build() -> &'static str {
    concat!(
        "pro[] build(): non = {\n",
        "    var build = .build();\n",
        "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
        "    var graph = build.graph();\n",
        "    var app = graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
        "    graph.install(app);\n",
        "    graph.add_run(app);\n",
        "};\n",
    )
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fol_frontend_artifacts_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
}

#[test]
fn build_and_emit_commands_report_explicit_root_artifacts() {
    let root = temp_root("build_emit");
    fs::create_dir_all(root.join("src")).expect("should create source root");
    fs::write(root.join("build.fol"), semantic_bin_build()).expect("should write build");
    fs::write(
        root.join("src/main.fol"),
        "fun[] main(): int = {\n    return 0\n};\n",
    )
    .expect("should write source");

    let (_, build) =
        run_command_from_args_in_dir(["fol", "code", "build"], &root).expect("build should pass");
    let (_, emit) = run_command_from_args_in_dir(["fol", "code", "emit", "rust"], &root)
        .expect("emit rust should pass");

    assert!(build
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == FrontendArtifactKind::BuildRoot));
    assert!(emit
        .artifacts
        .iter()
        .any(|artifact| artifact.kind == FrontendArtifactKind::BuildRoot));
    assert!(build.summary.contains(".fol/build/debug"));
    assert!(emit.summary.contains(".fol/build/emit/rust"));

    fs::remove_dir_all(root).ok();
}

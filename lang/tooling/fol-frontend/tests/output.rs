use fol_frontend::{
    run_command_from_args_in_dir, FrontendOutput, FrontendOutputConfig, OutputMode,
};
use std::fs;

fn temp_root(label: &str) -> fol_testkit::TempFixture {
    fol_testkit::TempFixture::new(&format!("fol_frontend_output_{label}"))
}

#[test]
fn plain_mode_command_summaries_stay_script_friendly() {
    let root = temp_root("plain");
    fs::create_dir_all(&root).expect("should create output root");

    run_command_from_args_in_dir(["fol", "work", "init", "--bin"], &root)
        .expect("init should succeed");
    let (_, result) =
        run_command_from_args_in_dir(["fol", "code", "build", "--keep-build-dir"], &root)
            .expect("build should succeed");
    let rendered = FrontendOutput::new(FrontendOutputConfig {
        mode: OutputMode::Plain,
    })
    .render_command_summary(&result)
    .expect("plain render should succeed");

    assert!(rendered.contains("command: build"));
    assert!(rendered.contains("summary: built 1 workspace package(s) into"));
    assert!(rendered.contains("emitted-rust:"));
    assert!(rendered.contains("binary:"));
    assert!(!rendered.contains('\u{1b}'));

    fs::remove_dir_all(root).ok();
}

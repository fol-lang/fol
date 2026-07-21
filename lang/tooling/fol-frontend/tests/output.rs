use fol_frontend::{
    run_command_from_args_in_dir, FrontendOutput, FrontendOutputConfig, OutputMode,
};
use std::fs;
use std::path::PathBuf;

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fol_frontend_output_{}_{}_{}",
        label,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ))
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
        ..FrontendOutputConfig::default()
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

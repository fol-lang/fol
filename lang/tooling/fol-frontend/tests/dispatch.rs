use fol_frontend::run_from_args_with_io;

#[test]
fn frontend_run_from_args_writes_rendered_command_summaries_without_root_duplication() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_from_args_with_io(["fol", "_complete", "code", "bu"], &mut stdout, &mut stderr);

    assert_eq!(code, 0);
    assert!(String::from_utf8(stdout).unwrap().contains("build"));
    assert!(stderr.is_empty());
}

#[test]
fn frontend_run_from_args_writes_rendered_errors_without_root_duplication() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_from_args_with_io(["fol", "code", "emit", "wat"], &mut stdout, &mut stderr);

    assert_eq!(code, 1);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr).unwrap().contains("fol --help"));
}

#[test]
fn frontend_run_from_args_prints_help_when_requested_or_when_no_command_is_given() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let help_code = run_from_args_with_io(["fol", "--help"], &mut stdout, &mut stderr);

    assert_eq!(help_code, 0);
    assert!(String::from_utf8(stdout.clone())
        .unwrap()
        .contains("Commands:"));
    assert!(stderr.is_empty());

    stdout.clear();
    let empty_code = run_from_args_with_io(["fol"], &mut stdout, &mut stderr);

    assert_eq!(empty_code, 0);
    assert!(String::from_utf8(stdout)
        .unwrap()
        .contains("Usage: fol"));
    assert!(stderr.is_empty());
}

#[test]
fn frontend_run_from_args_prints_version_without_error_wrapping() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let code = run_from_args_with_io(["fol", "--version"], &mut stdout, &mut stderr);

    assert_eq!(code, 0);
    assert!(String::from_utf8(stdout).unwrap().contains("fol"));
    assert!(stderr.is_empty());
}

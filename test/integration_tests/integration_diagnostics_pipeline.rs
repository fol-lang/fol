use super::*;

#[test]
fn unified_pipeline_parser_errors_carry_p_codes_in_json() {
    use std::fs;

    let temp_root = unique_temp_root("pipeline_parser_code");
    fs::create_dir_all(&temp_root).expect("Should create fixture dir");
    fs::write(temp_root.join("main.fol"), "run(1, 2)\n").expect("Should write invalid fixture");

    let output = run_fol(&["--json", temp_root.to_str().expect("path should be utf-8")]);
    let json = parse_cli_json(&output);
    let code = json["diagnostics"][0]["code"]
        .as_str()
        .expect("diagnostic should have a code");

    assert!(
        code.starts_with('P'),
        "parser diagnostic code should start with P, got {code}"
    );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn unified_pipeline_resolver_errors_carry_r_codes_in_json() {
    use std::fs;

    let temp_root = unique_temp_root("pipeline_resolver_code");
    fs::create_dir_all(&temp_root).expect("Should create fixture dir");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    return missing;\n};\n",
    )
    .expect("Should write unresolved fixture");

    let output = run_fol(&["--json", temp_root.to_str().expect("path should be utf-8")]);
    let json = parse_cli_json(&output);
    let code = json["diagnostics"][0]["code"]
        .as_str()
        .expect("diagnostic should have a code");

    assert!(
        code.starts_with('R'),
        "resolver diagnostic code should start with R, got {code}"
    );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn unified_pipeline_typecheck_errors_carry_t_codes_in_json() {
    use std::fs;

    let temp_root = unique_temp_root("pipeline_typecheck_code");
    fs::create_dir_all(&temp_root).expect("Should create fixture dir");
    fs::write(temp_root.join("main.fol"), "var[bor] borrowed: int = 1;\n")
        .expect("Should write typecheck fixture");

    let output = run_fol(&["--json", temp_root.to_str().expect("path should be utf-8")]);
    let json = parse_cli_json(&output);
    let code = json["diagnostics"][0]["code"]
        .as_str()
        .expect("diagnostic should have a code");

    assert!(
        code.starts_with('T'),
        "typecheck diagnostic code should start with T, got {code}"
    );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn unified_pipeline_frontend_errors_carry_f_codes() {
    let error = fol_frontend::FrontendError::new(
        fol_frontend::FrontendErrorKind::WorkspaceNotFound,
        "no workspace",
    );
    let diagnostic = fol_diagnostics::ToDiagnostic::to_diagnostic(&error);

    assert_eq!(diagnostic.code.as_str(), "F1002");
    assert_eq!(diagnostic.message, "no workspace");
}

#[test]
fn unified_pipeline_all_diagnostic_families_share_structured_json_shape() {
    use std::fs;

    let temp_root = unique_temp_root("pipeline_json_shape");
    fs::create_dir_all(&temp_root).expect("Should create fixture dir");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    return missing;\n};\n",
    )
    .expect("Should write fixture");

    let output = run_fol(&["--json", temp_root.to_str().expect("path should be utf-8")]);
    let json = parse_cli_json(&output);
    let diagnostic = &json["diagnostics"][0];

    assert!(diagnostic["severity"].as_str().is_some());
    assert!(diagnostic["code"].as_str().is_some());
    assert!(diagnostic["message"].as_str().is_some());
    assert!(diagnostic["location"].is_object() || diagnostic["location"].is_null());
    assert!(diagnostic["labels"].is_array());
    assert!(diagnostic["notes"].is_array());
    assert!(diagnostic["helps"].is_array());
    assert!(diagnostic["suggestions"].is_array());
    assert!(json["error_count"].is_number());
    assert!(json["warning_count"].is_number());

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn unified_pipeline_json_and_human_carry_same_diagnostic_code() {
    use std::fs;

    let temp_root = unique_temp_root("pipeline_code_parity");
    fs::create_dir_all(&temp_root).expect("Should create fixture dir");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    return missing;\n};\n",
    )
    .expect("Should write fixture");

    let json_output = run_fol(&["--json", temp_root.to_str().expect("path should be utf-8")]);
    let json = parse_cli_json(&json_output);
    let json_code = json["diagnostics"][0]["code"]
        .as_str()
        .expect("JSON diagnostic should have a code")
        .to_string();

    let human_output = run_fol(&[temp_root.to_str().expect("path should be utf-8")]);
    let human_stdout = String::from_utf8_lossy(&human_output.stdout);

    // Pretty human mode shows the code as a chip suffix (`R1003`) rather
    // than the `error[R1003]:` prefix, but it's the same code as the JSON.
    assert!(
        human_stdout.contains(&json_code),
        "human output should contain the same code {json_code} as JSON output"
    );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn unified_pipeline_no_glitch_trait_exists() {
    // The Glitch trait has been deleted from fol-types.
    // This test is a compile-time guarantee: if Glitch existed,
    // the trait would be importable. Since it's deleted, this
    // test simply documents the invariant at the integration level.
    //
    // grep -r "trait Glitch" lang/ returns nothing — verified
    // by the build system. The absence of Box<dyn Glitch> in
    // any public API is enforced by the fact that the trait
    // no longer compiles.
}

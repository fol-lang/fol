use super::*;

    fn copied_example_root(example_path: &str) -> std::path::PathBuf {
        let source = repo_root().join(example_path);
        let temp_root = unique_temp_root(&format!(
            "cli_example_{}",
            example_path.replace('/', "_")
        ));
        let target = temp_root.join("workspace");
        copy_dir_all(&source, &target);
        std::fs::remove_dir_all(target.join(".fol")).ok();
        target
    }

    #[test]
    fn test_cli_single_file_compile_succeeds_with_builtin_str_types() {
        use std::fs;

        let temp_root = unique_temp_root("cli_builtin_str_compile");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI builtin-str fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            "ali Text: str;\ntyp User: rec = {\n    var name: str;\n};\nfun[] main(path: str): Text = {\n    var local: str = path;\n    return local;\n};\n",
        )
        .expect("Should write builtin str CLI fixture");

        let output = run_fol(&[fixture
            .to_str()
            .expect("CLI builtin str fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should accept builtin str across alias, type, and routine surfaces, got status {:?} and output:\n{}",
            output.status.code(),
            stdout
        );
        assert!(
            !stdout.contains("could not resolve type 'str'"),
            "CLI output should no longer report resolver failures for builtin str"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_lowering_arithmetic_now_succeeds() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_arithmetic");
        fs::create_dir_all(&temp_root).expect("Should create temp lowering arithmetic fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(&fixture, "fun[] main(): int = {\n    return 1 + 2;\n};\n")
            .expect("Should write arithmetic fixture");

        let output = run_fol(&[fixture
            .to_str()
            .expect("CLI arithmetic fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should compile arithmetic binary operators, got status {:?} and output:\n{}",
            output.status.code(),
            stdout
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_lowering_repro_program_now_succeeds_end_to_end() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_repro_boundary");
        fs::create_dir_all(&temp_root).expect("Should create temp lowering repro fixture dir");
        let fixture = write_combined_lowering_repro_fixture(&temp_root);

        let output = run_fol(&[fixture
            .to_str()
            .expect("CLI lowering repro fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "CLI should compile the combined lowering repro now, got status {:?} and output:;\n{};\n{};",
            output.status.code(),
            stdout,
            stderr,
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Combined repro should compile cleanly through lowering, got stdout:;\n{};\n\nstderr:;\n{};",
            stdout,
            stderr,
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_combined_repro_stays_stable_and_inspectable() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_repro_dump");
        fs::create_dir_all(&temp_root).expect("Should create temp lowering dump fixture dir");
        write_combined_lowering_repro_fixture(&temp_root);

        let output = run_fol(&[
            "--dump-lowered",
            temp_root
                .to_str()
                .expect("CLI lowering dump fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for the repaired combined repro, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(stdout.contains("workspace entry="));
        assert!(stdout.contains("package"));
        assert!(stdout.contains("type-decl User"));
        assert!(stdout.contains("routine r1 build_user"));
        assert!(stdout.contains("routine r3 main"));
        assert!(stdout.contains("params [l0]"));
        assert!(stdout.contains("ConstructLinear { kind: Sequence"));
        assert!(stdout.contains("ConstructMap"));
        assert!(stdout.contains("FieldAccess { base:"));
        assert!(stdout.contains("IndexAccess { container:"));
        assert!(stdout.contains("entry-candidates"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_lowering_parameter_scope_regression_now_succeeds() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_parameter_scope");
        fs::create_dir_all(&temp_root)
            .expect("Should create temp parameter regression fixture dir");
        let fixture = write_parameter_scope_lowering_fixture(&temp_root);

        let output = run_fol(&[fixture
            .to_str()
            .expect("CLI parameter regression fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "CLI should compile the repaired parameter-scope lowering repro, got status {:?} and output:\n{}\n{}",
            output.status.code(),
            stdout,
            stderr,
        );
        assert!(stdout.contains("Compilation successful"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_lowering_container_regression_now_succeeds() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_container_regression");
        fs::create_dir_all(&temp_root)
            .expect("Should create temp container regression fixture dir");
        let fixture = write_container_lowering_fixture(&temp_root);

        let output = run_fol(&[fixture
            .to_str()
            .expect("CLI container regression fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "CLI should compile the repaired container lowering repro, got status {:?} and output:\n{}\n{}",
            output.status.code(),
            stdout,
            stderr,
        );
        assert!(stdout.contains("Compilation successful"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_lowering_early_return_when_regression_now_succeeds() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_early_return_when");
        fs::create_dir_all(&temp_root)
            .expect("Should create temp early-return regression fixture dir");
        let fixture = write_early_return_when_fixture(&temp_root);

        let output = run_fol(&[fixture
            .to_str()
            .expect("CLI early-return regression fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "CLI should compile the repaired early-return when repro, got status {:?} and output:\n{}\n{}",
            output.status.code(),
            stdout,
            stderr,
        );
        assert!(stdout.contains("Compilation successful"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_lowering_condition_loops_now_succeed() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_condition_loop_boundary");
        fs::create_dir_all(&temp_root).expect("Should create temp loop lowering fixture dir");
        fs::write(
            temp_root.join("main.fol"),
            concat!(
                "fun[] main(): int = {\n",
                "    var total: int = 0;\n",
                "    loop(total < 3) {\n",
                "        total = total + 1;\n",
                "    }\n",
                "    return total;\n",
                "};\n",
            ),
        )
        .expect("Should write loop lowering fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("loop lowering fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "CLI should lower condition loops without hitting the old boundary, got status {:?} and output:\n{}\n{}",
            output.status.code(),
            stdout,
            stderr,
        );
        assert!(stdout.contains("Compilation successful"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_lowering_nested_blocks_now_succeed() {
        use std::fs;

        let temp_root = unique_temp_root("cli_lowering_block_boundary");
        fs::create_dir_all(&temp_root).expect("Should create temp block lowering fixture dir");
        fs::write(
            temp_root.join("main.fol"),
            concat!(
                "fun[] main(): int = {\n",
                "    var value: int = 1;\n",
                "    {\n",
                "        var inner: int = value + 1;\n",
                "        value = inner;\n",
                "    }\n",
                "    return value;\n",
                "};\n",
            ),
        )
        .expect("Should write block lowering fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("block lowering fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "CLI should lower nested blocks without hitting the old boundary, got status {:?} and output:\n{}\n{}",
            output.status.code(),
            stdout,
            stderr,
        );
        assert!(stdout.contains("Compilation successful"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_explicit_recoverable_handling_lowers_successfully_across_multiple_routines() {
        use std::fs;

        let temp_root = unique_temp_root("cli_error_propagation");
        fs::create_dir_all(&temp_root).expect("Should create temp error propagation fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] leaf(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { report \"bad\" }\n",
                "        * { return 7 }\n",
                "    }\n",
                "};\n",
                "fun[] mid(flag: bol): int / str = {\n",
                "    return leaf(flag) || report \"mid-bad\";\n",
                "};\n",
                "fun[] main(flag: bol): int / str = {\n",
                "    return mid(flag) || report \"main-bad\";\n",
                "};\n",
            ),
        )
        .expect("Should write error propagation fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("error propagation fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "explicit recoverable-handling fixture should compile, got:\n{stdout}"
        );
        assert!(stdout.contains("CheckRecoverable"));
        assert!(stdout.contains("UnwrapRecoverable"));
        assert!(stdout.contains("Report"));
        assert!(!stdout.contains("ExtractRecoverableError"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_check_handling_lowers_without_error_propagation() {
        use std::fs;

        let temp_root = unique_temp_root("cli_error_check");
        fs::create_dir_all(&temp_root).expect("Should create temp error check fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] load(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { report \"bad\" }\n",
                "        * { return 7 }\n",
                "    }\n",
                "};\n",
                "fun[] main(flag: bol): bol = {\n",
                "    return check(load(flag));\n",
                "};\n",
            ),
        )
        .expect("Should write error check fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("error check fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "error check fixture should compile, got:\n{stdout}"
        );
        assert!(stdout.contains("CheckRecoverable"));
        assert!(!stdout.contains("ExtractRecoverableError"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_rejects_generic_routines_with_explicit_m1_boundary() {
        use std::fs;

        let temp_root = unique_temp_root("cli_generic_routine_lowering_m1");
        fs::create_dir_all(&temp_root).expect("Should create temp generic routine fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun pick(T)(value: T): T = {\n",
                "    return value;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    return pick(7);\n",
                "};\n",
            ),
        )
        .expect("Should write generic routine lowering fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("generic routine lowering fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        assert!(
            !output.status.success(),
            "generic routine lowering should fail cleanly in M1, got:\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            combined.contains("generic routine lowering is not yet supported in V2 Milestone 1"),
            "CLI lowering should preserve the explicit M1 generic-lowering boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_rejects_multi_param_generic_routines_with_explicit_m1_boundary() {
        use std::fs;

        let temp_root = unique_temp_root("cli_generic_pair_lowering_m1");
        fs::create_dir_all(&temp_root).expect("Should create temp generic pair fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun pair(T)(left: T, right: T): T = {\n",
                "    return right;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    return pair(1, 2);\n",
                "};\n",
            ),
        )
        .expect("Should write generic pair lowering fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("generic pair lowering fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        assert!(!output.status.success());
        assert!(
            combined.contains("generic routine lowering is not yet supported in V2 Milestone 1"),
            "CLI lowering should preserve the explicit M1 generic-lowering boundary wording for generic pairs"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_rejects_generic_routines_with_default_params_with_explicit_m1_boundary() {
        use std::fs;

        let temp_root = unique_temp_root("cli_generic_defaults_lowering_m1");
        fs::create_dir_all(&temp_root).expect("Should create temp generic default fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun pick(T)(value: T, fallback: int = 1): T = {\n",
                "    return value;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    return pick(7);\n",
                "};\n",
            ),
        )
        .expect("Should write generic default lowering fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("generic default lowering fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        assert!(!output.status.success());
        assert!(
            combined.contains("generic routine lowering is not yet supported in V2 Milestone 1"),
            "CLI lowering should preserve the explicit M1 generic-lowering boundary wording for generic defaults"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_rejects_protocol_standards_with_explicit_m2_boundary() {
        use std::fs;

        let temp_root = unique_temp_root("cli_standards_protocol_lowering_m2");
        fs::create_dir_all(&temp_root).expect("Should create temp standards lowering fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "std geo: pro = {\n",
                "    fun area(): int;\n",
                "};\n",
                "typ Rect()(geo): rec = {\n",
                "    var width: int;\n",
                "};\n",
                "fun (Rect)area(): int = {\n",
                "    return 1;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    return 0;\n",
                "};\n",
            ),
        )
        .expect("Should write standards lowering fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("standards lowering fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        assert!(
            !output.status.success(),
            "standards lowering should fail cleanly in M2, got:\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        assert!(
            combined.contains("protocol standard lowering is not yet supported in V2 Milestone 2"),
            "CLI lowering should preserve the explicit M2 standards-lowering boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_rejects_multi_routine_protocol_standards_with_explicit_m2_boundary() {
        use std::fs;

        let temp_root = unique_temp_root("cli_standards_protocol_pair_lowering_m2");
        fs::create_dir_all(&temp_root).expect("Should create temp standards lowering fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "std geo: pro = {\n",
                "    fun area(): int;\n",
                "    fun perimeter(): int;\n",
                "};\n",
                "typ Rect()(geo): rec = {\n",
                "    var width: int;\n",
                "};\n",
                "fun (Rect)area(): int = {\n",
                "    return 1;\n",
                "};\n",
                "fun (Rect)perimeter(): int = {\n",
                "    return 4;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    return 0;\n",
                "};\n",
            ),
        )
        .expect("Should write multi-routine standards lowering fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("standards lowering fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        assert!(!output.status.success());
        assert!(
            combined.contains("protocol standard lowering is not yet supported in V2 Milestone 2"),
            "multi-routine standards lowering should preserve the explicit M2 boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_rejects_cross_file_protocol_standards_with_explicit_m2_boundary() {
        use std::fs;

        let temp_root = unique_temp_root("cli_standards_protocol_cross_file_lowering_m2");
        fs::create_dir_all(&temp_root).expect("Should create temp standards lowering fixture dir");
        fs::write(
            temp_root.join("00_std.fol"),
            "std geo: pro = { fun area(): int; };\n",
        )
        .expect("Should write cross-file standard fixture");
        fs::write(
            temp_root.join("10_main.fol"),
            concat!(
                "typ Rect()(geo): rec = {\n",
                "    var width: int;\n",
                "};\n",
                "fun (Rect)area(): int = {\n",
                "    return 1;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    return 0;\n",
                "};\n",
            ),
        )
        .expect("Should write cross-file main fixture");

        let output = run_fol(&[
            "--dump-lowered",
            temp_root
                .to_str()
                .expect("standards lowering fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        assert!(!output.status.success());
        assert!(
            combined.contains("protocol standard lowering is not yet supported in V2 Milestone 2"),
            "cross-file standards lowering should preserve the explicit M2 boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_full_chain_keeps_new_standards_examples_on_their_current_boundaries() {
        let cases = [
            (
                "examples/standards_protocol_multi_m2",
                vec!["code", "build"],
                "protocol standard lowering is not yet supported in V2 Milestone 2",
            ),
            (
                "examples/fail_standard_missing_routine_m2",
                vec!["code", "check"],
                "missing required routine 'perimeter'",
            ),
            (
                "examples/fail_standard_signature_m2",
                vec!["code", "check"],
                "routine 'area' has incompatible signature",
            ),
            (
                "examples/fail_standard_import_ambiguity_m2",
                vec!["code", "check"],
                "standard 'geo' is ambiguous in lexical scope",
            ),
        ];

        for (example, args, expected) in cases {
            let root = copied_example_root(example);
            let output = run_fol_in_dir(&root, &args);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{stdout}\n{stderr}");

            assert!(
                !output.status.success(),
                "example '{example}' should stay on its current failure boundary: stdout=\n{}\nstderr=\n{}",
                stdout,
                stderr
            );
            assert!(
                combined.contains(expected),
                "example '{example}' should keep boundary '{expected}': stdout=\n{}\nstderr=\n{}",
                stdout,
                stderr
            );

            std::fs::remove_dir_all(root).ok();
        }
    }

    #[test]
    fn test_cli_full_chain_rejects_the_generic_plus_standards_seam_cleanly() {
        let root = copied_example_root("examples/fail_generic_standard_constraint_m1m2");
        let output = run_fol_in_dir(&root, &["code", "build"]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");

        assert!(
            !output.status.success(),
            "generic-plus-standards seam example should fail cleanly: stdout=\n{}\nstderr=\n{}",
            stdout,
            stderr
        );
        assert!(
            combined.contains("generic routine constraints are not yet supported in V2 Milestone 1"),
            "generic-plus-standards seam should keep the explicit M1 constraint boundary: stdout=\n{}\nstderr=\n{}",
            stdout,
            stderr
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn test_cli_pipe_or_default_lowers_successfully() {
        use std::fs;

        let temp_root = unique_temp_root("cli_error_pipe_or_default");
        fs::create_dir_all(&temp_root).expect("Should create temp pipe-or default fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] load(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { report \"bad\" }\n",
                "        * { return 7 }\n",
                "    }\n",
                "};\n",
                "fun[] main(flag: bol): int = {\n",
                "    return load(flag) || 5;\n",
                "};\n",
            ),
        )
        .expect("Should write pipe-or default fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("pipe-or default fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "pipe-or default fixture should compile, got:\n{stdout}"
        );
        assert!(stdout.contains("CheckRecoverable"));
        assert!(stdout.contains("UnwrapRecoverable"));
        assert!(stdout.contains("Const(Int(5))"));
        assert!(!stdout.contains("ExtractRecoverableError"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_pipe_or_report_lowers_successfully() {
        use std::fs;

        let temp_root = unique_temp_root("cli_error_pipe_or_report");
        fs::create_dir_all(&temp_root).expect("Should create temp pipe-or report fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] load(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { report \"bad\" }\n",
                "        * { return 7 }\n",
                "    }\n",
                "};\n",
                "fun[] main(flag: bol): int / str = {\n",
                "    return load(flag) || report \"fallback\";\n",
                "};\n",
            ),
        )
        .expect("Should write pipe-or report fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("pipe-or report fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "pipe-or report fixture should compile, got:\n{stdout}"
        );
        assert!(stdout.contains("CheckRecoverable"));
        assert!(stdout.contains("Report"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_pipe_or_panic_lowers_successfully() {
        use std::fs;

        let temp_root = unique_temp_root("cli_error_pipe_or_panic");
        fs::create_dir_all(&temp_root).expect("Should create temp pipe-or panic fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] load(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { report \"bad\" }\n",
                "        * { return 7 }\n",
                "    }\n",
                "};\n",
                "fun[] main(flag: bol): int = {\n",
                "    return load(flag) || panic \"fallback\";\n",
                "};\n",
            ),
        )
        .expect("Should write pipe-or panic fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("pipe-or panic fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "pipe-or panic fixture should compile, got:\n{stdout}"
        );
        assert!(stdout.contains("CheckRecoverable"));
        assert!(stdout.contains("Panic"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_recoverable_abi_stays_stable_across_workspace_call_paths() {
        use std::fs;

        let temp_root = unique_temp_root("cli_error_abi_workspace");
        let app_root = temp_root.join("app");
        let shared_root = temp_root.join("shared");
        fs::create_dir_all(&app_root).expect("Should create app root");
        fs::create_dir_all(&shared_root).expect("Should create shared root");
        fs::write(
            shared_root.join("lib.fol"),
            concat!(
                "fun[exp] remote(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { report \"shared-bad\" }\n",
                "        * { return 7 }\n",
                "    }\n",
                "};\n",
            ),
        )
        .expect("Should write shared recoverable fixture");
        fs::write(
            app_root.join("00_leaf.fol"),
            concat!(
                "fun[] leaf(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { report \"leaf-bad\" }\n",
                "        * { return 5 }\n",
                "    }\n",
                "};\n",
            ),
        )
        .expect("Should write local recoverable fixture");
        fs::write(
            app_root.join("05_mid.fol"),
            concat!(
                "use shared: loc = {\"../shared\"};\n",
                "fun[] mid(flag: bol): int / str = {\n",
                "    loop(flag) {\n",
                "        break;\n",
                "    }\n",
                "    when(flag) {\n",
                "        case(true) { return remote(flag) || report \"mid-shared-bad\" }\n",
                "        * { return leaf(flag) || report \"mid-leaf-bad\" }\n",
                "    }\n",
                "};\n",
            ),
        )
        .expect("Should write middle recoverable fixture");
        fs::write(
            app_root.join("10_main.fol"),
            concat!(
                "fun[] main(flag: bol): int / str = {\n",
                "    when(flag) {\n",
                "        case(true) { return mid(flag) || report \"main-mid-bad\" }\n",
                "        * { return leaf(flag) || report \"main-leaf-bad\" }\n",
                "    }\n",
                "};\n",
            ),
        )
        .expect("Should write entry recoverable fixture");

        let output = run_fol(&[
            "--dump-lowered",
            app_root
                .to_str()
                .expect("app root should be valid utf-8 for dump-lowered"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "recoverable ABI workspace fixture should compile, got:\n{stdout}"
        );
        assert!(stdout.contains("recoverable-abi kind=tagged-result-object"));
        assert!(stdout.contains("package app"));
        assert!(stdout.contains("package shared"));
        assert!(stdout.contains("CheckRecoverable"));
        assert!(stdout.contains("Report"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_arithmetic_compiles_with_structured_output() {
        use std::fs;

        let temp_root = unique_temp_root("cli_arithmetic_json");
        fs::create_dir_all(&temp_root).expect("Should create temp arithmetic JSON fixture dir");
        let fixture = temp_root.join("main.fol");
        fs::write(&fixture, "fun[] main(): int = {\n    return 1 + 2;\n};\n")
            .expect("Should write arithmetic fixture");

        let output = run_fol(&[
            "--json",
            fixture
                .to_str()
                .expect("CLI arithmetic fixture path should be utf-8"),
        ]);
        let payload = parse_cli_json(&output);

        assert!(
            output.status.success(),
            "CLI JSON arithmetic should compile successfully"
        );
        assert_eq!(payload["error_count"], 0);

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_folder_parse_errors_keep_json_locations_with_package_parser() {
        use std::fs;

        let temp_root = unique_temp_root("cli_folder_parse_error");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI error fixture");
        fs::write(temp_root.join("00_good.fol"), "var ok = 1;\n").expect("Should write good source");
        fs::write(temp_root.join("10_bad.fol"), "run(1, 2);\n")
            .expect("Should write invalid file-root source");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI error fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let compact = stdout
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();

        assert!(
            !output.status.success(),
            "CLI should fail on declaration-only package parse errors"
        );
        assert!(
            stdout.contains("10_bad.fol"),
            "JSON diagnostics should identify the failing second source unit"
        );
        assert!(
            compact.contains("\"line\":1"),
            "JSON diagnostics should preserve the failing line number"
        );
        assert!(
            compact.contains("\"column\":1"),
            "JSON diagnostics should preserve the failing column number"
        );
        assert!(
            stdout.contains("Executable calls are not allowed at file root"),
            "JSON diagnostics should keep the parser's file-root error wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_folder_resolver_errors_fail_parse_clean_programs() {
        use std::fs;

        let temp_root = unique_temp_root("cli_folder_resolver_error");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI resolver fixture");
        fs::write(temp_root.join("00_first.fol"), "var value = 1;\n")
            .expect("Should write first declaration source");
        fs::write(temp_root.join("10_second.fol"), "var value = 2;\n")
            .expect("Should write duplicate declaration source");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI resolver fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should fail when resolver rejects a parse-clean folder"
        );
        assert!(
            stdout.contains("duplicate symbol 'value'"),
            "CLI diagnostics should surface resolver duplicate-symbol messages"
        );
        assert!(
            stdout.contains("10_second.fol"),
            "CLI diagnostics should identify the duplicate source file"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_folder_resolver_errors_keep_json_locations() {
        use std::fs;

        let temp_root = unique_temp_root("cli_folder_resolver_error_json");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI resolver fixture");
        fs::write(temp_root.join("00_first.fol"), "var value = 1;\n")
            .expect("Should write first declaration source");
        fs::write(temp_root.join("10_second.fol"), "var value = 2;\n")
            .expect("Should write duplicate declaration source");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI resolver fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let compact = stdout
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();

        assert!(
            !output.status.success(),
            "CLI should fail in JSON mode when resolver rejects a parse-clean folder"
        );
        assert!(
            stdout.contains("10_second.fol"),
            "JSON resolver diagnostics should identify the duplicate source file"
        );
        assert!(
            compact.contains("\"line\":1"),
            "JSON resolver diagnostics should preserve the duplicate declaration line number"
        );
        assert!(
            compact.contains("\"column\":1"),
            "JSON resolver diagnostics should preserve the duplicate declaration column number"
        );
        assert!(
            stdout.contains("duplicate symbol 'value'"),
            "JSON resolver diagnostics should keep resolver duplicate-symbol wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_resolver_errors_keep_exact_json_locations_for_qualified_paths() {
        use std::fs;

        let temp_root = unique_temp_root("cli_resolver_qualified_location");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI resolver fixture");
        let main_file = temp_root.join("main.fol");
        fs::write(&main_file, "ali Broken: tools::Missing;\n")
            .expect("Should write unresolved qualified type fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI resolver fixture path should be utf-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let compact = stdout
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>();

        assert!(
            !output.status.success(),
            "CLI should fail in JSON mode when resolver rejects an unresolved qualified path"
        );
        assert!(
            stdout.contains(
                main_file
                    .to_str()
                    .expect("Temporary resolver fixture path should be valid UTF-8")
            ),
            "JSON resolver diagnostics should keep the exact source file for qualified paths"
        );
        assert!(
            compact.contains("\"line\":1"),
            "JSON resolver diagnostics should preserve the exact failing line number"
        );
        assert!(
            compact.contains("\"column\":13"),
            "JSON resolver diagnostics should preserve the exact qualified-path column"
        );
        assert!(
            stdout.contains("could not resolve qualified type 'tools::Missing'"),
            "JSON resolver diagnostics should keep the exact qualified-type wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

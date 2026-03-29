use super::*;

    #[test]
    fn test_cli_rejects_old_style_build_and_ordinary_dot_graph() {
        use std::fs;

        let temp_root = unique_temp_root("cli_build_graph_surface");
        fs::create_dir_all(temp_root.join("src")).expect("should create source root");
        fs::write(
            temp_root.join("src/main.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .expect("should write app source");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
                "};\n",
            ),
        )
        .expect("should write modern build file");

        let success = run_fol(&[temp_root
            .to_str()
            .expect("package path should be utf-8")]);
        assert!(
            success.status.success(),
            "modern ambient build.graph() build should succeed: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&success.stdout),
            String::from_utf8_lossy(&success.stderr)
        );

        fs::write(
            temp_root.join("build.fol"),
            "pro[] build(graph: Graph): non = {\n    return;\n};\n",
        )
        .expect("should rewrite old-style build file");
        let old_style = run_fol(&[temp_root
            .to_str()
            .expect("package path should be utf-8")]);
        let old_stdout = String::from_utf8_lossy(&old_style.stdout);
        assert!(!old_style.status.success(), "old-style build should fail");
        assert!(
            old_stdout.contains("pro[] build(): non")
                || old_stdout.contains(".graph()")
                || old_stdout.contains("canonical build entry")
                || old_stdout.contains("must not declare parameters")
                || old_stdout.contains("could not resolve type 'Graph'"),
            "old-style build failure should point at the ambient build contract"
        );

        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    graph.add_exe({ name = \"demo\", root = \"src/main.fol\" });\n",
                "};\n",
            ),
        )
        .expect("should restore modern build file");
        fs::write(
            temp_root.join("src/main.fol"),
            "fun[] main(): int = {\n    .graph();\n    return 0;\n};\n",
        )
        .expect("should write ordinary-source .graph misuse");

        let ordinary = run_fol(&[temp_root
            .to_str()
            .expect("package path should be utf-8")]);
        let ordinary_stdout = String::from_utf8_lossy(&ordinary.stdout);
        assert!(!ordinary.status.success(), "ordinary .graph() should fail");
        assert!(
            ordinary_stdout.contains(".graph"),
            "ordinary source failure should mention .graph() misuse"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_imported_symbol_mismatches_fail_full_chain() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_imported_symbol_error");
        let shared_root = temp_root.join("shared");
        let app_root = temp_root.join("app");
        fs::create_dir_all(&shared_root).expect("Should create shared fixture root");
        fs::create_dir_all(&app_root).expect("Should create app fixture root");
        fs::write(shared_root.join("lib.fol"), "var[exp] answer: int = 42;\n")
            .expect("Should write imported binding fixture");
        fs::write(
            app_root.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nvar label: str = answer;\n",
        )
        .expect("Should write imported binding consumer fixture");

        let output = run_fol(&[app_root
            .to_str()
            .expect("CLI imported binding fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should fail when imported symbol typing mismatches in the entry package"
        );
        assert!(
            stdout.contains("initializer for 'label' expects"),
            "CLI should preserve the imported binding mismatch wording"
        );
        assert!(
            stdout.contains("main.fol"),
            "CLI should preserve the imported binding consumer path"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_imported_aggregate_mismatches_fail_full_chain() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_imported_aggregate_error");
        let shared_root = temp_root.join("shared");
        let app_root = temp_root.join("app");
        fs::create_dir_all(&shared_root).expect("Should create shared fixture root");
        fs::create_dir_all(&app_root).expect("Should create app fixture root");
        fs::write(
            shared_root.join("types.fol"),
            "typ[exp] Meta: rec = {\n    ok: bol\n};\n\
             typ[exp] User: rec = {\n    meta: Meta\n};\n",
        )
        .expect("Should write imported aggregate fixture");
        fs::write(
            app_root.join("main.fol"),
            "use shared: loc = {\"../shared\"};\n\
             fun[] main(): shared::User = {\n\
                 return { meta = { ok = 1 } };\n\
             };\n",
        )
        .expect("Should write imported aggregate consumer fixture");

        let output = run_fol(&[app_root
            .to_str()
            .expect("CLI imported aggregate fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should fail when imported aggregate typing mismatches in the entry package"
        );
        assert!(
            stdout.contains("record field 'ok' expects"),
            "CLI should preserve the imported aggregate mismatch wording"
        );
        assert!(
            stdout.contains("main.fol"),
            "CLI should preserve the imported aggregate consumer path"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_imported_optional_shell_mismatches_fail_full_chain() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_imported_shell_error");
        let shared_root = temp_root.join("shared");
        let app_root = temp_root.join("app");
        fs::create_dir_all(&shared_root).expect("Should create shared fixture root");
        fs::create_dir_all(&app_root).expect("Should create app fixture root");
        fs::write(
            shared_root.join("types.fol"),
            "typ[exp] MaybeText: opt[str];\n",
        )
        .expect("Should write imported shell fixture");
        fs::write(
            app_root.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nvar value: int = 1;\nvar label: shared::MaybeText = value;\n",
        )
        .expect("Should write imported shell consumer fixture");

        let output = run_fol(&[app_root
            .to_str()
            .expect("CLI imported shell fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should fail when imported optional shell typing mismatches in the entry package"
        );
        assert!(
            stdout.contains("initializer for 'label' expects"),
            "CLI should preserve the imported shell mismatch wording"
        );
        assert!(
            stdout.contains("MaybeText") || stdout.contains("opt[str]"),
            "CLI should preserve the imported shell type identity"
        );
        assert!(
            stdout.contains("main.fol"),
            "CLI should preserve the imported shell consumer path"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_typecheck_errors_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_error_json");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI typecheck JSON fixture");
        fs::write(temp_root.join("main.fol"), "var[bor] borrowed: int = 1;\n")
            .expect("Should write the unsupported typecheck fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI typecheck JSON fixture path should be utf-8"),
        ]);
        let report = parse_cli_json(&output);
        let diagnostics = report["diagnostics"]
            .as_array()
            .expect("CLI JSON diagnostics should stay array-shaped");
        let first = diagnostics
            .first()
            .expect("CLI JSON diagnostics should include one typecheck error");

        assert!(
            !output.status.success(),
            "CLI should fail in JSON mode when typechecking rejects a parse-clean program"
        );
        assert_eq!(first["code"], "T1002");
        assert_eq!(first["location"]["line"], 1);
        assert_eq!(first["location"]["column"], 1);
        assert!(
            first["location"]["file"]
                .as_str()
                .is_some_and(|file| file.ends_with("main.fol")),
            "CLI JSON diagnostics should preserve the failing source-unit path"
        );
        assert!(
            first["message"]
                .as_str()
                .is_some_and(|message| message.contains("borrowing binding semantics")),
            "CLI JSON diagnostics should preserve the typecheck failure message"
        );
        assert_eq!(first["labels"].as_array().map(|items| items.len()), Some(1));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_generic_types_for_m1() {
        use std::fs;

        let temp_root = unique_temp_root("cli_generic_type_m1_boundary");
        fs::create_dir_all(&temp_root).expect("Should create temp generic-type fixture root");
        fs::write(
            temp_root.join("main.fol"),
            "typ Box(T): rec = {\n    value: int\n};\n",
        )
        .expect("Should write generic type fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI generic-type fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(!output.status.success(), "CLI should reject generic types in M1");
        assert!(
            stdout.contains("generic types are not yet supported"),
            "CLI should preserve the generic-type boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_generic_routine_constraints_for_m1() {
        use std::fs;

        let temp_root = unique_temp_root("cli_generic_constraint_m1_boundary");
        fs::create_dir_all(&temp_root).expect("Should create temp generic-constraint fixture root");
        fs::write(
            temp_root.join("main.fol"),
            "typ Bound: rec = {\n};\n\
             fun pick(T: Bound)(value: T): T = {\n    return value;\n};\n",
        )
        .expect("Should write generic routine constraint fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI generic-constraint fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should reject generic routine constraints in M1"
        );
        assert!(
            stdout.contains("generic routine constraints are not yet supported in V2 Milestone 1"),
            "CLI should preserve the generic routine constraint boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_standard_signature_mismatches_for_m2() {
        use std::fs;

        let temp_root = unique_temp_root("cli_standard_signature_mismatch_m2");
        fs::create_dir_all(&temp_root).expect("Should create temp standards M2 fixture root");
        fs::write(
            temp_root.join("main.fol"),
            "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(scale: int): int = {\n\
             return scale;\n\
         };\n",
        )
        .expect("Should write standards M2 signature mismatch fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI standards M2 fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(!output.status.success(), "CLI should reject standard signature mismatches in M2");
        assert!(
            stdout.contains("routine 'area' has incompatible signature")
                && stdout.contains("expected fun area(): int")
                && stdout.contains("found fun area(int): int"),
            "CLI should surface the explicit standard signature mismatch wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_standards_as_ordinary_types_for_m2() {
        use std::fs;

        let temp_root = unique_temp_root("cli_standard_type_boundary_m2");
        fs::create_dir_all(&temp_root).expect("Should create temp standards type boundary root");
        fs::write(
            temp_root.join("main.fol"),
            "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         fun use(value: geo): int = {\n\
             return 1;\n\
         };\n",
        )
        .expect("Should write standards type boundary fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI standards type boundary path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(!output.status.success(), "CLI should reject standards as ordinary types in M2");
        assert!(
            stdout.contains("standard 'geo' cannot be used as an ordinary type in V2 Milestone 2"),
            "CLI should preserve the standards-as-type boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_unsupported_standard_kinds_for_m2() {
        use std::fs;

        let temp_root = unique_temp_root("cli_unsupported_standard_kinds_m2");
        fs::create_dir_all(&temp_root).expect("Should create temp unsupported standards root");
        fs::write(
            temp_root.join("main.fol"),
            "std shape: blu = {\n\
             var size: int;\n\
         };\n\
         std display: ext = {\n\
             fun draw(): int;\n\
         };\n",
        )
        .expect("Should write unsupported standards fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI unsupported standards path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(!output.status.success(), "CLI should reject unsupported standard kinds in M2");
        assert!(
            stdout.contains("blueprint standards are planned for a future release")
                && stdout.contains("extended standards are planned for a future release"),
            "CLI should preserve the explicit unsupported standard-kind wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_duplicate_protocol_members_for_m2() {
        use std::fs;

        let temp_root = unique_temp_root("cli_duplicate_protocol_members_m2");
        fs::create_dir_all(&temp_root).expect("Should create temp duplicate protocol root");
        fs::write(
            temp_root.join("main.fol"),
            "std geo: pro = {\n\
             fun area(): int;\n\
             fun area(): int;\n\
         };\n",
        )
        .expect("Should write duplicate protocol member fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI duplicate protocol path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(!output.status.success(), "CLI should reject duplicate protocol members in M2");
        assert!(
            stdout.contains("Duplicate standard member 'area#0'"),
            "CLI should preserve the duplicate protocol member parser wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_template_style_generic_calls_for_m1() {
        use std::fs;

        let temp_root = unique_temp_root("cli_generic_template_call_m1_boundary");
        fs::create_dir_all(&temp_root).expect("Should create temp generic-call fixture root");
        fs::write(
            temp_root.join("main.fol"),
            "fun pick(T)(value: T): T = {\n    return value;\n};\n\
             fun[] main(): int = {\n    return pick$(1);\n};\n",
        )
        .expect("Should write template-style generic call fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI generic-call fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should reject template-style generic calls in M1"
        );
        assert!(
            stdout.contains("template instantiation is not yet supported"),
            "CLI should preserve the explicit template-instantiation boundary wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_rejects_invalid_check_calls_full_chain() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_invalid_check");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI invalid check fixture");
        fs::write(
            temp_root.join("main.fol"),
            "fun[] main(): bol = {\n    return check(1);\n};\n",
        )
        .expect("Should write invalid check fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI invalid check fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should fail when check(...) is used on a plain value"
        );
        assert!(
            stdout.contains("check(...) requires a routine call result with '/ ErrorType' in V1"),
            "CLI diagnostics should preserve the invalid check wording"
        );
        assert!(
            stdout.contains("main.fol"),
            "CLI diagnostics should preserve the failing source-unit path"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_keyword_intrinsic_arity_failures_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_keyword_intrinsic_arity");
        fs::create_dir_all(&temp_root)
            .expect("Should create temp CLI keyword intrinsic arity fixture");
        fs::write(
            temp_root.join("main.fol"),
            "fun[] main(): bol = {\n    return check();\n};\n",
        )
        .expect("Should write invalid keyword intrinsic arity fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI keyword intrinsic arity fixture path should be utf-8"),
        ]);

        assert!(
            !output.status.success(),
            "CLI should fail for wrong-arity keyword intrinsic calls",
        );

        let json = parse_cli_json(&output);
        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("CLI JSON output should expose diagnostics");
        let diagnostic = diagnostics.iter().find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .map(|message| {
                    message.contains("check(...) expects exactly 1 argument(s) but got 0")
                })
                .unwrap_or(false)
        });

        assert!(
            diagnostic.is_some(),
            "Expected keyword intrinsic arity diagnostic in CLI JSON output, got: {json}"
        );
        assert!(
            diagnostic
                .and_then(|diagnostic| diagnostic["location"].as_object())
                .is_some(),
            "Expected keyword intrinsic arity diagnostic to keep a structured location, got: {json}"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_typecheck_pipe_or_fallback_mismatches_keep_exact_locations() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_pipe_or_json");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI pipe-or JSON fixture");
        fs::write(
            temp_root.join("main.fol"),
            "fun[] load(): int / str = {\n\
                 report \"bad\";\n\
                 return 1;\n\
             };\n\
             fun[] main(): int = {\n\
                 return load() || \"fallback\";\n\
             };\n",
        )
        .expect("Should write pipe-or JSON fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI pipe-or JSON fixture path should be utf-8"),
        ]);
        let report = parse_cli_json(&output);
        let diagnostics = report["diagnostics"]
            .as_array()
            .expect("CLI JSON diagnostics should stay array-shaped");
        let first = diagnostics
            .first()
            .expect("CLI JSON diagnostics should include one typecheck error");

        assert!(
            !output.status.success(),
            "CLI should fail in JSON mode when a pipe-or fallback is incompatible"
        );
        assert_eq!(first["code"], "T1003");
        assert_eq!(first["location"]["line"], 6);
        assert_eq!(first["location"]["column"], 8);
        assert!(
            first["message"]
                .as_str()
                .is_some_and(|message| message.contains("recoverable-error fallback")),
            "CLI JSON diagnostics should preserve the fallback mismatch wording"
        );
        assert!(
            first["location"]["file"]
                .as_str()
                .is_some_and(|file| file.ends_with("main.fol")),
            "CLI JSON diagnostics should preserve the failing source-unit path"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_typecheck_imported_binding_mismatches_keep_exact_locations() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_imported_binding_json");
        let shared_root = temp_root.join("shared");
        let app_root = temp_root.join("app");
        fs::create_dir_all(&shared_root).expect("Should create shared fixture root");
        fs::create_dir_all(&app_root).expect("Should create app fixture root");
        fs::write(shared_root.join("lib.fol"), "var[exp] answer: int = 42;\n")
            .expect("Should write imported binding fixture");
        fs::write(
            app_root.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nvar label: str = answer;\n",
        )
        .expect("Should write imported binding consumer fixture");

        let output = run_fol(&[
            "--json",
            app_root
                .to_str()
                .expect("CLI imported binding fixture path should be utf-8"),
        ]);
        let report = parse_cli_json(&output);
        let first = report["diagnostics"][0].clone();

        assert!(
            !output.status.success(),
            "CLI should fail on imported binding mismatches"
        );
        assert_eq!(first["code"], "T1003");
        assert_eq!(first["location"]["line"], 2);
        assert_eq!(first["location"]["column"], 18);
        assert_eq!(first["location"]["length"], 6);
        assert!(
            first["location"]["file"]
                .as_str()
                .is_some_and(|file| file.ends_with("/app/main.fol")),
            "CLI JSON diagnostics should preserve the imported binding consumer path"
        );
        assert!(
            first["message"]
                .as_str()
                .is_some_and(|message| message.contains("initializer for 'label' expects")),
            "CLI JSON diagnostics should preserve the imported binding mismatch message"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_typecheck_nil_shell_errors_keep_exact_locations() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_nil_json");
        fs::create_dir_all(&temp_root).expect("Should create nil fixture root");
        fs::write(
            temp_root.join("main.fol"),
            "ali MaybeText: opt[str];\nvar label = nil;\n",
        )
        .expect("Should write nil fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI nil fixture path should be utf-8"),
        ]);
        let report = parse_cli_json(&output);
        let first = report["diagnostics"][0].clone();

        assert!(
            !output.status.success(),
            "CLI should fail on unsupported nil shell contexts"
        );
        assert_eq!(first["code"], "T1001");
        assert_eq!(first["location"]["line"], 2);
        assert_eq!(first["location"]["column"], 1);
        assert_eq!(first["location"]["length"], 3);
        assert!(
            first["location"]["file"]
                .as_str()
                .is_some_and(|file| file.ends_with("/main.fol")),
            "CLI JSON diagnostics should preserve the nil fixture path"
        );
        assert!(
            first["message"].as_str().is_some_and(|message| {
                message.contains(
                    "nil literals require an expected opt[...] or err[...] shell type in V1",
                )
            }),
            "CLI JSON diagnostics should preserve the nil shell message"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_typecheck_nested_record_mismatches_keep_exact_locations() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_nested_record_json");
        fs::create_dir_all(&temp_root).expect("Should create nested record fixture root");
        fs::write(
            temp_root.join("main.fol"),
            "typ Meta: rec = {\n\
                 ok: bol\n\
             };\n\
             typ User: rec = {\n\
                 meta: Meta\n\
             };\n\
             fun[] main(): User = {\n\
                 return { meta = { ok = 1 } };\n\
             };\n",
        )
        .expect("Should write nested record mismatch fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI nested record fixture path should be utf-8"),
        ]);
        let report = parse_cli_json(&output);
        let first = report["diagnostics"][0].clone();

        assert!(
            !output.status.success(),
            "CLI should fail on nested record mismatches"
        );
        assert_eq!(first["code"], "T1003");
        assert_eq!(first["location"]["line"], 8);
        assert_eq!(first["location"]["column"], 17);
        assert_eq!(first["location"]["length"], 1);
        assert!(
            first["location"]["file"]
                .as_str()
                .is_some_and(|file| file.ends_with("/main.fol")),
            "CLI JSON diagnostics should preserve the nested record fixture path"
        );
        assert!(
            first["message"]
                .as_str()
                .is_some_and(|message| message.contains("record field 'ok' expects")),
            "CLI JSON diagnostics should preserve the nested record mismatch message"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_modern_compiler_errors_do_not_fall_back_to_unknown_codes() {
        use std::fs;

        let parser_root = unique_temp_root("cli_json_known_parser_code");
        fs::create_dir_all(&parser_root).expect("Should create parser fixture root");
        fs::write(parser_root.join("bad.fol"), "run(1, 2)\n").expect("Should write parser fixture");
        let parser_output = run_fol(&[
            "--json",
            parser_root
                .to_str()
                .expect("Parser fixture path should be valid UTF-8"),
        ]);
        let parser_json = parse_cli_json(&parser_output);
        assert_ne!(
            parser_json["diagnostics"][0]["code"], "EUNKNOWN",
            "Parser diagnostics should use explicit structured parser codes"
        );

        let package_root = unique_temp_root("cli_json_known_package_code");
        let app_root = package_root.join("app");
        let loc_root = package_root.join("formal_pkg");
        fs::create_dir_all(&app_root).expect("Should create app fixture root");
        fs::create_dir_all(&loc_root).expect("Should create loc target fixture root");
        fs::write(
            loc_root.join("build.fol"),
            "pro[] build(): non = {\n    return;\n};\n",
        )
            .expect("Should write formal package control file");
        fs::write(
            app_root.join("main.fol"),
            "use formal: loc = {\"../formal_pkg\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write package fixture");
        let package_output = run_fol(&[
            "--json",
            app_root
                .to_str()
                .expect("Package fixture path should be valid UTF-8"),
        ]);
        let package_json = parse_cli_json(&package_output);
        assert_ne!(
            package_json["diagnostics"][0]["code"], "EUNKNOWN",
            "Package diagnostics should use explicit structured package codes"
        );

        let resolver_root = unique_temp_root("cli_json_known_resolver_code");
        fs::create_dir_all(resolver_root.join("alpha"))
            .expect("Should create first imported namespace fixture");
        fs::create_dir_all(resolver_root.join("beta"))
            .expect("Should create second imported namespace fixture");
        fs::write(
            resolver_root.join("alpha/values.fol"),
            "var[exp] answer: int = 1;\n",
        )
        .expect("Should write first imported exported value fixture");
        fs::write(
            resolver_root.join("beta/values.fol"),
            "var[exp] answer: int = 2;\n",
        )
        .expect("Should write second imported exported value fixture");
        fs::write(
            resolver_root.join("main.fol"),
            "use alpha: loc = {\"alpha\"};\nuse beta: loc = {\"beta\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write resolver fixture");
        let resolver_output = run_fol(&[
            "--json",
            resolver_root
                .to_str()
                .expect("Resolver fixture path should be valid UTF-8"),
        ]);
        let resolver_json = parse_cli_json(&resolver_output);
        assert_ne!(
            resolver_json["diagnostics"][0]["code"], "EUNKNOWN",
            "Resolver diagnostics should use explicit structured resolver codes"
        );

        fs::remove_dir_all(&parser_root).ok();
        fs::remove_dir_all(&package_root).ok();
        fs::remove_dir_all(&resolver_root).ok();
    }

    #[test]
    fn test_parser_error_locations_reach_diagnostics_outputs() {
        use fol_diagnostics::{DiagnosticReport, OutputFormat};
        use fol_lexer::lexer::stage3::Elements;
        use fol_lexer::token::KEYWORD;
        use fol_parser::ast::AstParser;
        use fol_stream::FileStream;

        let mut file_stream =
            FileStream::from_file("test/parser/simple_var.fol").expect("Should read test file");

        let mut lexer = Elements::init(&mut file_stream);
        lexer
            .set_key(KEYWORD::Illegal)
            .expect("Should force illegal token");

        let mut parser = AstParser::new();
        let mut diagnostics = DiagnosticReport::new();

        let parse_errors = parser
            .parse_script_package(&mut lexer)
            .expect_err("Parser should fail on illegal token");

        for diagnostic in parse_errors {
            diagnostics.add_diagnostic(diagnostic);
        }

        let human = diagnostics.output(OutputFormat::Human);
        assert!(
            human.contains("-->"),
            "Human diagnostics should include location"
        );
        assert!(
            human.contains("simple_var.fol"),
            "Human diagnostics should include source file"
        );

        let json = diagnostics.output(OutputFormat::Json);
        assert!(
            json.contains("\"line\""),
            "JSON diagnostics should include line field"
        );
        assert!(
            json.contains("\"column\""),
            "JSON diagnostics should include column field"
        );
    }

    #[test]
    fn test_parser_human_diagnostics_keep_snippet_shape() {
        use fol_diagnostics::{DiagnosticReport, OutputFormat};
        use fol_lexer::lexer::stage3::Elements;
        use fol_lexer::token::KEYWORD;
        use fol_parser::ast::AstParser;
        use fol_stream::FileStream;

        let mut file_stream =
            FileStream::from_file("test/parser/simple_var.fol").expect("Should read test file");

        let mut lexer = Elements::init(&mut file_stream);
        lexer
            .set_key(KEYWORD::Illegal)
            .expect("Should force illegal token");

        let mut parser = AstParser::new();
        let mut diagnostics = DiagnosticReport::new();
        let parse_errors = parser
            .parse_script_package(&mut lexer)
            .expect_err("Parser should fail on illegal token");

        for diagnostic in parse_errors {
            diagnostics.add_diagnostic(diagnostic);
        }

        let human = diagnostics.output(OutputFormat::Human);
        assert!(human.contains("| var x: int = 42;"));
        assert!(human.contains("^"));
        assert!(human.contains("simple_var.fol"));
    }

    #[test]
    fn test_multi_file_parser_errors_keep_second_file_locations() {
        use fol_diagnostics::{DiagnosticReport, OutputFormat};
        use fol_lexer::lexer::stage3::Elements;
        use fol_parser::ast::AstParser;
        use fol_stream::FileStream;
        use std::fs;

        let temp_root = unique_temp_root("parser_multifile_locations");
        let first = temp_root.join("00_good.fol");
        let second = temp_root.join("10_bad.fol");
        fs::create_dir_all(&temp_root).expect("Should create temp parser error fixture dir");
        fs::write(&first, "var ok = 1;\n").expect("Should write first source");
        fs::write(&second, "\"unterminated").expect("Should write malformed second source");

        let mut file_stream = FileStream::from_folder(
            temp_root
                .to_str()
                .expect("Multi-file parser fixture path should be utf-8"),
        )
        .expect("Should build a multi-file stream");
        let mut lexer = Elements::init(&mut file_stream);
        let mut parser = AstParser::new();
        let errors = parser
            .parse_script_package(&mut lexer)
            .expect_err("Second source should surface a parser-visible error");

        let diagnostic = errors
            .iter()
            .find(|d| {
                d.primary_location()
                    .and_then(|loc| loc.file.as_deref())
                    .is_some_and(|path| path.ends_with("10_bad.fol"))
            })
            .expect("A parse error should point at the malformed second file");

        let loc = diagnostic.primary_location().expect("diagnostic should have primary location");
        assert_eq!(
            loc.line,
            1,
            "Second file should restart at line 1"
        );
        assert_eq!(
            loc.column,
            1,
            "Second file should restart at column 1 for its first token"
        );

        let mut diagnostics = DiagnosticReport::new();
        diagnostics.add_diagnostic(diagnostic.clone());
        let human = diagnostics.output(OutputFormat::Human);
        assert!(
            human.contains("10_bad.fol"),
            "Diagnostic output should name the second file that actually failed"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_backend_cli_build_mode_reports_compiled_binary_path() {
        use std::fs;

        let temp_root = unique_temp_root("backend_cli_build");
        fs::create_dir_all(&temp_root).expect("Should create backend build fixture dir");
        let fixture = write_backend_scalar_fixture(&temp_root);

        let output = run_fol(&[fixture
            .to_str()
            .expect("Backend build fixture path should be valid utf-8")]);

        assert!(
            output.status.success(),
            "Backend build mode should succeed: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let binary_marker = "binary=";
        let binary_path = stdout
            .lines()
            .find_map(|line| line.split(binary_marker).nth(1))
            .expect("Backend build output should report a compiled binary path")
            .trim();

        assert!(
            Path::new(binary_path).exists(),
            "Reported backend binary should exist at '{}'",
            binary_path
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_backend_cli_emit_rust_mode_reports_generated_crate_root() {
        use std::fs;

        let temp_root = unique_temp_root("backend_cli_emit");
        fs::create_dir_all(&temp_root).expect("Should create backend emit fixture dir");
        let fixture = write_backend_scalar_fixture(&temp_root);

        let output = run_fol(&[
            "--emit-rust",
            "--keep-build-dir",
            fixture
                .to_str()
                .expect("Backend emit fixture path should be valid utf-8"),
        ]);

        assert!(
            output.status.success(),
            "Backend emit mode should succeed: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let root_marker = "root=";
        let root_path = stdout
            .lines()
            .find_map(|line| line.split(root_marker).nth(1))
            .and_then(|line| line.split(" files=").next())
            .expect("Emit mode should report a generated crate root")
            .trim();

        assert!(
            Path::new(root_path).exists(),
            "Reported generated Rust crate root should exist at '{}'",
            root_path
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_backend_cli_surfaces_build_failures_as_backend_diagnostics() {
        use std::fs;

        let temp_root = unique_temp_root("backend_cli_fail");
        fs::create_dir_all(&temp_root).expect("Should create backend failure fixture dir");
        let fixture = write_backend_scalar_fixture(&temp_root);
        let missing_runtime = temp_root.join("missing-runtime");

        let output = run_fol_with_env(
            &[fixture
                .to_str()
                .expect("Backend failure fixture path should be valid utf-8")],
            &[(
                "FOL_BACKEND_RUNTIME_PATH",
                missing_runtime
                    .to_str()
                    .expect("Missing runtime path should be valid utf-8"),
            )],
        );

        assert!(
            !output.status.success(),
            "Backend build failure should exit non-zero"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("BackendBuildFailure"),
            "Backend failure diagnostics should preserve backend error identity, got:\n{}",
            stdout
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_frontend_fetch_materializes_git_dependencies_and_writes_lockfile() {
        use std::fs;

        let temp_root = unique_temp_root("frontend_fetch_git");
        let app_root = temp_root.join("app");
        let remote_root = temp_root.join("logtiny-remote");
        create_git_package_repo(&remote_root, "logtiny", "0.1.0");
        create_app_with_git_dependency(&app_root, &remote_root);

        let output = run_fol_in_dir(&app_root, &["pack", "fetch"]);

        assert!(
            output.status.success(),
            "frontend fetch should succeed: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let lockfile = app_root.join("fol.lock");
        assert!(lockfile.is_file(), "fetch should write a fol.lock file");
        let lock_text = fs::read_to_string(&lockfile).expect("Should read generated lockfile");
        assert!(lock_text.contains("alias: logtiny"));
        assert!(lock_text.contains("source: git"));
        assert!(lock_text.contains("revision: "));
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("prepared 1 workspace package"),
            "fetch output should keep the successful fetch summary"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_frontend_fetch_supports_workspace_members_with_shared_git_dependencies() {
        use std::fs;

        let temp_root = unique_temp_root("frontend_fetch_workspace_git");
        let app_root = temp_root.join("app");
        let tool_root = temp_root.join("tool");
        let remote_root = temp_root.join("logtiny-remote");
        create_git_package_repo(&remote_root, "logtiny", "0.1.0");
        create_app_with_git_dependency(&app_root, &remote_root);
        create_app_with_git_dependency(&tool_root, &remote_root);
        fs::write(
            temp_root.join("fol.work.yaml"),
            "members:\n  - app\n  - tool\n",
        )
        .expect("Should write workspace config");

        let output = run_fol_in_dir(&temp_root, &["pack", "fetch"]);

        assert!(
            output.status.success(),
            "workspace fetch should succeed: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let lockfile = temp_root.join("fol.lock");
        let lock_text = fs::read_to_string(&lockfile).expect("Should read generated lockfile");
        assert!(
            lockfile.is_file(),
            "workspace fetch should write a workspace fol.lock file"
        );
        assert!(lock_text.contains("alias: logtiny"));
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("prepared 2 workspace package"),
            "workspace fetch output should report both members"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_frontend_locked_fetch_keeps_pinned_revision_after_remote_changes() {
        use std::fs;

        let temp_root = unique_temp_root("frontend_fetch_locked_pin");
        let app_root = temp_root.join("app");
        let remote_root = temp_root.join("logtiny-remote");
        create_git_package_repo(&remote_root, "logtiny", "0.1.0");
        create_app_with_git_dependency(&app_root, &remote_root);

        let initial = run_fol_in_dir(&app_root, &["pack", "fetch"]);
        assert!(initial.status.success(), "initial fetch should succeed");
        let lockfile = app_root.join("fol.lock");
        let pinned_before = read_lock_revision(&lockfile);

        fs::write(remote_root.join("src/lib.fol"), "var[exp] level: int = 2;\n")
            .expect("Should update remote source");
        for args in [vec!["add", "."], vec!["commit", "-m", "bump"]] {
            let status = Command::new("git")
                .args(&args)
                .current_dir(&remote_root)
                .status()
                .expect("Should run git command for remote update");
            assert!(status.success(), "git {:?} should succeed", args);
        }

        let locked = run_fol_in_dir(&app_root, &["pack", "fetch", "--locked"]);
        assert!(
            locked.status.success(),
            "locked fetch should succeed after remote changes: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&locked.stdout),
            String::from_utf8_lossy(&locked.stderr)
        );
        let pinned_after = read_lock_revision(&lockfile);
        assert_eq!(
            pinned_before, pinned_after,
            "locked fetch should keep the pinned revision"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_frontend_offline_fetch_uses_a_warm_cache_from_the_public_cli() {
        let temp_root = unique_temp_root("frontend_fetch_offline_warm");
        let app_root = temp_root.join("app");
        let remote_root = temp_root.join("logtiny-remote");
        create_git_package_repo(&remote_root, "logtiny", "0.1.0");
        create_app_with_git_dependency(&app_root, &remote_root);

        let initial = run_fol_in_dir(&app_root, &["pack", "fetch"]);
        assert!(initial.status.success(), "initial fetch should succeed");

        let offline = run_fol_in_dir(&app_root, &["pack", "fetch", "--offline"]);
        assert!(
            offline.status.success(),
            "offline fetch should succeed with a warm cache: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&offline.stdout),
            String::from_utf8_lossy(&offline.stderr)
        );
        assert!(
            app_root.join("fol.lock").is_file(),
            "offline fetch should keep the lockfile in place"
        );

        std::fs::remove_dir_all(&temp_root).ok();
    }

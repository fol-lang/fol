use super::*;

    #[test]
    fn test_cli_resolver_errors_keep_exact_json_locations_for_plain_unresolved_names() {
        use std::fs;

        let temp_root = unique_temp_root("cli_resolver_plain_unresolved_location");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI resolver fixture");
        let main_file = temp_root.join("main.fol");
        fs::write(
            &main_file,
            "fun[] main(): int = {\n    return missing;\n};\n",
        )
        .expect("Should write unresolved plain-name fixture");

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
            "CLI should fail in JSON mode when resolver rejects an unresolved plain name"
        );
        assert!(
            stdout.contains(
                main_file
                    .to_str()
                    .expect("Temporary resolver fixture path should be valid UTF-8")
            ),
            "JSON resolver diagnostics should keep the exact source file for plain unresolved names"
        );
        assert!(
            !compact.contains("\"file\":null"),
            "JSON resolver diagnostics for plain unresolved names should never drop the file field"
        );
        assert!(
            compact.contains("\"line\":2"),
            "JSON resolver diagnostics should preserve the exact failing line number"
        );
        assert!(
            compact.contains("\"column\":12"),
            "JSON resolver diagnostics should preserve the exact plain-name column"
        );
        assert!(
            stdout.contains("could not resolve name 'missing'"),
            "JSON resolver diagnostics should keep the exact unresolved plain-name wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_resolver_errors_keep_exact_json_locations_for_ambiguous_plain_names() {
        use std::fs;

        let temp_root = unique_temp_root("cli_resolver_plain_ambiguity_location");
        fs::create_dir_all(temp_root.join("alpha"))
            .expect("Should create first imported namespace fixture");
        fs::create_dir_all(temp_root.join("beta"))
            .expect("Should create second imported namespace fixture");
        fs::write(
            temp_root.join("alpha/values.fol"),
            "var[exp] answer: int = 1;\n",
        )
        .expect("Should write first imported exported value fixture");
        fs::write(
            temp_root.join("beta/values.fol"),
            "var[exp] answer: int = 2;\n",
        )
        .expect("Should write second imported exported value fixture");
        let main_file = temp_root.join("main.fol");
        fs::write(
            &main_file,
            "use alpha: loc = {\"alpha\"};\nuse beta: loc = {\"beta\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write ambiguous imported plain-name fixture");

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
            "CLI should fail in JSON mode when resolver rejects an ambiguous plain name"
        );
        assert!(
            stdout.contains(
                main_file
                    .to_str()
                    .expect("Temporary resolver fixture path should be valid UTF-8")
            ),
            "JSON resolver diagnostics should keep the exact source file for ambiguous plain names"
        );
        assert!(
            !compact.contains("\"file\":null"),
            "JSON resolver diagnostics for ambiguous plain names should never drop the file field"
        );
        assert!(
            compact.contains("\"line\":4"),
            "JSON resolver diagnostics should preserve the exact ambiguous line number"
        );
        assert!(
            compact.contains("\"column\":12"),
            "JSON resolver diagnostics should preserve the exact ambiguous plain-name column"
        );
        assert!(
            stdout.contains("name 'answer' is ambiguous in lexical scope"),
            "JSON resolver diagnostics should keep the exact ambiguous plain-name wording"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_parser_errors_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_parser_structured");
        fs::create_dir_all(&temp_root).expect("Should create temp parser fixture");
        fs::write(temp_root.join("00_good.fol"), "var ok = 1;\n").expect("Should write good source");
        let bad_file = temp_root.join("10_bad.fol");
        fs::write(&bad_file, "run(1, 2);\n").expect("Should write invalid file-root source");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI parser fixture path should be utf-8"),
        ]);
        let json = parse_cli_json(&output);
        let diagnostic = &json["diagnostics"][0];

        assert!(
            !output.status.success(),
            "Parser fixture should fail in JSON mode"
        );
        assert_eq!(json["error_count"], 1);
        assert_eq!(json["warning_count"], 0);
        assert_eq!(diagnostic["severity"], "Error");
        assert!(diagnostic["code"].as_str().is_some());
        assert_eq!(
            diagnostic["message"],
            "Executable calls are not allowed at file root"
        );
        assert_eq!(
            diagnostic["location"]["file"],
            bad_file
                .to_str()
                .expect("Temporary parser fixture path should be valid UTF-8")
        );
        assert_eq!(diagnostic["location"]["line"], 1);
        assert_eq!(diagnostic["location"]["column"], 1);
        assert_eq!(diagnostic["location"]["length"], 3);
        assert_eq!(
            diagnostic["labels"].as_array().map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            diagnostic["notes"].as_array().map(|items| items.len()),
            Some(0)
        );
        assert_eq!(
            diagnostic["helps"].as_array().map(|items| items.len()),
            Some(0)
        );
        assert_eq!(
            diagnostic["suggestions"]
                .as_array()
                .map(|items| items.len()),
            Some(0)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_package_errors_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_package_structured");
        let app_root = temp_root.join("app");
        let loc_root = temp_root.join("formal_pkg");
        fs::create_dir_all(&app_root).expect("Should create app fixture root");
        fs::create_dir_all(&loc_root).expect("Should create loc target fixture root");
        fs::write(
            loc_root.join("build.fol"),
            "pro[] build(): non = {\n    return;\n};\n",
        )
            .expect("Should write formal package control file");
        let main_file = app_root.join("main.fol");
        fs::write(
            &main_file,
            "use formal: loc = {\"../formal_pkg\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write loc misuse fixture");

        let output = run_fol(&[
            "--json",
            app_root
                .to_str()
                .expect("CLI package fixture path should be utf-8"),
        ]);
        let json = parse_cli_json(&output);
        let diagnostic = &json["diagnostics"][0];

        assert!(
            !output.status.success(),
            "Package fixture should fail in JSON mode"
        );
        assert_eq!(json["error_count"], 1);
        assert_eq!(diagnostic["severity"], "Error");
        assert!(diagnostic["code"].as_str().is_some());
        assert_eq!(
            diagnostic["location"]["file"],
            main_file
                .to_str()
                .expect("Temporary package fixture path should be valid UTF-8")
        );
        assert_eq!(diagnostic["location"]["line"], 1);
        assert_eq!(diagnostic["location"]["column"], 1);
        assert_eq!(
            diagnostic["labels"].as_array().map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            diagnostic["notes"].as_array().map(|items| items.len()),
            Some(0)
        );
        assert_eq!(
            diagnostic["helps"].as_array().map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            diagnostic["helps"][0],
            "replace the import source kind with pkg for formal packages"
        );
        let message = diagnostic["message"]
            .as_str()
            .expect("Package diagnostic message should stay a string");
        assert!(message.contains("build.fol"));
        assert!(message.contains("pkg instead of loc"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_resolver_errors_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_resolver_structured");
        fs::create_dir_all(temp_root.join("alpha"))
            .expect("Should create first imported namespace fixture");
        fs::create_dir_all(temp_root.join("beta"))
            .expect("Should create second imported namespace fixture");
        fs::write(
            temp_root.join("alpha/values.fol"),
            "var[exp] answer: int = 1;\n",
        )
        .expect("Should write first imported exported value fixture");
        fs::write(
            temp_root.join("beta/values.fol"),
            "var[exp] answer: int = 2;\n",
        )
        .expect("Should write second imported exported value fixture");
        let main_file = temp_root.join("main.fol");
        fs::write(
            &main_file,
            "use alpha: loc = {\"alpha\"};\nuse beta: loc = {\"beta\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write ambiguous imported plain-name fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("CLI resolver fixture path should be utf-8"),
        ]);
        let json = parse_cli_json(&output);
        let diagnostic = &json["diagnostics"][0];

        assert!(
            !output.status.success(),
            "Resolver fixture should fail in JSON mode"
        );
        assert_eq!(json["error_count"], 1);
        assert_eq!(diagnostic["severity"], "Error");
        assert!(diagnostic["code"].as_str().is_some());
        assert_eq!(
            diagnostic["location"]["file"],
            main_file
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        );
        assert_eq!(diagnostic["location"]["line"], 4);
        assert_eq!(diagnostic["location"]["column"], 12);
        assert_eq!(diagnostic["location"]["length"], 6);
        assert_eq!(
            diagnostic["labels"].as_array().map(|items| items.len()),
            Some(3)
        );
        assert_eq!(diagnostic["labels"][1]["kind"], "Secondary");
        assert_eq!(diagnostic["labels"][2]["kind"], "Secondary");
        assert_eq!(
            diagnostic["labels"][1]["message"],
            "candidate value binding declaration"
        );
        assert_eq!(
            diagnostic["labels"][2]["message"],
            "candidate value binding declaration"
        );
        assert_eq!(
            diagnostic["notes"].as_array().map(|items| items.len()),
            Some(0)
        );
        assert_eq!(
            diagnostic["helps"].as_array().map(|items| items.len()),
            Some(0)
        );
        let message = diagnostic["message"]
            .as_str()
            .expect("Resolver diagnostic message should stay a string");
        assert!(message.contains("name 'answer' is ambiguous in lexical scope"));
        assert!(message.contains("candidates:"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_resolver_errors_keep_structured_fields_for_missing_explicit_std_overrides() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_resolver_std_override");
        let missing_std_root = temp_root.join("missing-std");
        let store_root = temp_root.join("pkg");
        let app_root = temp_root.join("app");
        fs::create_dir_all(app_root.join("src")).expect("Should create resolver fixture root");
        fs::write(
            app_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
                "    graph.install(app);\n",
                "};\n",
            ),
        )
        .expect("Should write app build fixture");
        fs::write(
            app_root.join("src/main.fol"),
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
        )
        .expect("Should write missing explicit std-root fixture");

        let output = run_fol(&[
            "--json",
            "--std-root",
            missing_std_root
                .to_str()
                .expect("Missing explicit std-root path should be valid UTF-8"),
            "--package-store-root",
            store_root
                .to_str()
                .expect("Package store root path should be valid UTF-8"),
            app_root
                .to_str()
                .expect("Resolver fixture path should be valid UTF-8"),
        ]);
        let json = parse_cli_json(&output);
        let diagnostic = &json["diagnostics"][0];

        assert!(
            !output.status.success(),
            "Missing explicit std-root override should fail"
        );
        assert_eq!(diagnostic["code"], "R1001");
        let message = diagnostic["message"]
            .as_str()
            .expect("Resolver diagnostic message should stay a string");
        assert!(message.contains("does not exist"));

        fs::remove_dir_all(&temp_root).ok();
    }

#[test]
fn test_cli_json_resolver_errors_report_missing_bundled_std_modules() {
    use std::fs;

    let temp_root = unique_temp_root("cli_json_missing_bundled_std_module");
    let app_root = temp_root.join("app");
    fs::create_dir_all(app_root.join("src")).expect("Should create resolver fixture root");
    fs::write(
        app_root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "};\n",
        ),
    )
    .expect("Should write app build fixture");
    fs::write(
        app_root.join("src/main.fol"),
        "use os: pkg = {\"std/os\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write missing bundled std module fixture");

    let output = run_fol(&[
        "--json",
        "--package-store-root",
        repo_root()
            .join("lang/library")
            .to_str()
            .expect("Bundled library root should be valid UTF-8"),
        app_root
            .to_str()
            .expect("Resolver fixture path should be valid UTF-8"),
    ]);
    let json = parse_cli_json(&output);
    let diagnostic = &json["diagnostics"][0];

    assert!(
        !output.status.success(),
        "Missing bundled std module should fail"
    );
    let message = diagnostic["message"]
        .as_str()
        .expect("Resolver diagnostic message should stay a string");
    assert!(message.contains("resolver pkg import target"));
    assert!(message.contains("std/os"));

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_cli_json_resolver_errors_keep_exact_bundled_std_module_paths() {
    use std::fs;

    let temp_root = unique_temp_root("cli_json_missing_nested_bundled_std_module");
    let app_root = temp_root.join("app");
    fs::create_dir_all(app_root.join("src")).expect("Should create resolver fixture root");
    fs::write(
        app_root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "};\n",
        ),
    )
    .expect("Should write app build fixture");
    fs::write(
        app_root.join("src/main.fol"),
        "use math: pkg = {\"std/fmt/missing\"};\nfun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("Should write missing nested bundled std module fixture");

    let output = run_fol(&[
        "--json",
        "--package-store-root",
        repo_root()
            .join("lang/library")
            .to_str()
            .expect("Bundled library root should be valid UTF-8"),
        app_root
            .to_str()
            .expect("Resolver fixture path should be valid UTF-8"),
    ]);
    let json = parse_cli_json(&output);
    let diagnostic = &json["diagnostics"][0];

    assert!(
        !output.status.success(),
        "Missing nested bundled std module should fail"
    );
    let message = diagnostic["message"]
        .as_str()
        .expect("Resolver diagnostic message should stay a string");
    assert!(message.contains("resolver pkg import target"));
    assert!(message.contains("std/fmt/missing"));

    fs::remove_dir_all(&temp_root).ok();
}

    #[test]
    fn test_cli_json_resolver_errors_keep_notes_for_unsupported_import_kinds() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_resolver_unsupported_note");
        fs::create_dir_all(&temp_root).expect("Should create resolver fixture root");
        fs::write(temp_root.join("main.fol"), "use fmt: mod = {\"core::fmt\"};\n")
            .expect("Should write unsupported import fixture");

        let output = run_fol(&[
            "--json",
            temp_root
                .to_str()
                .expect("Resolver fixture path should be valid UTF-8"),
        ]);
        let json = parse_cli_json(&output);
        let diagnostic = &json["diagnostics"][0];

        assert!(
            !output.status.success(),
            "Unsupported import fixture should fail"
        );
        assert_eq!(
            diagnostic["notes"].as_array().map(|items| items.len()),
            Some(1)
        );
        assert_eq!(
            diagnostic["notes"][0],
            "supported import source kinds are loc, std, and pkg"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_human_resolver_errors_render_secondary_labels() {
        use std::fs;

        let temp_root = unique_temp_root("cli_human_resolver_labels");
        fs::create_dir_all(temp_root.join("alpha"))
            .expect("Should create first imported namespace fixture");
        fs::create_dir_all(temp_root.join("beta"))
            .expect("Should create second imported namespace fixture");
        fs::write(
            temp_root.join("alpha/values.fol"),
            "var[exp] answer: int = 1;\n",
        )
        .expect("Should write first imported exported value fixture");
        fs::write(
            temp_root.join("beta/values.fol"),
            "var[exp] answer: int = 2;\n",
        )
        .expect("Should write second imported exported value fixture");
        fs::write(
            temp_root.join("main.fol"),
            "use alpha: loc = {\"alpha\"};\nuse beta: loc = {\"beta\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write ambiguous imported plain-name fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("Resolver fixture path should be valid UTF-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "Ambiguous resolver fixture should fail"
        );
        // Pretty human mode: the code shows as a chip suffix (`R1005`), and the
        // secondary labels render as framed candidate sites.
        assert!(stdout.contains("R1005"));
        assert!(stdout.contains("candidate value binding declaration"));
        assert!(stdout.contains("alpha/values.fol"));
        assert!(stdout.contains("beta/values.fol"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_human_package_errors_render_help_guidance() {
        use std::fs;

        let temp_root = unique_temp_root("cli_human_package_help");
        let app_root = temp_root.join("app");
        let loc_root = temp_root.join("formal_pkg");
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
        .expect("Should write loc misuse fixture");

        let output = run_fol(&[app_root
            .to_str()
            .expect("Package fixture path should be valid UTF-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "Formal package loc misuse should fail"
        );
        assert!(stdout.contains("R1001"));
        assert!(stdout.contains("pkg instead of loc"));
        assert!(
            stdout.contains("help: replace the import source kind with pkg for formal packages")
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_accepts_v1_programs_after_resolution() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_success");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI typecheck fixture");
        fs::write(
            temp_root.join("main.fol"),
            "var value: int = 1;\nfun[] main(): int = {\n    return value;\n};\n",
        )
        .expect("Should write the successful typecheck fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI typecheck fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should accept parse-clean, resolve-clean, type-correct V1 programs, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Human CLI output should still report a successful compile after typechecking"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_typecheck_errors_fail_parse_clean_programs() {
        use std::fs;

        let temp_root = unique_temp_root("cli_typecheck_error");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI typecheck error fixture");
        fs::write(temp_root.join("main.fol"), "var[bor] borrowed: int = 1;\n")
            .expect("Should write the unsupported typecheck fixture");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI typecheck error fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should fail when typechecking rejects a parse-clean, resolve-clean program"
        );
        assert!(
            stdout.contains("borrowing binding semantics are planned for a future release"),
            "CLI diagnostics should surface the typecheck unsupported message"
        );
        assert!(
            stdout.contains("main.fol"),
            "CLI diagnostics should preserve the failing source-unit path"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

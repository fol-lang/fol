use super::*;

    #[test]
    fn test_cli_single_file_compile_succeeds_with_package_parser() {
        let output = run_fol(&["test/parser/simple_var.fol"]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should accept declaration-only single-file input, got status {:?} and output:\n{}",
            output.status.code(),
            stdout
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Human CLI output should still report a successful compile"
        );
    }

    #[test]
    fn test_cli_typecheck_accepts_loc_imported_symbols_after_workspace_handoff() {
        use std::fs;

        let temp_root = unique_temp_root("cli_loc_import");
        let shared_root = temp_root.join("shared");
        let app_root = temp_root.join("app");
        fs::create_dir_all(&shared_root).expect("Should create the shared fixture directory");
        fs::create_dir_all(&app_root).expect("Should create the app fixture directory");
        fs::write(shared_root.join("lib.fol"), "var[exp] answer: int = 42;\n")
            .expect("Should write the shared export fixture");
        fs::write(
            app_root.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write the loc import fixture");

        let output = run_fol(&[app_root
            .to_str()
            .expect("Temporary app fixture path should be valid UTF-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should typecheck imported loc symbols through the full workspace-aware chain, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Human CLI output should still report a successful compile for loc-imported packages"
        );
    }

    #[test]
    fn test_release_workflow_uses_the_current_stable_rust_toolchain() {
        let release_workflow =
            std::fs::read_to_string(repo_root().join(".github/workflows/release.yml"))
                .expect("release workflow should exist");

        assert!(
            release_workflow.contains("TOOLCHAIN_VERSION : stable"),
            "release workflow should track the current stable Rust toolchain"
        );
        assert!(
            !release_workflow.contains("TOOLCHAIN_VERSION : 1.70.0"),
            "release workflow should no longer pin the stale Rust 1.70.0 toolchain"
        );
    }

    #[test]
    fn test_workspace_crates_keep_authors_metadata() {
        let crates = [
            "lang/compiler/fol-intrinsics/Cargo.toml",
            "lang/compiler/fol-typecheck/Cargo.toml",
            "lang/compiler/fol-lower/Cargo.toml",
            "lang/execution/fol-backend/Cargo.toml",
            "lang/execution/fol-runtime/Cargo.toml",
            "lang/tooling/fol-editor/Cargo.toml",
            "lang/tooling/fol-frontend/Cargo.toml",
        ];

        for path in crates {
            let manifest = std::fs::read_to_string(repo_root().join(path))
                .expect("workspace Cargo.toml should exist");
            assert!(
                manifest.contains("authors = [\"Trim Bresilla <trim.bresilla@gmail.com>\"]"),
                "workspace crate manifest should keep authors metadata: {path}"
            );
        }
    }

    #[test]
    fn test_cli_resolves_std_imports_from_the_bundled_std_root_by_default() {
        use std::fs;

        let temp_root = unique_temp_root("cli_bundled_std_import");
        let app_root = temp_root.join("app");
        fs::create_dir_all(app_root.join("src"))
            .expect("Should create bundled std import fixture root");
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
        .expect("Should write bundled std build fixture");
        fs::write(
            app_root.join("src/main.fol"),
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
        )
        .expect("Should write bundled std import fixture");

        let output = run_fol(&[
            "--json",
            "--package-store-root",
            repo_root()
                .join("lang/library")
                .to_str()
                .expect("Bundled library root should be valid UTF-8"),
            app_root
                .to_str()
                .expect("Temporary bundled std fixture path should be valid UTF-8"),
        ]);

        assert!(
            output.status.success(),
            "CLI should resolve std imports through the bundled std root by default, got status {:?} and output:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_resolves_std_imports_with_explicit_std_root_configuration() {
        use std::fs;

        // Dependency-backed std imports are satisfied through the explicit
        // package-store root used for the declared alias.
        let temp_root = unique_temp_root("cli_std_root_import");
        let store_root = temp_root.join("pkg");
        let app_root = temp_root.join("app");
        fs::create_dir_all(store_root.join("std/fmt"))
            .expect("Should create the standard-library fixture directory");
        fs::create_dir_all(app_root.join("src"))
            .expect("Should create the importing package root fixture directory");
        fs::write(
            store_root.join("std/build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"std\", version = \"0.1.0\" });\n};\n",
        )
        .expect("Should write the standard-library build fixture");
        fs::write(
            store_root.join("std/fmt/root.fol"),
            "fun[exp] answer(): int = {\n    return 42;\n};\n",
        )
        .expect("Should write the standard-library export fixture");
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
        .expect("Should write the app build fixture");
        fs::write(
            app_root.join("src/main.fol"),
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
        )
        .expect("Should write the std import fixture");

        let output = run_fol(&[
            "--package-store-root",
            store_root
                .to_str()
                .expect("Temporary package-store root should be valid UTF-8"),
            app_root
                .to_str()
                .expect("Temporary app fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should resolve std imports through an explicit package-store root, got status {:?} and output:;\n{};",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Human CLI output should still report a successful compile for std-imported packages",
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_explicit_std_root_override_can_swap_bundled_std_for_dev_tests() {
        use std::fs;

        let temp_root = unique_temp_root("cli_std_root_override_swap");
        let store_root = temp_root.join("pkg");
        let app_root = temp_root.join("app");
        fs::create_dir_all(store_root.join("std/fmt"))
            .expect("Should create the override std fmt directory");
        fs::create_dir_all(app_root.join("src"))
            .expect("Should create the importing package root fixture directory");
        fs::write(
            store_root.join("std/build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"std\", version = \"0.1.0\" });\n};\n",
        )
        .expect("Should write the override std build fixture");
        fs::write(
            store_root.join("std/fmt/root.fol"),
            "fun[exp] shadow(): int = {\n    return 42;\n};\n",
        )
        .expect("Should write the override std fmt fixture");
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
        .expect("Should write the app build fixture");
        fs::write(
            app_root.join("src/main.fol"),
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::shadow();\n};\n",
        )
        .expect("Should write the std import fixture");

        let default_output = run_fol(&[
            "--package-store-root",
            repo_root()
                .join("lang/library")
                .to_str()
                .expect("Temporary package-store root should be valid UTF-8"),
            app_root
            .to_str()
            .expect("Temporary app fixture path should be valid UTF-8")]);
        assert!(
            !default_output.status.success(),
            "Without --std-root the bundled std should stay canonical and reject override-only names: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&default_output.stdout),
            String::from_utf8_lossy(&default_output.stderr)
        );

        let override_output = run_fol(&[
            "--package-store-root",
            store_root
                .to_str()
                .expect("Temporary package-store root should be valid UTF-8"),
            app_root
                .to_str()
                .expect("Temporary app fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&override_output.stdout);

        assert!(
            override_output.status.success(),
            "Explicit --std-root should intentionally swap bundled std during tests, got status {:?} and output:\n{}",
            override_output.status.code(),
            stdout,
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Human CLI output should still report a successful compile for override std imports",
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_accepts_explicit_package_store_root_configuration() {
        use std::fs;

        let temp_root = unique_temp_root("cli_package_store_root");
        let store_root = temp_root.join("store");
        let app_root = temp_root.join("app");
        fs::create_dir_all(store_root.join("json"))
            .expect("Should create the package-store fixture directory");
        fs::create_dir_all(&app_root)
            .expect("Should create the importing package root fixture directory");
        fs::write(
            store_root.join("json/build.fol"),
            "name: json\nversion: 1.0.0\n",
        )
        .expect("Should write the installed package metadata fixture");
        fs::create_dir_all(store_root.join("json/src"))
            .expect("Should create the installed package export root fixture");
        fs::write(
            store_root.join("json/build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"json\",\n        version = \"1.0.0\",\n    });\n};\n",
        )
        .expect("Should write the installed package build fixture");
        fs::write(
            store_root.join("json/src/lib.fol"),
            "var[exp] answer: int = 42;\n",
        )
        .expect("Should write the installed package export fixture");
        fs::write(
            app_root.join("main.fol"),
            "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return json::src::answer;\n};\n",
        )
        .expect("Should write the pkg import fixture");

        let output = run_fol(&[
            "--package-store-root",
            store_root
                .to_str()
                .expect("Temporary package-store fixture path should be valid UTF-8"),
            app_root
                .to_str()
                .expect("Temporary app fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should accept an explicit package-store root and resolve pkg imports, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Human CLI output should still report a successful compile for pkg-imported packages"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_succeeds_for_loc_import_graphs() {
        use std::fs;

        let temp_root = unique_temp_root("cli_dump_lowered_loc");
        let shared_root = temp_root.join("shared");
        let app_root = temp_root.join("app");
        fs::create_dir_all(&shared_root).expect("Should create the shared fixture directory");
        fs::create_dir_all(&app_root).expect("Should create the app fixture directory");
        fs::write(shared_root.join("lib.fol"), "var[exp] answer: int = 42;\n")
            .expect("Should write the shared export fixture");
        fs::write(
            app_root.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
        )
        .expect("Should write the loc import fixture");

        let output = run_fol(&[
            "--dump-lowered",
            app_root
                .to_str()
                .expect("Temporary app fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for loc-import graphs, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(stdout.contains("workspace entry=app"));
        assert!(stdout.contains("package app"));
        assert!(stdout.contains("package shared"));
        assert!(stdout.contains("entry-candidates"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_succeeds_for_std_import_graphs() {
        use std::fs;

        let temp_root = unique_temp_root("cli_dump_lowered_std");
        let app_root = temp_root.join("app");
        fs::create_dir_all(app_root.join("src"))
            .expect("Should create the importing package root fixture directory");
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
        .expect("Should write the app build fixture");
        fs::write(
            app_root.join("src/main.fol"),
            "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::fmt::answer();\n};\n",
        )
        .expect("Should write the std import fixture");

        let output = run_fol(&[
            "--dump-lowered",
            "--package-store-root",
            repo_root()
                .join("lang/library")
                .to_str()
                .expect("Bundled library root should be valid UTF-8"),
            app_root
                .to_str()
                .expect("Temporary app fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for std-import graphs, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(stdout.contains("workspace entry=app"));
        assert!(stdout.contains("package app"));
        assert!(stdout.contains("package std"));
        assert!(stdout.contains("entry-candidates"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_succeeds_for_pkg_import_graphs() {
        use std::fs;

        let temp_root = unique_temp_root("cli_dump_lowered_pkg");
        let store_root = temp_root.join("store");
        let app_root = temp_root.join("app");
        fs::create_dir_all(store_root.join("json"))
            .expect("Should create the package-store fixture directory");
        fs::create_dir_all(&app_root)
            .expect("Should create the importing package root fixture directory");
        fs::write(
            store_root.join("json/build.fol"),
            "name: json\nversion: 1.0.0\n",
        )
        .expect("Should write the installed package metadata fixture");
        fs::create_dir_all(store_root.join("json/src"))
            .expect("Should create the installed package export root fixture");
        fs::write(
            store_root.join("json/build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"json\",\n        version = \"1.0.0\",\n    });\n};\n",
        )
        .expect("Should write the installed package build fixture");
        fs::write(
            store_root.join("json/src/lib.fol"),
            "var[exp] answer: int = 42;\n",
        )
        .expect("Should write the installed package export fixture");
        fs::write(
            app_root.join("main.fol"),
            "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return json::src::answer;\n};\n",
        )
        .expect("Should write the pkg import fixture");

        let output = run_fol(&[
            "--dump-lowered",
            "--package-store-root",
            store_root
                .to_str()
                .expect("Temporary package-store fixture path should be valid UTF-8"),
            app_root
                .to_str()
                .expect("Temporary app fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for pkg-import graphs, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(stdout.contains("workspace entry=app"));
        assert!(stdout.contains("package app"));
        assert!(stdout.contains("package json"));
        assert!(stdout.contains("entry-candidates"));

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_repo_fixtures_only_use_quoted_import_targets_in_resolver_and_cli_paths() {
        let offenders = collect_unquoted_use_target_lines(
            &[
                repo_root().join("test/resolver"),
                repo_root().join("test/integration_tests"),
                repo_root().join("lang/tooling/fol-frontend/src/build_route/tests"),
            ],
            &[".rs", ".fol"],
            &["use std: pkg = {std};"],
        );

        assert!(
            offenders.is_empty(),
            "Resolver, CLI, and routed workspace fixtures should only use quoted import targets:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn test_repo_fixtures_only_use_quoted_import_targets_in_lower_type_and_backend_paths() {
        let offenders = collect_unquoted_use_target_lines(
            &[
                repo_root().join("lang/compiler/fol-lower/src"),
                repo_root().join("test/typecheck"),
                repo_root().join("lang/execution/fol-backend/src"),
            ],
            &[".rs", ".fol"],
            &[],
        );

        assert!(
            offenders.is_empty(),
            "Lowering, typecheck, and backend fixtures should only use quoted import targets:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn test_cli_rejects_unquoted_std_import_targets_with_parser_guidance() {
        use std::fs;

        let temp_root = unique_temp_root("cli_unquoted_std_import");
        let app_root = temp_root.join("app");
        fs::create_dir_all(app_root.join("src"))
            .expect("Should create explicit std dependency fixture root");
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
        .expect("Should write the app build fixture");
        fs::write(
            app_root.join("src/main.fol"),
            "use std: pkg = {std};\nfun[] main(): int = {\n    return 0;\n};\n",
        )
        .expect("Should write the unquoted std import fixture");

        let output = run_fol(&[
            app_root
                .to_str()
                .expect("Temporary app fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            !output.status.success(),
            "CLI should reject unquoted std import targets, got status {:?} and output:\n{}",
            output.status.code(),
            stdout
        );
        assert!(
            stdout.contains("Import targets must be quoted string literals inside braces"),
            "Parser diagnostics should point directly at the quoted-target rule"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_examples_and_docs_keep_quoted_import_targets_only() {
        let offenders = collect_unquoted_use_target_lines(
            &[
                repo_root().join("examples"),
                repo_root().join("test/apps/fixtures"),
                repo_root().join("test/app/formal"),
                repo_root().join("docs"),
                repo_root().join("book"),
                repo_root().join("AGENTS.md"),
            ],
            &[".fol", ".md"],
            &["use std: pkg = {std};"],
        );

        assert!(
            offenders.is_empty(),
            "Examples, fixtures, docs, and book should keep quoted import targets only:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn test_examples_and_docs_do_not_use_removed_std_source_kind_examples() {
        let offenders = collect_lines_containing_any(
            &[
                repo_root().join("examples"),
                repo_root().join("test/apps/fixtures"),
                repo_root().join("test/app/formal"),
                repo_root().join("docs"),
                repo_root().join("book"),
                repo_root().join("AGENTS.md"),
            ],
            &[".fol", ".md"],
            &[": std = "],
        );

        assert!(
            offenders.is_empty(),
            "Examples, docs, and book should not use the removed `std` import kind:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn test_examples_fixtures_docs_and_book_do_not_use_removed_std_mode_contracts() {
        let offenders = collect_lines_containing_any(
            &[
                repo_root().join("examples"),
                repo_root().join("test/apps/fixtures"),
                repo_root().join("test/app/formal"),
                repo_root().join("docs"),
                repo_root().join("book"),
                repo_root().join("AGENTS.md"),
            ],
            &[".fol", ".md"],
            &["fol_model = \"std\"", "std mode"],
        );

        assert!(
            offenders.is_empty(),
            "Examples, fixtures, docs, and book should not keep removed std-mode contracts:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn test_docs_book_and_agents_keep_current_v1_editor_contract() {
        let offenders = collect_lines_containing_any(
            &[
                repo_root().join("docs"),
                repo_root().join("book"),
                repo_root().join("AGENTS.md"),
            ],
            &[".md"],
            &[
                "rangeFormattingProvider",
                "rename across packages",
                "all compiler diagnostics have quick fixes",
                "code actions for every compiler diagnostic",
                "full V2 editor behavior",
            ],
        );

        assert!(
            offenders.is_empty(),
            "Docs, book, and contributor rules should not overclaim the shipped V1 editor surface:\n{}",
            offenders.join("\n")
        );

        let lsp_book = std::fs::read_to_string(repo_root().join("book/src/050_tooling/500_lsp.md"))
            .expect("LSP book chapter should exist");
        assert!(
            lsp_book.contains("code actions for compiler-suggested unresolved-name replacements"),
            "LSP book should keep the current narrow code-action contract"
        );
        assert!(
            lsp_book.contains("rename for same-file local and current-package top-level symbols"),
            "LSP book should keep the current rename boundary"
        );
        assert!(
            lsp_book.contains("`textDocument/rangeFormatting` remains unsupported"),
            "LSP book should keep the current range-formatting boundary"
        );

        let editor_sync =
            std::fs::read_to_string(repo_root().join("docs/editor-sync.md"))
                .expect("editor sync docs should exist");
        assert!(
            editor_sync.contains("Current editor non-goals"),
            "editor sync docs should state the current editor non-goals explicitly"
        );
        assert!(
            editor_sync.contains("shipped V2-aware coverage is intentionally narrow"),
            "editor sync docs should describe the current shipped V2 editor subset"
        );

        let agents =
            std::fs::read_to_string(repo_root().join("AGENTS.md")).expect("AGENTS should exist");
        assert!(
            agents.contains("Current editor non-goals"),
            "AGENTS should carry the current editor non-goal guidance"
        );
        assert!(
            lsp_book.contains(
                "generic-routine, generic-type, constrained-generic, and protocol-standard"
            ),
            "LSP book should describe the shipped V2 editor subset explicitly"
        );
    }

    #[test]
    fn test_positive_std_package_roots_keep_explicit_internal_standard_contract() {
        use std::path::PathBuf;

        let roots = [
            repo_root().join("examples"),
            repo_root().join("test/apps/fixtures"),
            repo_root().join("test/app/formal"),
        ];
        let mut build_files = Vec::new();
        for root in roots {
            collect_files_with_suffixes(&root, &["build.fol"], &mut build_files);
        }
        build_files.sort();

        let mut offenders = Vec::new();
        for build_file in build_files {
            let root = build_file
                .parent()
                .expect("build.fol should have a package root parent");
            let root_text = root.to_string_lossy();
            if root_text.contains("/fail_") || root_text.contains("_fail") {
                continue;
            }

            let mut fol_files = Vec::<PathBuf>::new();
            collect_files_with_suffixes(root, &[".fol"], &mut fol_files);
            let imports_std = fol_files.iter().any(|path| {
                std::fs::read_to_string(path)
                    .map(|text| text.contains("use std: pkg = {\"std\"};"))
                    .unwrap_or(false)
            });

            if !imports_std {
                continue;
            }

            let build_text =
                std::fs::read_to_string(&build_file).expect("should read build.fol for contract scan");
            if !build_text.contains("source = \"internal\"")
                || !build_text.contains("target = \"standard\"")
            {
                offenders.push(build_file.display().to_string());
            }
        }

        assert!(
            offenders.is_empty(),
            "Positive std-consuming package roots should declare bundled std explicitly:\n{}",
            offenders.join("\n")
        );
    }

    #[test]
    fn test_cli_dump_lowered_succeeds_for_intrinsic_comparison_calls() {
        use std::fs;

        let temp_root = unique_temp_root("cli_dump_lowered_intrinsic_comparisons");
        fs::create_dir_all(&temp_root).expect("Should create temp intrinsic comparison fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] main(): bol = {\n",
                "    var same: bol = .eq(1, 1);\n",
                "    var ordered: bol = .lt(\"Ada\", \"Lin\");\n",
                "    return .ge('z', 'a');\n",
                "};\n",
            ),
        )
        .expect("Should write intrinsic comparison fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("Temporary intrinsic comparison fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for intrinsic comparison calls, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.matches("IntrinsicCall").count() >= 3,
            "Lowered dump should retain explicit intrinsic calls for comparison families, got:\n{}",
            stdout,
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_intrinsic_comparison_failures_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_intrinsic_comparison_failures");
        fs::create_dir_all(&temp_root)
            .expect("Should create temp intrinsic comparison failure fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] main(): bol = {\n",
                "    return .lt(true, false);\n",
                "};\n",
            ),
        )
        .expect("Should write intrinsic comparison failure fixture");

        let output = run_fol(&[
            "--json",
            fixture.to_str().expect(
                "Temporary intrinsic comparison failure fixture path should be valid UTF-8",
            ),
        ]);

        assert!(
            !output.status.success(),
            "CLI should fail for invalid intrinsic comparison calls",
        );

        let json = parse_cli_json(&output);
        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("CLI JSON output should expose diagnostics");
        let ordered_error = diagnostics.iter().find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .map(|message| message.contains(".lt(...) expects two ordered scalar operands"))
                .unwrap_or(false)
        });

        assert!(
            ordered_error.is_some(),
            "Expected intrinsic comparison diagnostic in CLI JSON output, got: {json}"
        );
        assert!(
            ordered_error
                .and_then(|diagnostic| diagnostic["location"].as_object())
                .is_some(),
            "Expected intrinsic comparison diagnostic to keep a structured location, got: {json}"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_succeeds_for_intrinsic_boolean_calls() {
        use std::fs;

        let temp_root = unique_temp_root("cli_dump_lowered_intrinsic_boolean");
        fs::create_dir_all(&temp_root).expect("Should create temp intrinsic boolean fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] main(flag: bol): bol = {\n",
                "    var inverted: bol = .not(flag);\n",
                "    return .not(inverted);\n",
                "};\n",
            ),
        )
        .expect("Should write intrinsic boolean fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("Temporary intrinsic boolean fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for intrinsic boolean calls, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.matches("IntrinsicCall").count() >= 2,
            "Lowered dump should retain explicit intrinsic calls for '.not', got:\n{}",
            stdout,
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_intrinsic_boolean_failures_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_intrinsic_boolean_failures");
        fs::create_dir_all(&temp_root)
            .expect("Should create temp intrinsic boolean failure fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!("fun[] main(): bol = {\n", "    return .not(1);\n", "};\n",),
        )
        .expect("Should write intrinsic boolean failure fixture");

        let output = run_fol(&[
            "--json",
            fixture
                .to_str()
                .expect("Temporary intrinsic boolean failure fixture path should be valid UTF-8"),
        ]);

        assert!(
            !output.status.success(),
            "CLI should fail for invalid intrinsic boolean calls",
        );

        let json = parse_cli_json(&output);
        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("CLI JSON output should expose diagnostics");
        let boolean_error = diagnostics.iter().find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .map(|message| message.contains(".not(...) expects one boolean operand"))
                .unwrap_or(false)
        });

        assert!(
            boolean_error.is_some(),
            "Expected intrinsic boolean diagnostic in CLI JSON output, got: {json}"
        );
        assert!(
            boolean_error
                .and_then(|diagnostic| diagnostic["location"].as_object())
                .is_some(),
            "Expected intrinsic boolean diagnostic to keep a structured location, got: {json}"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_succeeds_for_intrinsic_length_calls() {
        use std::fs;

        let temp_root = unique_temp_root("cli_dump_lowered_intrinsic_length");
        fs::create_dir_all(&temp_root).expect("Should create temp intrinsic length fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] main(items: seq[int]): int = {\n",
                "    var text: int = .len(\"Ada\");\n",
                "    var count: int = .len(items);\n",
                "    return count;\n",
                "};\n",
            ),
        )
        .expect("Should write intrinsic length fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("Temporary intrinsic length fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for intrinsic length calls, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.matches("LengthOf").count() >= 2,
            "Lowered dump should retain dedicated LengthOf instructions for '.len', got:\n{}",
            stdout,
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_intrinsic_length_failures_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_intrinsic_length_failures");
        fs::create_dir_all(&temp_root)
            .expect("Should create temp intrinsic length failure fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "typ Flagged: rec = {\n",
                "    name: str;\n",
                "};\n",
                "fun[] main(value: Flagged): int = {\n",
                "    return .len(value);\n",
                "};\n",
            ),
        )
        .expect("Should write intrinsic length failure fixture");

        let output = run_fol(&[
            "--json",
            fixture
                .to_str()
                .expect("Temporary intrinsic length failure fixture path should be valid UTF-8"),
        ]);

        assert!(
            !output.status.success(),
            "CLI should fail for invalid intrinsic length calls",
        );

        let json = parse_cli_json(&output);
        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("CLI JSON output should expose diagnostics");
        let length_error = diagnostics.iter().find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .map(|message| {
                    message.contains(
                        ".len(...) expects one string, array, vector, sequence, set, or map operand",
                    )
                })
                .unwrap_or(false)
        });

        assert!(
            length_error.is_some(),
            "Expected intrinsic length diagnostic in CLI JSON output, got: {json}"
        );
        assert!(
            length_error
                .and_then(|diagnostic| diagnostic["location"].as_object())
                .is_some(),
            "Expected intrinsic length diagnostic to keep a structured location, got: {json}"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_dump_lowered_succeeds_for_intrinsic_echo_calls() {
        use std::fs;

        let temp_root = unique_temp_root("cli_dump_lowered_intrinsic_echo");
        fs::create_dir_all(&temp_root).expect("Should create temp intrinsic echo fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] main(flag: bol): bol = {\n",
                "    return .echo(flag);\n",
                "};\n",
            ),
        )
        .expect("Should write intrinsic echo fixture");

        let output = run_fol(&[
            "--dump-lowered",
            fixture
                .to_str()
                .expect("Temporary intrinsic echo fixture path should be valid UTF-8"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should dump lowered output for intrinsic echo calls, got status {:?} and output:\n{}",
            output.status.code(),
            stdout,
        );
        assert!(
            stdout.contains("RuntimeHook"),
            "Lowered dump should retain explicit runtime-hook instructions for '.echo', got:\n{}",
            stdout,
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_intrinsic_echo_failures_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_intrinsic_echo_failures");
        fs::create_dir_all(&temp_root).expect("Should create temp intrinsic echo failure fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!("fun[] main(): int = {\n", "    return .echo();\n", "};\n",),
        )
        .expect("Should write intrinsic echo failure fixture");

        let output = run_fol(&[
            "--json",
            fixture
                .to_str()
                .expect("Temporary intrinsic echo failure fixture path should be valid UTF-8"),
        ]);

        assert!(
            !output.status.success(),
            "CLI should fail for invalid intrinsic echo calls",
        );

        let json = parse_cli_json(&output);
        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("CLI JSON output should expose diagnostics");
        let echo_error = diagnostics.iter().find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .map(|message| {
                    message.contains(".echo(...) expects exactly 1 argument(s) but got 0")
                })
                .unwrap_or(false)
        });

        assert!(
            echo_error.is_some(),
            "Expected intrinsic echo diagnostic in CLI JSON output, got: {json}"
        );
        assert!(
            echo_error
                .and_then(|diagnostic| diagnostic["location"].as_object())
                .is_some(),
            "Expected intrinsic echo diagnostic to keep a structured location, got: {json}"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_v3_intrinsic_boundaries_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_intrinsic_v3_boundaries");
        fs::create_dir_all(&temp_root).expect("Should create temp intrinsic V3 boundary fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "fun[] main(value: int): int = {\n",
                "    return .de_alloc(value);\n",
                "};\n",
            ),
        )
        .expect("Should write intrinsic V3 boundary fixture");

        let output = run_fol(&[
            "--json",
            fixture
                .to_str()
                .expect("Temporary intrinsic V3 boundary fixture path should be valid UTF-8"),
        ]);

        assert!(
            !output.status.success(),
            "CLI should fail for V3-only intrinsic calls during the V1 milestone",
        );

        let json = parse_cli_json(&output);
        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("CLI JSON output should expose diagnostics");
        let v3_error = diagnostics.iter().find(|diagnostic| {
            diagnostic["message"]
                .as_str()
                .map(|message| {
                    message.contains(
                        ".de_alloc(...) is planned for a future release",
                    )
                })
                .unwrap_or(false)
        });

        assert!(
            v3_error.is_some(),
            "Expected explicit V3 intrinsic boundary diagnostic in CLI JSON output, got: {json}"
        );
        assert!(
            v3_error
                .and_then(|diagnostic| diagnostic["location"].as_object())
                .is_some(),
            "Expected V3 intrinsic boundary diagnostic to keep a structured location, got: {json}"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_json_cast_intrinsic_failures_keep_structured_fields() {
        use std::fs;

        let temp_root = unique_temp_root("cli_json_cast_intrinsic_failures");
        fs::create_dir_all(&temp_root).expect("Should create temp cast intrinsic failure fixture");
        let fixture = temp_root.join("main.fol");
        fs::write(
            &fixture,
            concat!(
                "var text: str = \"label\";\n",
                "var target: int = 0;\n",
                "fun[] main(value: int): int = {\n",
                "    return value as text;\n",
                "};\n",
                "fun[] side(value: int): int = {\n",
                "    return value cast target;\n",
                "};\n",
            ),
        )
        .expect("Should write cast intrinsic failure fixture");

        let output = run_fol(&[
            "--json",
            fixture
                .to_str()
                .expect("Temporary cast intrinsic fixture path should be valid UTF-8"),
        ]);

        assert!(
            !output.status.success(),
            "CLI should fail for deferred cast intrinsic surfaces during the V1 milestone",
        );

        let json = parse_cli_json(&output);
        let diagnostics = json["diagnostics"]
            .as_array()
            .expect("CLI JSON output should expose diagnostics");

        for expected in [
            "operator 'as' is not yet supported",
            "operator 'cast' is not yet supported",
        ] {
            let diagnostic = diagnostics.iter().find(|diagnostic| {
                diagnostic["message"]
                    .as_str()
                    .map(|message| message.contains(expected))
                    .unwrap_or(false)
            });

            assert!(
                diagnostic.is_some(),
                "Expected cast intrinsic diagnostic containing '{expected}', got: {json}"
            );
            assert!(
                diagnostic
                    .and_then(|diagnostic| diagnostic["location"].as_object())
                    .is_some(),
                "Expected cast intrinsic diagnostic '{expected}' to keep a structured location, got: {json}"
            );
        }

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_cli_folder_compile_succeeds_with_package_parser() {
        use std::fs;

        let temp_root = unique_temp_root("cli_folder_compile");
        fs::create_dir_all(&temp_root).expect("Should create temp CLI folder fixture");
        fs::write(temp_root.join("00_first.fol"), "var first = 1;\n")
            .expect("Should write first declaration source");
        fs::write(temp_root.join("10_second.fol"), "var second = 2;\n")
            .expect("Should write second declaration source");

        let output = run_fol(&[temp_root
            .to_str()
            .expect("CLI folder fixture path should be utf-8")]);
        let stdout = String::from_utf8_lossy(&output.stdout);

        assert!(
            output.status.success(),
            "CLI should accept declaration-only folders, got status {:?} and output:\n{}",
            output.status.code(),
            stdout
        );
        assert!(
            stdout.contains("Compilation successful"),
            "Human CLI output should still report a successful folder compile"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_entry_package_with_nested_source_namespaces_builds_and_runs() {
        use std::fs;

        // Regression: a source root that owns child namespaces emits its own
        // code into `<dir>/mod.rs`; the entry call path must not gain a
        // literal `mod` segment.
        let temp_root = unique_temp_root("nested_namespace_entry");
        fs::create_dir_all(temp_root.join("src/util"))
            .expect("Should create nested namespace fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"nested_ns\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"nested_ns\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write nested namespace build file");
        fs::write(
            temp_root.join("src/util/lib.fol"),
            "fun[exp] helper(): int = {\n    return 4;\n};\n",
        )
        .expect("Should write nested namespace helper");
        fs::write(
            temp_root.join("src/main.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .expect("Should write nested namespace entry");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "entry packages with nested source namespaces should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_source_namespaces_named_after_rust_keywords_build() {
        use std::fs;

        // Regression: FOL namespace directories may collide with Rust
        // keywords (`impl`, `mod`, `type`); backend module names escape them.
        let temp_root = unique_temp_root("keyword_namespace");
        fs::create_dir_all(temp_root.join("src/impl"))
            .expect("Should create keyword namespace fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"kw_ns\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"kw_ns\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write keyword namespace build file");
        fs::write(
            temp_root.join("src/impl/lib.fol"),
            "fun[exp] helper(): int = {\n    return 5;\n};\n",
        )
        .expect("Should write keyword namespace helper");
        fs::write(
            temp_root.join("src/main.fol"),
            "fun[] main(): int = {\n    return 0;\n};\n",
        )
        .expect("Should write keyword namespace entry");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "keyword-named source namespaces should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_constraint_calls_dispatch_per_instantiation_end_to_end() {
        use std::fs;

        // Standards-as-constraints: `measure(T: sized)` calls `thing.size()`
        // and each instantiation dispatches to its own conformer routine.
        let temp_root = unique_temp_root("constraint_dispatch");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create constraint dispatch fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"constraint_dispatch\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"constraint_dispatch\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write constraint dispatch build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "std sized: pro = {\n",
                "    fun size(): int;\n",
                "};\n",
                "typ A()(sized): rec = {\n",
                "    var a: int;\n",
                "};\n",
                "typ B()(sized): rec = {\n",
                "    var b: int;\n",
                "};\n",
                "fun (A)size(): int = {\n",
                "    return self.a;\n",
                "};\n",
                "fun (B)size(): int = {\n",
                "    return self.b;\n",
                "};\n",
                "fun measure(T: sized)(thing: T): int = {\n",
                "    return thing.size();\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    var x: A = { a = 11 };\n",
                "    var y: B = { b = 31 };\n",
                "    return measure(x) + measure(y);\n",
                "};\n",
            ),
        )
        .expect("Should write constraint dispatch source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "constraint calls should build and monomorphize per instantiation: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_constraint_calls_reach_standard_default_bodies_end_to_end() {
        use std::fs;

        let temp_root = unique_temp_root("constraint_default_body");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create constraint default fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"constraint_default_body\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"constraint_default_body\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write constraint default build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "std greet: pro = {\n",
                "    fun hello(): int = {\n",
                "        return 40;\n",
                "    };\n",
                "};\n",
                "typ A()(greet): rec = {\n",
                "    var x: int;\n",
                "};\n",
                "fun salute(T: greet)(thing: T): int = {\n",
                "    return thing.hello();\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    var a: A = { x = 0 };\n",
                "    return salute(a);\n",
                "};\n",
            ),
        )
        .expect("Should write constraint default source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "constraint calls should reach standard default bodies: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_same_named_receiver_routines_build_end_to_end() {
        use std::fs;

        // Regression: two receiver routines sharing a name (on different
        // types) previously collided during lowering symbol/body pairing.
        let temp_root = unique_temp_root("same_named_receivers");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create same-named receiver fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"same_named_receivers\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"same_named_receivers\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write same-named receiver build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "typ Left: rec = {\n",
                "    var value: int;\n",
                "};\n",
                "typ Right: rec = {\n",
                "    var item: int;\n",
                "};\n",
                "fun (Left)take(): int = {\n",
                "    return self.value;\n",
                "};\n",
                "fun (Right)take(): int = {\n",
                "    return self.item;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    var l: Left = { value = 1 };\n",
                "    var r: Right = { item = 2 };\n",
                "    return l.take() + r.take();\n",
                "};\n",
            ),
        )
        .expect("Should write same-named receiver source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "same-named receiver routines should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_default_body_with_conformer_override_builds_end_to_end() {
        use std::fs;

        // Regression: a standard default body plus a same-named conformer
        // override previously double-lowered into one routine.
        let temp_root = unique_temp_root("default_body_override");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create default override fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"default_body_override\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"default_body_override\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write default override build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "std greet: pro = {\n",
                "    fun hello(): int = {\n",
                "        return 1;\n",
                "    };\n",
                "};\n",
                "typ A()(greet): rec = {\n",
                "    var x: int;\n",
                "};\n",
                "typ B()(greet): rec = {\n",
                "    var y: int;\n",
                "};\n",
                "fun (B)hello(): int = {\n",
                "    return 2;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    var a: A = { x = 0 };\n",
                "    var b: B = { y = 0 };\n",
                "    return a.hello() + b.hello();\n",
                "};\n",
            ),
        )
        .expect("Should write default override source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "default body plus conformer override should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_nested_loc_import_of_generic_receiver_builds_and_runs() {
        use std::fs;

        // Regression chain: importing a generic receiver routine through a
        // `loc` target nested inside the package's own source tree used to
        // (a) stack-overflow typecheck import hydration on the generic
        // parameter's self-referential declared type, and then (b) emit a
        // module tree where the namespace unit's `mod.rs` lost its child
        // `pub mod` declarations.
        let temp_root = unique_temp_root("nested_loc_generic_receiver");
        fs::create_dir_all(temp_root.join("src/shared"))
            .expect("Should create nested loc fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"nested_loc_generic_receiver\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"nested_loc_generic_receiver\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write nested loc build file");
        fs::write(
            temp_root.join("src/shared/lib.fol"),
            concat!(
                "typ[exp] Box(T): rec = {\n",
                "    value: T\n",
                "};\n",
                "fun[exp] (Box[T])get(T)(): T = {\n",
                "    return self.value;\n",
                "};\n",
            ),
        )
        .expect("Should write nested loc shared source");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "use shared: loc = {\"shared\"};\n",
                "fun[] main(): int = {\n",
                "    var b: shared::Box[int] = { value = 5 };\n",
                "    return b.get();\n",
                "};\n",
            ),
        )
        .expect("Should write nested loc entry source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "nested loc generic receiver imports should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_nil_error_shell_bindings_build_end_to_end() {
        use std::fs;

        // Regression: `var maybe: err[int] = nil;` typechecked but emitted
        // `FolError::new(())`, pinning the payload to `()` instead of
        // leaving it to inference like `FolOption::nil()` does.
        let temp_root = unique_temp_root("nil_error_shell");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create nil error fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"nil_error_shell\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"nil_error_shell\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write nil error build file");
        fs::write(
            temp_root.join("src/main.fol"),
            "fun[] main(): int = {\n    var maybe: err[int] = nil;\n    return 0;\n};\n",
        )
        .expect("Should write nil error source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "nil error shells should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_hardening_round_fixes_build_end_to_end() {
        use std::fs;

        // Pins four probe-found fixes in one fixture:
        //  - var declarations inside when case bodies (block-scope recovery)
        //  - statement-position dot intrinsics (`.echo(x);`)
        //  - single-char metadata strings (name = "m")
        //  - exact-size array literals still working after arity checking
        let temp_root = unique_temp_root("hardening_round_fixes");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create hardening fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"m\", version = \"2\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"hardening_round_fixes\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write hardening build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "fun[] main(): int = {\n",
                "    var values: arr[int, 3] = {1, 2, 3};\n",
                "    .echo(.len(values));\n",
                "    when(true) {\n",
                "        case(true) {\n",
                "            var local: int = 3;\n",
                "            return local;\n",
                "        }\n",
                "        * { return 0; }\n",
                "    }\n",
                "};\n",
            ),
        )
        .expect("Should write hardening source");

        let fetch = run_fol_in_dir(&temp_root, &["pack", "fetch"]);
        assert!(fetch.status.success(), "std fetch should succeed");
        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "hardening fixture should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_exported_alias_and_intra_package_loc_import_build_end_to_end() {
        use std::fs;

        // Pins two fixes together:
        //  - `ali[exp]` parses and exports the alias across packages
        //  - a loc import pointing back inside the package resolves to the
        //    existing namespace instead of double-loading the directory
        //    (double-loading emitted two units into one backend file)
        let temp_root = unique_temp_root("exported_alias_intra_loc");
        fs::create_dir_all(temp_root.join("src")).expect("Should create alias fixture src dir");
        fs::create_dir_all(temp_root.join("units")).expect("Should create alias fixture units dir");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"exported_alias_intra_loc\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"exported_alias_intra_loc\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write alias fixture build file");
        fs::write(
            temp_root.join("units/lib.fol"),
            concat!(
                "ali[exp] Meters: int;\n",
                "\n",
                "fun[exp] double(value: Meters): Meters = {\n",
                "    return value + value;\n",
                "};\n",
            ),
        )
        .expect("Should write alias fixture library");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "use units: loc = {\"../units\"};\n",
                "\n",
                "fun[] main(): int = {\n",
                "    var distance: units::Meters = 21;\n",
                "    return units::double(distance);\n",
                "};\n",
            ),
        )
        .expect("Should write alias fixture entry");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "exported alias through an intra-package loc import should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        // Across a real package boundary (loc target outside the package
        // root) the alias is only reachable when exported.
        let outside_root = unique_temp_root("exported_alias_outside_loc");
        fs::create_dir_all(outside_root.join("app/src"))
            .expect("Should create outside-alias fixture app dir");
        fs::create_dir_all(outside_root.join("units"))
            .expect("Should create outside-alias fixture units dir");
        fs::write(
            outside_root.join("app/build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"exported_alias_outside\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"exported_alias_outside\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write outside-alias fixture build file");
        fs::write(
            outside_root.join("app/src/main.fol"),
            concat!(
                "use units: loc = {\"../../units\"};\n",
                "\n",
                "fun[] main(): int = {\n",
                "    var distance: units::Meters = 21;\n",
                "    return units::double(distance);\n",
                "};\n",
            ),
        )
        .expect("Should write outside-alias fixture entry");
        fs::write(
            outside_root.join("units/lib.fol"),
            concat!(
                "ali[exp] Meters: int;\n",
                "\n",
                "fun[exp] double(value: Meters): Meters = {\n",
                "    return value + value;\n",
                "};\n",
            ),
        )
        .expect("Should write outside-alias fixture library");

        let exported = run_fol_in_dir(&outside_root.join("app"), &["code", "check"]);
        assert!(
            exported.status.success(),
            "exported aliases should resolve across package boundaries: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&exported.stdout),
            String::from_utf8_lossy(&exported.stderr)
        );

        fs::write(
            outside_root.join("units/lib.fol"),
            concat!(
                "ali Meters: int;\n",
                "\n",
                "fun[exp] double(value: Meters): Meters = {\n",
                "    return value + value;\n",
                "};\n",
            ),
        )
        .expect("Should rewrite outside-alias fixture library without export");
        let hidden = run_fol_in_dir(&outside_root.join("app"), &["code", "check"]);
        assert!(
            !hidden.status.success(),
            "non-exported aliases should not resolve across package boundaries"
        );

        fs::remove_dir_all(&temp_root).ok();
        fs::remove_dir_all(&outside_root).ok();
    }

    #[test]
    fn test_check_enforces_declared_capability_model() {
        use std::fs;

        // `fol code check` must typecheck under the member's declared
        // capability model, not silently under the hosted model — a core
        // package using `str` has to fail at check time, not first at build.
        let temp_root = unique_temp_root("check_capability_model");
        fs::create_dir_all(temp_root.join("src")).expect("Should create check-model fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"check_capability_model\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"check_capability_model\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write check-model build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "fun[] main(): int = {\n",
                "    var name: str = \"hello\";\n",
                "    return 0;\n",
                "};\n",
            ),
        )
        .expect("Should write check-model source");

        let output = run_fol_in_dir(&temp_root, &["code", "check"]);
        assert!(
            !output.status.success(),
            "core packages using str should fail check"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("str requires heap support"),
            "check should surface the capability diagnostic: {stderr}"
        );

        // The same source under memo checks clean.
        let build = fs::read_to_string(temp_root.join("build.fol"))
            .expect("Should reread check-model build file")
            .replace("fol_model = \"core\"", "fol_model = \"memo\"");
        fs::write(temp_root.join("build.fol"), build)
            .expect("Should rewrite check-model build file");
        let output = run_fol_in_dir(&temp_root, &["code", "check"]);
        assert!(
            output.status.success(),
            "memo packages using str should check clean: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_variadic_marker_and_real_seq_params_stay_distinct() {
        use std::fs;

        // Variadic collection is only the explicit `... T` marker; a trailing
        // `seq[T]` parameter takes a real sequence value. Both forms must
        // build side by side in one package.
        let temp_root = unique_temp_root("variadic_vs_seq_params");
        fs::create_dir_all(temp_root.join("src")).expect("Should create variadic fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"variadic_vs_seq_params\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"variadic_vs_seq_params\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write variadic fixture build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "fun[] take(values: seq[int]): int = {\n",
                "    return .len(values);\n",
                "};\n",
                "\n",
                "fun[] spread(extras: ... int): int = {\n",
                "    return .len(extras);\n",
                "};\n",
                "\n",
                "fun[] main(): int = {\n",
                "    var items: seq[int] = {1, 2, 3};\n",
                "    var a: int = take(items);\n",
                "    var b: int = spread(1, 2, 3);\n",
                "    return a + b;\n",
                "};\n",
            ),
        )
        .expect("Should write variadic fixture source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "variadic marker and seq params should coexist: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        // Passing loose values to a real seq parameter must fail.
        let source = fs::read_to_string(temp_root.join("src/main.fol"))
            .expect("Should reread variadic fixture source")
            .replace("take(items)", "take(1, 2)");
        fs::write(temp_root.join("src/main.fol"), source)
            .expect("Should rewrite variadic fixture source");
        let output = run_fol_in_dir(&temp_root, &["code", "check"]);
        assert!(
            !output.status.success(),
            "loose arguments to a seq parameter should fail typecheck"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_field_assignment_into_mutable_record_builds() {
        use std::fs;

        // Book contract (structs chapter, "Accessing"): assigning into a field of
        // a mutable record instance must typecheck, lower, and build end to end.
        let temp_root = unique_temp_root("field_assignment");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create field-assignment fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"field_assign\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"field_assign\", root = \"src/main.fol\", fol_model = \"core\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write field-assignment build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "typ Counter: rec = {\n",
                "    total: int\n",
                "};\n",
                "\n",
                "fun[] main(): int = {\n",
                "    var[mut] counter: Counter = { total = 1 };\n",
                "    counter.total = 5;\n",
                "    return counter.total;\n",
                "};\n",
            ),
        )
        .expect("Should write field-assignment entry");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "field assignment into a mutable record should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_records_with_container_fields_build_and_run() {
        use std::fs;

        // Record fields render through FolEchoFormat, so container-typed
        // fields (which have no Display impl) must not break emission.
        let temp_root = unique_temp_root("record_container_fields");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create container-field fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"record_container_fields\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"record_container_fields\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write container-field build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "use std: pkg = {\"std\"};\n",
                "\n",
                "typ Bag: rec = {\n",
                "    items: seq[int];\n",
                "    labels: map[str, int]\n",
                "};\n",
                "\n",
                "fun[] main(): int = {\n",
                "    var bag: Bag = { items = {1, 2, 3}, labels = {{\"a\", 1}} };\n",
                "    return std::io::echo_int(.len(bag.items));\n",
                "};\n",
            ),
        )
        .expect("Should write container-field source");

        let fetch = run_fol_in_dir(&temp_root, &["pack", "fetch"]);
        assert!(fetch.status.success(), "std fetch should succeed");
        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "records with container fields should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_generic_variadic_routine_call_builds_end_to_end() {
        use std::fs;

        // P1 regression (hardening round 5): a generic routine with a variadic
        // parameter (`collect(T)(head: T, extras: ... T): T`) used to leak its
        // generic parameter into the caller's variadic pack (`FolSeq<t>` with
        // `t` unbound), so `code check` passed but `code build` emitted broken
        // Rust (E0425). The pack is now typed from its concrete elements.
        let temp_root = unique_temp_root("generic_variadic_build");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create generic variadic fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"gv\", version = \"0.1.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"gv\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write generic variadic build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "use std: pkg = {\"std\"};\n",
                "fun collect(T)(head: T, extras: ... T): T = { return head; };\n",
                "fun[] main(): int = { return std::io::echo_int(collect(1, 2, 3)); };\n",
            ),
        )
        .expect("Should write generic variadic source");

        let fetch = run_fol_in_dir(&temp_root, &["pack", "fetch"]);
        assert!(fetch.status.success(), "std fetch should succeed");
        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "generic variadic routine should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_generic_type_on_own_type_param_builds_end_to_end() {
        use std::fs;

        // P2 regression (hardening round 5): a generic routine that instantiates
        // a generic named type on its own type parameter (`var b: Box[T]` inside
        // `wrap(T)`) used to pass `code check` but fail `code build` with
        // "named lowered runtime type ... does not map to any lowered type
        // declaration". The routine is now monomorphized so `Box[int]` is
        // concrete and its structural decl is synthesized before backend.
        let temp_root = unique_temp_root("generic_type_own_param_build");
        fs::create_dir_all(temp_root.join("src"))
            .expect("Should create generic type param fixture dirs");
        fs::write(
            temp_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"gt\", version = \"0.1.0\" });\n",
                "    var graph = build.graph();\n",
                "    var app = graph.add_exe({ name = \"gt\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("Should write generic type param build file");
        fs::write(
            temp_root.join("src/main.fol"),
            concat!(
                "typ Box(T): rec = { value: T };\n",
                "fun wrap(T)(v: T): T = {\n",
                "    var b: Box[T] = { value = v };\n",
                "    return b.value;\n",
                "};\n",
                "fun[] main(): int = { return wrap(7); };\n",
            ),
        )
        .expect("Should write generic type param source");

        let output = run_fol_in_dir(&temp_root, &["code", "build"]);
        assert!(
            output.status.success(),
            "generic type on own type param should build: stdout=\n{}\nstderr=\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        fs::remove_dir_all(&temp_root).ok();
    }


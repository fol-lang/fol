use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_apps_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

fn fixture_root(name: &str) -> PathBuf {
    Path::new("test/apps/fixtures").join(name)
}

fn formal_pkg_build(name: &str) -> String {
    format!(
        concat!(
            "pro[] build(): non = {{\n",
            "    var build = .build();\n",
            "    build.meta({{\n",
            "        name = \"{name}\",\n",
            "        version = \"0.1.0\",\n",
            "    }});\n",
            "}};\n",
        ),
        name = name
    )
}

fn run_fol(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_fol"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("should run fol CLI")
}

fn compile_app(entry: &Path) -> std::process::Output {
    run_fol(&[entry.to_str().expect("fixture path should be valid utf-8")])
}

fn bundled_std_store_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lang/library")
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("copy root should be creatable");
    for entry in fs::read_dir(src).expect("copy source should be readable") {
        let entry = entry.expect("copy entry should be readable");
        let file_type = entry.file_type().expect("copy type should be readable");
        let to = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).expect("copy entry should succeed");
        }
    }
}

fn compile_app_keep_build_dir(entry: &Path) -> std::process::Output {
    run_fol(&[
        "--keep-build-dir",
        entry.to_str().expect("fixture path should be valid utf-8"),
    ])
}

fn compile_app_with_roots(
    entry: &Path,
    std_root: Option<&Path>,
    package_store_root: Option<&Path>,
    keep_build_dir: bool,
) -> std::process::Output {
    let mut args = Vec::new();
    if keep_build_dir {
        args.push("--keep-build-dir".to_string());
    }
    if let Some(std_root) = std_root {
        args.push("--std-root".to_string());
        args.push(
            std_root
                .to_str()
                .expect("std root should be valid utf-8")
                .to_string(),
        );
    }
    if let Some(package_store_root) = package_store_root {
        args.push("--package-store-root".to_string());
        args.push(
            package_store_root
                .to_str()
                .expect("package store root should be valid utf-8")
                .to_string(),
        );
    }
    args.push(
        entry
            .to_str()
            .expect("fixture path should be valid utf-8")
            .to_string(),
    )
    ;
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_fol(&arg_refs)
}

fn compile_app_expect_success(entry: &Path) -> std::process::Output {
    let output = compile_app(entry);
    assert!(
        output.status.success(),
        "expected app compile success\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn compile_app_keep_build_dir_expect_success(entry: &Path) -> std::process::Output {
    let output = compile_app_keep_build_dir(entry);
    assert!(
        output.status.success(),
        "expected kept-build-dir app compile success\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn compile_app_expect_failure(entry: &Path) -> std::process::Output {
    let output = compile_app(entry);
    assert!(
        !output.status.success(),
        "expected app compile failure\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn compile_app_with_roots_expect_success(
    entry: &Path,
    std_root: Option<&Path>,
    package_store_root: Option<&Path>,
) -> std::process::Output {
    let output = compile_app_with_roots(entry, std_root, package_store_root, false);
    assert!(
        output.status.success(),
        "expected rooted app compile success\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn compile_app_with_roots_keep_build_dir_expect_success(
    entry: &Path,
    std_root: Option<&Path>,
    package_store_root: Option<&Path>,
) -> std::process::Output {
    let output = compile_app_with_roots(entry, std_root, package_store_root, true);
    assert!(
        output.status.success(),
        "expected rooted kept-build-dir app compile success\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn built_binary_path(output: &std::process::Output) -> PathBuf {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let marker = "binary=";
    let binary = stdout
        .lines()
        .find_map(|line| line.split(marker).nth(1))
        .expect("compile success should report a binary path")
        .trim();
    PathBuf::from(binary)
}

fn emitted_crate_root(output: &std::process::Output) -> PathBuf {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let marker = "crate_root=";
    let root = stdout
        .lines()
        .find_map(|line| line.split(marker).nth(1))
        .and_then(|tail| tail.split(" binary=").next())
        .expect("compile success should report a crate root")
        .trim();
    PathBuf::from(root)
}

fn compile_and_run_app(entry: &Path) -> std::process::Output {
    let compile_output = compile_app_expect_success(entry);
    let binary = built_binary_path(&compile_output);
    Command::new(&binary)
        .output()
        .expect("should run compiled app binary")
}

fn assert_output_contains(output: &std::process::Output, needle: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains(needle) || stderr.contains(needle),
        "expected output to contain '{}'\nstdout=\n{}\nstderr=\n{}",
        needle,
        stdout,
        stderr
    );
}

fn assert_exit_code(output: &std::process::Output, expected: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "expected exit code {}\nstdout=\n{}\nstderr=\n{}",
        expected,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_artifact_paths_exist(output: &std::process::Output) {
    let root = emitted_crate_root(output);
    let binary = built_binary_path(output);
    assert!(
        root.exists(),
        "reported crate root should exist at '{}'",
        root.display()
    );
    assert!(
        binary.exists(),
        "reported binary should exist at '{}'",
        binary.display()
    );
}

fn assert_stdout_order(output: &std::process::Output, needles: &[&str], label: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut cursor = 0usize;

    for needle in needles {
        let Some(offset) = stdout[cursor..].find(needle) else {
            panic!(
                "expected stdout to contain '{needle}' in order for {label}\nstdout=\n{}",
                stdout
            );
        };
        cursor += offset + needle.len();
    }
}

#[test]
fn app_fixture_tree_exists() {
    let root = Path::new("test/apps");
    let fixtures = root.join("fixtures");

    assert!(root.exists(), "app test root should exist");
    assert!(
        fixtures.exists(),
        "app fixture root should exist at '{}'",
        fixtures.display()
    );
}

#[test]
fn full_v1_showcase_example_compiles_and_runs() {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test/apps/showcases/full_v1_showcase/app");

    let compile_output = compile_app_keep_build_dir_expect_success(&entry);
    assert_artifact_paths_exist(&compile_output);

    let run_output = Command::new(built_binary_path(&compile_output))
        .output()
        .expect("should run full v1 showcase binary");
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "7");
}

#[test]
fn call_binding_v1_showcase_example_compiles_and_runs() {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test/apps/showcases/call_binding_v1_showcase/app");

    let compile_output = compile_app_keep_build_dir_expect_success(&entry);
    assert_artifact_paths_exist(&compile_output);

    let run_output = Command::new(built_binary_path(&compile_output))
        .output()
        .expect("should run call binding v1 showcase binary");
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "33");
}

#[test]
fn call_binding_stress_fixture_compiles_and_runs() {
    let fixture = fixture_root("call_binding_stress");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "50");
}

#[test]
fn method_call_binding_stress_fixture_compiles_and_runs() {
    let fixture = fixture_root("method_call_binding_stress");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "47");
}

#[test]
fn generic_type_exec_fixture_compiles_and_runs() {
    let fixture = fixture_root("generic_type_exec");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "42");
}

#[test]
fn generic_standard_constraint_exec_fixture_compiles_and_runs() {
    let fixture = fixture_root("generic_standard_constraint_exec");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "1");
}

#[test]
fn dfr_v1_showcase_example_compiles_and_runs() {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test/apps/showcases/dfr_v1_showcase/app");

    let compile_output = compile_app_keep_build_dir_expect_success(&entry);
    assert_artifact_paths_exist(&compile_output);

    let run_output = Command::new(built_binary_path(&compile_output))
        .output()
        .expect("should run dfr v1 showcase binary");
    assert_exit_code(&run_output, 0);
    assert_stdout_order(&run_output, &["30", "20", "40", "10", "60", "70", "50"], "dfr v1 showcase");
}

#[test]
fn app_harness_compile_only_helper_handles_success_and_failure() {
    let temp_root = unique_temp_root("compile_only");
    let good_root = temp_root.join("good");
    let bad_root = temp_root.join("bad");

    fs::create_dir_all(&good_root).expect("good root");
    fs::create_dir_all(&bad_root).expect("bad root");
    fs::write(
        good_root.join("main.fol"),
        "fun[] main(): int = {\n    return 7;\n};\n",
    )
    .expect("good source");
    fs::write(
        bad_root.join("main.fol"),
        "fun[] main(): int = {\n    return \"bad\";\n};\n",
    )
    .expect("bad source");

    compile_app_expect_success(&good_root);
    compile_app_expect_failure(&bad_root);

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn app_harness_run_helper_executes_built_binary() {
    let temp_root = unique_temp_root("run_helper");
    fs::create_dir_all(&temp_root).expect("run root");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    return 5;\n};\n",
    )
    .expect("run source");

    let output = compile_and_run_app(&temp_root);

    assert!(
        output.status.success(),
        "compiled app binary should run successfully"
    );

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn v3_container_observations_compile_and_reuse_move_only_locals() {
    let temp_root = unique_temp_root("v3_container_observations");
    fs::create_dir_all(&temp_root).expect("observation fixture root");
    fs::write(
        temp_root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"v3_container_observations\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"main.fol\", fol_model = \"memo\" });\n",
            "    graph.install(app);\n",
            "};\n",
        ),
    )
    .expect("observation fixture build");
    fs::write(
        temp_root.join("main.fol"),
        concat!(
            "fun[] read(pointer: ptr[int]): int = { return *pointer; };\n",
            "fun[] main(): int = {\n",
            "    var stored_value: int = 7;\n",
            "    var query_value: int = 7;\n",
            "    var stored: ptr[int] = &stored_value;\n",
            "    var query: ptr[int] = &query_value;\n",
            "    var values: map[ptr[int], int] = {{stored, 3}};\n",
            "    var found: int = values[query];\n",
            "    @var heap_values: vec[int] = {4, 5};\n",
            "    var heap_len: int = .len(heap_values);\n",
            "    var heap_head: int = heap_values[0];\n",
            "    var heap_tail_len: int = .len(heap_values[1:]);\n",
            "    var flags: set[int] = {9};\n",
            "    var first_flag: int = flags[0];\n",
            "    var flag_sum: int = 0;\n",
            "    for (flag in flags) {\n",
            "        flag_sum = flag_sum + flag;\n",
            "    }\n",
            "    var gated: int = 0;\n",
            "    when(false) {\n",
            "        * { gated = 100; }\n",
            "    }\n",
            "    when(true) {\n",
            "        * { gated = 2; }\n",
            "    }\n",
            "    var first_replace: int = 1;\n",
            "    var second_replace: int = 2;\n",
            "    var[mut] replace: ptr[int] = &first_replace;\n",
            "    var old_replace: int = read(replace);\n",
            "    when(true) {\n",
            "        case(true) { replace = &second_replace; }\n",
            "        * { replace = &first_replace; }\n",
            "    }\n",
            "    replace = replace;\n",
            "    when(found + *query + .len(values) + heap_len + heap_head + heap_tail_len + .len(heap_values) + first_flag + flag_sum + .len(flags) + gated + old_replace + *replace) {\n",
            "        case(44) { return 0; }\n",
            "        * { panic \"ownership observation failed\"; }\n",
            "    }\n",
            "};\n",
        ),
    )
    .expect("observation fixture source");

    let compile_output = compile_app_keep_build_dir_expect_success(&temp_root);
    assert_artifact_paths_exist(&compile_output);
    let run_output = compile_and_run_app(&temp_root);
    assert_exit_code(&run_output, 0);
    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn v3_moved_records_reinitialize_from_record_literals() {
    let temp_root = unique_temp_root("v3_record_reinitialization");
    fs::create_dir_all(&temp_root).expect("record reinitialization fixture root");
    fs::write(
        temp_root.join("main.fol"),
        concat!(
            "typ Holder: rec = { link: ptr[int], count: int };\n",
            "fun[] consume(holder: Holder): int = { return holder.count; };\n",
            "fun[] main(): int = {\n",
            "    var first: int = 1;\n",
            "    var second: int = 2;\n",
            "    var[mut] holder: Holder = { link = &first, count = 10 };\n",
            "    var old_count: int = consume(holder);\n",
            "    holder = { link = &second, count = 20 };\n",
            "    var new_count: int = holder.count;\n",
            "    var new_link: ptr[int] = holder.link;\n",
            "    when(old_count + new_count + *new_link) {\n",
            "        case(32) { return 0; }\n",
            "        * { panic \"record reinitialization failed\"; }\n",
            "    }\n",
            "};\n",
        ),
    )
    .expect("record reinitialization fixture source");

    let compile_output = compile_app_keep_build_dir_expect_success(&temp_root);
    assert_artifact_paths_exist(&compile_output);
    let run_output = compile_and_run_app(&temp_root);
    assert_exit_code(&run_output, 0);
    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn void_anonymous_invokes_execute_as_statements() {
    let temp_root = unique_temp_root("void_anonymous_invoke");
    fs::create_dir_all(&temp_root).expect("void invoke fixture root");
    fs::write(
        temp_root.join("main.fol"),
        concat!(
            "fun[] factory(): {fun (ignored: int): non} = {\n",
            "    return pro[] (ignored: int) = { return; };\n",
            "};\n",
            "fun[] main(): int = {\n",
            "    (factory())(0);\n",
            "    return 0;\n",
            "};\n",
        ),
    )
    .expect("void invoke fixture source");

    let compile_output = compile_app_keep_build_dir_expect_success(&temp_root);
    assert_artifact_paths_exist(&compile_output);
    let run_output = compile_and_run_app(&temp_root);
    assert_exit_code(&run_output, 0);
    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn app_harness_root_helpers_support_std_and_pkg_layouts() {
    let temp_root = unique_temp_root("root_helpers");
    let app_root = temp_root.join("app");
    let pkg_root = temp_root.join("pkg");
    let math_root = pkg_root.join("math");

    fs::create_dir_all(&app_root).expect("app root");
    fs::create_dir_all(math_root.join("src")).expect("pkg src");
    fs::write(
        app_root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"main.fol\" });\n",
            "    graph.install(app);\n",
            "};\n",
        ),
    )
    .expect("app build");

    fs::write(
        app_root.join("main.fol"),
        concat!(
            "use std: pkg = {\"std\"};\n",
            "use math: pkg = {\"math\"};\n",
            "fun[] main(): int = {\n",
            "    return std::fmt::answer();\n",
            "};\n",
        ),
    )
    .expect("app source");
    fs::write(math_root.join("build.fol"), "name: math\nversion: 0.1.0\n")
        .expect("pkg manifest");
    fs::write(
        math_root.join("build.fol"),
        formal_pkg_build("math"),
    )
    .expect("pkg build");
    fs::write(math_root.join("src").join("lib.fol"), "var[exp] pkg_answer: int = 4;\n")
        .expect("pkg source");

    let merged_store_root = temp_root.join("store");
    copy_dir_all(&bundled_std_store_root().join("std"), &merged_store_root.join("std"));
    copy_dir_all(&math_root, &merged_store_root.join("math"));
    compile_app_with_roots_expect_success(&app_root, None, Some(&merged_store_root));

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn app_harness_assertion_helpers_cover_artifacts_and_status() {
    let temp_root = unique_temp_root("assertions");
    let ok_root = temp_root.join("ok");
    let bad_root = temp_root.join("bad");

    fs::create_dir_all(&ok_root).expect("ok root");
    fs::create_dir_all(&bad_root).expect("bad root");
    fs::write(
        ok_root.join("main.fol"),
        "fun[] main(): int = {\n    return 0;\n};\n",
    )
    .expect("ok source");
    fs::write(
        bad_root.join("main.fol"),
        "fun[] main(): int = {\n    return unknown_name;\n};\n",
    )
    .expect("bad source");

    let compile_output = compile_app_keep_build_dir_expect_success(&ok_root);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&ok_root);
    assert_exit_code(&run_output, 0);

    let failure_output = compile_app_expect_failure(&bad_root);
    assert_output_contains(&failure_output, "could not resolve");

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn scalar_entry_fixture_compiles_and_runs() {
    let fixture = fixture_root("scalar_entry");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn bindings_and_calls_fixture_compiles_and_runs() {
    let fixture = fixture_root("bindings_and_calls");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn control_when_fixture_compiles_and_runs() {
    let fixture = fixture_root("control_when");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn control_loop_break_fixture_compiles_and_runs() {
    let fixture = fixture_root("control_loop_break");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn control_iteration_fixture_compiles_and_runs() {
    let fixture = fixture_root("control_iteration");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn control_sibling_iterations_use_their_exact_loop_scopes() {
    let fixture = fixture_root("control_sibling_iterations");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn control_sibling_when_bodies_use_their_exact_scopes() {
    let fixture = fixture_root("control_sibling_when_scopes");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn procedure_call_fixture_compiles_and_runs() {
    let fixture = fixture_root("procedure_call");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn procedure_method_call_fixture_compiles_and_runs() {
    let fixture = fixture_root("procedure_method_call");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn same_folder_shared_scope_fixture_compiles_and_runs() {
    let fixture = fixture_root("same_folder_shared_scope");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn same_folder_hidden_visibility_fixture_fails_cleanly() {
    let fixture = fixture_root("same_folder_hidden_visibility");
    let output = compile_app_expect_failure(&fixture);

    assert_output_contains(&output, "could not resolve");
}

#[test]
fn subfolder_namespace_fixture_compiles_and_runs() {
    let fixture = fixture_root("subfolder_namespace");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn deep_namespace_chain_fixture_compiles_and_runs() {
    let fixture = fixture_root("deep_namespace_chain");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn loc_plain_values_fixture_compiles_and_runs() {
    let fixture = fixture_root("loc_plain_values").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn loc_types_and_records_fixture_compiles_and_runs() {
    let fixture = fixture_root("loc_types_and_records").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn loc_methods_fixture_compiles_and_runs() {
    let fixture = fixture_root("loc_methods").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn loc_recoverable_calls_fixture_compiles_and_runs() {
    let fixture = fixture_root("loc_recoverable_calls").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn std_basic_import_fixture_compiles_and_runs() {
    let root = fixture_root("std_basic_import");
    let app_root = root.join("app");

    let compile_output = compile_app_with_roots_keep_build_dir_expect_success(
        &app_root,
        None,
        Some(&bundled_std_store_root()),
    );
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled std fixture");
    assert_exit_code(&run_output, 0);
}

#[test]
fn std_namespace_import_fixture_compiles_and_runs() {
    let root = fixture_root("std_namespace_import");
    let app_root = root.join("app");

    let compile_output = compile_app_with_roots_keep_build_dir_expect_success(
        &app_root,
        None,
        Some(&bundled_std_store_root()),
    );
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled std namespace fixture");
    assert_exit_code(&run_output, 0);
}

#[test]
fn std_bundled_fmt_example_compiles_and_runs() {
    let fixture = PathBuf::from("examples/std_bundled_fmt");

    let compile_output = compile_app_with_roots_keep_build_dir_expect_success(
        &fixture,
        None,
        Some(&bundled_std_store_root()),
    );
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled bundled std fmt example");
    assert_exit_code(&run_output, 0);
}

#[test]
fn std_bundled_io_example_compiles_and_runs() {
    let fixture = PathBuf::from("examples/std_bundled_io");

    let compile_output = compile_app_with_roots_keep_build_dir_expect_success(
        &fixture,
        None,
        Some(&bundled_std_store_root()),
    );
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled bundled std io example");
    assert_exit_code(&run_output, 0);
}

#[test]
fn std_explicit_pkg_example_compiles_and_runs() {
    let fixture = PathBuf::from("examples/std_explicit_pkg");
    let compile_output =
        compile_app_with_roots_keep_build_dir_expect_success(&fixture, None, Some(&bundled_std_store_root()));
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled explicit std pkg example");
    assert_exit_code(&run_output, 0);
}

#[test]
fn pkg_basic_import_fixture_compiles_and_runs() {
    let root = fixture_root("pkg_basic_import");
    let app_root = root.join("app");
    let pkg_root = root.join("pkg");

    let compile_output =
        compile_app_with_roots_keep_build_dir_expect_success(&app_root, None, Some(&pkg_root));
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled pkg fixture");
    assert_exit_code(&run_output, 0);
}

#[test]
fn pkg_transitive_import_fixture_compiles_and_runs() {
    let root = fixture_root("pkg_transitive_import");
    let app_root = root.join("app");
    let pkg_root = root.join("pkg");

    let compile_output =
        compile_app_with_roots_keep_build_dir_expect_success(&app_root, None, Some(&pkg_root));
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled transitive pkg fixture");
    assert_exit_code(&run_output, 0);
}

#[test]
fn mixed_loc_std_pkg_fixture_compiles_and_runs() {
    let root = fixture_root("mixed_loc_std_pkg");
    let app_root = root.join("app");
    let merged_store_root = unique_temp_root("mixed_loc_std_pkg_store");
    copy_dir_all(&bundled_std_store_root().join("std"), &merged_store_root.join("std"));
    copy_dir_all(&root.join("pkg/math"), &merged_store_root.join("math"));

    let compile_output = compile_app_with_roots_keep_build_dir_expect_success(
        &app_root,
        None,
        Some(&merged_store_root),
    );
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let run_output = Command::new(&binary)
        .output()
        .expect("should run compiled mixed import fixture");
    assert_exit_code(&run_output, 0);
    fs::remove_dir_all(&merged_store_root).ok();
}

#[test]
fn record_flow_fixture_compiles_and_runs() {
    let fixture = fixture_root("record_flow");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn entry_flow_fixture_compiles_and_runs() {
    let fixture = fixture_root("entry_flow");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn alias_flow_fixture_compiles_and_runs() {
    let fixture = fixture_root("alias_flow");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn method_flow_fixture_compiles_and_runs() {
    let fixture = fixture_root("method_flow");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn call_niceties_fixture_compiles_and_runs() {
    let fixture = fixture_root("call_niceties");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "19");
}

#[test]
fn method_call_niceties_fixture_compiles_and_runs() {
    let fixture = fixture_root("method_call_niceties");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "19");
}

#[test]
fn dfr_scope_exit_fixture_compiles_and_runs_in_reverse_order() {
    let fixture = fixture_root("dfr_scope_exit");
    let run_output = compile_and_run_app(&fixture);
    let stdout = String::from_utf8_lossy(&run_output.stdout);

    let seven = stdout.find("7").expect("program should print body output first");
    let two = stdout.find("2").expect("program should print inner dfr output");
    let one = stdout.find("1").expect("program should print outer dfr output");

    assert!(
        seven < two && two < one,
        "expected body output before reverse-order defers\nstdout=\n{}",
        stdout
    );
}

#[test]
fn dfr_nested_scopes_fixture_compiles_and_runs() {
    let fixture = fixture_root("dfr_nested_scopes");
    let run_output = compile_and_run_app(&fixture);

    assert_exit_code(&run_output, 0);
    assert_stdout_order(&run_output, &["3", "2", "7", "1"], "nested dfr scopes");
}

#[test]
fn dfr_loop_break_fixture_compiles_and_runs() {
    let fixture = fixture_root("dfr_loop_break");
    let run_output = compile_and_run_app(&fixture);

    assert_exit_code(&run_output, 0);
    assert_stdout_order(&run_output, &["3", "2", "7", "1"], "dfr loop break");
}

#[test]
fn dfr_report_cleanup_fixture_compiles_and_runs() {
    let fixture = fixture_root("dfr_report_cleanup");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert!(
        !run_output.status.success(),
        "reported dfr cleanup fixture should fail at process boundary\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_stdout_order(&run_output, &["1"], "dfr report cleanup");
    assert_output_contains(&run_output, "main-bad");
}

#[test]
fn dfr_panic_cleanup_fixture_compiles_and_runs() {
    let fixture = fixture_root("dfr_panic_cleanup");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);
    let panic_output = Command::new(&binary)
        .output()
        .expect("should run dfr panic cleanup fixture");
    assert!(
        !panic_output.status.success(),
        "dfr panic cleanup fixture should fail\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&panic_output.stdout),
        String::from_utf8_lossy(&panic_output.stderr)
    );
    assert_stdout_order(&panic_output, &["1"], "dfr panic cleanup");
    assert_output_contains(&panic_output, "panic-bad");
}

#[test]
fn loc_call_niceties_fixture_compiles_and_runs() {
    let fixture = fixture_root("loc_call_niceties").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "28");
}

#[test]
fn loc_call_binding_stress_fixture_compiles_and_runs() {
    let fixture = fixture_root("loc_call_binding_stress").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "55");
}


#[test]
fn container_linear_fixture_compiles_and_runs() {
    let fixture = fixture_root("container_linear");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn container_slice_fixture_compiles_and_runs() {
    let fixture = fixture_root("container_slice");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn container_map_set_fixture_compiles_and_runs() {
    let fixture = fixture_root("container_map_set");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn container_cross_package_fixture_compiles_and_runs() {
    let fixture = fixture_root("container_cross_package").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn intrinsics_comparison_fixture_compiles_and_runs() {
    let fixture = fixture_root("intrinsics_comparison");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn intrinsics_not_len_echo_fixture_compiles_and_runs() {
    let fixture = fixture_root("intrinsics_not_len_echo");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
    assert_output_contains(&run_output, "2");
}

#[test]
fn intrinsics_panic_check_fixture_compiles_and_runs() {
    let fixture = fixture_root("intrinsics_panic_check");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);

    let panic_output = Command::new(&binary)
        .output()
        .expect("should run panicking intrinsic fixture");
    assert!(
        !panic_output.status.success(),
        "panic fixture should fail\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&panic_output.stdout),
        String::from_utf8_lossy(&panic_output.stderr)
    );
    assert_output_contains(&panic_output, "panic");
}

#[test]
fn recoverable_explicit_report_fixture_compiles_and_runs() {
    let fixture = fixture_root("recoverable_report");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert!(
        !run_output.status.success(),
        "explicitly reported recoverable error should fail at process boundary\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
    assert_output_contains(&run_output, "main-bad-input");
}

#[test]
fn recoverable_check_fixture_compiles_and_runs() {
    let fixture = fixture_root("recoverable_check");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);

    let ok_output = Command::new(&binary)
        .arg("false")
        .output()
        .expect("should run recoverable check success path");
    assert_exit_code(&ok_output, 0);

    let err_output = Command::new(&binary)
        .arg("true")
        .output()
        .expect("should run recoverable check failure path");
    assert_exit_code(&err_output, 0);
}

#[test]
fn recoverable_fallback_fixture_compiles_and_runs() {
    let fixture = fixture_root("recoverable_fallback");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);

    let ok_output = Command::new(&binary)
        .arg("false")
        .output()
        .expect("should run recoverable fallback success path");
    assert_exit_code(&ok_output, 0);

    let err_output = Command::new(&binary)
        .arg("true")
        .output()
        .expect("should run recoverable fallback error path");
    assert_exit_code(&err_output, 0);
}

#[test]
fn recoverable_package_boundary_fixture_compiles_and_runs() {
    let fixture = fixture_root("recoverable_package_boundary").join("app");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);

    let ok_output = Command::new(&binary)
        .arg("false")
        .output()
        .expect("should run package recoverable success path");
    assert_exit_code(&ok_output, 0);

    let err_output = Command::new(&binary)
        .arg("true")
        .output()
        .expect("should run package recoverable fallback path");
    assert_exit_code(&err_output, 0);
}

#[test]
fn shell_optional_fixture_compiles_and_runs() {
    let fixture = fixture_root("shell_optional");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);

    let ok_output = Command::new(&binary)
        .arg("true")
        .output()
        .expect("should run optional shell success path");
    assert_exit_code(&ok_output, 0);

    let nil_output = Command::new(&binary)
        .arg("false")
        .output()
        .expect("should run optional shell nil path");
    assert_exit_code(&nil_output, 0);
}

#[test]
fn shell_error_fixture_compiles_and_runs() {
    let fixture = fixture_root("shell_error");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn shell_vs_recoverable_boundary_fixture_compiles_and_runs() {
    let fixture = fixture_root("shell_vs_recoverable_boundary");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let binary = built_binary_path(&compile_output);

    let ok_output = Command::new(&binary)
        .arg("false")
        .output()
        .expect("should run shell-vs-recoverable success path");
    assert_exit_code(&ok_output, 0);

    let err_output = Command::new(&binary)
        .arg("true")
        .output()
        .expect("should run shell-vs-recoverable fallback path");
    assert_exit_code(&err_output, 0);
}

#[test]
fn arithmetic_operators_fixture_compiles_and_runs() {
    let fixture = fixture_root("arithmetic_operators");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn boolean_logic_fixture_compiles_and_runs() {
    let fixture = fixture_root("boolean_logic");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn string_operations_fixture_compiles_and_runs() {
    let fixture = fixture_root("string_operations");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn fail_hidden_cross_file_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_hidden_cross_file");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "could not resolve");
}

#[test]
fn fail_loc_targets_formal_pkg_root_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_loc_targets_formal_pkg_root").join("app");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "build.fol");
    assert_output_contains(&output, "pkg");
}

#[test]
fn fail_type_mismatch_real_app_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_type_mismatch_real_app");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "record field 'score'");
}

#[test]
fn fail_recoverable_plain_context_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_recoverable_plain_context");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "cannot use '/ ErrorType' routine results as plain values");
}

#[test]
fn fail_shell_unwrap_boundary_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_shell_unwrap_boundary");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "postfix '!'");
    assert_output_contains(&output, "/ ErrorType");
}

#[test]
fn fail_deferred_intrinsic_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_deferred_intrinsic");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "not yet supported");
    assert_output_contains(&output, ".cap");
}

#[test]
fn fail_dfr_return_nested_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_dfr_return_nested");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "return is not allowed inside dfr/edf blocks");
}

#[test]
fn fail_dfr_break_nested_fixture_fails_cleanly() {
    let fixture = fixture_root("fail_dfr_break_nested");

    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "break is not allowed inside dfr/edf blocks");
}

#[test]
fn fail_generic_routine_fixture_now_executes_as_supported_generic_code() {
    let fixture = fixture_root("fail_generic_routine");
    let run_output = compile_and_run_app(&fixture);
    assert!(
        run_output.status.success(),
        "supported generic routine fixture should execute cleanly\nstdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run_output.stdout),
        String::from_utf8_lossy(&run_output.stderr)
    );
}

#[test]
fn fail_pointer_type_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_pointer_type");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "raw pointers are a V4 interop surface");
}

#[test]
fn fail_pipe_operator_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_pipe_operator");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "pipe");
}

#[test]
fn fail_membership_operator_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_membership_operator");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "membership");
}

#[test]
fn fail_named_unpack_with_extra_variadic_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_named_unpack_after_named");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "Positional call arguments are not allowed after named arguments");
}

#[test]
fn fail_unknown_named_method_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_unknown_named_method");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "does not have a parameter named 'missing'");
}

#[test]
fn fail_unknown_named_free_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_unknown_named_free");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "does not have a parameter named 'missing'");
}

#[test]
fn fail_duplicate_named_free_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_duplicate_named_free");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "supplies parameter 'left' more than once");
}

#[test]
fn fail_duplicate_named_method_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_duplicate_named_method");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "supplies parameter 'by' more than once");
}

#[test]
fn fail_unpack_non_sequence_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_unpack_non_sequence");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "call to 'sum' expects");
    assert_output_contains(&output, "seq[");
    assert_output_contains(&output, "int");
}

#[test]
fn fail_unpack_non_variadic_free_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_unpack_non_variadic_free");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "call-site unpack is only supported for variadic calls in V1");
}

#[test]
fn fail_variadic_method_type_mismatch_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_variadic_method_type_mismatch");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "call to 'shift' expects");
    assert_output_contains(&output, "int");
    assert_output_contains(&output, "str");
}

#[test]
fn fail_missing_required_named_arg_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_missing_required_named_arg");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "missing required argument 'right'");
}

#[test]
fn fail_double_unpack_free_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_double_unpack_free");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "call-site unpack cannot be combined with other variadic arguments in V1");
}

#[test]
fn fail_double_unpack_method_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_double_unpack_method");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "call-site unpack cannot be combined with other variadic arguments in V1");
}

#[test]
fn fail_unpack_non_variadic_method_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_unpack_non_variadic_method");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "call-site unpack is only supported for variadic calls in V1");
}

#[test]
fn fail_missing_required_method_arg_fixture_rejects_cleanly() {
    let fixture = fixture_root("fail_missing_required_method_arg");
    let output = compile_app_expect_failure(&fixture);
    assert_output_contains(&output, "missing required argument 'by'");
}

#[test]
fn anonymous_routine_fixture_compiles_and_runs() {
    let fixture = fixture_root("anonymous_routine");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn higher_order_function_fixture_compiles_and_runs() {
    let fixture = fixture_root("higher_order_function");

    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);

    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn recoverable_error_propagation_fixture_compiles_and_runs() {
    let fixture = fixture_root("recoverable_error_propagation");
    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);
    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn optional_shell_check_fixture_compiles_and_runs() {
    let fixture = fixture_root("optional_shell_check");
    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);
    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn empty_containers_fixture_compiles_and_runs() {
    let fixture = fixture_root("empty_containers");
    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);
    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

#[test]
fn nested_function_calls_fixture_compiles_and_runs() {
    let fixture = fixture_root("nested_function_calls");
    let compile_output = compile_app_keep_build_dir_expect_success(&fixture);
    assert_artifact_paths_exist(&compile_output);
    let run_output = compile_and_run_app(&fixture);
    assert_exit_code(&run_output, 0);
}

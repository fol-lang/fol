use super::*;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

struct TimedOutput {
    output: Output,
    timed_out: bool,
}

fn strip_ansi(value: &str) -> String {
    let mut stripped = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        stripped.push(ch);
    }

    stripped
}

fn write_hosted_app(name: &str, source: &str) -> std::path::PathBuf {
    let root = unique_temp_root(name);
    std::fs::create_dir_all(root.join("src")).expect("V3 runtime proof src should exist");
    std::fs::write(
        root.join("build.fol"),
        format!(
            "pro[] build(): non = {{\n\
                 \x20   var build = .build();\n\
                 \x20   build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n\
                 \x20   build.add_dep({{ alias = \"std\", source = \"internal\", target = \"standard\" }});\n\
                 \x20   var graph = build.graph();\n\
                 \x20   graph.add_exe({{\n\
                 \x20       name = \"{name}\",\n\
                 \x20       root = \"src/main.fol\",\n\
                 \x20       fol_model = \"memo\",\n\
                 \x20   }});\n\
                 \x20   return;\n\
                 }};\n"
        ),
    )
    .expect("V3 runtime proof build file should write");
    std::fs::write(root.join("src/main.fol"), source)
        .expect("V3 runtime proof source should write");
    root
}

fn build_hosted_app(root: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fol"))
        .args(["--package-store-root"])
        .arg(repo_root().join("lang/library"))
        .args(["code", "build", "--keep-build-dir"])
        .current_dir(root)
        .output()
        .expect("V3 runtime proof should invoke the FOL CLI")
}

fn built_binary_path(output: &Output) -> std::path::PathBuf {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let binary = stdout
        .lines()
        .find_map(|line| {
            let plain = strip_ansi(line);
            if let Some(tail) = plain.split("binary=").nth(1) {
                return Some(tail.trim().to_string());
            }
            if plain.contains("binary") {
                return plain.split_whitespace().last().map(str::to_string);
            }
            None
        })
        .expect("successful V3 runtime proof build should report its binary");
    std::path::PathBuf::from(binary)
}

fn run_with_timeout(binary: &std::path::Path, timeout: Duration) -> TimedOutput {
    let mut child = Command::new(binary)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("V3 runtime proof binary should start");
    let deadline = Instant::now() + timeout;

    loop {
        if child
            .try_wait()
            .expect("V3 runtime proof binary status should be readable")
            .is_some()
        {
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            child
                .stdout
                .take()
                .expect("captured stdout should exist")
                .read_to_end(&mut stdout)
                .expect("captured stdout should be readable");
            child
                .stderr
                .take()
                .expect("captured stderr should exist")
                .read_to_end(&mut stderr)
                .expect("captured stderr should be readable");
            let status = child
                .wait()
                .expect("completed V3 runtime proof binary should be reapable");
            return TimedOutput {
                output: Output {
                    status,
                    stdout,
                    stderr,
                },
                timed_out: false,
            };
        }

        if Instant::now() >= deadline {
            child
                .kill()
                .expect("timed-out V3 runtime proof binary should be killable");
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            child
                .stdout
                .take()
                .expect("captured stdout should exist")
                .read_to_end(&mut stdout)
                .expect("captured stdout should be readable");
            child
                .stderr
                .take()
                .expect("captured stderr should exist")
                .read_to_end(&mut stderr)
                .expect("captured stderr should be readable");
            let status = child
                .wait()
                .expect("killed V3 runtime proof binary should be reapable");
            return TimedOutput {
                output: Output {
                    status,
                    stdout,
                    stderr,
                },
                timed_out: true,
            };
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_build_succeeds(root: &std::path::Path) -> Output {
    let build = build_hosted_app(root);
    assert!(
        build.status.success(),
        "V3 runtime proof should build: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    build
}

fn assert_successful_stdout(root: &std::path::Path, expected: &str) {
    let build = assert_build_succeeds(root);
    let run = run_with_timeout(&built_binary_path(&build), Duration::from_secs(5));
    assert!(
        !run.timed_out,
        "V3 runtime proof should complete instead of blocking: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert!(
        run.output.status.success(),
        "V3 runtime proof should run successfully: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert_eq!(
        strip_ansi(&String::from_utf8_lossy(&run.output.stdout)),
        expected
    );
}

#[test]
fn unawaited_eventual_is_joined_at_process_exit() {
    let root = write_hosted_app(
        "v3_unawaited_eventual_join",
        "fun[] fail_after_main(): int = {\n\
             \x20   panic(\"unawaited eventual joined\");\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   var pending = fail_after_main() | async;\n\
             \x20   return 0;\n\
             };\n",
    );
    let build = assert_build_succeeds(&root);
    let run = run_with_timeout(&built_binary_path(&build), Duration::from_secs(5));
    assert!(!run.timed_out, "joining the eventual should not hang");
    // A detached Rust thread can panic without changing the process status.
    // Failure here proves the generated exit guard joined and observed it.
    assert!(
        !run.output.status.success(),
        "the unawaited eventual panic must be observed by the process-exit join"
    );
    assert!(
        String::from_utf8_lossy(&run.output.stderr).contains("unawaited eventual joined"),
        "the joined task panic should retain its payload: stderr=\n{}",
        String::from_utf8_lossy(&run.output.stderr)
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn blocking_select_completes_when_every_channel_is_closed() {
    let root = write_hosted_app(
        "v3_select_all_closed",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var closed: chn[int];\n\
             \x20   var[mut] observed: int = 42;\n\
             \x20   select {\n\
             \x20       when closed as value { observed = value; }\n\
             \x20   };\n\
             \x20   return std::io::echo_int(observed);\n\
             };\n",
    );
    assert_successful_stdout(&root, "42\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn simultaneously_ready_select_arms_prefer_source_order() {
    let root = write_hosted_app(
        "v3_select_source_order",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var first: chn[int];\n\
             \x20   var second: chn[int];\n\
             \x20   var sent_first: err[int] = 19 | first[tx];\n\
             \x20   var sent_second: err[int] = 23 | second[tx];\n\
             \x20   var[mut] selected: int = 0;\n\
             \x20   select {\n\
             \x20       when first as value { selected = value; }\n\
             \x20       when second as value { selected = value; }\n\
             \x20   };\n\
             \x20   return std::io::echo_int(selected);\n\
             };\n",
    );
    assert_successful_stdout(&root, "19\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn move_only_pointer_payload_crosses_a_channel() {
    let root = write_hosted_app(
        "v3_channel_move_only_payload",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var seed: int = 42;\n\
             \x20   var pointer: ptr[int] = [ref]seed;\n\
             \x20   var channel: chn[ptr[int]];\n\
             \x20   var sent: err[ptr[int]] = [mov]pointer | channel[tx];\n\
             \x20   var received: opt[ptr[int]] = channel[rx];\n\
             \x20   return std::io::echo_int([drf]received[]);\n\
             };\n",
    );
    assert_successful_stdout(&root, "42\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn move_only_pointer_result_crosses_an_eventual() {
    let root = write_hosted_app(
        "v3_eventual_move_only_result",
        "use std: pkg = {\"std\"};\n\
             fun[] make_pointer(value: int): ptr[int] = {\n\
             \x20   var copy: int = value;\n\
             \x20   var pointer: ptr[int] = [ref]copy;\n\
             \x20   return [mov]pointer;\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   var pending = make_pointer(42) | async;\n\
             \x20   var received: ptr[int] = pending | await;\n\
             \x20   return std::io::echo_int([drf]received);\n\
             };\n",
    );
    assert_successful_stdout(&root, "42\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn move_capture_carries_an_owned_pointer_into_a_spawned_task() {
    // A spawned task captures an owned `ptr[int]` by `[mov]` (V3_MEM §2.3 value
    // capture / V3_PROC owned spawn capture): the pointer moves whole into the
    // task environment, is dereferenced there, and its value is sent back over a
    // captured sender endpoint.
    let root = write_hosted_app(
        "v3_spawn_move_capture",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var seed: int = 7;\n\
             \x20   var pointer: ptr[int] = [ref]seed;\n\
             \x20   var channel: chn[int];\n\
             \x20   [>]fun()[pointer[mov], channel[tx]] = {\n\
             \x20       var sent: err[int] = [drf]pointer | channel[tx];\n\
             \x20   };\n\
             \x20   var received: opt[int] = channel[rx];\n\
             \x20   return std::io::echo_int(received[]);\n\
             };\n",
    );
    assert_successful_stdout(&root, "7\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn copy_capture_duplicates_a_value_into_a_task_and_keeps_the_source_live() {
    // A spawned task captures a `copy` value by `[cpy]`: an independent copy
    // crosses the spawn boundary (sent back as 9) while the outer binding stays
    // usable (9), so the program echoes 18. Contrasts with `[mov]`, which would
    // consume the source.
    let root = write_hosted_app(
        "v3_spawn_copy_capture",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var amount: int = 9;\n\
             \x20   var channel: chn[int];\n\
             \x20   [>]fun()[amount[cpy], channel[tx]] = {\n\
             \x20       var sent: err[int] = amount | channel[tx];\n\
             \x20   };\n\
             \x20   var received: opt[int] = channel[rx];\n\
             \x20   var still_here: int = amount;\n\
             \x20   return std::io::echo_int(still_here + received[]);\n\
             };\n",
    );
    assert_successful_stdout(&root, "18\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn clone_capture_duplicates_a_clonable_record_and_keeps_the_source_live() {
    // A spawned task captures a clonable (non-copy) record by `[cln]`: an
    // independent clone crosses the spawn boundary (its `value` sent back as 9)
    // while the outer binding stays usable (9), so the program echoes 18. The
    // `str` field makes the record genuinely clone-not-copy.
    let root = write_hosted_app(
        "v3_spawn_clone_capture",
        "use std: pkg = {\"std\"};\n\
             typ Item: rec = {\n\
             \x20   value: int,\n\
             \x20   tag: str\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   var item: Item = { value = 9, tag = \"hi\" };\n\
             \x20   var channel: chn[int];\n\
             \x20   [>]fun()[item[cln], channel[tx]] = {\n\
             \x20       var sent: err[int] = item.value | channel[tx];\n\
             \x20   };\n\
             \x20   var received: opt[int] = channel[rx];\n\
             \x20   var still: int = item.value;\n\
             \x20   return std::io::echo_int(still + received[]);\n\
             };\n",
    );
    assert_successful_stdout(&root, "18\n");
    std::fs::remove_dir_all(root).ok();
}

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

fn write_hosted_app(name: &str, source: &str) -> crate::fixture::TempFixture {
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
    Command::new(env!("CARGO_BIN_EXE_folc"))
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
    // `main`'s int return is the process exit status, and these proofs answer
    // with the value they computed rather than with a status. What must hold
    // is that the program reached its own end — the exact stdout below is the
    // proof's real content.
    assert!(
        run.output.status.code().is_some(),
        "V3 runtime proof should terminate on its own terms, not on a signal: stdout=\n{}\nstderr=\n{}",
        String::from_utf8_lossy(&run.output.stdout),
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert_ne!(
        run.output.status.code(),
        Some(101),
        "V3 runtime proof should not abort with a runtime panic: stdout=\n{}\nstderr=\n{}",
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
fn global_constants_read_their_declared_initializers() {
    // Globals lazily materialized through their DECLARED initializer; the
    // pre-fix OnceLock initialized from the type default, so `con LIMIT: int
    // = 9` silently read 0 and `con` globals rendered as mutable.
    let root = write_hosted_app(
        "v3_global_initializers",
        "use std: pkg = {\"std\"};\n\
             typ Counter: rec = { total: int };\n\
             con LIMIT: int = 4;\n\
             con BASE: Counter = { total = 9 };\n\
             fun[] main(): int = {\n\
             \x20   std::io::echo_int(LIMIT);\n\
             \x20   return std::io::echo_int([cpy]BASE.total);\n\
             };\n",
    );
    assert_successful_stdout(&root, "4\n9\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn destructuring_binds_positional_elements() {
    // `var a, b = { x, y }` destructures positionally (book unpacking); the
    // pre-fix parser broadcast the whole container into every binding.
    let root = write_hosted_app(
        "v3_destructuring",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var first, second = { 7, 8 };\n\
             \x20   std::io::echo_int(first * 10 + second);\n\
             \x20   var xs: vec[int] = { 5, 6, 7 };\n\
             \x20   var a, b, c = xs;\n\
             \x20   return std::io::echo_int(a * 100 + b * 10 + c);\n\
             };\n",
    );
    assert_successful_stdout(&root, "78\n567\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn entry_members_resolve_through_dot_access() {
    // Entries are groups of named constants accessed as `Type.MEMBER`
    // (`::` stays a namespace path). A bare member access types as the entry
    // itself and coerces to its payload only under an explicit expectation.
    // No prior example exercised entries at runtime, so pin both reads.
    let root = write_hosted_app(
        "v3_entry_members",
        "use std: pkg = {\"std\"};\n\
             typ Color: ent = {\n\
             \x20   con RED: int = 2,\n\
             \x20   con BLUE: int = 5,\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   std::io::echo_int(Color.RED);\n\
             \x20   var red: int = Color.RED;\n\
             \x20   var blue: int = Color.BLUE;\n\
             \x20   return std::io::echo_int(blue + red);\n\
             };\n",
    );
    assert_successful_stdout(&root, "2\n7\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn whole_reassignment_restores_a_partially_moved_binding() {
    // §3.2: moving one field invalidates only that field, and whole-binding
    // reassignment of the (reassignable) root restores every field. The
    // restored aggregate is fully readable afterwards.
    let root = write_hosted_app(
        "v3_partial_move_restore",
        "use std: pkg = {\"std\"};\n\
             typ Pair: rec = { a: str, b: int };\n\
             fun[] main(): int = {\n\
             \x20   var pair: Pair = { a = \"x\", b = 1 };\n\
             \x20   var taken: str = [mov]pair.a;\n\
             \x20   std::io::echo_int(pair.b);\n\
             \x20   pair = { a = \"y\", b = 2 };\n\
             \x20   std::io::echo_int(pair.b);\n\
             \x20   return 0;\n\
             };\n",
    );
    assert_successful_stdout(&root, "1\n2\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn if_statements_branch_on_the_condition_value() {
    // `if` desugars to `when (cond) { case (true) ... * ... }`; the case value
    // must be the literal `true`, never a re-evaluation of the condition (a
    // self-comparison always matched, so every `if` took its then-branch).
    // Pins: false condition takes else, true condition takes then, an
    // else-less false `if` skips, and a following block/`if` statement is
    // independent — never silently absorbed as an else-branch.
    let root = write_hosted_app(
        "v3_if_branching",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var x: int = 1;\n\
             \x20   if (x > 3) {\n\
             \x20       std::io::echo_int(99);\n\
             \x20   } else {\n\
             \x20       std::io::echo_int(7);\n\
             \x20   };\n\
             \x20   if (x < 3) {\n\
             \x20       std::io::echo_int(11);\n\
             \x20   }\n\
             \x20   {\n\
             \x20       std::io::echo_int(12);\n\
             \x20   };\n\
             \x20   if (x > 100) {\n\
             \x20       std::io::echo_int(88);\n\
             \x20   }\n\
             \x20   if (x < 100) {\n\
             \x20       std::io::echo_int(13);\n\
             \x20   };\n\
             \x20   return 0;\n\
             };\n",
    );
    assert_successful_stdout(&root, "7\n11\n12\n13\n");
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

#[test]
fn mux_wrap_transfers_the_owner_into_one_mutex() {
    // Wrapping an owner into a mux[T] parameter requires `[mov]` and hands the
    // state to exactly one mutex. The pre-fix implicit wrap copied the owner
    // per boundary, so increments made under one wrap were silently lost when
    // the same binding was wrapped again.
    let root = write_hosted_app(
        "v3_mux_wrap_transfer",
        "use std: pkg = {\"std\"};\n\
             typ Counter: rec = { value: int };\n\
             fun[] coordinate(counter: mux[Counter]): int = {\n\
             \x20   [>]bump(counter);\n\
             \x20   [>]bump(counter);\n\
             \x20   return 0;\n\
             };\n\
             fun[] bump(counter: mux[Counter]): int = {\n\
             \x20   counter.lock();\n\
             \x20   counter.value = counter.value + 1;\n\
             \x20   return std::io::echo_int(counter.value);\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   var counter: Counter = { value = 0 };\n\
             \x20   coordinate([mov]counter);\n\
             \x20   return 0;\n\
             };\n",
    );
    // Increments serialize under one lock: the second task must observe the
    // first task's increment, whatever the scheduling order.
    assert_successful_stdout(&root, "1\n2\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn captured_closures_are_callable_inside_closure_bodies() {
    // A closure may capture another closure by `[mov]` and call it: the call
    // resolves to the capture binding, not the (frozen) outer local. The
    // pre-fix call path skipped Capture symbols entirely and misreported the
    // explicit capture as an unsupported implicit one.
    let root = write_hosted_app(
        "v3_closure_captures_closure",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var base: int = 3;\n\
             \x20   var inner: {fun (): int} = fun()[base[cpy]]: int = { return base; };\n\
             \x20   var outer: {fun (): int} = fun()[inner[mov]]: int = { return inner() + 1; };\n\
             \x20   return std::io::echo_int(outer());\n\
             };\n",
    );
    assert_successful_stdout(&root, "4\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn else_less_if_guards_fall_through_and_terminate() {
    // A bare `if` guard desugars to a `when` with an EMPTY synthesized
    // default arm; that arm yields no value, so the when must lower as a
    // statement. The pre-fix router classified it as value-producing and
    // lowering died with L1002 on every else-less early-return/report guard.
    let root = write_hosted_app(
        "v3_if_guard_fallthrough",
        "use std: pkg = {\"std\"};\n\
             fun[] pick(flag: int): int = {\n\
             \x20   if (flag > 0) {\n\
             \x20       return 1;\n\
             \x20   }\n\
             \x20   return 7;\n\
             };\n\
             fun[] risky(flag: int): int / int = {\n\
             \x20   if (flag > 0) {\n\
             \x20       report 99;\n\
             \x20   }\n\
             \x20   return 8;\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   std::io::echo_int(pick(1));\n\
             \x20   std::io::echo_int(pick(0));\n\
             \x20   std::io::echo_int(risky(1) || 3);\n\
             \x20   return std::io::echo_int(risky(0) || 3);\n\
             };\n",
    );
    assert_successful_stdout(&root, "1\n7\n3\n8\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn integer_division_faults_present_as_fol_runtime_faults() {
    // Division/modulo by zero panic per the arithmetics chapter, but the
    // message must be a fol runtime fault, not a raw Rust panic pointing
    // into generated code paths.
    let root = write_hosted_app(
        "v3_div_zero_fault",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var a: int = 10;\n\
             \x20   var b: int = 0;\n\
             \x20   return std::io::echo_int(a / b);\n\
             };\n",
    );
    let build = assert_build_succeeds(&root);
    let run = run_with_timeout(&built_binary_path(&build), Duration::from_secs(5));
    assert!(!run.timed_out, "the faulting division should not hang");
    assert!(!run.output.status.success());
    assert!(
        String::from_utf8_lossy(&run.output.stderr).contains("fol runtime fault: division by zero"),
        "stderr should carry the branded fault: {}",
        String::from_utf8_lossy(&run.output.stderr)
    );
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn lifetime_spelled_eventuals_flow_through_signatures_and_await() {
    // §8.1 conservative region model: with all storage escapes rejected, an
    // eventual travels only through `evt[L, T]`-spelled signatures and local
    // moves. Pin the full legal journey: spawn, pass down, forward back up,
    // await in a callee.
    let root = write_hosted_app(
        "v3_evt_lifetime_roundtrip",
        "use std: pkg = {\"std\"};\n\
             fun[] work(value: int): int = {\n\
             \x20   return value + 1;\n\
             };\n\
             fun forward(L: lif)(pending: evt[L, int]): evt[L, int] = {\n\
             \x20   return [mov]pending;\n\
             };\n\
             fun consume(L: lif)(pending: evt[L, int]): int = {\n\
             \x20   return pending | await;\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   var pending: evt[int] = work(40) | async;\n\
             \x20   var routed: evt[int] = forward([mov]pending);\n\
             \x20   return std::io::echo_int(consume([mov]routed) + 1);\n\
             };\n",
    );
    assert_successful_stdout(&root, "42\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn terminal_primitives_write_convert_and_keep_time() {
    // The TUI primitive layer: `.write` emits without a newline (two writes
    // land on one line), `int_to_str` renders decimals, `sleep_ms` forwards
    // its duration, and `now_ms` yields a positive epoch stamp.
    let root = write_hosted_app(
        "v3_terminal_primitives",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var left: str = std::io::write(\"4\");\n\
             \x20   var right: str = std::io::write(\"2\\n\");\n\
             \x20   var rendered: str = std::fmt::int_to_str(-137);\n\
             \x20   var echoed: str = std::io::echo_str(rendered);\n\
             \x20   std::io::echo_int(std::time::sleep_ms(1));\n\
             \x20   var stamp: int = std::time::now_ms();\n\
             \x20   if (stamp > 0) {\n\
             \x20       std::io::echo_int(1);\n\
             \x20   }\n\
             \x20   return 0;\n\
             };\n",
    );
    assert_successful_stdout(&root, "42\n-137\n1\n1\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn string_primitives_slice_index_and_absorb_chars() {
    // The text-shaping primitive layer: byte-range substrings, byte reads,
    // byte-to-string conversion, and `+` absorbing one-character literals
    // (which type as `chr`) on either side of a string.
    let root = write_hosted_app(
        "v3_string_primitives",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var text: str = \"hello world\";\n\
             \x20   var head: str = std::strn::sub(text, 0, 5);\n\
             \x20   var echoed: str = std::io::echo_str(head + \"!\");\n\
             \x20   std::io::echo_int(std::strn::byte_at(text, 0));\n\
             \x20   std::io::echo_int(std::strn::byte_at(text, 99));\n\
             \x20   var q: str = std::strn::from_byte(113);\n\
             \x20   var wrapped: str = std::io::echo_str(\"<\" + q + \">\");\n\
             \x20   var padded: str = \"0\" + std::fmt::int_to_str(7);\n\
             \x20   var shown: str = std::io::echo_str(padded);\n\
             \x20   return 0;\n\
             };\n",
    );
    assert_successful_stdout(&root, "hello!\n104\n-1\n<q>\n07\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn env_and_shell_hooks_read_the_host() {
    // `std::os::env` yields the variable or an empty string; `std::os::shell` runs a
    // command and forwards its exit code.
    let root = write_hosted_app(
        "v3_env_shell",
        "use std: pkg = {\"std\"};\n\
             fun[] main(): int = {\n\
             \x20   var missing: str = std::os::env(\"FOL_DEFINITELY_UNSET_VAR\");\n\
             \x20   std::io::echo_int(.len(missing));\n\
             \x20   var home: str = std::os::env(\"HOME\");\n\
             \x20   if (.len(home) > 0) {\n\
             \x20       std::io::echo_int(1);\n\
             \x20   }\n\
             \x20   std::io::echo_int(std::os::shell(\"exit 3\"));\n\
             \x20   std::io::echo_int(std::os::shell(\"true\"));\n\
             \x20   return 0;\n\
             };\n",
    );
    assert_successful_stdout(&root, "0\n1\n3\n0\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn filesystem_hooks_list_and_read() {
    // `std::fs::dir_list` yields sorted entries (dirs slash-suffixed);
    // `std::fs::read_file` yields contents or an empty string.
    let staging = unique_temp_root("v3_fs_hooks_data");
    std::fs::create_dir_all(staging.join("inner")).expect("fs hook staging dir");
    std::fs::write(staging.join("note.txt"), "steep").expect("fs hook staging file");
    let root = write_hosted_app(
        "v3_fs_hooks",
        &("use std: pkg = {\"std\"};\n".to_string()
            + &format!(
                "fun[] main(): int = {{\n\
             \x20   var entries: str = std::fs::dir_list(\"{dir}\");\n\
             \x20   var shown: str = std::io::echo_str(entries);\n\
             \x20   var source: str = std::fs::read_file(\"{dir}/note.txt\");\n\
             \x20   if (source == \"steep\") {{\n\
             \x20       std::io::echo_int(1);\n\
             \x20   }}\n\
             \x20   std::io::echo_int(.len(std::fs::read_file(\"no/such/file\")));\n\
             \x20   return 0;\n\
             }};\n",
                dir = staging.display()
            )),
    );
    assert_successful_stdout(&root, "inner/\nnote.txt\n1\n0\n");
    std::fs::remove_dir_all(root).ok();
    std::fs::remove_dir_all(staging).ok();
}

#[test]
fn cli_run_hands_the_terminal_to_the_program_it_launches() {
    // `fol code run` must not capture the child's streams. Capturing gives the
    // program a null stdin, so anything interactive -- a prompt, a key reader,
    // a full-screen TUI -- sees end of input immediately and can never work
    // through the tool that is supposed to launch it.
    let root = write_hosted_app(
        "v3_run_stdin_forwarding",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] main(): int = {\n\
         \x20   var key: int = std::io::read_key();\n\
         \x20   std::io::echo_int(key);\n\
         \x20   return 0;\n\
         };\n",
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_folc"))
        .args(["--package-store-root"])
        .arg(repo_root().join("lang/library"))
        .args(["code", "run"])
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run should start the FOL CLI");
    {
        use std::io::Write;
        child
            .stdin
            .take()
            .expect("run should expose a stdin pipe")
            .write_all(b"A")
            .expect("run should accept piped input");
    }
    let output = child.wait_with_output().expect("run should finish");

    let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "run should succeed: stdout=\n{stdout}\nstderr=\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // 65 is 'A'; -1 is the end-of-input a captured stdin would produce.
    assert!(
        stdout.contains("65"),
        "the program should have received the byte typed into `fol code run`, got:\n{stdout}"
    );
}

#[test]
fn string_number_and_write_hooks_back_real_cli_work() {
    // The primitives a command-line program cannot be written without:
    // searching and rewriting text, turning an argument into a number,
    // formatting a float, and writing a file back out.
    let staging = unique_temp_root("v3_cli_hooks_data");
    std::fs::create_dir_all(&staging).expect("cli hook staging dir");
    let target = staging.join("written.txt");
    let root = write_hosted_app(
        "v3_cli_hooks",
        &("use std: pkg = {\"std\"};\n".to_string()
            + &format!(
                "fun[] main(): int = {{\n\
             \x20   std::io::echo_int(std::strn::find(\"hello world\", \"world\"));\n\
             \x20   std::io::echo_int(std::strn::find(\"hello\", \"absent\"));\n\
             \x20   std::io::echo_str(std::strn::replace(\"a-b-c\", \"-\", \"+\"));\n\
             \x20   std::io::echo_int(std::strn::to_int(\"42\", 0));\n\
             \x20   std::io::echo_int(std::strn::to_int(\"not a number\", -7));\n\
             \x20   std::io::echo_str(std::fmt::float_to_str(3.14159, 2));\n\
             \x20   std::io::echo_int(std::fs::write_file(\"{path}\", \"written\"));\n\
             \x20   std::io::echo_str(std::fs::read_file(\"{path}\"));\n\
             \x20   std::io::echo_int(std::fs::write_file(\"/no/such/dir/x\", \"nope\"));\n\
             \x20   return 0;\n\
             }};\n",
                path = target.display()
            )),
    );
    assert_successful_stdout(&root, "6\n-1\na+b+c\n42\n-7\n3.14\n0\nwritten\n-1\n");
    assert_eq!(
        std::fs::read_to_string(&target).expect("the program should have written the file"),
        "written"
    );
}

#[test]
fn command_line_arguments_reach_the_program() {
    let root = write_hosted_app(
        "v3_argv_hooks",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] main(): int = {\n\
         \x20   std::io::echo_int(std::os::arg_count());\n\
         \x20   std::io::echo_str(std::os::arg(0));\n\
         \x20   std::io::echo_str(std::os::arg(1));\n\
         \x20   std::io::echo_str(std::os::arg(9));\n\
         \x20   return 0;\n\
         };\n",
    );
    let build = assert_build_succeeds(&root);
    let output = Command::new(built_binary_path(&build))
        .args(["first", "second"])
        .output()
        .expect("argv proof binary should run");

    assert!(output.status.success(), "argv proof should exit cleanly");
    // Index 0 is the first real argument, not the program name, and an index
    // past the end reads as empty rather than crashing.
    assert_eq!(
        strip_ansi(&String::from_utf8_lossy(&output.stdout)),
        "2\nfirst\nsecond\n\n"
    );
}

#[test]
fn standard_error_stays_separate_from_standard_output() {
    let root = write_hosted_app(
        "v3_stderr_hook",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] main(): int = {\n\
         \x20   std::io::echo_str(\"out\");\n\
         \x20   std::io::write_err(\"err\");\n\
         \x20   return 0;\n\
         };\n",
    );
    let build = assert_build_succeeds(&root);
    let run = run_with_timeout(&built_binary_path(&build), Duration::from_secs(5));

    assert!(
        run.output.status.success(),
        "stderr proof should exit cleanly"
    );
    assert_eq!(
        strip_ansi(&String::from_utf8_lossy(&run.output.stdout)),
        "out\n"
    );
    assert_eq!(
        strip_ansi(&String::from_utf8_lossy(&run.output.stderr)),
        "err"
    );
}

#[test]
fn deferred_blocks_run_inside_when_and_select_arms() {
    // A `when`/`select` arm body is its own scope, but it carries no syntax id
    // of its own, so the arm scope was inferred from the bindings the body
    // declared -- and an arm whose only statement is `dfr { ... }` declares
    // none. It fell back to the enclosing scope and the deferred block was then
    // rejected for belonging to the wrong parent. This is the book's own
    // "Nested scopes" example (book/src/700_sugar/250_dfr.md).
    let root = write_hosted_app(
        "v3_dfr_in_arms",
        "use std: pkg = {\"std\"};\n\
         \n\
         pro[] guarded(flag: bol): non = {\n\
         \x20   dfr { std::io::echo_str(\"outer\"); };\n\
         \x20   when(flag) {\n\
         \x20       case(true) {\n\
         \x20           dfr { std::io::echo_str(\"inner\"); };\n\
         \x20           return;\n\
         \x20       }\n\
         \x20       * { }\n\
         \x20   }\n\
         \x20   return;\n\
         };\n\
         \n\
         pro[] selected(): non = {\n\
         \x20   var ch: chn[int];\n\
         \x20   select {\n\
         \x20       * {\n\
         \x20           dfr { std::io::echo_str(\"select-arm\"); };\n\
         \x20       }\n\
         \x20   }\n\
         \x20   return;\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
         \x20   guarded(true);\n\
         \x20   std::io::echo_str(\"--\");\n\
         \x20   guarded(false);\n\
         \x20   selected();\n\
         \x20   return 0;\n\
         };\n",
    );
    // The arm's deferred block runs when the arm exits, before the routine's
    // own; the arm that never runs registers nothing.
    assert_successful_stdout(&root, "inner\nouter\n--\nouter\nselect-arm\n");
}

#[test]
fn mux_guard_binding_reads_a_non_record_payload_through_the_lock() {
    // A named guard binding aliases the mutex local, so reading it whole was
    // rendered as a clone of the `FolMutex<T>` handle -- assigned into a
    // `&mut T` slot. Records hid it (every access went through a field path);
    // an `int` or `str` payload has no fields to hide behind, and rustc
    // rejected the generated crate.
    let root = write_hosted_app(
        "v3_mux_guard_scalar_payload",
        "use std: pkg = {\"std\"};\n\
             typ Counter: rec = { n: int };\n\
             fun[] peek_int(state: mux[int]): int = {\n\
             \x20   var[mut, bor] guard: int = ([bor]state).lock();\n\
             \x20   return std::io::echo_int(guard + guard);\n\
             };\n\
             fun[] peek_str(state: mux[str]): int = {\n\
             \x20   var[bor] guard: str = ([bor]state).lock();\n\
             \x20   std::io::echo_str(guard + \"!\");\n\
             \x20   return 0;\n\
             };\n\
             fun[] peek_rec(state: mux[Counter]): int = {\n\
             \x20   var[mut, bor] guard: Counter = ([bor]state).lock();\n\
             \x20   guard.n = guard.n + 1;\n\
             \x20   return std::io::echo_int(guard.n);\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   var v: int = 41;\n\
             \x20   var s: str = \"guarded\";\n\
             \x20   var c: Counter = { n = 7 };\n\
             \x20   peek_int([mov]v);\n\
             \x20   peek_str([mov]s);\n\
             \x20   peek_rec([mov]c);\n\
             \x20   return 0;\n\
             };\n",
    );
    assert_successful_stdout(&root, "82\nguarded!\n8\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn borrowed_scalars_read_by_value_at_operators() {
    // An operator result took the left operand's lowered type verbatim, so a
    // borrowed operand produced a `&T` result slot that no arithmetic can fill,
    // and the operands themselves were emitted as raw references. Both sides
    // failed rustc on the generated crate.
    let root = write_hosted_app(
        "v3_borrowed_operands",
        "use std: pkg = {\"std\"};\n\
             fun[] bump(value: int[bor]): int = {\n\
             \x20   return value + 1;\n\
             };\n\
             fun[] flip(value: int[bor]): int = {\n\
             \x20   return -value;\n\
             };\n\
             fun[] shout(text: str[bor]): str = {\n\
             \x20   return text + \"!\";\n\
             };\n\
             fun[] main(): int = {\n\
             \x20   var v: int = 41;\n\
             \x20   var t: str = \"hi\";\n\
             \x20   std::io::echo_int(bump([bor]v));\n\
             \x20   std::io::echo_int(flip([bor]v));\n\
             \x20   std::io::echo_str(shout([bor]t));\n\
             \x20   return 0;\n\
             };\n",
    );
    assert_successful_stdout(&root, "42\n-41\nhi!\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn aggregates_holding_routine_values_build_and_run() {
    // Routine values are `Rc<dyn Fn(..)>` handles, which implement no `Default`
    // and no echo formatting. Zero-initializing a vec, array or record holding
    // one used to reach for `Default::default()`, and the record's echo impl
    // formatted the closure, so all three typechecked and then failed rustc on
    // the generated crate.
    let root = write_hosted_app(
        "v3_routine_value_aggregates",
        "use std: pkg = {\"std\"};\n\
             typ Boxx: rec = { tag: str, f: {fun (n: int): int} };\n\
             fun[] main(): int = {\n\
             \x20   var hooks: vec[{fun (n: int): int}];\n\
             \x20   var pending: seq[{fun (n: int): int}];\n\
             \x20   var b: Boxx = { tag = \"hi\", f = fun(n: int)[]: int = { return n + 1; } };\n\
             \x20   .echo(b);\n\
             \x20   var f: {fun (n: int): int} = fun(n: int)[]: int = { return n + 2; };\n\
             \x20   var slots: arr[{fun (n: int): int}, 1] = {[mov]f};\n\
             \x20   return std::io::echo_int(.len(hooks) + .len(pending) + .len(slots));\n\
             };\n",
    );
    assert_successful_stdout(&root, "Boxx { f: <routine>, tag: hi }\n1\n");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn higher_order_generic_routines_can_be_called() {
    // `T` was never substituted inside a routine type, so the parameter stayed
    // spelled `fun(): T` while the argument was `fun(): int` and the call was
    // rejected -- even with an explicit turbofish. Inference had the same hole,
    // and the generated Rust then used a nested `fn` for the placeholder, which
    // cannot name the enclosing routine's generic parameter.
    let root = write_hosted_app(
        "v3_generic_routine_types",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] seven(): int = {\n\
         \x20   return 7;\n\
         };\n\
         \n\
         fun[] apply(T)(f: {fun (): T}): T = {\n\
         \x20   return f();\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
         \x20   var g: {fun (): int} = [mov]seven;\n\
         \x20   std::io::echo_int(apply([mov]g));\n\
         \x20   return 0;\n\
         };\n",
    );
    assert_successful_stdout(&root, "7\n");
}

#[test]
fn json_mode_keeps_stdout_parseable_when_the_program_prints() {
    // The child inherited the same stdout the envelope is written to, so any
    // program that printed made `--output json` unparseable for a tool.
    let root = write_hosted_app(
        "v3_json_run_stream",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] main(): int = {\n\
         \x20   std::io::echo_int(41);\n\
         \x20   return 0;\n\
         };\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_folc"))
        .args(["--package-store-root"])
        .arg(repo_root().join("lang/library"))
        .args(["--output", "json", "code", "run"])
        .current_dir(&root)
        .output()
        .expect("json run should start");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        serde_json::from_str::<serde_json::Value>(stdout.trim()).is_ok(),
        "stdout must stay valid JSON, got:\n{stdout}"
    );
    // The program's own output is still delivered, on the other stream.
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("41"),
        "the program's output must not be discarded"
    );
}

#[test]
fn fin_values_held_in_record_fields_are_finalized_through_their_owner() {
    // Containment used to be rejected outright, because finalization was
    // registered only for a value a binding named directly. A field path names
    // it just as well, so the holder now finalizes what it owns -- and moving
    // the holder hands that duty to the receiving routine rather than running
    // it twice or not at all.
    let root = write_hosted_app(
        "v3_fin_record_field",
        "use std: pkg = {\"std\"};\n\
         \n\
         typ File()(fin): rec = { descriptor: int };\n\
         \n\
         pro (File)finalize(): non = {\n\
         \x20   var shown: int = std::io::echo_int(self.descriptor);\n\
         \x20   return;\n\
         };\n\
         \n\
         typ Pair: rec = { left: File, right: File, tag: int };\n\
         \n\
         pro[] consume(taken: Pair): non = {\n\
         \x20   var shown: int = std::io::echo_int(taken.tag);\n\
         \x20   return;\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
         \x20   var pair: Pair = { left = { descriptor = 1 }, right = { descriptor = 2 }, tag = 9 };\n\
         \x20   consume([mov]pair);\n\
         \x20   var held: Pair = { left = { descriptor = 3 }, right = { descriptor = 4 }, tag = 8 };\n\
         \x20   var shown: int = std::io::echo_int(held.tag);\n\
         \x20   return 0;\n\
         };\n",
    );
    // The moved holder is released by the callee (2, 1); main's own holder at
    // its scope exit (4, 3). Reverse field order in both, per V3_MEM 6.2.
    assert_successful_stdout(&root, "9\n2\n1\n8\n4\n3\n");
}

#[test]
fn fin_values_held_in_containers_are_finalized_at_scope_exit() {
    // A container holds a runtime number of values, so releasing them needs
    // iteration rather than a named call -- the reason this was rejected
    // outright until the IR grew a scope-exit finalize-each instruction.
    let root = write_hosted_app(
        "v3_fin_container",
        "use std: pkg = {\"std\"};\n\
         \n\
         typ File()(fin): rec = { descriptor: int };\n\
         \n\
         pro (File)finalize(): non = {\n\
         \x20   var shown: int = std::io::echo_int(self.descriptor);\n\
         \x20   return;\n\
         };\n\
         \n\
         typ Bag: rec = { files: vec[File] };\n\
         \n\
         fun[] main(): int = {\n\
         \x20   var pool: vec[File] = { { descriptor = 1 }, { descriptor = 2 } };\n\
         \x20   var lookup: map[int, File] = {{7, { descriptor = 3 }}};\n\
         \x20   var bag: Bag = { files = { { descriptor = 4 } } };\n\
         \x20   std::io::echo_str(\"scope end\");\n\
         \x20   return 0;\n\
         };\n",
    );
    // Owners release in reverse declaration order; a container hands out its
    // elements in order. The bag proves a container reached through a field
    // path works too.
    assert_successful_stdout(&root, "scope end\n4\n3\n1\n2\n");
}

#[test]
fn and_or_do_not_evaluate_the_operand_they_do_not_need() {
    // Both operands were lowered into locals before the operator ran, so
    // `false and f()` still called `f` -- which breaks the guard idiom the
    // operators exist for (`p != nil and p.field > 0`).
    let root = write_hosted_app(
        "v3_short_circuit",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] loud(tag: int, answer: bol): bol = {\n\
         \x20   std::io::echo_int(tag);\n\
         \x20   return answer;\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
         \x20   std::io::echo_bool(false and true);\n\
         \x20   std::io::echo_bool(true and false);\n\
         \x20   std::io::echo_bool(false or true);\n\
         \x20   var a: bol = loud(1, false) and loud(2, true);\n\
         \x20   var b: bol = loud(3, true) or loud(4, false);\n\
         \x20   var c: bol = loud(5, true) and loud(6, true);\n\
         \x20   return 0;\n\
         };\n",
    );
    // The skipped operands (2 and 4) never print; 5 and 6 both do, because
    // there the right side still decides the answer.
    assert_successful_stdout(&root, "false\nfalse\ntrue\n1\n3\n5\n6\n");
}

#[test]
fn constrained_generic_dispatch_reaches_each_conformer_not_a_structural_twin() {
    // Records interned structurally, so two declarations with the same field
    // list collapsed to one lowered type -- and constraint dispatch, which
    // matches a conformer by its receiver's lowered type, then called whichever
    // one it found first. Alpha's method never ran, silently: no diagnostic,
    // wrong answer. Distinct field names made the same program work, which is
    // what pinned it to interning rather than to dispatch.
    let root = write_hosted_app(
        "v3_conformer_identity",
        "use std: pkg = {\"std\"};\n\
         \n\
         std geometry: pro = {\n\
         \x20   fun size(): int;\n\
         };\n\
         \n\
         typ Alpha()(geometry): rec = { w: int };\n\
         typ Beta()(geometry): rec = { w: int };\n\
         \n\
         fun (Alpha)size(): int = {\n\
         \x20   return 111;\n\
         };\n\
         \n\
         fun (Beta)size(): int = {\n\
         \x20   return 222;\n\
         };\n\
         \n\
         fun[] show(T: geometry)(part: T): int = {\n\
         \x20   return part.size();\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
         \x20   var a: Alpha = { w = 1 };\n\
         \x20   var b: Beta = { w = 2 };\n\
         \x20   std::io::echo_int(show([mov]a));\n\
         \x20   std::io::echo_int(show([mov]b));\n\
         \x20   return 0;\n\
         };\n",
    );
    assert_successful_stdout(&root, "111\n222\n");
}

#[test]
fn global_constants_accept_a_negated_number() {
    // `-1` parses as a negation OVER a literal, not as a literal, so a global
    // `con` refused it while the same text inside a routine was accepted. This
    // runs rather than only typechecking, because the initializer is emitted
    // straight into the generated Rust and a wrong sign there is silent.
    let root = write_hosted_app(
        "v3_negative_global_const",
        "use std: pkg = {\"std\"};\n\
         \n\
         con MISSING: int = -1;\n\
         con DEEPER: int = -2;\n\
         con SCALE: flt = -1.5;\n\
         con PRESENT: int = 7;\n\
         \n\
         fun[] main(): int = {\n\
         \x20   std::io::echo_int(MISSING);\n\
         \x20   std::io::echo_int(DEEPER);\n\
         \x20   std::io::echo_int(PRESENT);\n\
         \x20   std::io::echo_bool(SCALE < 0.0);\n\
         \x20   return 0;\n\
         };\n",
    );
    assert_successful_stdout(&root, "-1\n-2\n7\ntrue\n");
}

#[test]
fn a_literal_argument_is_shaped_by_the_parameter_it_fills() {
    // A one-character text literal is a `chr` on its own, and a container
    // literal is an array, so both were rejected against a `str` or `vec`
    // parameter -- while the same literal passed to a user routine was accepted.
    //
    // This RUNS rather than only typechecking. An earlier attempt made
    // typecheck agree while lowering still emitted a Rust `char`, which no
    // typecheck-only test could have caught.
    let root = write_hosted_app(
        "v3_literal_argument_shape",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] main(): int = {\n\
         \x20   std::io::echo_str(.str_upper(\"a\"));\n\
         \x20   std::io::echo_int(.str_find(\"banana\", \"n\"));\n\
         \x20   std::io::echo_str(.str_from_bytes({104, 105}));\n\
         \x20   std::io::echo_int(.len(.run_capture(\"echo\", {\"hi\"})));\n\
         \x20   return 0;\n\
         };\n",
    );
    assert_successful_stdout(&root, "A\n2\nhi\n3\n");
}

#[test]
fn a_literal_operand_is_shaped_by_the_other_side_of_the_comparison() {
    // `text == "a"` compared a `str` against a `chr` and was rejected, though
    // `text + "a"` had always been accepted. Runs for the same reason as above:
    // shaping it in typecheck alone emitted a `char` against a `FolStr`.
    let root = write_hosted_app(
        "v3_literal_operand_shape",
        "use std: pkg = {\"std\"};\n\
         \n\
         fun[] main(): int = {\n\
         \x20   var text: str = \"a\";\n\
         \x20   var letter: chr = 'a';\n\
         \x20   std::io::echo_bool(text == \"a\");\n\
         \x20   std::io::echo_bool(\"a\" == text);\n\
         \x20   std::io::echo_bool(text != \"b\");\n\
         \x20   std::io::echo_bool(.str_upper(\"a\") == \"A\");\n\
         \x20   std::io::echo_bool(letter == 'a');\n\
         \x20   std::io::echo_bool((text + \"b\") == \"ab\");\n\
         \x20   return 0;\n\
         };\n",
    );
    assert_successful_stdout(&root, "true\ntrue\ntrue\ntrue\ntrue\ntrue\n");
}

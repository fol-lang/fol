use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub(super) const V3_PROC_M1_FAILURES: &[(&str, &str)] = &[
    (
        "examples/fail_proc_spawn_in_core_m1",
        "spawn requires hosted std support",
    ),
    (
        "examples/fail_proc_spawn_in_memo_m1",
        "spawn requires hosted std support",
    ),
    (
        "examples/fail_proc_spawn_rc_cross_m1",
        "shared Rc pointers cannot cross a spawn or async thread boundary",
    ),
    (
        "examples/fail_proc_spawn_recoverable_m1",
        "spawning a recoverable routine without await discards its error",
    ),
    (
        "examples/fail_proc_spawn_heap_use_after_move_m1",
        "use of moved heap-owned binding 'owned'",
    ),
];

pub(super) const V3_PROC_M2_FAILURES: &[(&str, &str)] = &[
    (
        "examples/fail_proc_channel_index_m2",
        "channel receivers are blocking pull expressions and cannot be indexed",
    ),
    (
        "examples/fail_proc_channel_in_core_m2",
        "channel types require hosted std support",
    ),
    (
        "examples/fail_proc_channel_in_memo_m2",
        "channel types require hosted std support",
    ),
    (
        "examples/fail_proc_channel_capture_rx_m2",
        "captured endpoint 'channel[tx]' is sender-only",
    ),
    (
        "examples/fail_proc_channel_spawn_consumer_m2",
        "routine 'consume' receives from a channel and cannot be spawned directly",
    ),
];

pub(super) const V3_PROC_M3_FAILURES: &[(&str, &str)] = &[
    (
        "examples/fail_proc_select_old_form_m3",
        "old select(channel as binding) form is not supported",
    ),
    (
        "examples/fail_proc_select_in_core_m3",
        "select requires hosted std support",
    ),
    (
        "examples/fail_proc_select_in_memo_m3",
        "select requires hosted std support",
    ),
    (
        "examples/fail_proc_mutex_double_paren_m3",
        "Expected generic parameter name",
    ),
    (
        "examples/fail_proc_mutex_in_core_m3",
        "mutex parameters require hosted std support",
    ),
    (
        "examples/fail_proc_mutex_in_memo_m3",
        "mutex parameters require hosted std support",
    ),
];

pub(super) const V3_PROC_M4_FAILURES: &[(&str, &str)] = &[
    (
        "examples/fail_proc_evt_named_m4",
        "eventual types are internal in V3 and cannot be named",
    ),
    (
        "examples/fail_proc_async_in_core_m4",
        "async pipe stages require hosted std support",
    ),
    (
        "examples/fail_proc_async_in_memo_m4",
        "async pipe stages require hosted std support",
    ),
    (
        "examples/fail_proc_await_in_core_m4",
        "await pipe stages require hosted std support",
    ),
    (
        "examples/fail_proc_await_in_memo_m4",
        "await pipe stages require hosted std support",
    ),
];

pub(super) const V3_MEM_M1_FAILURES: &[(&str, &str)] = &[
    ("examples/fail_mem_use_after_move_m1", "O1001"),
    (
        "examples/fail_mem_recursive_value_m1",
        "guard the recursive edge with owned heap indirection",
    ),
    (
        "examples/fail_mem_heap_in_core_m1",
        "heap allocation binding requires heap support",
    ),
];

pub(super) const V3_MEM_M2_FAILURES: &[(&str, &str)] = &[
    ("examples/fail_mem_owner_while_borrowed_m2", "O2001"),
    ("examples/fail_mem_second_mut_borrow_m2", "O2002"),
    (
        "examples/fail_mem_mut_borrow_immutable_owner_m2",
        "O2003",
    ),
];

pub(super) const V3_MEM_M3_FAILURES: &[(&str, &str)] = &[
    ("examples/fail_mem_ptr_raw_m3", "V4 interop surface"),
    (
        "examples/fail_mem_ptr_in_core_m3",
        "pointer construction requires heap support",
    ),
];

fn diagnostic_matrix_paths() -> Vec<&'static str> {
    [
        V3_PROC_M1_FAILURES,
        V3_PROC_M2_FAILURES,
        V3_PROC_M3_FAILURES,
        V3_PROC_M4_FAILURES,
        V3_MEM_M1_FAILURES,
        V3_MEM_M2_FAILURES,
        V3_MEM_M3_FAILURES,
    ]
    .into_iter()
    .flatten()
    .map(|(path, _)| *path)
    .collect()
}

#[test]
fn every_v3_fail_example_is_in_the_lsp_diagnostic_matrix() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../examples");
    let actual = fs::read_dir(&examples)
        .expect("repository examples directory should be readable")
        .map(|entry| entry.expect("example directory entry should be readable"))
        .filter(|entry| {
            entry
                .file_type()
                .expect("example entry type should be readable")
                .is_dir()
        })
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            (name.starts_with("fail_mem_") || name.starts_with("fail_proc_"))
                .then(|| format!("examples/{name}"))
        })
        .collect::<BTreeSet<_>>();

    let matrix_paths = diagnostic_matrix_paths();
    let represented = matrix_paths
        .iter()
        .map(|path| (*path).to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        matrix_paths.len(),
        represented.len(),
        "V3 LSP diagnostic matrix must not list a fail example more than once"
    );
    assert_eq!(
        actual, represented,
        "every checked-in fail_mem_* and fail_proc_* package must be exercised by the LSP diagnostic matrix"
    );
}

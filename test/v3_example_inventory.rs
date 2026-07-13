use std::collections::BTreeSet;
use std::path::Path;

pub(crate) type V3PositiveExample = (&'static str, Option<&'static str>);
pub(crate) type V3FailureExample = (&'static str, &'static str, bool);
pub(crate) type V3NavigationProbe = (&'static str, &'static str, usize, Option<u32>);

pub(crate) const V3_MEM_M1_POSITIVES: &[V3PositiveExample] = &[
    ("examples/mem_owner_reinitialize_m1", None),
    ("examples/mem_set_observation_m1", None),
    ("examples/mem_when_bool_gate_m1", None),
    ("examples/mem_linked_list_m1", None),
    ("examples/mem_tree_m1", None),
    ("examples/mem_move_stack_vs_heap_m1", None),
];

pub(crate) const V3_MEM_M2_POSITIVES: &[V3PositiveExample] = &[
    ("examples/mem_borrow_m2", None),
    ("examples/mem_borrow_giveback_m2", None),
    ("examples/mem_borrow_param_m2", None),
    ("examples/mem_mut_borrow_m2", None),
    ("examples/mem_edf_m2", Some("1\n1\n2\n")),
];

pub(crate) const V3_MEM_M3_POSITIVES: &[V3PositiveExample] = &[
    ("examples/mem_ptr_unique_m3", None),
    ("examples/mem_ptr_shared_m3", None),
    ("examples/mem_ptr_shared_recursive_m3", None),
];

pub(crate) const V3_PROC_M1_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_spawn_m1", Some("17\n")),
    ("examples/proc_spawn_move_heap_m1", Some("29\n")),
];

pub(crate) const V3_PROC_M2_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_channel_m2", Some("42\n")),
    ("examples/proc_channel_pull_m2", Some("41\n")),
    ("examples/proc_channel_capture_m2", Some("42\n")),
    ("examples/proc_channel_loop_m2", Some("42\n")),
];

pub(crate) const V3_PROC_M3_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_select_m3", Some("42\n")),
    ("examples/proc_mutex_m3", Some("1\n2\n")),
    (
        "examples/proc_mutex_explicit_unlock_m3",
        Some("42\n"),
    ),
];

pub(crate) const V3_PROC_M4_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_async_await_m4", Some("42\n")),
    ("examples/proc_await_error_m4", Some("42\n")),
];

pub(crate) const V3_MEM_M1_FAILURES: &[V3FailureExample] = &[
    ("examples/fail_mem_use_after_move_m1", "O1001", false),
    (
        "examples/fail_mem_discarded_move_m1",
        "use of moved heap-owned binding 'pointer'",
        false,
    ),
    (
        "examples/fail_mem_deferred_reinit_m1",
        "moved binding 'pointer' cannot be reinitialized inside dfr/edf",
        false,
    ),
    (
        "examples/fail_mem_recursive_value_m1",
        "guard the recursive edge with owned heap indirection",
        false,
    ),
    (
        "examples/fail_mem_heap_in_core_m1",
        "heap allocation binding requires heap support",
        false,
    ),
];

pub(crate) const V3_MEM_M2_FAILURES: &[V3FailureExample] = &[
    ("examples/fail_mem_owner_while_borrowed_m2", "O2001", false),
    ("examples/fail_mem_second_mut_borrow_m2", "O2002", false),
    (
        "examples/fail_mem_mut_borrow_immutable_owner_m2",
        "O2003",
        false,
    ),
    ("examples/fail_mem_borrow_reuse_m2", "O2004", false),
];

pub(crate) const V3_MEM_M3_FAILURES: &[V3FailureExample] = &[
    (
        "examples/fail_mem_ptr_raw_m3",
        "V4 interop surface",
        false,
    ),
    (
        "examples/fail_mem_ptr_in_core_m3",
        "pointer construction requires heap support",
        false,
    ),
    (
        "examples/fail_mem_pointer_field_deref_m3",
        "dereferencing through a move-only field projection",
        false,
    ),
    (
        "examples/fail_mem_shared_ptr_write_m3",
        "cannot write through ptr[shared, T]; shared pointers are read-only",
        false,
    ),
];

pub(crate) const V3_PROC_M1_FAILURES: &[V3FailureExample] = &[
    (
        "examples/fail_proc_spawn_in_core_m1",
        "spawn requires hosted std support",
        false,
    ),
    (
        "examples/fail_proc_spawn_in_memo_m1",
        "spawn requires hosted std support",
        false,
    ),
    (
        "examples/fail_proc_spawn_rc_cross_m1",
        "shared Rc pointers cannot cross a spawn",
        true,
    ),
    (
        "examples/fail_proc_spawn_recoverable_m1",
        "spawning a recoverable routine without await discards its error",
        true,
    ),
    (
        "examples/fail_proc_spawn_heap_use_after_move_m1",
        "use of moved heap-owned binding 'owned'",
        true,
    ),
    (
        "examples/fail_proc_spawn_indirect_m1",
        "spawn requires a direct call to a named routine declaration in V3",
        true,
    ),
];

pub(crate) const V3_PROC_M2_FAILURES: &[V3FailureExample] = &[
    (
        "examples/fail_proc_channel_index_m2",
        "channel receivers are blocking pull expressions and cannot be indexed",
        true,
    ),
    (
        "examples/fail_proc_channel_in_core_m2",
        "channel types require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_channel_in_memo_m2",
        "channel types require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_channel_capture_rx_m2",
        "captured endpoint 'channel[tx]' is sender-only",
        true,
    ),
    (
        "examples/fail_proc_channel_spawn_consumer_m2",
        "routine 'consume' receives from a channel and cannot be spawned directly",
        true,
    ),
];

pub(crate) const V3_PROC_M3_FAILURES: &[V3FailureExample] = &[
    (
        "examples/fail_proc_select_old_form_m3",
        "old select(channel as binding) form is not supported",
        true,
    ),
    (
        "examples/fail_proc_select_in_core_m3",
        "select requires hosted std support",
        false,
    ),
    (
        "examples/fail_proc_select_in_memo_m3",
        "select requires hosted std support",
        false,
    ),
    (
        "examples/fail_proc_mutex_double_paren_m3",
        "Expected generic parameter name",
        true,
    ),
    (
        "examples/fail_proc_mutex_in_core_m3",
        "mutex parameters require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_mutex_in_memo_m3",
        "mutex parameters require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_mutex_deferred_m3",
        "mutex field access through 'counter' is not allowed inside dfr/edf",
        true,
    ),
    (
        "examples/fail_proc_mutex_deferred_lock_m3",
        "mutex .lock() is not allowed inside dfr/edf",
        true,
    ),
    (
        "examples/fail_proc_mutex_deferred_unlock_m3",
        "mutex .unlock() is not allowed inside dfr/edf",
        true,
    ),
    (
        "examples/fail_proc_mutex_deferred_forward_m3",
        "mutex handles cannot be forwarded to [mux] parameter",
        true,
    ),
];

pub(crate) const V3_PROC_M4_FAILURES: &[V3FailureExample] = &[
    (
        "examples/fail_proc_evt_named_m4",
        "eventual types are internal in V3 and cannot be named",
        true,
    ),
    (
        "examples/fail_proc_async_in_core_m4",
        "async pipe stages require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_async_in_memo_m4",
        "async pipe stages require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_await_in_core_m4",
        "await pipe stages require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_await_in_memo_m4",
        "await pipe stages require hosted std support",
        false,
    ),
    (
        "examples/fail_proc_async_indirect_m4",
        "async requires a direct call to a named routine declaration in V3",
        true,
    ),
];

pub(crate) const V3_POSITIVE_GROUPS: &[&[V3PositiveExample]] = &[
    V3_MEM_M1_POSITIVES,
    V3_MEM_M2_POSITIVES,
    V3_MEM_M3_POSITIVES,
    V3_PROC_M1_POSITIVES,
    V3_PROC_M2_POSITIVES,
    V3_PROC_M3_POSITIVES,
    V3_PROC_M4_POSITIVES,
];

pub(crate) const V3_FAILURE_GROUPS: &[&[V3FailureExample]] = &[
    V3_MEM_M1_FAILURES,
    V3_MEM_M2_FAILURES,
    V3_MEM_M3_FAILURES,
    V3_PROC_M1_FAILURES,
    V3_PROC_M2_FAILURES,
    V3_PROC_M3_FAILURES,
    V3_PROC_M4_FAILURES,
];

pub(crate) const V3_NAVIGATION_PROBES: &[V3NavigationProbe] = &[
    ("examples/mem_owner_reinitialize_m1", "pointer", 4, Some(7)),
    ("examples/mem_set_observation_m1", "values", 2, Some(1)),
    ("examples/mem_when_bool_gate_m1", "gated", 2, Some(1)),
    ("examples/mem_linked_list_m1", "head", 2, Some(7)),
    ("examples/mem_tree_m1", "root", 2, Some(9)),
    ("examples/mem_move_stack_vs_heap_m1", "heap_a", 2, Some(8)),
    ("examples/mem_borrow_m2", "view", 2, Some(8)),
    ("examples/mem_borrow_giveback_m2", "view", 2, Some(6)),
    ("examples/mem_borrow_param_m2", "inspect", 2, Some(4)),
    ("examples/mem_mut_borrow_m2", "view", 2, Some(6)),
    ("examples/mem_edf_m2", "probe", 2, Some(2)),
    ("examples/mem_ptr_unique_m3", "pointer", 2, Some(2)),
    ("examples/mem_ptr_shared_m3", "first", 2, Some(2)),
    (
        "examples/mem_ptr_shared_recursive_m3",
        "tail_ptr",
        2,
        Some(7),
    ),
    ("examples/proc_spawn_m1", "worker", 2, None),
    ("examples/proc_spawn_move_heap_m1", "consume", 2, None),
    ("examples/proc_channel_m2", "produce", 2, None),
    ("examples/proc_channel_pull_m2", "channel", 2, None),
    ("examples/proc_channel_capture_m2", "channel", 3, None),
    ("examples/proc_channel_loop_m2", "channel", 2, None),
    ("examples/proc_select_m3", "first", 2, None),
    ("examples/proc_mutex_m3", "worker", 2, None),
    (
        "examples/proc_mutex_explicit_unlock_m3",
        "update",
        2,
        None,
    ),
    ("examples/proc_async_await_m4", "work", 2, None),
    ("examples/proc_await_error_m4", "probe", 2, None),
];

pub(crate) fn positive_example_paths() -> Vec<&'static str> {
    V3_POSITIVE_GROUPS
        .iter()
        .flat_map(|group| group.iter().map(|(path, _)| *path))
        .collect()
}

pub(crate) fn failure_example_paths() -> Vec<&'static str> {
    V3_FAILURE_GROUPS
        .iter()
        .flat_map(|group| group.iter().map(|(path, _, _)| *path))
        .collect()
}

pub(crate) fn assert_checked_in_example_directories(repo_root: &Path) {
    let entries = std::fs::read_dir(repo_root.join("examples"))
        .expect("repository examples directory should be readable")
        .map(|entry| entry.expect("example directory entry should be readable"))
        .filter(|entry| {
            entry
                .file_type()
                .expect("example entry type should be readable")
                .is_dir()
        })
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();

    let actual_failures = entries
        .iter()
        .filter(|name| name.starts_with("fail_mem_") || name.starts_with("fail_proc_"))
        .map(|name| format!("examples/{name}"))
        .collect::<BTreeSet<_>>();
    let actual_positives = entries
        .iter()
        .filter(|name| name.starts_with("mem_") || name.starts_with("proc_"))
        .map(|name| format!("examples/{name}"))
        .collect::<BTreeSet<_>>();

    let failure_paths = failure_example_paths();
    let represented_failures = failure_paths
        .iter()
        .map(|path| (*path).to_string())
        .collect::<BTreeSet<_>>();
    let positive_paths = positive_example_paths();
    let represented_positives = positive_paths
        .iter()
        .map(|path| (*path).to_string())
        .collect::<BTreeSet<_>>();
    let navigation_paths = V3_NAVIGATION_PROBES
        .iter()
        .map(|(path, _, _, _)| (*path).to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        failure_paths.len(),
        represented_failures.len(),
        "the canonical V3 failure inventory must not list an example more than once"
    );
    assert_eq!(
        positive_paths.len(),
        represented_positives.len(),
        "the canonical V3 positive inventory must not list an example more than once"
    );
    assert_eq!(
        V3_NAVIGATION_PROBES.len(),
        navigation_paths.len(),
        "the canonical V3 navigation inventory must not list an example more than once"
    );
    assert_eq!(
        represented_positives, navigation_paths,
        "every canonical V3 positive example must declare one navigation probe"
    );
    assert_eq!(
        actual_failures, represented_failures,
        "checked-in fail_mem_* and fail_proc_* packages must exactly match the canonical V3 failure inventory"
    );
    assert_eq!(
        actual_positives, represented_positives,
        "checked-in mem_* and proc_* packages must exactly match the canonical V3 positive inventory"
    );

    let published_memory = published_example_paths(
        &std::fs::read_to_string(repo_root.join("book/src/800_memory/_index.md"))
            .expect("published V3 memory inventory should be readable"),
        &["examples/mem_", "examples/fail_mem_"],
    );
    let represented_memory = represented_positives
        .iter()
        .chain(represented_failures.iter())
        .filter(|path| path.starts_with("examples/mem_") || path.starts_with("examples/fail_mem_"))
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        published_memory, represented_memory,
        "the published V3 memory inventory must exactly match the canonical machine inventory"
    );

    let published_processor = published_example_paths(
        &std::fs::read_to_string(repo_root.join("book/src/900_processor/_index.md"))
            .expect("published V3 processor inventory should be readable"),
        &["examples/proc_", "examples/fail_proc_"],
    );
    let represented_processor = represented_positives
        .iter()
        .chain(represented_failures.iter())
        .filter(|path| {
            path.starts_with("examples/proc_") || path.starts_with("examples/fail_proc_")
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        published_processor, represented_processor,
        "the published V3 processor inventory must exactly match the canonical machine inventory"
    );
}

fn published_example_paths(markdown: &str, prefixes: &[&str]) -> BTreeSet<String> {
    markdown
        .split('`')
        .filter(|part| prefixes.iter().any(|prefix| part.starts_with(prefix)))
        .map(str::to_string)
        .collect()
}

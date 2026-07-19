use std::collections::BTreeSet;
use std::path::Path;

pub(crate) type V3PositiveExample = (&'static str, Option<&'static str>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct V3FailureExample {
    pub(crate) path: &'static str,
    /// Code surfaced by package/frontend checking.
    pub(crate) code: &'static str,
    /// Code surfaced by direct compiler-backed editor analysis.
    pub(crate) lsp_code: &'static str,
    /// Stable diagnostic substring. Some compiler messages add source-specific detail.
    pub(crate) message_contains: &'static str,
    pub(crate) needs_std: bool,
    pub(crate) expect_related_site: bool,
}

macro_rules! v3_failure {
    ($path:literal, $code:literal, $message:literal, $needs_std:literal, $related:literal) => {
        V3FailureExample {
            path: $path,
            code: $code,
            lsp_code: $code,
            message_contains: $message,
            needs_std: $needs_std,
            expect_related_site: $related,
        }
    };
}

macro_rules! v3_failure_codes {
    ($path:literal, $code:literal, $lsp_code:literal, $message:literal, $needs_std:literal, $related:literal) => {
        V3FailureExample {
            path: $path,
            code: $code,
            lsp_code: $lsp_code,
            message_contains: $message,
            needs_std: $needs_std,
            expect_related_site: $related,
        }
    };
}

pub(crate) type V3NavigationProbe = (&'static str, &'static str, usize, Option<u32>);

pub(crate) const V3_MEM_M1_POSITIVES: &[V3PositiveExample] = &[
    ("examples/mem_owner_reinitialize_m1", None),
    ("examples/mem_set_observation_m1", None),
    ("examples/mem_when_bool_gate_m1", None),
    ("examples/mem_linked_list_m1", None),
    ("examples/mem_tree_m1", None),
    ("examples/mem_move_stack_vs_heap_m1", None),
    ("examples/mem_partial_move_m1", None),
    ("examples/mem_fin_finalizer_m1", Some("3\n7\n")),
    ("examples/mem_fin_move_m1", Some("88\n5\n")),
    ("examples/mem_fin_early_m1", Some("3\n7\n9\n")),
];

pub(crate) const V3_MEM_M2_POSITIVES: &[V3PositiveExample] = &[
    ("examples/mem_borrow_m2", None),
    ("examples/mem_borrow_giveback_m2", None),
    ("examples/mem_borrow_param_m2", None),
    ("examples/mem_mut_borrow_m2", None),
    ("examples/mem_ownership_ops_m2", None),
    ("examples/mem_capabilities_m2", None),
    ("examples/mem_copy_clone_m2", None),
    ("examples/mem_custom_clone_m2", Some("11\n")),
    ("examples/mem_borrow_receiver_m2", Some("10\n")),
    ("examples/mem_mut_receiver_m2", Some("42\n")),
    ("examples/mem_receiver_ops_m2", Some("42\n")),
    ("examples/mem_reborrow_m2", None),
    ("examples/mem_nll_last_use_m2", None),
    ("examples/mem_named_lifetime_m2", None),
    ("examples/mem_temp_borrow_m2", None),
    ("examples/mem_place_borrow_m2", None),
    ("examples/mem_edf_m2", Some("1\n1\n2\n")),
    ("examples/mem_dfr_capture_m2", Some("42\n")),
    ("examples/mem_closure_capture_m2", Some("42\n30\n30\n")),
];

pub(crate) const V3_MEM_M3_POSITIVES: &[V3PositiveExample] = &[
    ("examples/mem_ptr_unique_m3", None),
    ("examples/mem_ptr_shared_m3", None),
    ("examples/mem_ptr_shared_recursive_m3", None),
    ("examples/mem_ptr_weak_m3", None),
    ("examples/mem_ptr_weak_clone_m3", None),
    ("examples/mem_ptr_weak_upgrade_m3", None),
    ("examples/mem_ptr_weak_cycle_m3", None),
    ("examples/mem_ptr_weak_nested_m3", None),
    ("examples/mem_shell_access_m3", None),
    ("examples/mem_shell_prefix_m3", None),
];

pub(crate) const V3_PROC_M1_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_spawn_m1", Some("17\n")),
    ("examples/proc_spawn_move_heap_m1", Some("29\n")),
    ("examples/proc_spawn_canonical_m1", Some("17\n")),
    ("examples/proc_spawn_detached_m1", Some("41\n")),
    ("examples/proc_shared_sync_ptr_m1", Some("42\n")),
];

pub(crate) const V3_PROC_M2_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_channel_m2", Some("42\n")),
    ("examples/proc_channel_pull_m2", Some("41\n")),
    ("examples/proc_channel_capture_m2", Some("42\n")),
    ("examples/proc_sender_endpoint_m2", Some("37\n")),
    ("examples/proc_receiver_endpoint_m2", Some("37\n")),
    ("examples/proc_channel_loop_m2", Some("42\n")),
];

pub(crate) const V3_PROC_M3_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_select_m3", Some("42\n")),
    ("examples/proc_mutex_m3", Some("1\n2\n")),
    ("examples/proc_mutex_explicit_unlock_m3", Some("42\n")),
    ("examples/proc_mutex_local_m3", Some("42\n")),
    ("examples/proc_mutex_guard_m3", Some("42\n")),
    ("examples/proc_mutex_guard_end_m3", Some("43\n")),
];

pub(crate) const V3_PROC_M4_POSITIVES: &[V3PositiveExample] = &[
    ("examples/proc_async_await_m4", Some("42\n")),
    ("examples/proc_await_error_m4", Some("42\n")),
    ("examples/proc_evt_named_m4", Some("42\n")),
    ("examples/proc_evt_lifetime_m4", Some("42\n")),
];

pub(crate) const V3_MEM_M1_FAILURES: &[V3FailureExample] = &[
    v3_failure!(
        "examples/fail_mem_use_after_move_m1",
        "O1001",
        "use of moved heap-owned binding 'owner'",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_loop_move_m1",
        "O1001",
        "move-only binding 'slot' declared outside a repeating loop cannot be transferred from the loop body",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_explicit_move_reuse_m1",
        "O1001",
        "use of moved binding 'slot'",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_discarded_move_m1",
        "O1001",
        "use of moved heap-owned binding 'pointer'",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_deferred_reinit_m1",
        "O1001",
        "moved binding 'pointer' cannot be reinitialized inside dfr/edf",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_recursive_value_m1",
        "T1002",
        "guard the recursive edge with owned heap indirection",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_heap_in_core_m1",
        "T1002",
        "heap allocation binding requires heap support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_uninit_borrow_m1",
        "O2005",
        "borrow binding 'view' requires an initializer",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_partial_move_m1",
        "O1001",
        "use of partially moved binding 'bundle'",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_global_fin_m1",
        "T1002",
        "top-level bindings of a 'fin' type are forbidden",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_fin_partial_move_m1",
        "O1001",
        "cannot partially move field '.held' out of the 'fin' value 'res'",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_fin_fun_finalizer_m1",
        "T1001",
        "a custom finalizer 'finalize' must be declared 'pro', not 'fun'",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_fin_reuse_m1",
        "O1001",
        "use of moved heap-owned binding 'handle'",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_clone_fin_m1",
        "O1001",
        "'[cln]' requires the 'clone' capability",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_clone_fin_claim_m1",
        "T1001",
        "a type that claims 'clone' cannot itself be a non-clonable value",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_generic_copy_bound_m1",
        "T1003",
        "to satisfy the 'copy' capability for generic parameter 'T'",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_generic_send_bound_transitive_m1",
        "T1003",
        "to satisfy the 'send' capability for generic parameter 'T'",
        false,
        false
    ),
];

pub(crate) const V3_MEM_M2_FAILURES: &[V3FailureExample] = &[
    v3_failure!(
        "examples/fail_mem_deferred_report_m2",
        "T1001",
        "report is not allowed inside dfr/edf blocks",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_dfr_capture_bare_m2",
        "T1002",
        "must state its operation",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_dfr_capture_undeclared_m2",
        "O1001",
        "is not declared in its capture list",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_closure_move_only_m2",
        "O1001",
        "cannot place a move-only value in a routine value",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_copy_moveonly_m2",
        "O1001",
        "'[cpy]' requires the 'copy' capability",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_copy_record_field_m2",
        "O1001",
        "is not copy-safe",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_send_field_m2",
        "T1001",
        "'send' requires every field to be send-safe",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_send_nested_field_m2",
        "T1001",
        "'send' is verified recursively",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_clone_nested_field_m2",
        "T1001",
        "'clone' is verified recursively",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_share_field_m2",
        "T1001",
        "'share' requires every field to be share-safe",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_owner_while_borrowed_m2",
        "O2001",
        "owner 'owner' is inaccessible while borrowed",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_second_mut_borrow_m2",
        "O2002",
        "borrow conflicts with an active mutable borrow of the same owner",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_mut_borrow_immutable_owner_m2",
        "O2003",
        "mutable borrow requires an owner declared with 'var[mut]'",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_borrow_reuse_m2",
        "O2004",
        "borrow binding 'view' was already returned",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_conditional_giveback_m2",
        "O2001",
        "owner 'owner' is inaccessible while borrowed",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_deferred_giveback_m2",
        "O2004",
        "captured by a deferred (dfr/edf) block and cannot be given back early",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_ownership_op_combo_m2",
        "T1001",
        "an ownership operation needs exactly one source",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_copy_fin_conflict_m2",
        "T1001",
        "a type cannot claim both 'copy' and 'fin'",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_copy_field_m2",
        "T1001",
        "'copy' requires every field to be copy-safe",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_copy_nested_field_m2",
        "T1001",
        "'copy' is verified recursively",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_copy_operand_unclaimed_m2",
        "O1001",
        "does not claim 'copy'",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_fun_mut_receiver_m2",
        "T1001",
        "a 'fun' cannot take a mutable '[mut, bor]' receiver",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_temp_borrow_escape_m2",
        "O1001",
        "cannot return a borrow of a temporary value",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_place_borrow_owner_m2",
        "O2001",
        "owner 'o' is inaccessible while borrowed",
        false,
        true
    ),
    v3_failure!(
        "examples/fail_mem_escaping_borrow_m2",
        "O1001",
        "cannot return a borrow of the owned local 'job'",
        false,
        false
    ),
];

pub(crate) const V3_MEM_M3_FAILURES: &[V3FailureExample] = &[
    v3_failure!(
        "examples/fail_mem_ptr_raw_m3",
        "T1002",
        "V4 interop surface",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_ptr_weak_m3",
        "O1001",
        "a weak pointer 'ptr[weak, T]' cannot be dereferenced directly",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_shell_access_m3",
        "T1001",
        "inner-place access '[]' requires a pointer, 'opt[T]', or 'err[T]' receiver",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_ptr_in_core_m3",
        "T1002",
        "pointer construction requires heap support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_pointer_field_deref_m3",
        "O1001",
        "dereferencing through a move-only field projection",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_shared_ptr_write_m3",
        "T1001",
        "cannot write through ptr[shared, T]; shared pointers are read-only",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_shared_ptr_move_deref_m3",
        "O1001",
        "cannot move a move-only pointee out of ptr[shared, T]",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_mem_borrowed_ptr_move_deref_m3",
        "O1001",
        "cannot move a move-only pointee through a borrowed pointer",
        false,
        false
    ),
];

pub(crate) const V3_PROC_M1_FAILURES: &[V3FailureExample] = &[
    v3_failure!(
        "examples/fail_proc_spawn_in_core_m1",
        "T1002",
        "spawn requires hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_spawn_fin_m1",
        "O1001",
        "a 'fin' value cannot cross a spawn or async task boundary",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_spawn_in_memo_m1",
        "T1002",
        "spawn requires hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_spawn_rc_cross_m1",
        "O1001",
        "shared Rc pointers cannot cross a spawn",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_spawn_recoverable_m1",
        "T1002",
        "cannot spawn a recoverable routine",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_spawn_heap_use_after_move_m1",
        "O1001",
        "use of moved heap-owned binding 'owned'",
        true,
        true
    ),
    v3_failure!(
        "examples/fail_proc_spawn_indirect_m1",
        "T1002",
        "spawn requires a direct call to a named routine declaration in V3",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_spawn_detached_borrow_m1",
        "O1001",
        "borrowed values cannot cross a spawn or async thread boundary",
        true,
        false
    ),
];

pub(crate) const V3_PROC_M2_FAILURES: &[V3FailureExample] = &[
    v3_failure!(
        "examples/fail_proc_channel_index_m2",
        "T1002",
        "channel receivers are blocking pull expressions and cannot be indexed",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_clone_receiver_m2",
        "O1001",
        "'[cln]' requires the 'clone' capability",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_channel_in_core_m2",
        "T1002",
        "channel types require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_channel_in_memo_m2",
        "T1002",
        "channel types require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_channel_capture_rx_m2",
        "T1002",
        "captured endpoint 'channel[tx]' is sender-only",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_channel_spawn_consumer_m2",
        "T1002",
        "routine 'consume' receives from a channel and cannot be spawned directly",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_channel_receive_bare_m2",
        "T1003",
        "expects 'int' but got 'opt[int]'",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_channel_bare_send_m2",
        "T1002",
        "a channel send returns a must-handle 'err[T]'",
        true,
        false
    ),
];

pub(crate) const V3_PROC_M3_FAILURES: &[V3FailureExample] = &[
    v3_failure_codes!(
        "examples/fail_proc_select_old_form_m3",
        "K1001",
        "P1001",
        "old select(channel as binding) form is not supported",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_select_in_core_m3",
        "T1002",
        "select requires hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_select_in_memo_m3",
        "T1002",
        "select requires hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_select_empty_m3",
        "T1001",
        "blocking select requires at least one channel arm",
        true,
        false
    ),
    v3_failure_codes!(
        "examples/fail_proc_mutex_double_paren_m3",
        "K1001",
        "P1001",
        "Expected generic parameter name",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_mutex_in_core_m3",
        "T1002",
        "mutex parameters require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_mutex_in_memo_m3",
        "T1002",
        "mutex parameters require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_mutex_deferred_m3",
        "T1002",
        "mutex field access through 'counter' is not allowed inside dfr/edf",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_mutex_deferred_lock_m3",
        "T1002",
        "mutex .lock() is not allowed inside dfr/edf",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_mutex_deferred_unlock_m3",
        "T1002",
        "mutex .unlock() is not allowed inside dfr/edf",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_mutex_deferred_forward_m3",
        "T1002",
        "mutex handles cannot be forwarded to mux[T] parameter",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_mutex_guard_await_m3",
        "O1001",
        "a mutex guard cannot cross await",
        true,
        true
    ),
    v3_failure!(
        "examples/fail_proc_mutex_guard_move_m3",
        "O1001",
        "a mutex guard cannot be moved, copied, or cloned",
        true,
        false
    ),
];

pub(crate) const V3_PROC_M4_FAILURES: &[V3FailureExample] = &[
    v3_failure!(
        "examples/fail_proc_evt_detached_m4",
        "O1001",
        "an eventual handle cannot enter a detached task",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_evt_embedded_m4",
        "T1002",
        "evt[T] values cannot be embedded in aggregate",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_in_core_m4",
        "T1002",
        "async pipe stages require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_in_memo_m4",
        "T1002",
        "async pipe stages require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_await_in_core_m4",
        "T1002",
        "await pipe stages require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_await_in_memo_m4",
        "T1002",
        "await pipe stages require hosted std support",
        false,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_indirect_m4",
        "T1002",
        "async requires a direct call to a named routine declaration in V3",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_recoverable_discard_m4",
        "T1001",
        "discarding a recoverable eventual loses its error",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_recoverable_unawaited_m4",
        "T1001",
        "recoverable eventual binding 'pending' must be awaited and handled",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_await_recoverable_discard_m4",
        "T1001",
        "statement-position expression cannot use '/ ErrorType'",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_recoverable_overwrite_m4",
        "T1001",
        "recoverable eventual binding 'pending' cannot be overwritten",
        true,
        true
    ),
    v3_failure!(
        "examples/fail_proc_async_break_outer_m4",
        "T1001",
        "recoverable eventual binding 'slot' must be awaited and handled",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_edf_await_m4",
        "T1002",
        "await is not allowed inside edf",
        true,
        false
    ),
    v3_failure!(
        "examples/fail_proc_async_nested_capture_m4",
        "T1002",
        "implicit closure capture of outer local 'pending' is not supported",
        true,
        false
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
    ("examples/mem_ownership_ops_m2", "look", 2, Some(10)),
    ("examples/mem_capabilities_m2", "origin", 2, Some(10)),
    ("examples/mem_borrow_receiver_m2", "origin", 2, Some(17)),
    ("examples/mem_mut_receiver_m2", "counter", 2, Some(17)),
    ("examples/mem_receiver_ops_m2", "tally", 2, Some(12)),
    ("examples/mem_copy_clone_m2", "origin", 2, Some(14)),
    ("examples/mem_custom_clone_m2", "original", 2, Some(16)),
    ("examples/mem_reborrow_m2", "nested", 2, Some(5)),
    ("examples/mem_partial_move_m1", "bundle", 2, Some(13)),
    ("examples/mem_fin_finalizer_m1", "handle", 2, Some(16)),
    ("examples/mem_fin_move_m1", "handle", 2, Some(19)),
    ("examples/mem_fin_early_m1", "handle", 2, Some(18)),
    ("examples/mem_nll_last_use_m2", "view", 2, Some(9)),
    ("examples/mem_named_lifetime_m2", "left", 2, Some(6)),
    ("examples/mem_temp_borrow_m2", "seen", 2, Some(11)),
    ("examples/mem_place_borrow_m2", "part", 2, Some(9)),
    ("examples/mem_edf_m2", "probe", 2, Some(2)),
    ("examples/mem_dfr_capture_m2", "seen", 3, Some(11)),
    ("examples/mem_closure_capture_m2", "base", 4, Some(9)),
    ("examples/mem_ptr_unique_m3", "outer", 2, Some(3)),
    ("examples/mem_ptr_shared_m3", "first", 2, Some(2)),
    ("examples/mem_ptr_weak_m3", "strong", 2, Some(6)),
    ("examples/mem_ptr_weak_clone_m3", "mirror", 2, Some(9)),
    ("examples/mem_ptr_weak_upgrade_m3", "observer", 2, Some(8)),
    ("examples/mem_ptr_weak_cycle_m3", "revived", 2, Some(16)),
    ("examples/mem_ptr_weak_nested_m3", "registry", 2, Some(19)),
    ("examples/mem_shell_access_m3", "slot", 2, Some(10)),
    ("examples/mem_shell_prefix_m3", "handle", 2, Some(11)),
    (
        "examples/mem_ptr_shared_recursive_m3",
        "tail_ptr",
        2,
        Some(7),
    ),
    ("examples/proc_spawn_m1", "worker", 2, None),
    ("examples/proc_spawn_canonical_m1", "worker", 2, None),
    ("examples/proc_spawn_detached_m1", "worker", 2, None),
    ("examples/proc_shared_sync_ptr_m1", "observe", 2, None),
    ("examples/proc_spawn_move_heap_m1", "consume", 2, None),
    ("examples/proc_channel_m2", "produce", 2, None),
    ("examples/proc_channel_pull_m2", "channel", 2, None),
    ("examples/proc_channel_capture_m2", "channel", 3, None),
    ("examples/proc_sender_endpoint_m2", "emit", 2, None),
    ("examples/proc_receiver_endpoint_m2", "drain", 2, None),
    ("examples/proc_channel_loop_m2", "channel", 2, None),
    ("examples/proc_select_m3", "first", 2, None),
    ("examples/proc_mutex_m3", "worker", 2, None),
    ("examples/proc_mutex_explicit_unlock_m3", "update", 2, None),
    ("examples/proc_mutex_local_m3", "state", 2, None),
    ("examples/proc_mutex_guard_m3", "bump", 2, None),
    ("examples/proc_mutex_guard_end_m3", "bump", 2, None),
    ("examples/proc_async_await_m4", "work", 2, None),
    ("examples/proc_await_error_m4", "probe", 2, None),
    ("examples/proc_evt_named_m4", "work", 2, None),
    ("examples/proc_evt_lifetime_m4", "schedule", 2, None),
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
        .flat_map(|group| group.iter().map(|failure| failure.path))
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

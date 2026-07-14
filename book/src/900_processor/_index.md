# Concurrency

This section covers FOL's shipped V3 concurrency and asynchronous execution
model. Every processor surface is hosted `std`-only and is implemented with OS
threads and Rust standard-library synchronization; FOL does not use a separate
async runtime.

Here `std`-only describes source capability: the artifact uses
`fol_model = "memo"` and the package explicitly declares the bundled internal
`standard` dependency. It does not mean FOL needs `std` merely to launch a
program. Host-compatible `core` and unhosted `memo` executables can run too;
they simply cannot use processor constructs.

The current chapter split is:

- eventuals
- tasks, channels, `select`, and mutex parameters

Together they define the language-level model for task execution, coordination,
and concurrent ownership boundaries.

The processor pillar is not complete at compiler acceptance. Each row below is
also guarded through lowering/runtime behavior, evaluated frontend capability
routing, structured diagnostics and explanations, formatter/tool commands, LSP
diagnostics/navigation/completion/tokens, Tree-sitter grammar/queries/corpus,
tests, docs, and the book. The exact cross-layer mapping lives in
[Compiler Integration](../050_tooling/350_compiler_integration.md#end-to-end-feature-completeness)
and the repository-level `docs/editor-sync.md` matrix.

## Shipped example inventory

This is the canonical processor example inventory. The milestone chapters and
the V3 processor plan link here instead of maintaining smaller lists that can
drift. Adding, removing, or renaming a processor example requires updating this
published list and the shared machine inventory in
`test/v3_example_inventory.rs` in the same change; compiler integration and
editor tests consume that shared machine inventory.

### P1: spawn

Positive:

- `examples/proc_spawn_m1`
- `examples/proc_spawn_move_heap_m1`

Negative:

- `examples/fail_proc_spawn_heap_use_after_move_m1`
- `examples/fail_proc_spawn_in_core_m1`
- `examples/fail_proc_spawn_indirect_m1`
- `examples/fail_proc_spawn_in_memo_m1`
- `examples/fail_proc_spawn_rc_cross_m1`
- `examples/fail_proc_spawn_recoverable_m1`

### P2: channels

Positive:

- `examples/proc_channel_capture_m2`
- `examples/proc_channel_loop_m2`
- `examples/proc_channel_m2`
- `examples/proc_channel_pull_m2`

Negative:

- `examples/fail_proc_channel_capture_rx_m2`
- `examples/fail_proc_channel_in_core_m2`
- `examples/fail_proc_channel_in_memo_m2`
- `examples/fail_proc_channel_index_m2`
- `examples/fail_proc_channel_spawn_consumer_m2`

### P3: select and mutexes

Positive:

- `examples/proc_mutex_explicit_unlock_m3`
- `examples/proc_mutex_m3`
- `examples/proc_select_m3`

Negative:

- `examples/fail_proc_mutex_deferred_m3`
- `examples/fail_proc_mutex_deferred_forward_m3`
- `examples/fail_proc_mutex_deferred_lock_m3`
- `examples/fail_proc_mutex_deferred_unlock_m3`
- `examples/fail_proc_mutex_double_paren_m3`
- `examples/fail_proc_mutex_in_core_m3`
- `examples/fail_proc_mutex_in_memo_m3`
- `examples/fail_proc_select_in_core_m3`
- `examples/fail_proc_select_in_memo_m3`
- `examples/fail_proc_select_old_form_m3`

### P4: eventuals

Positive:

- `examples/proc_async_await_m4`
- `examples/proc_await_error_m4`

Negative:

- `examples/fail_proc_async_in_core_m4`
- `examples/fail_proc_async_indirect_m4`
- `examples/fail_proc_async_in_memo_m4`
- `examples/fail_proc_await_in_core_m4`
- `examples/fail_proc_await_in_memo_m4`
- `examples/fail_proc_evt_named_m4`

# Memory Model

This section explains FOL's shipped V3 memory model: ownership, lexical
borrowing, owned allocation, and typed unique/shared pointers.

The main topics are:

- ownership
- pointers
- stack vs heap intuition
- allocation lifetime
- cross-thread memory concerns

The detailed chapters are the normative source. This index also keeps the
published example matrix aligned with the shared machine inventory in
`test/v3_example_inventory.rs`. Adding, removing, or renaming a `mem_*` or
`fail_mem_*` package requires updating both inventories in the same change.

The memory pillar is not complete at compiler acceptance. Each row below is
also guarded through lowering/runtime behavior, frontend capability routing,
structured diagnostics and explanations, formatter/tool commands, LSP
diagnostics/navigation/completion/tokens, Tree-sitter grammar/queries/corpus,
tests, docs, and the book. The exact cross-layer mapping lives in
[Compiler Integration](../050_tooling/350_compiler_integration.md#end-to-end-feature-completeness)
and the repository-level `docs/editor-sync.md` matrix.

## Shipped example inventory

### M1: ownership and owned allocation

Positive:

- `examples/mem_owner_reinitialize_m1`
- `examples/mem_set_observation_m1`
- `examples/mem_when_bool_gate_m1`
- `examples/mem_linked_list_m1`
- `examples/mem_tree_m1`
- `examples/mem_move_stack_vs_heap_m1`

Negative:

- `examples/fail_mem_use_after_move_m1`
- `examples/fail_mem_discarded_move_m1`
- `examples/fail_mem_deferred_reinit_m1`
- `examples/fail_mem_recursive_value_m1`
- `examples/fail_mem_heap_in_core_m1`

### M2: lexical borrowing and deferred ownership

Positive:

- `examples/mem_borrow_m2`
- `examples/mem_borrow_giveback_m2`
- `examples/mem_borrow_param_m2`
- `examples/mem_mut_borrow_m2`
- `examples/mem_edf_m2`

Negative:

- `examples/fail_mem_owner_while_borrowed_m2`
- `examples/fail_mem_second_mut_borrow_m2`
- `examples/fail_mem_mut_borrow_immutable_owner_m2`
- `examples/fail_mem_borrow_reuse_m2`

### M3: typed pointers

Positive:

- `examples/mem_ptr_unique_m3`
- `examples/mem_ptr_shared_m3`
- `examples/mem_ptr_shared_recursive_m3`

Negative:

- `examples/fail_mem_borrowed_ptr_move_deref_m3`
- `examples/fail_mem_ptr_raw_m3`
- `examples/fail_mem_ptr_in_core_m3`
- `examples/fail_mem_pointer_field_deref_m3`
- `examples/fail_mem_shared_ptr_move_deref_m3`
- `examples/fail_mem_shared_ptr_write_m3`

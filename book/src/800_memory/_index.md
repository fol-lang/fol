# Memory Model

> **V3 reopened — this chapter describes the first shipped subset, not the
> final contract.** The ownership model is being deepened into one sound,
> CFG-based system (explicit `[mov]`/`[cpy]`/`[cln]`/`[bor]` operations,
> non-lexical borrows, static places, named lifetimes, custom finalization, and
> a full pointer family) under `plan/V3_MEM.md`. Until that lands, treat the
> present-tense wording below as the historical subset described in
> `plan/V3_MEM.md` Appendix A.

This section explains FOL's first shipped V3 memory subset: ownership, lexical
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
- `examples/mem_partial_move_m1`
- `examples/mem_fin_finalizer_m1`
- `examples/mem_fin_move_m1`
- `examples/mem_fin_early_m1`

Negative:

- `examples/fail_mem_use_after_move_m1`
- `examples/fail_mem_loop_move_m1`
- `examples/fail_mem_explicit_move_reuse_m1`
- `examples/fail_mem_discarded_move_m1`
- `examples/fail_mem_deferred_reinit_m1`
- `examples/fail_mem_recursive_value_m1`
- `examples/fail_mem_heap_in_core_m1`
- `examples/fail_mem_uninit_borrow_m1`
- `examples/fail_mem_partial_move_m1`
- `examples/fail_mem_global_fin_m1`
- `examples/fail_mem_fin_partial_move_m1`
- `examples/fail_mem_fin_fun_finalizer_m1`
- `examples/fail_mem_fin_reuse_m1`
- `examples/fail_mem_clone_fin_m1`
- `examples/fail_mem_clone_fin_claim_m1`
- `examples/fail_mem_generic_copy_bound_m1`
- `examples/fail_mem_generic_send_bound_transitive_m1`

### M2: lexical borrowing and deferred ownership

Positive:

- `examples/mem_borrow_m2`
- `examples/mem_borrow_giveback_m2`
- `examples/mem_borrow_param_m2`
- `examples/mem_mut_borrow_m2`
- `examples/mem_ownership_ops_m2`
- `examples/mem_capabilities_m2`
- `examples/mem_copy_clone_m2`
- `examples/mem_custom_clone_m2`
- `examples/mem_borrow_receiver_m2`
- `examples/mem_mut_receiver_m2`
- `examples/mem_receiver_ops_m2`
- `examples/mem_reborrow_m2`
- `examples/mem_nll_last_use_m2`
- `examples/mem_named_lifetime_m2`
- `examples/mem_temp_borrow_m2`
- `examples/mem_place_borrow_m2`
- `examples/mem_edf_m2`
- `examples/mem_dfr_capture_m2`

Negative:

- `examples/fail_mem_deferred_report_m2`
- `examples/fail_mem_dfr_capture_bare_m2`
- `examples/fail_mem_copy_moveonly_m2`
- `examples/fail_mem_copy_record_field_m2`
- `examples/fail_mem_send_field_m2`
- `examples/fail_mem_send_nested_field_m2`
- `examples/fail_mem_clone_nested_field_m2`
- `examples/fail_mem_share_field_m2`
- `examples/fail_mem_owner_while_borrowed_m2`
- `examples/fail_mem_second_mut_borrow_m2`
- `examples/fail_mem_mut_borrow_immutable_owner_m2`
- `examples/fail_mem_borrow_reuse_m2`
- `examples/fail_mem_conditional_giveback_m2`
- `examples/fail_mem_deferred_giveback_m2`
- `examples/fail_mem_ownership_op_combo_m2`
- `examples/fail_mem_copy_fin_conflict_m2`
- `examples/fail_mem_copy_field_m2`
- `examples/fail_mem_copy_nested_field_m2`
- `examples/fail_mem_copy_operand_unclaimed_m2`
- `examples/fail_mem_fun_mut_receiver_m2`
- `examples/fail_mem_escaping_borrow_m2`
- `examples/fail_mem_temp_borrow_escape_m2`
- `examples/fail_mem_place_borrow_owner_m2`

### M3: typed pointers

Positive:

- `examples/mem_ptr_unique_m3`
- `examples/mem_ptr_shared_m3`
- `examples/mem_ptr_shared_recursive_m3`
- `examples/mem_ptr_weak_m3`
- `examples/mem_ptr_weak_clone_m3`
- `examples/mem_ptr_weak_upgrade_m3`
- `examples/mem_ptr_weak_cycle_m3`
- `examples/mem_ptr_weak_nested_m3`
- `examples/mem_shell_access_m3`
- `examples/mem_shell_prefix_m3`

Negative:

- `examples/fail_mem_borrowed_ptr_move_deref_m3`
- `examples/fail_mem_ptr_raw_m3`
- `examples/fail_mem_ptr_weak_m3`
- `examples/fail_mem_shell_access_m3`
- `examples/fail_mem_ptr_in_core_m3`
- `examples/fail_mem_pointer_field_deref_m3`
- `examples/fail_mem_shared_ptr_move_deref_m3`
- `examples/fail_mem_shared_ptr_write_m3`

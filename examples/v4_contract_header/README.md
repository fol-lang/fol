# Normative C header contract

`demo.h` is the frozen shape of a FOL-generated C header, and `install.txt` is
the frozen install layout. Both are checked-in references rather than build
outputs: nothing generates a header today.

They exist so that M5, which writes the emitter, has one shape to match instead
of a choice to make. `v4_contract_header_freezes_naming_guard_and_status` in
`test/v4_boundary_freeze.rs` asserts the parts a later change is most likely to
drift on.

See `plan/V4_PLAN.md` section 4.16.

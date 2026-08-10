# mini_vm

A stack machine: an instruction entry type, programs held as containers of
instructions, and a fetch-decode-execute loop with real fault handling for
stack underflow, stack overflow, and division by zero.

```
folc --package-store-root <repo>/lang/library code run
folc --package-store-root <repo>/lang/library code test
```

## Layout

| path | role |
| --- | --- |
| `src/isa/lib.fol` | opcodes, the `Instr` record, arity, rendering |
| `src/stack/lib.fol` | bounded operand stack with `/ str` fault reporting |
| `src/machine/lib.fol` | the interpreter and its `Step` result |
| `src/program/lib.fol` | five hand-assembled sample programs |
| `src/asm/lib.fol` | disassembly |
| `src/main.fol` | runs every sample program and prints traces |
| `listing/lib.fol` | text layout, reached through a `loc` import |
| `test/main.fol` | result and fault-message checks |

## Notes on the surface it uses

- the operand stack is a record with a fixed number of slots. FOL containers
  are literal-only: there is no append, no element assignment, and no
  concatenation, so a `seq[int]` cannot be used as a stack.
- the stack routines report faults with the call-site form `/ str`. That form
  can only be probed with `check(...)` or defaulted with `|| fallback`, and
  neither hands back the reported message, so `machine::exec` converts
  immediately into a storable `err[str]` shell that callers read with
  `when ... on ...`.
- a `loc`-imported directory is a package of its own and cannot see the
  importer's namespaces, so `listing` knows nothing about the instruction set
  and disassembly lives in `src/asm` instead.

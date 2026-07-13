# Error Handling

FOL does not center error handling around exceptions.

This section distinguishes two broad error categories:
- breaking errors:
  unrecoverable failures, typically associated with `panic`
- recoverable errors:
  errors that can be propagated or handled, typically associated with `report`

The detailed chapters explain:
- how each category behaves
- how routines expose recoverable error types
- how error-aware forms interact with control flow and pipes
- how the current compiler reports syntax, package, and resolver failures

## Current compiler diagnostics

The current compiler surface guarantees the following reporting behaviors across
the active parser/package/resolver/typecheck/lower/backend chain:

- every diagnostic carries a stable producer-owned code (e.g. `R1003`); the
  default `human` output shows it next to a family chip, and `--output plain`
  shows it in brackets (`error[R1003]:`)
- any code can be expanded with `fol code explain <CODE>`
- all failures keep exact primary `file:line:column` locations
- human-readable diagnostics render source snippets and underline the primary span
- related sites such as duplicate declarations or ambiguity candidates appear
  as secondary labels
- JSON diagnostics preserve the same structured information with labels, notes,
  helps, and stable producer-owned diagnostic codes
- the parser recovers after failed declarations instead of cascading errors
- exact consecutive diagnostic duplicates are suppressed in compiler reports,
  with a hard cap at 50
- LSP publishing removes only exact wire-identical duplicates; diagnostics that
  merely share a line and code remain distinct

The exact wording of messages is still implementation detail, but the current
compiler contract is that locations, codes, and structured diagnostic shape are
stable enough to build tests and tooling around them.

For the detailed compiler-facing reporting model, see
[Compiler Diagnostics](/docs/spec/errors/300_diagnostics).

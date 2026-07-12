# Compiler Diagnostics

This chapter is about compiler reporting, not about language-level `panic` or
`report` semantics.

In other words:

- `panic` and `report` describe what *your program* does
- diagnostics describe what *the compiler* tells you when it cannot continue or
  when it wants to surface important information

## Why this chapter exists

FOL now has a real compiler pipeline:

- `fol-stream`
- `fol-lexer`
- `fol-parser`
- `fol-package`
- `fol-resolver`
- `fol-typecheck`
- `fol-lower`
- `fol-runtime`
- `fol-backend`
- `fol-diagnostics`

That means errors are no longer just loose strings printed from one place.
Compiler failures now move through a shared diagnostics layer with stable
structure.

This matters for three reasons:

- humans need readable compiler output
- tests need stable enough structure to assert against
- future tools need a machine-readable format that is not just a copy of the
  human renderer

## What a diagnostic contains

At the current compiler stage, a diagnostic can carry:

- severity
- main message
- a stable diagnostic code (e.g. `P1001`, `R1003`, `T1003`)
- one primary location
- zero or more related locations
- notes
- helps
- suggestions

The current compiler mostly emits `Error`, but the reporting layer also supports
`Warning` and `Info`.

## Diagnostic codes

Every diagnostic carries a stable producer-owned code. The code identifies the
error family and specific failure without relying on message text.

Current code families:

| Prefix | Producer        | Examples                        |
|--------|-----------------|---------------------------------|
| `P1xxx`| parser          | `P1001` syntax, `P1002` file root |
| `K1xxx`| package loading | `K1001` metadata, `K1002` layout  |
| `R1xxx`| resolver        | `R1003` unresolved, `R1005` ambiguous |
| `T1xxx`| type checker    | `T1003` type mismatch           |
| `L1xxx`| lowering        | `L1001` unsupported surface     |
| `F1xxx`| frontend        | `F1001` invalid input, `F1002` workspace not found |
| `K11xx`| build evaluator | `K1101` build failure           |

Codes are structurally assigned. The parser carries an explicit `ParseErrorKind`
field on each error rather than deriving the code from message text. This means
message wording can change without breaking code identity.

The default `human` output shows the code after the message, alongside a
plain-language family chip (e.g. `NAMES ... R1003`). The `plain` output shows it
in brackets:

```text
error[R1003]: could not resolve name 'answer'
```

Any code can be expanded on demand with `fol explain <CODE>` (see
[Tool Commands](../050_tooling/200_tool_commands.md)).

JSON output includes the code as a top-level field:

```json
{ "code": "R1003", "message": "could not resolve name 'answer'" }
```

## Primary location

The most important part of a diagnostic is its primary location.

That location is currently expressed as:

- file
- line
- column
- optional span length

This is what allows the compiler to point at the exact token or source span that
caused the failure.

Every diagnostic now carries a real location. Parser errors that previously
lacked locations (safety-bound overflows, constraint violations like duplicate
parameter names) now extract file/line/column from the current token position.

Typical examples:

- a parser error at the token that made a declaration invalid
- a package-loading error at the control file or package root that failed
- a resolver error at the unresolved identifier or ambiguous reference
- a typecheck error at the expression or declaration whose types do not match
- a lowering error at the typed surface that has no current `V1` lowering rule

## Related locations

Some compiler failures are not well described by one location alone.

For example:

- duplicate declarations
- ambiguous references
- duplicate package metadata fields

In those cases the compiler keeps one primary site and can also attach related
sites as secondary labels.

That allows the compiler to say things like:

- this declaration conflicts with an earlier declaration
- this name could refer to either of these two candidates
- this metadata field was already defined elsewhere

## Notes, helps, and suggestions

FOL diagnostics separate extra guidance into different buckets instead of
forcing everything into one long message.

The current contract is:

- the main message says what went wrong
- notes add technical context
- helps add actionable guidance
- suggestions describe a possible replacement or next step when the producer can
  express one

This split matters because tooling and tests can preserve structure instead of
trying to parse intent back out of prose.

## Error recovery

The parser implements error recovery so that a single syntax mistake does not
cascade into dozens of unrelated errors.

When a declaration parse fails, the parser calls `sync_to_next_declaration` to
skip forward to the next declaration-start keyword (`fun`, `var`, `def`, `typ`,
`pro`, `log`, `seg`, `ali`, `lab`, `con`, `use`) or EOF. This means:

- `fun[exp] emit(...) = { ... }` produces exactly 1 error, not 20+
- two broken declarations separated by a good one produce 2 errors, and the
  good declaration still parses correctly

## Cascade suppression

Even with parser recovery, edge cases in any pipeline stage can cascade.

The diagnostic report layer applies two safety nets:

- **same-code, same-line dedup**: if the most recently added diagnostic has the
  same code and same line as a new one, the new one is suppressed
- **hard cap**: the report accepts at most 50 diagnostics total and shows
  "(output truncated)" when the limit is reached

These limits prevent walls of identical errors without hiding genuinely distinct
failures.

## Human-readable diagnostics

The CLI has two human-facing output modes, selected with the global `--output`
flag. Both render from the same structured diagnostic model.

### `human` (default)

The default renderer prints a framed, colored report:

- a family chip that names the problem in plain language (`PARSER`, `NAMES`,
  `TYPES`, `LOWERING`, `PACKAGE`, `BUILD`, `BACKEND`), the bold message, and the
  diagnostic code
- a framed source snippet (`┌─ file:line:column`) with the offending line and a
  caret under the primary span
- secondary labels for related sites
- `= note:`, `= help:`, and `= try:` lines
- a footer with a one-line plain-language hint and a
  `` run `fol explain <CODE>` for more `` pointer
- a closing `found N error(s)` summary

Illustrative shape (colors omitted):

```text
 NAMES  could not resolve name 'answer'   R1003
  ┌─ app/main.fol:3:12
    │
  3 │     return answer
    │            ^^^^^^ unresolved name
  = note: no visible declaration with that name was found in the current scope chain
  = help: check imports or declare the name before use
  a name or import problem — a symbol could not be resolved  ·  run `fol explain R1003` for more

found 1 error
```

Colors disable themselves automatically when stdout is not a terminal, so piped
and CI output stays plain text.

### `plain`

`--output plain` prints the older, unstyled human format:

- a severity prefix with the diagnostic code in brackets (e.g. `error[R1003]:`)
- an arrow line with `file:line:column`
- a source snippet with an underline for the primary span
- `note:`/`help:` lines and related-label summaries

Illustrative shape:

```text
error[R1003]: could not resolve name 'answer'
  --> app/main.fol:3:12
    |
  3 |     return answer
    |            ^^^^^^ unresolved name
  note: no visible declaration with that name was found in the current scope chain
  help: check imports or declare the name before use
```

In both modes the messages are clean human-readable text. The compiler does not
prepend internal kind labels like `ResolverUnresolvedName:` to messages; the
diagnostic code is the stable identifier.

## Source fallbacks

Sometimes the compiler knows the location but cannot render the source line
itself.

Examples:

- the file is no longer readable
- the file path is missing
- the requested line is outside the current file contents

In those cases the compiler still keeps the location and falls back cleanly
instead of crashing the renderer.

So the priority order is:

1. exact location
2. source snippet when available
3. explicit fallback note when the snippet cannot be shown

## JSON diagnostics

When the CLI is invoked with `--json`, diagnostics are emitted as structured
JSON instead of human-readable text.

This output is meant for scripts, tests, editor tooling, and future integration
layers.

Important rule:

- JSON is not a lossy summary of human output

Instead, both human and JSON outputs are generated from the same structured
diagnostic model.

The editor/LSP layer should follow that same rule too: editor diagnostics should
be adapted from the shared structured diagnostic model rather than rebuilt from
free-form strings.

That means JSON can preserve:

- severity
- code
- message
- primary location
- related labels
- notes
- helps
- suggestions

Illustrative shape:

```json
{
  "severity": "Error",
  "code": "R1003",
  "message": "could not resolve name 'answer'",
  "location": {
    "file": "app/main.fol",
    "line": 3,
    "column": 12,
    "length": 6
  },
  "labels": [
    {
      "kind": "Primary",
      "message": "unresolved name",
      "location": {
        "file": "app/main.fol",
        "line": 3,
        "column": 12,
        "length": 6
      }
    }
  ],
  "notes": [
    "no visible declaration with that name was found in the current scope chain"
  ],
  "helps": [
    "check imports or declare the name before use"
  ],
  "suggestions": []
}
```

Again, the exact payload can evolve, but the important guarantee is that the
structured fields are first-class rather than reverse-engineered from text.

## Which compiler phases currently participate

At head, the main producers that lower into the shared diagnostics layer are:

- parser
- package loading
- resolver
- type checking
- lowering
- build evaluator
- backend
- frontend (workspace discovery and input validation)

That means diagnostics are already strong across:

- syntax errors (with error recovery so cascades are contained)
- package metadata and package-root errors
- import-loading failures
- unresolved names
- duplicate names
- ambiguous references
- type mismatches and unsupported semantic surfaces inside `V1`
- unsupported lowered `V1` surfaces before target emission
- backend emission and build failures when lowered `V1` workspaces cannot become
  runnable artifacts
- build graph evaluation failures

This is the important boundary for the current compiler stage:

- the compiler can now parse, resolve, type-check, and lower the supported `V1`
  subset
- diagnostics already cover failures from each of those stages plus backend
  emission/build failures
- the project now does promise a finished first backend for the current `V1`
  subset, while later targets, optimizations, C ABI work, and Rust interop
  work remain outside
  this chapter

## What diagnostics do not guarantee

Diagnostics are strong, but they are not a substitute for the later semantic
phases.

Current limits still matter:

- parser diagnostics do not imply type checking has happened
- `report` compatibility still belongs to later semantic work
- type mismatch, coercion, and conversion diagnostics are future type-checker work
- ownership and borrowing diagnostics are future semantic work
- C ABI diagnostics are future `V4` package/type/backend work
- Rust interop diagnostics are future `V4` package/type/backend work

So the current guarantee is:

- if stream, lexer, parser, package loading, resolver, typechecker, or lowering
  can identify the problem now, diagnostics should be structured and exact

But not:

- all language-semantic errors already exist today

## Practical rule of thumb

When reading compiler output, think in this order:

1. look at the diagnostic code in brackets to identify the error family
2. trust the primary location
3. use related labels to understand competing or earlier sites
4. read notes for technical context
5. read helps for the most actionable next step

That mental model matches how the compiler currently structures reporting.

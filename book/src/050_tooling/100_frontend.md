# Frontend Workflow

The public FOL entrypoint is the `fol` tool.

`fol` is implemented by the `fol-frontend` crate. It sits above:

- `fol-package`
- `fol-resolver`
- `fol-typecheck`
- `fol-lower`
- `fol-backend`

Its job is orchestration, not semantic analysis.

## What The Frontend Owns

The frontend owns:

- CLI parsing with `clap`
- grouped command structure
- package and workspace discovery
- project scaffolding
- package preparation and fetch/update workflows
- build, run, test, and emit orchestration
- editor command dispatch under `fol tool`
- shell completions
- user-facing summaries and workflow errors

Bundled std reminder:

- the current binary scaffold defaults to `fol_model = "memo"` and bundled
  `std` because its generated source uses hosted `std.io`
- a bare executable may use `core` or `memo` and does not need bundled `std`
- scaffolding should not teach manual std dependency setup
- running and testing host-compatible artifacts is frontend/toolchain behavior,
  not a bundled-std capability

Compiler truth remains in the compiler crates.

## Public Command Groups

The root command groups are:

- `fol work`
- `fol pack`
- `fol code`
- `fol tool`

Root aliases are single-letter only:

- `fol w`
- `fol p`
- `fol c`
- `fol t`

Run:

```text
fol --help
fol <group> --help
fol <group> <command> --help
```

for the relevant usage surface.

## Root Discovery

For workflow commands, the frontend discovers roots upward from the current
directory or from an explicit path.

It recognizes:

- package roots via `build.fol`
- workspace roots via `fol.work.yaml`

A package root is one buildable package.

A workspace root is a parent root that owns multiple member packages and
frontend-managed roots like build/cache/package-store locations.

## Direct Package Use

The normal workflow shape is group-first:

```text
fol work init --bin
fol pack fetch
fol code check
fol code build
fol code run
```

There is still a frontend-owned direct package path for compile-oriented use,
but the grouped commands are the public workflow shape.

## Output

Frontend summaries support:

- `human`
- `plain`
- `json`

The frontend also owns workflow-level errors such as:

- missing workspace roots
- fetch/update/store failures
- clean/completion/workflow command failures

Compiler diagnostics stay separate and flow through compiler-owned diagnostic
models.

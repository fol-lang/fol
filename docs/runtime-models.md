# Runtime Models

This document is the canonical runtime-tier matrix for FOL.

`fol_model` is selected per artifact in `build.fol`.

It is not:

- a source-file pragma
- an import convention
- a compatibility mode

It is:

- a semantic capability boundary
- a backend/runtime linkage boundary

Import reminder:

- only `std` is an importable source-level library namespace
- `core` and `memo` are compiler/runtime capability choices, not `use` targets

## The three concepts

Everything in this document reduces to three separate ideas. Keeping them
apart is what makes the model coherent:

1. **Declared capability mode** — the `fol_model` string on each artifact in
   `build.fol`. There are exactly two: `core` and `memo`.
2. **Bundled `std` dependency** — an explicit
   `build.add_dep({ alias = "std", source = "internal", target = "standard" })`
   declaration. It is a package dependency, never a third `fol_model`.
3. **Effective runtime tier** — what the artifact actually typechecks and
   links against. Derived, never written by hand:

| Declared mode | Bundled `std` declared? | Effective API tier | `.echo` substrate |
|---------------|--------------------------|--------------------|-------------------|
| `core`        | (not allowed)            | `core`             | forbidden         |
| `memo`        | no                       | `memo`             | forbidden         |
| `memo`        | yes                      | hosted APIs (`std`) | legal             |

The promotion in the last row is the one rule people trip over: declaring the
bundled internal `standard` dependency upgrades the artifact's effective API
capability to hosted. That is what lets the bundled std wrappers (which are
built on the hosted `.echo(...)` primitive) typecheck — and it also legalizes
direct `.echo(...)` in your own source as the raw substrate
(`examples/std_substrate_echo` pins exactly this). Style still says ordinary
hosted code goes through `std.io`, but legality is decided by the dependency
declaration, not the spelling.

The build output reports both halves explicitly, for example:
`capability_mode=memo, bundled_std=1/1`.

Recommended style:

- spell `fol_model` explicitly when the artifact is `core`
- omit `fol_model` when the artifact is meant to take the default `memo`
- treat `core` and `memo` as capability choices
- treat bundled `std` as a declared internal dependency, not as a third model
- `graph.add_run(...)`, `fol code run`, and `fol code test` work for
  host-compatible `core` and `memo` artifacts without bundled `std`
- treat frontend process launching as build-host behavior, not as a
  source-language hosted capability

## Artifact source scopes

An artifact's `root` selects both its entry file and its compilation scope.
For an artifact-targeted build, the frontend:

1. resolves and canonicalizes `root` relative to the package root
2. requires the resolved root to be a source file inside that package
3. recursively parses the directory containing that root
4. checks that whole source directory under the artifact's `fol_model`

The containment check follows the resolved path, so `..` paths and symlinks
cannot be used to make an artifact root escape its declaring package.

This directory-scoped compilation is also the mixed-model isolation boundary.
Artifacts declared with different public models (`core` versus `memo`) must
use disjoint source directories. Two scopes overlap when they are the same
directory or when either directory contains the other; the frontend rejects
that graph instead of silently checking `core` source as `memo`. Sibling
directories such as `core/`, `memo/`, and `app/` are the intended layout.

Commands that need to compile an entire package under one model still reject a
mixed-model package. Select an exact artifact or its named build step, or split
the artifacts into separate package members.

## Tiers

### `core`

Meaning:

- no heap
- no source-level hosted OS/runtime APIs
- executable artifacts are allowed

Allowed language surface:

- scalars: `int`, `flt`, `bol`, `chr`
- arrays: `arr[...]`
- records, entries, aliases
- routines and method sugar
- control flow
- compile-time ownership and move checking
- lexical borrowing (`var[bor]`, `#owner`, `!borrow`)
- `dfr` and error-only `edf`
- typed `ptr[...]` declarations for analysis (but not allocating `&value`)
- `opt[...]`, `err[...]`
- array `.len(...)`
- `panic(...)`

Forbidden surface:

- `str`
- `vec[...]`
- `seq[...]`
- `set[...]`
- `map[...]`
- `.echo(...)`
- processor and other hosted language APIs

Choose `core` when:

- the artifact must avoid heap allocation completely
- the artifact should be valid for embedded-first targets
- arrays and plain records are enough
- source-visible console and OS APIs are not part of the contract

Allowed example:

```fol
fun[] checksum(values: arr[int, 3]): int = {
    return .len(values) + values[0];
};
```

Forbidden example:

```fol
fun[] label(): str = {
    return "core-nope";
};
```

### `memo`

Meaning:

- heap-backed runtime facilities
- still no source-level hosted OS/runtime APIs
- executable artifacts are allowed

Adds:

- `str`
- `vec[...]`
- `seq[...]`
- `set[...]`
- `map[...]`
- `@var` / `var[new]` owned heap allocation
- unique/shared pointer construction with `&value`
- dynamic/string `.len(...)`

Still forbidden **unless the bundled internal `standard` dependency is
declared** (which upgrades the effective API tier to hosted):

- `.echo(...)`
- `[>]` spawn, `chn[...]`, `select`, and `[mux]` routine parameters
- `| async` and `| await`
- process/console/filesystem/network services

Choose `memo` when:

- the artifact needs strings or dynamic containers
- the artifact still should not depend on source-visible hosted OS/runtime APIs
- you want to keep heap usage explicit in `build.fol`

Allowed example:

```fol
fun[] label(prefix: str, extras: ... str): str = {
    return prefix + extras[0];
};
```

Forbidden example (a `memo` artifact with no bundled std dependency):

```fol
fun[] main(): int = {
    .echo("memo-nope");
    return 0;
};
```

## Bundled `std`

`std` is not a third `fol_model`.

It is the bundled standard-library package shipped with FOL.

Projects opt into it explicitly in `build.fol`:

```fol
build.add_dep({
    alias = "std",
    source = "internal",
    target = "standard",
});
```

Then source code imports it through the declared dependency alias:

```fol
use std: pkg = {"std"};
```

## Quick selection rule

- pick `core` first if the artifact can stay array-only and no-heap
- move to `memo` when you actually need `str` or dynamic containers
- add bundled `std` only when the package genuinely needs shipped
  hosted-library wrappers or V3 processor facilities

The intent is to keep capability growth and dependency growth explicit.

Executable-artifact examples that build and run without bundled std:

- `examples/core_run_min`
- `examples/memo_run_min`

These examples declare graph run targets and execute through the ordinary
frontend path. Their `core` or `memo` tier constrains which APIs the source may
use; it does not prevent the toolchain from launching the resulting
host-compatible executable.

Hosted std examples with explicit bundled dependency:

- `examples/std_bundled_io`
- `examples/std_substrate_echo`

## Choose your model

Use `core` when the artifact can stay array-only and fixed-shape:

```fol
var graph = .build().graph();
graph.add_static_lib({
    name = "math",
    root = "src/lib.fol",
    fol_model = "core",
});
```

Move to `memo` when the artifact itself genuinely needs heap-backed strings or
dynamic containers:

```fol
var graph = .build().graph();
graph.add_static_lib({
    name = "text",
    root = "src/lib.fol",
    fol_model = "memo",
});
```

Add bundled `std` when the package needs shipped hosted-library wrappers:

```fol
var build = .build();
build.add_dep({
    alias = "std",
    source = "internal",
    target = "standard",
});
```

Direct boundary reminder:

- a `core` artifact must not declare `str`, `seq`, `vec`, `set`, or `map`
- a `memo` artifact without the bundled `standard` dependency must not call
  `.echo(...)`
- a `core` or `memo` artifact may declare an executable, a graph run target,
  and test artifacts without bundled `standard`
- `fol code run` and `fol code test` may launch those host-compatible artifacts;
  target compatibility is independent of bundled-std API legality

Transitive boundary reminder:

- a `core` artifact still cannot consume heap-backed API from a `memo`
  dependency
- a `core` or `memo` artifact cannot consume bundled `std` APIs unless the
  bundled internal `standard` dependency was declared
- a `memo` artifact with bundled `std` may consume both `core` and `memo`
  dependencies in one graph

## Transitive boundary rule

Capability legality is checked at the consuming artifact boundary, not only at
the dependency's own artifact boundary.

That means:

- a `core` artifact cannot consume heap-backed API from a `memo` package just
  because the dependency itself was declared with `fol_model = "memo"`
- a `core` or `memo` artifact cannot reach `.echo(...)` indirectly through an
  imported bundled `std` package unless the package declared internal
  `standard`
- a `memo` artifact with bundled `std` may consume `core` and `memo` packages
  in the same graph

The consuming artifact model always wins.

## Guarantees by capability

| Declared mode | Heap | Host-compatible execution | Hosted language APIs | Typical artifact shape |
|---------------|------|---------------------------|----------------------|------------------------|
| `core`        | no   | yes                       | no                   | embedded logic, fixed-shape libs, no-heap executables |
| `memo`        | yes  | yes                       | only with explicit bundled `std` | heap utilities, alloc-like executables, bundled-std consumers |

## Current implementation status

Implemented today:

- `.echo(...)` requires hosted std support; declaring the bundled internal
  `standard` dependency on a `memo` artifact provides it (the whole
  consuming workspace typechecks at the hosted API capability)
- `str`, `vec`, `seq`, `set`, and `map` are rejected in `core`
- array `.len(...)` stays valid in `core`
- dynamic/string `.len(...)` requires `memo`
- ownership and lexical borrowing are checked in every model
- pointer type declarations are analyzable in `core`, while `&value`, owned
  allocation, and pointer construction require `memo` or hosted `std`
- all processor surfaces (`[>]`, channels, `select`, mutex parameters,
  `async`, and `await`) require hosted `std`
- routed `run` / `test` accept host-compatible `core` and `memo` artifacts
  without bundled `std`; the dependency is required only when source uses its
  hosted APIs
- emitted Rust imports the matching internal runtime module
- public `fol_model = "memo"` currently maps to the internal heap runtime
  module `fol_runtime::memo`
- packages import bundled `std` only through explicit internal dependency
  declaration

## Runtime export contract

The backend should treat the three runtime modules as intentionally different
public surfaces.

Executable entry and recoverable process-outcome adaptation are backend-only
substrate shared by every tier. The generated wrapper calls the separate
`fol_runtime::process` adapter directly; `core`, `memo`, and `std` do not
re-export it as source-language capability.

- `fol_runtime::core`
  - no heap-backed types
  - no source-visible hosted hooks like `.echo(...)`
  - executable wrappers may pair it with `fol_runtime::process`
- internal heap runtime module
  - heap-backed strings and dynamic containers
  - still no source-visible hosted hooks like `.echo(...)`
  - executable wrappers may pair it with `fol_runtime::process`
- `fol_runtime::std`
  - hosted hooks such as `.echo(...)`
  - OS-thread spawn, channels, selection, mutex, and eventual support
  - memo-tier heap types re-exported for host artifacts

Backend authors should not import a wider tier than the lowered artifact
actually requires. `core` emission should stay `core`-only. `memo` emission
currently routes through the internal heap runtime module and must not silently
widen to `std`. `std` is the only tier whose source may reach hosted language
APIs. Backend process entry and frontend host-tool launching are orthogonal to
that source-language gate.

## Editor note

The editor should follow the same model split.

The intended contract is:

- LSP semantic diagnostics should come from the real compiler pipeline
- `fol_model` should affect editor diagnostics and completion the same way it
  affects `fol code build`
- tree-sitter grammar and structural capture layout stay hand-authored
- repetitive editor name lists are compiler-derived, not manually copied

This means adding a language feature should not require a second semantic
implementation in `fol-editor`. Only syntax-structure changes should normally
need targeted tree-sitter or editor UX updates.

## Build example

```fol
pro[] build(): non = {
    var build = .build();
    build.meta({ name = "mixed_models_workspace", version = "0.1.0" });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var corelib = graph.add_static_lib({
        name = "corelib",
        root = "src/core/lib.fol",
        fol_model = "core",
    });

    var heaplib = graph.add_static_lib({
        name = "heaplib",
        root = "src/memo/lib.fol",
        fol_model = "memo",
    });

    var tool = graph.add_exe({
        name = "tool",
        root = "src/main.fol",
        fol_model = "memo",
    });
}
```

## Example packages

- `examples/core_blink_shape`
- `examples/core_dfr`
- `examples/core_records`
- `examples/core_surface_showcase`
- `examples/core_run_min`
- `examples/memo_defaults`
- `examples/memo_containers`
- `examples/memo_collections`
- `examples/memo_surface_showcase`
- `examples/memo_run_min`
- `examples/std_cli`
- `examples/std_bundled_fmt`
- `examples/std_bundled_io`
- `examples/std_explicit_pkg`
- `examples/std_alias_pkg`
- `examples/std_echo_min`
- `examples/std_logtiny_git`
- `examples/std_named_calls`
- `examples/std_surface_showcase`
- `examples/mixed_models_workspace`

Negative example packages:

- `examples/fail_core_heap_reject`
- `examples/fail_memo_echo`
- `examples/fail_core_alloc_boundary`
- `examples/fail_core_std_import`
- `examples/fail_memo_std_missing_dep`

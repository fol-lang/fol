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

| Declared mode | Bundled `std` declared? | Effective tier | `.echo` substrate |
|---------------|--------------------------|----------------|-------------------|
| `core`        | (not allowed)            | `core`         | forbidden         |
| `memo`        | no                       | `memo`         | forbidden         |
| `memo`        | yes                      | hosted (`std`) | legal             |

The promotion in the last row is the one rule people trip over: declaring the
bundled internal `standard` dependency upgrades the artifact's effective
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
- treat `graph.add_run(...)` as independent from std-library presence

## Tiers

### `core`

Meaning:

- no heap
- no OS/runtime services

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
- hosted process-entry assumptions

Choose `core` when:

- the artifact must avoid heap allocation completely
- the artifact should be valid for embedded-first targets
- arrays and plain records are enough
- console/process services are not part of the contract

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
- still no hosted OS/runtime services

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
declared** (which upgrades the effective tier to hosted):

- `.echo(...)`
- `[>]` spawn, `chn[...]`, `select`, and `[mux]` routine parameters
- `| async` and `| await`
- process/console/filesystem/network services

Choose `memo` when:

- the artifact needs strings or dynamic containers
- the artifact still should not depend on hosted OS/runtime services
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
- add bundled `std` only when the package genuinely needs shipped hosted-library
  wrappers

The intent is to keep capability growth and dependency growth explicit.

Runnable examples without bundled std:

- `examples/core_run_min`
- `examples/memo_run_min`

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
- a `core` or `memo` artifact may still be runnable without bundled std if it
  does not import bundled std APIs

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

| Capability | Heap | Bundled `std` allowed by itself | Typical artifact shape |
|------------|------|----------------------------------|------------------------|
| `core`     | no   | no                               | embedded logic, fixed-shape libs |
| `memo`     | yes  | only when dependency declared    | heap utilities, host tools, bundled-std consumers |

## Current implementation status

Implemented today:

- `.echo(...)` requires hosted std support; declaring the bundled internal
  `standard` dependency on a `memo` artifact provides it (the whole
  consuming workspace typechecks at the hosted capability)
- `str`, `vec`, `seq`, `set`, and `map` are rejected in `core`
- array `.len(...)` stays valid in `core`
- dynamic/string `.len(...)` requires `memo`
- ownership and lexical borrowing are checked in every model
- pointer type declarations are analyzable in `core`, while `&value`, owned
  allocation, and pointer construction require `memo` or hosted `std`
- all processor surfaces (`[>]`, channels, `select`, mutex parameters,
  `async`, and `await`) require hosted `std`
- routed `run` / `test` require the explicit bundled internal `standard`
  dependency because executing artifacts is a hosted operation
- emitted Rust imports the matching internal runtime module
- public `fol_model = "memo"` currently maps to the internal heap runtime
  module `fol_runtime::memo`
- packages import bundled `std` only through explicit internal dependency
  declaration

## Runtime export contract

The backend should treat the three runtime modules as intentionally different
public surfaces.

- `fol_runtime::core`
  - no heap-backed types
  - no hosted hooks like `.echo(...)`
  - no hosted process-outcome helpers
- internal heap runtime module
  - heap-backed strings and dynamic containers
  - still no hosted hooks like `.echo(...)`
  - still no hosted process-outcome helpers
- `fol_runtime::std`
  - hosted hooks such as `.echo(...)`
  - hosted process-outcome helpers
  - OS-thread spawn, channels, selection, mutex, and eventual support
  - memo-tier heap types re-exported for host artifacts

Backend authors should not import a wider tier than the lowered artifact
actually requires. `core` emission should stay `core`-only. `memo` emission
currently routes through the internal heap runtime module and must not silently
widen to `std`. `std` is the only tier that may rely on
hosted runtime entry and console hooks.

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
- `examples/memo_defaults`
- `examples/memo_containers`
- `examples/memo_collections`
- `examples/memo_surface_showcase`
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

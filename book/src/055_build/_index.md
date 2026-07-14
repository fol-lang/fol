# Build System

FOL uses a Zig-style build model. The build specification is a normal FOL
program called `build.fol`. It goes through the full compiler pipeline and
is executed against a build graph IR instead of emitting backend code.

The build system lives in `lang/execution/fol-build`. It handles:

- graph IR construction for artifacts, steps, options, modules, and generated files
- full control flow in build programs (when, loop, helper routines)
- `-D` CLI option passing into the build program
- named step selection at the command line

## Entry Point

Every buildable package must have a `build.fol` at its root with exactly one
canonical entry:

```fol
pro[] build(): non = {
    var build = .build();
    build.meta({ name = "app", version = "0.1.0" });
    var graph = build.graph();
    ...
}
```

The active build context is accessed explicitly through the build-only ambient
accessor:

```fol
.build()
```

There is no injected `graph` parameter anymore. `.build()` returns an opaque
build-only handle. Users do not name its type explicitly. Package metadata and
direct dependencies are configured through that handle, and graph work is
reached through `build.graph()`.

## Minimal Example

```fol
pro[] build(): non = {
    var build = .build();
    build.meta({ name = "app", version = "0.1.0" });
    var graph = build.graph();
    var app = graph.add_exe({ name = "app", root = "src/main.fol" });
    graph.install(app);
    graph.add_run(app);
}
```

This registers package metadata, adds an executable, marks it for installation,
and binds a default run step.

## What `fol-build` Owns

- `graph.rs` — build graph IR (steps, artifacts, modules, options, generated files)
- `api.rs` — Rust-level graph mutation interface
- `semantic.rs` — method signatures and type info for the resolver and typechecker
- `stdlib.rs` — `BuildStdlibScope`: the ambient scope injected into `build.fol`
- `executor.rs` — executes the lowered FOL IR against the build graph
- `eval.rs` — evaluate a `build.fol` from source; entry point for `fol-package`
- `option.rs` — build option kinds, target triples, optimize modes
- `runtime.rs` — runtime representation of artifacts, generated files, step bindings
- `step.rs` — step planning, ordering, cache keys, execution reports
- `codegen.rs` — system tool and codegen request types
- `artifact.rs` — artifact pipeline definitions and output types
- `dependency.rs` — inter-package dependency surfaces

Use this section for:

- understanding the shape of `build.fol`
- the full graph API reference
- control flow available inside `build.fol`
- build options and `-D` flags
- artifact types, modules, and generated files
- dependency handles and unified output handles

## Near-Term Architecture

The next build round is about extending the existing explicit surface, not
replacing it.

The intended layering is:

- `build.add_dep({...})` declares a direct dependency and returns a dependency
  handle
- `build.export_*({...})` declares the build-facing surface a package chooses to
  expose
- `graph.file_from_root(...)` and `graph.dir_from_root(...)` remain the typed
  source-path producers
- broader path-oriented exports and dependency path queries sit on top of those
  producers instead of collapsing back into raw string paths
- dependency modes, install reporting, and system integration should become more
  concrete without changing the top-level `.build()` structure

This means the near-term additions should look like richer values and richer
queries on top of the current build graph, not a new manifest format and not a
public `Graph` or `Build` type.

## Standalone Examples

These checked-in example packages exercise the current public build surface:

- `examples/build_dep_exports`
- `examples/build_source_paths`
- `examples/build_dep_modes`
- `examples/build_described_steps`
- `examples/build_generated_dirs`
- `examples/build_dep_handles`
- `examples/build_output_handles`
- `examples/build_install_prefix`
- `examples/build_system_lib`
- `examples/build_system_tool`

Runtime-model reminder:

- examples that rely on hosted language APIs such as `.echo(...)` or V3
  processor facilities should spell `fol_model = "memo"` and declare bundled
  `std`
- `core` and `memo` examples may be executable and use routed `run` / `test`;
  they should stay free of source-level hosted API assumptions
- frontend host-tool and artifact launching is separate from language
  capability tiering

Bundled std reminder:

- `std` ships with FOL under `lang/library/std`
- packages using hosted APIs add bundled std explicitly through:
  `.build().add_dep({ alias = "std", source = "internal", target = "standard" })`
- normal hosted-API packages should rely on the bundled shipped `std`, not an
  external replacement package

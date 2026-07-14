# Cross Compilation

FOL package builds now stay on one backend path:

```text
FOL source
  -> lowered FOL IR
  -> generated Rust crate
  -> rustc
  -> native binary
```

Normal artifact builds do not call Cargo anymore. Cargo is still useful for
`fol code emit rust`, but `fol code build` and `fol code run` compile the
generated crate directly with `rustc`.

## Selecting A Target

Use either:

```bash
fol code build --target aarch64-unknown-linux-gnu
fol code build --target x86_64-pc-windows-gnu
```

or inside `build.fol`:

```fol
pro[] build(): non = {
    var build = .build();
    build.meta({ name = "app", version = "0.1.0" });
    var graph = build.graph();
    var target = graph.standard_target();
    var app = graph.add_exe({
        name = "app",
        root = "src/main.fol",
        target = target,
    });
    graph.install(app);
    graph.add_run(app);
};
```

Target precedence is:

1. `--target`
2. artifact target declared in `build.fol`
3. host default

## Accepted Target Spellings

The backend accepts both canonical Rust triples and the shorter FOL spellings
already used in build code.

Examples:

- `x86_64-linux-gnu` -> `x86_64-unknown-linux-gnu`
- `x86_64-linux-musl` -> `x86_64-unknown-linux-musl`
- `aarch64-linux-gnu` -> `aarch64-unknown-linux-gnu`
- `aarch64-linux-musl` -> `aarch64-unknown-linux-musl`
- `x86_64-windows-gnu` -> `x86_64-pc-windows-gnu`
- `x86_64-windows-msvc` -> `x86_64-pc-windows-msvc`
- `aarch64-windows-msvc` -> `aarch64-pc-windows-msvc`
- `x86_64-macos-gnu` -> `x86_64-apple-darwin`
- `aarch64-macos-gnu` -> `aarch64-apple-darwin`

Unknown spellings are rejected before the backend tries to build.

## Build vs Run

Cross-building and cross-running are different operations.

- `fol code build` supports host and non-host targets
- `fol code emit rust` stays available for source inspection
- `fol code run` is host-only
- `fol code test` is host-only

If the selected target does not match the current machine, `run` and `test`
fail early with a diagnostic instead of trying to execute the foreign binary.
"Host-only" here is a target-compatibility rule. It is independent of the
source-language capability tier: host-compatible `core` and `memo` artifacts
can execute without bundled `std`.

## Output Layout

Compiled binaries are target-scoped so host and cross builds do not overwrite
each other.

Typical layout:

```text
.fol/build/<profile>/bin/<target>/<artifact>
.fol/build/<profile>/fol-backend/runtime/<target>/<profile>/...
```

That means:

- host builds and cross builds can coexist
- runtime artifacts are compiled per target
- the generated entry crate links against the matching target runtime

## `emit rust`

`fol code emit rust` still writes a Cargo-compatible crate for debugging and
inspection. That command is source emission, not the product binary build path.

The binary build path remains direct `rustc`.

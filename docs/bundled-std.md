# Bundled Std

FOL ships its standard library source with the toolchain.

Current phase:

- bundled `std` is still intentionally small
- only actually shipped public names should be documented as available
- internal runtime rename work is implementation cleanup, not a new public
  library tier

Finalized design contract:

- public capability modes are only:
  - `core`
  - `memo`
- omitted `fol_model` defaults to:
  - `memo`
- bundled standard-library package identity is:
  - `standard`
- the normal dependency alias in user projects is:
  - `std`
- source code should reach bundled std through the dependency system with `pkg`
  imports, for example:
  - `use std: pkg = {"std"};`
- `graph.add_run(...)` may declare a run target independently of std-library
  presence
- building or checking an executable `core` or unhosted `memo` artifact does
  not require bundled std
- executing a run or test target on the host does require a `memo` artifact
  plus the explicit bundled `standard` dependency

Normal build usage:

- users do not download `std` separately
- users add the bundled standard library explicitly in `build.fol`:

```fol
build.add_dep({
    alias = "std",
    source = "internal",
    target = "standard",
});
```

Implementation split:

- `core` and `memo` remain compiler/runtime capability layers in Rust
- `std` is the importable bundled library and should grow mostly in FOL

## What Ships With FOL

The FOL distribution should be read as three separate pieces:

- compiler and runtime substrate:
  - parser
  - resolver
  - typechecker
  - backend
  - runtime-owned `core` and `memo` capability support
- bundled library source:
  - `lang/library/std`
- optional external dependencies:
  - added through `.build().add_dep(...)`
  - bundled std uses the same dependency surface with `source = "internal"`

Dependency distinction:

- bundled std:
  - `source = "internal"`
  - `target = "standard"`
  - usually `alias = "std"`
- external packages:
  - `source = "loc" | "pkg" | "git"`
  - examples like `examples/std_logtiny_git` stay ordinary external dependencies
  - they do not replace or implicitly provide bundled std

Import rule:

- only `std` is imported from source code as a dependency alias
- `core` and `memo` are selected through `fol_model`, not imported

An explicit `--std-root <DIR>` override may still exist for development and testing, but it is not the normal user path.

## Bootstrap Surface

The bundled shipped std is intentionally small right now.

Current public bootstrap modules:

- `std.fmt`
- `std.fmt.math`
- `std.io`

Current bootstrap routines:

- `fmt::answer(): int`
- `fmt::double(int): int`
- `fmt::triple(int): int`
- `fmt::sum2(int, int): int`
- `fmt::math::answer(): int`
- `io::echo_int(int): int`
- `io::echo_str(str): str`
- `io::echo_bool(bol): bol`
- `io::echo_chr(chr): chr`

`std.io` is intentionally narrow right now. It wraps the hosted `.echo(...)`
primitive instead of replacing it.

Current rule:

- `.echo(...)` remains the low-level hosted substrate
- `std.io` is the first bundled public wrapper over that substrate
- executable artifacts can still be built without bundled std, but routed
  `run` / `test` host execution requires the explicit bundled dependency even
  when source code does not import a `std` module

That keeps the first shipped std honest:

- real FOL package
- real import path
- real hosted example coverage
- no fake placeholder `std.os` module yet

Canonical bootstrap example packages:

- buildable unhosted executable artifacts:
  - `examples/core_run_min`
  - `examples/memo_run_min`
- bundled-std consumers:
  - `examples/std_bundled_fmt`
  - `examples/std_bundled_io`
  - `examples/std_explicit_pkg`
  - `examples/std_alias_pkg`
  - `examples/std_substrate_echo`

Current shipped public routines:

- `fmt::answer(): int`
- `fmt::double(int): int`
- `fmt::triple(int): int`
- `fmt::sum2(int, int): int`
- `fmt::math::answer(): int`
- `io::echo_int(int): int`
- `io::echo_str(str): str`
- `io::echo_bool(bol): bol`
- `io::echo_chr(chr): chr`

Older hosted std examples should use bundled std modules when one already exists.
That means current echo-based examples should prefer `std.io` instead of calling
`.echo(...)` directly unless the example is explicitly about the primitive
substrate.

The one explicit raw-substrate example is:

- `examples/std_substrate_echo`

No other shipped example should use raw `.echo(...)` when an equivalent bundled
`std.io` wrapper already exists.

## Editing Bundled Std

Normal local iteration should edit:

- `lang/library/std`

Normal compiler and CLI flows should pick it up automatically without extra flags.

Use an explicit `--std-root <DIR>` override only when you deliberately want to:

- test an alternate std checkout
- isolate resolver/import behavior with a synthetic std tree
- compare bundled std against a temporary experimental root

That override is for development and tests. It is not the normal user workflow.

## Shipped V2 Example Execution

The shipped executable `V2` examples that use bundled `std` are:

- `examples/generic_type_exec_m1m2`
- `examples/generic_standard_constraint_m1m2`

Their checked-in `build.fol` files use the normal bundled-`std` declaration:

```fol
build.add_dep({
    alias = "std",
    source = "internal",
    target = "standard",
});
```

Normal local execution should run from the example root with ordinary frontend
commands:

- `fol code build`
- `fol code run`

The normal user path does not require `--package-store-root` or `--std-root`.
Those flags exist for harnesses, fixture isolation, and explicit override work,
not as part of the shipped V2 example contract.

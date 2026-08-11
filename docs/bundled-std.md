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
- building or checking an executable `core` or std-free `memo` artifact does
  not require bundled std
- running or testing a host-compatible `core` or `memo` artifact does not
  require bundled std
- bundled std gates source-visible hosted APIs, not artifact execution
- launching artifacts and system tools is frontend/build-host behavior and is
  orthogonal to `fol_model`
- target compatibility is also orthogonal: a cross-target artifact still needs
  an appropriate runner, and bundled std does not make it host-executable

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

## What the dependency changes

The internal dependency declaration is package-level. For a `memo` artifact it:

- makes the dependency alias available to `use std: pkg = {"std"};`
- raises the artifact's effective API tier so hosted FOL APIs are legal
- links the hosted runtime surface required by those APIs

It does not:

- create a third `fol_model`
- widen a `core` artifact in the same package
- grant permission to run or test an artifact
- make a foreign machine target executable on the build host
- import `std` into source automatically

The Rust-oriented analogy is `core` for a `no_std`-style fixed-shape source
surface, `memo` for the same source model plus alloc-like heap facilities, and
the explicit bundled dependency for hosted FOL APIs. This analogy describes
the FOL source contract, not whether compiler or backend implementation code
uses host facilities internally.

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

## Where Bundled Std Physically Lives

Two separate roots are involved, and they resolve differently.

The **std root** (where the `standard` package's sources live):

1. an explicit `--std-root` flag or `FOL_STD_ROOT`
2. a `std_root` declared in `fol.work.yaml`
3. **`std/` next to the running `folc` binary** — the installed toolchain
   layout (`$FOL_HOME/toolchains/vX.X.X/{folc, std/, runtime/}`) managed by
   `fol self`, which makes a released toolchain fully self-contained
4. the source-tree path compiled into dev builds (`lang/library/std`)

The **package store root**, which is what `use std: pkg = {"std"}` resolves
through, has its own chain — explicit → declared → `<project>/.fol/pkg` →
`$FOL_HOME/pkg` → the toolchain's bundled store — with the last three skipped
unless they actually contain packages. `.fol/pkg` belongs to *that* chain, not
to std-root resolution.

The `runtime/` crate sources the backend compiles emitted Rust against follow
the same binary-relative rule as std. See the book's Toolchain Management
chapter for the full `fol` / `folc` split and the release asset contract.

## Bootstrap Surface

The bundled shipped std is intentionally small right now.

Current public modules:

- `std.os`
- `std.fs`
- `std.fmt`
- `std.fmt.math`
- `std.io`
- `std.term`
- `std.time`
- `std.strn`

Current shipped routines:

- `os::env(str): str`
- `os::shell(str): int`
- `os::arg_count(): int`
- `os::arg(int): str`
- `fs::dir_list(str): str`
- `fs::read_file(str): str`
- `fs::write_file(str, str): int`
- `fmt::int_to_str(int): str`
- `fmt::float_to_str(flt, int): str`
- `fmt::math::answer(): int`
- `io::echo_int(int): int`
- `io::echo_str(str): str`
- `io::echo_bool(bol): bol`
- `io::echo_chr(chr): chr`
- `io::write(str): str`
- `io::write_err(str): str`
- `io::read_key(): int`
- `io::read_key_ms(int): int`
- `term::raw_mode(bol): bol`
- `term::cols(): int`
- `term::rows(): int`
- `time::sleep_ms(int): int`
- `time::now_ms(): int`
- `strn::sub(str, int, int): str`
- `strn::byte_at(str, int): int`
- `strn::from_byte(int): str`
- `strn::find(str, str): int`
- `strn::replace(str, str, str): str`
- `strn::to_int(str, int): int`

`std.io` is intentionally narrow right now. It wraps the hosted `.echo(...)`
primitive instead of replacing it.

Current rule:

- `.echo(...)` remains the low-level hosted substrate
- `std.io` is the first bundled public wrapper over that substrate
- executable artifacts can be built, run, and tested without bundled std
- the explicit bundled dependency is required when source code uses
  `std`-backed hosted APIs, not merely because the frontend executes a binary

Cross-target execution remains separate. The current frontend rejects
`fol code run` and `fol code test` for a target that cannot execute on the
host; use an appropriate external runner for that artifact. Adding bundled
std does not change target compatibility.

That keeps the first shipped std honest:

- real FOL package
- real import path
- real hosted example coverage
- no fake placeholder `std.os` module yet

Canonical bootstrap example packages:

- std-free executable artifacts that use the normal build/run route:
  - `examples/core_run_min`
  - `examples/memo_run_min`
- bundled-std consumers:
  - `examples/std_bundled_fmt`
  - `examples/std_bundled_io`
  - `examples/std_explicit_pkg`
  - `examples/std_alias_pkg`
  - `examples/std_substrate_echo`

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

These examples declare bundled std because their source uses hosted std APIs.
The `fol code run` command itself does not impose that dependency.

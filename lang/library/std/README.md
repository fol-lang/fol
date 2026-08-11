# Bundled `std`

This is the bundled FOL standard-library root.

Normal projects should declare:

```fol
build.add_dep({
    alias = "std",
    source = "internal",
    target = "standard",
});
```

and then import bundled std through the dependency alias, for example:

```fol
use std: pkg = {"std"};
```

Bundled std is the normal path.

Use an explicit `--std-root <DIR>` override only for development and testing.

`core` and `memo` are not imported from here. They remain compiler/runtime capability modes.

In Rust terms, they fill the roles of a `core`-only source surface and a
`core`-plus-`alloc` source surface. This is a capability analogy; the current
compiler and Rust backend remain build-host tools.

Phase-2 scope:

- bundled `std` should remain intentionally small
- only real shipped public names should be documented here
- the internal runtime `alloc` -> `memo` rename is implementation cleanup, not
  a new public library family

External dependencies stay separate from bundled std.

- bundled std:
  - `source = "internal"`
  - `target = "standard"`
  - normally `alias = "std"`
- external packages:
  - `source = "loc" | "pkg" | "git"`
  - example: `examples/std_logtiny_git`

## Bootstrap Scope

`std` should start small and grow gradually.

The current bundled bootstrap surface is intentionally tiny:

- `std.os.env(str): str`, `std.os.shell(str): int`
- `std.fs.dir_list(str): str`, `std.fs.read_file(str): str`,
  `std.fs.write_file(str, str): int`
- `std.os.arg_count(): int`, `std.os.arg(int): str`
- `std.fmt.answer(): int`, `std.fmt.double(int): int`,
  `std.fmt.triple(int): int`, `std.fmt.sum2(int, int): int`,
  `std.fmt.int_to_str(int): str`
- `std.fmt.math.answer(): int`
- `std.io.echo_int(int): int`, `std.io.echo_str(str): str`,
  `std.io.echo_bool(bol): bol`, `std.io.echo_chr(chr): chr`,
  `std.io.write(str): str`, `std.io.read_key(): int`,
  `std.io.read_key_ms(int): int`
- `std.term.raw_mode(bol): bol`, `std.term.cols(): int`, `std.term.rows(): int`
- `std.time.sleep_ms(int): int`, `std.time.now_ms(): int`
- `std.strn.sub(str, int, int): str`, `std.strn.byte_at(str, int): int`,
  `std.strn.from_byte(int): str`

That is enough to prove:

- the toolchain ships a real importable `std`
- bundled std resolves through one explicit internal dependency declaration
- FOL-authored std modules are declared with `fol_model = "memo"` and link into
  runnable hosted consumers through the explicit dependency

Bundled std is needed here because these modules expose hosted language APIs,
not because the artifact executes. Host-compatible `core` and `memo`
executables can build, run, and test without bundled std; process launching is
a frontend/toolchain concern.

The declaration also does not solve target compatibility. A cross-target
artifact needs an appropriate external runner; current frontend `run` and
`test` commands reject targets that cannot execute on the build host.

`std.io` is currently just a thin FOL wrapper over the hosted `.echo(...)`
substrate.

`std.os` is still deferred until it has one honest user-facing API.

The only shipped raw-substrate example is:

- `examples/std_substrate_echo`

Everything else should prefer bundled `std.io` once an equivalent wrapper
exists.

## Shipped Surface Summary

Current shipped bundled modules:

- `std.os`
- `std.fs`
- `std.fmt`
- `std.fmt.math`
- `std.io`
- `std.term`
- `std.time`
- `std.strn`

Current shipped public routines:

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

Canonical bootstrap examples:

- `examples/std_bundled_fmt`
- `examples/std_bundled_io`
- `examples/std_explicit_pkg`
- `examples/std_alias_pkg`

Anything outside that list should not be documented as already shipped.

## Growth Rule

When bundled `std` gains a new public name:

- add or update a real example package
- add or update CLI/integration coverage
- add or update LSP/tree-sitter coverage
- update this README and `docs/bundled-std.md`
- update the relevant book pages

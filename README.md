<p align="center">
    <img alt="logo" src="./book/src/images/logo.svg" width="300px">
</p>


<a href="https://fol-lang.github.io/fol/" style="color: rgb(179, 128, 255)"></a><h2><p align="center" style="color: rgb(179, 128, 255)">https://fol-lang.github.io/fol/</p></h2></a>

<p align="center">
  <a href="https://github.com/fol-lang/fol/blob/develop/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://github.com/fol-lang/fol/actions/workflows/tests.yml"><img alt="Tests" src="https://github.com/fol-lang/fol/actions/workflows/tests.yml/badge.svg?branch=develop"></a>
</p>

<p align="center">general-purpose and systems programming language</p>
<hr>


FOL is a general-purpose, systems programming language designed for robustness, efficiency, portability, expressiveness and most importantly elegance. Heavily inspired (and shamelessly copying) from languages: rust, zig, nim, c, go, and cpp. In Albanian language "fol" means "to speak".

<p align="center">  ** FOL IS IN ACTIVE DEVELOPMENT **  </p>

## Installation & Toolchain

FOL ships as two binaries: **`fol`**, the toolchain manager — the only binary
on your `PATH` and the only thing a distribution packages — and **`folc`**,
the engine (compiler, build graph, package system, LSP), which lives inside
versioned toolchain directories that `fol` installs and selects for you. You
always type `fol`; it forwards `code`/`work`/`pack`/`tool` commands to the
right engine and handles `fol self …` itself.

```console
$ curl -fL -o ~/.local/bin/fol \
    https://github.com/fol-lang/fol/releases/download/v0.2.1/fol-v0.2.1-x86_64-linux
$ chmod +x ~/.local/bin/fol
$ export FOL_HOME="$HOME/.fol"     # add to your shell profile
```

Toolchains, packages, and configuration live under `FOL_HOME`. Without the
variable, `fol` falls back to `<project>/.fol/toolchain`, keeping everything
next to the project's build artifacts; with neither, it errors with setup
instructions.

```
$FOL_HOME/
├── toolchains/v0.2.1/{folc, std/, runtime/}   ← immutable, self-contained units
├── toolchains/dev.toml                        ← a linked source checkout
├── pkg/                                       ← packages, shared by all versions
└── config                                     ← default toolchain
```

A project pins its language version on the first comment line of `build.fol`:

```fol
//fol 0.2.1
```

Selection order is `+<toolchain>` argument → `FOL_TOOLCHAIN` env → the
`//fol` pin → the configured default. A pinned version that is not installed
is **fetched automatically** from the GitHub release
(`fol-compiler-and-lib-v<version>-<target>.tar.gz`, containing exactly
`folc` + `std/` + `runtime/`; Linux-only: `x86_64-linux`, `aarch64-linux`):

```console
$ fol code run
toolchain 0.2.1 not installed, fetching...
fetched fol 0.2.1 -> ~/.fol/toolchains/v0.2.1
42
```

Managing toolchains:

```console
fol self install 0.2.1              # fetch a released toolchain
fol self install 0.2.1 --from .     # copy one out of a built source tree
fol self link dev ~/code/fol        # use a source checkout, always fresh
fol self default 0.2.1              # set the default
fol self list                       # what's installed, default marked
fol self which                      # which folc this directory resolves to
fol self remove 0.2.1               # delete a toolchain or link
```

Full details — dispatch mechanics, per-project versions, the release asset
contract — are in the book's
[Toolchain Management](https://fol-lang.github.io/fol/025_toolchain/_index.html)
chapter.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the compiler pipeline, crate map, and how data flows from source to binary.

## Runtime Models

Every FOL artifact selects a capability mode in `build.fol`. Set `fol_model`
explicitly for `core`; omitting it selects the default `memo` mode:

- `core`: no heap and no source-level hosted runtime APIs; executable
  artifacts are still supported
- `memo`: heap-enabled (`str`, `vec`, `seq`, `set`, `map`), still no hosted
  runtime APIs; executable artifacts are still supported

As a Rust-oriented mental model, `core` is the FOL-source analogue of
`#![no_std]` plus `core`, while `memo` is the analogue of `no_std` plus an
allocator-backed `alloc` surface. This analogy describes which APIs FOL source
may use; it does not claim that the current Rust backend emits a freestanding
binary.

There is no third model. Hosted language API capability for `memo` artifacts
comes from declaring the bundled standard-library dependency at package level:

```fol
build.add_dep({ alias = "std", source = "internal", target = "standard" });
```

which upgrades the artifact's effective API tier to hosted and makes the
shipped `std` package importable (`use std: pkg = {"std"};`).

Bundled `std` gates hosted language APIs such as console and processor
facilities; it does not gate `fol code run` or `fol code test`. Launching a
host-compatible artifact or build tool is a frontend/toolchain concern,
separate from the source-language capability model.

Target compatibility is a separate execution check. `fol code build` may
produce a cross-target artifact, but the current `fol code run` and
`fol code test` paths reject a target that cannot execute on the build host.
Such an artifact needs an appropriate external runner; adding bundled `std`
does not make a foreign target locally executable.

Use the smallest mode that matches the artifact contract. The full matrix,
the effective-tier derivation, and examples are in
[docs/runtime-models.md](docs/runtime-models.md) and
[docs/bundled-std.md](docs/bundled-std.md).

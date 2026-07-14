<p align="center">
    <img alt="logo" src="./book/src/images/logo.svg" width="300px">
</p>


<a href="https://follang.github.io/" style="color: rgb(179, 128, 255)"></a><h2><p align="center" style="color: rgb(179, 128, 255)">https://follang.github.io/</p></h2></a>

<p align="center">
  <a href="https://github.com/follang/fol/blob/develop/LICENSE.md"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License: MIT"></a>
  <a href="https://travis-ci.org/follang/fol"><img alt="Travis (.org)" src="https://img.shields.io/travis/follang/fol"></a>
  <a href="https://codecov.io/github/follang/fol"><img alt="Codecov" src="https://img.shields.io/codecov/c/github/follang/fol"></a>
  <a href="https://gitter.im/follang/community"><img alt="Gitter" src="https://img.shields.io/gitter/room/bresilla/follang"></a>
  <a href="https://github.com/follang/fol/blob/develop/.all-contributorsrc"><img src="https://img.shields.io/badge/all_contributors-1-orange.svg" alt="Contributors"></a>
</p>

<p align="center">general-purpose and systems programming language</p>
<hr>


FOL is a general-purpose, systems programming language designed for robustness, efficiency, portability, expressiveness and most importantly elegance. Heavily inspired (and shamelessly copying) from languages: rust, zig, nim, c, go, and cpp. In Albanian language "fol" means "to speak".

<p align="center">  ** FOL IS IN ACTIVE DEVELOPMENT **  </p>

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

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

Every FOL artifact declares a capability mode (`fol_model`) in `build.fol`:

- `core`: no heap, no hosted runtime services
- `memo`: heap-enabled (`str`, `vec`, `seq`, `set`, `map`), still no hosted
  runtime services

There is no third model. Hosted capability comes from declaring the bundled
standard-library dependency on a `memo` artifact:

```fol
build.add_dep({ alias = "std", source = "internal", target = "standard" });
```

which upgrades the artifact's effective runtime tier to hosted and makes the
shipped `std` package importable (`use std: pkg = {"std"};`).

Use the smallest mode that matches the artifact contract. The full matrix,
the effective-tier derivation, and examples are in
[docs/runtime-models.md](docs/runtime-models.md) and
[docs/bundled-std.md](docs/bundled-std.md).

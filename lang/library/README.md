# Bundled FOL Libraries

This tree contains FOL library source that ships with the toolchain.

Current intent:

- `std` lives here as bundled source
- users should not download `std` separately for normal usage
- `core` and `memo` remain compiler/runtime capability modes, not importable libraries
- `core` is the no-FOL-heap source surface; `memo` adds alloc-like FOL values
- host-compatible artifacts in either mode may run and test without bundled
  `std`; the library gates hosted source APIs, not process launching

The normal bundled standard-library root is:

- `lang/library/std`

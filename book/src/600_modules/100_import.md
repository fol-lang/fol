# Imports

An import declaration states that the source file containing the declaration depends on functionality of the imported package and enables access to exported identifiers of that package.

Syntax to import a library is:
```
use alias: source_kind = {"source"}
```

Current source kinds are:
- `loc` for local directory imports
- `pkg` for formal dependency packages, including installed/fetched packages
  and a declared bundled-standard alias

## What `use` imports

`use` works against the source-layout model:

- the folder root is the package
- subfolders are namespaces inside that package
- a file is only a source file, not a separate module by itself

So a `use` target is normally:

- a whole package
- or one namespace inside that package

`use` does **not** mean "import another file from the same folder".
Files in the same package already share package scope.
Imports are for reaching another package or another namespace boundary.

Also note:

- `use` brings in exported functionality
- `hid` declarations remain file-only even inside the same package
- importing a package does not erase the original file boundary rules
- `use` is for consuming functionality, not for defining package dependencies

## Import kinds

### `loc`

`loc` is the simplest import kind:

- it points to a local directory
- that directory is scanned as a FOL package / namespace tree
- no `build.fol` is required

This makes `loc` useful for local workspace code, experiments, and monorepo-style sharing.

### `pkg`

`pkg` is for formal dependency packages. The dependency may come from an
installed/fetched external package or from the explicitly declared internal
`standard` target; source syntax does not gain a separate `std` source kind.

Unlike `loc`, a `pkg` import does not just point at an arbitrary source directory.
It points at an installed package root that must define its identity and build surface explicitly.
The package layer discovers that root first, and ordinary name resolution happens only after the package has been prepared.

For a `pkg` package root:

- `build.fol` is required
- `build.fol` stores package metadata, direct dependencies, and build logic

## Package Metadata And Build Files

Formal packages use one control file at the root:

- `build.fol`

### `build.fol`

`build.fol` is the package build entry file.

This file is responsible for:

- declaring package metadata
- declaring direct dependencies
- declaring package build logic
- declaring artifacts, steps, and generated outputs through the build API
- becoming the canonical package entrypoint for `fol code build/run/test/check`

`build.fol` is still an ordinary FOL file.
It is parsed with the same front-end as other `.fol` sources.
The difference is that the package layer evaluates one canonical build routine inside it.

Today that means:

- `fol code build/run/test/check` starts from `build.fol`
- the canonical entry is `pro[] build(): non`
- old `def root: loc = ...` and `def build(...)` forms are not the build model

So:

- `def` is still a general FOL declaration form
- `build.fol` is not a separate mini-language
- `build.fol` uses an ordinary routine entrypoint, like Zig's `build.zig`

That means:

- ordinary source `.fol` files use `use` to consume packages/namespaces
- `build.fol` uses `pro[] build(): non` plus `.build()` to configure package metadata,
  direct dependencies, and the build graph

So `use` and the build routine serve different jobs:

- `use` = consume functionality
- `pro[] build()` in `build.fol` = define package/build surface

## Standard library
Bundled std is reached through the dependency system. In `build.fol`, add:

```
build.add_dep({
    alias = "std",
    source = "internal",
    target = "standard",
});
```

Then import from the `std` dependency alias with `pkg`:

```
use std: pkg = {"std"};

fun[] main(): int = {
    return std::fmt::answer();
};
```
To use only one namespace of `fmt`:
```
use std: pkg = {"std"};

fun[] main(): int = {
    return std::fmt::math::answer();
};
```

Using the bundled `std.io` bootstrap surface:
```
use std: pkg = {"std"};

fun[] main(): int = {
    var shown: str = std::io::echo_str("hello");
    return 7;
};
```
## Local libraries
To include a local package or namespace, point `loc` at the directory:

```
use bend: loc = {"../folder/bender"};
```
Then to acces only a namespace:
```
use space: loc = {"../folder/bender/space"};
```

That second form is namespace import, not "single file import".
If `space` contains multiple `.fol` files in the same folder, they still belong to the same imported namespace.

`loc` does not require `build.fol`.
But if the target directory already defines `build.fol` at its root, that directory is treated as a formal package root and should be imported through `pkg`, not `loc`.

## External packages
External packages are imported through their declared `pkg` dependency alias:

```
use space: pkg = {"space"};
```

Nested installed package paths use the same quoted target rule:

```
use nested: pkg = {"other/package/nested"};
```

Old unquoted targets are invalid and should fail in the parser:

```fol
use std: pkg = {std};
```

`pkg` imports are different from `loc`:

- the imported root is an installed package root
- that root must contain `build.fol`
- `build.fol` is the package control file and currently defines package metadata,
  dependencies, exports, and build declarations that package loading depends on
- raw transport URLs do not appear in source code; package acquisition and installed-package preparation are separate from ordinary source resolution

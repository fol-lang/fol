# Build Options

Build options are named values passed from the command line into `build.fol`.
They follow Zig's `-D` convention.

## Syntax

```text
fol code build -Dname=value
```

Multiple options can be passed in one command:

```text
fol code build -Dtarget=x86_64-linux-gnu -Doptimize=release-fast -Dstrip=true
```

## Standard Options

Two options are pre-defined by the build system: `target` and `optimize`.
They are read via dedicated graph methods.

### Target

```fol
var target = graph.standard_target();
```

CLI: `-Dtarget=arch-os-env`

Format: `arch-os-env` triple. Examples:

| Triple                | Meaning                        |
|-----------------------|--------------------------------|
| `x86_64-linux-gnu`    | x86-64 Linux with glibc        |
| `x86_64-linux-musl`   | x86-64 Linux with musl libc    |
| `aarch64-linux-gnu`   | ARM64 Linux with glibc         |
| `aarch64-linux-musl`  | ARM64 Linux with musl libc     |
| `x86_64-macos-gnu`    | x86-64 macOS                   |
| `aarch64-macos-gnu`   | ARM64 macOS                    |
| `x86_64-windows-gnu`  | x86-64 Windows with MinGW      |
| `x86_64-windows-msvc` | x86-64 Windows with MSVC       |
| `aarch64-windows-msvc` | ARM64 Windows with MSVC       |

Supported architectures: `x86_64`, `aarch64`.
Supported operating systems: `linux`, `macos`, `windows`.
Supported environments: `gnu`, `musl`, `msvc`.

If not set, the host target is used.

`standard_target()` always exposes the compact FOL spelling shown above, so
`when` and `case` comparisons are stable even when the CLI accepted a canonical
Rust triple. Artifact identity and the backend separately retain the matching
canonical Rust triple (for example, `x86_64-unknown-linux-gnu`).

To pass the resolved value to an artifact:

```fol
var target = graph.standard_target();
var app = graph.add_exe({
    name   = "app",
    root   = "src/main.fol",
    target = target,
});
```

### Optimize

```fol
var optimize = graph.standard_optimize();
```

CLI: `-Doptimize=mode`

Valid modes:

| Mode            | Meaning                              |
|-----------------|--------------------------------------|
| `debug`         | No optimization, full debug info     |
| `release-safe`  | Optimized with safety checks         |
| `release-fast`  | Maximum speed, no safety checks      |
| `release-small` | Minimize binary size                 |

Default: `debug`. A `fol code ... --release` request supplies
`release-safe` when no explicit optimize value is present. An explicit
`-Doptimize=...` or `--optimize ...` value takes precedence and also determines
whether the artifact uses the debug or release output/rustc profile, so the
directory identity cannot disagree with the compiler profile.

To pass the resolved value to an artifact:

```fol
var optimize = graph.standard_optimize();
var app = graph.add_exe({
    name     = "app",
    root     = "src/main.fol",
    optimize = optimize,
});
```

## User Options

`graph.option(...)` declares a named option specific to the package.

```fol
var strip = graph.option({
    name    = "strip",
    kind    = "bool",
    default = false,
});
```

CLI: `-Dstrip=true`

### Option Kinds

| Kind       | Example CLI               | Default type |
|------------|---------------------------|--------------|
| `bool`     | `-Dverbose=true`          | `false`      |
| `int`      | `-Djobs=8`                | `0`          |
| `str`      | `-Dprefix=/usr`           | `""`         |
| `enum`     | `-Dbackend=llvm`          | first value  |
| `path`     | `-Droot=src/main.fol`     | `""`         |
| `target`   | `-Dtarget=x86_64-linux-gnu` | host target |
| `optimize` | `-Doptimize=release-fast` | `debug`      |

### Using Option Values

Option handle values can be interpolated into strings or compared with `==`:

```fol
var root_opt = graph.option({ name = "root", kind = "path", default = "src/main.fol" });
var app = graph.add_exe({ name = "app", root = root_opt });
```

Comparing in a `when` condition:

```fol
var strip = graph.option({ name = "strip", kind = "bool", default = false });
when(strip == true) {
    {
        var strip_step = graph.step("strip");
    }
};
```

## Shorthand vs Long Form

Both of these are equivalent:

```text
fol code build -Dtarget=x86_64-linux-gnu
fol code build --build-option target=x86_64-linux-gnu
```

`-D` is the shorthand. `-Dtarget=` and `-Doptimize=` route to the dedicated
standard option slots. All other `-Dname=value` pairs route to user-declared
options.

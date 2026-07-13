# Graph API

`build.graph()` is the public access point to the build graph in `build.fol`.
All graph construction goes through method calls on the returned handle.

This layer is intentionally narrower than the whole build surface:

- package metadata belongs on `build.meta({...})`
- direct dependencies belong on `build.add_dep({...})`
- future dependency queries and output handles sit above raw graph mutation and
  should not collapse back into ad hoc string-only graph APIs

The canonical shape is:

```fol
pro[] build(): non = {
    var build = .build();
    build.meta({ name = "app", version = "0.1.0" });
    var graph = build.graph();
    ...
}
```

## Artifacts

### `graph.add_exe`

Adds an executable artifact.

```fol
var app = graph.add_exe({
    name     = "app",
    root     = "src/main.fol",
    fol_model = "memo",   // spell this explicitly for each artifact
    target   = target,    // optional
    optimize = optimize,  // optional
});
```

Returns an `Artifact` handle.

Required fields: `name`, `root`.
Optional fields: `fol_model`, `target`, `optimize`.

`root` is the path to the entry-point `.fol` source file relative to the
package root.

The resolved root must be a real source file contained by that package. The
frontend canonicalizes the path before checking containment, so neither `..`
nor a symlink may make an artifact root escape the package.

For an artifact-targeted build, the directory containing `root` is the source
scope: FOL recursively parses that directory and checks every source in it
under the artifact's `fol_model`. A package may declare both `core` and `memo`
artifacts only when their source scopes are disjoint. The scopes overlap, and
the graph is rejected, when they are the same directory or one contains the
other. Put different-model artifacts in sibling directories or separate
package members; do not put `core.fol` and `memo.fol` beside each other in one
recursively parsed source directory.

Package-wide commands that require one capability model reject mixed-model
packages. Use an exact artifact or named build step when targeting one of the
disjoint scopes.

`fol_model` selects one of the two runtime capability modes for the artifact:

- `core`
  no heap, no OS/runtime services
- `memo`
  heap-backed facilities, still no OS/runtime services

Bundled `std` is not a third `fol_model`. A `memo` artifact reaches the hosted
tier only when the package explicitly declares the bundled internal dependency:

```fol
build.add_dep({ alias = "std", source = "internal", target = "standard" });
```

Source code reaches that dependency through its declared alias, for example
`use std: pkg = {"std"};`.

The recommended style is to spell `fol_model` explicitly on every artifact so
the capability contract is visible in `build.fol`.

The important boundary is semantic and runtime-facing:

- `core` artifacts must not use heap-backed `str`, `vec`, `seq`, `set`, or
  `map`
- `core` artifacts may still use arrays, records, routines, control flow, and
  `dfr`
- `memo` artifacts may use heap-backed runtime types but not hosted services
- bundled `std` wrappers require an explicit internal `standard` dependency
- hosted wrappers, processor features, and host-executed `run` / `test` routes
  require `fol_model = "memo"` plus that explicit bundled `std` dependency

Current implementation note:

- `core` already means “no heap and no OS/runtime services” at the language and
  runtime-contract level
- `core` artifacts are still emitted through the current Rust backend pipeline
- that means `core` is ready for semantic/runtime restriction work now, but it
  should not yet be read as “finished embedded backend support”

Mixed-model example:

```fol
pro[] build(): non = {
    var build = .build();
    build.meta({ name = "workspace_tools", version = "0.1.0" });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var corelib = graph.add_static_lib({
        name = "corelib",
        root = "core/lib.fol",
        fol_model = "core",
    });
    var memolib = graph.add_static_lib({
        name = "memolib",
        root = "memo/lib.fol",
        fol_model = "memo",
    });
    var tool = graph.add_exe({
        name = "tool",
        root = "app/main.fol",
        fol_model = "memo",
    });

    graph.install(tool);
    graph.add_run(tool);
};
```

The separate `core/`, `memo/`, and `app/` directories above are semantic, not
just organizational: they keep the recursively parsed artifact scopes
disjoint. Moving those roots into the same directory would make the `core` and
`memo` contracts overlap and the frontend would reject the targeted build.

### `graph.add_static_lib`

Adds a static library artifact.

```fol
var core = graph.add_static_lib({ name = "core", root = "src/core/lib.fol" });
```

Returns an `Artifact` handle.

Library and test artifact config records follow the same optional fields:

- `fol_model`
- `target`
- `optimize`

### `graph.add_shared_lib`

Adds a shared (dynamic) library artifact.

```fol
var sdk = graph.add_shared_lib({ name = "sdk", root = "src/sdk/lib.fol" });
```

Returns an `Artifact` handle.

### `graph.add_test`

Adds a test artifact.

```fol
var tests = graph.add_test({ name = "tests", root = "src/tests.fol" });
```

Returns an `Artifact` handle.

### `graph.add_module`

Adds a standalone module that can be imported by other artifacts.

```fol
var utils = graph.add_module({ name = "utils", root = "src/utils.fol" });
```

Returns a `Module` handle.

## Installation and Runs

### `graph.install`

Marks an artifact for installation.

```fol
graph.install(app);
```

Returns an `Install` handle.

Install projections use the selected install prefix. Artifact installs resolve
under the prefix by kind:

- executables under `bin/`
- libraries under `lib/`

### `graph.install_file`

Installs either a source-file handle or a generated output handle.

```fol
var defaults = graph.file_from_root("config/defaults.toml");
graph.install_file({ name = "defaults", source = defaults });
```

```fol
var cfg = graph.write_file({
    name = "cfg",
    path = "config/generated.toml",
    contents = "ok",
});

graph.install_file({ name = "generated-cfg", source = cfg });
```

Returns an `Install` handle.

### `graph.install_dir`

Installs a source directory handle.

```fol
var assets = graph.dir_from_root("assets");
graph.install_dir({ name = "assets", source = assets });
```

Returns an `Install` handle.

The install prefix is selected by frontend/workspace configuration, not by the
package itself. Changing the prefix should move projected install destinations
without changing `build.fol`.

### `graph.add_run`

Registers an artifact as a run target. Binds the default `run` step when only
one executable exists and no explicit `run` step has been registered.

```fol
var run = graph.add_run(app);
```

Returns a `Run` handle. See [Handle API](./300_handle_api.md) for `Run` methods.

## Steps

### `graph.step`

Creates a named custom step.

```fol
var docs = graph.step("docs");
var docs = graph.step("docs", "Generate documentation");
```

Returns a `Step` handle. See [Handle API](./300_handle_api.md) for `Step` methods.

Named steps are selectable on the command line:

```text
fol code build docs
fol code build --step docs
```

When a description is present, frontend step planning and unknown-step
diagnostics surface it as part of the available step catalog.

## Native System Libraries

### `graph.add_system_lib`

Declares a typed system-library request that can be linked into an artifact.

```fol
var ssl = graph.add_system_lib({
    name = "ssl",
    mode = "dynamic",
    search_path = "/usr/lib",
});

app.link(ssl);
```

Supported config fields:

- `name`
  required library or framework name
- `mode`
  optional, defaults to `dynamic`, also accepts `static`
- `framework`
  optional bool, defaults to `false`
- `search_path`
  optional system library search root

Current scope is intentionally narrow:

- library names are typed, not raw linker fragments
- frameworks are expressed through `framework = true`
- frameworks must use `dynamic`
- the build graph records native link intent without exposing a linker-script DSL

## Dependency Config Notes

Forwarded dependency args are already best suited for stable build config:

- `target`
- `optimize`
- named user options

The current build story intentionally does not treat dependency config as a
general environment-selection surface. Keep forwarded dependency args explicit
and typed rather than mixing them with ad hoc environment shaping.

## Generated Directories

### `graph.add_system_tool_dir`

Declares a typed system-tool step that produces a directory handle.

```fol
var assets = graph.add_system_tool_dir({
    tool = "assetpack",
    output_dir = "gen/assets",
});

graph.install_dir({ name = "assets", source = assets });
```

### `graph.add_codegen_dir`

Declares a codegen step that produces a directory handle.

```fol
var docs = graph.add_codegen_dir({
    kind = "asset",
    input = "assets/raw",
    output_dir = "gen/packed",
});
```

Generated directory handles are valid in directory-oriented consumers:

- `graph.install_dir`
- `build.export_dir`

## Current Execution Semantics

Step execution is still serial today. The build graph keeps deterministic step
ordering and explicit dependency edges, but it does not claim parallel
execution yet.

Current reporting distinguishes:

- requested
- executed
- skipped-from-cache
- skipped-by-foreign-run-policy
- produced outputs

That reporting is intended for frontend summaries and tests. Produced outputs
now participate in step cache-key semantics, so generated-file changes can
invalidate dependent steps predictably.

## Options

### `graph.standard_target`

Reads the `-Dtarget` option. Returns a `Target` handle.

```fol
var target = graph.standard_target();
```

The value is provided at build time via `-Dtarget=x86_64-linux-gnu`. If no
value is provided, `target` resolves to the host target.

An optional config record is accepted:

```fol
var target = graph.standard_target({ default = "x86_64-linux-gnu" });
```

### `graph.standard_optimize`

Reads the `-Doptimize` option. Returns an `Optimize` handle.

```fol
var optimize = graph.standard_optimize();
```

The value is provided via `-Doptimize=release-fast`. Defaults to `debug` if
not set.

Valid values: `debug`, `release-safe`, `release-fast`, `release-small`.

### `graph.option`

Declares a named user option readable via `-D`.

```fol
var root_opt = graph.option({
    name    = "root",
    kind    = "path",
    default = "src/main.fol",
});
```

Returns a `UserOption` handle.

Required fields: `name`, `kind`.
Optional field: `default`.

Option kinds:

| Kind       | Description                    | CLI Example            |
|------------|--------------------------------|------------------------|
| `bool`     | Boolean flag                   | `-Dstrip=true`         |
| `int`      | Integer value                  | `-Djobs=4`             |
| `str`      | Arbitrary string               | `-Dprefix=/usr/local`  |
| `enum`     | One of a fixed set of strings  | `-Dbackend=llvm`       |
| `path`     | File or directory path         | `-Droot=src/main.fol`  |
| `target`   | Target triple                  | `-Dtarget=x86_64-linux-gnu` |
| `optimize` | Optimization mode              | `-Doptimize=release-fast`   |

## Generated Files

### `graph.write_file`

Declares a file to be written with static contents at build time.

```fol
var header = graph.write_file({
    name     = "version.h",
    path     = "gen/version.h",
    contents = "#define VERSION 1\n",
});
```

Returns a `GeneratedFile` handle.

### `graph.copy_file`

Declares a file to be copied from a source-file handle.

```fol
var template = graph.file_from_root("config/template.toml");
var cfg = graph.copy_file({
    name   = "config",
    source = template,
    dest   = "gen/config.toml",
});
```

Returns a `GeneratedFile` handle.

### `graph.add_system_tool`

Declares a system tool invocation that produces a file.

```fol
var packed = graph.add_system_tool({
    tool = "flatc",
    args = { "--fol" },
    file_args = { schema, defaults },
    env = { MODE = "strict" },
    output = "gen/schema.fol",
});
```

Returns a `GeneratedFile` handle.

The generated file is keyed by the output path. Use this handle with
`step.attach(...)` or `artifact.add_generated(...)`.

Typed fields:

- `tool`
  required tool name
- `args`
  optional string arguments
- `file_args`
  optional source-file or generated-output handles passed as file arguments
- `env`
  optional string-to-string environment map
- `output`
  required generated output path

### `graph.add_codegen`

Declares a FOL codegen step.

```fol
var schema = graph.add_codegen({
    kind   = "schema",
    input  = "schema/api.yaml",
    output = "gen/api.fol",
});
```

Returns a `GeneratedFile` handle.

Codegen kinds: `fol-to-fol`, `schema`, `asset-preprocess`.

## Path Utilities

### `graph.file_from_root`

Returns a source-file handle rooted under the package root.

```fol
var cfg = graph.file_from_root("config/default.toml");
```

Useful when passing source files into `copy_file`, `install_file`, or
`run.add_file_arg`.

### `graph.dir_from_root`

Returns a source-dir handle rooted under the package root.

```fol
var assets = graph.dir_from_root("assets");
```

### `graph.build_root`

Returns the package root directory as an absolute path string.

```fol
var root = graph.build_root();
```

### `graph.install_prefix`

Returns the installation prefix. Defaults to the workspace install directory.

```fol
var prefix = graph.install_prefix();
```

## Dependencies

Direct dependencies are declared on `build.add_dep({...})`, not on `graph`.

See [Handle API](./300_handle_api.md) for querying modules, artifacts, steps,
and generated outputs from a `Dependency` handle.

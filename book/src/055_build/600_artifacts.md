# Artifacts, Modules, and Generated Files

The build graph tracks three kinds of compilable or producible outputs:
artifacts, modules, and generated files.

## Artifacts

Artifacts are the primary compiled outputs of a package.

| Kind           | Method              | Output                 |
|----------------|---------------------|------------------------|
| Executable     | `graph.add_exe`     | Binary                 |
| Static library | `graph.add_static_lib` | `.a` / `.lib`       |
| Shared library | `graph.add_shared_lib` | `.so` / `.dylib`    |
| Test bundle    | `graph.add_test`    | Runnable test binary   |

All artifact constructors accept the same base config record:

```fol
var app = graph.add_exe({
    name     = "app",      // required: output name
    root     = "src/main.fol",  // required: entry-point source file
    fol_model = "core",    // optional: core or memo (memo if omitted)
    target   = target,     // optional: Target handle
    optimize = optimize,   // optional: Optimize handle
});
```

`fol_model` governs source-visible capabilities, not whether the artifact is a
binary. Both `core` and `memo` can produce runnable executables and test
bundles. A `memo` artifact declares bundled `std` separately only when its
source needs hosted APIs.

`name` must be lowercase. Allowed characters: `a-z`, `0-9`, `-`, `_`, `.`.

### Artifact Name Validation

The build system validates artifact names at evaluation time. Invalid names
(uppercase, spaces, special characters) produce a build error.

### Linking

Static and shared libraries can be linked into executables using
`artifact.link(dep)`:

```fol
var core = graph.add_static_lib({ name = "core", root = "src/core/lib.fol" });
var app  = graph.add_exe({ name = "app", root = "src/main.fol" });
app.link(core);
```

Typed system-library requests can be linked the same way:

```fol
var ssl = graph.add_system_lib({ name = "ssl", mode = "dynamic" });
app.link(ssl);
```

Framework-style requests use the same surface:

```fol
var metal = graph.add_system_lib({
    name = "Metal",
    framework = true,
});
app.link(metal);
```

Linking is transitive through the graph. If `core` itself links `utils`, `app`
will also see `utils`.

### Installation

Any artifact can be marked for installation:

```fol
graph.install(app);
graph.install(core);
```

Files and directories can also be installed directly:

```fol
var defaults = graph.file_from_root("config/defaults.toml");
var assets = graph.dir_from_root("assets");
graph.install_file({ name = "defaults", source = defaults });
graph.install_dir({ name = "assets", source = assets });
```

## Modules

Modules are named source units that can be shared across artifacts without
being standalone binaries.

```fol
var utils = graph.add_module({ name = "utils", root = "src/utils.fol" });
var app   = graph.add_exe({ name = "app", root = "src/main.fol" });
app.import(utils);
```

`artifact.import(module)` makes the module visible in the importing artifact's
source scope. Equivalent to Zig's `artifact.root_module.addImport(name, dep)`.

Modules from dependencies are accessed via `dep.module(name)`:

```fol
var build  = .build();
var dep    = build.add_dep({ alias = "mylib", source = "loc", target = "../mylib" });
var logger = dep.module("logger");
app.import(logger);
```

`dep.module(name)` resolves only explicitly exported build modules. It does not
change the ordinary package import rules used in source files.

## Generated Files

Generated files are outputs produced before compilation. They must be declared
in the graph so the build system knows to produce them and what depends on them.

### Kinds

| Kind          | Method                   | Description                           |
|---------------|--------------------------|---------------------------------------|
| Write         | `graph.write_file`       | Written with literal string contents  |
| Copy          | `graph.copy_file`        | Copied from a source path             |
| Tool output   | `graph.add_system_tool`  | Produced by an external tool          |
| Tool dir      | `graph.add_system_tool_dir` | Produced as a generated directory |
| Codegen       | `graph.add_codegen`      | Produced by the FOL codegen pipeline  |
| Codegen dir   | `graph.add_codegen_dir`  | Produced as a generated directory     |
| Captured run  | `run.capture_stdout()`   | Stdout of a run step                  |

### Connecting Generated Files

A generated file must be connected to the graph entity that depends on it.

Attach to a step (the step triggers its production):

```fol
var schema = graph.file_from_root("schema/api.yaml");
var defaults = graph.write_file({
    name = "defaults",
    path = "gen/defaults.txt",
    contents = "strict",
});
var gen  = graph.add_system_tool({
    tool = "flatc",
    args = { "--fol" },
    file_args = { schema, defaults },
    env = { MODE = "strict" },
    output = "gen/types.fol",
});
var step = graph.step("proto");
step.attach(gen);
```

Add to an artifact (artifact cannot compile without it):

```fol
var schema = graph.add_codegen({
    kind   = "schema",
    input  = "schema/api.yaml",
    output = "gen/api.fol",
});
var app = graph.add_exe({ name = "app", root = "src/main.fol" });
app.add_generated(schema);
```

Capture stdout and feed it as an arg to another run:

```fol
var gen_tool   = graph.add_exe({ name = "gen", root = "tools/gen.fol" });
var gen_run    = graph.add_run(gen_tool);
var gen_output = gen_run.capture_stdout();

var app_run = graph.add_run(app);
app_run.add_file_arg(gen_output);
```

Generated directories can be installed or exported as dirs:

```fol
var assets = graph.add_system_tool_dir({
    tool = "assetpack",
    output_dir = "gen/assets",
});

build.export_dir({ name = "assets", dir = assets });
graph.install_dir({ name = "assets", source = assets });
```

## Steps

Steps are named build phases. The build system has several implicit steps
(`build`, `run`, `test`, `install`). Custom steps are declared with
`graph.step`.

```fol
var docs = graph.step("docs", "Generate documentation");
```

Steps are executed in dependency order. A step depends on another step via
`step.depend_on`:

```fol
var compile = graph.step("compile");
var docs    = graph.step("docs");
docs.depend_on(compile);
```

### Default Step Bindings

When only one executable exists in a package, the build system automatically
binds it to the default `build` and `run` steps. When multiple executables
exist, explicit step bindings are required via `graph.add_run(artifact)`.

These bindings select what the frontend launches; they do not upgrade the
artifact to hosted `std`. `fol code run` and `fol code test` accept std-free
`core` and `memo` artifacts when the evaluated target matches the host.

### Selecting Steps at the Command Line

```text
fol code build         # run the install steps
fol code build docs    # run the "docs" step
fol code run           # run the default run step
fol code run --step serve  # run the "serve" step
fol code test          # run test steps
```

Cross-target artifacts can still be built. The current frontend has no runner
configuration, so `run` and `test` reject a foreign selected target before
launch instead of treating bundled `std` as execution permission.

## Graph Validation

After the build program executes, the graph is validated:

- No step dependency cycles
- All artifact inputs (modules, generated files) are resolvable
- Install targets point to declared artifacts

Validation errors are reported as build evaluation errors before any
compilation begins.

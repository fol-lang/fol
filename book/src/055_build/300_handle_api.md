# Handle API

Graph methods return typed handles. Each handle type exposes its own set of
methods for configuring relationships and behavior.

## Path Handle Audit

Before the broader path convergence work, the public build surface still splits
path-like values across multiple families:

- `SourceFile`
- `SourceDir`
- `GeneratedFile`
- dependency-generated outputs returned by `dep.generated(...)`

Those values already compose in several places, but they do not yet read like
one obvious path capability. This chapter keeps the current concrete names while
the underlying path model converges.

## Path Convergence Direction

The intended direction is one broader path capability with:

- a path class:
  - file
  - dir
- a provenance:
  - source-root path
  - local generated output
  - dependency-exported path/output

The producer names can stay concrete:

- `graph.file_from_root(...)`
- `graph.dir_from_root(...)`
- `graph.write_file(...)`
- `graph.copy_file(...)`
- `dep.file(...)`
- `dep.dir(...)`
- `dep.path(...)`
- `dep.generated(...)`

But path consumers should read those values through one common model instead of
hand-written special cases for every producer. In practice the public rule is:

- producers stay specific
- consumers validate the incoming path by:
  - path class: file or dir
  - provenance: source, local generated, or dependency-exported

So `install_file`, `run.add_file_arg`, and `artifact.add_generated` now reject
wrong path provenance with exact diagnostics instead of falling through to
generic handle errors.

## Artifact

The `Artifact` handle is returned by `add_exe`, `add_static_lib`,
`add_shared_lib`, and `add_test`.

### `artifact.link`

Links another artifact as a dependency. The linker will include it.

```fol
var core = graph.add_static_lib({ name = "core", root = "src/core/lib.fol" });
var app  = graph.add_exe({ name = "app", root = "src/main.fol" });
app.link(core);
```

Equivalent to Zig's `artifact.linkLibrary(dep)`.

### `artifact.import`

Imports a module into this artifact's compilation scope.

```fol
var utils = graph.add_module({ name = "utils", root = "src/utils.fol" });
var app   = graph.add_exe({ name = "app", root = "src/main.fol" });
app.import(utils);
```

Equivalent to Zig's `artifact.root_module.addImport(name, module)`.

### `artifact.add_generated`

Declares that this artifact depends on a generated file being produced before
it can compile. The value stays a generated-output handle; it does not need to
be converted back into a string path.

```fol
var schema = graph.add_codegen({
    kind   = "schema",
    input  = "schema/api.yaml",
    output = "gen/api.fol",
});
var app = graph.add_exe({ name = "app", root = "src/main.fol" });
app.add_generated(schema);
```

---

## Run

The `Run` handle is returned by `graph.add_run`. All `Run` methods are
chainable and return `Run`.

A `Run` handle describes a build-host action. Creating it does not expose
hosted source APIs and does not require bundled `std`; the referenced artifact
keeps its declared `core` or `memo` capability contract. Execution still
requires a host-compatible target.

### `run.add_arg`

Appends a literal string argument to the run command.

```fol
var run = graph.add_run(app);
run.add_arg("--config").add_arg("config/default.toml");
```

### `run.add_file_arg`

Appends any file-class path handle that the run surface can consume:

- source-file handles
- local generated outputs
- dependency-exported file/path handles

```fol
var defaults = graph.file_from_root("config/defaults.toml");
var cfg = graph.copy_file({
    name   = "config",
    source = defaults,
    dest   = "gen/config.toml",
});
var run = graph.add_run(app);
run.add_file_arg(cfg);
run.add_file_arg(defaults);
```

Equivalent to Zig's `run.addFileArg(file)`.

Generated-output handles compose across the graph surface:

```fol
fun[] emit_cfg() = {
    return .build().graph().write_file({
        name = "cfg",
        path = "config/generated.toml",
        contents = "ok",
    });
}

var cfg = emit_cfg();
app.add_generated(cfg);
run.add_file_arg(cfg);
graph.install_file({ name = "install-cfg", source = cfg });
```

Dependency-exported file/path handles compose the same way on file consumers:

```fol
var dep = build.add_dep({
    alias = "shared",
    source = "loc",
    target = "deps/shared",
});
var shared_schema = dep.path("schema");
run.add_file_arg(shared_schema);
graph.install_file({ name = "schema", source = shared_schema });
```

### `run.add_dir_arg`

Appends a directory path as an argument.

```fol
var run = graph.add_run(app);
run.add_dir_arg("assets/");
```

### `run.capture_stdout`

Captures the standard output of the run command as a generated file.

```fol
var run    = graph.add_run(generator_tool);
var output = run.capture_stdout();
app.add_generated(output);
```

Returns a `GeneratedFile` handle. Equivalent to Zig's `run.captureStdOut()`.

### `run.set_env`

Sets an environment variable for the run command.

```fol
var run = graph.add_run(app);
run.set_env("LOG_LEVEL", "debug");
```

### `run.depend_on`

Makes this run step depend on another step completing first.

```fol
var codegen = graph.step("codegen");
var run     = graph.add_run(app);
run.depend_on(codegen);
```

---

## Step

The `Step` handle is returned by `graph.step`. All `Step` methods are
chainable and return `Step`.

### `step.depend_on`

Makes this step depend on another step.

```fol
var compile = graph.step("compile");
var bundle  = graph.step("bundle");
bundle.depend_on(compile);
```

### `step.attach`

Attaches a generated file production to this step. When the step runs, the
attached generated file is produced first.

```fol
var strip_tool = graph.add_system_tool({
    tool   = "strip",
    output = "gen/app.stripped",
});
var strip_step = graph.step("strip");
strip_step.attach(strip_tool);
```

---

## Install

The `Install` handle is returned by `graph.install`, `graph.install_file`,
and `graph.install_dir`.

### `install.depend_on`

Makes this install step depend on another step.

```fol
var check   = graph.step("check");
var install = graph.install(app);
install.depend_on(check);
```

---

## Dependency

The `Dependency` handle is returned by `build.add_dep({...})`. It exposes the
public surface of another package.

```fol
var build = .build();
var dep = build.add_dep({
    alias = "logtiny",
    source = "git",
    target = "git+https://github.com/bresilla/logtiny.git",
    version = "tag:v0.1.3",
    hash = "b242d319644a",
    mode = "lazy",
});
```

Dependency handles query already-declared package dependencies. They do not add
new graph mutations themselves.

Declared dependency modes:

- `eager`
- `lazy`
- `on-demand`

Current behavior:

- `eager`
  prepares package-store dependencies up front during formal package loading
- `lazy`
  keeps the declaration and defers package-store preparation until a later
  import/build path needs it
- `on-demand`
  preserves the strongest deferred intent and stays visible in summaries and
  fetch results

Dependency-handle queries resolve only names that the dependency explicitly
exports from its own `build.fol`:

```fol
var build = .build();
var graph = build.graph();
var codec = graph.add_module({ name = "codec", root = "src/codec.fol" });
var lib = graph.add_static_lib({ name = "json", root = "src/main.fol" });

build.export_module({ name = "api", module = codec });
build.export_artifact({ name = "runtime", artifact = lib });
```

If a dependency does not export a build-facing module, artifact, step, or
generated output, dependency handles do not see it.

The currently implemented explicit export kinds are:

- module
- artifact
- step
- generated output

The next missing export kinds are:

- source file
- source dir
- broader path
- generated dir

The intended next public shape is:

```fol
var cfg = graph.file_from_root("config/default.toml");
var assets = graph.dir_from_root("assets");
var schema = graph.write_file({
    name = "schema",
    path = "gen/schema.fol",
    contents = "ok",
});

build.export_file({ name = "config", file = cfg });
build.export_dir({ name = "assets", dir = assets });
build.export_path({ name = "schema", path = schema });
```

That direction keeps path exports explicit and typed instead of collapsing back
into ad hoc string registries.

Source import roots remain separate. A dependency can still be imported in
ordinary package source through its alias even when it exports no build-facing
handles at all.

Import resolution still follows the current alias-projection model under
`.fol/pkg/<alias>`. Dependency handles do not replace ordinary package imports;
they expose the build-facing surface of the already-declared dependency.

### `dependency.module`

Resolves a named module from the dependency.

```fol
var module = dep.module("logtiny");
app.import(module);
```

### `dependency.artifact`

Resolves a named artifact from the dependency.

```fol
var lib = dep.artifact("logtiny");
app.link(lib);
```

### `dependency.step`

Resolves a named step from the dependency.

```fol
var step = dep.step("check");
app_step.depend_on(step);
```

### `dependency.generated`

Resolves a named generated output from the dependency.

```fol
var types = dep.generated("bindings");
app.add_generated(types);
```

# Build Direction

This note records the near-term direction of the FOL build system.

It is not a promise to copy another build system exactly. It is the current
shape we intend to grow into from the existing `build.fol` surface.

## Current Shape

Today the public layering is:

- `.build()` for the package build context
- `build.meta({...})` for package metadata
- `build.add_dep({...})` for direct dependencies
- `build.graph()` for artifact and step graph mutation

This already covers the core FOL build contract:

- one control file
- one canonical `pro[] build(): non`
- package metadata and dependencies in the build language
- graph-based artifact construction

## What Is Still Missing

The next useful pieces are not more random graph methods. They are missing
capability classes.

### Round 3 Gap Audit

The current public build surface is materially stronger than it was before:

- dependency handles are real values
- explicit build exports exist
- source file and source dir handles exist
- generated outputs use one composable handle family
- dependency modes are public
- install-prefix projection is real
- step descriptions are real
- typed system tools exist

The remaining gaps that still matter are:

- no named dependency exports for source files, source dirs, or broader path
  values
- no dependency-handle queries for exported files, dirs, or general paths
- path capability is still split across multiple public handle families
- dependency modes are public and preserved end to end, but only package-store
  eager preloading is fully distinct today; deeper deferred preparation can
  still become more aggressive later
- CLI/help/reporting still under-exposes step, install, and output information
- there is still no typed system-library surface
- generated-directory workflows are still thin compared to generated-file flows

### Path Handle Split Today

The current path-like values are still exposed through separate concrete
families:

- `SourceFile`
- `SourceDir`
- `GeneratedFile`
- dependency-generated output handles

That split is honest today, but it is not the final intended feel. The next
path round should preserve the strong producer distinctions while making path
consumers behave as though they are working against one broader path capability.

The intended public feel is:

- producers remain explicit
- consumers validate against one common path model
- diagnostics name both:
  - the required path class
  - the actual path provenance when that matters

That means this round does not need to delete `SourceFile`, `SourceDir`, or
`GeneratedFile` immediately. It does need to stop treating every consumer as a
fresh handwritten branch over those concrete producers.

### Dependency Handles

Direct dependencies are now real build values instead of only metadata
declarations.

The current shape is:

```fol
var logtiny = build.add_dep({
    alias = "logtiny",
    source = "git",
    target = "git+https://github.com/bresilla/logtiny.git",
    version = "tag:v0.1.3",
    hash = "b242d319644a",
});
```

and then:

```fol
var logtiny_mod = logtiny.module("logtiny");
var logtiny_lib = logtiny.artifact("logtiny");
var logtiny_gen = logtiny.generated("bindings");
```

The current dependency import model stays explicit:

- ordinary source imports still resolve by alias projection under `.fol/pkg/<alias>`
- dependency handles query the build-facing surface of that package
- dependency handles only see explicit exports from the dependency package

That public contract is:

```fol
build.export_module({ name = "api", module = codec });
build.export_artifact({ name = "runtime", artifact = lib });
build.export_step({ name = "check", step = docs });
build.export_output({ name = "schema", output = bindings });
```

This keeps source imports and build-surface queries separate instead of
collapsing them into one implicit registry.

### Output Handles

Generated and copied files now use one output-handle family that can represent:

- files written by `graph.write_file(...)`
- files copied by `graph.copy_file(...)`
- captured stdout from run/system-tool/codegen steps
- generated files exported by dependency packages

This behaves like one composable build value instead of a pile of unrelated
string paths. The same handle family now covers local and dependency-generated
outputs alike.

### Explicit Dependency Arguments

Dependencies now accept explicit forwarded build arguments:

```fol
var dep = build.add_dep({
    alias = "json",
    source = "pkg",
    target = "json",
    args = {
        target = graph.standard_target(),
        optimize = graph.standard_optimize(),
        use_fast_parser = true,
    },
});
```

The important rule is explicitness:

- no ambient forwarding
- no hidden inheritance
- the package author chooses what to pass

### Install Prefix

The build graph now describes what gets installed, while the output prefix
stays a user/tool choice rather than something hardcoded into the package.

That means FOL should continue to separate:

- build/cache internals
- fetched dependency storage
- final installed outputs

### Step Execution

Step execution is still serial today. The current work is about making cache
boundaries and reporting honest and explicit, not pretending the executor is
already parallel.

The current reporting direction is:

- requested
- executed
- skipped-from-cache
- skipped-by-foreign-run-policy
- produced outputs

### System Integration

System integration now exists as a narrow typed surface through
`graph.add_system_tool({...})`.

The current typed inputs are:

- `tool`
- `args`
- `file_args`
- `env`
- `output`

That is intentionally smaller than a full native-toolchain DSL. It is enough
to model external build tools without collapsing back into vague stringly build
helpers.

What is still missing here:

- first-class system-library requests
- provider selection
- richer native linking policy
- parallel command execution

## What This Does Not Mean

This direction does not mean:

- reintroducing public `Graph` or `Build` types
- collapsing package metadata back into YAML
- making dependency behavior implicit
- turning the graph API into a stringly catch-all
- introducing a separate build manifest file
- pretending step execution is parallel before it is
- adding compatibility string-path fallbacks beside typed path handles
- growing a broad shell-script DSL inside `build.fol`
- reintroducing public build type names just to expose more features

The current design constraint remains:

- package metadata through `build.meta`
- direct dependencies through `build.add_dep`
- graph mutation through `build.graph`

The next round should add richer values on top of that layering, not replace it.

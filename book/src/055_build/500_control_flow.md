# Control Flow in build.fol

`build.fol` is a real FOL program. It supports `when`, `loop`, and user-defined
helper routines.

## when

`when` conditionally executes build operations based on a boolean expression.

The canonical form for a conditional block with no case arms uses a
double-brace default body:

```fol
when(optimize == "release-fast") {
    {
        var strip_step = graph.step("strip");
        var packed = graph.add_system_tool({
            tool   = "strip",
            output = "gen/app.stripped",
        });
        strip_step.attach(packed);
    }
};
```

With explicit `case` arms:

```fol
when(target) {
    case("x86_64-linux-gnu") {
        graph.step("asan", "Enable address sanitizer");
    }
    case("aarch64-linux-gnu") {
        graph.step("tsan", "Enable thread sanitizer");
    }
    * {
        graph.step("default-check");
    }
};
```

The inner double-brace `{ { ... } }` is required for the default arm when no
`case` clauses are present. This is how the FOL parser distinguishes the
default body from raw statements.

### Conditional Expressions

Any boolean or option value comparison is valid:

```fol
when(optimize == "release-fast") { { ... } };
when(strip == true) { { ... } };
when(target == "x86_64-linux-gnu") { { ... } };
```

The comparison is resolved at build evaluation time using the values passed
via `-D` flags. If no value was provided, the declared default is used.

## loop

`loop` iterates over a list and runs the body for each element.

```fol
loop(name in {"core", "io", "utils"}) {
    graph.add_static_lib({ name = name, root = name });
};
```

The loop variable (`name`) is bound in scope for each iteration. It can be
used anywhere inside the body as a string value.

Loop over a list with multiple fields:

```fol
loop(name in {"core", "io"}) {
    var lib = graph.add_static_lib({ name = name, root = name });
    graph.install(lib);
};
```

The iterable is a container literal `{ elem1, elem2, ... }`. Currently only
string lists are supported as loop iterables in `build.fol`.

## Helper Routines

Current boundary:

- defining helper `fun[]`/`pro[]` routines in `build.fol` and calling them from
  `build()` is not part of the current build surface; the evaluator cannot
  execute them today
- this section describes later design work, not current behavior
- the `when`, `loop`, and `case` build control flow above is the current build
  surface

`build.fol` can define helper `fun[]` and `pro[]` routines. They are visible
only within the file — they are not exported to the package.

### Helper Function

```fol
fun[] make_lib(name: str, root: str): Artifact = {
    return .build().graph().add_static_lib({ name = name, root = root });
}

pro[] build(): non = {
    var build = .build();
    build.meta({ name = "app", version = "0.1.0" });
    var graph = build.graph();
    var core = make_lib("core", "src/core/lib.fol");
    var io   = make_lib("io",   "src/io/lib.fol");
    var app  = graph.add_exe({ name = "app", root = "src/main.fol" });
    app.link(core);
    app.link(io);
    graph.install(app);
    graph.add_run(app);
}
```

The helper `make_lib` accesses the graph through `.build().graph()`. The graph
handle is not a public type name and is not passed as a user-declared
parameter.

### Helpers Calling Helpers

Helpers can call other helpers:

```fol
fun[] lib_root(name: str): str = {
    return "src/" + name + "/lib.fol";
}

fun[] add_lib(name: str): Artifact = {
    return .build().graph().add_static_lib({ name = name, root = lib_root(name) });
}

pro[] build(): non = {
    var build = .build();
    build.meta({ name = "app", version = "0.1.0" });
    var graph = build.graph();
    var core = add_lib("core");
    var app  = graph.add_exe({ name = "app", root = "src/main.fol" });
    app.link(core);
    graph.install(app);
}
```

## Combined Example

```fol
fun[] make_lib(name: str): Artifact = {
    return .build().graph().add_static_lib({ name = name, root = name });
}

pro[] build(): non = {
    var build = .build();
    build.meta({ name = "app", version = "0.1.0" });
    var graph = build.graph();
    var target   = graph.standard_target();
    var optimize = graph.standard_optimize();
    var strip    = graph.option({ name = "strip", kind = "bool", default = false });

    loop(name in {"core", "io", "net"}) {
        make_lib(name);
    };

    var app = graph.add_exe({
        name     = "app",
        root     = "src/main.fol",
        target   = target,
        optimize = optimize,
    });
    graph.install(app);
    graph.add_run(app);

    when(strip == true) {
        {
            var strip_step = graph.step("strip");
            var packed = graph.add_system_tool({
                tool   = "strip",
                output = "gen/app.stripped",
            });
            strip_step.attach(packed);
        }
    };
}
```

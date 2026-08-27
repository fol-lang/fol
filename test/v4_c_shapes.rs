//! Every C construct the boundary meets, and what it does with each.
//!
//! `V4_CONTINUE.md` closed 35 of 35 tasks with every lane green, and the
//! boundary still could not bind Lua, SQLite, or zlib. Nothing was measuring
//! the *shape* of what crosses -- only that the shapes already chosen kept
//! working. This is that measurement.
//!
//! Each row is a minimal header, a definition that compiles, and an overlay,
//! run through `fol tool bind c`. Three verdicts, and they are not the same
//! thing:
//!
//! - `Binds` -- the manifest is written. **Not** the same as usable: a raw
//!   pointer parameter binds and is then refused at mount, so an accepted row
//!   is only evidence once a FOL program calls it.
//! - `Refused` -- declined by name, with the phrase that must appear.
//! - `Blocker` -- declined today and *should not be*. Closing one fails this
//!   test until its row moves, because a gap that quietly starts working is a
//!   gap nobody wrote down.
//!
//! Run by `make test-v4-c-shapes`. Tracked in `plan/V4_GAPS.md`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn store_root() -> PathBuf {
    repo_root().join("lang/library")
}

fn strip_ansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && matches!(chars.peek(), Some('[')) {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }
        out.push(ch);
    }
    out
}

fn interop_environment() -> Option<(String, PathBuf)> {
    let compiler = std::env::var_os("FOL_INTEROP_GCC")
        .and_then(|value| value.to_str().map(str::to_string))
        .or_else(|| {
            ["gcc", "cc"].into_iter().find_map(|candidate| {
                Command::new(candidate)
                    .arg("--version")
                    .output()
                    .is_ok_and(|out| out.status.success())
                    .then(|| which(candidate))
                    .flatten()
            })
        })?;
    let temp = std::env::var_os("FOL_INTEROP_TEMP")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let temp = temp.canonicalize().ok()?;
    Some((compiler, temp))
}

fn which(program: &str) -> Option<String> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {program}"))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|path| !path.is_empty())
}

fn require_or_skip() -> Option<(String, PathBuf)> {
    match interop_environment() {
        Some(environment) => Some(environment),
        None if std::env::var_os("FOL_H7_REQUIRED").is_some() => {
            panic!("FOL_H7_REQUIRED is set but no C toolchain or probe directory is available")
        }
        None => None,
    }
}

fn folc() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_folc"))
}

/// What `fol tool bind c` should do with a construct.
#[derive(PartialEq)]
enum Verdict {
    /// Accepted. The phrase is unused.
    Binds,
    /// Refused deliberately; the phrase must appear in the diagnostic.
    Refused(&'static str),
    /// Refused, and that is the defect. `plan/V4_GAPS.md` carries the reason.
    Blocker(&'static str),
    /// Binds, writes a manifest, and is then refused when a FOL package
    /// mounts it. The gate holds one stage late, and a manifest is written as
    /// evidence for a surface no program can use.
    BindsButUnusable(&'static str),
}

struct Shape {
    name: &'static str,
    /// Header body, inserted between the guard and `#endif`.
    header: &'static str,
    /// Definitions, so the symbols resolve and the construct is what is judged
    /// rather than a missing provider symbol.
    defs: &'static str,
    /// Overlay body after `version = 1`.
    overlay: &'static str,
    verdict: Verdict,
}

const INFALLIBLE: &str = "[routine.probe]\nerror = \"infallible\"\n";

const HANDLE: &str = "[handle.T]\ndestroy = \"probe_free\"\n\n\
     [routine.probe]\nerror = \"infallible\"\nhandle = \"T\"\n\
     handle_role = \"produces\"\n\n[routine.probe_free]\nerror = \"infallible\"\n\
     handle = \"T\"\nhandle_role = \"consumes\"\n";

const CALLBACK: &str = "[routine.probe]\nerror = \"infallible\"\n\
     callback = \"f\"\ncallback_context = \"c\"\n";

const HANDLE_MAKER: &str = "[handle.T]\ndestroy = \"probe_free\"\n\n\
     [routine.probe]\nerror = \"infallible\"\nhandle = \"T\"\n\
     handle_role = \"produces\"\ncallback = \"m\"\ncallback_context = \"none\"\n\n\
     [routine.probe_free]\nerror = \"infallible\"\n\
     handle = \"T\"\nhandle_role = \"consumes\"\n";

const BUFFER: &str = "[routine.probe]\nerror = \"infallible\"\n\
     buffer = \"b\"\nbuffer_length = \"n\"\n";

/// The corpus. Every row was run, none was reasoned about.
const SHAPES: &[Shape] = &[
    // ---- scalars and the ordinary surface -------------------------------
    Shape { name: "scalar_widths", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "uint64_t probe(uint8_t a, uint16_t b, uint32_t c);",
        defs: "uint64_t probe(uint8_t a,uint16_t b,uint32_t c){ return a+b+c; }" },
    Shape { name: "size_t_param", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "size_t probe(size_t n);", defs: "size_t probe(size_t n){ return n; }" },
    Shape { name: "many_params", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "int32_t probe(int32_t a,int32_t b,int32_t c,int32_t d,int32_t e,int32_t f,int32_t g,int32_t h);",
        defs: "int32_t probe(int32_t a,int32_t b,int32_t c,int32_t d,int32_t e,int32_t f,int32_t g,int32_t h){ return a+h; }" },
    Shape { name: "void_result", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "void probe(int32_t v);", defs: "void probe(int32_t v){ (void)v; }" },
    Shape { name: "enum_explicit_values", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "enum E { A = 5, B = 9 }; enum E probe(int32_t v);",
        defs: "enum E probe(int32_t v){ (void)v; return A; }" },
    Shape { name: "pointer_to_pointer", verdict: Verdict::BindsButUnusable("argv, and every out-pointer-to-pointer"), overlay: INFALLIBLE,
        header: "int32_t probe(char **argv);", defs: "int32_t probe(char **argv){ (void)argv; return 0; }" },

    // ---- typedefs: complete shapes resolve, opaque ones do not ----------
    Shape { name: "typedef_scalar", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "typedef int32_t myint; myint probe(myint v);", defs: "myint probe(myint v){ return v; }" },
    Shape { name: "typedef_named_struct", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "struct S { int32_t a; }; typedef struct S S_t; int32_t probe(S_t s);",
        defs: "int32_t probe(S_t s){ return s.a; }" },
    Shape { name: "typedef_anon_struct", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "typedef struct { int32_t a; } S_t; int32_t probe(S_t s);",
        defs: "int32_t probe(S_t s){ return s.a; }" },
    Shape { name: "typedef_enum", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "typedef enum { A, B } E; int32_t probe(E e);", defs: "int32_t probe(E e){ return (int32_t)e; }" },

    // ---- strings and buffers --------------------------------------------
    Shape { name: "const_char_star", verdict: Verdict::BindsButUnusable("passing a string to a C routine, which every C library takes"), overlay: INFALLIBLE,
        header: "int32_t probe(const char *s);", defs: "int32_t probe(const char *s){ return s[0]; }" },
    Shape { name: "returned_c_string", verdict: Verdict::BindsButUnusable("returning a string, equally universal"), overlay: INFALLIBLE,
        header: "const char *probe(int32_t v);", defs: "const char *probe(int32_t v){ (void)v; return 0; }" },
    Shape { name: "result_with_out_length", verdict: Verdict::BindsButUnusable("the pointer+out-length result convention"), overlay: INFALLIBLE,
        header: "const char *probe(int32_t v, size_t *len);",
        defs: "const char *probe(int32_t v, size_t *len){ (void)v; *len=0; return 0; }" },
    Shape { name: "buffer_pair_const", verdict: Verdict::Binds, overlay: BUFFER,
        header: "uint32_t probe(const uint8_t *b, size_t n);",
        defs: "uint32_t probe(const uint8_t *b, size_t n){ (void)b; return (uint32_t)n; }" },
    Shape { name: "buffer_pair_mutable", verdict: Verdict::Binds, overlay: BUFFER,
        header: "void probe(uint8_t *b, size_t n);", defs: "void probe(uint8_t *b, size_t n){ (void)b;(void)n; }" },

    // ---- opaque handles --------------------------------------------------
    Shape { name: "handle_bare_struct", verdict: Verdict::Binds, overlay: HANDLE,
        header: "struct T; struct T *probe(int32_t s); void probe_free(struct T *t);",
        defs: "struct T { int32_t v; }; struct T *probe(int32_t s){ (void)s; return 0; } void probe_free(struct T *t){ (void)t; }" },
    // Closed in GERC: the target of a typedef is a name for the record, not a
    // use of one, so an opaque record may stand there.
    Shape { name: "handle_typedef_same_name", verdict: Verdict::Binds, overlay: HANDLE,
        header: "typedef struct T T; T *probe(int32_t s); void probe_free(T *t);",
        defs: "struct T { int32_t v; }; T *probe(int32_t s){ (void)s; return 0; } void probe_free(T *t){ (void)t; }" },
    Shape { name: "handle_typedef_other_name", verdict: Verdict::Binds, overlay: HANDLE,
        header: "typedef struct T_s T; T *probe(int32_t s); void probe_free(T *t);",
        defs: "struct T_s { int32_t v; }; T *probe(int32_t s){ (void)s; return 0; } void probe_free(T *t){ (void)t; }" },
    // A callback that hands the handle back. The declared handle domain is not
    // consulted inside a callback signature, so `[A1006]` refuses `T` there
    // while the same `T *` binds fine as the routine's own return. A callback
    // returning `int32_t` beside it binds, which is how this was isolated.
    Shape { name: "handle_callback_returns_handle",
        verdict: Verdict::Blocker("an opaque handle inside a callback signature"),
        overlay: HANDLE_MAKER,
        header: "typedef struct T T; typedef T *(*maker)(int32_t); \
                 T *probe(maker m); void probe_free(T *t);",
        defs: "struct T { int32_t v; }; T *probe(maker m){ (void)m; return 0; } void probe_free(T *t){ (void)t; }" },

    // ---- structs ---------------------------------------------------------
    Shape { name: "struct_by_value", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "struct S { int32_t a; int32_t b; }; int32_t probe(struct S s);",
        defs: "int32_t probe(struct S s){ return s.a; }" },
    // Closed by naming complete records in the selection PARC is given, so it
    // stops minimising them to an opaque view, plus a FOL mapping that lends a
    // rebuilt struct for the call.
    Shape { name: "struct_by_const_pointer", verdict: Verdict::Binds,
        overlay: INFALLIBLE,
        header: "struct S { int32_t a; int32_t b; }; int32_t probe(const struct S *s);",
        defs: "int32_t probe(const struct S *s){ return s->a; }" },
    // Binds now that the definition reaches FOL, and still cannot be mounted:
    // a mutable pointer is an out-parameter whose writes have to come back,
    // and nothing copies them back yet. Only `const` is mapped.
    Shape { name: "struct_by_mut_pointer",
        verdict: Verdict::BindsButUnusable("an out-parameter struct, whose writes nothing returns"),
        overlay: INFALLIBLE,
        header: "struct S { int32_t a; int32_t b; }; void probe(struct S *s);",
        defs: "void probe(struct S *s){ s->a = 1; }" },
    // The same parameter as `struct_by_const_pointer`, accepted -- because an
    // unrelated by-value use elsewhere makes GERC materialise the definition.
    // Neither the declaration nor the overlay differs.
    Shape { name: "struct_by_pointer_beside_by_value", verdict: Verdict::Binds,
        overlay: "[routine.probe]\nerror = \"infallible\"\n\n[routine.probe_byval]\nerror = \"infallible\"\n",
        header: "struct S { int32_t a; }; int32_t probe(const struct S *s); int32_t probe_byval(struct S s);",
        defs: "int32_t probe(const struct S *s){ return s->a; } int32_t probe_byval(struct S s){ return s.a; }" },
    Shape { name: "struct_returned_by_value",
        verdict: Verdict::Refused("does not cross yet"), overlay: INFALLIBLE,
        header: "struct P { int32_t x, y; }; struct P probe(int32_t v);",
        defs: "struct P probe(int32_t v){ struct P p={v,v}; return p; }" },
    Shape { name: "struct_nested_field",
        verdict: Verdict::Refused("which is itself a record"), overlay: INFALLIBLE,
        header: "struct I { int32_t a; }; struct O { struct I i; }; int32_t probe(struct O o);",
        defs: "int32_t probe(struct O o){ return o.i.a; }" },
    Shape { name: "struct_anonymous_member",
        verdict: Verdict::Refused("anonymous struct member"), overlay: INFALLIBLE,
        header: "struct O { struct { int32_t x; }; int32_t z; }; int32_t probe(struct O o);",
        defs: "int32_t probe(struct O o){ return o.x + o.z; }" },
    Shape { name: "struct_array_field",
        verdict: Verdict::Refused("array"), overlay: INFALLIBLE,
        header: "struct S { int32_t a[4]; }; int32_t probe(struct S s);",
        defs: "int32_t probe(struct S s){ return s.a[0]; }" },
    Shape { name: "struct_enum_field", verdict: Verdict::Binds, overlay: INFALLIBLE,
        header: "enum E { A }; struct S { enum E e; }; int32_t probe(struct S s);",
        defs: "int32_t probe(struct S s){ return (int32_t)s.e; }" },
    Shape { name: "struct_function_pointer_field",
        verdict: Verdict::Refused("function pointer"), overlay: INFALLIBLE,
        header: "struct V { int32_t (*f)(void *); }; int32_t probe(struct V v);",
        defs: "int32_t probe(struct V v){ (void)v; return 0; }" },
    Shape { name: "struct_self_referential",
        verdict: Verdict::Refused("refers to itself"), overlay: INFALLIBLE,
        header: "struct N { struct N *next; int32_t v; }; int32_t probe(struct N *n);",
        defs: "int32_t probe(struct N *n){ return n->v; }" },
    Shape { name: "union_parameter",
        verdict: Verdict::Refused("is a union"), overlay: INFALLIBLE,
        header: "union U { int32_t i; float f; }; int32_t probe(union U v);",
        defs: "int32_t probe(union U v){ return v.i; }" },
    Shape { name: "union_field",
        verdict: Verdict::Refused("is a union"), overlay: INFALLIBLE,
        header: "union U { int32_t i; }; struct S { union U u; }; int32_t probe(struct S s);",
        defs: "int32_t probe(struct S s){ return s.u.i; }" },
    Shape { name: "bitfield",
        verdict: Verdict::Refused("bitfield"), overlay: INFALLIBLE,
        header: "struct B { unsigned a : 3; unsigned c : 5; }; int32_t probe(struct B v);",
        defs: "int32_t probe(struct B v){ return (int32_t)v.a; }" },
    Shape { name: "packed_struct",
        verdict: Verdict::Refused(""), overlay: INFALLIBLE,
        header: "struct __attribute__((packed)) P { char a; int32_t b; }; int32_t probe(struct P v);",
        defs: "int32_t probe(struct P v){ return v.b; }" },
    // Refused earlier now, by the probe profile rather than by FOL: naming the
    // record in the selection asks PARC for a definition it will not give.
    Shape { name: "flexible_array_member",
        verdict: Verdict::Refused("only nonzero fixed-size arra"), overlay: INFALLIBLE,
        header: "struct F { int32_t n; int32_t rest[]; }; int32_t probe(struct F *v);",
        defs: "int32_t probe(struct F *v){ return v->n; }" },

    // ---- callbacks -------------------------------------------------------
    Shape { name: "callback_inline_context_first", verdict: Verdict::Binds, overlay: CALLBACK,
        header: "int32_t probe(int32_t (*f)(void *, int32_t), void *c);",
        defs: "int32_t probe(int32_t (*f)(void*,int32_t), void *c){ return f(c,1); }" },
    // Closed by resolving the alias before asking whether the parameter is a
    // function pointer -- which is what a real header always needs.
    Shape { name: "callback_typedef", verdict: Verdict::Binds,
        overlay: CALLBACK,
        header: "typedef int32_t (*F)(void *, int32_t); int32_t probe(F f, void *c);",
        defs: "int32_t probe(F f, void *c){ return f(c,1); }" },
    Shape { name: "callback_typedef_context", verdict: Verdict::Binds, overlay: CALLBACK,
        header: "typedef void *Ctx; int32_t probe(int32_t (*f)(void *, int32_t), Ctx c);",
        defs: "int32_t probe(int32_t (*f)(void*,int32_t), void *c){ return f(c,1); }" },
    Shape { name: "callback_context_last",
        verdict: Verdict::Refused("first parameter is not the context"), overlay: CALLBACK,
        header: "int32_t probe(int32_t (*f)(int32_t, void *), void *c);",
        defs: "int32_t probe(int32_t (*f)(int32_t,void*), void *c){ return f(1,c); }" },
    // Closed: `callback_context = "none"`. The trampoline never found the
    // closure through the context, so there was no mechanism to add.
    Shape { name: "callback_no_context", verdict: Verdict::Binds,
        overlay: "[routine.probe]\nerror = \"infallible\"\ncallback = \"f\"\n\
                  callback_context = \"none\"\n",
        header: "int32_t probe(int32_t (*f)(int32_t));",
        defs: "int32_t probe(int32_t (*f)(int32_t)){ return f(1); }" },
    // Undeclared, it is still a bare function pointer and still refused: the
    // declaration is what says the provider has no context, rather than the
    // overlay having forgotten to name one.
    Shape { name: "callback_no_context_undeclared",
        verdict: Verdict::Refused("function pointer"), overlay: INFALLIBLE,
        header: "int32_t probe(int32_t (*f)(int32_t));",
        defs: "int32_t probe(int32_t (*f)(int32_t)){ return f(1); }" },

    // ---- refused for stated reasons --------------------------------------
    Shape { name: "variadic",
        verdict: Verdict::Refused("variadic"), overlay: INFALLIBLE,
        header: "int32_t probe(int32_t count, ...);", defs: "int32_t probe(int32_t count, ...){ return count; }" },
    Shape { name: "long_double",
        verdict: Verdict::Refused("long double"), overlay: INFALLIBLE,
        header: "long double probe(long double v);", defs: "long double probe(long double v){ return v; }" },
    Shape { name: "volatile_parameter",
        verdict: Verdict::Refused(""), overlay: INFALLIBLE,
        header: "int32_t probe(volatile int32_t v);", defs: "int32_t probe(volatile int32_t v){ return v; }" },
    Shape { name: "function_pointer_result",
        verdict: Verdict::Refused("function pointer"), overlay: INFALLIBLE,
        header: "typedef int32_t (*fp)(int32_t); fp probe(int32_t v);",
        defs: "static int32_t helper(int32_t v){ return v; } fp probe(int32_t v){ (void)v; return helper; }" },
    Shape { name: "array_parameter",
        verdict: Verdict::BindsButUnusable("an array decays before FOL sees it"), overlay: INFALLIBLE,
        header: "int32_t probe(int32_t values[10]);", defs: "int32_t probe(int32_t values[10]){ return values[0]; }" },
];

/// Bind one shape and report what happened.
fn bind_shape(root: &Path, shape: &Shape, compiler: &str, temp: &Path) -> (bool, String) {
    let native = root.join("native");
    std::fs::create_dir_all(&native).expect("native directory");
    std::fs::create_dir_all(root.join("interop")).expect("interop directory");
    std::fs::write(
        native.join("probe.h"),
        format!(
            "#ifndef P_H\n#define P_H\n#include <stdint.h>\n#include <stddef.h>\n{}\n#endif\n",
            shape.header
        ),
    )
    .expect("header writable");
    std::fs::write(
        native.join("probe.c"),
        format!("#include \"probe.h\"\n{}\n", shape.defs),
    )
    .expect("source writable");
    std::fs::write(
        root.join("interop/probe.toml"),
        format!("version = 1\n\n{}", shape.overlay),
    )
    .expect("overlay writable");

    let object = native.join("probe.o");
    let compiled = Command::new(compiler)
        .arg("-c")
        .arg("-o")
        .arg(&object)
        .arg(native.join("probe.c"))
        .output()
        .expect("the C compiler should run");
    assert!(
        compiled.status.success(),
        "{}: the construct must be valid C, or the row tests nothing:\n{}",
        shape.name,
        String::from_utf8_lossy(&compiled.stderr)
    );
    assert!(
        Command::new("ar")
            .arg("rcs")
            .arg(native.join("libprobe.a"))
            .arg(&object)
            .status()
            .expect("ar should run")
            .success(),
        "{}: the provider should archive",
        shape.name
    );

    let output = Command::new(folc())
        .args([
            "tool",
            "bind",
            "c",
            "--alias",
            "probe",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--header",
            "native/probe.h",
            "--provider",
            "native/libprobe.a",
            "--provider-kind",
            "static",
            "--annotations",
            "interop/probe.toml",
            "--out",
            "interop/probe.folabi.json",
        ])
        .current_dir(root)
        .env("FOL_INTEROP_GCC", compiler)
        .env("FOL_INTEROP_TEMP", temp)
        .output()
        .expect("folc should run");
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    (output.status.success(), text)
}

/// Whether a FOL package can mount what the bind produced.
///
/// Binding writes a manifest; mounting is where the surface becomes callable
/// FOL. They are different gates, and four shapes pass the first and fail the
/// second -- which is invisible unless something builds a package.
fn mounts(root: &Path, compiler: &str, temp: &Path) -> (bool, String) {
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("src directory");
    std::fs::write(
        root.join("build.fol"),
        r#"pro[] build(): non = {
    var build = .build();
    build.meta({
        name = "shape_app", version = "0.1.0",
        description = "one construct, mounted", license = "MIT",
    });
    build.add_dep({ alias = "std", source = "internal", target = "standard" });
    var graph = build.graph();
    var app = graph.add_exe({
        name = "shape_app", root = "src/main.fol", fol_model = "memo",
    });
    var header = graph.file_from_root("native/probe.h");
    var provider = graph.file_from_root("native/libprobe.a");
    var overlay = graph.file_from_root("interop/probe.toml");
    app.add_c_import({
        alias = "probe",
        header = header,
        provider = provider,
        provider_kind = "static",
        annotations = overlay,
    });
    graph.install(app);
    return;
};
"#,
    )
    .expect("build.fol writable");
    std::fs::write(
        src.join("main.fol"),
        "use std: pkg = {\"std\"};\nuse prb: pkg = {\"probe\"};\n\n\
         fun[] main(): int = {\n    var shown: int = std::io::echo_int(0);\n    return 0;\n};\n",
    )
    .expect("main.fol writable");

    let output = Command::new(folc())
        .args(["code", "build", "--package-store-root"])
        .arg(store_root())
        .current_dir(root)
        .env("FOL_INTEROP_GCC", compiler)
        .env("FOL_INTEROP_TEMP", temp)
        .output()
        .expect("folc should run");
    let text = strip_ansi(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    (output.status.success(), text)
}

/// The whole corpus, against what each construct should do.
#[test]
fn every_c_construct_lands_where_the_gap_plan_says() {
    let Some((compiler, temp)) = require_or_skip() else {
        eprintln!("skipping: no C toolchain for the shape corpus");
        return;
    };
    let fixture = fol_testkit::TempFixture::new("fol_v4_shapes");
    let mut wrong = Vec::new();

    for shape in SHAPES {
        let root = fixture.path().join(shape.name);
        let (ok, output) = bind_shape(&root, shape, &compiler, &temp);

        assert!(
            !output.contains("panicked") && !output.contains("RUST_BACKTRACE"),
            "{} panicked instead of reporting:\n{output}",
            shape.name
        );

        match &shape.verdict {
            Verdict::Binds | Verdict::BindsButUnusable(_) if !ok => wrong.push(format!(
                "{}: expected to bind, was refused:\n{output}",
                shape.name
            )),
            Verdict::Refused(phrase) | Verdict::Blocker(phrase) if ok => wrong.push(format!(
                "{}: expected refusal ({phrase}) but it bound. If this gap \
                 closed, move its row in this file and in plan/V4_GAPS.md.",
                shape.name
            )),
            Verdict::Refused(phrase) if !phrase.is_empty() && !output.contains(phrase) => wrong
                .push(format!(
                    "{}: refused, but not for {phrase:?}:\n{output}",
                    shape.name
                )),
            _ => {}
        }

        // Binding is not the gate a caller meets. Only a package proves it.
        match &shape.verdict {
            Verdict::Binds => {
                let (mounted, report) = mounts(&root, &compiler, &temp);
                if !mounted {
                    wrong.push(format!(
                        "{}: bound, then could not be mounted:\n{report}",
                        shape.name
                    ));
                }
            }
            Verdict::BindsButUnusable(why) => {
                let (mounted, report) = mounts(&root, &compiler, &temp);
                if mounted {
                    wrong.push(format!(
                        "{}: now mounts ({why}). If this gap closed, move its row \
                         here and in plan/V4_GAPS.md.",
                        shape.name
                    ));
                } else if !report.contains("uses a pointer type") {
                    wrong.push(format!(
                        "{}: refused at mount, but not as a pointer type:\n{report}",
                        shape.name
                    ));
                }
            }
            _ => {}
        }
    }

    assert!(
        wrong.is_empty(),
        "the C boundary's shape moved:\n\n{}",
        wrong.join("\n\n")
    );
}

/// The blockers, listed on their own so the count is visible.
///
/// These are not "unsupported constructs" -- those are `Refused`, decided and
/// documented. Each of these is a shape a real C library uses and the boundary
/// should accept. The number going down is the only progress that matters.
#[test]
fn the_blocker_count_is_what_the_gap_plan_records() {
    let blockers: Vec<&str> = SHAPES
        .iter()
        .filter(|shape| matches!(shape.verdict, Verdict::Blocker(_)))
        .map(|shape| shape.name)
        .collect();
    assert_eq!(
        blockers,
        vec!["handle_callback_returns_handle"],
        "plan/V4_GAPS.md names these blockers; this list and that one move together"
    );
}

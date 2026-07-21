# Versions In Depth

## Why versions coexist cleanly

Each `$FOL_HOME/toolchains/v<version>/` directory is a closed world:

- **`folc`** — the engine binary for that exact language version.
- **`std/`** — the standard library sources that version was released with.
  `folc` finds them *next to its own binary*, so a v0.2.1 engine can never
  accidentally compile against a v0.4.0 standard library.
- **`runtime/`** — the runtime crate sources. `folc` emits Rust and compiles
  it against this crate; shipping it inside the toolchain is what makes the
  directory self-sufficient on a machine that has no FOL checkout.

Because nothing inside a toolchain references anything outside it, installing,
removing, or switching versions can never corrupt another version. The shared
parts of the home — `pkg/` for third-party packages and `config` for the
default — are version-independent by design (packages are source; every
toolchain compiles them itself).

## Dispatch mechanics

`fol` decides which engine runs before any engine code executes:

```
fol [+<toolchain>] <anything else> …
      │
      ├─ `self …`?  → handled by the manager itself, never forwarded
      │
      ├─ resolve:  +arg → FOL_TOOLCHAIN → //fol pin → config default
      │            (a pinned version that is missing is fetched now)
      │
      └─ exec  $FOL_HOME/toolchains/v<X>/folc  <anything else> …
```

The manager passes your arguments through verbatim and replaces itself with
`folc` (`exec`), so there is no wrapper process at runtime. A recursion guard
(`FOL_DISPATCHED`) makes a mis-installed toolchain fail loudly instead of
looping.

Help and version flags are forwarded too: `fol --help` shows the *engine's*
help for the toolchain your project resolves to, which is why the commands you
see always match the version you are actually running. Only when nothing is
installed yet does the manager print its own copy of the help, with a hint to
`fol self install`.

## Per-project toolchains in practice

Two projects on one machine, different language versions, zero ceremony:

```console
$ head -1 ~/work/legacy/build.fol
//fol 0.2.1
$ head -1 ~/work/greenfield/build.fol
//fol 0.4.0

$ cd ~/work/legacy     && fol code build     # runs the v0.2.1 engine
$ cd ~/work/greenfield && fol code build     # runs the v0.4.0 engine
```

Editors get the same guarantee for free: the language server is started as
`fol tool lsp`, so it goes through the same resolution and always matches the
compiler that builds the project.

## One-shot overrides

```console
$ fol +0.4.0 code build        # try a newer engine without touching the pin
$ FOL_TOOLCHAIN=dev fol code test   # a whole shell session on the dev link
$ fol self which               # show which folc the current directory gets
```

# Toolchain Management

FOL ships as **two binaries with two jobs**:

| binary | role | where it lives |
|--------|------|----------------|
| `fol`  | the toolchain **manager** — the only binary on your `PATH`, the only thing a distribution packages | `/usr/bin/fol` (or anywhere on `PATH`) |
| `folc` | the toolchain **engine** — compiler, build graph, package system, LSP | inside a versioned toolchain directory, managed by `fol` |

You always type `fol`. It owns the `fol self …` command family and forwards
everything else (`fol code build`, `fol work init`, `fol tool lsp`, …) to the
right `folc` for the project you are standing in. You never invoke `folc`
yourself — if you do, `folc self` even redirects you back to `fol`.

This split is what makes multiple language versions coexist on one machine:
`fol` changes rarely and is version-agnostic; every `folc` is an immutable,
self-contained unit that can be installed, pinned, and removed independently.

## FOL_HOME

Everything the manager owns lives under a single directory:

1. **`$FOL_HOME`** if the environment variable is set — the shared,
   machine-wide home (for example `export FOL_HOME="$HOME/.fol"`).
2. Otherwise, **`<project>/.fol/toolchain`** — the manager walks up from the
   current directory to the nearest `build.fol` and keeps toolchains next to
   the project's build artifacts, inside the `.fol/` directory every project
   already has.
3. If neither exists (no variable, not inside a project), `fol` stops with an
   error telling you both ways out.

The layout inside the home:

```
$FOL_HOME/
├── toolchains/
│   ├── v0.2.1/            ← one immutable toolchain per version
│   │   ├── folc           ← the engine binary
│   │   ├── std/           ← the standard library, exactly as released
│   │   └── runtime/       ← the runtime crate sources folc compiles against
│   └── dev.toml           ← a *linked* toolchain (see below)
├── pkg/                   ← third-party packages, shared by all versions
└── config                 ← the default toolchain (`default = 0.2.1`)
```

A toolchain directory is **self-contained**: `folc` resolves `std/` and
`runtime/` relative to its own binary, so nothing else on the machine — no
environment variables, no source checkout — is needed for it to compile and
run programs.

## Pinning a version

A project declares which toolchain it wants on the **first comment line of
`build.fol`**:

```fol
//fol 0.2.1

pro[] build(): non = {
    ...
};
```

The manager scans only the leading comment block (it never needs to *run* the
build program to learn the version — that would be a chicken-and-egg problem),
matching `//fol <version>`. Checking this one line into the repository means
everyone who clones the project builds it with the same toolchain.

## Selection order

For any forwarded command, the toolchain is chosen by the first match:

1. `fol +0.1.4 code build` — explicit one-shot override, `+<version|name>`
2. `FOL_TOOLCHAIN=dev` — environment override
3. the `//fol <version>` pin in the nearest `build.fol`
4. the default recorded in `$FOL_HOME/config` (`fol self default …`)
5. if exactly one toolchain is installed, it is used; otherwise an error

## Automatic fetching

If a pinned version is not installed, `fol` downloads it on the spot and
continues:

```console
$ fol code run
toolchain 0.2.1 not installed, fetching...
fetched fol 0.2.1 -> ~/.fol/toolchains/v0.2.1
42
```

Releases carry exactly two kinds of assets per target
(FOL is Linux-only — `x86_64-linux` and `aarch64-linux`):

- `fol-compiler-and-lib-v<version>-<target>.tar.gz` — the toolchain:
  `folc` + `std/` + `runtime/`, unpacked verbatim into
  `$FOL_HOME/toolchains/v<version>/`
- `fol-v<version>-<target>` — the `fol` binary itself, ready to drop on `PATH`

## The `fol self` commands

```console
fol self install 0.2.1              # fetch a released toolchain
fol self install 0.2.1 --from .     # copy folc + std + runtime from a built source tree
fol self link dev ~/code/fol        # register a source checkout as toolchain "dev"
fol self default 0.2.1              # set the default toolchain
fol self list                       # show installed toolchains, default marked
fol self remove 0.2.1               # delete a toolchain (or a link)
fol self which                      # print the folc the current directory resolves to
```

See [Installing & Developing](./100_install.md) for the day-to-day flows and
[Versions In Depth](./200_versions.md) for the dispatch mechanics.

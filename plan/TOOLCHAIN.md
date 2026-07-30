# FOL Toolchain Manager — Design & Implementation Plan

Status: design LOCKED with owner (2026-07-21). IMPLEMENTED same day:
`fol-self` crate (manager binary `fol`), root binary renamed `folc`,
binary-relative std in fol-package, end-to-end gates green (sandbox FOL_HOME,
install --from, pin dispatch, toolchain-std marker proof, +dev override).

## Locked decisions

1. **FOL_HOME, with a project-local fallback.** If `FOL_HOME` is set, it is
   the home. If not, the manager walks up from cwd to the nearest `build.fol`
   and uses `<project>/.fol/toolchain` as the home — the project's `.fol/`
   directory already holds builds and artifacts, so toolchains land beside
   them (owner request 2026-07-21). Only when neither exists (no env, no
   project) does the manager error with a setup hint.
2. **Two binaries.**
   - `fol` — the MANAGER. Small, ~zero deps, distro-packaged, lives on PATH.
     Owns `fol self …` and version dispatch. Contains NO language logic.
   - `folc` — the TOOLCHAIN. Today's fat binary (compiler + code/pack/tool/
     work/test), renamed. Lives inside `$FOL_HOME/toolchains/vX.X.X/`.
   Distros package only `fol`; it fetches/manages everything else.
3. **One toolchain binary, not four.** `code`/`pack`/`tool`/`work` all need the
   full compiler linked in; splitting gives N large identical binaries plus
   rustup-style intra-toolchain version skew. The manager execs "the
   toolchain", so the internal layout can change later without any user-facing
   difference.
4. **Version pin = first comment line of `build.fol`.** No separate pin file.
   Format: `//fol 0.2.0` — regex `^//\s*fol\s+(\S+)`. The manager scans the
   leading comment/blank block of `build.fol` (never runs the compiler for
   this). No pin → default toolchain.
5. **Missing pinned toolchain → auto-download** (Go behavior, not rustup):
   `toolchain 0.4.0 not installed, fetching...` then build continues.
6. **`pkg/` is shared across all toolchain versions** (packages are source;
   same model as Cargo registry / Go module cache).
7. **`dev` toolchain is a LINK into the source checkout**, not a copy — solves
   the from-source developer flow and std staleness (std resolves from the
   live repo tree; no fingerprinting needed).

## FOL_HOME layout

```
$FOL_HOME/
├── toolchains/
│   ├── v0.2.0/
│   │   ├── folc            ← real toolchain binary
│   │   └── std/            ← its exact std
│   ├── v0.4.0/…
│   └── dev.toml            ← linked source checkout (manifest, see below)
├── pkg/                    ← third-party packages, shared by all versions
└── config                  ← default toolchain (`default = dev`)
/usr/bin/fol                ← manager (distro or self-managed)
```

Linked-toolchain manifest (`toolchains/<name>.toml`):

```toml
repo = "/home/bresilla/data/code/bresilla/fol"
# optional explicit overrides; otherwise derived from repo at dispatch time:
# bin = "<repo>/target/{release,debug}/folc"   (release preferred, then debug)
# std = "<repo>/lang/library/std"
```

Optional (nice-to-have): manager also scans a read-only system location
(`/usr/lib/fol/toolchains/`) so a distro may ship a full offline toolchain;
`fol self list` shows it as installed + non-removable.

## Dispatch (manager behavior on any non-`self` command)

Resolution order for which toolchain runs:

1. `fol +0.1.4 code build` — explicit override (rustup `+` syntax)
2. `FOL_TOOLCHAIN` env var
3. `//fol X.X.X` pin in `build.fol` (walk up from cwd to find it)
4. `$FOL_HOME/config` default
5. If no default: exactly one installed toolchain → use it; else error with
   `fol self default <version>` hint.

Then:
- Version pin not installed → auto-download into `toolchains/v<ver>/`.
- **std wiring needs NO env from the manager.** Empirically verified:
  `use std: pkg = {"std"}` resolves through the package STORE root, not
  `FOL_STD_ROOT` (setting only FOL_STD_ROOT leaves imports on the compiled-in
  path). And the manager must not set `FOL_PACKAGE_STORE_ROOT` either — env
  outranks a project's local `.fol/pkg` in the chain and would shadow fetched
  packages. Instead, folc itself is toolchain-aware:
  `fol_package::available_bundled_std_root()` first looks for `std/` NEXT TO
  THE RUNNING BINARY (`current_exe` sibling — the installed-toolchain layout),
  then falls back to the compiled-in source-tree path (dev builds, linked
  checkouts). The store root derives from it, so both import paths follow.
- Set `FOL_DISPATCHED=1` recursion guard, then `exec` folc with all remaining
  args verbatim.

Version-correct LSP for free: editors invoke `fol tool lsp` through the shim →
the language server matches the project's pinned compiler.

**All user-facing surface belongs to `fol`** (owner rule). Help/version/no-args
forward to the resolved folc so `fol` always presents the full styled frontend
(work/pack/code/tool + self); the manager renders its own styled replica only
when no toolchain resolves. folc's root help lists the `self` group; invoking
`folc self` directly redirects (branded F1001) to the manager.

## `fol self` subcommands

```
fol self install <ver>              # fetch release tarball → toolchains/v<ver>/
fol self install <ver> --from <dir> # copy folc + lang/library/std from a built
                                    # source tree (local/offline install; also
                                    # how we test end-to-end pre-release-server)
fol self link <name> <repo-root>    # write toolchains/<name>.toml
fol self default <ver|name>         # write $FOL_HOME/config
fol self list                       # installed toolchains, default marked
fol self remove <ver|name>
fol self which                      # resolved folc path for cwd (debugging)
```

Download: shell out to `curl -fL` (fallback `wget`) — keeps the manager
zero-dep. URL template (release infra TBD):
`https://github.com/bresilla/fol/releases/download/v{ver}/fol-v{ver}-{target}.tar.gz`
with target from `uname -m` + OS. Until releases exist, network install fails
with a clear message; `--from` is the working path.

## Implementation map (repo-specific)

1. **Root `Cargo.toml`**: `[[bin]] name = "fol"` → `"folc"` (path unchanged:
   `lang/tooling/fol-frontend/src/main.rs`). Add workspace member
   `lang/tooling/fol-self`.
2. **Test harness rename**: 4 files reference `env!("CARGO_BIN_EXE_fol")` →
   `CARGO_BIN_EXE_folc`:
   - test/run_tests.rs
   - test/apps/test_apps.rs
   - test/integration_tests/integration_v3_runtime_proofs.rs
   - test/integration_tests/integration_editor_and_build.rs
   Also sweep docs/Makefile for `--bin fol` mentions.
3. **New crate `lang/tooling/fol-self`** → binary `fol`. std-only (no external
   deps). Modules: FOL_HOME check, config read/write, pin scan, toolchain
   resolve (dir vs link manifest), install (curl/tar + --from copy), dispatch
   (env + exec).
4. **Unit tests** in fol-self: pin regex (leading comment block scan, `v`
   prefix normalization), resolution precedence, link-manifest parsing.
5. **Integration proof**: build workspace → `fol self install 0.2.0 --from .`
   into a temp FOL_HOME → `fol code build` an example project from a foreign
   cwd, judged by explicit exit code (background-gate discipline).

## Deferred / follow-ups

- Wire package FETCH destination to `$FOL_HOME/pkg` (today fetched packages
  land per-project; store-chain rework is separate from the manager).
- `fol self update` (self-update of the manager) — needs release infra first.
- Release CI producing toolchain tarballs; then flip the download URL live.
- System toolchain dir scan (`/usr/lib/fol/toolchains/`).

# Installing & Developing

## Getting `fol`

`fol` is the only binary you install by hand — everything else it fetches
itself. Take the `fol-v<version>-<target>` asset from a release, drop it on
your `PATH`, and set your home:

```console
$ curl -fL -o ~/.local/bin/fol \
    https://github.com/fol-lang/fol/releases/download/v0.2.1/fol-v0.2.1-x86_64-linux
$ chmod +x ~/.local/bin/fol
$ export FOL_HOME="$HOME/.fol"        # add to your shell profile
```

From that moment a plain `fol code run` inside any pinned project fetches the
right toolchain automatically. To install one ahead of time:

```console
$ fol self install 0.2.1
fetched fol 0.2.1 -> ~/.fol/toolchains/v0.2.1
$ fol self default 0.2.1
default toolchain is now 0.2.1
```

Prefer keeping everything project-local? Skip `FOL_HOME` entirely — inside a
project the manager uses `<project>/.fol/toolchain` as its home, so the
toolchain sits beside the project's build outputs and nothing global exists.

## Working on FOL itself

A source checkout becomes a first-class toolchain through a **link** — no
copying, always fresh:

```console
$ fol self link dev ~/code/fol
linked toolchain 'dev' -> /home/you/code/fol
$ fol self default dev
```

A linked toolchain resolves its `folc` from the checkout's
`target/release/folc` or `target/debug/folc` — whichever was **built most
recently** — and its `std` and `runtime` straight from the live source tree.
Rebuild the compiler, and every `fol` invocation immediately uses it.

To freeze a source build into a normal, immutable toolchain instead (useful
for testing the install flow offline):

```console
$ cargo build --release --bin folc && cargo build --release -p fol-self
$ fol self install 0.2.2 --from .
installed fol 0.2.2 -> ~/.fol/toolchains/v0.2.2 (from /home/you/code/fol)
```

## Releasing

Pushing a `v*` tag runs the `release` workflow, which builds `folc` and `fol`
on every supported Linux target, stages `{folc, std/, runtime/}`, and uploads:

- `fol-compiler-and-lib-v<version>-<target>.tar.gz`
- `fol-v<version>-<target>`

The tarball layout is a contract: `fol self install` unpacks it verbatim into
`$FOL_HOME/toolchains/v<version>/`, and an integration test pins the workflow
to that layout.

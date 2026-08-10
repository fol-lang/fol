# config_workspace

A two-package FOL workspace built around a real tool: **confcheck**, a linter
for `key = value` configuration files.

```
config_workspace/
  fol.work.yaml          workspace root, lists both members
  confkit/               the analysis library
    build.fol            static lib + build module + test bundle
    src/lib.fol          public API: parsing and per-line rules
    src/scan/            byte and line helpers
    src/schema/          known keys, value kinds, required keys
    src/verdict/         Finding and Tally records
    tests/main.fol       23 behaviour checks, run by `fol code test`
  confcheck/             the executable
    build.fol            exe + install + run, depends on confkit
    src/main.fol         drives a run and decides what to print
    src/sample/          the documents being checked
    src/render/          finding and summary formatting
```

## Running it

From `confcheck/`:

```sh
folc --package-store-root <repo>/lang/library code run
```

From `confkit/`:

```sh
folc --package-store-root <repo>/lang/library code test
```

`code check` works from the workspace root and checks both members.

## What the checker does

`confkit` measures a document against a small schema: five known keys, each
with a value kind (`text`, `integer`, `boolean`), two of them required. It
reports six rules:

| Code | Meaning |
|------|---------|
| C001 | line is not `key = value` |
| C002 | assignment with no key |
| C003 | unknown setting |
| C004 | setting redefined |
| C005 | value does not match the key's kind |
| C006 | required setting missing |

`confcheck` runs both sample documents — one clean, one that trips every rule —
prints each finding with its line number, and summarises per document.

## Notes on the language surface

Three things shaped how this code is written, and are worth knowing before
editing it:

- **`and` / `or`, not `&&` / `||`.** `&&` does not parse at all, and `||` is
  the recoverable-fallback operator, not boolean or.
- **Neither `and` nor `or` short-circuits.** Both sides always evaluate, so a
  guard like `i < .len(xs) and xs[i] == k` faults instead of guarding. Every
  compound condition here keeps both sides total.
- **Import aliases are package-global.** Two files in one package cannot both
  declare `use std`, and an alias declared in any file is visible in all of
  them. Each package here declares its aliases once, in its root file.
- **No comments inside `build()`.** A `//` in the build routine body fails
  build evaluation, so every build.fol comment sits at file root.

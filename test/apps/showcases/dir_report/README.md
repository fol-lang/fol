# dir-report

Walks a directory tree, classifies every entry it finds, and prints a listing
followed by totals.

```text
dir-report [root] [max-depth] [show-depth]
```

- `root` — where to start (default `.`)
- `max-depth` — how deep the recursion goes (default `3`)
- `show-depth` — how much of the tree is echoed before the totals (default `1`)

Exit status is `1` when the walk found no files at all, so a caller scripting
it can tell an empty result from a populated one.

## Layout

| path | role |
| --- | --- |
| `src/text` | line cursor and padding over the newline-joined `dir_list` output |
| `src/kinds` | entry classification by trailing slash and extension |
| `src/counts` | the `Tally` record and its merge |
| `src/walk` | the recursive traversal |
| `src/render` | the report rows |
| `test/app.fol` | the suite, run with `fol code test` |
| `assets/tree` | fixture tree the suite walks; its byte total is asserted |

## Running

```bash
fol code run -- /some/directory 3 2
fol code test
```

## Notes on the surface

Two things in this package are shaped by current compiler behaviour rather
than by preference:

- `kinds` exposes each `Kind` member through a routine (`kind_dir()` and
  friends). An `ent` member only typechecks where an `int` is already
  expected, so it cannot appear directly in a comparison.
- `text::eq` exists because a one-character double-quoted literal is inferred
  as `chr`, and `str == chr` does not typecheck. Passing both sides through
  parameters declared `str` is what makes the comparison compile.

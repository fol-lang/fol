# cli_taskbook

A two-package FOL workspace holding a real command-line tool: **taskbook**, a
task tracker whose store is a plain text file.

```
cli_taskbook/
  fol.work.yaml            workspace root, lists both members
  taskkit/                 the workflow library
    build.fol              static lib + build module, both exported by name
    src/lib.fol            public API: add / move_to / drop / set_priority / view
    src/text/              byte-level scanning: trim, split, token membership
    src/entry/             one task, and the line that carries it
    src/book/              the whole document: ids, mutations, views, survey
  taskbook/                the executable
    build.fol              exe + install + run, depends on taskkit
    src/main.fol           argv dispatch, file I/O, exit statuses
    src/args/              the command line
    src/view/              the table and summary a person reads
    src/checks/            taskkit's behaviour suite, run by `taskbook check`
```

## Running it

```sh
folc code check test/apps/showcases/cli_taskbook
folc code run   test/apps/showcases/cli_taskbook            # suite, then a demo session
folc code run   test/apps/showcases/cli_taskbook -- --help
folc code run   test/apps/showcases/cli_taskbook -- --file /tmp/t.tasks add "buy milk" --tag home
```

A bare run does two things: it runs the 79 pure behaviour checks, then a
scripted session that writes a store to `$TMPDIR`, reads it back, and checks
every answer against what the workflow is supposed to produce. Both must pass
for the process to exit 0.

## The tool

```
usage: taskbook [--file <store>] <command>

  add <title> [--tag a,b] [--prio 1..5]    add a task
  list [--state todo|doing|done] [--tag t] show tasks, highest priority first
  start <id>                               todo -> doing
  finish <id>                              todo|doing -> done
  reopen <id>                              done -> todo
  prio <id> <1..5>                         change a task's priority
  remove <id>                              delete a task
  stats                                    counts per state
  check                                    run the taskkit behaviour suite
  demo                                     suite, then a scripted session
```

The store defaults to `taskbook.tasks`, overridden by `--file` or by
`$TASKBOOK_FILE`.

Statuses are the interface, so scripts can branch on them:

| status | meaning |
|--------|---------|
| 0 | the command did what it said |
| 1 | legal command, matched nothing (`list` with no hits) |
| 2 | the command line was wrong |
| 3 | the store could not be read or written |
| 4 | no task carries that id |
| 5 | the workflow forbids that transition |
| 6 | a value was refused (empty title, priority out of range, …) |

## The store format

```
#next 4
# groceries for the weekend
1|0|3|home,errand|buy milk
2|1|5|work,urgent|ship release
```

Fields are `id|state|priority|tags|title`. The title is the remainder after the
fourth separator, so it may contain `|`; nothing else may, and a newline
anywhere in a task is refused before it can reach the file.

Three properties are worth knowing, and all three are checked:

- **Ids are never recycled.** `#next` is a high-water mark that survives
  deletion. Deriving the next id from the live rows would hand a deleted task's
  id to a new one, and `taskbook done 7` recalled from a shell history would
  quietly finish the wrong work.
- **Unknown lines are preserved.** `#` lines and rows this build cannot decode
  are copied verbatim through every mutation. Only the *views* (`list`) leave
  them out, and `stats` counts the unreadable ones.
- **Tags match by token.** `--tag home` does not match a task tagged
  `homework`.

## Notes on the language surface

Five things shaped how this code is written.

- **The text is the data structure.** `vec[...]` has no append, no index
  assignment (`values[0] = 9` is rejected) and no concatenation, so a growable
  in-memory table of tasks cannot be built. Every mutation here returns a new
  document string, and the store is re-walked on demand.
- **One entry routine per workspace.** `fol code run <path>` compiles every
  `.fol` file under the path, including files no artifact references. A second
  `fun[] main` anywhere — a `graph.add_test` bundle, for instance — makes the
  path form fail with F1004 even though `cd <path> && fol code run` succeeds.
  That is why the behaviour suite lives in `taskbook/src/checks` and is invoked
  by a subcommand rather than by `fol code test`.
- **`pkg` cannot reach a `loc` dependency.** taskbook declares taskkit properly
  with `build.add_dep({ source = "loc", ... })`, and the book says a directory
  with a `build.fol` should be imported through `pkg` — but `pkg` only ever
  searches the package store, so `use kit: loc = {"../../taskkit/src"}` is the
  only spelling that links.
- **`report` and `at` are taken.** `at` is rejected as a routine name at the
  declaration; `report` is accepted there and only fails at the call site, with
  a message about routine error types.
- **No comments inside `build()`.** A `//` in the build routine body fails
  build evaluation, so every build.fol comment sits at file root.

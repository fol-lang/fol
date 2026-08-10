# traffic_fsm

A traffic-light controller written as an explicit state machine: an entry type
for the phases, an entry type for the inputs, a transition table that is
readable on its own, and a driver that applies one event at a time.

```
folc --package-store-root <repo>/lang/library code run
folc --package-store-root <repo>/lang/library code test
```

## Layout

| path | role |
| --- | --- |
| `src/states/lib.fol` | `Phase` / `Event` entry types and the `Signal` record |
| `src/table/lib.fol` | hold times, accepted events, and the transition table |
| `src/driver/lib.fol` | applies one event and returns the next `Signal` |
| `src/report/lib.fol` | per-phase occupancy tally and its histogram |
| `src/main.fol` | runs a scripted morning and prints the trace |
| `common/lib.fol` | text helpers, reached through a `loc` import |
| `test/main.fol` | transition-table and fault-path checks |

## Notes on the surface it uses

- phase and event codes are declared with `ent`, but every binding is typed
  `int`. An `ent` member only typechecks where a type is already expected, so
  `Phase.RED` works as a `var` initializer and as a call argument, but not as
  an `is` pattern, not as a `when` branch result, and not as an operand of
  `==`. The transition table therefore binds an `int` local first.
- the transition tables use the arrow form of `when` (`is 0 -> 3;`), the
  driver uses the statement form.
- containers are literal-only, so the occupancy histogram is a record with one
  counter per phase rather than a `map`.

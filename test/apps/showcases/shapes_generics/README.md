# shapes_generics

`cutsheet` -- a small sheet-metal planner, written to exercise FOL's generic
surface end to end.

Three drawn parts (a lid, a gasket, a bracket) are measured through one
constrained generic routine, re-measured as they will actually be produced,
totalled, and turned into a stock order.

```
folc code run              # print the cut sheet
folc code run -- check     # run the self-checks
```

## Modules

| module              | what it holds                                                        |
| ------------------- | -------------------------------------------------------------------- |
| `src/shape/`        | the `geometry` protocol standard, `Rect`/`Circle`/`Tri` conformers, a custom `clone` override, a Newton square root, and constrained generic routines |
| `src/transform/`    | the generic wrappers `Scaled(T)` and `Kerfed(T)`, higher-order routines over routine values, and closure factories that return and compose transforms |
| `src/collection/`   | the generic containers `Pair(L, R)` and `Trio(T)` with generic receiver routines, a generic fold, and a `Tally` accumulator with a mutating `pro (T[mut, bor])` receiver |
| `src/checks/`       | thirteen self-checks against values worked out by hand                 |

## Generic surface exercised

- generic types with one and two type parameters (`Trio(T)`, `Pair(L, R)`)
- generic receiver routines, including one returning a different instantiation
  of its own container (`Pair[L, R].flip() -> Pair[R, L]`)
- a protocol standard used as a generic constraint, with constraint calls
  dispatched to each conformer at monomorphisation
- generic routines taking routine values, with the result type inferred from
  the routine value at the call site (`fold_pair(L, R, C)`, `map_trio(T, U)`)
- routines that return routine values, and a `chain` that composes two of them
  through move captures
- a custom `clone` override reached through `[cln]`

## Shapes the language forced

Three things in this package are written the way they are because the
straightforward version does not work today:

- **`geometry` requires exactly one routine.** A constraint call consumes the
  generic value it dispatches on, so a second call on the same value is
  rejected. The standard therefore returns a `Metrics` record holding every
  answer at once instead of separate `area`/`perim`/`label` routines.
- **The generic containers are fixed-arity.** Reading an element out of a
  `vec[T]` at a generic element type is rejected, and so is `.len` on a
  `vec[T]` reached through a record field, so the containers hold named slots
  and aggregation happens at the concrete `Metrics` element type.
- **The self-checks live behind a `check` argument, not in `graph.add_test`.**
  A second artifact root that imports a namespace declaring a standard fails to
  build with `T1005 type import failed`.

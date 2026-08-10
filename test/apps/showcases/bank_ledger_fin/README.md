# bank_ledger_fin

A double-entry settlement engine whose accounts compete for a scarce resource.

A batch of postings may only be written while the account it hits holds one of
the journal device's four write slots. A slot is a `fin` type, so the compiler
decides who releases it and guarantees it is released exactly once. Every
acquisition prints an `[acq]` line and every release prints a `[fin]` line from
the finalizer and nowhere else, which makes the transcript an audit: the two
counts must match, and this run produces thirteen of each.

The accounting is real. Money is whole cents in `int`, a batch must sum to zero
across its legs, every code must exist in the chart, and the trial balance is
re-derived after posting. Batches that fail those rules are rejected before a
single slot leaves the device.

## What the run demonstrates

| Surface                | Where                                                     |
| ---------------------- | --------------------------------------------------------- |
| scope-exit finalize    | `engine::settle` acquires each account's slot in its own block |
| moved-in finalize      | `engine::file_leg` hands a `Ticket` to `slot::archive`, which owns it |
| finalize through a field | `slot::Ticket` holds the `Slot` the archive releases    |
| container finalize     | `slot::reserve_device` returns `vec[Slot]`                 |
| explicit `[fin]value`  | `engine::write_correction` hands the slot back the moment the write attempt ends |
| `dfr`                  | the journal unlock that runs on every exit                 |
| `edf`                  | the rollback that runs only when a batch is reported       |
| `dfr[x[mut, bor]]`     | `scoped_cleanups` counts its own deferred bodies so the checks can assert on them |

## Layout

| Path                 | What lives there                                          |
| -------------------- | --------------------------------------------------------- |
| `src/model/lib.fol`  | `Side` entry, `Posting`/`Account`/`Receipt`, leg validation |
| `src/slot/lib.fol`   | the `fin` write slot, its finalizer, tickets, device reservation |
| `src/engine/lib.fol` | chart of accounts, batches, settlement, corrections, balances |
| `src/render/lib.fol` | cents-to-money, column padding, the self-check verdict lines |
| `src/main.fol`       | the driver, the printed trace, and the self-checks          |

## Running

```
fol code run
```

## Exit status

`0` when every self-check passes, `1` otherwise. The count of failing checks is
printed on the last line.

## A note on the `[fin]` placement in `write_correction`

The slot is released before either exit is taken, rather than inside the branch
that reports. That is deliberate: the current compiler finalizes twice when
`[fin]` sits inside a branch that then leaves the routine, and not at all on
paths that never reach a top-level `[fin]`. Both are reproduced under
`/tmp/claude-1000/round3-repros/bank_ledger_fin/`.

# ledger-records

A double-entry ledger auditor written in FOL.

It carries a chart of accounts and a journal for a one-person consultancy's
first month of trading, then prints the journal, the trial balance, the budget
overruns, and a balanced/out-of-balance verdict. Money is whole cents in `int`
throughout, so no total is ever a rounded float.

Pass one extra posting on the command line as `day|code|dr|amount|memo` and it
is parsed, validated, checked against the chart of accounts, and either
accepted or rejected with the reason.

## Layout

| Path                    | What lives there                                        |
| ----------------------- | ------------------------------------------------------- |
| `src/model/lib.fol`     | `Side` entry, `Posting`/`Account` records, validation   |
| `src/store/lib.fol`     | chart of accounts, journal, balances, budgets           |
| `src/reporting/lib.fol` | money formatting, column padding, the rendered report   |
| `src/main.fol`          | the driver and the command-line posting review          |
| `tests/main.fol`        | the test bundle, importing `../src` as a local package  |

## Running

```
fol code run
fol code run -- '11|5200|dr|24750|second desk'
fol code test
```

## Exit status

`0` when the books balance, `1` when they do not, `2` when a posting in the
journal is structurally broken. The test bundle exits with the number of
failed checks.

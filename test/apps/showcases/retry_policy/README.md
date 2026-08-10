# retry_policy

A retry/backoff policy engine over fallible operations.

Six simulated operations are driven through three policies. Every number the
program prints is a consequence of the backoff curve and the fault classifier,
and the self-checks at the bottom assert those numbers against values computed
by hand.

```
fol code run test/apps/showcases/retry_policy
```

## Modules

| module        | what lives there                                                   |
|---------------|--------------------------------------------------------------------|
| `src/faults`  | the fault taxonomy, the retryable/fatal split, peer cool-down hints |
| `src/policy`  | the backoff curve, jitter, budget rules, policy validation          |
| `src/ops`     | the fallible operations and the three layers they fail through      |
| `src/engine`  | the retry driver, its named exits, and fallback chains              |
| `src/report`  | rendering                                                           |

## The operations

| operation       | behaviour                                                    |
|-----------------|--------------------------------------------------------------|
| `cold-cache`    | answers on the first attempt                                 |
| `flaky-index`   | times out twice, then succeeds                               |
| `throttled-api` | throttled once, and the peer dictates the next delay         |
| `dead-replica`  | refuses the connection forever                               |
| `sealed-vault`  | rejects the credentials; never worth a retry                 |
| `garbled-feed`  | the transport succeeds and the body fails its checksum       |

`flaky-index` and `dead-replica` are the two shapes that justify a retry engine:
one recovers if you wait, the other never does and has to be abandoned.

## The exits

A run records *why* it stopped, not just that it did:

- `succeeded`
- `out of attempts` — the retry count ran out
- `out of time budget` — the next delay would not fit
- `fault is not retryable` — a fatal fault, refused on the first attempt
- `policy rejected` — the policy itself failed validation before any call

## Error surfaces

- `int / str` for the effect layers, handled at the call site with `||`.
- `|| report` to forward a failure outward, `|| 0` to supply a value, and
  `|| panic` to assert a case the catalogue makes impossible.
- `err[str]` wherever a reason has to be stored, `opt[int]` wherever an answer
  may be absent, and `when ... on ... *` for every one of them.

Nothing sleeps: `waited` is the simulated cost of the delays the policy asked
for, which is what makes the schedule assertable.

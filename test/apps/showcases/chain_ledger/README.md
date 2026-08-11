# chain_ledger

naivechain (https://github.com/lhartikk/naivechain, MIT) translated into FOL.

The original is a ~200-line JavaScript blockchain: a block of
`{index, previousHash, timestamp, data, hash}`, a SHA-256 `calculateHash`,
`generateNextBlock`, a three-check `isValidNewBlock`, `isValidChain`,
longest-chain `replaceChain`, an Express HTTP API, and a WebSocket peer
protocol with three message kinds.

```
folc code check test/apps/showcases/chain_ledger
folc code run   test/apps/showcases/chain_ledger
```

## Layout

| path | what it holds |
| --- | --- |
| `src/hash/lib.fol` | the rolling digest that stands in for SHA-256 |
| `src/block/lib.fol` | the Block record, `calculate_hash`, the genesis block |
| `src/chain/lib.fol` | storage, `generate_next_block`, validation, `replace_chain` |
| `src/node/lib.fol` | the three message kinds, the wire codec, `handleBlockchainResponse` |
| `src/main.fol` | the demo and the self-checks, plus the full DEVIATIONS header |

## What it demonstrates

Mines a five-block chain and validates it, rewrites a middle block's data and
shows validation failing at exactly that index with the original's
`invalid hash: X Y` message, then runs the longest-chain rule: a longer valid
chain wins, a shorter one does not, and a longer invalid one is rejected. The
peer half then encodes a chain to the wire, decodes it, appends a peer's tip,
answers `QUERY_ALL` for a gapped tip, replaces on a longer chain, ignores a
shorter one, and sorts an out-of-order message back into index order.

## The one thing to know

**This chain is not cryptographic.** FOL has no bitwise operators, so SHA-256
cannot be written at all. `src/hash` uses `h = (h * 31 + byte) % 1000000007`
instead. It is deterministic and sensitive to every input field, which is all
the validation logic needs, but collisions are trivial to construct. The full
list of departures from the original is in the header of `src/main.fol`.

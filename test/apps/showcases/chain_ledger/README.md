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

**The hash is a real SHA-256.** `std::hash::sha256_hex` is built from FOL's
bitwise intrinsics over 32-bit words, so this port hashes what CryptoJS.SHA256
hashes. The genesis block proves it: naivechain hardcodes
`816534932c2b7154836da6afc367695e6337db8a921823784c14378abed4f7d7` and never
re-derives it, and `calculate_hash` here reproduces that literal exactly. The
full list of departures from the original is in the header of `src/main.fol`.

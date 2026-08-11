# json_tokens

[jsmn](https://github.com/zserge/jsmn) — Serge Zaitsev's zero-allocation JSON
tokenizer, ~500 lines of C — translated to FOL, in the `JSMN_STRICT` +
`JSMN_PARENT_LINKS` configuration.

A single left-to-right pass fills a flat array of `{type, start, end, size,
parent}` tokens. Nothing is copied out of the input: a token is a pair of
offsets into the caller's buffer. `toksuper` names the container a new token
belongs to, each token links up to its parent, and a closing bracket walks that
chain to find the innermost container still open.

```
src/token    the type tags (1/2/4/8) and error codes (-1/-2/-3), and jsmntok_t
src/bytes    the character classes jsmn spells as switch labels
src/parser   jsmn_init, jsmn_alloc_token, jsmn_fill_token, jsmn_parse_primitive,
             jsmn_parse_string, jsmn_parse
src/suite    41 fixtures with the answers the real jsmn.h printed
src/main     the report, the self-checks, and the DEVIATIONS notes
```

`src/main.fol` opens with the full list of places jsmn's design could not be
carried into FOL. The short version: the token array had to move inside the
parser record, because only a receiver may take `[mut, bor]`; the array is
fixed at 32 slots, because `arr[T, N]` carries N in its type; and
`jsmn_alloc_token`'s returned pointer became an index, because every field poke
has to rebuild the whole element.

Every expected value in `src/suite` was produced by compiling the real
`jsmn.h` against a printing driver, so the self-checks compare against C, not
against this port. All 41 agree.

```
folc code check test/apps/showcases/json_tokens
folc code run   test/apps/showcases/json_tokens
```

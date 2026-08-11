# Numbers

## `std::nums` — arithmetic helpers

```fol
fun[exp] clamp_int(value: int, low: int, high: int): int
fun[exp] clamp_flt(value: flt, low: flt, high: flt): flt
fun[exp] sign_int(value: int): int
fun[exp] is_even(value: int): bol
fun[exp] is_odd(value: int): bol
fun[exp] gcd(left: int, right: int): int
fun[exp] lcm(left: int, right: int): int
fun[exp] pow_int(base: int, exponent: int): int
fun[exp] degrees_to_radians(value: flt): flt
fun[exp] radians_to_degrees(value: flt): flt
```

`sign_int` yields `-1`, `0`, or `1`. `gcd` takes absolute values, so it is safe
on negatives, and `lcm` is built from it.

These live in FOL because none of them needs the compiler: they are a few lines
each over the operators. The transcendental functions — `.sqrt(...)`,
`.sin(...)`, `.ln(...)` and the rest — are intrinsics instead, because
reimplementing them in FOL would be slower and less accurate than the ones the
platform already has.

### Overflow that reports rather than faults

Plain `+`, `-`, and `*` **fault** on overflow rather than wrapping. When you
want the other behaviour, the `.checked_*`, `.wrapping_*`, and `.saturating_*`
intrinsics give it directly. What they cannot give is the value *and* a flag,
because an intrinsic returns one value — so that pair lives here:

```fol
typ[exp] Overflowed: rec = {
    value: int,
    overflowed: bol,
};

fun[exp] overflowing_add(left: int, right: int): Overflowed
fun[exp] overflowing_sub(left: int, right: int): Overflowed
```

The record is the pair. FOL has no tuple type, so this is how any routine
returns two things.

## `std::rand` — seeded random numbers

The `.random_int(...)`, `.random_flt(...)`, and `.random_bytes(...)` intrinsics
take entropy from the operating system: unpredictable, and **unrepeatable**.
That second property is usually the problem — a test or a simulation needs the
same answer twice. This module is the other tool.

```fol
typ[exp] Rng: rec = { state: int };
typ[exp] Draw: rec = { rng: Rng, value: int };
typ[exp] DrawVec: rec = { rng: Rng, values: vec[int] };

fun[exp] seeded(seed: int): Rng
fun[exp] from_os(): Rng
fun[exp] next(rng: Rng): Draw
fun[exp] next_range(rng: Rng, low: int, high: int): Draw
fun[exp] next_bool(rng: Rng): Draw
fun[exp] shuffle_int(rng: Rng, values: vec[int]): DrawVec
fun[exp] choice_int(rng: Rng, values: vec[int], fallback: int): Draw
```

Because the routines are pure, each returns the advanced generator alongside
the value, and the caller threads it along. That is what `Draw` is for:

```fol
var[mut] rng: std::rand::Rng = std::rand::seeded(2024);
var roll: std::rand::Draw = std::rand::next_range(rng, 1, 7);
rng = roll.rng;
// roll.value is the die
```

Forgetting to carry `roll.rng` forward is the one easy mistake: the next call
then starts from the same state and returns the same number.

`next_range` is half-open — `next_range(rng, 1, 7)` is a six-sided die — and
uses rejection rather than a modulo, so no value is favoured when the span does
not divide evenly. `choice_int` needs a fallback because an empty vector has
nothing to choose.

Seed from `from_os()` when you want a different stream per run but still want to
log the seed and reproduce a failure.

### What the generator is

xorshift64\*: Vigna's 12/25/27 shift triple over a full 64-bit state, with the
output passed through a multiplicative scrambler.

The scrambler matters more than it looks. A plain xorshift is *linear over
GF(2)*, so its outputs are far more predictable than a frequency histogram
suggests. Taking the rank of a 96×96 binary matrix built from its output shows
it immediately:

```text
plain xorshift, no scrambler:  rank 46 of 96   (on every seed)
xorshift64* as shipped:        rank 95 of 96   (on every seed)
```

95 is the expected rank for a genuinely random binary matrix. A test that only
reduces values with `%` — die rolls, buckets, coin flips — cannot see the
difference at all.

It is **not cryptography**. The state is recoverable from the output, which is
exactly what makes it repeatable. Use `.random_bytes(...)` when unpredictability
matters.

Any 64-bit seed works, negatives included. Only zero is special: it is a fixed
point that would emit zeros forever, so it is replaced.

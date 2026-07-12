# Ranges

Current boundary:

- range expressions are not part of the current compiler surface; the compiler
  rejects them with a "not yet supported" diagnostic (T1002)
- range-based loop headers and range matching described in other chapters
  inherit this boundary and are not executable today
- everything below describes intended design, not current behavior
- this is later design work, not part of the current V1 compiler surface

There are two range expressions:

- Defined ranges
- Undefined ranges

## Defined ranges

Defined ranges represent a group of values that are generated as a sequence based on some predefined rules. Ranges are represented with two dots  `..` operator.

```
{ 1..8 }                // a range from 1 to 8
{ 1,2,3,4,5,6,7,8 }

{ 8..1 }                // a range from 8 to 1
{ 8,7,6,5,4,3,2,1 }

{ 1..8..2 }             // a range from 1 to 8 jumping by 2
{ 1,3,5,7 }

{ 3..-3 }               // a range from 4 to -4
{ 3,2,1,0,-1,-2,-3 }

{ -3..3 }               // a range from -3 to 3
{ -3,-2,-1,0,1,2,3 }

{ ..5 }                 // a range form 0 to 5 
{ 0,1,2,3,4,5 }

{ ..-5 }                // a range from 0 to -5
{ 0,-1,-2,-3,-4,-5 }

{ 5.. }                 // a range from 5 to 0
{ 5,4,3,2,1,0 }

{ -5.. }                // a range from -5 to 0
{ -5,-4,-3,-2,-1,0 }
```

syntax | meaning
--- | ---
start`..`end | from start to end
`..`end | from zero to end
start`..` | from start to zero

## Undefined ranges

Undefined ranges represent values that have only one side defined at the definition time, and the compiler defines the other side at compile time. They are represented with three dots `...`

```
{ 2... }                // from 2 to infinity
```

syntax | meaning
--- | ---
start`...` | from start to infinite


In most of the cases, they are used for **variadic parameters** passing:
```
fun calc(number: ...int): int = { return number[0] + number[1] + number[2] * number[3]}
```

# unit_registry

A physical-unit converter built on one conformance standard.

Nine scales -- metre, foot, inch, kilogram, pound, minute, kelvin, celsius,
fahrenheit -- conform to `scale::unit`, a standard with six required routines.
Every routine that does real work is generic over that standard and calls
several of its requirements on the same value, so adding a tenth scale means
writing one type plus six short routines and changing nothing else.

## Why the contract is a bijection, not a factor

Temperature scales are affine: 0 degC is 273.15 K, and 0 degF is not 0 degC.
A registry that stored one multiplier per unit would silently produce nonsense
for those. `unit` instead requires `to_base` and `from_base`, the two directions
of a bijection onto the dimension's SI unit. The multiplicative factor and the
offset are then *recovered* from the contract rather than stored:

    factor = to_base(1) - to_base(0)
    offset = to_base(0)

That is how `bridge::is_affine` works, and how `bridge::crossing` finds the one
value where two scales read the same number (-40, for celsius/fahrenheit)
without any per-unit table.

## Layout

    src/dim/lib.fol      dimension and fault codes (`ent`), and their names
    src/scale/lib.fol    the `unit` standard and all nine conformers
    src/bridge/lib.fol   generic conversion, guards, ordering, composition
    src/sheet/lib.fol    rendering, including a generic conversion ladder
    src/checks/lib.fol   self-checks with hand-computed expected values
    src/main.fol         prints the registry and the tables, then self-checks

## Guards

A conversion is refused, not approximated, when the two units belong to
different dimensions, and a thermal result below absolute zero is reported as
such. Both refusals come out of the same generic `bridge::convert` as the
successes, carrying a `dim::Fault` code so the reason prints next to its row.

## Run

    folc code run test/apps/showcases/unit_registry

The last section runs 20 self-checks against values computed by hand from the
defining constants (the international foot is exactly 0.3048 m, the pound
exactly 0.45359237 kg), so a regression in constraint dispatch or in the affine
arithmetic shows up as a `FAIL` line.

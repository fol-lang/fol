# Numbers

A number in the current front-end is either an integer or a floating-point literal.
Imaginary suffix forms are intentionally outside the hardened lexer/parser contract for
this phase.

## Intigers
An integer has one of four forms:

- A decimal literal starts with a decimal digit and continues with decimal digits and
  optional separating underscores.
- A hex literal starts with `0x` or `0X` and then uses hex digits with optional
  separating underscores.
- An octal literal starts with `0o` or `0O` and then uses octal digits with optional
  separating underscores.
- A binary literal starts with `0b` or `0B` and then uses binary digits with optional
  separating underscores.

```
var decimal: int = 45;
var hexadec: int = 0x6AF53BD5;
var octal: int = 0o722371;
var binary: int = 0b010010010;
```
### Underscore

Underscore character `U+005F` (`_`) is a special character, that does not represent anything withing the number laterals. An integer lateral containing this character is the same as the one without. It is used only as a syntastc sugar:

```
var aNumber: int = 540_467;
var bNumber: int = 540467;

assert(aNumber, bNumber)
```

## Floating points

A floating-point has one of two forms:

- A decimal literal followed by a period character `U+002E` (`.`). This is optionally followed by another decimal literal.
- A decimal literal that follows a period character `U+002E` (`.`).
- A decimal literal followed by a period with no fractional digits is also accepted by
  the current front-end.

```
var aFloat: flt = 3.4;
var bFloat: flt = .4;
var cFloat: flt = 1.;
```

## Current front-end note

Imaginary literals such as `5i` remain a language-design topic in the book, but they
are not tokenized or lowered by the current hardened stream/lexer/parser pipeline.

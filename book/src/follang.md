# FOL Language Specification

FOL is intended as a general-purpose and systems-oriented programming language with a compact declaration syntax, a rich type system, and a strong preference for explicit structure.

This book is the specification draft for the language.

## Core Shape

Much of the surface syntax follows the same high-level shape:

```fol
declaration[options] name: type = { body }
```

That pattern appears across:
- bindings
- routines
- type declarations
- module-like declarations
- standards

## Main Declaration Families

The main declaration families are:

```fol
use    // import declarations
def    // named definitions such as modules, blocks, and tests
seg    // segment/module-like declarations

var    // bindings; use var[imu] for immutability
con    // constants
lab    // labels and label-like bindings

fun    // functions
pro    // procedures
log    // logical routines

typ    // named types
ali    // aliases
std    // standards: protocol, blueprint, extension
```

## Expression And Control Surface

FOL combines block-oriented control flow with expression forms:

```fol
if (condition) { ... }
when (value) { ... }
while (condition) { ... }
loop (condition) { ... }
for (binder in iterable) { ... }
each (binder in iterable) { ... }
```

Expressions include:
- literals
- calls and method calls
- ranges and container literals
- access forms
- pipes
- rolling expressions
- anonymous routines and lambdas

## How To Use This Book

The book is organized from language foundation to higher-level facilities:

1. lexical structure
2. statements and expressions
3. metaprogramming
4. types
5. declarations and items
6. modules and source layout
7. errors
8. sugar and convenience forms
9. conversions
10. memory model
11. concurrency

Read [Notation And Conventions](./000_overview/100_conventions.md) before using chapter examples as a normative reference.

For the current user-facing tool workflow, read
[Frontend Workflow](./000_overview/200_frontend.md).

## Example

```fol
use log: pkg = {"std/fmt/log"};

var[hid] prefix: str = "arith";

fun[] main(): int = {
    .echo(prefix);
    .echo(add(3, 5));
    return 0;
}

fun[exp] add(a, b: int): int = {
    return a + b;
}
```

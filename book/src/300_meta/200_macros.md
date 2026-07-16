# Macros

Current boundary:

- the `def`-based meta family (macros, alternatives, defaults, templates) is
  planned for a future release; none of it is part of the current compiler
  surface
- everything below describes intended design, not current behavior

Macros are intended to support source-level replacement. They will not be able
to redefine the compiler-owned memory sigils `@`, `#`, `!`, `&`, or `*`; those
spellings already have fixed ownership, borrowing, and pointer semantics.

```fol
def '$'(a: any): mac = '.to_string'
```

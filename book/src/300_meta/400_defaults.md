# Defaults

Current boundary:

- the `def`-based meta family (macros, alternatives, defaults, templates) is
  planned for a future release; none of it is part of the current compiler
  surface
- everything below describes intended design, not current behavior

Defaults are intended to change option defaults within an already legal
capability tier. They cannot turn a heap-backed type into a `core` type or grant
hosted APIs. In particular, `str` remains a `memo` type regardless of a future
mutability/default declaration. A possible future spelling for changing its
option defaults is:
```
def 'str': def[] = 'str[new,mut,nor]'
```

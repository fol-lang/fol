# Alternatives

Current boundary:

- the `def`-based meta family (macros, alternatives, defaults, templates) is
  planned for a future release; none of it is part of the current compiler
  surface
- everything below describes intended design, not current behavior

Alternatives are used when we want to simplify code. For example, define an alternative, so whenever you write `+var` it is the same as `var[+]`.
```
def '+var': alt = 'var[+]'
def '~var': alt = 'var[~]'
def '.pointer_content': alt = '.pointer_value'
```

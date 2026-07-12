# Tests

Current boundary:

- `def ... : tst[...]` test blocks are not implemented in the current compiler
  surface
- everything below describes intended design, not current behavior

Blocks defined with type `tst`, have access to the module (or namespace) defined in `tst["name", access]`.

```
def test1: tst["sometest", shko] = {}
def "some unit testing": tst[shko] = {}
```


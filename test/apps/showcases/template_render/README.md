# template-render

Substitutes `{{name}}` placeholders in a text template from a `key = value`
file and writes the result out.

```text
template-render [template] [vars] [output]
```

With no arguments it renders `assets/greeting.tmpl` against
`assets/greeting.vars` into `out/greeting.txt`, so `fol code run` inside the
package does something useful on its own.

Exit status:

| code | meaning |
| --- | --- |
| `0` | every placeholder resolved |
| `1` | rendered, but some placeholder had no binding |
| `2` | the template was empty or unreadable |
| `3` | the output could not be written |

## Template rules

- `{{name}}` is replaced by the value bound to `name`; surrounding spaces in
  the braces are ignored, so `{{ name }}` is the same placeholder
- an opening `{{` with no `}}` on the same line is ordinary text, and does not
  swallow the placeholder that follows it
- an unbound name renders as `<missing:name>` and is also reported on stderr

## Vars file

One `key = value` per line. Blank lines and lines starting with `#` are
ignored, whitespace around the key and value is trimmed, and the last binding
for a key wins.

## Layout

| path | role |
| --- | --- |
| `src/text` | scanning primitives: slice, trim, offset search, line cursor |
| `src/vars` | the variable set: file bindings plus a built-in fallback `map` |
| `src/tmpl` | the placeholder scanner and the `Output` record |
| `test/app.fol` | the suite, run with `fol code test` |

## Notes on the surface

The file bindings are kept as raw text and scanned on lookup rather than
loaded into a `map`: a `map` can only be built from a literal whose entry
count is fixed at compile time, and there is no runtime insert. The fallback
table, whose keys *are* known at compile time, is a real `map[str, str]`.

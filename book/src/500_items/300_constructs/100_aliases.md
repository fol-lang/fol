# Aliases

An alias declaration gives a new name to an existing type surface. The alias
does not create a new object model. It gives the type a stable name that can
be reused in declarations, signatures, and receiver-qualified routines.

There are two related forms:

- aliasing
- extending

## Aliasing

```fol
typ[ali] I5: arr[int, 5];
```

Now code can use `I5` instead of repeating `arr[int, 5]`:

```fol
var fiveIntegers: I5 = { 0, 1, 2, 3, 4 };
```

Another example is naming a constrained color component type:

```fol
typ[ali] rgb: int[8][.range(255)];
typ[ali] rgbSet: set[rgb, rgb, rgb];
```

Aliases are useful because:

- they avoid repeating long type expressions
- they give a type surface a meaningful name
- they provide a named receiver surface for receiver-qualified routines

Receiver-qualified routines remain procedural. If a value is written as
`value.method(arg)`, read that as call-site sugar for `method(value, arg)`.
The routine is still separate from the data declaration.

### Standalone alias declarations

The standalone `ali` declaration is the short form of the same aliasing
surface:

```fol
ali Meters: int;
```

Like other declarations, an alias is package-visible by default. Exporting it
across package boundaries uses the same option surface as `typ`, `fun`, and
`var`:

```fol
ali[exp] Meters: int;
```

`exp` is the only option an alias declaration accepts.

Current milestone note:

- aliasing and extension over current `V1` built-in and declared types are part
  of the present language surface
- true foreign-type interop remains later work
- C ABI and Rust interop belong to the planned `V4` milestone, not the current
  compiler contract

## Extending

Extensions expose an existing type under an explicit receiver surface so that
new receiver-qualified routines can be declared for it.

```fol
typ[ext] type: type;
```

This is still procedural. It does not turn the type into an object or create
inheritance. It only allows routines to be written against that receiver
surface.

For example, an explicit receiver surface for `int`:

```fol
typ[ext] int: int;

pro (int)print(): non = {
    .echo(self)
}

pro main(): non = {
    5.print()
}
```

The call `5.print()` is still just receiver sugar for `print(5)`.

Another example turns a string into a vector of characters through a
receiver-qualified routine:

```fol
typ[ext] str: str;

fun (str)to_array(): vec[chr] = {
    loop(x in self) {
        yield x;
    }
}

pro main(): non = {
    var characters: vec[chr] = "a random str".to_array();
    .echo(characters)
}
```

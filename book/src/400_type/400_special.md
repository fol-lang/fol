# Special

Current boundary:

- the `opt[...]` optional shell and the `err[...]` error shell are the current
  compiler surface
- `uni[...]` (union), `any`, and `nev` (never) are not implemented; the compiler
  rejects them with a "not yet supported" diagnostic
- those sections describe later design work, not current behavior

## Optional
Either are empty or have a value
```
opt[]
```

## Never
```
nev[]
```
The never type is a type with no values, representing the result of computations that never complete. 


## Union
Union is a data type that allows different data types to be stored in the same memory locations. Union provides an efficient way of reusing the memory location, as only one of its members can be accessed at a time. It uses a single memory location to hold more than one variables. However, only one of its members can be accessed at a time and all other members will contain garbage values. The memory required to store a union variable is the memory required for the largest element of the union.

We can use the unions in the following locations.

- Share a single memory location for a variable and use the same location for another variable of different data type.
- Use it if you want to use, for example, a long variable as two short type variables.
- We don’t know what type of data is to be passed to a function, and you pass union which contains all the possible data types.

```
var aUnion: uni[int[8], int, flt]; 
```



## Any
```
any[]
```

## Null

```
nil
```

In the current `V1` compiler milestone, `nil` is accepted only when type
checking already knows it is flowing into an `opt[...]` or `err[...]` shell.
Plain inference from `nil` alone is not supported.

Current `V1` shell note:

- `opt[...]` and `err[...]` are shell values.
- postfix unwrap `value!` applies to those shell values.
- routine calls declared with `ResultType / ErrorType` are not `err[...]`
  shells.
- use `check(...)` or `expr || fallback` for those routine calls instead of
  `!`.
- use `err[...]` when you need a storable error value.

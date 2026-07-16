# Routines

Routines are callable declarations.

FOL has three routine families:
- `pro`:
  procedures, used for effectful work
- `fun`:
  functions, intended for ordinary value-producing computation
- `log`:
  logical routines and relation-like callable forms

Routine declarations support a shared structural pattern:

```fol
fun[options] name(params): return_type = { body }
pro[options] name(params): return_type = { body }
log[options] name(params): return_type = { body }
```

FOL also allows an alternate header style:

```fol
fun[options] name: return_type = (params) { body }
```

This chapter family covers parameters, calls, defaults, variadics, return values, and routine-specific semantics.

Current `V1` routine model:

- values are returned with an explicit `return`; there is no implicit `result`
  variable in the current compiler
- the short form that omits the return type is not current; the return type
  must be declared
- last-expression / implicit-tail return is not current
- routine overloading is not supported; duplicate routine names are rejected
- a routine that declares a recoverable error type (`: T / E`) must have both a
  return path and a `report` path

The sections further down that show `result = ...`, short-form omission,
last-expression return, and same-name overloading describe earlier design, not
the current compiler surface.

## Types
There are two main types of routines in fol:

- Procedurues

    A procedure is a piece of code that is called by name. It can be passed data to operate on (i.e. the parameters) and can optionally return data (the return value). All data that is passed to a procedure is explicitly passed.

- Functions

    A function is called pure function if it always returns the same result for same argument values and it has no side effects like modifying an argument (or global variable) or outputting to I/O. The only result of calling a pure function is the return value.

## Parameters
### Formal parameters

Routines typically describe computations. There are two ways that a routine can gain access to the data that it is to process: through direct access to nonlocal variables (declared elsewhere but visible in the routine) or through parameter passing. Data passed through parameters are accessed using names that are local to the routine. Routine create their own unnamed namespace. Every routine has its own Workspace. This means that every variable inside the routine is only usable during the execution of the routine (and then the variables go away). 

Parameter passing is more flexible than direct access to nonlocal variables. Prrameters are special variables that are part of a routine’s signature. When a routine has parameters, you can provide it with concrete values for those parameters. The parameters in the routine header are called formal parameters. They are sometimes thought of as dummy variables because they are not variables in the usual sense: In most cases, they are bound to storage only when the routine is called, and that binding is often through some other program variables.

Parameters are declared as a list of identifiers separated by semicolon (or by a colon, but for code cleanness, the semicolon is preferred). A parameter is given a type by : typename. If after the parameter the `:` is not declared, but `,` colon to identfy another paremeter, of which both parameters are of the same type if after the second one the `:` and the type is placed. Then the same type parameters continue to grow with `,` until `:` is reached.
```
fun[] calc(el1, el2, el3: int[64]; changed: bol = true): int[64] = { result = el1 + el2 - el3 }
```

In routine signatures, you must declare the type of each parameter. Requiring type annotations in routine definitions is obligatory, which means the compiler almost never needs you to use them elsewhere in the code to figure out what you mean. Routine can parameter overloaded too. It makes possible to create multiple routine of the same name with different implementations. Calls to an overloaded routine will run a specific implementation of that routine appropriate to the context of the call, allowing one routine call to perform different tasks depending on context:

```
fun retBigger(el2, el2: int): int = { return el1 | this > el2 | el2 }
fun retBigger(el2, el2: flt): flt = { return el1 | this > el2 | el2 }

pro main: int = {
    retBigger(4, 5);                                        // calling a routine with intigers
    retBigger(4.5, .3);                                     // calling another routine with same name but floats
}
```
The overloading resolution algorithm determines which routine is the best match for the arguments. Example:
```
pro toLower(c: char): char = {                              // toLower for characters
    if (c in {'A' ... 'Z'}){
        result = chr(ord(c) + (ord('a') - ord('A')))
    } else {
        result = c
    }
}

pro toLower(s: str): str = {                                // toLower for strings
    result = newString(.len(s))
    for i in {0 ... len(s) - 1}:
        result[i] = toLower(s[i])                           // calls toLower for characters; no recursion!
}
```

### Actual parameters
routine call statements must include the name of the routine and a list of parameters to be bound to the formal parameters of the routine. These parameters are called actual parameters. They must be distinguished from formal parameters, because the two usually have different restrictions on their forms.

#### Positional parameters

The correspondence between actual and formal parameters, or the binding of actual parameters to formal parameters - is done by position: The first actual parameter is bound to the first formal parameter and so forth. Such parameters are called positional parameters. This is an effective and safe method of relating actual parameters to their corresponding formal parameters, as long as the parameter lists are relatively short. 

```
fun[] calc(el1, el2, el3: int): int = { result = el1 + el2 - el3 }

pro main: int = {
    calc(3,4,5);                                            // calling routine with positional arguments
}
```

#### Keyword parameters

When parameter lists are long, however, it is easy to make mistakes in the order of actual parameters in the list. One solution to this problem is with keyword parameters, in which the name of the formal parameter to which an actual parameter is to be bound is specified with the actual parameter in a call. The advantage of keyword parameters is that they can appear in any order in the actual parameter list. 

```
fun[] calc(el1, el2, el3: int): int = { result = el1 + el2 - el3 }

pro main: int = {
    calc(el3 = 5, el2 = 4, el1 = 3);                        // calling routine with keywords arguments
}
```
#### Mixed parameters
Keyword and positional arguments can be used at the same time too. In `V1`,
ordinary positional arguments must come before named arguments. After a named
argument appears, later ordinary arguments are rejected because position is no
longer well defined.

The one supported exception is call-site unpack for the final variadic
parameter. `...items` may appear after named arguments when it is feeding that
final variadic input.

```
fun[] calc(el1, el2, el3: int, el4, el5: flt): int = { result[0] = ((el1 + el2) * el4 ) - (el3 ** el5);  }

pro main: int = {
    calc(3, 4, el5 = 2, el4 = 5, el3 = 6);                  // element $el3 needs to be keyeorded at the end because 
                                                            // its positional place is taken by keyword argument $el5
}
```

This remains invalid:

```fol
calc(el3 = 5, 4, el1 = 3)
```

But this is valid in `V1` when the last parameter is variadic:

```fol
fun[] score(base: int, step: int = 2, extras: ... int): int = {
    return base;
}

fun[] run(): int = {
    var extras: seq[int] = {4, 5};
    return score(base = 3, ...extras);
}
```

### Default arguments
Formal parameters can have default values too. A default value is used if no
actual parameter is passed to the formal parameter.

In `V1`:

- omitted parameters use their declared default
- named arguments may skip over defaulted parameters
- defaults can coexist with a final variadic parameter

```
fun[] calc(el1, el2, el3: int, rise: bool = true): int = { result[0] = el1 + el2 * el3 | this | el1 + el2;  }

pro main: int = {
    calc(3,3,2);                                            // this returns 6, last positional parameter is not passed but 
                                                            // the default `true` is used from the routine declaration
    calc(3,3,2,false)                                       // this returns 12
    calc(el1 = 3, el2 = 3, el3 = 2)                         // named arguments may also rely on the default
}
```

### Variadic routine
The use of `...` as the type of argument at the end of the argument list
declares the routine as variadic. This must appear as the last argument of the
routine.

In `V1`, the final variadic parameter is bound as a `seq[...]` value. Extra
trailing call arguments are collected into that sequence.

```
fun[] calc(rise: bool; ints: ... int): int = { result[0] = ints[0] + ints[1] + ints[2] * ints[3] | this | ints[0] + ints[1];  }

pro main: int = {
    calc(true,3,3,3,2);                                     // this returns 81, four parmeters are passed as variadic arguments
    calc(true,3,3,2)                                        // this returns 0, as the routine multiplies with the forth varadic parameter
                                                            // and we have given only three (thus the forth is initialized as zero)
}
```

Call-site unpack is the companion feature to variadics. It forwards an existing
sequence into the final variadic parameter:

```fol
fun[] calc(rise: bool; ints: ... int): int = {
    return ints[0];
}

fun[] run(values: seq[int]): int = {
    return calc(true, ...values);
}
```

This also works after named arguments:

```fol
fun[] score(base: int, step: int = 2, extras: ... int): int = {
    return base;
}

fun[] run(values: seq[int]): int = {
    return score(base = 3, ...values);
}
```

`V1` intentionally does not support pseudo-arguments such as
`extras[0] = 1` at call sites. Variadic inputs are still just routine
parameters, so call binding stays limited to:

- ordinary positional arguments
- named arguments by declared parameter name
- one final `...sequence` unpack for the variadic tail

{{% notice warn %}}

Nested procedures don't have access to the outer scope, while nested function have but can't change the state of it.

{{% /notice %}}

## Return

The return type of the routine has to always be defined, just after the formal parameter definition. Following the general rule of **FOL**: 
```
fun[] add(el1, el2: int[64]): int[64] = { result = el1 + el2 }
```

To make it shorter (so we don't have to type `int[64]` two times), we can use a *short form* by omitting the return type. The compiler then will assign the returntype the same as the functions return value.
```
fun[] add(el1, el2: int[64]) = { result = el1 + el2 }
```
{{% notice info %}}

Current `V1` routine summary:

- routines declare a success type after `:`
- routines may also declare a recoverable error type after `/`
- `report expr` exits through that declared error path
- routine call results declared with `/ ErrorType` are not `err[...]` shell
  values
- use `check(...)` or `expr || fallback` for those calls
- ordinary plain-value use of `/ ErrorType` calls is rejected
- keep postfix `!` for `opt[...]` and `err[...]` shell values

{{% /notice %}}

The implicitly declared variable `result` is of the same type of the return type. For it top be implicitly declared, the return type of the function shoud be always declared, and not use the short form. The variable is initialized with zero value, and if not changed during the body implementation, the same value will return (so zero).
```
pro main(): int = {
    fun[] add(el1, el2: int[64]): int[64] = { result = el1 + el2 }          // using the implicitly declared $result variable
    fun[] sub(el1, el2: int[64]) = { return el1 - el2 }                     // can't access the result variable, thus we use return
}
```
Recoverable error-aware routines use the current signature form:
```
fun[] read(path: str): int / str = {
    report "missing path"
}
```
and are handled at the call site with `check(...)` or `||` rather than shell
unwrap or plain propagation.

Current intrinsic note:

- `.echo(...)` is a dot-root diagnostic intrinsic
- `check(...)` is a keyword intrinsic for recoverable-call inspection
- `panic(...)` is a keyword intrinsic for immediate abort
- `as` and `cast` are registry-owned but still deferred in current `V1`

Model reminder:

- any routine example that calls `.echo(...)` assumes a `memo` artifact with
  bundled `std` support available
- routine examples without hosted behavior should stay valid in `core` or
  `memo` where the surrounding chapter claims they are model-neutral

The final expression in the function will be used as return value. For this to be used, the return type of the function needs to be defined (so the function cnat be in the short form)). ver this can be used only in one statement body.
```
pro main(): int = {
    fun[] add(el1, el2: int[64]): int[64] = { el1 + el2 }                   // This is tha last statement, this will serve as return
    fun[] someting(el1,el2: int): int = {
        if (condition) {

        } else {

        }
        el1 + el2                                                           // this will throw an error, cand be used in kulti statement body
    }
    fun[] add(el1, el2: int[64]) = { el1 + el2 }                            // this will throw an error, we can't use the short form of funciton in this way
```
Alternatively, `return` and `report` can exit a routine early from within
control flow. See the recoverable-error chapter for the full current `V1`
contract and the shell-vs-routine distinction.

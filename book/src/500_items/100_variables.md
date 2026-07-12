# Variables

Current boundary:

- ordinary `var`, `con`, and `lab` declarations are the current compiler surface
- the `@var` and heap `var[new]` spellings are later design work, not current
  behavior
- the pipe-ternary and many-to-many assignment forms shown below are later
  design work, not current behavior

Here are some of the ways that variables can be defined:
```
var[mut] counter: int = 98
var[exp] label: str = "this is a string"
var[~] ratio = 192.56
+var short_flag = true
var names: arr[str, 3] = { "one", "two", "three" }
var scores: seq[int] = { 20, 25, 45, 68, 73, 98 }
var pair: set[int, str] = { 12, "word" }
var picked = names[1]
```

## Assignments

Following the general rule of **FOL**:
```
declaration[options] name: type[options] = { implementation; };
```
then declaring a new variable is like this:
```
var[exp] aVar: int = 64
```

however, the short version can be used too, and the compiler figures out at compute time the type:
```
var shortVar = 24;                      // compiler gives this value of `int[arch]`
```

When new variable is created, and uses an old variable to assign, the resulting
binding is a new value binding rather than an alias to the old name:
```
pro[] main: int = {
    var aVar: int = 55;
    var newVar: int = aVar;
    return newVar;
}
```
Ownership, borrowing, and pointer-level aliasing are later systems-language
work and are described in the memory chapters as future milestones rather than
as part of the current `V1` compiler contract.

Variables can be assigned to an output of a function:
```
pro[] main: int = {
    fun addFunc(x, y: int): int = {
        return x + y;
    }
    var aVar: int = addFunc(4, 5);
}
```
### Piping / Ternary

Piping can be used as ternary operator. More about piping can be [found here](/docs/spec/pipes). Here is an example, the code below basically says: **if the function internally had an error, don't exit the program, but assign another value (or default value) to the variable**:
```
pro[] main: int = {
    fun addFunc(x, y: int): int = {
        return x + y;
    }
    var aVar: int = addFunc(4, 5) | result > 8 | return 6;
}
```
### Borrowing

If we want to reference a variable, the easiest way is to borrow the variable, use inside another scope (or the same) and return it back. If the ownership is not returned manually, by the end of the scope, it gets returned automatically. 
```
pro[] main: int = {
    var[~] aVar: int = 55;
    {
        var[bor] newVar: int = aVar         // var[bor] represents borrowing
        .echo(newVar)                       // this return 55
    }
        .echo(aVar)                         // here $aVar it not accesible, as the ownership returns at the end of the scope
        .echo(newVar)                       // we cant access the variable because the scope has ended
}
```
More on borrowing you can find [here](/docs/spec/pointers/#borrowing)

## Options
As with all other blocks, `var` have their options: `var[opt]`:

Options can be of two types: 
  - flags eg. `var[mut]`
  - values eg. `var[pri=2]`

Flag options can have symbol aliases eg. `var[mut]` is the somename as `var[~]`.

```
|  opt   | s |   type    | description                                       | control       |
----------------------------------------------------------------------------------------------
|  mut   | ~ |   flag    | making a variable mutable                         | mutability    |
|  imu   |   |   flag    | making a variable imutable (default)              |               |
|  sta   | ! |   flag    | making a variable a static                        |               |
|  rac   | ? |   flag    | making a variable reactive                        |               |
----------------------------------------------------------------------------------------------
|  exp   | + |   flag    | making a global variable exported                 | visibility    |
|  nor   |   |   flag    | making a global variable normal (default)         |               |
|  hid   | - |   flag    | making a global variable file-local               |               |
```

### Alternatives
There is a shorter way for variables using alternatives, for example, instead of using `var[+]`, a leaner `+var` can be used instead.
```
+var aVar: int = 55
fun[] main(): int = {
    .echo(aVar)
    return aVar
}
```
However, when we use two option in varable, only one can use the alternative form, so instead of using `var[mut,exp]`, this can be used `+var[mut]` or `+var[~]`, or vice varsa `~var[exp]` or `~var[+]`:
```
+var[mut] aVar: int = 55
fun[] main(): int = {
    .echo(aVar)
    return aVar
}
```
## Types

### Immutable types (constants)
By default when a variable is defined without options, it is immutable type, for example here an intiger variable:
```
pro[] main: int = {
    var aNumber: int = 5;
    aNumber = 54;                       // reassigning varibale $aNumber thorws an error
}
```
### Mutable types
If we want a variable to be mutable, we have to explicitly pass as an option to the variable `var[mut]` or `var[~]`:
```
pro[] main: int = {
    var[mut] aNumber: int = 5
    var[~] anotherNumber: int = 24
    aNumber, anotherNumber = 6          // this is completely fine, we assign two wariables new values
}
```
### Reactive types
Current milestone note: reactive variables are part of a later milestone, not
the current `V1` compiler contract. The syntax may appear in design examples,
but present-day `V1` typechecking rejects reactive semantics explicitly.

Reactive types is a types that flows and propagates changes. 

For example, in an normal variable setting, `var a = b + c` would mean that `a` is being assigned the result of `b + c` in the instant the expression is evaluated, and later, the values of `b` and `c` can be changed with no effect on the value of `a`. On the other hand, declared as reactive, the value of `a` is automatically updated whenever the values of `b` or `c` change, without the program having to re-execute the statement `a = b + c` to determine the presently assigned value of `a`.
```
pro[] main: int = {
    var[mut] b, c = 5, 4;
    var[rac] a: int = b + c
    .echo(a)                            // prints 9
    c = 10;
    .echo(a)                            // now it prints 10
}
```
### Static types
Current milestone note: static variables are also part of later systems/runtime
work. The current `V1` compiler keeps them outside the implemented subset.

Is a variable which allows a value to be retained from one call of the function to another, meaning that its lifetime declaration. and can be used as `var[sta]` or `var[!]`. This variable is special, because if it is initialized, it is placed in the [data segment](https://en.wikipedia.org/wiki/Data_segment) (aka: initialized data) of the program memory. If the variable is not set, it is places in [.bss segmant](https://en.wikipedia.org/wiki/.bss) (aka: uninitialized data)
```
pro[] main: int = {
    {
        var[!] aNumber: int = 5
    }
    {
        .echo(aNumber)                  // it works as it is a static variable.
    }
}
```

### Scope

As discussed before, files in the same package share one package scope. That means package-level functions and variables may be used across sibling files without importing those sibling files one by one.

However, package-private declarations are still different from exported declarations:

- default visibility means the declaration is available inside the same package
- `exp` / `+` means the declaration may be used through imports from outside the package
- `hid` / `-` means the declaration is visible only inside its own file

So the visibility model is:

- package scope by default
- exported outside the package with `exp`
- file-only with `hid`

In order for a variable to be accessed by the importer, it needs the `exp` flag option, so `var[exp]`, or `var[+]`.

*package **shko**, file1.fol*
```
fun[exp] add(a, b: int): int = { return a + b }
fun sub(a, b: int): int = { return a - b }
```
*package **vij**, file1.fol*
```
use shko: loc = {"../folder/shko"}

fun[] main(): int = {
    .echo(add(5, 4))                    // this works, `add` is exported
    .echo(sub(5, 4))                    // this fails, `sub` is not exported
    return add(5, 4)
}
```
There is even the opposite option too. If we want a function or variable to be used only inside its own file, even though the package is shared, then we use the `hid` option flag: `var[hid]` or `var[-]`.

*file1.fol*
```
var[-] aVar: str = "yo, sup!"
```

*file2.fol*
```
fun[] main(): int = {
    .echo(aVar)                           // this throws, `aVar` is hidden to its own file
    return 0
}
```
## Multiple

### Many to many

Many variables can be assigned at once, This is especially usefull, if variables have same options but different types eg. variable is mutabe and exported:

```
~var[exp] oneVar: int[32] = 24, twoVar = 13, threeVar: string = "shko";
```

Or to assign multiple variables of the same type:
```
~var[exp] oneVar, twoVar: int[32] = 24, 13;
```

To assign multiple variables of multiple types, the type is omitted, however, this way we can not put options on the type (obviously, the default type is assign by compiler):
```
~var[exp] oneVar, twoVar, threeVar = 24, 13, "shko";
```

Another "shameless plagiarism" from golang can be used by using `( ... )` to group variables:
```
~var[exp] (
    oneVar: int[32] = 13,
    twoVar: int[8] = 13,
    threeVar: str = "shko",
)
```

### Many to one

Many variables of the same type can be assigned to one output too:

```
var oneVar, twoVar: int[8] = 2;
```
However, each of them gets a copy of the variable on a new memory address:

```
.assert(&oneVar == &twoVar)           // this will return false
```

### One to many

And lastly, one variable can be assigned to multiple ones. This by using container types:
```
oneVar grouppy: seq[int] = { 5, 2, 4, 6 }
```
Or a more complicated one:
```
var anothermulti: set[str, seq[num[f32]]] = { "string", {5.5, 4.3, 7, .5, 3.2} }
```

Or a very simple one:
```
var simplemulti: any = { 5, 6, {"go", "go", "go"} }
```

## Containers

Containers are of special type, they hold other types within. As described before, there are few of them

### Access
To acces container variables, brackets like this `[]` are use:
```
var shortvar = anothermulti[1][3]     // compiler will copy the value `anothermulti[1][3]` (which is a float) to a new memory location
```

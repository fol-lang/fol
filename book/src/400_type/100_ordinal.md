# Ordinal

Ordinal types

Ordinal types have the following characteristics:

- Ordinal types are countable and ordered. This property allows the operation of functions as inc, ord, dec on ordinal types to be defined.
- Ordinal values have a smallest possible value. Trying to count further down than the smallest value gives a checked runtime or static error.
- Ordinal values have a largest possible value. Trying to count further than the largest value gives a checked runtime or static error.


Ordinal types are the most primitive type of data:

- Intigers: `int[options]`
- Floating: `flt[options]`
- Characters: `chr[options]`
- Booleans: `bol`

### Intiger type
An integer is a number without a fractional component. We used one integer of the u32 type, the type declaration indicates that the value it’s associated with should be an unsigned integer (signed integer types start with i, instead of u) that takes up 32 bits of space: 
```
var aVar: int[u32] = 45;
```
Each variant can be either signed or unsigned and has an explicit size. Signed and unsigned refer to whether it’s possible for the number to be negative or positive—in other words, whether the number needs to have a sign with it (signed) or whether it will only ever be positive and can therefore be represented without a sign (unsigned). It’s like writing numbers on paper: when the sign matters, a number is shown with a plus sign or a minus sign; however, when it’s safe to assume the number is positive, it’s shown with no sign.
```
Length  |   Signed  | Unsigned  |
-----------------------------------
8-bit   |   8       |   u8      |
16-bit  |   16      |	u16     |
32-bit	|   32      |	u32     |
64-bit	|   64	    |   u64     |
128-bit	|   128     |	u128    |
arch	|   arch    |	uarch   |
```

### Float type
Fol also has two primitive types for floating-point numbers, which are numbers with decimal points. Fol’s floating-point types are `flt[32]` and `flt[64]`, which are 32 bits and 64 bits in size, respectively. The default type is `flt[64]` because on modern CPUs it’s roughly the same speed as `flt[32]` but is capable of more precision. 
```
Length  |    Type  |
--------------------
32-bit	|   32     |
64-bit	|   64     |
arch	|   arch   |
```
Floating-point numbers are represented according to the IEEE-754 standard. The `flt[32]` type is a single-precision float, and `flt[f64]` has double precision.

```
pro[] main: int = {
    var aVar: flt = 2.;                         // float 64 bit
    var bVar: flt[64] = .3;                     // float 64 bit
    .echo(.eq(aVar, bVar))                      // compare values with the V1 intrinsic surface

    var bVar: flt[32] = .54;                    // float 32 bit
}
```
### Character type
In The Unicode Standard 8.0, Section 4.5 "General Category" defines a set of character categories. Fol treats all characters in any of the letter as Unicode letters, and those in the Number category as Unicode digits. 
```
chr[utf8,utf16,utf32]
```

```
def testChars: tst["some testing on chars"] = {
    var bytes = "hello";
    .echo(.len(bytes));
    .echo(bytes[1]);
    .echo(.eq("e", "\x65"));
}

Current `V1` intrinsic note:

- `.eq(...)` is implemented for scalar equality
- `.len(...)` is implemented for strings and supported containers
- string `.len(...)` counts UTF-8 BYTES, not characters (`.len("héllo")` is 6)
- older helper surfaces such as `.assert(...)`, `.typeof(...)`, and related
  compile-time queries are not part of the current implemented intrinsic subset
```
### Boolean type
The boolean type is named `bol` in Fol and can be one of the two pre-defined values `true` and `false`. 
```
bol
```

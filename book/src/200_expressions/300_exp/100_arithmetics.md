# Calculations

In fol, every calcultaion, needs to be enclosed in rounded brackets `( //to evaluate )` - except in one line evaluating, the curly brackets are allowed too `{ // to evaluate }`:

```
fun adder(a, b: int): int = {
    retun a + b                                                 // this will throw an error 
}

fun adder(a, b: int): int = {
    retun (a + b)                                               // this is the right way to enclose 
}
```

Order of evaluation is strictly left-to-right, inside-out as it is typical for most others imperative programming languages:

```
.echo((12 / 4 / 8))                                             // 0.375 (12 / 4 = 3.0, then 3 / 8 = 0.375)
.echo((12 / (4 / 8)))                                           // 24 (4 / 8 = 0.5, then 12 / 0.5 = 24)
```


Calculation expressions include:

- arithmetics
- comparison
- logical
- compounds


## Arithmetics

The behavior of arithmetic operators is only on intiger and floating point primitive types. For other types, there need to be operator overloading implemented.
 
 symbol | description
 --- | ---
\-   | substraction
\*   | multiplication
\+   | addition
/    | division
%    | reminder
^    | exponent

```
assert((3 + 6), 9);
assert((5.5 - 1.25), 4.25);
assert((-5 * 14), -70);
assert((14 / 3), 4);
assert((100 % 7), 2);
```

Current integer semantics:

- division truncates toward zero (`-7 / 2` is `-3`)
- remainder takes the sign of the dividend (`-7 % 2` is `-1`)
- integers are 64-bit two's-complement; overflow panics at runtime (debug)
  rather than wrapping, and division/modulo by zero panics
- declared widths such as `int[8]` currently share the 64-bit runtime
  representation and are not range-enforced yet
- floats follow IEEE-754: `1.0 / 0.0` is infinity, `0.0 / 0.0` is NaN (no panic)

```
```


## Comparisons

Comparison operators are also defined both for primitive types and many type in the standard library. Parentheses are required when chaining comparison operators. For example, the expression `a == b == c` is invalid and may be written as `((a == b) == c)`.

Symbol  |	Meaning
---     | --- 
==	    | equal
!=	    | not equal
\>	    | greater than
\<	    | Less than
\>=	    | greater than or equal to
\<=	    | Less than or equal to


```
assert((123 == 123));
assert((23 != -12));
assert((12.5 > 12.2));
assert((1 < 3));
assert(('A' <= 'B'));
assert(("World" >= "Hello"));
```

## Logical

A branch of algebra in which all operations are either true or false, thus operates only on booleans, and all relationships between the operations can be expressed with logical operators such as:

- `and` (conjunction), denoted `(x and y)`, satisfies `(x and y) = 1` if `x = y = 1`, and `(x and y) = 0` otherwise.
- `or` (disjunction), denoted `(x or y)`, satisfies `(x or y) = 0` if `x = y = 0`, and `(x or) = 1` otherwise.
- `not` (negation), denoted `(not x)`, satisfies `(not x) = 0` if `x = 1` and (not x) = 1` if `x = 0`.

```
assert((true and false), (false and true));
assert((true or false), true)
assert((not true), false)
```

Current evaluation note:

- `and`/`or` evaluate both operands eagerly; they do NOT short-circuit, so
  the right operand runs even when the left already decides the result
  (`(false) and (1/0 > 0)` still evaluates `1/0`). Guard side-effecting or
  fallible right operands explicitly.



## Compounds

There are further assignment operators that can be used to modify the value of an existing variable. These are the compounds or aka compound assignments. A compound assignment operator is used to simplify the coding of some expressions. For example, using the operators described earlier we can increase a variable's value by ten using the following code:
```
value = value + 10;
```
This statement has an equivalent using the compound assignment operator for addition (+=).

```
value += 10;
```

There are compound assignment operators for each of the six binary arithmetic operators: `+`, `-`, `*`, `/`, `%` and `^`. Each is constructed using the arithmetic operator followed by the assignment operator. The following code gives examples for addition `+=`, subtraction `-=`, multiplication `*=`, division `/=` and modulus `%=`:
```

var value: int = 10;
(value += 10);        // value = 20
(value -= 5);         // value = 15
(value *= 10);        // value = 150
(value /= 3);         // value = 50
(value %= 8);         // value = 2
```

Compound assignment operators provide two benefits. Firstly, they produce more compact code; they are often called shorthand operators for this reason. Secondly, the variable being operated upon, or operand, will only be evaluated once in the compiled application. This can make the code more efficient.

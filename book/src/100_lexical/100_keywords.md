# Keywords

Fol has a number of restricted groups of keywords:

## BK (build-in keywords)

```

BK_OR              `or`
BK_XOR             `xor`
BK_AND             `and`

BK_IF              `if`
BK_FOR             `for`
BK_WHEN            `when`
BK_EACH            `each`
BK_LOOP            `loop`

BK_IS              `is`
BK_HAS             `has`
BK_IN              `in`

BK_THIS            `this`
BK_SELF            `self`

BK_BREAK           `break`
BK_RETURN          `return`
BK_YEILD           `yield`
BK_DFR             `dfr`
BK_PANIC           `panic`
BK_REPORT          `report`
BK_CHECK           `check`
BK_ASSERT          `assert`
BK_WHERE           `where`

BK_TRUE            `true`
BK_FALSE           `false`

BK_AS              `as`
BK_CAST            `cast`

BK_DO              `do`

```

```
BUILD-IN KEYWORDS - BK:
`(BK_AS|BK_IN|...)`

```


## AK (assignment keywords)

```

AK_USE             `use`
AK_DEF             `def`
AK_VAR             `var`
AK_FUN             `fun`
AK_PRO             `pro`
AK_LOG             `log`
AK_TYP             `typ`
AK_STD             `std`

```


```
ASSIGNMENT KEYWORDS - AK:
`(AK_USE|AK_DEF|...)`
```



## TK (type keywords)

```
TK_INT             `int`
TK_FLT             `flt`
TK_CHR             `chr`
TK_BOL             `bol`

TK_ARR             `arr`
TTKVEC             `vec`
TK_SEQ             `seq`
TK_MAT             `mat`
TK_SET             `set`
TK_MAP             `map`

TK_STR             `str`
TK_NUM             `num`

TK_OPT             `opt`
TK_MUL             `mul`
TK_ANY             `any`

TK_PTR             `ptr`
TK_ERR             `err`
TK_NON             `non`

TK_REC             `rec`
TK_LST             `lst`
TK_ENM             `enm`
TK_UNI             `uni`
TK_CLS             `cls`

TK_STD             `std`
TK_MOD             `mod`
TK_BLK             `blk`
```

```
TYPE KEYWORDS - TK:
`(TK_INT|TK_FLT|...)`
```



Note that all of the **type keywords** are of three characters long. It is recomanded that new identifiers not to be of the same number of characters, as one day in the future that same identifier can be used s a keyword in FOL compiler.


## OK (option keywords)

```
OK_EXP              `exp`
OK_HID              `hid`
```

Older draft material sometimes used `pub`. The current compiler/book contract
uses `exp` for exported visibility and `hid` for file-local visibility.


```
OPTION KEYWORDS - OK:
`((OK_EXP|OK_HID|...),?)*`
```

## Assigning

```
`(`*WS*`)*(\W)?(`*AK*`)(\[(`*OK*`)?\])?`

`(`*WS*`)*(`*AK*`)`
| `(`*WS*`)*\W(`*AK*`)`
| `(`*WS*`)*(`*AK*`)(\[\])`
| `(`*WS*`)*\W(`*AK*`)(\[\])`
| `(`*WS*`)*(`*AK*`)(\[(`*OK*`)\])`
| `(`*WS*`)*\W(`*AK*`)(\[(`*OK*`)\])`
```

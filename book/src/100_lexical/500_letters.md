# Strings

## Characters 
A character is a **single** Unicode element enclosed within quotes `U+0022` (`"`) with the exception of `U+0022` itself, which must be escaped by a preceding `U+005C` character (`\`).

```
var aCharacter: chr = "\n"
var anotherOne: str = "語\n"
```
### Raw characters

Raw character literals do not process any escapes. They are enclosed within single-quotes `U+0027` (`'`) with the exception of `U+0027` itself:
```
var aCharacter: chr = 'z'
```


## Strings
A string is a **single** or a **sequence** of Unicode elements enclosed within quotes `U+0022` (`"`) with the exception of `U+0022` itself, which must be escaped by a preceding `U+005C` character (`\`).
```
var hiInEnglish: str = "Hello, world!\n"
var hInCantonese: str = "日本語"
```

Line-breaks are allowed in strings. A line-break is either a newline (`U+000A`) or a pair of carriage return and newline (`U+000D`, `U+000A`). Both byte sequences are normally translated to `U+000A`, but as a special exception, when an unescaped `U+005C` character (`\` occurs immediately before the line-break, the `U+005C` character, the line-break, and all whitespace at the beginning of the next line are ignored. Thus a and b are equal:

```
var a: str = "foobar";
var b: str = "foo\
              bar";

assert(a,b);
```

### Escape sequences

Some additional escapes are available in either character or non-raw string literals.

code | description
--- | ---
\p	|   platform specific newline: CRLF on Windows, LF on Unix
\r, \c	|   carriage return
\n, \l	|   line feed (often called newline)
\f	|   form feed
\t	|   tabulator
\v	|   vertical tabulator
\\\	|   backslash
\\"	|   quotation mark
\\'	|   apostrophe
\ '0'..'9'+	|   character with decimal value d; all decimal digits directly following are used for the character
\a	|   alert
\b	|   backspace
\e	|   escape [ESC]
\x HH	|   character with hex value HH; exactly two hex digits are allowed
\u HHHH	|   unicode codepoint with hex value HHHH; exactly four hex digits are allowed
\u {H+}	|   unicode codepoint; all hex digits enclosed in {} are used for the codepoint

### Raw strings
Just like [raw characters](/docs/100_lex/strings/#raw-characters), raw string literals do not process any escapes either. They are enclosed within single-quotes `U+0027` (`'`) with the exception of `U+0027` itself:

```
var hiInEnglish: str = 'Hello, world!'
```


## Booleans
The two values of the boolean type are written `true` and `false`:

```
var isPresent: bol = false;
```

# Unpacking

Current boundary:

- unpacking / iterable-destructuring forms (`var a, *_ = ...`) are not part of
  the current compiler surface; the compiler rejects them
- everything below describes intended design, not current behavior
- this is later design work, not part of the current V1 compiler surface

Unpacking—also known as iterable destructuring—is another form of pattern matching used to extract data from collections of data. Take a look at the following example:
```
var start, *_ = { 1, 4, 3, 8 }
.echo(start)                                // Prints 1
.echo(_)                                    // Prints [4, 3, 8]
```

In this example, we’re able to extract the first element of the list and ignore the rest. Likewise, we can just as easily extract the last element of the list:
```
var *_, end = { "red", "blue", "green" }
.echo(end)                                  // Prints "green"
```

In fact, with pattern matching, we can extract whatever we want from a data set assuming we know it’s structure:
```
var start, *_, (last_word_first_letter, *_) = { "Hi", "How", "are", "you?" }
.echo(last_word_first_letter)               // Prints "y"
.echo(start)                                // Prints "Hi"
```

Now, that has to be one of the coolest programming language features. Instead of extracting data by hand using indices, we can just write a pattern to match which values we want to unpack or destructure.

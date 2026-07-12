# Inquiry

Current boundary:

- inquiry `where(...)` clauses parse, but no inquiry body typechecks today; the
  `is` operator used inside them is gated
- everything below describes intended design, not current behavior; inquiries
  are later design work, not part of the current compiler surface

Inquiries are inline unit tests and are a part of the basic syntax sugar. In other words, we don’t have to import any libraries or build up any suites to run tests.

Instead, FOL includes a couple of clauses for testing within the source code:
```
fun sum(l: ...?int): int = {
    when(l.length()) {
        is 0 => 0;
        is 1 => l[0];
        $ => l[0] + sum(l[1:]);
    }

    where(self) {
        sum() is 0;
        sum(8) is 8;
        sum(1, 2, 3) is 6;
    }
} 
```

Here, we can see an awesome list sum function. Within the function, there are two basic cases: empty and not empty. In the empty case, the function returns 0. Otherwise, the function performs the sum.

At that point, most languages would be done, and testing would be an afterthought. Well, that’s not true in FOL. To add tests, we just include a where clause. In this case, we test an empty list and a list with an expected sum of 6.

When the code is executed, the tests run. However, the tests are non-blocking, so code will continue to run barring any catastrophic issues.

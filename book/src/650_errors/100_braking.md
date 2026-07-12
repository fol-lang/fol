# Breaking errors

`panic` keyword allows a program to terminate immediately and provide feedback to the caller of the program. It should be used when a program reaches an unrecoverable state. This most commonly occurs when a bug of some kind has been detected and it’s not clear to the programmer how to handle the error.

```
pro main(): int = {
    panic "Hello";
    .echo("End of main");                                                       //unreachable statement
}
```

In the above example, the program will terminate immediately when it encounters the `panic` keyword.
Output:
```
main.fol:3
routine 'main' panicked at 'Hello'
-------
```

Trying to acces an out of bound element of array:
```
pro main(): int = {
    var a: arr[int, 3] = {10,20,30};
    a[10];                                                                      //invokes a panic since index 10 cannot be reached
}
```
Output:
```
main.fol:4
routine 'main' panicked at 'index out of bounds: the len is 3 but the index is 10'
-------
a[10];
   ^-------- index out of bounds: the len is 3 but the index is 10

```
A program can invoke `panic` if business rules are violated, for example: if the value assigned to the variable is odd it throws an error:
```
pro main(): int = {
   var no = 13; 
   //try with odd and even
   if (no % 2 == 0) {
      .echo("Thank you , number is even");
   } else {
      panic "NOT_AN_EVEN"; 
   }
   .echo("End of main");
}
```

Output:
```
main.fol:9
routine 'main' panicked at 'NOT_AN_EVEN'
-------
```

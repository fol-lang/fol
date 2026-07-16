# Control

At least two linguistic mechanisms are necessary to make the computations in programs flexible and powerful: some means of selecting among alternative control flow paths (of statement execution) and some means of causing the repeated execution of statements or sequences of statements. Statements that provide these kinds of capabilities are called control statements. A control structure is a control statement and the collection of statements whose execution it controls. This set of statements is in turn generally structured as a block, which in addition to grouping, also defines a lexical scope. 

There are two types of control flow mechanisms:
- choice - `when`
- loop - `loop` (with `while`, `for`, and `each` as loop-header spellings of
  the same mechanism)


## Choice type
```
when(condition){ case(condition){} case(condition){} * {} };
when(variable){ is (value){}; is (value){}; * {}; };
when(variable){ in (iterator){}; in (iterator){}; * {}; };
when(iterable){ has (member){}; has (member){}; * {}; };
when(generic){ of (type){}; of (type){}; * {}; };
```

Current boundary:

- `case` and `is` arms (plus the required `*` default) are the current `V1`
  surface, in both statement bodies and the arrow expression form
  (`is 3 -> 7;`)
- a `when` with no `case`/`is` arms is a statement-only boolean gate: its
  default body runs only when the selector is `true`; arbitrary values are not
  coerced to truth values
- `in` (range/set matching), `has` (membership), `of` (type matching), and
  `on` are declared matching syntax whose semantics are later-milestone work;
  the compiler rejects them with explicit boundary diagnostics
- channel multiplexing is not an `on`-arm variant of ordinary `when`; the
  shipped V3 processor form is the separate `select { when channel as value
  { ... } }` statement shown below
### Condition
```
when(true) {
    case (x == 6){ // implementation }
    case (y.set()){ // implementation } 
    * { // default implementation }
}
```

### Valueation
```
when(x) {
    is (6){ // implementation }
    is (>7){ // implementation } 
    * { // default implementation }
}
```
### Iteration
```
when(2*x) {
    in ({0..4}){ // implementation }
    in ({ 5, 6, 7, 8, }){ // implementation } 
    * { // default implementation }
}
```
### Contains
```
when({4,5,6,7,8,9,0,2,3,1}) {
    has (5){ // implementation }
    has (10){ // implementation } 
    * { // default implementation }
}
```
### Generics
```
when(T) {
    of (int){ // implementation }
    of (str){ // implementation } 
    * { // default implementation }
}
```
### Channel multiplexing (`V3`)

`select` waits on direct channel bindings. Each `when` arm names one channel
and binds the payload received from it:

```fol
select {
    when first as value {
        consume(value);
    }
    when second as value {
        consume(value);
    }
    * {
        handle_not_ready();
    }
};
```

The optional `*` arm runs immediately when no receiver is ready. Without it,
the statement polls until one source-order arm receives a value or every arm
has closed. The processor surface is hosted `std`-only. See
[Tasks, Channels, and Mutexes](../../900_processor/200_corutines.md) for the
endpoint lifecycle and fairness contract.


## Loop type

```
loop(condition){};
loop(iterable){};
```

### Condition
```
loop( x == 5 ){
    // implementation
};
```

### Enumeration 
```
loop( x in {..100}){
    // implementation
}

loop( x in {..100}) if ( x % 2 == 0 )){
    // implementation
}

loop( x in {..100} if ( x in somearra ) and ( x in anotherarray )){
    // implementation
}

```
### Iteration
```
loop( x in array ){
    // implementation
}
```

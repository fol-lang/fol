# Structs

Structs are the way to declare new type of data. A struct binds an identifier, the type name, to a type.

A struct definition creates a new, distinct type and are few of them in FOL:

- records
- entries

## Definition

### Records

A record is an aggregate of data elements in which the individual elements are identified by names and types and accessed through offsets from the beginning of the structure. There is frequently a need in programs to model a collection of data in which the individual elements are not of the same type or size. For example, information about a college student might include name, student number, grade point average, and so forth. A data type for such a collection might use a character string for the name, an integer for the student number, a  floating- point for the grade point average, and so forth. Records are designed for this kind of need.

It may appear that records and heterogeneous [set](/docs/spec/types/#sets) are the same, but that is not the case. The elements of a heterogeneous `set[]` are all references to data values that may reside in scattered locations. The elements of a record are of potentially different sizes and reside in adjacent memory locations. Records are primarily data layouts.
```
typ user: rec = {
    var username: str;
    var email: str;
    var sign_in_count: int[64];
    var active: bol;
};
```

#### Records are data, not objects

`typ ...: rec = { ... }` declares a data type. FOL does not treat records as
objects with hidden behavior, inheritance, or class-owned method bodies. If a
record has operations associated with it, those operations are still declared
as ordinary receiver-qualified routines outside the record body.

```fol
typ computer: rec = {
    brand: str;
    memory: int
}

fun (computer)get_type(): str = {
    return self.brand
}

var laptop: computer = { brand = "acme", memory = 16 }
.echo(laptop.get_type())
```

The call `laptop.get_type()` is procedural sugar for calling the receiver
routine with `laptop` as its first explicit input.

Current `V1` backend/runtime note:

- backends may emit records and entries as plain target-language structs/enums
- that does not change the language model: they are still data plus ordinary
  receiver-qualified routines
- when runtime-visible formatting is needed, generated backends should preserve
  the `fol-runtime` aggregate formatting contract instead of inventing a
  backend-specific display shape



### Entries

Is an a group of constants (identified with `ent`) consisting of a set of named values called elements.
```
typ color: ent = {
    var BLUE: str = "#0037cd" 
    var RED str = "#ff0000" 
    var BLACK str = "#000000" 
    var WHITE str = "#ffffff" 
};

if( something == color.BLUE ) { doathing } else { donothing }
```

#### Entries as enums
Unums represent enumerated data. An enumeration type (or enum type) is a value type defined by a set of named constants of the underlying integral numeric type.
```
typ aUnion: ent = {
    var BLUE, RED, BLACK, WHITE: int[8] = {..3}
}
```

## Initializaion

To use a record after we’ve defined it, we create an instance of that record by specifying concrete values for each of the fields. We create an instance by stating the name of the record and then add curly brackets containing key: value pairs, where the keys are the names of the fields and the values are the data we want to store in those fields. We don’t have to specify the fields in the same order in which we declared them in the record. In other words, the record definition is like a general template for the type, and instances fill in that template with particular data to create values of the type.
```
@var user1: user = {
    email = "someone@example.com",
    username = "someusername123",
    active = true,
    sign_in_count = 1,
};
```

### Named initialization:
```
@var[mut] user1: user = { email = "someone@example.com", username = "someusername123", active = true, sign_in_count = 1 }
```

### Ordered initialization
```
@var[mut] user1: user = { "someone@example.com", "someusername123", true, 1 }
```


## Accessing

To get a specific value from a record, use dot notation: if we wanted just
this user's email address, we can use `user1.email` wherever we want that
value. If the instance is mutable, we can change a value by assigning into a
particular field. Note that the entire instance must be mutable; FOL doesn't
allow us to mark only certain fields as mutable.

```fol
var user1: user = {
    email = "someone@example.com",
    username = "someusername123",
    active = true,
    sign_in_count = 1,
};

user1.email = "new.mail@example.com";
```

Current boundaries:

- assignment targets a single field of a mutable binding (`user1.email = ...`);
  nested targets such as `a.b.c = ...` are not part of the current surface
- bracket field access (`user1[email]`) is a later design surface, not part of
  the current compiler contract
## Returning

As with any expression, we can construct a new instance of the record as the last expression in the function body to implicitly return that new instance. As specified [in function return](/docs/spec/functions/#return), the final expression in the function will be used as return value. For this to be used, the return type of the function needs to be defined (here is defined as `user`) and this can be used only in one statement body. Here we have declared only one variable `user1` and that itslef spanc into multi rows:
```
pro buildUser(email, username: str): user = { user1: user = {
    email = "someone@example.com",
    username = "someusername123",
    active = true,
    sign_in_count = 1,
} }
```
## Nesting

Records can be nested by creating a record type using other record types as the type for the fields of record. Nesting one record within another can be a useful way to model more complex structures: 
```
var empl1: employee = {
    FirstName = "Mark",
    LastName =  "Jones",
    Email =     "mark@gmail.com",
    Age =       25,
    MonthlySalary = {
        Basic = 15000.00,
        Bonus = {
            HTA =    2100.00,
            RA =   5000.00,
        },
    },
}
```

## Defauling

Records can have default values in their fields too. 
```
typ user: rec = {
    var username: str;
    var email: str;
    var sign_in_count: int[64] = 1;
    var active: bol = true;
};
```

This makes possible to enforce some fields (empty ones), and leave the defaults untouched: 
```
@var[mut] user1: user = { email = "someone@example.com", username = "someusername123" }

```
## Limiting

We can also restrict the values (with ranges) assigned to each field:
```
typ rgb: rec[] = {
    var r: int[8][.range(255)];
    var g: int[8][.range(255)];
    var b: int[8][.range(255)];
}

var mint: rgb = { 153, 255, 187 }
```

This of course can be achieve just with variable types and aliased types and sets too, but we would need to create two types:
```
typ rgb: set[int[8][.range(255)], int[8][.range(255)], int[8][.range(255)]];

var mint: rgb = { 153, 255, 187 }
```
## Methods

A record may have receiver-qualified routines associated with it. This does not
turn the record into an object model. It only means a routine may use dot-call
syntax when its first explicit input is a value of that record type. To create
such a routine for a record, declare the receiver type on the routine itself:
```
fun (recieverRecord)someFunction(): str = { self.somestring; };
```

After declaring the record receiver, the routine body may refer to that input
through `self`. A receiver is simply the explicit first input that enables
dot-call syntax. The data and the routine remain separate declarations.
```
typ user: rec = {
    var username: str;
    var email: str;
    var sign_in_count: int[64];
    var active: bol;
};

fun (user)getName(): str = { result = self.username; };
```

Receiver-qualified routines have one main ergonomic benefit over plain routine
calls: they let the call site read `value.method(...)`. In the same package,
multiple routines may still share the same method name if the receiver types
are different.

Each record value can therefore use the dot form, but the underlying model
remains procedural:

```fol
user1.getName()
```

should be read as:

```fol
getName(user1)
```

There is no hidden record-owned method table in that spelling.

The routine itself is still declared separately:
```
var[mut] user1: user = { email = "someone@example.com", username = "someusername123", active = true, sign_in_count = 1 }

.echo(user1.getName());
```

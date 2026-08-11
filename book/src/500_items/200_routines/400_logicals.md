# Logicals

Logicals, which are logic routines, and represent logic programming, state the routine as a set of logical relations (e.g., a grandparent is the parent of a parent of someone). Such rutines are similar to the database languages. A program is executed by an “inference engine” that answers a query by searching these relations systematically to make inferences that will answer a query. 

{{% notice info %}}

One of the main goals of the development of symbolic logic hasbeen to capture the notion of logical consequence with formal, mechanical, means. If the conditions for a certain class of problems can be formalized within a suitable logic as a set of premises, and if a problem to be solved can bestated as a sentence in the logic, then a solution might be found by constructing a formal proof of the problem statement from the premises

{{% /notice %}}

## Declaration

In FOL, logic programming is considered as a first class citzen with axioms (`axi`) as facts and logicals (`log`) as rules, thus resembling Prolog language. For example:

### Facts
Declaring a list of facts (axioms)
```
var likes: axi[str, str] = { {"bob","alice"} , {"alice","bob"}, {"dan","sally"} };
```

### Rules
Declaring a rule that states if A likes B and B likes A, they are dating
```
log dating(a, b: str): bol = {
    likes:[a,b] and
    likes:[b,a]
}
```
Declaring a rule that states if A likes B and B likes A, they are just friends
```
log frends(a, b): bol = {
    likes:[a,b] or
    likes:[b,a]
}

```
{{% notice warn %}}

Rules can have **only** facts and varibles within 

{{% /notice %}}


## Return
A logical `log` can return different values, but they are either of type `bol`, or of type container (axioms `axi` or vectors `vec`):

Lets define a axiom of parents and childrens called `parents` and another one of parents that can dance called `dances`:
```
var parent: axi[str, str] = { {"albert","bob"}, 
                              {"albert","betty"},
                              {"albert","bill"},
                              {"alice","bob"},
                              {"alice","betty"},
                              {"alice","bill"},
                              {"bob","carl"},
                              {"bob","tom"} };
var dances axi[str] = { "albert", "alice", "carl" };
```
### Boolean

Here we return a boolean `bol`. This rule check if a parent can dance:
```
log can_parent_dance(a: str): bol = {
    parent:[a,_] and dances:[a]
}

can_parent_dance("albert")          // return true, "albert" is both a parent and can dance
can_parent_dance("bob")             // return false, "bob" is a parent but can't dance
can_parent_dance("carl")            // return false, "carl" is not a parent
```
Lets examine this: 
`parent:[a,_] and dances:[a]`
this is a combintion of two facts. Here we say if `a` is parent of anyone (we dont care whose, that's why we use meh symbol `[a,_]`) and if true, then we check if parent `a` (since he is a parent now, we fact-checked) can dance. 

### Vector
The same, we can create a vector of elements. For example, if we want to get the list of parents that dance:
```
log all_parents_that_dance(): vec[str] = {
    parent:[*->X,_] and
    dances:[X->Y]
    Y
}

all_parents_that_dance()            // this will return a string vector {"albert", "alice"}
```
Now lets analyze the body of the rule:
```
parent:[*->X,_] and
dances:[X->Y]
Y
```
Here are a combination of facts and variable assignment through [**silents**](/docs/700_sugar/silents/). Silents are a single letter identifiers. If a silent constant is not declared, it gets declared and assigned **in-place**.

Taking a look each line:
`parent:[X,_] and`
this gets all parents (`[*->X,_]`),and assign them to **silent** `X`. So, `X` is a list of all parents. 
then:
`dances[X->Y]:` 
this takes the list of parents `X` and checks each if they can dance, and filter it by assigning it to `Y` so `[X->Y]` it will have only the parents that can dance.
then:
`Y`
this just returns the list `Y` of parents that can dance.

## Relationship
If `A` is `object` and `objects` can be destroyed, then `A` can be destroyed. As a result axioms can be related or conditioned to other axioms too, much like facts. 

For example: if `carl` is the son of `bob` and `bob` is the son of `albert` then `carl` must be the grandson of `albert`: 
```
log grandparent(a: str): vec[str] = {
    parent[*->X,a]: and 
    parent[*->Y,X]:
    Y
}
```
Or: if `bob` is the son of `albert` and `betty` is the doughter of `albert`, then `bob` and `betty` must be syblings:
```
log are_syblings(a, b: str): vec[str] = {
    parent[*->X,a]: and
    parent[X->Y,b]:
    Y
}
```
Same with uncle relationship:
```
var brothers: axi[str] = { {"bob":"bill"}, {"bill","bob"} };

log has_uncle(a: str): vec[str] = {
    parent[*->Y,a]: and
    brothers[Y,*->Z]:;
    Z
}
```

## Conditional facts

Here an example, the axioms `hates` will add a memeber `romeo` only if the relation `x` is satisfied:
```
var stabs: axi = {{"tybalt","mercutio","sword"}}
var hates: axi;

log romeHates(X: str): bol = {
    stabs[X,"mercutio",_]:
}

hates+["romeo",X] if (romeHates(X));
```
### Anonymous logicals
Conditional facts can be added with the help of anonymous logicals/rules:
```
eats+[x,"cheesburger"] if (eats[x,"bread"] and eats[X,"cheese"]);

eats+[x:"cheesburger"] if (log (a: str): bol = {
    eats[a,"bread"]: and
    eats[a,"cheese"]:
}(x));

```

## Nested facts
```
var line: axi = { {{4,5},{4,8}}, {{8,5},{4,5}} }

log vertical(line: axi): bol = {
    line[*->A,*->B]: and 
    A[*->X,Y*->]: and
    B[X,*->Y2]:
}

log horizontal(line: axi): bol = {
    line[*->A,*->B]: and 
    A[*->X,*->Y]: and
    B[*->X2,Y]:
}

assert vertical(line.at(0));
assert horizontal(line.at(1));
```

## Filtering
Another example of filtering a more complex axion:
```
var class: axi;

class.add({"cs340","spring",{"tue","thur"},{12,13},"john","coor_5"})
class.add({"cs340",winter,{"wed","fri"},{15,16},"bruce","coor_3"})

log instructor(class: str): vec[str] = {
    class[class,_,[_,"fri"],_,*->X,_]
    X
}
```

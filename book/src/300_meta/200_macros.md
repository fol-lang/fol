# Macros

Current boundary:

- the `def`-based meta family (macros, alternatives, defaults, templates) is
  planned for a future release; none of it is part of the current compiler
  surface
- everything below describes intended design, not current behavior

Are a very complicated system, and yet can be used as simply as in-place replacement. A lot of build-in macros exist in the language to make the code more easy to type. Below are some system defined macros. 

For example, wherever `$` is before `any` variable name, its replaced with `.to_string`. Or wherever `!` is before `bol` name, its replaced with `.not` but when the same `!` is placed before `ptr` it is replaced with `.delete_pointer`.

```
def '$'(a: any): mac = '.to_string'
def '!'(a: bol): mac = '.not '
def '!'(a: ptr): mac = '.delete_pointer';
def '*'(a: ptr): mac = '.pointer_value';
def '#'(a: any): mac = '.borrow_from';
def '&'(a: any): mac = '.address_of';
```

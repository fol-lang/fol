# Declarations

Each `.fol` file inside a folder is part of the same package.

The important part is that files are **connected, but not merged**:

- every physical `.fol` file is still its own source file
- files in the same folder share one package scope
- declarations are order independent across those files
- a declaration started in one file can **not** continue into the next file

There is no need to import sibling files from the same package. They already belong to the same package scope.

## Package, Namespace, And File Scope

It helps to think about source layout in three layers:

- **package scope**: all `.fol` files directly inside the package folder
- **namespace scope**: declarations inside subfolders of that package
- **file scope**: declarations marked `hid`, which stay visible only inside their own file

In short:

- same folder = same package
- subfolder = nested namespace
- `hid` = file only
- files are never imported directly as standalone modules

### Package scope

Files that live directly in the package root share one package scope:

```
root/
    math/
        add.fol
        sub.fol
```

Both `add.fol` and `sub.fol` belong to package `math`, so declarations from one file may be used by the other without importing the sibling file.

### Namespace scope

Subfolders do not create a new package by themselves. They create nested namespaces inside the same package:

```
root/
    math/
        add.fol
        stats/
            mean.fol
```

Here:

- `add.fol` is in package namespace `math`
- `mean.fol` is in package namespace `math::stats`

Code may reach namespace members either by:

- direct `use`
- or qualified access with `::`

### File scope

Sometimes a declaration should stay inside one file even though the package is shared.
That is what `hid` is for.

```
// file1.fol
var[hid] cache_key: str = "local"
```

```
// file2.fol
pro[] main: int = {
    .echo(cache_key)      // error: hidden declarations are file-only
}
```

## Namespaces

A namespace can be defined in a subfolder of the main folder, and namespaces can be nested.

To acces the namespace there are two ways:
- direct import with `use`
- or code access with `::`

### Direct import

```
use aNS: loc = {""home/folder/printing/logg""}

pro[] main: int = {
    logg.warn("something")
}

```
### Code access

```
use aNS: loc = {""home/folder/printing""}

pro[] main: int = {
    printing::logg.warn("something")
}

```

## Mental model

For source layout, the mental model is:

- one folder root gives one package
- each file in that folder is a real source file in that package
- subfolders extend the namespace path
- `use` imports packages or namespaces
- `hid` keeps a declaration private to one file
- `loc` imports a local directory tree without a package control file
- `pkg` imports a formal external package defined by `build.fol`
- bundled std is declared as an internal dependency and imported through `pkg`

This means FOL is **not** "one file = one module".
The package is the folder; the file is a source unit inside that package.

## Package Roots

When a directory is treated as a package root, the exact contract depends on the import kind:

- `loc`: plain local directory import, no package metadata required
- `pkg`: installed external package import with explicit root files

For `pkg`, the root is not just "a folder containing `.fol` files".
It is a formal package root with:

- `build.fol` as the package control file

This keeps the language model clean:

- source files `use` other namespaces/packages
- package build execution starts from `pro[] build(): non` in `build.fol`
- package metadata and direct dependencies are configured through `.build()` inside `build.fol`
- package loading happens before ordinary name resolution

`build.fol` itself is still ordinary FOL syntax.
It is not a separate mini-language.
The package layer simply gives package/build meaning to the canonical build
routine there.

## Blocks

Current boundary:

- the namespace, package-scope, and import material above is the current
  compiler surface
- the block forms in this section (`_{}`, `.{}`, named `blk[]` blocks, and
  unconditional jump) are not implemented; they are later design work, not part
  of the current compiler surface

Block statement is used for scopes where members get destroyed when scope is finished. And there are two ways to define a block: 
- unnamed blocks and 
- named blocks

### Unnamed blocks
Are simply scopes, that may or may not return value, and are represented as: `{ //block }`, with `.` before the brackets for return types and `_` for non return types:
```
pro[] main: int = {
    _{
        .echo("simple type block")
    }
    .echo(.{ return "return type block" })
}
```

### Named blocks
Blocks can be used as labels too, when we want to unconditionally jump to a specific part of the code.
```
pro[] main: int = {
    def block: blk[] = {            // $block A named block that can be referenced
        // implementation
    }
    def mark: blk[]                 // $mark A named block that can be referenced, usually for "jump" statements
}
```

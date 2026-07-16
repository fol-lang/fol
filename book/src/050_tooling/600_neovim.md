# Neovim Integration

Neovim integration has two separate pieces:

- Tree-sitter for syntax/highlighting/queries
- LSP for diagnostics and semantic editor features

They should be configured together, but they do different jobs.

## LSP Setup

The language server command is:

```lua
vim.lsp.config("fol", {
  cmd = { "fol", "tool", "lsp" },
  filetypes = { "fol" },
  root_markers = { "fol.work.yaml", "build.fol", ".git" },
})

vim.lsp.enable("fol")
```

Also ensure Neovim recognizes `.fol` files:

```lua
vim.filetype.add({
  extension = { fol = "fol" },
})
```

## Tree-sitter Setup

First generate a bundle:

```text
fol tool tree generate /tmp/fol
```

Then point Neovim’s Tree-sitter parser config at that bundle:

```lua
local parser_config = require("nvim-treesitter.parsers").get_parser_configs()

parser_config.fol = {
  install_info = {
    url = "/tmp/fol",
    files = { "src/parser.c" },
    requires_generate_from_grammar = false,
  },
  filetype = "fol",
}
```

The query files are expected at:

- `/tmp/fol/queries/fol/highlights.scm`
- `/tmp/fol/queries/fol/locals.scm`
- `/tmp/fol/queries/fol/symbols.scm`

Neovim also needs that bundle on `runtimepath` so it can find the queries.

## Practical Model

Tree-sitter provides:

- highlight captures
- locals captures
- symbol-style structure queries

LSP provides:

- diagnostics
- hover
- definitions
- references
- rename for same-file local and current-package top-level symbols
- semantic tokens
- document symbols

So the normal editor shape is:

1. Neovim opens a `.fol` file
2. Tree-sitter handles syntax/highlighting
3. Neovim launches `fol tool lsp`
4. the server provides semantic editor features

## Debugging A Setup

Useful checks:

```vim
:echo &filetype
:lua print(vim.inspect(vim.lsp.get_clients({ bufnr = 0 })))
```

And from the shell:

```text
fol tool tree generate /tmp/fol
fol tool lsp
```

If `fol tool lsp` prints nothing and waits, that is correct.

It is a stdio server, not an interactive shell command.

If the server refuses to start, check that Neovim opened the file inside a
directory tree that contains `build.fol` or `fol.work.yaml`.

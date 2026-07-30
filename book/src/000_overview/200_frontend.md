# Frontend Workflow

A frontend layer sits above the compiler pipeline. It is implemented in
`fol-frontend` and shipped as the `folc` engine, which the `fol` manager
dispatches to (see [Toolchain Management](../025_toolchain/_index.md)).

The frontend owns:

- workflow commands: project/workspace setup, fetch and update flows,
  build/run/test/emit
- direct single-file compile dispatch
- root help and output rendering
- frontend diagnostics
- editor tooling dispatch under `fol tool`

Use this page as an entrypoint pointer; the detailed reference lives in the
Tooling section:

- [Tooling](../050_tooling/_index.md)
- [Frontend Workflow](../050_tooling/100_frontend.md)
- [Tool Commands](../050_tooling/200_tool_commands.md)

## Current Boundary

The current frontend milestone is about local workflows and the first backend.

It already covers:

- project and workspace scaffolding
- documented `build.fol` package entry files for new projects
- root discovery
- package preparation through `fol-package`
- git-backed dependency fetching and materialization
- `fol.lock` writing, locked fetches, offline warm-cache fetches, and update flows
- workspace dependency/status reporting
- build/run/test orchestration for shipped `V1`, the bounded `V2` subset, and
  the shipped `V3` surfaces
- routed workspace `build/run/test/check` entry through `build.fol`
- emitted Rust and lowered IR output
- editor-tooling entrypoints for parse, highlight, symbols, and LSP startup
- shell completions
- safe cleanup of build/cache/git/package-store roots
- frontend-owned direct compile routing

Future work is still expected around:

- richer package-store policy beyond the current read/write chains
- lockfile/version solving beyond the current pinned git contract
- additional backend targets

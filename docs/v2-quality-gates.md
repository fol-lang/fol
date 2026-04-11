# V2 Quality Gates

This file turns the plan's cross-cutting `V2` gates into tracked repository
contract.

## Compiler Pipeline Gate

Every executable `V2` language slice must land with:

- parser coverage in `test/parser`
- resolver coverage in `test/resolver`
- typecheck coverage in `test/typecheck`
- lowering coverage in `lang/compiler/fol-lower`
- backend or emitted-Rust coverage in `test/integration_tests`
- compile-and-run app or example coverage in `test/apps` or `test/integration_tests`

The current canonical roots for the shipped `V2` matrix are:

- `test/parser/test_parser_parts/v2_generics_m1.rs`
- `test/parser/test_parser_parts/implementation_declarations.rs`
- `test/resolver/test_resolver_parts/generic_routines.rs`
- `test/typecheck/test_typecheck_generics_m1.rs`
- `test/typecheck/test_typecheck_standards_m2.rs`
- `lang/compiler/fol-lower/src/decls/tests.rs`
- `test/integration_tests/integration_editor_and_build.rs`
- `test/apps/test_apps.rs`

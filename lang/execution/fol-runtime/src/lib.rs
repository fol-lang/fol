//! Runtime support foundations for executable FOL `V1` programs.
//!
//! `fol-runtime` is the support crate that future generated programs will link
//! against. It is not a front-end phase and it is not the backend itself.
//!
//! The intended compiler split is:
//!
//! - `fol-runtime` owns runtime data/layout/helper semantics
//! - `fol-backend` will later own code generation
//!
//! Current `V1` runtime scope:
//!
//! - builtin scalar support
//! - memo-tier strings
//! - memo-tier heap containers and container helpers
//! - optional/error shells
//! - recoverable routine results
//! - backend-facing runtime hooks such as `.echo(...)`
//!
//! The runtime model is converging on explicit internal tiers:
//!
//! - [`core`]
//! - [`memo`]
//! - [`std`]
//!
//! The heap-backed runtime families now belong to [`memo`], while [`containers`]
//! remains the helper layer for indexing, slicing, and rendering.
//!
//! Explicitly out of scope for this runtime crate:
//!
//! - ownership / borrowing / pointers
//! - a runtime-owned generic reification model
//! - object-style standards/dispatch machinery
//! - concurrency runtime
//! - C ABI
//!
//! Full `V2` generics, generic types, and procedural standards still execute
//! through the current backend strategy. The important boundary is narrower:
//! `fol-runtime` itself does not define a second witness/dictionary system or
//! an object-style runtime contract for standards. Those semantics stay in the
//! compiler/lowering/backend pipeline under monomorphization.
//!
//! # Backend Mapping: Builtins
//!
//! The first backend should map lowered builtins using this rule:
//!
//! - prefer native Rust operators or expressions for pure scalar operations
//! - use `fol-runtime` helpers for runtime-sensitive or policy-sensitive behavior
//!
//! Current `V1` expectation:
//!
//! - `.eq`, `.nq`, `.lt`, `.gt`, `.ge`, `.le`
//!   - lower to native Rust comparisons on already-lowered scalar values
//! - `.not`
//!   - lower to native Rust boolean negation
//! - `.len`
//!   - lower through the active model module's `len(...)`
//! - `.echo`
//!   - lower through [`std::echo`]
//! - `check`
//!   - lower through the active model module's `check_recoverable(...)`
//! - recoverable top-level result handling
//!   - lower through [`std::outcome_from_recoverable`]
//!
//! The backend should not reimplement `.len` or `.echo` inline. Those are part
//! of the runtime contract so later backends can share the same behavior.
//!
//! # Backend Mapping: Lowered Instructions
//!
//! The current lowered `V1` IR mixes instructions that can become plain Rust
//! syntax with instructions that require stable `fol-runtime` support.
//!
//! Native-emission friendly instructions:
//!
//! - `Const`
//! - `LoadLocal`
//! - `StoreLocal`
//! - `Call`
//! - `IntrinsicCall` for scalar comparisons and boolean negation
//! - `FieldAccess` for backend-authored record layouts
//! - `Cast` once the backend implements the admitted `V1` cast policy
//! - control terminators such as `Jump`, `Branch`, and `Return`
//!
//! Runtime-backed instructions or lowered surfaces:
//!
//! - `LengthOf`
//!   - must call the active model module's `len(...)`
//! - `RuntimeHook`
//!   - currently `.echo(...)`, which must call [`std::echo`]
//! - `CheckRecoverable`
//!   - must inspect [`abi::FolRecover`] through the active model module
//! - `UnwrapRecoverable`
//!   - must unwrap the success lane of [`abi::FolRecover`]
//! - `ExtractRecoverableError`
//!   - must extract the error lane of [`abi::FolRecover`]
//! - `ConstructLinear`
//!   - sequence and vector lowering must map to [`memo::FolSeq`] and
//!     [`memo::FolVec`]
//! - `ConstructSet`
//!   - must map to [`memo::FolSet`] to preserve deterministic ordering
//! - `ConstructMap`
//!   - must map to [`memo::FolMap`] to preserve deterministic ordering
//! - `ConstructOptional`
//!   - must map to [`shell::FolOption`]
//! - `ConstructError`
//!   - must map to [`shell::FolError`]
//! - `IndexAccess`
//!   - for runtime containers, must use the runtime indexing contract
//! - `UnwrapShell`
//!   - must follow the runtime shell boundary rather than routine-recoverable
//!     semantics
//! - `Report`
//!   - must produce the error lane of [`abi::FolRecover`]
//! - `Panic`
//!   - must route through the backend's panic strategy while preserving the
//!     runtime printable-message contract
//!
//! Backend-authored records and entries may compile into plain Rust structs and
//! enums, but their public formatting behavior should still follow
//! [`aggregate::FolRecord`], [`aggregate::render_record`], and
//! [`aggregate::FolEntry`] so generated `.echo(...)` output stays stable.
//!
//! # Backend Mapping: Generated Crate Names And Imports
//!
//! The first backend should generate one temporary Rust crate per lowered FOL
//! workspace.
//!
//! Import expectations for that generated crate:
//!
//! - declare a dependency on the package named `fol-runtime`
//! - import runtime items through `fol_runtime`, matching Rust's crate-name
//!   hyphen-to-underscore rule
//! - prefer one stable model alias per emitted module, such as
//!   `use fol_runtime::core as rt;`
//!   - or `memo` / `std` depending on the artifact's `fol_model`
//! - use fully qualified imports for less-common runtime modules when needed,
//!   for example:
//!   - `fol_runtime::memo::FolSeq`
//!   - `fol_runtime::shell::FolOption`
//!   - `fol_runtime::abi::FolRecover`
//!
//! Generated Rust should not guess alternate runtime package names and should
//! not inline shadow copies of runtime types into emitted modules.
//!
//! Namespace/layout expectations:
//!
//! - group emitted Rust by lowered package and namespace, not by original `.fol`
//!   file count
//! - backend-generated local helper names may be mangled, but runtime imports
//!   should stay readable and stable
//! - `fol-runtime` remains the single support dependency for current `V1`
//!   semantics; generated crates should not split the runtime contract across
//!   multiple ad hoc support crates
//!
//! # Backend Integration Guide
//!
//! A first Rust backend should integrate with `fol-runtime` in this order:
//!
//! 1. Lower a full workspace through `fol-lower` and treat that lowered
//!    workspace as the only backend input.
//! 2. Create one generated Rust crate for that lowered workspace.
//! 3. Add `fol-runtime` as a dependency and import the artifact's model module
//!    as `rt` in each emitted module.
//! 4. Emit backend-authored Rust structs and enums for lowered records and
//!    entries, then implement [`aggregate::FolRecord`] or [`aggregate::FolEntry`]
//!    where runtime formatting needs to stay stable.
//! 5. Map lowered container, shell, and recoverable shapes onto the public
//!    runtime types:
//!    - [`memo::FolVec`]
//!    - [`memo::FolSeq`]
//!    - [`memo::FolSet`]
//!    - [`memo::FolMap`]
//!    - [`shell::FolOption`]
//!    - [`shell::FolError`]
//!    - [`abi::FolRecover`]
//! 6. Lower builtin/runtime-sensitive operations through the model helpers
//!    instead of inlining policy:
//!    - `rt::len(...)`
//!    - `rt::echo(...)`
//!    - `rt::check_recoverable(...)`
//!    - `rt::outcome_from_recoverable(...)`
//! 7. Keep pure scalar comparison and boolean negation native in the emitted
//!    Rust where possible.
//! 8. Lower top-level entry routines so recoverable outcomes become
//!    [`std::FolProcessOutcome`] values with the documented exit-code policy.
//! 9. Only after emitted Rust typechecks against `fol-runtime` should the
//!    backend invoke `cargo build` or `rustc`.
//!
//! Current `V1` backends should treat `fol-runtime` as stable support code, not
//! as an optimizer target. If a future backend wants to inline or replace a
//! runtime helper, it should first preserve the same public behavior and only
//! then optimize behind that contract.

pub mod abi;
pub mod memo;
pub mod aggregate;
pub mod builtins;
pub mod containers;
pub mod core;
pub mod error;
pub mod shell;
pub mod std;
pub mod value;

pub const CRATE_NAME: &str = "fol-runtime";

pub fn crate_name() -> &'static str {
    CRATE_NAME
}

pub use error::{RuntimeError, RuntimeErrorKind};

#[cfg(test)]
mod tests {
    use super::*;

    const CORE_SOURCE: &str = include_str!("core.rs");
    const MEMO_SOURCE: &str = include_str!("memo.rs");
    const STD_SOURCE: &str = include_str!("std.rs");

    #[test]
    fn crate_name_matches_expected_runtime_identity() {
        assert_eq!(crate_name(), "fol-runtime");
    }

    #[test]
    fn public_runtime_module_shell_is_importable() {
        assert_eq!(memo::module_name(), "memo");
        assert_eq!(abi::module_name(), "abi");
        assert_eq!(aggregate::module_name(), "aggregate");
        assert_eq!(builtins::module_name(), "builtins");
        assert_eq!(containers::module_name(), "containers");
        assert_eq!(core::module_name(), "core");
        assert_eq!(error::module_name(), "error");
        assert_eq!(shell::module_name(), "shell");
        assert_eq!(std::module_name(), "std");
        assert_eq!(value::module_name(), "value");
    }

    #[test]
    fn runtime_model_modules_expose_expected_capabilities() {
        assert_eq!(core::tier_name(), "core");
        assert!(!core::HAS_HEAP);
        assert!(!core::HAS_OS);

        assert_eq!(memo::tier_name(), "memo");
        assert!(memo::HAS_HEAP);
        assert!(!memo::HAS_OS);

        assert_eq!(std::tier_name(), "std");
        assert!(std::HAS_HEAP);
        assert!(std::HAS_OS);
    }

    #[test]
    fn runtime_model_module_exports_stay_intentionally_distinct() {
        assert!(!CORE_SOURCE.contains("FolStr("));
        assert!(!CORE_SOURCE.contains("FolVec("));
        assert!(!CORE_SOURCE.contains("FolSeq("));
        assert!(!CORE_SOURCE.contains("pub fn echo"));
        assert!(!CORE_SOURCE.contains("pub use crate::memo::"));
        assert!(!CORE_SOURCE.contains("FolProcessOutcome"));

        assert!(MEMO_SOURCE.contains("pub struct FolStr"));
        assert!(MEMO_SOURCE.contains("pub struct FolVec"));
        assert!(MEMO_SOURCE.contains("pub struct FolSeq"));
        assert!(!MEMO_SOURCE.contains("pub fn echo"));
        assert!(!MEMO_SOURCE.contains("FolProcessOutcome"));

        assert!(STD_SOURCE.contains("pub fn echo"));
        assert!(STD_SOURCE.contains("FolProcessOutcome"));
        assert!(STD_SOURCE.contains("pub use crate::memo::{FolMap, FolSeq, FolSet, FolStr, FolVec};"));
    }

    #[test]
    fn runtime_model_flags_and_helpers_freeze_backend_contract() {
        assert_eq!(core::capabilities().name, "core");
        assert_eq!(memo::capabilities().name, "memo");
        assert_eq!(std::capabilities().name, "std");

        assert!(!core::capabilities().has_heap);
        assert!(!core::capabilities().has_os);
        assert!(memo::capabilities().has_heap);
        assert!(!memo::capabilities().has_os);
        assert!(std::capabilities().has_heap);
        assert!(std::capabilities().has_os);

        let core_len: fn(&[i64; 2]) -> i64 = core::len::<[i64; 2]>;
        let memo_len: fn(&memo::FolStr) -> i64 = memo::len::<memo::FolStr>;
        let std_echo: fn(memo::FolStr) -> memo::FolStr = std::echo::<memo::FolStr>;

        assert_eq!(core_len(&[1, 2]), 2);
        assert_eq!(memo_len(&memo::FolStr::from("Ada")), 3);
        assert_eq!(std_echo(memo::FolStr::from("ok")), memo::FolStr::from("ok"));
    }

    /// Split runtime source into whole Rust identifiers so contract checks
    /// match names exactly instead of matching accidental substrings. This is
    /// what keeps the hosted `echo` service distinct from the pure
    /// `render_echo` / `FolEchoFormat` formatting surface that `core`
    /// legitimately re-exports as part of its records/entries display contract.
    fn source_defines_or_reexports_identifier(source: &str, identifier: &str) -> bool {
        source
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .any(|token| token == identifier)
    }

    #[test]
    fn core_source_stays_free_of_accidental_heap_or_hosted_reexports() {
        // These are whole-identifier names, not substrings: the heap container
        // types, the hosted `echo` service, and the hosted process outcome. The
        // pure formatting helpers `render_echo` and `FolEchoFormat` are separate
        // identifiers and remain a legitimate part of the `core` tier surface.
        for forbidden in [
            "FolStr",
            "FolVec",
            "FolSeq",
            "FolSet",
            "FolMap",
            "echo",
            "FolProcessOutcome",
        ] {
            assert!(
                !source_defines_or_reexports_identifier(CORE_SOURCE, forbidden),
                "core runtime tier should not expose '{forbidden}' through source reexports"
            );
        }
    }

    #[test]
    fn runtime_errors_can_be_constructed_with_stable_kinds() {
        let error = RuntimeError::new(
            RuntimeErrorKind::InvariantViolation,
            "runtime invariant failed",
        );

        assert_eq!(error.kind(), RuntimeErrorKind::InvariantViolation);
        assert_eq!(error.message(), "runtime invariant failed");
    }

    #[test]
    fn public_recoverable_abi_freezes_success_path_through_model_modules() {
        let value = memo::FolRecover::<memo::FolInt, memo::FolStr>::ok(7);

        assert!(!memo::check_recoverable(&value));
        assert!(memo::recoverable_succeeded(&value));
        assert_eq!(value.value_ref(), Some(&7));
        assert_eq!(Result::<memo::FolInt, memo::FolStr>::from(value), Ok(7));
    }

    #[test]
    fn public_recoverable_abi_freezes_failure_path_through_model_modules() {
        let value = memo::FolRecover::<memo::FolInt, memo::FolStr>::err(
            memo::FolStr::from("bad-input"),
        );

        assert!(memo::check_recoverable(&value));
        assert!(!memo::recoverable_succeeded(&value));
        assert_eq!(
            value.error_ref().map(|error| error.as_str()),
            Some("bad-input")
        );
        assert_eq!(Result::<memo::FolInt, memo::FolStr>::from(value), Err(memo::FolStr::from("bad-input")));
    }

    #[test]
    fn public_shell_values_stay_distinct_from_recoverable_results() {
        let optional = memo::FolOption::some(7);
        let error_shell = memo::FolError::new(memo::FolStr::from("broken"));
        let recoverable =
            memo::FolRecover::<memo::FolInt, memo::FolStr>::err(memo::FolStr::from("broken"));

        assert_eq!(
            ::std::any::type_name::<memo::FolOption<memo::FolInt>>(),
            "fol_runtime::shell::FolOption<i64>"
        );
        assert_eq!(
            ::std::any::type_name::<memo::FolError<memo::FolStr>>(),
            "fol_runtime::shell::FolError<fol_runtime::memo::FolStr>"
        );
        assert_eq!(
            ::std::any::type_name::<memo::FolRecover<memo::FolInt, memo::FolStr>>(),
            "fol_runtime::abi::FolRecover<i64, fol_runtime::memo::FolStr>"
        );

        assert_eq!(memo::unwrap_optional_shell(optional), Ok(7));
        assert_eq!(memo::unwrap_error_shell(error_shell), memo::FolStr::from("broken"));
        assert!(memo::check_recoverable(&recoverable));
    }
}

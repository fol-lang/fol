//! Extended, human-readable explanations for diagnostic codes.
//!
//! This module is the single source of truth for two things the CLI and other
//! tooling reuse:
//!
//! - [`family_for_code`]: the plain-language family (`"TYPES"`, `"NAMES"`, ...)
//!   and one-line "what this means" hint inferred from a code's letter prefix.
//!   The pretty diagnostic renderer reuses this so the family chip and the
//!   `fol code explain <CODE>` output never drift apart.
//! - [`explanation`]: a longer explanation (title + body) for the codes the
//!   compiler actually emits.
//!
//! Honesty rule: only codes that are actually constructed somewhere in the
//! compiler/runtime crates get an entry here. Every registered code is grounded
//! in a real error-construction site. The completeness test in this module
//! keeps the letter prefixes honest, and the frontend/integration tests keep
//! the shipped surface in sync.

/// Plain-language family label and a one-line hint for a diagnostic code,
/// inferred from the code's letter prefix.
///
/// The prefixes map to the compiler stage that owns them:
///
/// - `P` — parser (syntax)
/// - `R` — resolver (names/imports)
/// - `T` — typechecker (types/capabilities)
/// - `O` — ownership and borrowing
/// - `L` — lowering (typed program -> IR)
/// - `K` — package / build-graph model
/// - `F` — frontend / build configuration
/// - `B` — backend (code generation)
pub fn family_for_code(code: &str) -> (&'static str, &'static str) {
    match code.as_bytes().first().map(|b| b.to_ascii_uppercase()) {
        Some(b'P') => (
            "PARSER",
            "a syntax slip — the code is shaped in a way FOL cannot parse",
        ),
        Some(b'R') => (
            "NAMES",
            "a name or import problem — a symbol could not be resolved",
        ),
        Some(b'T') => (
            "TYPES",
            "a type or capability mismatch — the values do not fit the expected types",
        ),
        Some(b'O') => (
            "OWNERSHIP",
            "an ownership or borrowing violation — a value is not accessible in this state",
        ),
        Some(b'L') => (
            "LOWERING",
            "a lowering problem — a construct could not be turned into runnable IR",
        ),
        Some(b'K') => (
            "PACKAGE",
            "a package or build-graph problem in the package/build model",
        ),
        Some(b'F') => (
            "BUILD",
            "a build or configuration problem in the frontend/package graph",
        ),
        Some(b'B') => (
            "BACKEND",
            "a code-generation problem while emitting the program",
        ),
        _ => ("ERROR", "a problem the compiler could not accept"),
    }
}

/// A longer, human-readable explanation for a diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Explanation {
    /// The canonical (uppercase) code, e.g. `"T1003"`.
    pub code: &'static str,
    /// A short one-line title naming the problem.
    pub title: &'static str,
    /// The multi-line body: what it means, why it happens, and how to fix it.
    pub body: &'static str,
}

/// Look up the extended explanation for a diagnostic code.
///
/// The lookup is case-insensitive, so `explanation("t1003")` and
/// `explanation("T1003")` return the same entry. Returns `None` for codes that
/// are not registered (either not a real emitted code, or one without an
/// extended explanation yet).
pub fn explanation(code: &str) -> Option<&'static Explanation> {
    let normalized = code.trim().to_ascii_uppercase();
    REGISTRY.iter().find(|entry| entry.code == normalized)
}

/// Every code that has a registered explanation, in registry order.
pub fn registered_codes() -> impl Iterator<Item = &'static str> {
    REGISTRY.iter().map(|entry| entry.code)
}

macro_rules! explanation {
    ($code:literal, $title:literal, $body:literal) => {
        Explanation {
            code: $code,
            title: $title,
            body: $body,
        }
    };
}

/// The explanation registry. Each entry is grounded in a real error site:
///
/// - `P*`  — `fol-parser` (`ParseErrorKind`)
/// - `R*`  — `fol-resolver` (`ResolverErrorKind`)
/// - `T*`  — `fol-typecheck` (`TypecheckErrorKind`)
/// - `O*`  — `fol-typecheck` ownership and lexical-borrow checks
/// - `L*`  — `fol-lower` (`LoweringErrorKind`)
/// - `K10*` — `fol-package` (`PackageErrorKind`)
/// - `K11*` — `fol-build` (`BuildEvaluationErrorKind`)
/// - `F*`  — `fol-frontend` (`FrontendErrorKind`)
static REGISTRY: &[Explanation] = &[
    explanation!(
        "O1001",
        "ownership violation",
        "A value was used after its ownership moved, or while ownership rules made it inaccessible.\n\n\
         V3 clone-safe values clone when transferred. Heap-owned values, unique pointers, and\n\
         aggregates containing unique ownership move, including when a value-producing expression\n\
         is evaluated and its result is discarded.\n\n\
         This code also covers ownership boundaries that cannot be represented safely yet:\n\
         - dereferencing a unique pointer clones a clone-safe pointee, but transfers a move-only\n\
           pointee and consumes that pointer; shared and borrowed pointer dereferences are\n\
           read-only and therefore require clone-safe pointees\n\
         - dereferencing a unique pointer reached through a field would partially observe a\n\
           move-only place, so it requires place-aware projection IR\n\
         - a moved whole binding may be reinitialized in ordinary control flow, but not inside\n\
           `dfr` or `edf`, where the assignment is delayed until scope exit\n\
         - borrowed values, `ptr[shared, T]` (`Rc`), and unresolved generic values cannot cross\n\
           a spawn or async OS-thread boundary\n\
         - channel endpoint acquisition cannot be delayed inside `dfr` or `edf`, and the receiver\n\
           lifecycle must remain attached to its direct owning binding\n\
         - eventuals are move-only: assignment transfers them, await consumes them once, and V3\n\
           does not carry them through composites or generic parameters\n\n\
         Use the value before moving it, borrow it when borrowing is appropriate, or assign a\n\
         fresh value to a mutable whole binding in ordinary control flow. Restructure task,\n\
         channel, eventual, or deferred code so ownership crosses only a supported boundary."
    ),
    explanation!(
        "O2001",
        "owner accessed while borrowed",
        "A lexical borrow is still active, so the owner is inaccessible. Let the borrow scope end or return the borrower early with `!binding`."
    ),
    explanation!(
        "O2002",
        "conflicting borrow",
        "A mutable borrow is exclusive. Return the active borrower or enter a later lexical scope before borrowing the same owner again."
    ),
    explanation!(
        "O2003",
        "mutable borrow of immutable owner",
        "A mutable borrow requires an owner declared with `var[mut]` and a borrower declared with `var[mut, bor]`."
    ),
    explanation!(
        "O2004",
        "returned borrow reused",
        "The borrow was returned with `!binding`; the borrower is no longer accessible after give-back."
    ),
    explanation!(
        "O2005",
        "uninitialized ownership binding",
        "A borrowed or heap-allocating declaration must identify its source at the declaration. `var[bor] view: T;` needs an owner to loan from and `@var owned: T;` needs a value to move or clone into the allocation. Only a plain `var[mut] slot: T;` may be declared uninitialized, and its first use is then guarded by definite initialization."
    ),
    // ── Parser (fol-parser :: ParseErrorKind) ──────────────────────────
    explanation!(
        "P1001",
        "syntax error",
        "FOL could not parse the code around the reported span. The tokens are\n\
         shaped in a way the grammar does not accept (this also covers lexer-level\n\
         rejections, which are reported as syntax errors).\n\n\
         Why it happens:\n\
         - a delimiter is missing or unbalanced (`;`, `}`, `)`, `]`)\n\
         - a keyword or operator is used where an expression was expected\n\
         - an expression or statement was left incomplete\n\n\
         How to fix:\n\
         - look just before the caret; a closing/opening delimiter is often missing\n\
         - compare the construct against a working example of the same shape"
    ),
    explanation!(
        "P1002",
        "invalid top-level item",
        "The top level of a file contains something FOL does not accept there.\n\
         Only items (routines, types, definitions, and imports) may appear at\n\
         file scope; statements and executable calls may not.\n\n\
         Why it happens:\n\
         - an executable call was written at file root (e.g. `run(1, 2);`)\n\
         - a bare statement or expression was placed outside any routine\n\n\
         How to fix:\n\
         - move the code inside a routine body\n\
         - keep the file top level to declarations and imports"
    ),
    explanation!(
        "P1003",
        "construct in the wrong context",
        "A construct is syntactically valid elsewhere but not in the context\n\
         where it was written.\n\n\
         Why it happens:\n\
         - a declaration appears in an expression position\n\
         - a form is nested somewhere its enclosing block does not allow\n\n\
         How to fix:\n\
         - check what the surrounding block permits\n\
         - relocate the construct to a context that accepts it"
    ),
    explanation!(
        "P1004",
        "malformed literal",
        "A literal value could not be parsed.\n\n\
         Why it happens:\n\
         - a numeric literal is malformed or out of the accepted form\n\
         - a string/char literal is unterminated or badly escaped\n\n\
         How to fix:\n\
         - correct the literal's spelling and delimiters\n\
         - ensure strings are closed and escapes are valid"
    ),
    explanation!(
        "P1005",
        "unsupported syntax",
        "The syntax is recognized by the grammar but is not supported by the\n\
         current language milestone.\n\n\
         Why it happens:\n\
         - a form exists in the design but is not implemented yet\n\n\
         How to fix:\n\
         - use a currently supported construct instead\n\
         - check the book for the current supported surface"
    ),
    // ── Resolver (fol-resolver :: ResolverErrorKind) ───────────────────
    explanation!(
        "R1001",
        "invalid resolver input",
        "The resolver received input it cannot accept. This is most often an\n\
         import-target problem discovered while binding names.\n\n\
         Why it happens:\n\
         - a `pkg` import target or declared bundled-std package does not exist on disk\n\
         - a formal package root was imported with `loc` instead of `pkg`\n\
         - a required std or package-store root was not provided\n\n\
         How to fix:\n\
         - check the import source and target path\n\
         - import formal packages with `pkg`, plain folders with `loc`\n\
         - declare bundled std through `build.add_dep(...)`; use `--std-root <DIR>` only\n\
           for an explicit development override\n\
         - pass `--package-store-root <DIR>` when required, or run `fol pack fetch`\n\
           to materialize declared external dependencies"
    ),
    explanation!(
        "R1002",
        "unsupported import or construct",
        "The resolver hit an import source kind or construct it does not support.\n\n\
         Why it happens:\n\
         - an import uses a source kind other than `loc` or `pkg`\n\n\
         How to fix:\n\
         - use one of the supported import source kinds: `loc`, `pkg`\n\
         - reach bundled std through a declared internal dependency and a quoted `pkg` import"
    ),
    explanation!(
        "R1003",
        "unresolved name",
        "A name was used but never declared or imported in a reachable scope.\n\n\
         Why it happens:\n\
         - the name is misspelled\n\
         - the declaration lives in another scope or module and was not imported\n\
         - the symbol is not exported by the module it comes from\n\n\
         How to fix:\n\
         - fix the spelling, or declare the binding before using it\n\
         - import the symbol (and make sure it is exported)"
    ),
    explanation!(
        "R1004",
        "duplicate symbol",
        "The same name is declared more than once in a scope where names must be\n\
         unique.\n\n\
         Why it happens:\n\
         - two declarations share a name in the same scope or module\n\n\
         How to fix:\n\
         - rename one of the declarations, or remove the redundant one\n\
         - the secondary label points at the first declaration"
    ),
    explanation!(
        "R1005",
        "ambiguous reference",
        "A name matches more than one visible declaration, so the resolver cannot\n\
         pick a single meaning.\n\n\
         Why it happens:\n\
         - two imported modules both export the same name\n\n\
         How to fix:\n\
         - qualify the reference to select the intended declaration\n\
         - or rename/remove one of the conflicting imports"
    ),
    explanation!(
        "R1006",
        "import cycle",
        "Imports form a cycle: modules depend on each other in a loop that cannot\n\
         be ordered.\n\n\
         Why it happens:\n\
         - module A imports B which (directly or transitively) imports A\n\n\
         How to fix:\n\
         - break the cycle by moving shared declarations into a third module\n\
         - restructure dependencies so imports form a tree"
    ),
    explanation!(
        "R1099",
        "internal resolver error",
        "The resolver hit an internal invariant it did not expect. This indicates\n\
         a compiler bug rather than a problem in your code.\n\n\
         How to fix:\n\
         - please report it with a minimal program that reproduces the error"
    ),
    // ── Typecheck (fol-typecheck :: TypecheckErrorKind) ────────────────
    explanation!(
        "T1001",
        "invalid typecheck input",
        "The typechecker received input it cannot accept while checking an\n\
         expression or declaration.\n\n\
         Why it happens:\n\
         - an expression is malformed for the position it appears in\n\
         - an operand or argument shape does not match what the checker expects\n\
         - `report` appears inside `dfr` or `edf`, where deferred replay cannot initiate a\n\
           recoverable error exit\n\
         - a `/ ErrorType` call or awaited result is discarded instead of handled with\n\
           `check(...)` or `||`\n\
         - a recoverable eventual is discarded, left live at lexical fallthrough, `break`,\n\
           `return`, or `report`, or overwritten before its error is handled\n\
         - a blocking `select {}` has no channel arm and no default arm\n\n\
         How to fix:\n\
         - re-read the reported span and adjust the expression to a valid form\n\
         - move `report` into ordinary control flow and keep deferred bodies non-terminating\n\
         - bind recoverable async work; transferring it carries the obligation to the destination\n\
         - await it exactly once and handle the result immediately with `check(...)` or `||`\n\
           before final discard, overwrite, lexical fallthrough, `break`, `return`, or `report`\n\
         - add at least one `when channel as binding` arm, or add a default `*` arm, to `select`"
    ),
    explanation!(
        "T1002",
        "unsupported construct or capability",
        "A construct is not supported under the current capability model, or is\n\
         outside an implemented language boundary.\n\n\
         Why it happens:\n\
         - a heap-backed value (`str`, `vec[...]`, `seq[...]`, `set[...]`,\n\
           `map[...]`) is used under `fol_model = core`\n\
         - `.echo(...)` or any processor surface (spawn, channels, `select`, `mux[T]`,\n\
           async, or await) is used without the bundled hosted `std` dependency\n\
         - spawn or async is given a stored routine value or routine parameter instead of a\n\
           direct named routine call, or a bare spawn could discard a recoverable error\n\
         - an inner routine implicitly captures an outer local instead of receiving it through\n\
           an explicit parameter or supported explicit capture surface\n\
         - `edf` tries to access or await eventual state even though error-only cleanup does not\n\
           run on successful exits and therefore cannot discharge a normal-path obligation\n\
         - a channel is placed in an unsupported composite/projected/top-level shape instead\n\
           of a direct routine-owned binding with one receiver lifecycle\n\
         - a `dfr` or `edf` body accesses a mutex field, calls `.lock()` / `.unlock()`, or\n\
           forwards the handle to another `mux[T]` routine; V3 guard effects are immediate\n\
         - a feature is outside the current release boundary (for example raw\n\
           pointers or explicit deallocation at the V4/FFI boundary)\n\n\
         How to fix:\n\
         - move to `fol_model = memo` for heap-backed values; for hosted facilities and\n\
           the processor surface, also declare bundled `std` on that memo artifact\n\
         - call a named routine directly for spawn/async, keep channels in direct routine-local\n\
           bindings, pass outer values to nested routines explicitly, and perform mutex guard or\n\
           eventual work in ordinary control flow; explicit channel sender-endpoint captures may\n\
           instead use the zero-parameter anonymous spawn form\n\
         - otherwise use a construct inside the currently shipped V1/V2/V3 boundary"
    ),
    explanation!(
        "T1003",
        "incompatible types",
        "Two types were required to match but do not.\n\n\
         Why it happens:\n\
         - a returned value does not match the routine's declared return type\n\
         - an argument does not match the parameter type\n\
         - branches of an expression produce different types\n\
         - a binding's value does not match its declared type\n\n\
         How to fix:\n\
         - make the value's type match what is expected at that position\n\
         - adjust the declared type, or convert the value explicitly"
    ),
    explanation!(
        "T1004",
        "scope resolution failed",
        "The typechecker could not resolve a scope it needed while checking an\n\
         expression.\n\n\
         Why it happens:\n\
         - a reference points into a scope that could not be reconstructed\n\n\
         How to fix:\n\
         - ensure the referenced binding is declared and in scope\n\
         - fix any earlier resolver errors first, since they can cascade here"
    ),
    explanation!(
        "T1005",
        "type import failed",
        "A type referenced across a module boundary could not be imported or\n\
         loaded during typechecking.\n\n\
         Why it happens:\n\
         - the type's module is missing, not exported, or failed to load\n\n\
         How to fix:\n\
         - make sure the type is exported and its module resolves\n\
         - fix earlier import/resolver errors, then re-check"
    ),
    explanation!(
        "T1006",
        "symbol table corrupted",
        "The typechecker found the symbol table in an inconsistent state. This is\n\
         an internal invariant failure, not a problem in your code.\n\n\
         How to fix:\n\
         - please report it with a minimal reproduction"
    ),
    explanation!(
        "T1007",
        "unsupported syntax in typecheck",
        "The typechecker reached a syntax form it does not handle.\n\n\
         Why it happens:\n\
         - a syntactic form is accepted by the parser but not yet supported by\n\
           type checking\n\n\
         How to fix:\n\
         - use a currently supported construct\n\
         - check the book for the current supported surface"
    ),
    explanation!(
        "T1099",
        "internal typecheck error",
        "The typechecker hit an internal invariant it did not expect. This\n\
         indicates a compiler bug rather than a problem in your code.\n\n\
         How to fix:\n\
         - please report it with a minimal program that reproduces the error"
    ),
    // ── Lowering (fol-lower :: LoweringErrorKind) ──────────────────────
    explanation!(
        "L1001",
        "unsupported lowering",
        "A construct passed earlier stages but cannot be lowered to backend IR in\n\
         the current milestone.\n\n\
         Why it happens:\n\
         - a feature is accepted by parsing/typecheck but lowering for it does not\n\
           exist yet\n\n\
         How to fix:\n\
         - use a construct that is supported end-to-end\n\
         - check the book for the current executable surface"
    ),
    explanation!(
        "L1002",
        "invalid lowering input",
        "Lowering received typed input that was incomplete or inconsistent.\n\n\
         Why it happens:\n\
         - a typed node was missing information lowering needs\n\
         - usually a symptom of an earlier-stage inconsistency\n\n\
         How to fix:\n\
         - fix any earlier parser/resolver/typecheck errors first\n\
         - if none exist, please report it with a minimal reproduction"
    ),
    explanation!(
        "L1099",
        "internal lowering error",
        "Lowering hit an internal invariant it did not expect. This indicates a\n\
         compiler bug rather than a problem in your code.\n\n\
         How to fix:\n\
         - please report it with a minimal program that reproduces the error"
    ),
    // ── Package (fol-package :: PackageErrorKind) ──────────────────────
    explanation!(
        "K1001",
        "invalid package input",
        "The package layer rejected package metadata or an import target while\n\
         loading the package graph.\n\n\
         Why it happens:\n\
         - a package metadata field is invalid or declared more than once\n\
         - a formal package root was imported with `loc` instead of `pkg`\n\n\
         How to fix:\n\
         - correct the package metadata in `build.fol`\n\
         - import formal package roots with `pkg`"
    ),
    explanation!(
        "K1002",
        "unsupported package operation",
        "The package layer hit an operation or import source kind it does not\n\
         support.\n\n\
         Why it happens:\n\
         - an import uses an unsupported source kind\n\
         - a package operation is not implemented\n\n\
         How to fix:\n\
         - use a supported import source kind and package operation"
    ),
    explanation!(
        "K1003",
        "package import cycle",
        "Packages depend on each other in a cycle that cannot be ordered.\n\n\
         Why it happens:\n\
         - package A depends on B which (directly or transitively) depends on A\n\n\
         How to fix:\n\
         - break the dependency cycle by restructuring the packages"
    ),
    explanation!(
        "K1099",
        "internal package error",
        "The package layer hit an internal invariant it did not expect. This\n\
         indicates a compiler bug rather than a problem in your code.\n\n\
         How to fix:\n\
         - please report it with a minimal reproduction"
    ),
    // ── Build evaluation (fol-build :: BuildEvaluationErrorKind) ────────
    explanation!(
        "K1101",
        "invalid build input",
        "Evaluating `build.fol` received invalid input.\n\n\
         Why it happens:\n\
         - a build API was called with arguments it cannot accept\n\
         - a build graph value is malformed\n\n\
         How to fix:\n\
         - re-read the reported `build.fol` span and correct the call\n\
         - compare against a working `build.fol` for the same target kind"
    ),
    explanation!(
        "K1102",
        "forbidden build-time capability",
        "`build.fol` evaluation tried an operation that build-time code is not\n\
         allowed to perform. Build evaluation is sandboxed and deterministic.\n\n\
         Forbidden operations include:\n\
         - arbitrary filesystem reads or writes\n\
         - arbitrary network access\n\
         - wall-clock access\n\
         - ambient environment access outside declared inputs\n\
         - uncontrolled process execution\n\n\
         How to fix:\n\
         - remove the forbidden operation from `build.fol`\n\
         - declare the inputs you need explicitly instead of reaching outside"
    ),
    explanation!(
        "K1103",
        "build validation failed",
        "A `build.fol` evaluation validation check failed.\n\n\
         Why it happens:\n\
         - the build graph violated a required invariant (e.g. determinism or a\n\
           structural constraint)\n\n\
         How to fix:\n\
         - re-read the reported message; it names the failed constraint\n\
         - adjust the build graph so the check passes"
    ),
    explanation!(
        "K1199",
        "internal build error",
        "`build.fol` evaluation hit an internal invariant it did not expect. This\n\
         indicates a compiler bug rather than a problem in your code.\n\n\
         How to fix:\n\
         - please report it with a minimal reproduction"
    ),
    // ── Frontend (fol-frontend :: FrontendErrorKind) ───────────────────
    explanation!(
        "F1001",
        "invalid frontend input",
        "The CLI received invalid input or arguments.\n\n\
         Why it happens:\n\
         - a command was given arguments it does not accept\n\
         - no command was provided where one was required\n\n\
         How to fix:\n\
         - run `fol --help`, or `fol <group> <command> --help`, to check usage"
    ),
    explanation!(
        "F1002",
        "workspace not found",
        "No FOL package or workspace root was found for the command.\n\n\
         Why it happens:\n\
         - the command was run outside a package/workspace directory\n\n\
         How to fix:\n\
         - run the command inside a package or workspace root\n\
         - or create one with `fol work init --bin` / `fol work init --workspace`"
    ),
    explanation!(
        "F1003",
        "package operation failed",
        "A package operation (such as fetching or loading dependencies) failed.\n\n\
         Why it happens:\n\
         - a declared dependency could not be materialized or resolved\n\n\
         How to fix:\n\
         - run `fol pack fetch` to materialize declared dependencies\n\
         - check the dependency source and network/offline settings"
    ),
    explanation!(
        "F1004",
        "command failed",
        "A frontend command failed to complete.\n\n\
         Why it happens:\n\
         - compilation reported errors (see the diagnostics above this summary)\n\
         - an underlying step or subprocess returned a failure\n\n\
         How to fix:\n\
         - fix the reported diagnostics, then re-run the command"
    ),
    explanation!(
        "F1099",
        "internal frontend error",
        "The frontend hit an internal invariant it did not expect. This indicates\n\
         a tooling bug rather than a problem in your code.\n\n\
         How to fix:\n\
         - please report it with the command you ran"
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_code_returns_explanation_text() {
        let explanation = explanation("T1003").expect("T1003 should be registered");
        assert_eq!(explanation.code, "T1003");
        assert_eq!(explanation.title, "incompatible types");
        assert!(explanation.body.contains("do not"));
    }

    #[test]
    fn ownership_explanation_covers_v3_resource_boundaries() {
        let explanation = explanation("O1001").expect("O1001 should be registered");
        assert!(explanation.body.contains("result is discarded"));
        assert!(explanation.body.contains("clone-safe values clone"));
        assert!(explanation
            .body
            .contains("aggregates containing unique ownership move"));
        assert!(explanation.body.contains("reinitialize"));
        assert!(explanation.body.contains("place-aware projection IR"));
        assert!(explanation.body.contains("transfers a move-only"));
        assert!(explanation.body.contains("shared and borrowed pointer"));
        assert!(explanation
            .body
            .contains("spawn or async OS-thread boundary"));
        assert!(explanation.body.contains("channel endpoint acquisition"));
        assert!(explanation.body.contains("eventuals are move-only"));
        assert!(explanation.body.contains("`dfr` or `edf`"));
    }

    #[test]
    fn invalid_input_explanation_covers_empty_blocking_select() {
        let explanation = explanation("T1001").expect("T1001 should be registered");
        assert!(explanation.body.contains("blocking `select {}`"));
        assert!(explanation
            .body
            .contains("at least one `when channel as binding` arm"));
    }

    #[test]
    fn invalid_input_explanation_covers_deferred_report_boundary() {
        let explanation = explanation("T1001").expect("T1001 should be registered");
        assert!(explanation
            .body
            .contains("`report` appears inside `dfr` or `edf`"));
        assert!(explanation
            .body
            .contains("keep deferred bodies non-terminating"));
    }

    #[test]
    fn invalid_input_explanation_covers_recoverable_eventual_obligations() {
        let explanation = explanation("T1001").expect("T1001 should be registered");
        assert!(explanation
            .body
            .contains("recoverable eventual is discarded"));
        assert!(explanation
            .body
            .contains("left live at lexical fallthrough"));
        assert!(explanation.body.contains("`break`"));
        assert!(explanation.body.contains("`return`"));
        assert!(explanation.body.contains("`report`"));
        assert!(explanation.body.contains("overwritten"));
        assert!(explanation
            .body
            .contains("transferring it carries the obligation"));
        assert!(explanation.body.contains("await it exactly once"));
        assert!(explanation.body.contains("`check(...)` or `||`"));
    }

    #[test]
    fn unsupported_explanation_covers_v3_processor_boundaries() {
        let explanation = explanation("T1002").expect("T1002 should be registered");
        assert!(explanation.body.contains("any processor surface"));
        assert!(explanation.body.contains("direct named routine call"));
        assert!(explanation.body.contains("anonymous spawn form"));
        assert!(explanation.body.contains("one receiver lifecycle"));
        assert!(explanation.body.contains("forwards the handle"));
        assert!(explanation
            .body
            .contains("implicitly captures an outer local"));
        assert!(explanation
            .body
            .contains("cannot discharge a normal-path obligation"));
        assert!(explanation.body.contains("V4/FFI boundary"));
    }

    #[test]
    fn import_explanations_use_only_current_public_source_kinds() {
        let missing = explanation("R1001").expect("R1001 should be registered");
        assert!(missing.body.contains("declared bundled-std package"));
        assert!(!missing.body.contains("`std`/`pkg`"));

        let unsupported = explanation("R1002").expect("R1002 should be registered");
        assert!(unsupported.body.contains("other than `loc` or `pkg`"));
        assert!(unsupported.body.contains("quoted `pkg` import"));
        assert!(!unsupported.body.contains("`loc`, `std`"));
    }

    #[test]
    fn lookup_is_case_insensitive() {
        let upper = explanation("T1003").expect("T1003 should resolve");
        let lower = explanation("t1003").expect("t1003 should resolve");
        assert_eq!(upper, lower);
        assert_eq!(lower.code, "T1003");
    }

    #[test]
    fn lookup_tolerates_surrounding_whitespace() {
        assert_eq!(explanation("  t1003 ").map(|e| e.code), Some("T1003"));
    }

    #[test]
    fn unknown_code_returns_none() {
        assert!(explanation("Z9999").is_none());
        assert!(explanation("").is_none());
        assert!(explanation("EUNKNOWN").is_none());
    }

    #[test]
    fn family_labels_follow_the_code_prefix_case_insensitively() {
        assert_eq!(family_for_code("P1001").0, "PARSER");
        assert_eq!(family_for_code("R1003").0, "NAMES");
        assert_eq!(family_for_code("T1003").0, "TYPES");
        assert_eq!(family_for_code("O1001").0, "OWNERSHIP");
        assert_eq!(family_for_code("L1001").0, "LOWERING");
        assert_eq!(family_for_code("K1001").0, "PACKAGE");
        assert_eq!(family_for_code("K1101").0, "PACKAGE");
        assert_eq!(family_for_code("F1002").0, "BUILD");
        assert_eq!(family_for_code("B1000").0, "BACKEND");
        assert_eq!(family_for_code("t1003").0, "TYPES");
        assert_eq!(family_for_code("Z9999").0, "ERROR");
    }

    #[test]
    fn registered_codes_are_uppercase_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for code in registered_codes() {
            assert_eq!(code, code.to_ascii_uppercase(), "code must be uppercase");
            assert!(seen.insert(code), "duplicate registered code: {code}");
        }
        assert!(
            seen.len() >= 30,
            "registry should cover the real code surface"
        );
    }

    #[test]
    fn every_registered_code_has_a_recognized_family_prefix() {
        // Guards against registering a code whose prefix falls through to the
        // generic ERROR family, which would mean the letter is not a real stage.
        for code in registered_codes() {
            let (family, _) = family_for_code(code);
            assert_ne!(
                family, "ERROR",
                "registered code {code} has no recognized family prefix"
            );
        }
    }
}

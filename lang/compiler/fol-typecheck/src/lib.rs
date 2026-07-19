//! Whole-workspace type checking for the shipped FOL V1, V2, and V3 language
//! surfaces.
//!
//! This crate owns semantic types, capability-model legality, ownership and
//! processor checks, typed results, and compiler diagnostics.

pub mod builtins;
mod channel_analysis;
pub mod config;
pub mod decls;
pub mod editor;
pub mod errors;
pub mod exprs;
pub mod model;
pub mod session;
pub mod types;

pub use builtins::BuiltinTypeIds;
pub use config::{TypecheckCapabilityModel, TypecheckConfig};
pub use editor::{
    editor_builtin_type_names, editor_container_type_names, editor_declaration_keywords,
    editor_implemented_intrinsics, editor_intrinsic_available_in_model, editor_model_capability,
    editor_processor_keyword_available_in_model, editor_processor_keyword_infos,
    editor_shell_type_names, editor_source_kind_names, editor_structured_type_infos,
    editor_type_family_available_in_model, EditorIntrinsicInfo, EditorModelCapability,
    EditorProcessorKeywordContext, EditorProcessorKeywordInfo, EditorStructuredTypeInfo,
    EditorTypeFamily,
};
pub use errors::{TypecheckError, TypecheckErrorKind};
pub use fol_parser::ast::ParsedSourceUnitKind;
pub use model::{
    ActiveMutexGuard, RecordFieldLayout, RecoverableCallEffect, TypedConformance,
    TypedConformanceClaim, TypedExportMount, TypedNode, TypedPackage, TypedProgram, TypedReference,
    TypedSourceUnit, TypedStandard, TypedStandardField, TypedStandardRoutine, TypedSymbol,
    TypedWorkspace,
};
pub use types::{
    BuiltinType, CheckedType, CheckedTypeId, DeclaredTypeKind, GenericConstraint, RoutineType,
    TypeTable,
};

pub type TypecheckResult<T> = Result<T, Vec<TypecheckError>>;

#[derive(Debug, Default)]
pub struct Typechecker {
    config: TypecheckConfig,
}

impl Typechecker {
    pub fn new() -> Self {
        Self::with_config(TypecheckConfig::default())
    }

    pub fn with_config(config: TypecheckConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> TypecheckConfig {
        self.config
    }

    pub fn check_resolved_program(
        &mut self,
        resolved: fol_resolver::ResolvedProgram,
    ) -> TypecheckResult<TypedProgram> {
        session::TypecheckSession::with_config(self.config).check_resolved_program(resolved)
    }

    pub fn check_resolved_workspace(
        &mut self,
        resolved: fol_resolver::ResolvedWorkspace,
    ) -> TypecheckResult<TypedWorkspace> {
        session::TypecheckSession::with_config(self.config).check_resolved_workspace(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedSourceUnitKind, TypecheckCapabilityModel, TypecheckConfig, TypecheckError,
        TypecheckErrorKind, Typechecker,
    };
    use fol_parser::ast::AstParser;
    use fol_parser::ast::SyntaxOrigin;
    use fol_resolver::resolve_package;
    use fol_stream::FileStream;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_typecheck_fixture(contents: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fol_typecheck_lib_{stamp}"));
        fs::create_dir_all(&dir).expect("should create typecheck fixture dir");
        let path = dir.join("main.fol");
        fs::write(&path, contents).expect("should write typecheck fixture");
        path
    }

    fn typecheck_fixture_errors(contents: &str) -> Vec<TypecheckError> {
        let path = write_typecheck_fixture(contents);
        let mut stream =
            FileStream::from_file(path.to_str().expect("utf8 temp path")).expect("open fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("fixture should parse");
        let resolved = resolve_package(syntax).expect("fixture should resolve");
        Typechecker::new()
            .check_resolved_program(resolved)
            .expect_err("fixture should be rejected")
    }

    fn typecheck_fixture_ok(contents: &str) {
        let path = write_typecheck_fixture(contents);
        let mut stream =
            FileStream::from_file(path.to_str().expect("utf8 temp path")).expect("open fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("fixture should parse");
        let resolved = resolve_package(syntax).expect("fixture should resolve");
        Typechecker::new()
            .check_resolved_program(resolved)
            .expect("fixture should typecheck");
    }

    #[test]
    fn shell_choice_when_on_binds_the_optional_payload() {
        // A `when` over an `opt[T]` scrutinee binds the present payload in the
        // `on(value)` branch and takes `*` on nil (V3_MEM §3.3 safe inner access).
        typecheck_fixture_ok(
            "fun[] first(): opt[int] = { return 7; };\n\
             fun[] main(): int = {\n\
                 var slot: opt[int] = first();\n\
                 when(slot) {\n\
                     on(value) { return value; }\n\
                     * { return 0; }\n\
                 }\n\
             };\n",
        );
    }

    #[test]
    fn shell_choice_when_on_requires_a_shell_scrutinee() {
        // `on` is only valid over a nil-able shell; a plain scrutinee is rejected.
        let errors = typecheck_fixture_errors(
            "fun[] main(): int = {\n\
                 var n: int = 3;\n\
                 when(n) {\n\
                     on(value) { return value; }\n\
                     * { return 0; }\n\
                 }\n\
             };\n",
        );
        assert!(errors.iter().any(|error| error
            .message()
            .contains("requires an 'opt[T]' or 'err[T]' scrutinee")));
    }

    #[test]
    fn shell_choice_when_on_binds_the_error_payload() {
        // `err[T]` is a nil-able shell: `on(code)` binds the stored error
        // payload and `*` takes the nil (no-error) branch (V3_MEM §3.3/§8.2).
        typecheck_fixture_ok(
            "fun[] risky(): err[int] = { return nil; };\n\
             fun[] main(): int = {\n\
                 var slot: err[int] = risky();\n\
                 when(slot) {\n\
                     on(code) { return code; }\n\
                     * { return 0; }\n\
                 }\n\
             };\n",
        );
    }

    #[test]
    fn eventual_type_can_be_named_for_a_local_binding() {
        // V3_MEM §8.1: local declarations may elide the lexical lifetime `L`
        // and spell the eventual produced by `| async` as `evt[T]`.
        typecheck_fixture_ok(
            "fun[] compute(seed: int): int = { return seed; };\n\
             fun[] main(): int = {\n\
                 var work: evt[int] = compute(7) | async;\n\
                 var value: int = work | await;\n\
                 return value;\n\
             };\n",
        );
    }

    #[test]
    fn recoverable_eventual_type_can_be_named_with_its_error_channel() {
        // `evt[T / E]` names a recoverable eventual (V3_MEM §8.1).
        typecheck_fixture_ok(
            "fun[] risky(seed: int): int / str = {\n\
                 when(seed) {\n\
                     is(0) => report \"zero\";\n\
                     * => return seed;\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var work: evt[int / str] = risky(7) | async;\n\
                 var value: int = work | await || 0;\n\
                 return value;\n\
             };\n",
        );
    }

    #[test]
    fn a_bare_eventual_type_name_still_needs_a_value_type() {
        let errors = typecheck_fixture_errors(
            "fun[] main(): int = {\n\
                 var work: evt = 0;\n\
                 return 0;\n\
             };\n",
        );
        assert!(errors.iter().any(|error| error
            .message()
            .contains("an eventual type needs a value type")));
    }

    #[test]
    fn capability_standards_are_accepted_in_conformance_lists() {
        // The compiler-owned capability standards do not require a `std`
        // declaration and are recognized directly.
        typecheck_fixture_ok(
            "typ Point()(copy): rec = { x: int, y: int };\n\
             typ Job()(clone, send): rec = { input: int };\n\
             fun[] main(): int = { return 0; };\n",
        );
    }

    #[test]
    fn capability_copy_and_fin_cannot_coexist() {
        let errors = typecheck_fixture_errors("typ Bad()(copy, fin): rec = { value: int };\n");
        assert!(errors.iter().any(|error| error
            .message()
            .contains("cannot claim both 'copy' and 'fin'")));
    }

    #[test]
    fn capability_copy_requires_copy_safe_fields() {
        let errors = typecheck_fixture_errors("typ Bad()(copy): rec = { link: ptr[int] };\n");
        assert!(errors.iter().any(|error| error
            .message()
            .contains("requires every field to be copy-safe")));
    }

    #[test]
    fn returning_a_borrow_of_an_owned_local_is_rejected() {
        let errors = typecheck_fixture_errors(
            "typ Job: rec = { input: int };\n\
             fun dangle(job: Job): Job[bor] = { return [bor]job; };\n",
        );
        assert!(errors.iter().any(|error| error
            .message()
            .contains("would dangle after the routine returns")));
    }

    #[test]
    fn returning_a_borrow_binding_of_a_local_is_rejected_transitively() {
        // The borrow chain roots in the owned local `owner`, so returning the
        // borrow binding `view` still dangles.
        let errors = typecheck_fixture_errors(
            "typ Job: rec = { input: int };\n\
             fun leak(): Job[bor] = {\n\
             var owner: Job = { input = 7 };\n\
             var[bor] view: Job = [bor]owner;\n\
             return view;\n\
             };\n",
        );
        assert!(errors
            .iter()
            .any(|error| error.message().contains("owned local 'owner'")));
    }

    #[test]
    fn observing_a_borrowed_parameter_typechecks() {
        typecheck_fixture_ok(
            "typ Job: rec = { input: int };\n\
             fun size(job[bor]: Job): int = { return job.input; };\n\
             fun[] main(): int = { return 0; };\n",
        );
    }

    #[test]
    fn compiler_owned_generic_constraint_kinds_are_recognized() {
        // `item` (any type) and `lif` (a lifetime parameter) are recognized
        // directly in generic headers, like the capability standards.
        typecheck_fixture_ok(
            "fun hold(L: lif, T: item)(value: T): T = { return value; };\n\
             fun[] main(): int = { var a: int = hold(5); return a; };\n",
        );
    }

    #[test]
    fn typechecker_foundation_can_be_constructed() {
        let _ = Typechecker::new();
        let configured = Typechecker::with_config(TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        });

        assert_eq!(
            configured.config().capability_model,
            TypecheckCapabilityModel::Core
        );
    }

    #[test]
    fn typechecker_foundation_exposes_typecheck_error_surface() {
        let error = TypecheckError::with_origin(
            TypecheckErrorKind::Unsupported,
            "generics are not yet supported",
            SyntaxOrigin {
                file: Some("pkg/main.fol".to_string()),
                line: 3,
                column: 5,
                length: 7,
            },
        );

        assert_eq!(error.kind(), TypecheckErrorKind::Unsupported);
        assert_eq!(
            error
                .diagnostic_location()
                .expect("Typecheck error should expose its syntax origin")
                .line,
            3
        );
    }

    #[test]
    fn typechecker_can_wrap_a_resolved_program_in_a_typed_shell() {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream =
            FileStream::from_file(fixture_path).expect("Should open typecheck fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = fol_parser::ast::AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Typecheck fixture should parse");
        let resolved = resolve_package(syntax).expect("Typecheck fixture should resolve");

        let typed = Typechecker::new()
            .check_resolved_program(resolved)
            .expect("Typed shell should accept resolved programs");

        assert_eq!(typed.package_name(), "parser");
        assert_eq!(typed.type_table().len(), 6);
        assert_eq!(typed.resolved().source_units.len(), 1);
    }

    #[test]
    fn typechecker_reexports_parsed_source_unit_kinds() {
        assert_eq!(
            ParsedSourceUnitKind::Build,
            fol_parser::ast::ParsedSourceUnitKind::Build
        );
    }

    #[test]
    fn when_expressions_require_a_default_branch_before_lowering() {
        let errors = typecheck_fixture_errors(
            "fun[] main(): int = {\n    return when(1) {\n        is 1 -> 1\n    };\n};\n",
        );

        assert!(
            errors.iter().any(|error| error
                .message()
                .contains("when expressions require a default branch")),
            "typecheck should reject missing-default when expressions before lowering: {errors:#?}"
        );
    }
}

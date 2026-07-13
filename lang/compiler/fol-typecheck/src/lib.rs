//! Whole-program type checking for the `V1` FOL language subset.
//!
//! This crate is introduced in stages. The early foundation slices only provide
//! the workspace boundary and a small public API surface so later commits can
//! grow semantic types, typed results, and diagnostics incrementally.

pub mod builtins;
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
    editor_shell_type_names, editor_source_kind_names, editor_type_family_available_in_model,
    EditorIntrinsicInfo, EditorModelCapability, EditorTypeFamily,
};
pub use errors::{TypecheckError, TypecheckErrorKind};
pub use fol_parser::ast::ParsedSourceUnitKind;
pub use model::{
    RecordFieldLayout, RecoverableCallEffect, TypedConformance, TypedConformanceClaim,
    TypedExportMount, TypedNode, TypedPackage, TypedProgram, TypedReference, TypedSourceUnit,
    TypedStandard, TypedStandardField, TypedStandardRoutine, TypedSymbol, TypedWorkspace,
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

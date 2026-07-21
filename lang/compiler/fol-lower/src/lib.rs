//! Lowering from typed FOL workspaces into the backend-oriented IR used by the
//! shipped V1, V2, and V3 language surfaces.

mod boundaries;
pub mod control;
pub mod decls;
mod errors;
pub mod exprs;
pub mod ids;
pub mod model;
mod mono;
pub mod render;
pub mod session;
pub mod types;
mod verify;

pub use boundaries::{v1_lowering_boundaries, UnsupportedLoweringSurface};
pub use control::{
    LoweredBinaryOp, LoweredBlock, LoweredInstr, LoweredInstrKind, LoweredLinearKind, LoweredLocal,
    LoweredOperand, LoweredRoutine, LoweredTerminator, LoweredUnaryOp,
};
pub use errors::{LoweringError, LoweringErrorKind};
pub use ids::{
    IdTable, LoweredBlockId, LoweredGlobalId, LoweredInstrId, LoweredLocalId, LoweredPackageId,
    LoweredRoutineId, LoweredTypeId,
};
pub use model::{
    LoweredConformance, LoweredEntryCandidate, LoweredExportMount, LoweredFieldLayout,
    LoweredGlobal, LoweredGlobalInit, LoweredPackage, LoweredRecoverableAbi, LoweredSourceMap,
    LoweredSourceMapEntry, LoweredSourceSymbol, LoweredSourceUnit, LoweredStandard,
    LoweredStandardRoutine, LoweredSymbolOwnership, LoweredTypeDecl, LoweredTypeDeclKind,
    LoweredVariantLayout, LoweredWorkspace,
};
pub use render::render_lowered_workspace;
pub use session::LoweringSession;
pub use types::{LoweredBuiltinType, LoweredRoutineType, LoweredType, LoweredTypeTable};

pub type LoweringResult<T> = Result<T, Vec<LoweringError>>;

#[derive(Debug, Default)]
pub struct Lowerer;

impl Lowerer {
    pub fn new() -> Self {
        Self
    }

    pub fn lower_typed_workspace(
        &mut self,
        typed: fol_typecheck::TypedWorkspace,
    ) -> LoweringResult<LoweredWorkspace> {
        LoweringSession::new(typed).lower_workspace()
    }
}

#[cfg(test)]
mod tests {
    use super::{LoweredWorkspace, Lowerer, LoweringError, LoweringErrorKind, LoweringResult};
    use fol_parser::ast::AstParser;
    use fol_resolver::resolve_package_workspace;
    use fol_stream::FileStream;
    use fol_typecheck::Typechecker;

    #[test]
    fn lowering_api_exposes_constructor_and_result_alias() {
        let _ = Lowerer::new();

        let result: LoweringResult<()> = Ok(());
        assert!(result.is_ok());
    }

    #[test]
    fn lowering_api_exposes_basic_error_surface() {
        let error = LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            "lowering shell is not implemented yet",
        );
        assert_eq!(error.message(), "lowering shell is not implemented yet");
        assert_eq!(error.to_string(), "lowering shell is not implemented yet");
    }

    #[test]
    fn lowering_smoke_accepts_typed_workspace_inputs() {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream = FileStream::from_file(fixture_path).expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        let lowered = Lowerer::new()
            .lower_typed_workspace(typed)
            .expect("Lowering shell should accept typed workspaces");

        assert_eq!(lowered.package_count(), 1);
    }

    #[test]
    fn lowered_workspace_shell_is_constructible() {
        fn _assert_type_exists(_: &LoweredWorkspace) {}
    }
}

// Proper AST Design for FOL Language
// Based on FOL language specification from the book

pub mod node;
pub mod options;
pub mod syntax;
pub mod types;
pub mod visitor;

pub use node::AstNode;
pub use options::{
    binding_is_mutable, decl_visibility, fun_decl_visibility, type_decl_visibility,
    use_decl_visibility, var_decl_visibility, BinaryOperator, CallSurface, CharEncoding, CommentKind, ContainerType,
    DeclOption, FloatSize, FunOption, IntSize, Literal, StandardKind, TypeOption, UnaryOperator,
    UseOption, UsePathSegment, UsePathSeparator, VarOption,
};
pub use syntax::{
    ParsedDeclScope, ParsedDeclVisibility, ParsedPackage, ParsedSourceUnit, ParsedSourceUnitKind,
    ParsedTopLevel, ParsedTopLevelMeta, SyntaxIndex, SyntaxNodeId, SyntaxOrigin,
};
pub use types::{
    BindingPattern, ChannelEndpoint, EntryVariantMeta, FolType, Generic, InquiryTarget,
    LoopCondition, Parameter, QualifiedPath, RecordFieldMeta, RecordInitField, RollingBinding,
    TypeDefinition, WhenCase,
};
pub use visitor::AstVisitor;

// Re-export modules
pub mod parser;
pub use parser::{AstParser, ParseError};

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn syntax_index_assigns_stable_insertion_order_ids() {
        let mut index = SyntaxIndex::default();
        let first = index.insert(SyntaxOrigin {
            file: Some("alpha.fol".to_string()),
            line: 1,
            column: 1,
            length: 3,
        });
        let second = index.insert(SyntaxOrigin {
            file: Some("beta.fol".to_string()),
            line: 2,
            column: 4,
            length: 2,
        });

        assert_eq!(first, SyntaxNodeId(0));
        assert_eq!(second, SyntaxNodeId(1));
        assert_eq!(
            index.origin(second),
            Some(&SyntaxOrigin {
                file: Some("beta.fol".to_string()),
                line: 2,
                column: 4,
                length: 2,
            })
        );
    }
}

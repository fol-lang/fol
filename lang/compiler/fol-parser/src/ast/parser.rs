// AST Parser Implementation for FOL

use super::{
    AstNode, BinaryOperator, CharEncoding, ContainerType, DeclOption, EntryVariantMeta, FloatSize,
    FolType, FunOption, Generic, InquiryTarget, IntSize, Literal, LoopCondition, Parameter,
    QualifiedPath, RecordFieldMeta, RollingBinding, StandardKind, SyntaxIndex, SyntaxNodeId,
    SyntaxOrigin, TypeDefinition, TypeOption, UnaryOperator, UseOption, VarOption, WhenCase,
};
use fol_diagnostics::{Diagnostic, DiagnosticLocation, ToDiagnostic};
use fol_lexer::token::{BUILDIN, KEYWORD, LITERAL, OPERATOR, SYMBOL, VOID};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub(super) kind: ParseErrorKind,
    message: String,
    file: Option<String>,
    line: usize,
    column: usize,
    length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    Syntax,
    FileRoot,
    Context,
    Literal,
    Unsupported,
}

impl ParseError {
    pub fn from_token(token: &fol_lexer::lexer::stage3::element::Element, message: String) -> Self {
        let loc = token.loc();
        Self {
            kind: ParseErrorKind::Syntax,
            message,
            file: loc.source().map(|src| src.path(true)),
            line: loc.row(),
            column: loc.col(),
            length: loc.len(),
        }
    }

    pub fn from_token_with_kind(
        token: &fol_lexer::lexer::stage3::element::Element,
        kind: ParseErrorKind,
        message: String,
    ) -> Self {
        let loc = token.loc();
        Self {
            kind,
            message,
            file: loc.source().map(|src| src.path(true)),
            line: loc.row(),
            column: loc.col(),
            length: loc.len(),
        }
    }

    pub fn kind(&self) -> ParseErrorKind {
        self.kind
    }

    pub fn diagnostic_code(&self) -> &'static str {
        self.kind.diagnostic_code()
    }

    pub fn file(&self) -> Option<String> {
        self.file.clone()
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn length(&self) -> usize {
        self.length
    }
}

impl ParseErrorKind {
    pub fn diagnostic_code(self) -> &'static str {
        match self {
            Self::Syntax => "P1001",
            Self::FileRoot => "P1002",
            Self::Context => "P1003",
            Self::Literal => "P1004",
            Self::Unsupported => "P1005",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<fol_lexer::LexerError> for ParseError {
    fn from(e: fol_lexer::LexerError) -> Self {
        Self {
            kind: ParseErrorKind::Syntax,
            message: e.to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        }
    }
}

impl ToDiagnostic for ParseError {
    fn to_diagnostic(&self) -> Diagnostic {
        Diagnostic::error(self.diagnostic_code(), self.message.clone()).with_primary_label(
            DiagnosticLocation {
                file: self.file(),
                line: self.line(),
                column: self.column(),
                length: Some(self.length()),
            },
        )
    }
}

pub(super) struct ParseDepthGuard<'a> {
    depth: &'a Cell<usize>,
}

impl Drop for ParseDepthGuard<'_> {
    fn drop(&mut self) {
        let current = self.depth.get();
        debug_assert!(current > 0, "parser context depth underflow");
        self.depth.set(current.saturating_sub(1));
    }
}

/// Simple AST Parser for FOL
pub struct AstParser {
    routine_depth: Cell<usize>,
    loop_depth: Cell<usize>,
    nesting_depth: Cell<usize>,
    syntax_index: RefCell<Option<SyntaxIndex>>,
}

/// Hard ceiling on syntactic nesting (expressions, type references, and
/// block bodies). Recursive descent otherwise follows user nesting one
/// native stack frame chain per level and overflows the stack — a hard
/// SIGABRT, not a diagnostic — at roughly 160 nested parentheses on an
/// 8 MB main-thread stack, and (with large debug-build frames) at about
/// 40 levels on 2 MB worker threads (test harnesses, LSP executors). The
/// ceiling must stay safe on the SMALLEST stack the parser runs on while
/// remaining far deeper than any real program's nesting.
const MAX_NESTING_DEPTH: usize = 64;

impl Default for AstParser {
    fn default() -> Self {
        Self::new()
    }
}

impl AstParser {
    fn enter_depth<'a>(
        &'a self,
        depth: &'a Cell<usize>,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<ParseDepthGuard<'a>, ParseError> {
        let new_depth = depth.get().checked_add(1).ok_or_else(|| {
            if let Ok(token) = tokens.curr(false) {
                ParseError::from_token(
                    &token,
                    "Depth guard overflow; possible infinite recursion in parser".to_string(),
                )
            } else {
                ParseError {
                    kind: ParseErrorKind::Syntax,
                    message: "Depth guard overflow; possible infinite recursion in parser"
                        .to_string(),
                    file: None,
                    line: 0,
                    column: 0,
                    length: 0,
                }
            }
        })?;
        depth.set(new_depth);
        Ok(ParseDepthGuard { depth })
    }

    pub(super) fn enter_routine_context(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<ParseDepthGuard<'_>, ParseError> {
        self.enter_depth(&self.routine_depth, tokens)
    }

    pub(super) fn is_inside_routine(&self) -> bool {
        self.routine_depth.get() > 0
    }

    pub(super) fn enter_loop_context(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<ParseDepthGuard<'_>, ParseError> {
        self.enter_depth(&self.loop_depth, tokens)
    }

    /// Bounded guard for syntactic nesting (expressions, type references,
    /// block bodies). Returns a clean parse error past MAX_NESTING_DEPTH
    /// instead of letting recursive descent overflow the native stack.
    pub(super) fn enter_nesting(
        &self,
        tokens: &fol_lexer::lexer::stage3::Elements,
    ) -> Result<ParseDepthGuard<'_>, ParseError> {
        if self.nesting_depth.get() >= MAX_NESTING_DEPTH {
            return Err(if let Ok(token) = tokens.curr(false) {
                ParseError::from_token(
                    &token,
                    format!("Nesting exceeds the maximum supported depth ({MAX_NESTING_DEPTH})"),
                )
            } else {
                ParseError {
                    kind: ParseErrorKind::Syntax,
                    message: format!(
                        "Nesting exceeds the maximum supported depth ({MAX_NESTING_DEPTH})"
                    ),
                    file: None,
                    line: 0,
                    column: 0,
                    length: 0,
                }
            });
        }
        self.enter_depth(&self.nesting_depth, tokens)
    }

    pub(super) fn is_inside_loop(&self) -> bool {
        self.loop_depth.get() > 0
    }

    pub(super) fn start_syntax_tracking(&self) {
        let previous = self.syntax_index.replace(Some(SyntaxIndex::default()));
        debug_assert!(
            previous.is_none(),
            "parser syntax tracking should not already be active"
        );
    }

    pub(super) fn finish_syntax_tracking(&self) -> SyntaxIndex {
        self.syntax_index
            .borrow_mut()
            .take()
            .expect("syntax tracking must be active when finishing")
    }

    pub(super) fn record_syntax_origin(
        &self,
        token: &fol_lexer::lexer::stage3::element::Element,
    ) -> Option<SyntaxNodeId> {
        self.syntax_index
            .borrow_mut()
            .as_mut()
            .map(|index| index.insert(SyntaxOrigin::from_token(token)))
    }
}

pub(super) fn canonical_identifier_key(name: &str) -> String {
    fol_types::canonical_identifier_key(name)
}

#[cfg(test)]
mod tests {
    use super::{ParseErrorKind, *};
    use crate::ast::ChannelEndpoint;
    use fol_types::canonical_identifier_key;

    fn parse_string(input: &str) -> Result<crate::ParsedPackage, Vec<Diagnostic>> {
        let dir = std::env::temp_dir().join(format!(
            "fol_parser_recovery_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.fol"), input).unwrap();
        let mut stream =
            fol_stream::FileStream::from_file(dir.join("test.fol").to_str().unwrap()).unwrap();
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let result = parser.parse_package(&mut lexer);
        let _ = std::fs::remove_dir_all(&dir);
        result
    }

    #[test]
    fn single_parse_error_does_not_cascade_into_many() {
        // Use square brackets for params (should be parens) to force a parse error.
        let input = concat!(
            "fun[] broken[x: int]: int = {\n",
            "    return 1\n",
            "}\n",
            "\n",
            "fun[] valid(): int = {\n",
            "    return 2\n",
            "}\n",
        );
        let result = parse_string(input);
        match result {
            Ok(_) => panic!("expected parse errors for malformed fun declaration"),
            Err(errors) => {
                assert!(
                    errors.len() <= 3,
                    "should not cascade: got {} errors",
                    errors.len()
                );
            }
        }
    }

    #[test]
    fn two_broken_declarations_produce_bounded_errors() {
        let input = concat!(
            "fun[bad1](): int = { return 1 }\n",
            "fun[bad2](): int = { return 2 }\n",
        );
        let result = parse_string(input);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(
            errors.len() <= 4,
            "two bad decls should produce bounded errors, got {}",
            errors.len()
        );
    }

    #[test]
    fn give_back_prefix_is_preserved_in_routine_bodies() {
        let source = "fun[] main(): int = { var value: int = 1; var[bor] view: int = value; !view; return value; };";
        let package = parse_string(source).expect("give-back fixture should parse");
        let node = &package.source_units[0].items[0].node;
        let AstNode::FunDecl { body, .. } = node else {
            panic!("expected routine declaration");
        };
        assert!(
            body.iter().any(|node| matches!(
                node,
                AstNode::UnaryOp {
                    op: UnaryOperator::GiveBack,
                    ..
                }
            )),
            "parsed body: {body:#?}"
        );
    }

    #[test]
    fn spawn_preserves_the_complete_direct_call_task() {
        let source = concat!(
            "fun[] worker(value: int): int = { return value; };",
            "fun[] main(): int = { [>]worker(7); return 0; };",
        );
        let package = parse_string(source).expect("spawn fixture should parse");
        let AstNode::FunDecl { body, .. } = &package.source_units[0].items[1].node else {
            panic!("expected main routine declaration");
        };
        assert!(
            matches!(
                body.first(),
                Some(AstNode::Spawn {
                    task
                }) if matches!(task.as_ref(), AstNode::FunctionCall { name, args, .. }
                    if name == "worker" && args.len() == 1)
            ),
            "parsed body: {body:#?}"
        );
    }

    #[test]
    fn pipe_send_preserves_the_channel_transmitter_endpoint() {
        let source = "fun[] main(): int = { var channel: chn[int]; 7 | channel[tx]; return 0; };";
        let package = parse_string(source).expect("channel-send fixture should parse");
        let AstNode::FunDecl { body, .. } = &package.source_units[0].items[0].node else {
            panic!("expected main routine declaration");
        };
        assert!(
            body.iter().any(|node| matches!(
                node,
                AstNode::BinaryOp {
                    op: BinaryOperator::Pipe,
                    right,
                    ..
                } if matches!(right.as_ref(), AstNode::ChannelAccess {
                    endpoint: ChannelEndpoint::Tx,
                    ..
                })
            )),
            "parsed body: {body:#?}"
        );
    }

    #[test]
    fn canonical_identifier_key_normalizes_ascii_case_and_underscores() {
        assert_eq!(canonical_identifier_key("Foo_Bar"), "foobar");
        assert_eq!(canonical_identifier_key("foo__bar"), "foobar");
        assert_eq!(canonical_identifier_key("MIXED_Case_Name"), "mixedcasename");
    }

    #[test]
    fn canonical_identifier_key_preserves_non_ascii_while_normalizing_ascii() {
        assert_eq!(canonical_identifier_key("Straße_Name"), "straßename");
        assert_eq!(canonical_identifier_key("Δelta_Name"), "Δeltaname");
    }

    #[test]
    fn parse_error_kind_structural_assignment() {
        let error = ParseError {
            kind: ParseErrorKind::FileRoot,
            message: "Executable calls are not allowed at file root".to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        };
        assert_eq!(error.kind(), ParseErrorKind::FileRoot);
        assert_eq!(error.diagnostic_code(), "P1002");

        let error = ParseError {
            kind: ParseErrorKind::Context,
            message: "break is not allowed outside loops".to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        };
        assert_eq!(error.kind(), ParseErrorKind::Context);
        assert_eq!(error.diagnostic_code(), "P1003");

        let error = ParseError {
            kind: ParseErrorKind::Literal,
            message: "decimal literal is out of range for i64".to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        };
        assert_eq!(error.kind(), ParseErrorKind::Literal);
        assert_eq!(error.diagnostic_code(), "P1004");

        let error = ParseError {
            kind: ParseErrorKind::Unsupported,
            message: "this parser surface is not implemented yet".to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        };
        assert_eq!(error.kind(), ParseErrorKind::Unsupported);
        assert_eq!(error.diagnostic_code(), "P1005");

        let error = ParseError {
            kind: ParseErrorKind::Syntax,
            message: "Expected ')' after tuple element".to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        };
        assert_eq!(error.kind(), ParseErrorKind::Syntax);
        assert_eq!(error.diagnostic_code(), "P1001");
    }

    #[test]
    fn parse_error_from_token_defaults_to_syntax_kind() {
        // Verify that from_token defaults to Syntax kind
        let error = ParseError {
            kind: ParseErrorKind::Syntax,
            message: "any message".to_string(),
            file: None,
            line: 0,
            column: 0,
            length: 0,
        };
        assert_eq!(error.kind(), ParseErrorKind::Syntax);
    }

    #[test]
    fn parse_error_to_diagnostic_preserves_parser_codes() {
        let error = ParseError {
            kind: ParseErrorKind::FileRoot,
            message: "Executable calls are not allowed at file root".to_string(),
            file: Some("pkg/main.fol".to_string()),
            line: 1,
            column: 1,
            length: 3,
        };
        let diagnostic = error.to_diagnostic();

        assert_eq!(diagnostic.code.as_str(), "P1002");
        assert_eq!(
            diagnostic.message,
            "Executable calls are not allowed at file root"
        );
        assert_eq!(
            diagnostic.primary_location(),
            Some(&DiagnosticLocation {
                file: Some("pkg/main.fol".to_string()),
                line: 1,
                column: 1,
                length: Some(3),
            })
        );
    }

    #[test]
    fn parse_error_kind_diagnostic_codes() {
        assert_eq!(ParseErrorKind::Syntax.diagnostic_code(), "P1001");
        assert_eq!(ParseErrorKind::FileRoot.diagnostic_code(), "P1002");
        assert_eq!(ParseErrorKind::Context.diagnostic_code(), "P1003");
        assert_eq!(ParseErrorKind::Literal.diagnostic_code(), "P1004");
        assert_eq!(ParseErrorKind::Unsupported.diagnostic_code(), "P1005");
    }
}

#[path = "parser_parts/access_expression_parsers.rs"]
mod access_expression_parsers;
#[path = "parser_parts/binary_expression_parsers.rs"]
mod binary_expression_parsers;
#[path = "parser_parts/binding_alternative_parsers.rs"]
mod binding_alternative_parsers;
#[path = "parser_parts/binding_declaration_parsers.rs"]
mod binding_declaration_parsers;
#[path = "parser_parts/binding_option_parsers.rs"]
mod binding_option_parsers;
#[path = "parser_parts/binding_value_parsers.rs"]
mod binding_value_parsers;
#[path = "parser_parts/call_expression_parsers.rs"]
mod call_expression_parsers;
#[path = "parser_parts/declaration_option_parsers.rs"]
mod declaration_option_parsers;
#[path = "parser_parts/declaration_parsers.rs"]
mod declaration_parsers;
#[path = "parser_parts/expression_atoms_and_literal_lowering.rs"]
mod expression_atoms_and_literal_lowering;
#[path = "parser_parts/flow_body_parsers.rs"]
mod flow_body_parsers;
#[path = "parser_parts/grouped_binding_parsers.rs"]
mod grouped_binding_parsers;
#[path = "parser_parts/grouped_type_parsers.rs"]
mod grouped_type_parsers;
#[path = "parser_parts/inquiry_clause_parsers.rs"]
mod inquiry_clause_parsers;
#[path = "parser_parts/lookahead_and_assignment_helpers.rs"]
mod lookahead_and_assignment_helpers;
#[path = "parser_parts/match_and_anonymous_parsers.rs"]
mod match_and_anonymous_parsers;
#[path = "parser_parts/pipe_expression_parsers.rs"]
mod pipe_expression_parsers;
#[path = "parser_parts/pipe_lambda_parsers.rs"]
mod pipe_lambda_parsers;
#[path = "parser_parts/postfix_expression_parsers.rs"]
mod postfix_expression_parsers;
#[path = "parser_parts/primary_expression_parsers.rs"]
mod primary_expression_parsers;
#[path = "parser_parts/program_parsing.rs"]
mod program_parsing;
#[path = "parser_parts/rolling_expression_parsers.rs"]
mod rolling_expression_parsers;
#[path = "parser_parts/routine_body_parsers.rs"]
mod routine_body_parsers;
#[path = "parser_parts/routine_capture_parsers.rs"]
mod routine_capture_parsers;
#[path = "parser_parts/routine_declaration_parsers.rs"]
mod routine_declaration_parsers;
#[path = "parser_parts/routine_header_parsers.rs"]
mod routine_header_parsers;
#[path = "parser_parts/segment_declaration_parsers.rs"]
mod segment_declaration_parsers;
#[path = "parser_parts/source_kind_type_parsers.rs"]
mod source_kind_type_parsers;
#[path = "parser_parts/special_type_parsers.rs"]
mod special_type_parsers;
#[path = "parser_parts/standard_declaration_parsers.rs"]
mod standard_declaration_parsers;
#[path = "parser_parts/statement_parsers.rs"]
mod statement_parsers;
#[path = "parser_parts/test_type_parsers.rs"]
mod test_type_parsers;
#[path = "parser_parts/type_definition_parsers.rs"]
mod type_definition_parsers;
#[path = "parser_parts/type_lowering_parsers.rs"]
mod type_lowering_parsers;
#[path = "parser_parts/type_references_and_blocks.rs"]
mod type_references_and_blocks;
#[path = "parser_parts/use_declaration_parsers.rs"]
mod use_declaration_parsers;
#[path = "parser_parts/use_option_parsers.rs"]
mod use_option_parsers;

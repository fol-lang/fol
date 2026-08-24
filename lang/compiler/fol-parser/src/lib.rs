// FOL Parser - Clean AST-only implementation

pub mod ast;

// Re-export the AST parser
pub use ast::*;
use fol_stream::{FileStream, Source};

// Keep the public parser API's concrete diagnostic type; boxing it would be a
// source-breaking signature change for callers.
#[allow(clippy::result_large_err)]
pub fn parse_type_reference_text(
    source: &str,
) -> Result<ast::FolType, fol_diagnostics::Diagnostic> {
    let wrapped = format!("ali Tmp: {source};\n");
    let stream = FileStream::from_preloaded(vec![Source {
        call: "<inline-type>".to_string(),
        path: "<inline-type>".to_string(),
        data: wrapped,
        namespace: "inline".to_string(),
        package: "inline".to_string(),
    }])
    .map_err(|error| fol_diagnostics::Diagnostic::error("P1001", error.to_string()))?;
    let mut stream = stream;
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = ast::AstParser::new();
    let package = parser.parse_package(&mut lexer).map_err(|errors| {
        errors.into_iter().next().unwrap_or_else(|| {
            fol_diagnostics::Diagnostic::error("P1001", "inline type parse failed")
        })
    })?;
    let item = package
        .source_units
        .first()
        .and_then(|unit| unit.items.first())
        .ok_or_else(|| {
            fol_diagnostics::Diagnostic::error(
                "P1001",
                "inline type parse did not produce a declaration",
            )
        })?;
    let ast::AstNode::AliasDecl { target, .. } = &item.node else {
        return Err(fol_diagnostics::Diagnostic::error(
            "P1001",
            "inline type parse did not produce an alias target",
        ));
    };
    Ok(strip_type_syntax_ids(target.clone()))
}

fn strip_type_syntax_ids(typ: ast::FolType) -> ast::FolType {
    match typ {
        ast::FolType::Array { element_type, size } => ast::FolType::Array {
            element_type: Box::new(strip_type_syntax_ids(*element_type)),
            size,
        },
        ast::FolType::Vector { element_type } => ast::FolType::Vector {
            element_type: Box::new(strip_type_syntax_ids(*element_type)),
        },
        ast::FolType::Sequence { element_type } => ast::FolType::Sequence {
            element_type: Box::new(strip_type_syntax_ids(*element_type)),
        },
        ast::FolType::Matrix {
            element_type,
            dimensions,
        } => ast::FolType::Matrix {
            element_type: Box::new(strip_type_syntax_ids(*element_type)),
            dimensions,
        },
        ast::FolType::Set { types } => ast::FolType::Set {
            types: types.into_iter().map(strip_type_syntax_ids).collect(),
        },
        ast::FolType::Map {
            key_type,
            value_type,
        } => ast::FolType::Map {
            key_type: Box::new(strip_type_syntax_ids(*key_type)),
            value_type: Box::new(strip_type_syntax_ids(*value_type)),
        },
        ast::FolType::Channel { element_type } => ast::FolType::Channel {
            element_type: Box::new(strip_type_syntax_ids(*element_type)),
        },
        ast::FolType::ChannelSender { element_type } => ast::FolType::ChannelSender {
            element_type: Box::new(strip_type_syntax_ids(*element_type)),
        },
        ast::FolType::ChannelReceiver { element_type } => ast::FolType::ChannelReceiver {
            element_type: Box::new(strip_type_syntax_ids(*element_type)),
        },
        ast::FolType::Mutex { inner } => ast::FolType::Mutex {
            inner: Box::new(strip_type_syntax_ids(*inner)),
        },
        ast::FolType::Record { fields } => ast::FolType::Record {
            fields: fields
                .into_iter()
                .map(|(name, typ)| (name, strip_type_syntax_ids(typ)))
                .collect(),
        },
        ast::FolType::Entry { variants } => ast::FolType::Entry {
            variants: variants
                .into_iter()
                .map(|(name, typ)| (name, typ.map(strip_type_syntax_ids)))
                .collect(),
        },
        ast::FolType::Optional { inner } => ast::FolType::Optional {
            inner: Box::new(strip_type_syntax_ids(*inner)),
        },
        ast::FolType::Owned { inner } => ast::FolType::Owned {
            inner: Box::new(strip_type_syntax_ids(*inner)),
        },
        ast::FolType::Borrowed {
            inner,
            lifetime,
            mutable,
        } => ast::FolType::Borrowed {
            inner: Box::new(strip_type_syntax_ids(*inner)),
            lifetime,
            mutable,
        },
        ast::FolType::Multiple { types } => ast::FolType::Multiple {
            types: types.into_iter().map(strip_type_syntax_ids).collect(),
        },
        ast::FolType::Union { types } => ast::FolType::Union {
            types: types.into_iter().map(strip_type_syntax_ids).collect(),
        },
        ast::FolType::Pointer { qualifier, target } => ast::FolType::Pointer {
            qualifier,
            target: Box::new(strip_type_syntax_ids(*target)),
        },
        ast::FolType::Error { inner } => ast::FolType::Error {
            inner: inner.map(|inner| Box::new(strip_type_syntax_ids(*inner))),
        },
        ast::FolType::Eventual {
            value_type,
            error_type,
            lifetime,
        } => ast::FolType::Eventual {
            value_type: Box::new(strip_type_syntax_ids(*value_type)),
            error_type: error_type.map(|error_type| Box::new(strip_type_syntax_ids(*error_type))),
            lifetime,
        },
        ast::FolType::Limited { base, limits } => ast::FolType::Limited {
            base: Box::new(strip_type_syntax_ids(*base)),
            limits,
        },
        ast::FolType::Function {
            params,
            return_type,
            env_lifetime,
        } => ast::FolType::Function {
            params: params.into_iter().map(strip_type_syntax_ids).collect(),
            return_type: Box::new(strip_type_syntax_ids(*return_type)),
            env_lifetime,
        },
        ast::FolType::Generic { name, constraints } => ast::FolType::Generic {
            name,
            constraints: constraints.into_iter().map(strip_type_syntax_ids).collect(),
        },
        ast::FolType::Named { name, .. } => ast::FolType::Named {
            syntax_id: None,
            name,
        },
        ast::FolType::QualifiedNamed { path } => ast::FolType::QualifiedNamed {
            path: ast::QualifiedPath::new(path.segments),
        },
        other => other,
    }
}

/// Source kind names used by import declarations and tree-sitter.
pub const SOURCE_KIND_NAMES: &[&str] = &["loc", "pkg"];

/// Container type names used by syntax and tree-sitter.
pub const CONTAINER_TYPE_NAMES: &[&str] = &["arr", "vec", "seq", "set", "map"];

/// Shell type names used by syntax and tree-sitter.
pub const SHELL_TYPE_NAMES: &[&str] = &["opt", "err"];

#[cfg(test)]
mod tests {
    use super::*;
    use ast::FolType;

    #[test]
    fn borrow_type_options_parse_to_borrowed_types() {
        assert!(matches!(
            parse_type_reference_text("int[bor]"),
            Ok(FolType::Borrowed {
                mutable: false,
                lifetime: None,
                ..
            })
        ));
        assert!(matches!(
            parse_type_reference_text("int[mut, bor]"),
            Ok(FolType::Borrowed { mutable: true, .. })
        ));
        match parse_type_reference_text("int[bor=L]") {
            Ok(FolType::Borrowed { lifetime, .. }) => {
                assert_eq!(lifetime.as_deref(), Some("L"));
            }
            other => panic!("expected a borrowed type with a lifetime, got {other:?}"),
        }
    }

    #[test]
    fn eventual_types_parse_with_and_without_an_error_channel() {
        match parse_type_reference_text("evt[int]") {
            Ok(FolType::Eventual {
                value_type,
                error_type,
                ..
            }) => {
                assert!(matches!(*value_type, FolType::Int { .. }));
                assert!(error_type.is_none());
            }
            other => panic!("expected an infallible eventual type, got {other:?}"),
        }
        match parse_type_reference_text("evt[int / str]") {
            Ok(FolType::Eventual {
                value_type,
                error_type,
                ..
            }) => {
                assert!(matches!(*value_type, FolType::Int { .. }));
                assert!(error_type.is_some_and(|error| error.is_builtin_str()));
            }
            other => panic!("expected a recoverable eventual type, got {other:?}"),
        }
        // Public spelling with a named parent-scope lifetime (V3_MEM §8.1).
        match parse_type_reference_text("evt[L, int]") {
            Ok(FolType::Eventual {
                value_type,
                error_type,
                lifetime,
            }) => {
                assert_eq!(lifetime.as_deref(), Some("L"));
                assert!(matches!(*value_type, FolType::Int { .. }));
                assert!(error_type.is_none());
            }
            other => panic!("expected a lifetime-carrying eventual type, got {other:?}"),
        }
        match parse_type_reference_text("evt[L, int / str]") {
            Ok(FolType::Eventual {
                value_type,
                error_type,
                lifetime,
            }) => {
                assert_eq!(lifetime.as_deref(), Some("L"));
                assert!(matches!(*value_type, FolType::Int { .. }));
                assert!(error_type.is_some_and(|error| error.is_builtin_str()));
            }
            other => panic!("expected a recoverable lifetime-carrying eventual, got {other:?}"),
        }
    }

    #[test]
    fn channel_endpoint_types_parse_the_tx_marker() {
        match parse_type_reference_text("chn[int]") {
            Ok(FolType::Channel { element_type }) => {
                assert!(matches!(*element_type, FolType::Int { .. }));
            }
            other => panic!("expected a full channel type, got {other:?}"),
        }
        match parse_type_reference_text("chn[tx, int]") {
            Ok(FolType::ChannelSender { element_type }) => {
                assert!(matches!(*element_type, FolType::Int { .. }));
            }
            other => panic!("expected a sender endpoint type, got {other:?}"),
        }
        match parse_type_reference_text("chn[rx, int]") {
            Ok(FolType::ChannelReceiver { element_type }) => {
                assert!(matches!(*element_type, FolType::Int { .. }));
            }
            other => panic!("expected a receiver endpoint type, got {other:?}"),
        }
    }

    #[test]
    fn non_borrow_type_options_are_unaffected() {
        // A sized integer option must still parse as an integer, not a borrow.
        assert!(matches!(
            parse_type_reference_text("int[64]"),
            Ok(FolType::Int { .. })
        ));
    }
}

#[cfg(test)]
mod empty_line_comment_tests {
    use fol_stream::{FileStream, Source};

    /// Parse a snippet and count the `use` declarations that survived.
    ///
    /// Comments are themselves items, so a raw item count would move with the
    /// comment under test rather than with what it damaged.
    fn use_declarations(source: &str) -> usize {
        let stream = FileStream::from_preloaded(vec![Source {
            call: "<test>".to_string(),
            path: "<test>".to_string(),
            data: source.to_string(),
            namespace: "test".to_string(),
            package: "test".to_string(),
        }])
        .expect("the stream should build");
        let mut stream = stream;
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = crate::ast::AstParser::new();
        let package = parser
            .parse_package(&mut lexer)
            .expect("the snippet should parse");
        package
            .source_units
            .first()
            .map(|unit| {
                unit.items
                    .iter()
                    .filter(|item| matches!(item.node, crate::ast::AstNode::UseDecl { .. }))
                    .count()
            })
            .unwrap_or_default()
    }

    /// An empty `//` used to swallow the line after it.
    ///
    /// The line-comment scanner bumped once past the second slash and then ran
    /// a loop testing the character *after* the cursor. With any comment text
    /// that bump consumed comment text, which is right; with an empty `//` it
    /// consumed the newline, and the loop then ate the whole following line.
    ///
    /// The visible damage was a silently dropped declaration: a file whose
    /// first `use` sat directly under a `//` line lost that import and failed
    /// later with an unresolved-name error pointing at the use site, with
    /// nothing wrong there.
    #[test]
    fn an_empty_line_comment_does_not_swallow_the_next_line() {
        let expected = use_declarations("use std: pkg = {\"std\"};\n");
        assert_eq!(expected, 1, "the baseline should have one use declaration");

        for source in [
            "//\nuse std: pkg = {\"std\"};\n",
            "// note\n//\nuse std: pkg = {\"std\"};\n",
            "//\n//\nuse std: pkg = {\"std\"};\n",
        ] {
            assert_eq!(
                use_declarations(source),
                1,
                "an empty // swallowed the declaration in {source:?}"
            );
        }
    }

    /// A comment with text still ends at its own newline.
    #[test]
    fn a_line_comment_still_ends_at_its_newline() {
        for source in [
            "// note\nuse std: pkg = {\"std\"};\n",
            "// \nuse std: pkg = {\"std\"};\n",
            "//x\nuse std: pkg = {\"std\"};\n",
        ] {
            assert_eq!(use_declarations(source), 1, "for {source:?}");
        }
    }
}

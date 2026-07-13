// FOL Parser - Clean AST-only implementation

pub mod ast;

// Re-export the AST parser
pub use ast::*;
use fol_stream::{FileStream, Source};

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
        ast::FolType::Limited { base, limits } => ast::FolType::Limited {
            base: Box::new(strip_type_syntax_ids(*base)),
            limits,
        },
        ast::FolType::Function {
            params,
            return_type,
        } => ast::FolType::Function {
            params: params.into_iter().map(strip_type_syntax_ids).collect(),
            return_type: Box::new(strip_type_syntax_ids(*return_type)),
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
pub const SOURCE_KIND_NAMES: &[&str] = &["loc", "std", "pkg"];

/// Container type names used by syntax and tree-sitter.
pub const CONTAINER_TYPE_NAMES: &[&str] = &["arr", "vec", "seq", "set", "map"];

/// Shell type names used by syntax and tree-sitter.
pub const SHELL_TYPE_NAMES: &[&str] = &["opt", "err"];

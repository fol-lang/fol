use fol_parser::ast::{AstNode, Literal, RecordInitField};

use super::types::ExecValue;

// ---- Option kind helpers ---

#[derive(Clone, Copy)]
pub(super) enum OptionKind {
    Target,
    Optimize,
    Bool,
    Int,
    String,
    Enum,
    Path,
}

pub(super) fn parse_option_kind(raw: &str) -> Option<OptionKind> {
    match raw {
        "target" => Some(OptionKind::Target),
        "optimize" => Some(OptionKind::Optimize),
        "bool" => Some(OptionKind::Bool),
        "int" => Some(OptionKind::Int),
        "str" => Some(OptionKind::String),
        "enum" => Some(OptionKind::Enum),
        "path" => Some(OptionKind::Path),
        _ => None,
    }
}

pub(super) fn build_option_kind(kind: OptionKind) -> crate::BuildOptionKind {
    match kind {
        OptionKind::Target => crate::BuildOptionKind::Target,
        OptionKind::Optimize => crate::BuildOptionKind::Optimize,
        OptionKind::Bool => crate::BuildOptionKind::Bool,
        OptionKind::Int => crate::BuildOptionKind::Int,
        OptionKind::String => crate::BuildOptionKind::String,
        OptionKind::Enum => crate::BuildOptionKind::Enum,
        OptionKind::Path => crate::BuildOptionKind::Path,
    }
}

pub(super) fn option_exec_value(kind: OptionKind, name: String) -> ExecValue {
    match kind {
        OptionKind::Target => ExecValue::Target(name),
        OptionKind::Optimize => ExecValue::Optimize(name),
        OptionKind::Bool
        | OptionKind::Int
        | OptionKind::String
        | OptionKind::Enum
        | OptionKind::Path => ExecValue::OptionRef {
            name,
            kind: build_option_kind(kind),
        },
    }
}

pub(super) fn parse_option_default(
    kind: OptionKind,
    fields: &[RecordInitField],
    resolve_str: impl Fn(&AstNode) -> Option<String>,
) -> Option<crate::BuildOptionValue> {
    let field = fields.iter().find(|f| f.name == "default")?;
    match (kind, &field.value) {
        (OptionKind::Bool, AstNode::Literal(Literal::Boolean(b))) => {
            Some(crate::BuildOptionValue::Bool(*b))
        }
        (OptionKind::Int, AstNode::Literal(Literal::Integer(i))) => {
            Some(crate::BuildOptionValue::Int(*i))
        }
        (OptionKind::String, _) => resolve_str(&field.value).map(crate::BuildOptionValue::String),
        (OptionKind::Enum, _) => resolve_str(&field.value).map(crate::BuildOptionValue::Enum),
        (OptionKind::Path, _) => resolve_str(&field.value).map(crate::BuildOptionValue::Path),
        _ => None,
    }
}

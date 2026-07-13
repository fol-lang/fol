use std::collections::HashMap;

use super::node::AstNode;
use super::syntax::SyntaxNodeId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QualifiedPath {
    pub segments: Vec<String>,
    pub syntax_id: Option<SyntaxNodeId>,
    /// Syntax anchor of the final path segment (the segment that names the
    /// referenced symbol); `syntax_id` anchors the root segment.
    pub final_syntax_id: Option<SyntaxNodeId>,
}

impl QualifiedPath {
    pub fn new(segments: Vec<String>) -> Self {
        Self {
            segments,
            syntax_id: None,
            final_syntax_id: None,
        }
    }

    pub fn with_syntax_id(segments: Vec<String>, syntax_id: Option<SyntaxNodeId>) -> Self {
        Self {
            segments,
            syntax_id,
            final_syntax_id: None,
        }
    }

    pub fn with_segment_syntax_ids(
        segments: Vec<String>,
        syntax_id: Option<SyntaxNodeId>,
        final_syntax_id: Option<SyntaxNodeId>,
    ) -> Self {
        Self {
            segments,
            syntax_id,
            final_syntax_id,
        }
    }

    pub fn from_joined(path: &str) -> Self {
        Self {
            segments: path
                .split("::")
                .map(|segment| segment.to_string())
                .collect(),
            syntax_id: None,
            final_syntax_id: None,
        }
    }

    pub fn syntax_id(&self) -> Option<SyntaxNodeId> {
        self.syntax_id
    }

    pub fn final_syntax_id(&self) -> Option<SyntaxNodeId> {
        self.final_syntax_id
    }

    pub fn is_qualified(&self) -> bool {
        self.segments.len() > 1
    }

    pub fn joined(&self) -> String {
        self.segments.join("::")
    }
}

/// FOL Type system
#[derive(Debug, Clone, PartialEq)]
pub enum FolType {
    // Ordinal types
    Int {
        size: Option<super::options::IntSize>,
        signed: bool,
    },
    Float {
        size: Option<super::options::FloatSize>,
    },
    Char {
        encoding: super::options::CharEncoding,
    },
    Bool,

    // Container types
    Array {
        element_type: Box<FolType>,
        size: Option<usize>,
    },
    Vector {
        element_type: Box<FolType>,
    },
    Sequence {
        element_type: Box<FolType>,
    },
    Matrix {
        element_type: Box<FolType>,
        dimensions: Vec<usize>,
    },
    Set {
        types: Vec<FolType>,
    }, // Tuple-like heterogeneous set
    Map {
        key_type: Box<FolType>,
        value_type: Box<FolType>,
    },
    Channel {
        element_type: Box<FolType>,
    },

    // Complex types
    Record {
        fields: HashMap<String, FolType>,
    },
    Entry {
        variants: HashMap<String, Option<FolType>>,
    }, // Enum-like

    // Special types
    Optional {
        inner: Box<FolType>,
    }, // opt[T]
    Multiple {
        types: Vec<FolType>,
    }, // mul[T1, T2, ...]
    Union {
        types: Vec<FolType>,
    }, // uni[T1, T2, ...]
    Never,
    Any,
    Pointer {
        target: Box<FolType>,
    },
    Error {
        inner: Option<Box<FolType>>,
    },
    Limited {
        base: Box<FolType>,
        limits: Vec<AstNode>,
    },
    None,

    // Function types
    Function {
        params: Vec<FolType>,
        return_type: Box<FolType>,
    },

    // Generic and module types
    Generic {
        name: String,
        constraints: Vec<FolType>,
    },
    Module {
        name: String,
    },
    Block {
        name: String,
    },
    Test {
        name: Option<String>,
        access: Vec<String>,
    },
    Package {
        name: String,
    },
    Location {
        name: String,
    },

    // User-defined type reference
    Named {
        syntax_id: Option<SyntaxNodeId>,
        name: String,
    },
    QualifiedNamed {
        path: QualifiedPath,
    },
}

impl FolType {
    pub fn is_builtin_str(&self) -> bool {
        matches!(self, FolType::Named { name, .. } if name == "str")
    }

    pub fn named_text(&self) -> Option<String> {
        match self {
            FolType::Named { name, .. } => Some(name.clone()),
            FolType::QualifiedNamed { path } => Some(path.joined()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InquiryTarget {
    SelfValue,
    ThisValue,
    Named(String),
    Quoted(String),
    Qualified(QualifiedPath),
}

impl InquiryTarget {
    pub fn duplicate_key(&self) -> String {
        match self {
            InquiryTarget::SelfValue => "self".to_string(),
            InquiryTarget::ThisValue => "this".to_string(),
            InquiryTarget::Named(name) | InquiryTarget::Quoted(name) => name.clone(),
            InquiryTarget::Qualified(path) => path.joined(),
        }
    }

    pub fn display_label(&self) -> String {
        match self {
            InquiryTarget::SelfValue => "self".to_string(),
            InquiryTarget::ThisValue => "this".to_string(),
            InquiryTarget::Named(name) => name.clone(),
            InquiryTarget::Quoted(name) => format!("\"{}\"", name),
            InquiryTarget::Qualified(path) => path.joined(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BindingPattern {
    Name(String, Option<SyntaxNodeId>),
    Rest(String),
    Sequence(Vec<BindingPattern>),
}

impl BindingPattern {
    pub fn is_destructuring(&self) -> bool {
        match self {
            BindingPattern::Name(..) => false,
            BindingPattern::Rest(_) => true,
            BindingPattern::Sequence(parts) => {
                parts.len() != 1 || parts.iter().any(BindingPattern::is_destructuring)
            }
        }
    }
}

/// Function/Procedure parameters
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub param_type: FolType,
    pub is_borrowable: bool,
    pub is_mutex: bool,
    /// Declared with the explicit `... T` variadic marker. The parameter's
    /// `param_type` is the collected `seq[T]`; a plain trailing `seq[T]`
    /// parameter is NOT variadic.
    pub is_variadic: bool,
    pub default: Option<AstNode>,
    /// Syntax id of the parameter NAME token, so tooling can locate the
    /// parameter's own declaration span. The resolver derives the parameter
    /// symbol origin from this id. `None` when the parameter was synthesized
    /// without a source name token.
    pub syntax_id: Option<SyntaxNodeId>,
}

/// Generic type parameters
#[derive(Debug, Clone, PartialEq)]
pub struct Generic {
    pub name: String,
    pub constraints: Vec<FolType>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordFieldMeta {
    pub default: Option<AstNode>,
    pub options: Vec<super::options::VarOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntryVariantMeta {
    pub default: Option<AstNode>,
    pub options: Vec<super::options::VarOption>,
}

/// Type definitions for structs/records/entries
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDefinition {
    Record {
        fields: HashMap<String, FolType>,
        field_meta: HashMap<String, RecordFieldMeta>,
        /// Field names in source declaration order. `fields`/`field_meta` are
        /// keyed maps that lose ordering; positional record initialization
        /// binds values to fields using this declaration order.
        field_order: Vec<String>,
        members: Vec<AstNode>,
    },
    Entry {
        variants: HashMap<String, Option<FolType>>,
        variant_meta: HashMap<String, EntryVariantMeta>,
        members: Vec<AstNode>,
    },
    Alias {
        target: FolType,
    },
}

/// When statement cases
#[derive(Debug, Clone, PartialEq)]
pub enum WhenCase {
    /// case(condition) { body }
    Case {
        condition: AstNode,
        body: Vec<AstNode>,
    },
    /// is(value) { body } - for value matching
    Is { value: AstNode, body: Vec<AstNode> },
    /// in(range/set) { body } - for range/set matching
    In { range: AstNode, body: Vec<AstNode> },
    /// has(member) { body } - for containment checking
    Has { member: AstNode, body: Vec<AstNode> },
    /// of(type) { body } - for type matching
    Of {
        type_match: FolType,
        body: Vec<AstNode>,
    },
    /// on(channel) { body } - for channel matching
    On {
        channel: AstNode,
        body: Vec<AstNode>,
    },
}

/// Loop condition types
#[derive(Debug, Clone, PartialEq)]
pub enum LoopCondition {
    /// loop(condition) - while-like loop
    Condition(Box<AstNode>),
    /// loop(var in iterable) - for-like loop
    Iteration {
        var: String,
        type_hint: Option<FolType>,
        iterable: Box<AstNode>,
        condition: Option<Box<AstNode>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChannelEndpoint {
    Tx,
    Rx,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RollingBinding {
    pub name: String,
    pub type_hint: Option<FolType>,
    pub iterable: AstNode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordInitField {
    pub name: String,
    pub value: AstNode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fol_type_recognizes_builtin_str_without_treating_other_names_as_builtin() {
        assert!(FolType::Named {
            syntax_id: None,
            name: "str".to_string()
        }
        .is_builtin_str());
        assert!(!FolType::Named {
            syntax_id: None,
            name: "String".to_string()
        }
        .is_builtin_str());
        assert!(!FolType::QualifiedNamed {
            path: QualifiedPath::from_joined("pkg::str")
        }
        .is_builtin_str());
    }
}

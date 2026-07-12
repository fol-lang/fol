use super::syntax::ParsedDeclVisibility;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UsePathSeparator {
    Slash,
    DoubleColon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsePathSegment {
    pub separator: Option<UsePathSeparator>,
    pub spelling: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommentKind {
    Backtick,
    Doc,
    SlashLine,
    SlashBlock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallSurface {
    Plain,
    DotIntrinsic,
    KeywordIntrinsic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardKind {
    Protocol,
    Blueprint,
    Extended,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeclOption {
    Export,
    Hidden,
    Normal,
}

/// Integer sizes
#[derive(Debug, Clone, PartialEq)]
pub enum IntSize {
    I8,
    I16,
    I32,
    I64,
    I128,
    Arch,
}

/// Float sizes
#[derive(Debug, Clone, PartialEq)]
pub enum FloatSize {
    F32,
    F64,
    Arch,
}

/// Character encodings
#[derive(Debug, Clone, PartialEq)]
pub enum CharEncoding {
    Utf8,
    Utf16,
    Utf32,
}

/// Container type for literals
#[derive(Debug, Clone, PartialEq)]
pub enum ContainerType {
    Array,
    Vector,
    Sequence,
    Set,
    Map,
}

/// Literal values
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    String(String),
    Character(char),
    Boolean(bool),
    Nil,
}

/// Binary operators
#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // Logical
    And,
    Or,
    Xor,

    // Other
    In,
    Has,
    Is,
    As,
    Cast,
    Pipe,
    PipeOr,
}

/// Unary operators
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOperator {
    Neg,
    Not,
    Ref,
    Deref,
    Unwrap,
}

/// Variable declaration options
#[derive(Debug, Clone, PartialEq)]
pub enum VarOption {
    Mutable,   // mut or ~
    Immutable, // imu (default)
    Static,    // sta or !
    Reactive,  // rac or ?
    Export,    // exp or +
    Normal,    // nor (default)
    Hidden,    // hid or -
    New,       // allocate on heap
    Borrowing, // bor - borrowing a value
}

/// Function/Procedure options
#[derive(Debug, Clone, PartialEq)]
pub enum FunOption {
    Export,   // exp or +
    Hidden,   // hid or -
    Mutable,  // mut
    Iterator, // itr
}

/// Type declaration options
#[derive(Debug, Clone, PartialEq)]
pub enum TypeOption {
    Export,    // exp or +
    Set,       // set
    Get,       // get
    Nothing,   // nothing
    Extension, // ext
    Alias,     // ali — the book's typ[ali] aliasing marker
}

/// Use declaration options
#[derive(Debug, Clone, PartialEq)]
pub enum UseOption {
    Export,
    Hidden,
    Normal,
}

pub fn decl_visibility(options: &[DeclOption]) -> ParsedDeclVisibility {
    if options
        .iter()
        .any(|option| matches!(option, DeclOption::Hidden))
    {
        ParsedDeclVisibility::Hidden
    } else if options
        .iter()
        .any(|option| matches!(option, DeclOption::Export))
    {
        ParsedDeclVisibility::Exported
    } else {
        ParsedDeclVisibility::Normal
    }
}

/// Whether a `var`/`lab` binding was declared mutable (`var[mut]` / `var[~]`).
/// Bindings are immutable by default per the variables chapter of the book.
pub fn binding_is_mutable(options: &[VarOption]) -> bool {
    options
        .iter()
        .any(|option| matches!(option, VarOption::Mutable))
}

pub fn var_decl_visibility(options: &[VarOption]) -> ParsedDeclVisibility {
    if options
        .iter()
        .any(|option| matches!(option, VarOption::Hidden))
    {
        ParsedDeclVisibility::Hidden
    } else if options
        .iter()
        .any(|option| matches!(option, VarOption::Export))
    {
        ParsedDeclVisibility::Exported
    } else {
        ParsedDeclVisibility::Normal
    }
}

pub fn fun_decl_visibility(options: &[FunOption]) -> ParsedDeclVisibility {
    if options
        .iter()
        .any(|option| matches!(option, FunOption::Hidden))
    {
        ParsedDeclVisibility::Hidden
    } else if options
        .iter()
        .any(|option| matches!(option, FunOption::Export))
    {
        ParsedDeclVisibility::Exported
    } else {
        ParsedDeclVisibility::Normal
    }
}

pub fn type_decl_visibility(options: &[TypeOption]) -> ParsedDeclVisibility {
    if options
        .iter()
        .any(|option| matches!(option, TypeOption::Export))
    {
        ParsedDeclVisibility::Exported
    } else {
        ParsedDeclVisibility::Normal
    }
}

pub fn use_decl_visibility(options: &[UseOption]) -> ParsedDeclVisibility {
    if options
        .iter()
        .any(|option| matches!(option, UseOption::Hidden))
    {
        ParsedDeclVisibility::Hidden
    } else if options
        .iter()
        .any(|option| matches!(option, UseOption::Export))
    {
        ParsedDeclVisibility::Exported
    } else {
        ParsedDeclVisibility::Normal
    }
}

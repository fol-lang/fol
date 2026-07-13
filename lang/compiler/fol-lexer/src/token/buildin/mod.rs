use std::fmt;

/// Declaration keyword names used by syntax tooling and tree-sitter.
pub const DECLARATION_KEYWORDS: &[&str] = &[
    "fun", "var", "def", "typ", "pro", "log", "seg", "ali", "lab", "con", "use", "std",
];

/// Control flow keyword names.
pub const CONTROL_KEYWORDS: &[&str] = &[
    "if", "else", "when", "while", "loop", "for", "each", "do", "case",
    "break", "return", "yield", "dfr", "select",
];

/// Operator keyword names.
pub const OPERATOR_KEYWORDS: &[&str] = &[
    "not", "or", "xor", "nor", "and", "nand", "as", "cast", "is", "at", "has", "in", "on", "of",
];

/// Literal keyword names.
pub const LITERAL_KEYWORDS: &[&str] = &["true", "false"];

/// Diagnostic/error keyword names.
pub const DIAGNOSTIC_KEYWORDS: &[&str] = &["panic", "report", "check", "assert"];

/// Other keyword names.
pub const OTHER_KEYWORDS: &[&str] = &["let", "this", "self", "where", "get", "async", "await"];

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BUILDIN {
    Use,
    Def,
    Seg,
    Var,
    Log,
    Con,
    Fun,
    Pro,
    Typ,
    Std,
    Ali,
    Lab,

    Not,
    Or,
    Xor,
    Nor,
    And,
    Nand,
    As,
    Cast,
    If,
    Else,
    When,
    While,
    Loop,
    Is,
    At,
    Has,
    In,
    On,
    Case,
    This,
    Selfi,
    Break,
    Return,
    Yield,
    Dfr,
    Panic,
    Report,
    Check,
    Assert,
    Where,
    True,
    False,
    Each,
    For,
    Do,
    Get,
    Of,
    Let,
    Async,
    Await,
    Select,
}

impl fmt::Display for BUILDIN {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = match self {
            BUILDIN::Use => "use",
            BUILDIN::Var => "var",
            BUILDIN::Log => "log",
            BUILDIN::Con => "con",
            BUILDIN::Def => "def",
            BUILDIN::Seg => "seg",
            BUILDIN::Fun => "fun",
            BUILDIN::Pro => "pro",
            BUILDIN::Typ => "typ",
            BUILDIN::Std => "std",
            BUILDIN::Ali => "ali",
            BUILDIN::Lab => "lab",
            BUILDIN::Not => "not",
            BUILDIN::Or => "or",
            BUILDIN::Xor => "xor",
            BUILDIN::Nor => "nor",
            BUILDIN::And => "and",
            BUILDIN::Nand => "nand",
            BUILDIN::As => "as",
            BUILDIN::Cast => "cast",
            BUILDIN::If => "if",
            BUILDIN::Else => "else",
            BUILDIN::While => "while",
            BUILDIN::Of => "of",
            BUILDIN::When => "when",
            BUILDIN::Loop => "loop",
            BUILDIN::Is => "is",
            BUILDIN::Has => "has",
            BUILDIN::In => "in",
            BUILDIN::On => "on",
            BUILDIN::At => "at",
            BUILDIN::Case => "case",
            BUILDIN::This => "this",
            BUILDIN::Selfi => "self",
            BUILDIN::Break => "break",
            BUILDIN::Return => "return",
            BUILDIN::Yield => "yield",
            BUILDIN::Dfr => "dfr",
            BUILDIN::Panic => "panic",
            BUILDIN::Report => "report",
            BUILDIN::Check => "check",
            BUILDIN::Assert => "assert",
            BUILDIN::Where => "where",
            BUILDIN::True => "true",
            BUILDIN::False => "false",
            BUILDIN::Each => "each",
            BUILDIN::For => "for",
            BUILDIN::Do => "do",
            BUILDIN::Get => "get",
            BUILDIN::Let => "let",
            BUILDIN::Async => "async",
            BUILDIN::Await => "await",
            BUILDIN::Select => "select",
        };
        write!(f, "BUILDIN:{}", t)
    }
}

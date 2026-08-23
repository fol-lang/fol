//! The projectability classifier.
//!
//! Every rejection reason from section 4.6's boundary restrictions, plus the
//! path to the offending field. The path matters: for a record of a record of
//! a `vec`, "this type is not projectable" is useless, and
//! `Outer.middle.items` is actionable.
//!
//! The classifier runs before backend emission, so the backend never has to
//! rediscover ABI legality -- which is exactly what M4's STOP forbids.

/// Why a type cannot cross the C boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiRejection {
    /// `arch`/`uarch`: target-dependent by construction.
    ArchitectureSizedNumeric { spelling: String },
    /// `i128`/`u128`: no portable C counterpart.
    OversizedInteger { spelling: String },
    /// A character encoding other than UTF-32.
    UnsupportedCharacterEncoding { encoding: String },
    /// A generic declaration or an unsubstituted parameter.
    Generic { name: String },
    /// A structural aggregate with no declared name.
    AnonymousAggregate,
    /// An entry whose variant tags are positional rather than explicit.
    UnstableEntryTag { entry: String },
    /// An internal container with no canonical projection.
    InternalContainer { spelling: String },
    /// An owning or shared pointer without a canonical wrapper.
    UnwrappedPointer { spelling: String },
    /// A raw pointer missing one of its required facts.
    IncompletePointerContract { missing: String },
    /// A standard, protocol, routine object, or closure.
    RoutineOrProtocolObject { spelling: String },
    /// A concurrency object.
    ConcurrencyObject { spelling: String },
    /// An aggregate that contains itself by value.
    RecursiveByValue { name: String },
    /// A packed, bitfield, or flexible-array form.
    UnsupportedLayout { detail: String },
    /// An external symbol that is empty, non-ASCII, reserved, or duplicated.
    InvalidExternalSymbol { symbol: String, reason: String },
    /// An effect or capability the artifact's model does not permit.
    CapabilityTooStrong { detail: String },
}

impl AbiRejection {
    /// The diagnostic code this rejection is reported under.
    ///
    /// Returned as a string rather than a `DiagnosticCode` because `fol-abi`
    /// may depend only on `fol-types`; the diagnostic layer builds the code
    /// from this. Every code here is registered in `fol-diagnostics`, and the
    /// registry guard requires the reverse -- a registered code with no
    /// producer documents a diagnostic that cannot happen.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::InvalidExternalSymbol { .. } => "A1002",
            Self::IncompletePointerContract { .. } => "A1003",
            // Everything else is a type that cannot cross the boundary.
            _ => "A1001",
        }
    }

    /// A short, stable reason code for diagnostics and tests.
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::ArchitectureSizedNumeric { .. } => "architecture-sized-numeric",
            Self::OversizedInteger { .. } => "oversized-integer",
            Self::UnsupportedCharacterEncoding { .. } => "unsupported-character-encoding",
            Self::Generic { .. } => "generic",
            Self::AnonymousAggregate => "anonymous-aggregate",
            Self::UnstableEntryTag { .. } => "unstable-entry-tag",
            Self::InternalContainer { .. } => "internal-container",
            Self::UnwrappedPointer { .. } => "unwrapped-pointer",
            Self::IncompletePointerContract { .. } => "incomplete-pointer-contract",
            Self::RoutineOrProtocolObject { .. } => "routine-or-protocol-object",
            Self::ConcurrencyObject { .. } => "concurrency-object",
            Self::RecursiveByValue { .. } => "recursive-by-value",
            Self::UnsupportedLayout { .. } => "unsupported-layout",
            Self::InvalidExternalSymbol { .. } => "invalid-external-symbol",
            Self::CapabilityTooStrong { .. } => "capability-too-strong",
        }
    }
}

impl std::fmt::Display for AbiRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ArchitectureSizedNumeric { spelling } => write!(
                f,
                "'{spelling}' is architecture-sized, and a stable ABI cannot depend on the \
                 pointer width of whoever compiled it"
            ),
            Self::OversizedInteger { spelling } => {
                write!(f, "'{spelling}' has no portable C counterpart")
            }
            Self::UnsupportedCharacterEncoding { encoding } => write!(
                f,
                "character encoding '{encoding}' does not cross the C boundary; only utf32 does"
            ),
            Self::Generic { name } => write!(
                f,
                "'{name}' is generic; wrap it in an ordinary non-generic routine or type before \
                 exporting it"
            ),
            Self::AnonymousAggregate => write!(
                f,
                "an anonymous aggregate has no name to give the generated C type"
            ),
            Self::UnstableEntryTag { entry } => write!(
                f,
                "entry '{entry}' has no explicit discriminants, so inserting a variant would \
                 renumber the ones after it"
            ),
            Self::InternalContainer { spelling } => write!(
                f,
                "'{spelling}' is an internal representation with no canonical C projection"
            ),
            Self::UnwrappedPointer { spelling } => write!(
                f,
                "'{spelling}' is an owning or shared pointer; only a raw address token crosses \
                 the boundary"
            ),
            Self::IncompletePointerContract { missing } => write!(
                f,
                "the raw pointer is missing its {missing}, which a C caller cannot infer"
            ),
            Self::RoutineOrProtocolObject { spelling } => write!(
                f,
                "'{spelling}' is a routine or protocol object and has no C representation"
            ),
            Self::ConcurrencyObject { spelling } => write!(
                f,
                "'{spelling}' is a concurrency object and does not cross the boundary"
            ),
            Self::RecursiveByValue { name } => write!(
                f,
                "'{name}' contains itself by value, which has no finite C layout"
            ),
            Self::UnsupportedLayout { detail } => {
                write!(f, "unsupported layout: {detail}")
            }
            Self::InvalidExternalSymbol { symbol, reason } => {
                write!(f, "external symbol '{symbol}' is invalid: {reason}")
            }
            Self::CapabilityTooStrong { detail } => write!(
                f,
                "the declaration needs a capability the artifact's model does not allow: {detail}"
            ),
        }
    }
}

/// A rejection plus the exact path to the offending field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiClassification {
    /// e.g. `Outer.middle.items`. The root alone is the declaration itself.
    pub path: Vec<String>,
    pub rejection: AbiRejection,
}

impl AbiClassification {
    pub fn new(path: Vec<String>, rejection: AbiRejection) -> Self {
        Self { path, rejection }
    }

    /// The dotted path, for a diagnostic's related label.
    pub fn rendered_path(&self) -> String {
        self.path.join(".")
    }
}

impl std::fmt::Display for AbiClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.rendered_path(), self.rejection)
    }
}

/// C identifiers a generated symbol may not take.
///
/// Not an exhaustive list of every reserved C name -- that is unbounded -- but
/// the classes a generated symbol could plausibly collide with.
pub fn is_reserved_c_identifier(symbol: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
        "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
        "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
        "union", "unsigned", "void", "volatile", "while", "main",
    ];
    if KEYWORDS.contains(&symbol) {
        return true;
    }
    // A leading underscore followed by an uppercase letter, or any double
    // underscore, is reserved for the implementation in every scope.
    if symbol.contains("__") {
        return true;
    }
    let mut chars = symbol.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some('_'), Some(second)) if second.is_ascii_uppercase() || second == '_'
    )
}

/// Validate one external symbol.
pub fn classify_external_symbol(symbol: &str) -> Option<AbiRejection> {
    if symbol.is_empty() {
        return Some(AbiRejection::InvalidExternalSymbol {
            symbol: symbol.to_string(),
            reason: "it is empty".to_string(),
        });
    }
    if !symbol.is_ascii() {
        return Some(AbiRejection::InvalidExternalSymbol {
            symbol: symbol.to_string(),
            reason: "it is not ASCII".to_string(),
        });
    }
    let mut chars = symbol.chars();
    let first = chars.next().expect("checked non-empty");
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Some(AbiRejection::InvalidExternalSymbol {
            symbol: symbol.to_string(),
            reason: "a C identifier starts with a letter or underscore".to_string(),
        });
    }
    if !chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Some(AbiRejection::InvalidExternalSymbol {
            symbol: symbol.to_string(),
            reason: "a C identifier holds only letters, digits, and underscores".to_string(),
        });
    }
    if is_reserved_c_identifier(symbol) {
        return Some(AbiRejection::InvalidExternalSymbol {
            symbol: symbol.to_string(),
            reason: "it is reserved in C".to_string(),
        });
    }
    None
}

/// Reject a duplicated external symbol across a whole export set.
pub fn classify_duplicate_symbols(symbols: &[String]) -> Vec<AbiClassification> {
    let mut seen: Vec<&str> = Vec::new();
    let mut found = Vec::new();
    for symbol in symbols {
        if seen.contains(&symbol.as_str()) {
            found.push(AbiClassification::new(
                vec![symbol.clone()],
                AbiRejection::InvalidExternalSymbol {
                    symbol: symbol.clone(),
                    reason: "it is exported more than once".to_string(),
                },
            ));
        } else {
            seen.push(symbol);
        }
    }
    found
}

//! The explicit annotation overlay for an imported C surface.
//!
//! C cannot express which of its declarations are callable, what a return code
//! means, or whether a function may unwind. Section 4.13 makes those facts
//! explicit rather than guessed, so this module is the whole vocabulary of
//! what a header author is allowed to promise.
//!
//! The format is a strict subset of TOML: a `version` key, then one
//! `[routine.<symbol>]` table per selected declaration. The subset is parsed
//! here rather than pulled in as a dependency, because this crate depends on
//! `fol-types` and nothing else, and because a strict reader that rejects
//! everything it does not recognize is exactly what an overlay needs.

use std::collections::BTreeMap;

/// The only schema version this compiler accepts.
///
/// The overlay is part of both fingerprints, so a version bump is a
/// deliberate, visible change rather than a silent reinterpretation.
pub const ANNOTATION_SCHEMA_VERSION: u32 = 1;

/// How an imported routine reports failure.
///
/// The vocabulary is deliberately short. Section 4.13 admits exactly two
/// mappings in V4 and rejects the rest: a convention FOL cannot check is a
/// convention FOL will get wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportErrorConvention {
    /// The call cannot fail. Its C result is the FOL result.
    Infallible,
    /// An integer status plus a typed out-parameter.
    Status {
        /// Codes that mean success. The out value is read only for these.
        success: Vec<i64>,
        /// Codes that mean a reportable failure.
        failure: Vec<i64>,
        /// The parameter carrying the success value.
        out_parameter: String,
    },
}

impl ImportErrorConvention {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Infallible => "infallible",
            Self::Status { .. } => "status",
        }
    }

    /// Whether the synthesized FOL routine returns a recoverable result.
    pub const fn is_recoverable(&self) -> bool {
        matches!(self, Self::Status { .. })
    }
}

/// What an imported routine is permitted to do.
///
/// This is checked against the artifact's capability model before the call is
/// eligible, so a `core` artifact cannot reach an allocating C function
/// through an import any more than through a FOL one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportEffects {
    /// The provider allocates with the host allocator.
    pub allocates: bool,
    /// The provider performs host I/O.
    pub hosted: bool,
}

/// What a routine does with an opaque handle.
///
/// A C pointer to an incomplete type carries no ownership information at all,
/// so the three roles below are exactly the facts the overlay must supply. They
/// are per-routine rather than per-type because the same `sqlite3 *` is
/// produced by one call, lent to many, and consumed by one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HandleRole {
    /// The routine's result is a new handle whose release FOL now owes.
    Produces,
    /// The routine borrows a handle for the duration of the call.
    Borrows,
    /// The routine releases the handle. This is the domain's destroy symbol.
    Consumes,
}

impl HandleRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Produces => "produces",
            Self::Borrows => "borrows",
            Self::Consumes => "consumes",
        }
    }

    pub fn from_keyword(value: &str) -> Option<Self> {
        Some(match value {
            "produces" => Self::Produces,
            "borrows" => Self::Borrows,
            "consumes" => Self::Consumes,
            _ => return None,
        })
    }
}

/// A routine's relationship to one handle domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandleUse {
    /// The domain name, which is also the FOL type name.
    pub domain: String,
    pub role: HandleRole,
}

/// One declared handle domain.
///
/// The domain is the identity the destroy adapter checks: a handle produced by
/// one provider is a different FOL type from one produced by another, so
/// passing the wrong handle to a destroy is a type error rather than a runtime
/// hazard. That is why the domain, not the C pointee spelling, is what the
/// overlay names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleDomain {
    pub name: String,
    /// The exact C symbol that releases a handle of this domain.
    pub destroy: String,
}

/// A routine's synchronous callback, named by the overlay.
///
/// Both halves are needed and neither is inferable. C's type system cannot say
/// which `void *` belongs to which function pointer, and a routine with two of
/// each would be resolved by position -- which is exactly the guess that hands
/// a provider the wrong context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackUse {
    /// The parameter holding the function pointer.
    pub parameter: String,
    /// The parameter carrying the opaque context.
    pub context: String,
}

/// A domain of owned buffers, paired with the routine that releases them.
///
/// The same shape as a handle domain and for the same reason: memory a
/// provider allocated is memory only that provider can free, and which routine
/// owes the release is not something C's types record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferDomain {
    pub name: String,
    /// The routine that releases a buffer of this domain.
    pub destroy: String,
}

/// What a routine does with an owned buffer of some domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferRole {
    /// Returns one, which FOL then owes to the destroy.
    Produces,
    /// Takes one back and releases it.
    Consumes,
}

impl BufferRole {
    pub fn from_keyword(text: &str) -> Option<Self> {
        match text {
            "produces" => Some(Self::Produces),
            "consumes" => Some(Self::Consumes),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Produces => "produces",
            Self::Consumes => "consumes",
        }
    }
}

/// A routine's use of an owned buffer domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedBufferUse {
    pub domain: String,
    pub role: BufferRole,
    /// The parameter carrying the element count: an out-parameter for a
    /// producer, an ordinary one for the destroy.
    pub length: String,
    /// The parameter carrying the allocated capacity, when the provider
    /// reports one. Optional because many providers do not.
    pub capacity: Option<String>,
}

/// A routine's pointer/length pair, named by the overlay.
///
/// C carries a buffer as two unrelated parameters, and nothing in the type
/// system says they belong together: `checksum(const uint8_t *, size_t)` could
/// as easily be a pointer and an unrelated count. Pairing them is what lets
/// the length be *derived* from the FOL value rather than passed beside it,
/// which is the only version a caller cannot get wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferUse {
    /// The parameter holding the address.
    pub parameter: String,
    /// The parameter carrying the element count.
    pub length: String,
}

/// What an overlay says about one pointer parameter.
///
/// C states none of this. A `const char *` might be borrowed for the call or
/// handed over to keep, might accept `NULL` or not, and the compiler cannot
/// tell which by looking. Left undeclared these default to the conservative
/// reading -- non-null, borrowed, valid only for the call -- which is what the
/// projection used to hardcode for every pointer with no way to say otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PointerContract {
    /// `true` when the parameter accepts `NULL`.
    pub nullable: bool,
    /// `true` when ownership transfers to the callee.
    pub transferred: bool,
    /// `true` when the pointer may be retained past the call.
    pub retained: bool,
    /// The declared direction, or `None` to infer it from constness.
    pub direction: Option<crate::interface::AbiDirection>,
}

/// One selected declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineAnnotation {
    /// The exact C symbol.
    pub symbol: String,
    /// The FOL name it is reachable under, defaulting to the symbol.
    pub fol_name: String,
    pub error: ImportErrorConvention,
    pub effects: ImportEffects,
    /// The handle domain this routine produces, borrows, or consumes.
    pub handle: Option<HandleUse>,
    /// The synchronous callback this routine invokes during the call.
    pub callback: Option<CallbackUse>,
    /// The pointer/length pair this routine takes as one buffer.
    pub buffer: Option<BufferUse>,
    /// The owned buffer domain this routine produces or consumes.
    pub owned_buffer: Option<OwnedBufferUse>,
    /// Declared pointer contracts, by parameter name.
    pub pointers: BTreeMap<String, PointerContract>,
}

/// The accepted overlay for one import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationOverlay {
    routines: BTreeMap<String, RoutineAnnotation>,
    handles: BTreeMap<String, HandleDomain>,
    buffers: BTreeMap<String, BufferDomain>,
}

impl AnnotationOverlay {
    /// The declared handle domains, in name order.
    pub fn handles(&self) -> impl Iterator<Item = &HandleDomain> {
        self.handles.values()
    }

    /// One declared handle domain by name.
    pub fn handle(&self, name: &str) -> Option<&HandleDomain> {
        self.handles.get(name)
    }

    /// The declared owned-buffer domains, in name order.
    pub fn buffers(&self) -> impl Iterator<Item = &BufferDomain> {
        self.buffers.values()
    }

    /// One declared owned-buffer domain by name.
    pub fn buffer(&self, name: &str) -> Option<&BufferDomain> {
        self.buffers.get(name)
    }

    /// The annotation for one C symbol, or `None` when the overlay does not
    /// select it. An unselected declaration is not callable, which is how a
    /// header can contain more than FOL imports.
    pub fn routine(&self, symbol: &str) -> Option<&RoutineAnnotation> {
        self.routines.get(symbol)
    }

    /// Selected declarations, in symbol order.
    pub fn routines(&self) -> impl Iterator<Item = &RoutineAnnotation> {
        self.routines.values()
    }

    pub fn len(&self) -> usize {
        self.routines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.routines.is_empty()
    }

    /// Parse and validate one overlay file.
    pub fn parse(text: &str) -> Result<Self, AnnotationError> {
        Parser::new(text).run()
    }
}

/// Why an overlay was refused.
///
/// Every variant names the line, because an overlay is hand-written and a
/// rejection without a location is a scavenger hunt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnnotationError {
    MissingVersion,
    UnsupportedVersion {
        line: u32,
        found: u32,
    },
    UnknownKey {
        line: u32,
        key: String,
    },
    UnknownTable {
        line: u32,
        table: String,
    },
    MalformedLine {
        line: u32,
    },
    DuplicateRoutine {
        line: u32,
        symbol: String,
    },
    InvalidSymbol {
        line: u32,
        symbol: String,
    },
    KeyOutsideTable {
        line: u32,
        key: String,
    },
    MissingKey {
        symbol: String,
        key: &'static str,
    },
    /// A buffer paired with itself as its own length.
    BufferIsItsOwnLength {
        symbol: String,
        parameter: String,
    },
    /// A routine declaring both a borrowed pairing and an owned domain.
    BufferIsBothOwnedAndBorrowed {
        symbol: String,
    },
    /// A second `[buffer.<Name>]` table for one domain.
    DuplicateBufferDomain {
        line: u32,
        domain: String,
    },
    /// A `buffer_role` that names no role.
    UnknownBufferRole {
        line: u32,
        role: String,
    },
    /// A routine naming an owned-buffer domain no table declares.
    UndeclaredBufferDomain {
        symbol: String,
        domain: String,
    },
    /// A consumer of a domain that is not that domain's declared destroy.
    BufferConsumerIsNotTheDestroy {
        symbol: String,
        domain: String,
        destroy: String,
    },
    /// A domain whose destroy is not a selected routine.
    UnboundBufferDestroy {
        domain: String,
        destroy: String,
    },
    /// A destroy that does not declare itself the domain's consumer.
    BufferDestroyRoleMismatch {
        domain: String,
        destroy: String,
    },
    /// A domain with no producer, or with more than one.
    BufferProducerCount {
        domain: String,
        found: usize,
    },
    /// A convention section 4.13 rejects outright rather than approximating.
    RejectedConvention {
        line: u32,
        convention: String,
    },
    UnusedKey {
        symbol: String,
        key: String,
    },
    EmptyStatusSet {
        symbol: String,
        key: &'static str,
    },
    OverlappingStatusCodes {
        symbol: String,
        code: i64,
    },
    UnknownEffect {
        line: u32,
        effect: String,
    },
    UnknownHandleRole {
        line: u32,
        role: String,
    },
    DuplicateHandle {
        line: u32,
        domain: String,
    },
    /// A routine names a handle domain the overlay never declares.
    UndeclaredHandle {
        symbol: String,
        domain: String,
    },
    /// A domain's `destroy` symbol has no `[routine.<symbol>]` table, so the
    /// release FOL would owe is not a call it can make.
    UnboundDestroy {
        domain: String,
        destroy: String,
    },
    /// The declared destroy symbol does not consume the domain it releases.
    DestroyRoleMismatch {
        domain: String,
        destroy: String,
    },
    /// A routine consumes a handle domain that names a different destroy.
    ConsumerIsNotTheDestroy {
        symbol: String,
        domain: String,
        destroy: String,
    },
}

impl AnnotationError {
    /// The registered diagnostic code, per the section 4.13 vocabulary.
    ///
    /// A1004 is the overlay's own code: it says the annotation file is wrong,
    /// as distinct from A1001-A1003, which say a type or a symbol is.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::RejectedConvention { .. } => "A1005",
            _ => "A1004",
        }
    }
}

impl std::fmt::Display for AnnotationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingVersion => write!(
                f,
                "the annotation overlay must start with `version = {ANNOTATION_SCHEMA_VERSION}`"
            ),
            Self::UnsupportedVersion { line, found } => write!(
                f,
                "line {line}: annotation schema version {found} is not supported; \
                 this compiler accepts version {ANNOTATION_SCHEMA_VERSION}"
            ),
            Self::UnknownKey { line, key } => {
                write!(f, "line {line}: unknown annotation key '{key}'")
            }
            Self::UnknownTable { line, table } => write!(
                f,
                "line {line}: unknown annotation table '[{table}]'; expected '[routine.<symbol>]'"
            ),
            Self::MalformedLine { line } => {
                write!(
                    f,
                    "line {line}: expected `key = value` or `[routine.<symbol>]`"
                )
            }
            Self::DuplicateRoutine { line, symbol } => {
                write!(f, "line {line}: '{symbol}' is annotated twice")
            }
            Self::InvalidSymbol { line, symbol } => {
                write!(f, "line {line}: '{symbol}' is not a C identifier")
            }
            Self::KeyOutsideTable { line, key } => write!(
                f,
                "line {line}: '{key}' appears before any '[routine.<symbol>]' table"
            ),
            Self::MissingKey { symbol, key } => {
                write!(f, "routine '{symbol}' is missing required key '{key}'")
            }
            Self::BufferIsItsOwnLength { symbol, parameter } => write!(
                f,
                "routine '{symbol}' names '{parameter}' as both its buffer and its length"
            ),
            Self::BufferIsBothOwnedAndBorrowed { symbol } => write!(
                f,
                "routine '{symbol}' declares both a borrowed buffer and an owned one; a buffer \
                 is lent for the call or handed over, not both"
            ),
            Self::DuplicateBufferDomain { line, domain } => write!(
                f,
                "line {line}: buffer domain '{domain}' is declared twice"
            ),
            Self::UnknownBufferRole { line, role } => write!(
                f,
                "line {line}: '{role}' is not a buffer role; use 'produces' or 'consumes'"
            ),
            Self::UndeclaredBufferDomain { symbol, domain } => write!(
                f,
                "routine '{symbol}' uses buffer domain '{domain}', which no \
                 [buffer.{domain}] table declares"
            ),
            Self::BufferConsumerIsNotTheDestroy {
                symbol,
                domain,
                destroy,
            } => write!(
                f,
                "routine '{symbol}' consumes buffer domain '{domain}', whose destroy is \
                 '{destroy}'; a domain has one release path"
            ),
            Self::UnboundBufferDestroy { domain, destroy } => write!(
                f,
                "buffer domain '{domain}' names destroy '{destroy}', which the overlay does \
                 not select"
            ),
            Self::BufferDestroyRoleMismatch { domain, destroy } => write!(
                f,
                "buffer domain '{domain}' names destroy '{destroy}', which does not declare \
                 itself the consumer of '{domain}'"
            ),
            Self::BufferProducerCount { domain, found } => write!(
                f,
                "buffer domain '{domain}' has {found} producers; exactly one is needed so the \
                 domain has a single origin"
            ),
            Self::RejectedConvention { line, convention } => write!(
                f,
                "line {line}: error convention '{convention}' is rejected; V4 supports \
                 'infallible' and 'status' only, and never guesses errno, a last-error \
                 slot, an undocumented sentinel, unwind, or longjmp"
            ),
            Self::UnusedKey { symbol, key } => write!(
                f,
                "routine '{symbol}' sets '{key}', which its error convention does not use"
            ),
            Self::EmptyStatusSet { symbol, key } => {
                write!(f, "routine '{symbol}' declares an empty '{key}' set")
            }
            Self::OverlappingStatusCodes { symbol, code } => write!(
                f,
                "routine '{symbol}' lists status code {code} as both success and failure"
            ),
            Self::UnknownEffect { line, effect } => write!(
                f,
                "line {line}: unknown effect '{effect}'; expected 'allocates' or 'hosted'"
            ),
            Self::UnknownHandleRole { line, role } => write!(
                f,
                "line {line}: unknown handle role '{role}'; expected 'produces', 'borrows', or 'consumes'"
            ),
            Self::DuplicateHandle { line, domain } => {
                write!(f, "line {line}: handle domain '{domain}' is declared twice")
            }
            Self::UndeclaredHandle { symbol, domain } => write!(
                f,
                "routine '{symbol}' names handle domain '{domain}', which no '[handle.{domain}]' table declares"
            ),
            Self::UnboundDestroy { domain, destroy } => write!(
                f,
                "handle domain '{domain}' names destroy symbol '{destroy}', which is not a selected routine; \
                 FOL would owe a release it cannot call"
            ),
            Self::DestroyRoleMismatch { domain, destroy } => write!(
                f,
                "'{destroy}' is the destroy for handle domain '{domain}' but does not declare \
                 `handle = \"{domain}\"` with `handle_role = \"consumes\"`"
            ),
            Self::ConsumerIsNotTheDestroy {
                symbol,
                domain,
                destroy,
            } => write!(
                f,
                "routine '{symbol}' consumes handle domain '{domain}', whose declared destroy is '{destroy}'; \
                 a domain has exactly one release"
            ),
        }
    }
}

impl std::error::Error for AnnotationError {}

/// Conventions named explicitly so that writing one gets the section 4.13
/// explanation rather than a bare "unknown value".
const REJECTED_CONVENTIONS: &[&str] = &[
    "errno",
    "last_error",
    "sentinel",
    "null_sentinel",
    "unwind",
    "exception",
    "longjmp",
];

/// One `[routine.<symbol>]` table as read, before required-key checking.
#[derive(Default)]
struct PendingRoutine {
    line: u32,
    fol_name: Option<String>,
    error: Option<String>,
    success: Option<Vec<i64>>,
    failure: Option<Vec<i64>>,
    out_parameter: Option<String>,
    effects: ImportEffects,
    handle: Option<String>,
    handle_role: Option<HandleRole>,
    callback: Option<String>,
    callback_context: Option<String>,
    buffer: Option<String>,
    buffer_length: Option<String>,
    buffer_domain: Option<String>,
    buffer_role: Option<BufferRole>,
    buffer_capacity: Option<String>,
    /// `nullable`, `transferred`, and `retained` sets, by parameter name.
    pointers: BTreeMap<String, PointerContract>,
}

/// Which kind of table the parser is currently inside.
#[derive(Clone)]
enum Table {
    Routine(String),
    Handle(String),
    Buffer(String),
}

struct Parser<'a> {
    text: &'a str,
    version: Option<u32>,
    order: Vec<String>,
    pending: BTreeMap<String, PendingRoutine>,
    handles: BTreeMap<String, Option<String>>,
    buffers: BTreeMap<String, Option<String>>,
    current: Option<Table>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            version: None,
            order: Vec::new(),
            pending: BTreeMap::new(),
            handles: BTreeMap::new(),
            buffers: BTreeMap::new(),
            current: None,
        }
    }

    fn run(mut self) -> Result<AnnotationOverlay, AnnotationError> {
        for (index, raw) in self.text.lines().enumerate() {
            let line = index as u32 + 1;
            let content = strip_comment(raw).trim();
            if content.is_empty() {
                continue;
            }
            if let Some(table) = content.strip_prefix('[') {
                self.read_table(line, table)?;
            } else {
                self.read_key(line, content)?;
            }
        }
        self.finish()
    }

    fn read_table(&mut self, line: u32, table: &str) -> Result<(), AnnotationError> {
        let name = table
            .strip_suffix(']')
            .ok_or(AnnotationError::MalformedLine { line })?;
        if let Some(domain) = name.strip_prefix("handle.") {
            let domain = domain.trim();
            if !is_c_identifier(domain) {
                return Err(AnnotationError::InvalidSymbol {
                    line,
                    symbol: domain.to_string(),
                });
            }
            if self.handles.contains_key(domain) {
                return Err(AnnotationError::DuplicateHandle {
                    line,
                    domain: domain.to_string(),
                });
            }
            self.handles.insert(domain.to_string(), None);
            self.current = Some(Table::Handle(domain.to_string()));
            return Ok(());
        }

        if let Some(domain) = name.strip_prefix("buffer.") {
            let domain = domain.trim();
            if !is_c_identifier(domain) {
                return Err(AnnotationError::InvalidSymbol {
                    line,
                    symbol: domain.to_string(),
                });
            }
            if self.buffers.contains_key(domain) {
                return Err(AnnotationError::DuplicateBufferDomain {
                    line,
                    domain: domain.to_string(),
                });
            }
            self.buffers.insert(domain.to_string(), None);
            self.current = Some(Table::Buffer(domain.to_string()));
            return Ok(());
        }

        let symbol = name
            .strip_prefix("routine.")
            .ok_or_else(|| AnnotationError::UnknownTable {
                line,
                table: name.to_string(),
            })?
            .trim();
        if !is_c_identifier(symbol) {
            return Err(AnnotationError::InvalidSymbol {
                line,
                symbol: symbol.to_string(),
            });
        }
        if self.pending.contains_key(symbol) {
            return Err(AnnotationError::DuplicateRoutine {
                line,
                symbol: symbol.to_string(),
            });
        }
        self.pending.insert(
            symbol.to_string(),
            PendingRoutine {
                line,
                ..PendingRoutine::default()
            },
        );
        self.order.push(symbol.to_string());
        self.current = Some(Table::Routine(symbol.to_string()));
        Ok(())
    }

    fn read_key(&mut self, line: u32, content: &str) -> Result<(), AnnotationError> {
        let (key, value) = content
            .split_once('=')
            .ok_or(AnnotationError::MalformedLine { line })?;
        let key = key.trim();
        let value = value.trim();

        let Some(table) = self.current.clone() else {
            if key == "version" {
                let parsed = parse_integer(value).ok_or(AnnotationError::MalformedLine { line })?;
                let parsed =
                    u32::try_from(parsed).map_err(|_| AnnotationError::MalformedLine { line })?;
                if parsed != ANNOTATION_SCHEMA_VERSION {
                    return Err(AnnotationError::UnsupportedVersion {
                        line,
                        found: parsed,
                    });
                }
                self.version = Some(parsed);
                return Ok(());
            }
            return Err(AnnotationError::KeyOutsideTable {
                line,
                key: key.to_string(),
            });
        };

        let symbol = match table {
            Table::Handle(domain) => {
                if key != "destroy" {
                    return Err(AnnotationError::UnknownKey {
                        line,
                        key: key.to_string(),
                    });
                }
                let destroy = parse_string(value).ok_or(AnnotationError::MalformedLine { line })?;
                if !is_c_identifier(&destroy) {
                    return Err(AnnotationError::InvalidSymbol {
                        line,
                        symbol: destroy,
                    });
                }
                self.handles.insert(domain, Some(destroy));
                return Ok(());
            }
            Table::Buffer(domain) => {
                if key != "destroy" {
                    return Err(AnnotationError::UnknownKey {
                        line,
                        key: key.to_string(),
                    });
                }
                let destroy = parse_string(value).ok_or(AnnotationError::MalformedLine { line })?;
                if !is_c_identifier(&destroy) {
                    return Err(AnnotationError::InvalidSymbol {
                        line,
                        symbol: destroy,
                    });
                }
                self.buffers.insert(domain, Some(destroy));
                return Ok(());
            }
            Table::Routine(symbol) => symbol,
        };

        let routine = self
            .pending
            .get_mut(&symbol)
            .expect("the current table was inserted when it was opened");
        match key {
            "handle" => {
                let domain = parse_string(value).ok_or(AnnotationError::MalformedLine { line })?;
                routine.handle = Some(domain);
            }
            "handle_role" => {
                let role = parse_string(value).ok_or(AnnotationError::MalformedLine { line })?;
                let parsed = HandleRole::from_keyword(&role);
                routine.handle_role =
                    Some(parsed.ok_or(AnnotationError::UnknownHandleRole { line, role })?);
            }
            "callback" => {
                routine.callback =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "callback_context" => {
                routine.callback_context =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "buffer" => {
                routine.buffer =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "buffer_length" => {
                routine.buffer_length =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "buffer_domain" => {
                routine.buffer_domain =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "buffer_role" => {
                let role = parse_string(value).ok_or(AnnotationError::MalformedLine { line })?;
                routine.buffer_role = Some(
                    BufferRole::from_keyword(&role)
                        .ok_or(AnnotationError::UnknownBufferRole { line, role })?,
                );
            }
            "buffer_capacity" => {
                routine.buffer_capacity =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            // The pointer contracts C cannot state. Each names the parameters
            // it applies to, so one routine declares all of its pointers
            // without a table per parameter.
            "nullable" | "transferred" | "retained" | "reads" | "writes" | "reads_writes" => {
                let names =
                    parse_string_array(value).ok_or(AnnotationError::MalformedLine { line })?;
                for name in names {
                    let contract = routine.pointers.entry(name).or_default();
                    match key {
                        "nullable" => contract.nullable = true,
                        "transferred" => contract.transferred = true,
                        "retained" => contract.retained = true,
                        "reads" => contract.direction = Some(crate::interface::AbiDirection::In),
                        "writes" => contract.direction = Some(crate::interface::AbiDirection::Out),
                        _ => contract.direction = Some(crate::interface::AbiDirection::InOut),
                    }
                }
            }
            "fol_name" => {
                routine.fol_name =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "error" => {
                let convention =
                    parse_string(value).ok_or(AnnotationError::MalformedLine { line })?;
                if REJECTED_CONVENTIONS.contains(&convention.as_str()) {
                    return Err(AnnotationError::RejectedConvention { line, convention });
                }
                if convention != "infallible" && convention != "status" {
                    return Err(AnnotationError::RejectedConvention { line, convention });
                }
                routine.error = Some(convention);
            }
            "status_ok" => {
                routine.success = Some(
                    parse_integer_array(value).ok_or(AnnotationError::MalformedLine { line })?,
                );
            }
            "status_error" => {
                routine.failure = Some(
                    parse_integer_array(value).ok_or(AnnotationError::MalformedLine { line })?,
                );
            }
            "out" => {
                routine.out_parameter =
                    Some(parse_string(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "effects" => {
                let effects =
                    parse_string_array(value).ok_or(AnnotationError::MalformedLine { line })?;
                for effect in effects {
                    match effect.as_str() {
                        "allocates" => routine.effects.allocates = true,
                        "hosted" => routine.effects.hosted = true,
                        _ => return Err(AnnotationError::UnknownEffect { line, effect }),
                    }
                }
            }
            other => {
                return Err(AnnotationError::UnknownKey {
                    line,
                    key: other.to_string(),
                })
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<AnnotationOverlay, AnnotationError> {
        if self.version.is_none() {
            return Err(AnnotationError::MissingVersion);
        }
        let mut routines = BTreeMap::new();
        for (symbol, pending) in self.pending {
            routines.insert(symbol.clone(), pending.resolve(&symbol)?);
        }

        let mut handles = BTreeMap::new();
        for (name, destroy) in self.handles {
            let destroy = destroy.ok_or(AnnotationError::MissingKey {
                symbol: name.clone(),
                key: "destroy",
            })?;
            handles.insert(name.clone(), HandleDomain { name, destroy });
        }

        // The three cross-checks that make a handle domain trustworthy. Each
        // catches an overlay that would compile into a program owing a release
        // it cannot perform.
        for routine in routines.values() {
            let Some(use_) = &routine.handle else {
                continue;
            };
            let Some(domain) = handles.get(&use_.domain) else {
                return Err(AnnotationError::UndeclaredHandle {
                    symbol: routine.symbol.clone(),
                    domain: use_.domain.clone(),
                });
            };
            if use_.role == HandleRole::Consumes && domain.destroy != routine.symbol {
                return Err(AnnotationError::ConsumerIsNotTheDestroy {
                    symbol: routine.symbol.clone(),
                    domain: domain.name.clone(),
                    destroy: domain.destroy.clone(),
                });
            }
        }
        for domain in handles.values() {
            let Some(destroy) = routines.get(&domain.destroy) else {
                return Err(AnnotationError::UnboundDestroy {
                    domain: domain.name.clone(),
                    destroy: domain.destroy.clone(),
                });
            };
            let consumes = destroy.handle.as_ref().is_some_and(|use_| {
                use_.domain == domain.name && use_.role == HandleRole::Consumes
            });
            if !consumes {
                return Err(AnnotationError::DestroyRoleMismatch {
                    domain: domain.name.clone(),
                    destroy: domain.destroy.clone(),
                });
            }
        }

        let mut buffers = BTreeMap::new();
        for (name, destroy) in self.buffers {
            let destroy = destroy.ok_or(AnnotationError::MissingKey {
                symbol: name.clone(),
                key: "destroy",
            })?;
            buffers.insert(name.clone(), BufferDomain { name, destroy });
        }

        // The same three cross-checks a handle domain gets, for the same
        // reason: memory a provider allocated is memory only that provider can
        // free, and an overlay that gets the pairing wrong compiles into a
        // program owing a release it cannot perform.
        for routine in routines.values() {
            let Some(use_) = &routine.owned_buffer else {
                continue;
            };
            let Some(domain) = buffers.get(&use_.domain) else {
                return Err(AnnotationError::UndeclaredBufferDomain {
                    symbol: routine.symbol.clone(),
                    domain: use_.domain.clone(),
                });
            };
            if use_.role == BufferRole::Consumes && domain.destroy != routine.symbol {
                return Err(AnnotationError::BufferConsumerIsNotTheDestroy {
                    symbol: routine.symbol.clone(),
                    domain: domain.name.clone(),
                    destroy: domain.destroy.clone(),
                });
            }
        }
        for domain in buffers.values() {
            let Some(destroy) = routines.get(&domain.destroy) else {
                return Err(AnnotationError::UnboundBufferDestroy {
                    domain: domain.name.clone(),
                    destroy: domain.destroy.clone(),
                });
            };
            let consumes = destroy.owned_buffer.as_ref().is_some_and(|use_| {
                use_.domain == domain.name && use_.role == BufferRole::Consumes
            });
            if !consumes {
                return Err(AnnotationError::BufferDestroyRoleMismatch {
                    domain: domain.name.clone(),
                    destroy: domain.destroy.clone(),
                });
            }
            // One producer, so the domain has exactly one origin. Two would
            // mean two allocation paths sharing one release, which the
            // provider may or may not support and the overlay cannot promise.
            let producers = routines
                .values()
                .filter(|routine| {
                    routine.owned_buffer.as_ref().is_some_and(|use_| {
                        use_.domain == domain.name && use_.role == BufferRole::Produces
                    })
                })
                .count();
            if producers != 1 {
                return Err(AnnotationError::BufferProducerCount {
                    domain: domain.name.clone(),
                    found: producers,
                });
            }
        }

        Ok(AnnotationOverlay {
            routines,
            handles,
            buffers,
        })
    }
}

impl PendingRoutine {
    fn resolve(self, symbol: &str) -> Result<RoutineAnnotation, AnnotationError> {
        let convention = self.error.ok_or(AnnotationError::MissingKey {
            symbol: symbol.to_string(),
            key: "error",
        })?;
        let error = match convention.as_str() {
            "infallible" => {
                // Naming a status set on an infallible routine means the
                // author believed it could fail; taking the keys silently
                // would make that belief invisible.
                for (value, key) in [
                    (self.success.is_some(), "status_ok"),
                    (self.failure.is_some(), "status_error"),
                    (self.out_parameter.is_some(), "out"),
                ] {
                    if value {
                        return Err(AnnotationError::UnusedKey {
                            symbol: symbol.to_string(),
                            key: key.to_string(),
                        });
                    }
                }
                ImportErrorConvention::Infallible
            }
            _ => {
                let success = self.success.ok_or(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "status_ok",
                })?;
                let failure = self.failure.ok_or(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "status_error",
                })?;
                let out_parameter = self.out_parameter.ok_or(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "out",
                })?;
                for (set, key) in [(&success, "status_ok"), (&failure, "status_error")] {
                    if set.is_empty() {
                        return Err(AnnotationError::EmptyStatusSet {
                            symbol: symbol.to_string(),
                            key,
                        });
                    }
                }
                if let Some(code) = success.iter().find(|code| failure.contains(code)) {
                    return Err(AnnotationError::OverlappingStatusCodes {
                        symbol: symbol.to_string(),
                        code: *code,
                    });
                }
                ImportErrorConvention::Status {
                    success,
                    failure,
                    out_parameter,
                }
            }
        };
        let _ = self.line;
        // The two handle keys are one fact spelled in two places, so neither
        // half stands alone: a domain without a role says nothing about
        // ownership, and a role without a domain names nothing to release.
        let handle = match (self.handle, self.handle_role) {
            (Some(domain), Some(role)) => Some(HandleUse { domain, role }),
            (Some(_), None) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "handle_role",
                })
            }
            (None, Some(_)) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "handle",
                })
            }
            (None, None) => None,
        };
        // Same rule as the handle pair, for the same reason: a function pointer
        // with no named context has nothing to pass, and a context with no
        // named function pointer names nothing to pass it to.
        let callback = match (self.callback, self.callback_context) {
            (Some(parameter), Some(context)) => Some(CallbackUse { parameter, context }),
            (Some(_), None) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "callback_context",
                })
            }
            (None, Some(_)) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "callback",
                })
            }
            (None, None) => None,
        };
        // An owned buffer names a domain and a role, like a handle, plus the
        // parameter its length comes back through. `buffer_length` means the
        // same thing in both spellings -- where this routine's buffer reports
        // its extent -- so it is shared rather than duplicated.
        let owned_buffer = match (self.buffer_domain, self.buffer_role) {
            (Some(domain), Some(role)) => {
                if self.buffer.is_some() {
                    return Err(AnnotationError::BufferIsBothOwnedAndBorrowed {
                        symbol: symbol.to_string(),
                    });
                }
                let length = self
                    .buffer_length
                    .clone()
                    .ok_or(AnnotationError::MissingKey {
                        symbol: symbol.to_string(),
                        key: "buffer_length",
                    })?;
                Some(OwnedBufferUse {
                    domain,
                    role,
                    length,
                    capacity: self.buffer_capacity,
                })
            }
            (Some(_), None) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "buffer_role",
                })
            }
            (None, Some(_)) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "buffer_domain",
                })
            }
            (None, None) => None,
        };
        // Same rule again: an address with no named length is a buffer whose
        // extent nobody knows, and a length with no named address counts
        // nothing.
        let buffer = match (
            self.buffer,
            if owned_buffer.is_some() {
                None
            } else {
                self.buffer_length
            },
        ) {
            (Some(parameter), Some(length)) => {
                // Pairing a parameter with itself would make the length its
                // own extent, which is not a shape C can produce.
                if parameter == length {
                    return Err(AnnotationError::BufferIsItsOwnLength {
                        symbol: symbol.to_string(),
                        parameter,
                    });
                }
                Some(BufferUse { parameter, length })
            }
            (Some(_), None) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "buffer_length",
                })
            }
            (None, Some(_)) => {
                return Err(AnnotationError::MissingKey {
                    symbol: symbol.to_string(),
                    key: "buffer",
                })
            }
            (None, None) => None,
        };
        Ok(RoutineAnnotation {
            symbol: symbol.to_string(),
            fol_name: self.fol_name.unwrap_or_else(|| symbol.to_string()),
            error,
            effects: self.effects,
            handle,
            callback,
            buffer,
            owned_buffer,
            pointers: self.pointers,
        })
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(index) => &line[..index],
        None => line,
    }
}

fn parse_string(value: &str) -> Option<String> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('"')).then(|| inner.to_string())
}

fn parse_integer(value: &str) -> Option<i64> {
    value.parse().ok()
}

fn parse_integer_array(value: &str) -> Option<Vec<i64>> {
    array_items(value)?
        .iter()
        .map(|item| parse_integer(item))
        .collect()
}

fn parse_string_array(value: &str) -> Option<Vec<String>> {
    array_items(value)?
        .iter()
        .map(|item| parse_string(item))
        .collect()
}

fn array_items(value: &str) -> Option<Vec<&str>> {
    let inner = value.strip_prefix('[')?.strip_suffix(']')?.trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    Some(inner.split(',').map(str::trim).collect())
}

fn is_c_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCALAR_OVERLAY: &str = r#"
version = 1

# a call that cannot fail
[routine.c_math_add_one]
fol_name = "add_one"
error = "infallible"

[routine.c_math_checked_div]
error = "status"
status_ok = [0]
status_error = [1, 2]
out = "result"
effects = ["allocates"]
"#;

    #[test]
    fn an_overlay_selects_only_the_routines_it_names() {
        let overlay = AnnotationOverlay::parse(SCALAR_OVERLAY).expect("overlay should parse");

        assert_eq!(overlay.len(), 2);
        assert!(overlay.routine("c_math_add_one").is_some());
        // A declaration the header has but the overlay does not name is not
        // callable, which is what lets a header carry more than FOL imports.
        assert!(overlay.routine("c_math_unlisted").is_none());
    }

    #[test]
    fn a_routine_defaults_its_fol_name_to_its_symbol() {
        let overlay = AnnotationOverlay::parse(SCALAR_OVERLAY).expect("overlay should parse");

        assert_eq!(
            overlay
                .routine("c_math_add_one")
                .expect("selected")
                .fol_name,
            "add_one"
        );
        assert_eq!(
            overlay
                .routine("c_math_checked_div")
                .expect("selected")
                .fol_name,
            "c_math_checked_div"
        );
    }

    #[test]
    fn a_status_mapping_carries_its_codes_and_out_parameter() {
        let overlay = AnnotationOverlay::parse(SCALAR_OVERLAY).expect("overlay should parse");
        let routine = overlay.routine("c_math_checked_div").expect("selected");

        assert!(routine.error.is_recoverable());
        assert_eq!(
            routine.error,
            ImportErrorConvention::Status {
                success: vec![0],
                failure: vec![1, 2],
                out_parameter: "result".to_string(),
            }
        );
        assert!(routine.effects.allocates);
        assert!(!routine.effects.hosted);
    }

    #[test]
    fn every_guessed_error_convention_is_refused_by_name() {
        for convention in REJECTED_CONVENTIONS {
            let text = format!("version = 1\n[routine.f]\nerror = \"{convention}\"\n");
            let error =
                AnnotationOverlay::parse(&text).expect_err("a guessed convention must be refused");
            assert_eq!(
                error,
                AnnotationError::RejectedConvention {
                    line: 3,
                    convention: convention.to_string(),
                },
                "'{convention}' should be refused by name"
            );
            assert_eq!(error.diagnostic_code(), "A1005");
        }
    }

    #[test]
    fn an_unknown_key_is_refused_rather_than_ignored() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[routine.f]\nerror = \"infallible\"\nownership = \"transferred\"\n",
        )
        .expect_err("an unknown key must be refused");

        assert_eq!(
            error,
            AnnotationError::UnknownKey {
                line: 4,
                key: "ownership".to_string(),
            }
        );
    }

    #[test]
    fn an_overlay_without_a_version_is_refused() {
        let error = AnnotationOverlay::parse("[routine.f]\nerror = \"infallible\"\n")
            .expect_err("a versionless overlay must be refused");
        assert_eq!(error, AnnotationError::MissingVersion);
    }

    #[test]
    fn a_future_schema_version_is_refused_rather_than_read() {
        let error = AnnotationOverlay::parse("version = 2\n").expect_err("version 2 is not ours");
        assert_eq!(
            error,
            AnnotationError::UnsupportedVersion { line: 1, found: 2 }
        );
    }

    #[test]
    fn an_incomplete_status_mapping_is_refused() {
        for (text, expected) in [
            (
                "version = 1\n[routine.f]\nerror = \"status\"\n",
                AnnotationError::MissingKey {
                    symbol: "f".to_string(),
                    key: "status_ok",
                },
            ),
            (
                "version = 1\n[routine.f]\nerror = \"status\"\nstatus_ok = [0]\n",
                AnnotationError::MissingKey {
                    symbol: "f".to_string(),
                    key: "status_error",
                },
            ),
            (
                "version = 1\n[routine.f]\nerror = \"status\"\nstatus_ok = [0]\nstatus_error = [1]\n",
                AnnotationError::MissingKey {
                    symbol: "f".to_string(),
                    key: "out",
                },
            ),
        ] {
            assert_eq!(
                AnnotationOverlay::parse(text).expect_err("an incomplete mapping must be refused"),
                expected
            );
        }
    }

    #[test]
    fn a_status_code_cannot_mean_both_success_and_failure() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[routine.f]\nerror = \"status\"\nstatus_ok = [0, 1]\nstatus_error = [1]\nout = \"v\"\n",
        )
        .expect_err("an overlapping code must be refused");

        assert_eq!(
            error,
            AnnotationError::OverlappingStatusCodes {
                symbol: "f".to_string(),
                code: 1,
            }
        );
    }

    #[test]
    fn an_infallible_routine_cannot_carry_status_keys() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[routine.f]\nerror = \"infallible\"\nstatus_ok = [0]\n",
        )
        .expect_err("a contradictory annotation must be refused");

        assert_eq!(
            error,
            AnnotationError::UnusedKey {
                symbol: "f".to_string(),
                key: "status_ok".to_string(),
            }
        );
    }

    #[test]
    fn a_routine_annotated_twice_is_refused() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[routine.f]\nerror = \"infallible\"\n[routine.f]\nerror = \"infallible\"\n",
        )
        .expect_err("a duplicate table must be refused");

        assert_eq!(
            error,
            AnnotationError::DuplicateRoutine {
                line: 4,
                symbol: "f".to_string(),
            }
        );
    }

    #[test]
    fn an_unknown_effect_is_refused() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[routine.f]\nerror = \"infallible\"\neffects = [\"threads\"]\n",
        )
        .expect_err("an unknown effect must be refused");

        assert_eq!(
            error,
            AnnotationError::UnknownEffect {
                line: 4,
                effect: "threads".to_string(),
            }
        );
    }

    #[test]
    fn a_table_that_is_not_a_routine_is_refused() {
        let error = AnnotationOverlay::parse("version = 1\n[provider]\npath = \"x\"\n")
            .expect_err("an unknown table must be refused");

        assert_eq!(
            error,
            AnnotationError::UnknownTable {
                line: 2,
                table: "provider".to_string(),
            }
        );
    }

    #[test]
    fn keys_before_any_routine_table_are_refused() {
        let error = AnnotationOverlay::parse("version = 1\nerror = \"infallible\"\n")
            .expect_err("a stray key must be refused");

        assert_eq!(
            error,
            AnnotationError::KeyOutsideTable {
                line: 2,
                key: "error".to_string(),
            }
        );
    }

    #[test]
    fn routines_are_reported_in_symbol_order_whatever_the_file_order() {
        let overlay = AnnotationOverlay::parse(
            "version = 1\n[routine.zeta]\nerror = \"infallible\"\n[routine.alpha]\nerror = \"infallible\"\n",
        )
        .expect("overlay should parse");

        let symbols: Vec<&str> = overlay
            .routines()
            .map(|routine| routine.symbol.as_str())
            .collect();
        assert_eq!(symbols, vec!["alpha", "zeta"]);
    }

    /// A complete handle domain: one producer, one borrower, one destroy.
    const HANDLE_OVERLAY: &str = "version = 1\n\
         [handle.Widget]\n\
         destroy = \"widget_free\"\n\
         [routine.widget_new]\n\
         error = \"infallible\"\n\
         handle = \"Widget\"\n\
         handle_role = \"produces\"\n\
         effects = [\"allocates\"]\n\
         [routine.widget_size]\n\
         error = \"infallible\"\n\
         handle = \"Widget\"\n\
         handle_role = \"borrows\"\n\
         [routine.widget_free]\n\
         error = \"infallible\"\n\
         handle = \"Widget\"\n\
         handle_role = \"consumes\"\n";

    #[test]
    fn a_handle_domain_names_its_producer_borrower_and_destroy() {
        let overlay = AnnotationOverlay::parse(HANDLE_OVERLAY).expect("overlay should parse");

        let domain = overlay.handle("Widget").expect("the domain is declared");
        assert_eq!(domain.destroy, "widget_free");

        let roles: Vec<(&str, HandleRole)> = overlay
            .routines()
            .filter_map(|routine| {
                routine
                    .handle
                    .as_ref()
                    .map(|use_| (routine.symbol.as_str(), use_.role))
            })
            .collect();
        assert_eq!(
            roles,
            vec![
                ("widget_free", HandleRole::Consumes),
                ("widget_new", HandleRole::Produces),
                ("widget_size", HandleRole::Borrows),
            ]
        );
    }

    /// A domain whose destroy is not a selected routine is refused.
    ///
    /// Accepting it would produce a FOL program owing a release it has no call
    /// to perform -- a leak the type system would insist on and the overlay
    /// made impossible.
    #[test]
    fn a_destroy_that_is_not_a_selected_routine_is_refused() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[handle.Widget]\ndestroy = \"widget_free\"\n\
             [routine.widget_new]\nerror = \"infallible\"\n\
             handle = \"Widget\"\nhandle_role = \"produces\"\n",
        )
        .expect_err("an unbound destroy must be refused");

        assert_eq!(
            error,
            AnnotationError::UnboundDestroy {
                domain: "Widget".to_string(),
                destroy: "widget_free".to_string(),
            }
        );
    }

    /// A second consumer of one domain is refused: a domain has one release.
    #[test]
    fn only_the_declared_destroy_may_consume_a_domain() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[handle.Widget]\ndestroy = \"widget_free\"\n\
             [routine.widget_free]\nerror = \"infallible\"\n\
             handle = \"Widget\"\nhandle_role = \"consumes\"\n\
             [routine.widget_close]\nerror = \"infallible\"\n\
             handle = \"Widget\"\nhandle_role = \"consumes\"\n",
        )
        .expect_err("a second consumer must be refused");

        assert_eq!(
            error,
            AnnotationError::ConsumerIsNotTheDestroy {
                symbol: "widget_close".to_string(),
                domain: "Widget".to_string(),
                destroy: "widget_free".to_string(),
            }
        );
    }

    /// Naming a domain no `[handle.<Name>]` table declares is refused.
    #[test]
    fn an_undeclared_handle_domain_is_refused() {
        let error = AnnotationOverlay::parse(
            "version = 1\n[routine.widget_new]\nerror = \"infallible\"\n\
             handle = \"Widget\"\nhandle_role = \"produces\"\n",
        )
        .expect_err("an undeclared domain must be refused");

        assert_eq!(
            error,
            AnnotationError::UndeclaredHandle {
                symbol: "widget_new".to_string(),
                domain: "Widget".to_string(),
            }
        );
    }

    /// Each half of the handle fact requires the other.
    #[test]
    fn a_handle_domain_and_its_role_are_one_fact() {
        assert_eq!(
            AnnotationOverlay::parse(
                "version = 1\n[routine.f]\nerror = \"infallible\"\nhandle = \"Widget\"\n"
            )
            .expect_err("a domain without a role must be refused"),
            AnnotationError::MissingKey {
                symbol: "f".to_string(),
                key: "handle_role",
            }
        );
        assert_eq!(
            AnnotationOverlay::parse(
                "version = 1\n[routine.f]\nerror = \"infallible\"\nhandle_role = \"borrows\"\n"
            )
            .expect_err("a role without a domain must be refused"),
            AnnotationError::MissingKey {
                symbol: "f".to_string(),
                key: "handle",
            }
        );
    }

    #[test]
    fn an_unknown_handle_role_is_refused_by_name() {
        assert_eq!(
            AnnotationOverlay::parse(
                "version = 1\n[routine.f]\nerror = \"infallible\"\n\
                 handle = \"Widget\"\nhandle_role = \"steals\"\n"
            )
            .expect_err("an unknown role must be refused"),
            AnnotationError::UnknownHandleRole {
                line: 5,
                role: "steals".to_string(),
            }
        );
    }

    #[test]
    fn a_handle_table_accepts_only_its_destroy_key() {
        assert_eq!(
            AnnotationOverlay::parse("version = 1\n[handle.Widget]\ndestroy = \"f\"\nsize = 8\n")
                .expect_err("a stray key must be refused"),
            AnnotationError::UnknownKey {
                line: 4,
                key: "size".to_string(),
            }
        );
    }
    /// The three contracts C cannot state, declared per parameter.
    #[test]
    fn an_overlay_declares_pointer_contracts_by_parameter() {
        let overlay = AnnotationOverlay::parse(
            "version = 1\n\
             [routine.widget_apply]\n\
             fol_name = \"apply\"\n\
             error = \"infallible\"\n\
             nullable = [\"maybe\"]\n\
             transferred = [\"owned\"]\n\
             retained = [\"kept\", \"owned\"]\n",
        )
        .expect("the overlay should parse");
        let routine = overlay.routine("widget_apply").expect("the routine");

        let maybe = routine.pointers.get("maybe").expect("maybe is declared");
        assert!(maybe.nullable);
        assert!(!maybe.transferred && !maybe.retained);

        // One parameter may carry more than one contract.
        let owned = routine.pointers.get("owned").expect("owned is declared");
        assert!(owned.transferred && owned.retained);
        assert!(!owned.nullable);

        let kept = routine.pointers.get("kept").expect("kept is declared");
        assert!(kept.retained);
        assert!(!kept.nullable && !kept.transferred);

        // An undeclared parameter has no entry, and the projection reads that
        // as the conservative default rather than as a missing declaration.
        assert!(!routine.pointers.contains_key("plain"));
    }

    /// A contract key whose value is not a list of names is a malformed line,
    /// not a silently ignored one.
    #[test]
    fn a_malformed_pointer_contract_is_refused() {
        let error = AnnotationOverlay::parse(
            "version = 1\n\
             [routine.widget_apply]\n\
             fol_name = \"apply\"\n\
             error = \"infallible\"\n\
             nullable = \"maybe\"\n",
        )
        .expect_err("a bare string is not a list of parameter names");
        assert!(
            matches!(error, AnnotationError::MalformedLine { .. }),
            "{error:?}"
        );
    }
}

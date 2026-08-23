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

/// One selected declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineAnnotation {
    /// The exact C symbol.
    pub symbol: String,
    /// The FOL name it is reachable under, defaulting to the symbol.
    pub fol_name: String,
    pub error: ImportErrorConvention,
    pub effects: ImportEffects,
}

/// The accepted overlay for one import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnnotationOverlay {
    routines: BTreeMap<String, RoutineAnnotation>,
}

impl AnnotationOverlay {
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
    UnsupportedVersion { line: u32, found: u32 },
    UnknownKey { line: u32, key: String },
    UnknownTable { line: u32, table: String },
    MalformedLine { line: u32 },
    DuplicateRoutine { line: u32, symbol: String },
    InvalidSymbol { line: u32, symbol: String },
    KeyOutsideTable { line: u32, key: String },
    MissingKey { symbol: String, key: &'static str },
    /// A convention section 4.13 rejects outright rather than approximating.
    RejectedConvention { line: u32, convention: String },
    UnusedKey { symbol: String, key: String },
    EmptyStatusSet { symbol: String, key: &'static str },
    OverlappingStatusCodes { symbol: String, code: i64 },
    UnknownEffect { line: u32, effect: String },
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
                write!(f, "line {line}: expected `key = value` or `[routine.<symbol>]`")
            }
            Self::DuplicateRoutine { line, symbol } => {
                write!(f, "line {line}: '{symbol}' is annotated twice")
            }
            Self::InvalidSymbol { line, symbol } => write!(
                f,
                "line {line}: '{symbol}' is not a C identifier"
            ),
            Self::KeyOutsideTable { line, key } => write!(
                f,
                "line {line}: '{key}' appears before any '[routine.<symbol>]' table"
            ),
            Self::MissingKey { symbol, key } => {
                write!(f, "routine '{symbol}' is missing required key '{key}'")
            }
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
}

struct Parser<'a> {
    text: &'a str,
    version: Option<u32>,
    order: Vec<String>,
    pending: BTreeMap<String, PendingRoutine>,
    current: Option<String>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            version: None,
            order: Vec::new(),
            pending: BTreeMap::new(),
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
        let name = table.strip_suffix(']').ok_or(AnnotationError::MalformedLine { line })?;
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
        self.current = Some(symbol.to_string());
        Ok(())
    }

    fn read_key(&mut self, line: u32, content: &str) -> Result<(), AnnotationError> {
        let (key, value) = content
            .split_once('=')
            .ok_or(AnnotationError::MalformedLine { line })?;
        let key = key.trim();
        let value = value.trim();

        let Some(symbol) = self.current.clone() else {
            if key == "version" {
                let parsed = parse_integer(value).ok_or(AnnotationError::MalformedLine { line })?;
                let parsed = u32::try_from(parsed)
                    .map_err(|_| AnnotationError::MalformedLine { line })?;
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

        let routine = self
            .pending
            .get_mut(&symbol)
            .expect("the current table was inserted when it was opened");
        match key {
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
                routine.success =
                    Some(parse_integer_array(value).ok_or(AnnotationError::MalformedLine { line })?);
            }
            "status_error" => {
                routine.failure =
                    Some(parse_integer_array(value).ok_or(AnnotationError::MalformedLine { line })?);
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
        Ok(AnnotationOverlay { routines })
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
        Ok(RoutineAnnotation {
            symbol: symbol.to_string(),
            fol_name: self.fol_name.unwrap_or_else(|| symbol.to_string()),
            error,
            effects: self.effects,
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
    array_items(value)?.iter().map(|item| parse_integer(item)).collect()
}

fn parse_string_array(value: &str) -> Option<Vec<String>> {
    array_items(value)?.iter().map(|item| parse_string(item)).collect()
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
            overlay.routine("c_math_add_one").expect("selected").fol_name,
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
            let text = format!(
                "version = 1\n[routine.f]\nerror = \"{convention}\"\n"
            );
            let error = AnnotationOverlay::parse(&text)
                .expect_err("a guessed convention must be refused");
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
        assert_eq!(error, AnnotationError::UnsupportedVersion { line: 1, found: 2 });
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
}

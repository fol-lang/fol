//! The pre-emission verifier.
//!
//! Walks a declaration's whole type graph and reports every non-projectable
//! type with the exact path to the offending nested field. It runs before
//! backend emission so the backend never has to rediscover ABI legality --
//! which is what M4's STOP forbids.
//!
//! The walk is over a caller-supplied description rather than a compiler type,
//! because `fol-abi` may not depend on `fol-typecheck` or `fol-lower`. The
//! compiler describes what it has; this crate decides whether it is legal.

use crate::compat::{AbiClassification, AbiRejection};

/// What the compiler tells the verifier about one type.
///
/// Deliberately structural: `fol-abi` cannot see `CheckedType`, so the caller
/// projects into this and the answer comes back with a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateType {
    /// A sized integer that already passed width checks.
    Int {
        spelling: String,
        bits: Option<u16>,
    },
    Float {
        spelling: String,
    },
    Bool,
    Char {
        encoding: String,
    },
    /// A UTF-8 string the caller owns and lends for the duration of one call.
    ///
    /// Legal as a parameter and not as a result. Inbound, C hands over a
    /// pointer and a length, the wrapper validates them, and FOL copies into
    /// its own owning string -- so the caller's pointer is never retained.
    /// Outbound there is nothing to lend: handing C a pointer into FOL's own
    /// string raises who-frees-it, which is the owned-buffer contract in
    /// section 12.4 rather than a borrowed view.
    BorrowedString,
    /// A named record with its fields in declaration order.
    Record {
        name: Option<String>,
        fields: Vec<(String, CandidateType)>,
    },
    /// A named entry with its variants in declaration order.
    Entry {
        name: Option<String>,
        /// `None` on a variant means a tag with no explicit discriminant.
        variants: Vec<(String, Option<i64>, Option<CandidateType>)>,
    },
    /// A raw address token with its four contracts.
    RawPointer {
        target: Box<CandidateType>,
        mutability: Option<bool>,
        nullability: Option<bool>,
        ownership: Option<bool>,
        escape: Option<bool>,
        /// Required when ownership transfers.
        destructor: Option<String>,
    },
    /// A managed pointer: unique, shared, or weak.
    ManagedPointer {
        spelling: String,
    },
    /// An internal container with no projection.
    Container {
        spelling: String,
    },
    /// A routine, protocol, or closure object.
    RoutineObject {
        spelling: String,
    },
    /// A channel, eventual, mutex, or task.
    ConcurrencyObject {
        spelling: String,
    },
    /// A generic declaration or an unsubstituted parameter.
    Generic {
        name: String,
    },
    /// A packed, bitfield, or flexible-array form.
    UnsupportedLayout {
        detail: String,
    },
    Void,
}

/// Verify one type, reporting every problem found anywhere in its graph.
///
/// Returns all classifications rather than the first: a record with three bad
/// fields should report three, so one build round fixes all of them.
/// Where a type sits in a signature.
///
/// Some shapes are legal in one position and not another. A borrowed view is
/// the case that forces the distinction: a caller can lend a buffer for the
/// duration of a call, but a callee cannot lend one back without answering who
/// frees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiPosition {
    Parameter,
    Result,
    Error,
}

impl AbiPosition {
    /// Whether a value in this position outlives the call.
    ///
    /// A parameter does not: the caller owns it and the call borrows it. A
    /// result and an error do, because the caller reads them after the call
    /// returns.
    pub const fn outlives_call(self) -> bool {
        matches!(self, Self::Result | Self::Error)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Result => "result",
            Self::Error => "error",
        }
    }
}

/// Verify a type in parameter position.
///
/// Kept as the plain name because most callers verify parameters, and because
/// this is the permissive position: anything legal here is legal to check
/// without knowing more.
pub fn verify_type(root: &str, candidate: &CandidateType) -> Vec<AbiClassification> {
    verify_type_at(root, candidate, AbiPosition::Parameter)
}

/// Verify a type in a known signature position.
pub fn verify_type_at(
    root: &str,
    candidate: &CandidateType,
    position: AbiPosition,
) -> Vec<AbiClassification> {
    let mut found = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    walk(&mut found, &mut seen, vec![root.to_string()], candidate);

    // A borrowed view is checked against the position separately from the
    // structural walk: the shape is fine either way, and it is only the
    // direction that makes it illegal.
    if position.outlives_call() && borrows(candidate) {
        found.push(AbiClassification::new(
            vec![root.to_string()],
            AbiRejection::BorrowedViewOutlivesCall {
                position: position.as_str().to_string(),
            },
        ));
    }
    found
}

/// Whether a type lends memory it does not own.
fn borrows(candidate: &CandidateType) -> bool {
    match candidate {
        CandidateType::BorrowedString => true,
        // A record carrying a view lends it just as surely as a bare one.
        CandidateType::Record { fields, .. } => fields.iter().any(|(_, field)| borrows(field)),
        CandidateType::Entry { variants, .. } => variants
            .iter()
            .any(|(_, _, payload)| payload.as_ref().is_some_and(borrows)),
        _ => false,
    }
}

fn walk(
    found: &mut Vec<AbiClassification>,
    // Names currently being walked, for by-value recursion detection.
    active: &mut Vec<String>,
    path: Vec<String>,
    candidate: &CandidateType,
) {
    let reject = |found: &mut Vec<AbiClassification>, rejection| {
        found.push(AbiClassification::new(path.clone(), rejection));
    };

    match candidate {
        CandidateType::Int { spelling, bits } => match bits {
            None => reject(
                found,
                AbiRejection::ArchitectureSizedNumeric {
                    spelling: spelling.clone(),
                },
            ),
            Some(128) => reject(
                found,
                AbiRejection::OversizedInteger {
                    spelling: spelling.clone(),
                },
            ),
            Some(_) => {}
        },
        CandidateType::Float { .. } | CandidateType::Bool | CandidateType::Void => {}
        CandidateType::Char { encoding } => {
            if encoding != "utf32" {
                reject(
                    found,
                    AbiRejection::UnsupportedCharacterEncoding {
                        encoding: encoding.clone(),
                    },
                );
            }
        }
        CandidateType::Generic { name } => {
            reject(found, AbiRejection::Generic { name: name.clone() })
        }
        // Structurally always fine: a pointer and a length, both validated by
        // the wrapper. Whether it is *legal here* is a position question, and
        // `verify_type_at` answers that one.
        CandidateType::BorrowedString => {}
        CandidateType::Container { spelling } => reject(
            found,
            AbiRejection::InternalContainer {
                spelling: spelling.clone(),
            },
        ),
        CandidateType::ManagedPointer { spelling } => reject(
            found,
            AbiRejection::UnwrappedPointer {
                spelling: spelling.clone(),
            },
        ),
        CandidateType::RoutineObject { spelling } => reject(
            found,
            AbiRejection::RoutineOrProtocolObject {
                spelling: spelling.clone(),
            },
        ),
        CandidateType::ConcurrencyObject { spelling } => reject(
            found,
            AbiRejection::ConcurrencyObject {
                spelling: spelling.clone(),
            },
        ),
        CandidateType::UnsupportedLayout { detail } => reject(
            found,
            AbiRejection::UnsupportedLayout {
                detail: detail.clone(),
            },
        ),
        CandidateType::RawPointer {
            target,
            mutability,
            nullability,
            ownership,
            escape,
            destructor,
        } => {
            for (value, name) in [
                (mutability, "mutability"),
                (nullability, "nullability"),
                (ownership, "ownership"),
                (escape, "escape"),
            ] {
                if value.is_none() {
                    reject(
                        found,
                        AbiRejection::IncompletePointerContract {
                            missing: name.to_string(),
                        },
                    );
                }
            }
            // Ownership transfer without a destructor names no one to release
            // the resource, which is a leak the signature cannot express.
            if ownership == &Some(true) && destructor.is_none() {
                reject(
                    found,
                    AbiRejection::IncompletePointerContract {
                        missing: "paired destroy routine".to_string(),
                    },
                );
            }
            let mut nested = path.clone();
            nested.push("*".to_string());
            walk(found, active, nested, target);
        }
        CandidateType::Record { name, fields } => {
            let Some(name) = name else {
                reject(found, AbiRejection::AnonymousAggregate);
                return;
            };
            if active.contains(name) {
                reject(found, AbiRejection::RecursiveByValue { name: name.clone() });
                return;
            }
            active.push(name.clone());
            for (field_name, field) in fields {
                let mut nested = path.clone();
                nested.push(field_name.clone());
                walk(found, active, nested, field);
            }
            active.pop();
        }
        CandidateType::Entry { name, variants } => {
            let Some(name) = name else {
                reject(found, AbiRejection::AnonymousAggregate);
                return;
            };
            if variants.iter().any(|(_, tag, _)| tag.is_none()) {
                reject(
                    found,
                    AbiRejection::UnstableEntryTag {
                        entry: name.clone(),
                    },
                );
            }
            if active.contains(name) {
                reject(found, AbiRejection::RecursiveByValue { name: name.clone() });
                return;
            }
            active.push(name.clone());
            for (variant_name, _, payload) in variants {
                if let Some(payload) = payload {
                    let mut nested = path.clone();
                    nested.push(variant_name.clone());
                    walk(found, active, nested, payload);
                }
            }
            active.pop();
        }
    }
}

/// Verify a whole export set: every type, every symbol, and uniqueness.
pub fn verify_export_set(exports: &[(String, String, CandidateType)]) -> Vec<AbiClassification> {
    let mut found = Vec::new();
    for (fol_path, symbol, candidate) in exports {
        if let Some(rejection) = crate::compat::classify_external_symbol(symbol) {
            found.push(AbiClassification::new(vec![fol_path.clone()], rejection));
        }
        found.extend(verify_type(fol_path, candidate));
    }
    let symbols: Vec<String> = exports
        .iter()
        .map(|(_, symbol, _)| symbol.clone())
        .collect();
    found.extend(crate::compat::classify_duplicate_symbols(&symbols));
    found
}

#[cfg(test)]
mod code_tests {
    use crate::compat::AbiRejection;

    /// Every rejection reports under a registered code, and each of the three
    /// codes has at least one producer.
    #[test]
    fn every_rejection_maps_to_a_registered_code() {
        let cases = [
            (
                AbiRejection::InternalContainer {
                    spelling: String::new(),
                },
                "A1001",
            ),
            (
                AbiRejection::Generic {
                    name: String::new(),
                },
                "A1001",
            ),
            (AbiRejection::AnonymousAggregate, "A1001"),
            (
                AbiRejection::InvalidExternalSymbol {
                    symbol: String::new(),
                    reason: String::new(),
                },
                "A1002",
            ),
            (
                AbiRejection::IncompletePointerContract {
                    missing: String::new(),
                },
                "A1003",
            ),
        ];
        for (rejection, expected) in cases {
            assert_eq!(rejection.diagnostic_code(), expected);
        }
    }
}

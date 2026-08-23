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
pub fn verify_type(root: &str, candidate: &CandidateType) -> Vec<AbiClassification> {
    let mut found = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    walk(&mut found, &mut seen, vec![root.to_string()], candidate);
    found
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

//! The required negative classifier cases from section 9.
//!
//! One test per class the plan enumerates. Each asserts the reason code and
//! that the message says something a reader can act on, because a classifier
//! that rejects without explaining is a wall.

use fol_abi::*;

/// Every rejection renders a message naming the problem.
fn message(rejection: &AbiRejection) -> String {
    rejection.to_string()
}

#[test]
fn default_and_architecture_sized_numerics_are_rejected() {
    let rejection = AbiRejection::ArchitectureSizedNumeric {
        spelling: "arch".to_string(),
    };
    assert_eq!(rejection.reason(), "architecture-sized-numeric");
    assert!(message(&rejection).contains("pointer width"));
}

#[test]
fn oversized_integers_are_rejected() {
    let rejection = AbiRejection::OversizedInteger {
        spelling: "i128".to_string(),
    };
    assert_eq!(rejection.reason(), "oversized-integer");
    assert!(message(&rejection).contains("no portable C counterpart"));
}

#[test]
fn unsupported_character_encodings_are_rejected() {
    let rejection = AbiRejection::UnsupportedCharacterEncoding {
        encoding: "utf16".to_string(),
    };
    assert_eq!(rejection.reason(), "unsupported-character-encoding");
    assert!(message(&rejection).contains("utf32"));
}

#[test]
fn generic_declarations_and_parameters_are_rejected() {
    let rejection = AbiRejection::Generic {
        name: "Pair[T]".to_string(),
    };
    assert_eq!(rejection.reason(), "generic");
    // The message must say what to do instead, or the rule reads as arbitrary.
    assert!(message(&rejection).contains("non-generic"));
}

#[test]
fn anonymous_aggregates_are_rejected() {
    let rejection = AbiRejection::AnonymousAggregate;
    assert_eq!(rejection.reason(), "anonymous-aggregate");
    assert!(message(&rejection).contains("no name"));
}

#[test]
fn unstable_entry_tags_are_rejected() {
    let rejection = AbiRejection::UnstableEntryTag {
        entry: "Shape".to_string(),
    };
    assert_eq!(rejection.reason(), "unstable-entry-tag");
    assert!(message(&rejection).contains("renumber"));
}

#[test]
fn internal_containers_without_a_projection_are_rejected() {
    for spelling in ["str", "vec[int]", "map[str,int]", "opt[int]", "seq[int]"] {
        let rejection = AbiRejection::InternalContainer {
            spelling: spelling.to_string(),
        };
        assert_eq!(rejection.reason(), "internal-container");
        assert!(message(&rejection).contains("no canonical C projection"));
    }
}

#[test]
fn owning_and_shared_pointers_are_rejected() {
    let rejection = AbiRejection::UnwrappedPointer {
        spelling: "ptr[shared, int]".to_string(),
    };
    assert_eq!(rejection.reason(), "unwrapped-pointer");
    assert!(message(&rejection).contains("raw address token"));
}

#[test]
fn a_raw_pointer_missing_its_facts_is_rejected() {
    for missing in [
        "mutability",
        "nullability",
        "ownership",
        "escape",
        "destructor",
    ] {
        let rejection = AbiRejection::IncompletePointerContract {
            missing: missing.to_string(),
        };
        assert_eq!(rejection.reason(), "incomplete-pointer-contract");
        assert!(message(&rejection).contains(missing));
        assert!(message(&rejection).contains("cannot infer"));
    }
}

#[test]
fn routine_and_protocol_objects_are_rejected() {
    for spelling in ["fun(int): int", "shape::geometry", "closure"] {
        let rejection = AbiRejection::RoutineOrProtocolObject {
            spelling: spelling.to_string(),
        };
        assert_eq!(rejection.reason(), "routine-or-protocol-object");
        assert!(message(&rejection).contains("no C representation"));
    }
}

#[test]
fn concurrency_objects_are_rejected() {
    for spelling in ["chn[tx,int]", "chn[rx,int]", "evt[int]", "mux[int]", "task"] {
        let rejection = AbiRejection::ConcurrencyObject {
            spelling: spelling.to_string(),
        };
        assert_eq!(rejection.reason(), "concurrency-object");
        assert!(message(&rejection).contains("does not cross the boundary"));
    }
}

#[test]
fn recursive_by_value_aggregates_are_rejected() {
    let rejection = AbiRejection::RecursiveByValue {
        name: "Node".to_string(),
    };
    assert_eq!(rejection.reason(), "recursive-by-value");
    assert!(message(&rejection).contains("no finite C layout"));
}

#[test]
fn packed_bitfield_and_flexible_forms_are_rejected() {
    let rejection = AbiRejection::UnsupportedLayout {
        detail: "a bitfield has no portable layout".to_string(),
    };
    assert_eq!(rejection.reason(), "unsupported-layout");
    assert!(message(&rejection).contains("bitfield"));
}

#[test]
fn capabilities_stronger_than_the_artifact_model_are_rejected() {
    let rejection = AbiRejection::CapabilityTooStrong {
        detail: "the routine needs heap allocation and the artifact is fol_model = core"
            .to_string(),
    };
    assert_eq!(rejection.reason(), "capability-too-strong");
    assert!(message(&rejection).contains("core"));
}

/// Invalid, reserved, and duplicate external symbols.
#[test]
fn external_symbols_are_validated() {
    assert!(classify_external_symbol("fol_demo_add").is_none());
    assert!(classify_external_symbol("_leading_underscore").is_none());

    for (symbol, expected) in [
        ("", "empty"),
        ("fol_démo", "ASCII"),
        ("9lives", "starts with"),
        ("has space", "letters, digits"),
        ("has-dash", "letters, digits"),
    ] {
        let rejection = classify_external_symbol(symbol)
            .unwrap_or_else(|| panic!("'{symbol}' should be rejected"));
        assert!(
            rejection.to_string().contains(expected),
            "'{symbol}': expected a message about {expected}, got {rejection}"
        );
    }
}

/// C reserves keywords, anything with a double underscore, and `_` plus an
/// uppercase letter.
#[test]
fn reserved_c_identifiers_are_rejected() {
    for symbol in ["int", "struct", "return", "main", "fol__add", "_Atomic"] {
        assert!(
            is_reserved_c_identifier(symbol),
            "'{symbol}' should be reserved"
        );
        let rejection = classify_external_symbol(symbol)
            .unwrap_or_else(|| panic!("'{symbol}' should be rejected"));
        assert_eq!(rejection.reason(), "invalid-external-symbol");
    }
    assert!(!is_reserved_c_identifier("fol_add"));
    assert!(!is_reserved_c_identifier("_fol_add"));
}

#[test]
fn duplicate_external_symbols_are_rejected() {
    let symbols = vec![
        "fol_demo_add".to_string(),
        "fol_demo_sub".to_string(),
        "fol_demo_add".to_string(),
    ];
    let found = classify_duplicate_symbols(&symbols);
    assert_eq!(found.len(), 1);
    assert!(found[0].rejection.to_string().contains("more than once"));
    assert!(classify_duplicate_symbols(&symbols[..2]).is_empty());
}

/// The classification names the exact nested field, not just the declaration.
///
/// For a record of a record of a `vec`, "not projectable" is useless and
/// `Outer.middle.items` is actionable.
#[test]
fn a_classification_reports_the_full_nested_path() {
    let classification = AbiClassification::new(
        vec![
            "Outer".to_string(),
            "middle".to_string(),
            "items".to_string(),
        ],
        AbiRejection::InternalContainer {
            spelling: "vec[int]".to_string(),
        },
    );
    assert_eq!(classification.rendered_path(), "Outer.middle.items");
    let rendered = classification.to_string();
    assert!(rendered.starts_with("Outer.middle.items: "));
    assert!(rendered.contains("vec[int]"));
}

/// Every reason code is distinct, so a diagnostic can key on it.
#[test]
fn reason_codes_are_unique() {
    let reasons = [
        AbiRejection::ArchitectureSizedNumeric {
            spelling: String::new(),
        },
        AbiRejection::OversizedInteger {
            spelling: String::new(),
        },
        AbiRejection::UnsupportedCharacterEncoding {
            encoding: String::new(),
        },
        AbiRejection::Generic {
            name: String::new(),
        },
        AbiRejection::AnonymousAggregate,
        AbiRejection::UnstableEntryTag {
            entry: String::new(),
        },
        AbiRejection::InternalContainer {
            spelling: String::new(),
        },
        AbiRejection::UnwrappedPointer {
            spelling: String::new(),
        },
        AbiRejection::IncompletePointerContract {
            missing: String::new(),
        },
        AbiRejection::RoutineOrProtocolObject {
            spelling: String::new(),
        },
        AbiRejection::ConcurrencyObject {
            spelling: String::new(),
        },
        AbiRejection::RecursiveByValue {
            name: String::new(),
        },
        AbiRejection::UnsupportedLayout {
            detail: String::new(),
        },
        AbiRejection::InvalidExternalSymbol {
            symbol: String::new(),
            reason: String::new(),
        },
        AbiRejection::CapabilityTooStrong {
            detail: String::new(),
        },
    ];
    let mut codes: Vec<&str> = reasons.iter().map(AbiRejection::reason).collect();
    let total = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), total, "two rejections share a reason code");
    // All twelve required classes from section 9 are represented.
    assert!(total >= 12);
}

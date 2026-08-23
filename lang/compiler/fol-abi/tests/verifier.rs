//! The pre-emission verifier walks nested aggregates and points at the exact
//! bad field.

use fol_abi::*;

fn int() -> CandidateType {
    CandidateType::Int {
        spelling: "i32".to_string(),
        bits: Some(32),
    }
}

#[test]
fn a_projectable_type_reports_nothing() {
    let record = CandidateType::Record {
        name: Some("Point".to_string()),
        fields: vec![("x".to_string(), int()), ("y".to_string(), int())],
    };
    assert_eq!(verify_type("Point", &record), Vec::new());
}

/// The classifier walks nested aggregates and names the exact bad field.
#[test]
fn the_verifier_points_at_the_exact_nested_field() {
    let inner = CandidateType::Record {
        name: Some("Middle".to_string()),
        fields: vec![
            ("ok".to_string(), int()),
            (
                "items".to_string(),
                CandidateType::Container {
                    spelling: "vec[int]".to_string(),
                },
            ),
        ],
    };
    let outer = CandidateType::Record {
        name: Some("Outer".to_string()),
        fields: vec![("middle".to_string(), inner)],
    };

    let found = verify_type("Outer", &outer);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rendered_path(), "Outer.middle.items");
    assert_eq!(found[0].rejection.reason(), "internal-container");
}

/// Every problem is reported, not just the first, so one build round fixes all.
#[test]
fn every_bad_field_is_reported() {
    let record = CandidateType::Record {
        name: Some("Bag".to_string()),
        fields: vec![
            (
                "a".to_string(),
                CandidateType::Container {
                    spelling: "str".to_string(),
                },
            ),
            (
                "b".to_string(),
                CandidateType::RoutineObject {
                    spelling: "fun()".to_string(),
                },
            ),
            (
                "c".to_string(),
                CandidateType::ConcurrencyObject {
                    spelling: "mux[int]".to_string(),
                },
            ),
        ],
    };
    let found = verify_type("Bag", &record);
    assert_eq!(found.len(), 3);
    let paths: Vec<String> = found.iter().map(AbiClassification::rendered_path).collect();
    assert_eq!(paths, vec!["Bag.a", "Bag.b", "Bag.c"]);
}

#[test]
fn architecture_sized_and_oversized_integers_are_caught() {
    let arch = CandidateType::Int {
        spelling: "arch".to_string(),
        bits: None,
    };
    assert_eq!(
        verify_type("f", &arch)[0].rejection.reason(),
        "architecture-sized-numeric"
    );

    let huge = CandidateType::Int {
        spelling: "i128".to_string(),
        bits: Some(128),
    };
    assert_eq!(
        verify_type("f", &huge)[0].rejection.reason(),
        "oversized-integer"
    );
}

#[test]
fn only_utf32_characters_pass() {
    for (encoding, expected) in [("utf32", 0), ("utf8", 1), ("utf16", 1)] {
        let candidate = CandidateType::Char {
            encoding: encoding.to_string(),
        };
        assert_eq!(verify_type("c", &candidate).len(), expected, "{encoding}");
    }
}

#[test]
fn an_anonymous_aggregate_is_caught() {
    let record = CandidateType::Record {
        name: None,
        fields: vec![("x".to_string(), int())],
    };
    assert_eq!(
        verify_type("f", &record)[0].rejection.reason(),
        "anonymous-aggregate"
    );
}

#[test]
fn an_entry_without_explicit_tags_is_caught() {
    let entry = CandidateType::Entry {
        name: Some("Shape".to_string()),
        variants: vec![
            ("circle".to_string(), Some(0), None),
            ("square".to_string(), None, None),
        ],
    };
    assert_eq!(
        verify_type("Shape", &entry)[0].rejection.reason(),
        "unstable-entry-tag"
    );

    let tagged = CandidateType::Entry {
        name: Some("Shape".to_string()),
        variants: vec![
            ("circle".to_string(), Some(0), None),
            ("square".to_string(), Some(1), None),
        ],
    };
    assert!(verify_type("Shape", &tagged).is_empty());
}

/// A raw pointer must carry all four contracts, plus a destructor when
/// ownership transfers.
#[test]
fn an_incomplete_raw_pointer_contract_is_caught() {
    let bare = CandidateType::RawPointer {
        target: Box::new(int()),
        mutability: None,
        nullability: None,
        ownership: None,
        escape: None,
        destructor: None,
    };
    let found = verify_type("p", &bare);
    assert_eq!(found.len(), 4);
    for classification in &found {
        assert_eq!(
            classification.rejection.reason(),
            "incomplete-pointer-contract"
        );
    }

    let complete = CandidateType::RawPointer {
        target: Box::new(int()),
        mutability: Some(false),
        nullability: Some(false),
        ownership: Some(false),
        escape: Some(false),
        destructor: None,
    };
    assert!(verify_type("p", &complete).is_empty());

    // Ownership transfer with no destructor names nobody to release it.
    let transferred = CandidateType::RawPointer {
        target: Box::new(int()),
        mutability: Some(true),
        nullability: Some(false),
        ownership: Some(true),
        escape: Some(true),
        destructor: None,
    };
    let found = verify_type("p", &transferred);
    assert_eq!(found.len(), 1);
    assert!(found[0].rejection.to_string().contains("destroy routine"));
}

/// The verifier follows a pointer's target.
#[test]
fn a_bad_pointer_target_is_reported_at_its_own_path() {
    let pointer = CandidateType::RawPointer {
        target: Box::new(CandidateType::Container {
            spelling: "vec[int]".to_string(),
        }),
        mutability: Some(false),
        nullability: Some(false),
        ownership: Some(false),
        escape: Some(false),
        destructor: None,
    };
    let found = verify_type("p", &pointer);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rendered_path(), "p.*");
}

/// An aggregate containing itself by value has no finite C layout.
#[test]
fn recursive_by_value_aggregates_are_caught() {
    // `Node { next: Node }` -- built by hand, since the real compiler rejects
    // it earlier; the verifier must not recurse forever if one reaches it.
    let inner = CandidateType::Record {
        name: Some("Node".to_string()),
        fields: vec![("value".to_string(), int())],
    };
    let outer = CandidateType::Record {
        name: Some("Node".to_string()),
        fields: vec![("next".to_string(), inner)],
    };
    let found = verify_type("Node", &outer);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].rejection.reason(), "recursive-by-value");
}

/// A whole export set: types, symbols, and uniqueness together.
#[test]
fn an_export_set_is_verified_end_to_end() {
    let exports = vec![
        ("api::add".to_string(), "fol_demo_add".to_string(), int()),
        (
            "api::bad".to_string(),
            "int".to_string(),
            CandidateType::Container {
                spelling: "vec[int]".to_string(),
            },
        ),
        ("api::again".to_string(), "fol_demo_add".to_string(), int()),
    ];

    let found = verify_export_set(&exports);
    let reasons: Vec<&str> = found.iter().map(|f| f.rejection.reason()).collect();

    // `int` is a reserved C keyword, `vec[int]` has no projection, and
    // `fol_demo_add` is exported twice.
    assert!(reasons.contains(&"invalid-external-symbol"));
    assert!(reasons.contains(&"internal-container"));
    assert_eq!(
        reasons
            .iter()
            .filter(|reason| **reason == "invalid-external-symbol")
            .count(),
        2,
        "the reserved name and the duplicate are both reported"
    );
}

/// A clean export set passes.
#[test]
fn a_clean_export_set_passes() {
    let exports = vec![
        ("api::add".to_string(), "fol_demo_add".to_string(), int()),
        ("api::sub".to_string(), "fol_demo_sub".to_string(), int()),
    ];
    assert_eq!(verify_export_set(&exports), Vec::new());
}

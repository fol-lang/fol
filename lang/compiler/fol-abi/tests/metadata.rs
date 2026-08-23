//! The metadata API editors and build completion read.

use fol_abi::*;

/// One source for the section 4.6 matrix.
///
/// Section 9 forbids duplicating it in editor code: two copies drift, and the
/// copy people see would be the wrong one.
#[test]
fn the_matrix_covers_every_scalar_spelling() {
    let rows = scalar_matrix();
    let spellings: Vec<&str> = rows.iter().map(|row| row.fol_spelling).collect();

    for width in fol_types::IntWidth::ALL {
        assert!(
            spellings.contains(&width.as_str()),
            "{} is missing from the matrix",
            width.as_str()
        );
    }
    for encoding in fol_types::CharEncoding::ALL {
        assert!(spellings.contains(&encoding.as_str()));
    }
    assert!(spellings.contains(&"bol"));
}

/// The projection column comes from the model that emits it, so it cannot
/// drift.
#[test]
fn projections_agree_with_the_type_model() {
    assert_eq!(c_projection_for("i32"), Some("int32_t"));
    assert_eq!(c_projection_for("u8"), Some("uint8_t"));
    assert_eq!(c_projection_for("f64"), Some("double"));
    assert_eq!(c_projection_for("bol"), Some("fol_bool_t"));
    assert_eq!(c_projection_for("utf32"), Some("fol_char_t"));
}

/// Unsupported spellings are listed with a reason rather than omitted, so an
/// editor can say why instead of showing nothing.
#[test]
fn unsupported_rows_carry_a_reason() {
    for spelling in ["arch", "uarch", "i128", "u128", "utf8", "utf16"] {
        let row = full_matrix()
            .into_iter()
            .find(|row| row.fol_spelling == spelling)
            .unwrap_or_else(|| panic!("{spelling} should be listed"));
        assert!(!row.supported, "{spelling} should not be supported");
        assert!(!row.note.is_empty(), "{spelling} needs a reason");
        assert_eq!(c_projection_for(spelling), None);
    }
}

#[test]
fn aggregate_rows_cover_the_supported_and_rejected_shapes() {
    let rows = aggregate_matrix();
    let supported: Vec<&str> = rows
        .iter()
        .filter(|row| row.supported)
        .map(|row| row.fol_spelling)
        .collect();
    assert!(supported.contains(&"named record"));
    assert!(supported.contains(&"named entry"));
    assert!(supported.contains(&"ptr[raw, T]"));
    assert!(supported.contains(&"ptr[raw, mut, T]"));

    let rejected: Vec<&str> = rows
        .iter()
        .filter(|row| !row.supported)
        .map(|row| row.fol_spelling)
        .collect();
    assert!(rejected.iter().any(|row| row.contains("vec")));
    assert!(rejected.iter().any(|row| row.contains("chn")));
    assert!(rejected.iter().any(|row| row.contains("shared")));
}

/// The status values match section 4.7 and the frozen header in section 4.16.
#[test]
fn status_values_match_the_frozen_contract() {
    let by_name: Vec<(&str, i32)> = STATUS_VALUES
        .iter()
        .map(|(name, value, _)| (*name, *value))
        .collect();
    assert_eq!(
        by_name,
        vec![
            ("FOL_STATUS_OK", 0),
            ("FOL_STATUS_REPORT", 1),
            ("FOL_STATUS_INVALID_ARGUMENT", -1),
            ("FOL_STATUS_PANIC", -2),
            ("FOL_STATUS_INTERNAL", -3),
        ]
    );
    for (_, _, description) in STATUS_VALUES {
        assert!(!description.is_empty());
    }
}

/// Every row is either supported with a projection, or unsupported with a
/// reason. A row that is neither tells a reader nothing.
#[test]
fn every_row_is_actionable() {
    for row in full_matrix() {
        if row.supported {
            assert_ne!(
                row.c_projection, "-",
                "{} has no projection",
                row.fol_spelling
            );
        } else {
            assert!(!row.note.is_empty(), "{} has no reason", row.fol_spelling);
        }
    }
}

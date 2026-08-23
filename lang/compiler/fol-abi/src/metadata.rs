//! Compiler metadata for editors and build completion.
//!
//! One source for "what can cross the C boundary". The editor and the build
//! language both need to answer that, and section 9 forbids duplicating the
//! section 4.6 matrix in editor code -- two copies drift, and the copy people
//! see would be the wrong one.

use crate::types::AbiScalar;

/// One row of the section 4.6 type matrix, in a form an editor can render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiTypeMatrixRow {
    /// How FOL spells it.
    pub fol_spelling: &'static str,
    /// How it appears in generated C.
    pub c_projection: &'static str,
    /// Whether it crosses today, or is named here only so a diagnostic can
    /// refer to it.
    pub supported: bool,
    /// Why, when it does not.
    pub note: &'static str,
}

/// Every scalar spelling and what it projects to.
///
/// Built from `AbiScalar::c_type` where a row is supported, so the projection
/// column cannot drift from the model that emits it.
pub fn scalar_matrix() -> Vec<AbiTypeMatrixRow> {
    let mut rows = Vec::new();
    for width in fol_types::IntWidth::ALL {
        let supported = width.bits().is_some_and(|bits| bits <= 64);
        rows.push(AbiTypeMatrixRow {
            fol_spelling: width.as_str(),
            c_projection: if supported {
                // Leaked to `'static` once per width; the set is fixed and
                // small, and an editor asks for it repeatedly.
                Box::leak(AbiScalar::Int(*width).c_type().into_boxed_str())
            } else {
                "-"
            },
            supported,
            note: match width.bits() {
                None => "architecture-sized; a stable ABI cannot depend on the compiling host",
                Some(128) => "no portable C counterpart",
                Some(_) => "",
            },
        });
    }
    for width in fol_types::FloatWidth::ALL {
        let supported = *width != fol_types::FloatWidth::Arch;
        rows.push(AbiTypeMatrixRow {
            fol_spelling: width.as_str(),
            c_projection: if supported {
                Box::leak(AbiScalar::Float(*width).c_type().into_boxed_str())
            } else {
                "-"
            },
            supported,
            note: if supported {
                ""
            } else {
                "architecture-sized; a stable ABI cannot depend on the compiling host"
            },
        });
    }
    rows.push(AbiTypeMatrixRow {
        fol_spelling: "bol",
        c_projection: "fol_bool_t",
        supported: true,
        note: "only 0 and 1 are valid; imports validate",
    });
    for encoding in fol_types::CharEncoding::ALL {
        rows.push(AbiTypeMatrixRow {
            fol_spelling: encoding.as_str(),
            c_projection: if encoding.crosses_c_boundary() {
                "fol_char_t"
            } else {
                "-"
            },
            supported: encoding.crosses_c_boundary(),
            note: if encoding.crosses_c_boundary() {
                "a Unicode scalar value; imports validate"
            } else {
                "a code unit that may be part of a sequence, with no single-scalar projection"
            },
        });
    }
    rows
}

/// The aggregate and reference rows.
pub fn aggregate_matrix() -> Vec<AbiTypeMatrixRow> {
    vec![
        AbiTypeMatrixRow {
            fol_spelling: "named record",
            c_projection: "generated struct",
            supported: true,
            note: "source field order, target layout, no hidden fields",
        },
        AbiTypeMatrixRow {
            fol_spelling: "named entry",
            c_projection: "tag plus payload union",
            supported: true,
            note: "fixed tag width and explicit stable discriminants",
        },
        AbiTypeMatrixRow {
            fol_spelling: "str",
            c_projection: "{const uint8_t *ptr; size_t len;}",
            supported: true,
            note: "UTF-8, call-scoped, never retained unless stated",
        },
        AbiTypeMatrixRow {
            fol_spelling: "ptr[raw, T]",
            c_projection: "const T *",
            supported: true,
            note: "non-null; `opt ptr[raw, T]` is the nullable form",
        },
        AbiTypeMatrixRow {
            fol_spelling: "ptr[raw, mut, T]",
            c_projection: "T *",
            supported: true,
            note: "non-null and writable",
        },
        AbiTypeMatrixRow {
            fol_spelling: "vec / map / set / seq / opt",
            c_projection: "-",
            supported: false,
            note: "internal representations with no canonical projection",
        },
        AbiTypeMatrixRow {
            fol_spelling: "ptr[shared] / ptr[weak]",
            c_projection: "-",
            supported: false,
            note: "managed pointers; only a raw address token crosses",
        },
        AbiTypeMatrixRow {
            fol_spelling: "chn / evt / mux / task",
            c_projection: "-",
            supported: false,
            note: "concurrency objects do not cross the boundary",
        },
        AbiTypeMatrixRow {
            fol_spelling: "routine / closure / standard",
            c_projection: "-",
            supported: false,
            note: "no C representation; a callback is a function pointer plus context",
        },
    ]
}

/// Everything an editor needs to describe the boundary.
pub fn full_matrix() -> Vec<AbiTypeMatrixRow> {
    let mut rows = scalar_matrix();
    rows.extend(aggregate_matrix());
    rows
}

/// The C spelling for one FOL spelling, or `None` when it does not cross.
pub fn c_projection_for(fol_spelling: &str) -> Option<&'static str> {
    full_matrix()
        .into_iter()
        .find(|row| row.fol_spelling == fol_spelling && row.supported)
        .map(|row| row.c_projection)
}

/// The reserved status values from section 4.7, for completion and docs.
pub const STATUS_VALUES: &[(&str, i32, &str)] = &[
    ("FOL_STATUS_OK", 0, "success"),
    (
        "FOL_STATUS_REPORT",
        1,
        "a FOL recoverable report; the typed error out parameter is initialized",
    ),
    (
        "FOL_STATUS_INVALID_ARGUMENT",
        -1,
        "invalid foreign argument: null, tag, boolean, Unicode, length",
    ),
    (
        "FOL_STATUS_PANIC",
        -2,
        "a contained FOL or implementation panic",
    ),
    (
        "FOL_STATUS_INTERNAL",
        -3,
        "an internal wrapper or runtime failure",
    ),
];

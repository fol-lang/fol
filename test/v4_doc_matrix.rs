//! The C boundary as the documents describe it, checked against what ships.
//!
//! M9 claimed the README, architecture, and book "present exactly the shipped
//! matrix". It was true when written and false four milestones later: M10-M13
//! moved the boundary, and three documents went on describing where it used to
//! be. The README still said opaque handles and callbacks "cannot yet be
//! exported"; `ARCHITECTURE.md` still listed importing C structs as outside
//! V4. Nothing was checking, because nothing tied prose to code.
//!
//! The tie is the **example**. A shape that crosses has a package that proves
//! it crosses -- built, linked, and run by a lane in `make verify` -- so the
//! examples are the one description of the boundary that cannot drift from the
//! code without a test going red.
//!
//! Two guards, which fail in opposite directions:
//!
//! - Every `examples/v4_c_*` package appears in `SHAPES` below. Adding an
//!   example without classifying it fails, so a new crossing shape cannot
//!   arrive unnoticed.
//! - A shape whose example exists must be described as crossing, and none of
//!   its `retired` phrases -- sentences that were true before it crossed --
//!   may survive in any document.
//!
//! Run by `make test-v4-doc-matrix`.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Where the boundary is described in prose.
const DOCUMENTS: &[&str] = &[
    "README.md",
    "ARCHITECTURE.md",
    "book/src/950_interop/_index.md",
];

#[derive(PartialEq)]
enum Kind {
    /// A shape that crosses the C boundary. Its row is load-bearing.
    Crossing,
    /// A package that proves something else -- a contract, a probe, a
    /// lifecycle. It describes no new shape, so it owes the matrix nothing.
    Supporting,
}

struct Shape {
    /// The package that proves it, under `examples/`.
    example: &'static str,
    kind: Kind,
    /// Phrases the interop chapter must carry for this shape. Empty for a
    /// supporting package.
    documented_as: &'static [&'static str],
    /// Sentences that were true before this shape crossed, and are now false
    /// wherever they appear. This is where a retired exclusion goes to die.
    retired: &'static [&'static str],
}

/// Every `examples/v4_c_*` package, classified.
///
/// A new example forces a new row, and a `Crossing` row forces the chapter to
/// describe it. That is the whole guard: the cost of moving the boundary
/// includes saying so.
const SHAPES: &[Shape] = &[
    Shape {
        example: "v4_c_export_scalar",
        kind: Kind::Crossing,
        documented_as: &["the exact-width C scalar", "FOL_STATUS_PANIC"],
        retired: &[],
    },
    Shape {
        example: "v4_c_record",
        kind: Kind::Crossing,
        documented_as: &["field order preserved"],
        retired: &[],
    },
    Shape {
        example: "v4_c_string_view",
        kind: Kind::Crossing,
        documented_as: &["fol_str_view_t"],
        retired: &[],
    },
    Shape {
        example: "v4_c_import_scalar",
        kind: Kind::Crossing,
        documented_as: &["the measured FOL scalar"],
        retired: &[],
    },
    Shape {
        example: "v4_c_opaque_handle",
        kind: Kind::Crossing,
        documented_as: &["a `lin` linear resource with a paired destroy routine"],
        retired: &[],
    },
    Shape {
        example: "v4_c_callback",
        kind: Kind::Crossing,
        documented_as: &["a FOL closure invoked by the provider"],
        retired: &[],
    },
    // M10: C structs and enums inbound. The exclusion it retired outlived it
    // in `ARCHITECTURE.md` by three milestones.
    Shape {
        example: "v4_c_roundtrip_fol",
        kind: Kind::Crossing,
        documented_as: &["named structs and enums, as parameters"],
        retired: &[
            "importing C structs\n  and enums",
            "importing named aggregates",
        ],
    },
    // M11: handles and callbacks outbound. The exclusion it retired outlived
    // it in the README until the M16 audit.
    Shape {
        example: "v4_c_export_handle",
        kind: Kind::Crossing,
        documented_as: &["an opaque `fol_<domain>_t *`, released by a paired destroy"],
        retired: &[
            "they cannot yet be\nexported to it",
            "exporting handles or callbacks to C",
            "**Exporting** an opaque handle",
        ],
    },
    Shape {
        example: "v4_c_export_callback",
        kind: Kind::Crossing,
        documented_as: &["a C function pointer plus its own `void *` context"],
        retired: &["exporting handles\nor callbacks"],
    },
    // M12: entries carry stated discriminants.
    Shape {
        example: "v4_c_export_entry",
        kind: Kind::Crossing,
        documented_as: &["a C enum with the discriminants the variants state"],
        retired: &["a C enum with explicit stable discriminants"],
    },
    // M13: paired buffers, borrowed and owned.
    Shape {
        example: "v4_c_buffer",
        kind: Kind::Crossing,
        documented_as: &[
            "one borrowed FOL vector, its length derived",
            "a FOL vector, copied and released in the call",
        ],
        retired: &["Pointer/length slice pairing"],
    },
    // B5: text reaching C as a NUL-terminated string. Before this, no imported
    // routine could take a string at all.
    Shape {
        example: "v4_c_string_arg",
        kind: Kind::Crossing,
        documented_as: &["a FOL closure invoked by the provider"],
        retired: &[],
    },
    Shape {
        example: "v4_c_callback_no_context",
        kind: Kind::Crossing,
        documented_as: &["a FOL closure invoked by the provider"],
        retired: &[],
    },
    // Not a shape: a whole third-party library, which is the completion rule.
    Shape {
        example: "v4_c_zlib",
        kind: Kind::Supporting,
        documented_as: &[],
        retired: &[],
    },
    Shape {
        example: "v4_c_differential",
        kind: Kind::Supporting,
        documented_as: &[],
        retired: &[],
    },
    Shape {
        example: "v4_c_handle_lifecycle",
        kind: Kind::Supporting,
        documented_as: &[],
        retired: &[],
    },
];

fn read(document: &str) -> String {
    std::fs::read_to_string(repo_root().join(document))
        .unwrap_or_else(|error| panic!("{document} should be readable: {error}"))
}

/// Every `examples/v4_c_*` package is classified.
///
/// This is the half that catches a *new* shape. Adding an example without a
/// row fails here, and the row cannot be added without deciding whether it
/// crosses -- which is the decision that was never forced before.
#[test]
fn every_c_example_is_classified_in_the_matrix() {
    let mut unclassified = Vec::new();
    let entries = std::fs::read_dir(repo_root().join("examples")).expect("examples should exist");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Negative packages prove a refusal, not a crossing.
        if !name.starts_with("v4_c_") {
            continue;
        }
        if !SHAPES.iter().any(|shape| shape.example == name) {
            unclassified.push(name);
        }
    }
    unclassified.sort();
    assert!(
        unclassified.is_empty(),
        "these C examples are not in the documentation matrix, so nothing \
         requires the boundary docs to mention what they prove: {unclassified:?}\n\
         Add a row to SHAPES in this file: Crossing if it proves a shape crosses \
         the C boundary, Supporting if it proves something else."
    );
}

/// Every row's example exists, so no row can be fiction.
#[test]
fn every_matrix_row_names_a_real_example() {
    for shape in SHAPES {
        let path = repo_root().join("examples").join(shape.example);
        assert!(
            path.is_dir(),
            "SHAPES names '{}', which is not an example. A row without a package \
             proves nothing.",
            shape.example
        );
        assert!(
            path.join("build.fol").is_file(),
            "'{}' has no build.fol, so no lane can be building it",
            shape.example
        );
    }
}

/// A shape that crosses is described as crossing.
#[test]
fn every_crossing_shape_is_in_the_interop_chapter() {
    let chapter = read("book/src/950_interop/_index.md");
    for shape in SHAPES {
        if shape.kind != Kind::Crossing {
            continue;
        }
        assert!(
            !shape.documented_as.is_empty(),
            "'{}' crosses but names no phrase the chapter must carry",
            shape.example
        );
        for phrase in shape.documented_as {
            assert!(
                chapter.contains(phrase),
                "the interop chapter no longer says {phrase:?}, which is how it \
                 describes what '{}' proves. Either the wording moved -- update \
                 this row -- or the shape stopped being documented.",
                shape.example
            );
        }
    }
}

/// A retired exclusion does not survive anywhere.
///
/// This is the half that caught the real defect. Each phrase was a true
/// statement about the boundary before its example existed; once the example
/// exists, the sentence is a lie wherever it is still written.
#[test]
fn no_document_still_denies_a_shape_that_crosses() {
    let documents: Vec<(&str, String)> = DOCUMENTS.iter().map(|d| (*d, read(d))).collect();
    let mut stale = Vec::new();
    for shape in SHAPES {
        if !repo_root().join("examples").join(shape.example).is_dir() {
            continue;
        }
        for phrase in shape.retired {
            // Compared with newlines collapsed, so a sentence stays caught
            // when it is rewrapped.
            let flat: String = phrase.split_whitespace().collect::<Vec<_>>().join(" ");
            for (name, body) in &documents {
                let body_flat: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
                if body_flat.contains(&flat) {
                    stale.push(format!(
                        "{name} still says {flat:?}, but examples/{} proves otherwise",
                        shape.example
                    ));
                }
            }
        }
    }
    assert!(
        stale.is_empty(),
        "the boundary moved and the documents did not:\n  {}",
        stale.join("\n  ")
    );
}

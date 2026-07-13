//! Compiler-owned LSP diagnostic adapter.
//!
//! This module provides the canonical conversion from `Diagnostic` to
//! LSP-compatible wire types. Editor crates consume these types instead
//! of building their own conversion logic.
//!
//! ## Contract
//!
//! - **Severity**: `Error` → 1, `Warning` → 2, `Info` → 3
//! - **Code**: `diagnostic.code.as_str()` copied verbatim
//! - **Source**: always `"fol"`
//! - **Message**: `[{code}] {message}`, with notes and helps appended
//! - **Range**: 1-indexed compiler scalar columns → 0-indexed scalar columns;
//!   the editor transport converts them to negotiated LSP position units from
//!   the active source buffer
//! - **Related info**: secondary paths resolve against the primary source and
//!   become absolute percent-encoded file URIs

use crate::{Diagnostic, DiagnosticLabelKind, DiagnosticLocation, Severity};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LspPosition {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspDiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
}

impl Serialize for LspDiagnosticSeverity {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for LspDiagnosticSeverity {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u8::deserialize(deserializer)?;
        match value {
            1 => Ok(Self::Error),
            2 => Ok(Self::Warning),
            3 => Ok(Self::Information),
            _ => Err(serde::de::Error::custom(format!(
                "invalid severity: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LspDiagnosticRelatedInformation {
    pub location: LspLocation,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LspLocation {
    pub uri: String,
    pub range: LspRange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LspDiagnostic {
    pub range: LspRange,
    pub severity: LspDiagnosticSeverity,
    pub code: String,
    pub source: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_information: Vec<LspDiagnosticRelatedInformation>,
}

pub fn location_to_range(location: &DiagnosticLocation) -> LspRange {
    let line = location.line.saturating_sub(1) as u32;
    let start_character = location.column.saturating_sub(1) as u32;
    let end_character = start_character + location.length.unwrap_or(1).max(1) as u32;
    LspRange {
        start: LspPosition {
            line,
            character: start_character,
        },
        end: LspPosition {
            line,
            character: end_character,
        },
    }
}

pub fn diagnostic_to_lsp(diagnostic: &Diagnostic) -> LspDiagnostic {
    let primary = diagnostic
        .primary_location()
        .cloned()
        .or_else(|| {
            diagnostic
                .labels
                .first()
                .map(|label| label.location.clone())
        })
        .unwrap_or(DiagnosticLocation {
            file: None,
            line: 1,
            column: 1,
            length: Some(1),
        });

    let mut message = format!("[{}] {}", diagnostic.code.as_str(), diagnostic.message);
    if !diagnostic.notes.is_empty() {
        message.push_str("\nnotes:");
        for note in &diagnostic.notes {
            message.push_str("\n- ");
            message.push_str(note);
        }
    }
    if !diagnostic.helps.is_empty() {
        message.push_str("\nhelps:");
        for help in &diagnostic.helps {
            message.push_str("\n- ");
            message.push_str(help);
        }
    }

    let related_information = diagnostic
        .labels
        .iter()
        .filter(|label| label.kind == DiagnosticLabelKind::Secondary)
        .filter_map(|label| {
            label
                .location
                .file
                .as_ref()
                .map(|file| LspDiagnosticRelatedInformation {
                    location: LspLocation {
                        uri: diagnostic_file_uri(file, primary.file.as_deref()),
                        range: location_to_range(&label.location),
                    },
                    message: label
                        .message
                        .clone()
                        .unwrap_or_else(|| "related".to_string()),
                })
        })
        .collect::<Vec<_>>();

    LspDiagnostic {
        range: location_to_range(&primary),
        severity: match diagnostic.severity {
            Severity::Error => LspDiagnosticSeverity::Error,
            Severity::Warning => LspDiagnosticSeverity::Warning,
            Severity::Info => LspDiagnosticSeverity::Information,
        },
        code: diagnostic.code.as_str().to_string(),
        source: "fol".to_string(),
        message,
        related_information,
    }
}

fn diagnostic_file_uri(file: &str, primary_file: Option<&str>) -> String {
    let path = Path::new(file);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let base = primary_file
            .map(Path::new)
            .map(|primary| {
                if primary.is_absolute() {
                    primary.to_path_buf()
                } else {
                    current_dir.join(primary)
                }
            })
            .and_then(|primary| primary.parent().map(Path::to_path_buf))
            .unwrap_or(current_dir);
        base.join(path)
    };
    let normalized = normalize_path(&absolute);
    format!(
        "file://{}",
        percent_encode_path(&normalized.to_string_lossy())
    )
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                let _ = normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

/// Deduplicate exact LSP wire diagnostics, keeping the first occurrence.
///
/// This is the **view-layer** dedup. It runs after diagnostics from
/// multiple compiler stages have been converted to LSP format, catching
/// exact cross-stage duplicates without hiding distinct messages, ranges,
/// severities, or related information that happen to share a line and code.
///
/// The **report-layer** dedup in [`DiagnosticReport::add_diagnostic`]
/// handles exact consecutive duplicates and a hard cap at 50 diagnostics.
/// Both layers are intentional and complementary.
pub fn dedup_lsp_diagnostics(diagnostics: Vec<LspDiagnostic>) -> Vec<LspDiagnostic> {
    let mut unique = Vec::new();
    for diagnostic in diagnostics {
        if !unique.contains(&diagnostic) {
            unique.push(diagnostic);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagnosticLocation;

    #[test]
    fn locations_convert_to_zero_based_lsp_ranges() {
        let range = location_to_range(&DiagnosticLocation {
            file: Some("/tmp/demo.fol".to_string()),
            line: 4,
            column: 3,
            length: Some(5),
        });
        assert_eq!(range.start.line, 3);
        assert_eq!(range.start.character, 2);
        assert_eq!(range.end.character, 7);
    }

    #[test]
    fn contract_source_is_always_fol() {
        let lsp = diagnostic_to_lsp(&Diagnostic::error("P1001", "test"));
        assert_eq!(lsp.source, "fol");
    }

    #[test]
    fn contract_severity_maps_all_variants() {
        assert_eq!(
            diagnostic_to_lsp(&Diagnostic::error("E0001", "e")).severity,
            LspDiagnosticSeverity::Error
        );
        assert_eq!(
            diagnostic_to_lsp(&Diagnostic::warning("W0001", "w")).severity,
            LspDiagnosticSeverity::Warning
        );
        assert_eq!(
            diagnostic_to_lsp(&Diagnostic::info("I0001", "i")).severity,
            LspDiagnosticSeverity::Information
        );
    }

    #[test]
    fn contract_code_is_verbatim() {
        let lsp = diagnostic_to_lsp(&Diagnostic::error("R1003", "unresolved"));
        assert_eq!(lsp.code, "R1003");
    }

    #[test]
    fn contract_message_prefixed_with_code() {
        let lsp = diagnostic_to_lsp(&Diagnostic::error("T1003", "type mismatch"));
        assert_eq!(lsp.message, "[T1003] type mismatch");
    }

    #[test]
    fn contract_notes_and_helps_appended() {
        let diagnostic = Diagnostic::error("R1003", "unresolved")
            .with_note("context")
            .with_help("add import");
        let lsp = diagnostic_to_lsp(&diagnostic);
        assert!(lsp.message.contains("\nnotes:\n- context"));
        assert!(lsp.message.contains("\nhelps:\n- add import"));
    }

    #[test]
    fn contract_missing_location_defaults_to_origin() {
        let lsp = diagnostic_to_lsp(&Diagnostic::error("P1001", "no location"));
        assert_eq!(lsp.range.start.line, 0);
        assert_eq!(lsp.range.start.character, 0);
    }

    #[test]
    fn contract_secondary_labels_become_related_info() {
        let diagnostic = Diagnostic::error("R1003", "test")
            .with_primary_label_message(
                DiagnosticLocation {
                    file: Some("/tmp/a.fol".to_string()),
                    line: 2,
                    column: 5,
                    length: Some(6),
                },
                "here",
            )
            .with_secondary_label(
                DiagnosticLocation {
                    file: Some("/tmp/b.fol".to_string()),
                    line: 1,
                    column: 1,
                    length: Some(3),
                },
                "related",
            );
        let lsp = diagnostic_to_lsp(&diagnostic);
        assert_eq!(lsp.related_information.len(), 1);
        assert_eq!(lsp.related_information[0].message, "related");
        assert_eq!(lsp.related_information[0].location.uri, "file:///tmp/b.fol");
    }

    #[test]
    fn contract_secondary_paths_resolve_and_percent_encode_as_file_uris() {
        let diagnostic = Diagnostic::error("R1003", "test")
            .with_primary_label(DiagnosticLocation {
                file: Some("/tmp/fol uri/main.fol".to_string()),
                line: 1,
                column: 1,
                length: Some(1),
            })
            .with_secondary_label(
                DiagnosticLocation {
                    file: Some("../shared/a b#%.fol".to_string()),
                    line: 1,
                    column: 1,
                    length: Some(1),
                },
                "related",
            );

        let lsp = diagnostic_to_lsp(&diagnostic);
        assert_eq!(
            lsp.related_information[0].location.uri,
            "file:///tmp/shared/a%20b%23%25.fol"
        );
    }

    #[test]
    fn contract_secondary_without_file_excluded() {
        let diagnostic = Diagnostic::error("R1003", "test").with_secondary_label(
            DiagnosticLocation {
                file: None,
                line: 1,
                column: 1,
                length: Some(1),
            },
            "no file",
        );
        let lsp = diagnostic_to_lsp(&diagnostic);
        assert!(lsp.related_information.is_empty());
    }

    #[test]
    fn dedup_keeps_different_codes_on_same_line() {
        let d1 = diagnostic_to_lsp(&Diagnostic::error("P1001", "syntax").with_primary_label(
            DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 5,
                column: 1,
                length: Some(1),
            },
        ));
        let d2 = diagnostic_to_lsp(
            &Diagnostic::error("R1003", "unresolved").with_primary_label(DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 5,
                column: 3,
                length: Some(2),
            }),
        );
        let result = dedup_lsp_diagnostics(vec![d1, d2]);
        assert_eq!(result.len(), 2, "different codes on same line must be kept");
    }

    #[test]
    fn dedup_keeps_same_code_on_different_lines() {
        let d1 = diagnostic_to_lsp(&Diagnostic::error("P1001", "first").with_primary_label(
            DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 3,
                column: 1,
                length: Some(1),
            },
        ));
        let d2 = diagnostic_to_lsp(&Diagnostic::error("P1001", "second").with_primary_label(
            DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 7,
                column: 1,
                length: Some(1),
            },
        ));
        let result = dedup_lsp_diagnostics(vec![d1, d2]);
        assert_eq!(result.len(), 2, "same code on different lines must be kept");
    }

    #[test]
    fn report_and_lsp_dedup_layers_complement_each_other() {
        use crate::DiagnosticReport;

        // Report-layer: exact consecutive duplicates are suppressed.
        let mut report = DiagnosticReport::new();
        let loc = DiagnosticLocation {
            file: Some("a.fol".to_string()),
            line: 5,
            column: 1,
            length: Some(1),
        };
        report.add_diagnostic(Diagnostic::error("P1001", "a").with_primary_label(loc.clone()));
        report.add_diagnostic(Diagnostic::error("P1001", "a").with_primary_label(loc.clone()));
        assert_eq!(
            report.diagnostics.len(),
            1,
            "report layer deduped consecutive"
        );

        // Now simulate cross-stage: parser + resolver emit the exact wire
        // diagnostic for the same source problem.
        let lsp_diags: Vec<LspDiagnostic> = report
            .diagnostics
            .iter()
            .map(diagnostic_to_lsp)
            .chain(std::iter::once(diagnostic_to_lsp(
                &Diagnostic::error("P1001", "a").with_primary_label(loc.clone()),
            )))
            .collect();

        assert_eq!(lsp_diags.len(), 2, "before view-layer dedup");
        let deduped = dedup_lsp_diagnostics(lsp_diags);
        assert_eq!(deduped.len(), 1, "view-layer caught cross-stage duplicate");
    }

    #[test]
    fn dedup_keeps_same_line_and_code_with_distinct_ranges_and_messages() {
        let d1 = diagnostic_to_lsp(&Diagnostic::error("P1001", "first").with_primary_label(
            DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 5,
                column: 1,
                length: Some(1),
            },
        ));
        let d2 = diagnostic_to_lsp(&Diagnostic::error("P1001", "second").with_primary_label(
            DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 5,
                column: 1,
                length: Some(1),
            },
        ));
        let d3 = diagnostic_to_lsp(&Diagnostic::error("P1001", "second").with_primary_label(
            DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 5,
                column: 3,
                length: Some(2),
            },
        ));
        let result = dedup_lsp_diagnostics(vec![d1, d2, d3]);
        assert_eq!(result.len(), 3);
        assert!(result[0].message.contains("first"));
        assert!(result[1].message.contains("second"));
        assert_eq!(result[0].range, result[1].range);
        assert_ne!(result[1].range, result[2].range);
    }

    #[test]
    fn dedup_removes_only_exact_wire_duplicates() {
        let diagnostic = diagnostic_to_lsp(
            &Diagnostic::error("P1001", "duplicate").with_primary_label(DiagnosticLocation {
                file: Some("a.fol".to_string()),
                line: 5,
                column: 3,
                length: Some(2),
            }),
        );
        let result = dedup_lsp_diagnostics(vec![diagnostic.clone(), diagnostic]);

        assert_eq!(result.len(), 1);
        assert!(result[0].message.contains("duplicate"));
    }
}

use crate::{source, Diagnostic, DiagnosticLabelKind, DiagnosticReport, Severity};

pub fn render_report(report: &DiagnosticReport) -> String {
    let mut output = String::new();

    for diagnostic in &report.diagnostics {
        output.push_str(&render_diagnostic(diagnostic));
        output.push('\n');
    }

    if report.error_count > 0 || report.warning_count > 0 {
        output.push('\n');
        if report.error_count > 0 {
            let label = if report.error_count == 1 { "" } else { "s" };
            if report.diagnostics.len() >= 50 {
                output.push_str(&format!(
                    "error: found {}+ error{} (output truncated)",
                    report.error_count, label,
                ));
            } else {
                output.push_str(&format!(
                    "error: found {} error{}",
                    report.error_count, label
                ));
            }
        }
        if report.warning_count > 0 {
            if report.error_count > 0 {
                output.push_str(", ");
            }
            let label = if report.warning_count == 1 { "" } else { "s" };
            output.push_str(&format!(
                "warning: {} warning{}",
                report.warning_count, label
            ));
        }
        output.push('\n');
    }

    output
}

pub fn render_diagnostic(diagnostic: &Diagnostic) -> String {
    let mut output = String::new();

    let prefix = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    };

    if diagnostic.code.as_str() != "EUNKNOWN" {
        output.push_str(&format!(
            "{}[{}]: {}",
            prefix,
            diagnostic.code.as_str(),
            diagnostic.message
        ));
    } else {
        output.push_str(&format!("{}: {}", prefix, diagnostic.message));
    }

    if let Some(loc) = diagnostic.primary_location() {
        output.push('\n');
        if let Some(file) = &loc.file {
            output.push_str(&format!("  {} {}:{}:{}", "-->", file, loc.line, loc.column));
        } else {
            output.push_str(&format!("  {} line {}:{}", "-->", loc.line, loc.column));
        }

        match source::load_source_line(loc) {
            Ok(source_line) => {
                let line_number = loc.line.to_string();
                let gutter_width = line_number.len();
                output.push('\n');
                output.push_str(&format!("  {} |", " ".repeat(gutter_width)));
                output.push('\n');
                output.push_str(&format!("  {} | {}", line_number, source_line));
                output.push('\n');
                output.push_str(&format!(
                    "  {} | {}",
                    " ".repeat(gutter_width),
                    source::primary_underline(loc)
                ));
                if let Some(message) = diagnostic
                    .primary_label()
                    .and_then(|label| label.message.as_deref())
                {
                    output.push(' ');
                    output.push_str(message);
                }
            }
            Err(source::SourceSnippetError::MissingFilePath) => {}
            Err(source::SourceSnippetError::ReadFailed) => {
                output.push('\n');
                output.push_str(&format!(
                    "  {} source unavailable: could not read {}",
                    "note:",
                    loc.file.as_deref().unwrap_or("<unknown>")
                ));
            }
            Err(source::SourceSnippetError::MissingLine) => {
                output.push('\n');
                output.push_str(&format!(
                    "  {} source unavailable: line {} is outside the file",
                    "note:", loc.line
                ));
            }
        }
    }

    for label in diagnostic
        .labels
        .iter()
        .filter(|label| label.kind == DiagnosticLabelKind::Secondary)
    {
        output.push('\n');
        output.push_str(&format!("  {} {}", "note:", related_label_summary(label)));
    }

    for note in &diagnostic.notes {
        output.push('\n');
        output.push_str(&format!("  {} {}", "note:", note));
    }

    for help in &diagnostic.helps {
        output.push('\n');
        output.push_str(&format!("  {} {}", "help:", help));
    }

    output
}

fn related_label_summary(label: &crate::DiagnosticLabel) -> String {
    let location = &label.location;
    let prefix = if let Some(file) = &location.file {
        format!("{file}:{}:{}", location.line, location.column)
    } else {
        format!("line {}:{}", location.line, location.column)
    };

    match label.message.as_deref() {
        Some(message) => format!("{prefix}: {message}"),
        None => prefix,
    }
}

#[cfg(test)]
mod tests {
    use crate::{Diagnostic, DiagnosticLocation, Severity};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn render_diagnostic_uses_primary_location_and_first_help() {
        let diagnostic = Diagnostic::new(Severity::Error, "R4000", "renderer split")
            .with_primary_label(DiagnosticLocation {
                file: Some("pkg/main.fol".to_string()),
                line: 3,
                column: 2,
                length: Some(5),
            })
            .with_help("use this renderer");

        let rendered = super::render_diagnostic(&diagnostic);

        assert!(rendered.contains("renderer split"));
        assert!(rendered.contains("--> pkg/main.fol:3:2"));
        assert!(rendered.contains("help: use this renderer"));
    }

    #[test]
    fn render_diagnostic_shows_primary_source_snippet_and_underline() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for test file naming")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fol_diagnostics_render_{stamp}.fol"));
        std::fs::write(&path, "var answer: int = 42;\n")
            .expect("test source file should be writable");

        let diagnostic = Diagnostic::new(Severity::Error, "R4001", "snippet renderer")
            .with_primary_label_message(
                DiagnosticLocation {
                    file: Some(path.to_string_lossy().into_owned()),
                    line: 1,
                    column: 5,
                    length: Some(6),
                },
                "primary binding",
            );

        let rendered = super::render_diagnostic(&diagnostic);
        let _ = std::fs::remove_file(&path);

        assert!(rendered.contains("var answer: int = 42;"));
        assert!(rendered.contains("^^^^^^ primary binding"));
    }

    #[test]
    fn render_diagnostic_shows_secondary_labels_notes_and_multiple_helps() {
        let diagnostic = Diagnostic::new(Severity::Warning, "R4002", "secondary renderer")
            .with_primary_label(DiagnosticLocation {
                file: Some("pkg/main.fol".to_string()),
                line: 3,
                column: 2,
                length: Some(5),
            })
            .with_secondary_label(
                DiagnosticLocation {
                    file: Some("pkg/lib.fol".to_string()),
                    line: 9,
                    column: 1,
                    length: Some(4),
                },
                "related declaration",
            )
            .with_note("shadowed here")
            .with_help("rename the local binding")
            .with_help("or qualify the imported name");

        let rendered = super::render_diagnostic(&diagnostic);

        assert!(rendered.contains("note: pkg/lib.fol:9:1: related declaration"));
        assert!(rendered.contains("note: shadowed here"));
        assert!(rendered.contains("help: rename the local binding"));
        assert!(rendered.contains("help: or qualify the imported name"));
    }

    #[test]
    fn render_diagnostic_falls_back_cleanly_when_source_file_is_missing() {
        let diagnostic = Diagnostic::new(Severity::Error, "R4003", "missing source")
            .with_primary_label(DiagnosticLocation {
                file: Some("/tmp/fol_diagnostics_missing_file.fol".to_string()),
                line: 7,
                column: 3,
                length: None,
            });

        let rendered = super::render_diagnostic(&diagnostic);

        assert!(rendered.contains("--> /tmp/fol_diagnostics_missing_file.fol:7:3"));
        assert!(rendered.contains(
            "note: source unavailable: could not read /tmp/fol_diagnostics_missing_file.fol"
        ));
    }

    #[test]
    fn render_diagnostic_uses_single_caret_when_span_length_is_missing() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for test file naming")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fol_diagnostics_lengthless_{stamp}.fol"));
        std::fs::write(&path, "value\n").expect("test source file should be writable");

        let diagnostic = Diagnostic::new(Severity::Error, "R4004", "lengthless span")
            .with_primary_label(DiagnosticLocation {
                file: Some(path.to_string_lossy().into_owned()),
                line: 1,
                column: 2,
                length: None,
            });

        let rendered = super::render_diagnostic(&diagnostic);
        let _ = std::fs::remove_file(&path);

        assert!(rendered.contains("| value"));
        assert!(rendered.contains("|  ^"));
    }

    #[test]
    fn render_report_shows_warning_and_info_severities_distinctly() {
        let mut report = crate::DiagnosticReport::new();
        report.add_diagnostic(Diagnostic::warning("W4005", "warning renderer"));
        report.add_diagnostic(Diagnostic::info("I4006", "info renderer"));

        let rendered = super::render_report(&report);

        assert!(rendered.contains("warning[W4005]: warning renderer"));
        assert!(rendered.contains("info[I4006]: info renderer"));
        assert!(rendered.contains("warning: 1 warning"));
    }
}

//! Pretty, human-friendly diagnostic rendering for the CLI `human` output mode.
//!
//! Reads the structured [`DiagnosticReport`] model directly (the same model the
//! LSP consumes) and renders a framed, colored report: a family chip that names
//! the kind of problem in plain language ("a typo", "a type mismatch"), a source
//! frame with a caret under the offending span, secondary labels, notes/helps,
//! and a footer pointing at `fol explain <CODE>`.
//!
//! Colors come from the in-house [`crate::ansi`] palette, which disables itself
//! when stdout is not a terminal — so piped/CI output stays plain text while an
//! interactive terminal gets the full look. The `plain` and `json` modes keep
//! using the library renderers unchanged.

use crate::ansi::Colored;
use fol_diagnostics::{
    source, Diagnostic, DiagnosticLabelKind, DiagnosticLocation, DiagnosticReport, Severity,
};

const GUTTER: &str = "  ";

/// Render a full report in the pretty human format.
pub fn render_report_pretty(report: &DiagnosticReport) -> String {
    let mut out = String::new();
    for (index, diagnostic) in report.diagnostics.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&render_diagnostic_pretty(diagnostic));
    }
    if report.error_count > 0 || report.warning_count > 0 {
        out.push('\n');
        out.push_str(&render_summary(report));
        out.push('\n');
    }
    out
}

/// Map an error code + severity to a plain-language family name and a one-line
/// "what this means" hint. Warnings/infos take precedence over the code family.
///
/// The code-prefix mapping is owned by `fol-diagnostics` so the family chip here
/// and the `fol explain <CODE>` output never drift apart.
fn family(code: &str, severity: &Severity) -> (&'static str, &'static str) {
    match severity {
        Severity::Warning => return ("WARNING", "something worth a second look"),
        Severity::Info => return ("NOTE", "an informational note"),
        Severity::Error => {}
    }
    fol_diagnostics::family_for_code(code)
}

fn chip(text: &str, severity: &Severity) -> String {
    let padded = format!(" {text} ");
    let styled = match severity {
        Severity::Error => padded.black().on_red().bold(),
        Severity::Warning => padded.black().on_yellow().bold(),
        Severity::Info => padded.black().on_blue().bold(),
    };
    format!("{styled}")
}

pub fn render_diagnostic_pretty(diagnostic: &Diagnostic) -> String {
    let mut out = String::new();
    let code = diagnostic.code.as_str();
    let has_code = code != "EUNKNOWN";
    let (family_label, family_hint) = family(code, &diagnostic.severity);

    // ── header: chip · message · code ──
    out.push_str(&chip(family_label, &diagnostic.severity));
    out.push(' ');
    out.push_str(&format!("{}", diagnostic.message.as_str().bold()));
    if has_code {
        out.push_str("   ");
        out.push_str(&format!("{}", code.bright_black()));
    }
    out.push('\n');

    // ── source frame for the primary label ──
    let primary = diagnostic.primary_label();
    if let Some(label) = primary {
        render_frame(
            &mut out,
            &label.location,
            label.message.as_deref(),
            &diagnostic.severity,
        );
    }

    // ── secondary labels ──
    for label in diagnostic
        .labels
        .iter()
        .filter(|label| label.kind == DiagnosticLabelKind::Secondary)
    {
        render_secondary(&mut out, &label.location, label.message.as_deref());
    }

    // ── notes, helps, suggestions ──
    for note in &diagnostic.notes {
        out.push_str(&format!(
            "{}{} {}\n",
            GUTTER,
            "= note:".bright_blue().bold(),
            note
        ));
    }
    for help in &diagnostic.helps {
        out.push_str(&format!(
            "{}{} {}\n",
            GUTTER,
            "= help:".green().bold(),
            help
        ));
    }
    for suggestion in &diagnostic.suggestions {
        let mut line = suggestion.message.clone();
        if let Some(replacement) = &suggestion.replacement {
            line.push_str(&format!(": `{replacement}`"));
        }
        out.push_str(&format!(
            "{}{} {}\n",
            GUTTER,
            "= try:".green().bold(),
            line
        ));
    }

    // ── footer: plain-language hint + explain hook ──
    let explain = if has_code {
        format!("{family_hint}  ·  run `fol explain {code}` for more")
    } else {
        family_hint.to_string()
    };
    out.push_str(&format!("{}{}\n", GUTTER, explain.bright_black().italic()));
    out
}

fn render_frame(
    out: &mut String,
    location: &DiagnosticLocation,
    label_message: Option<&str>,
    severity: &Severity,
) {
    let where_line = match &location.file {
        Some(file) => format!("{file}:{}:{}", location.line, location.column),
        None => format!("line {}:{}", location.line, location.column),
    };
    out.push_str(&format!(
        "{}{} {}\n",
        GUTTER,
        "┌─".bright_black(),
        where_line.cyan()
    ));

    match source::load_source_line(location) {
        Ok(source_line) => {
            let number = location.line.to_string();
            let width = number.len();
            out.push_str(&format!("{GUTTER}{} {}\n", " ".repeat(width), "│".bright_black()));
            out.push_str(&format!(
                "{GUTTER}{} {} {}\n",
                number.bright_black(),
                "│".bright_black(),
                source_line
            ));
            let underline = source::primary_underline(location);
            let carets = match severity {
                Severity::Error => underline.red().bold(),
                Severity::Warning => underline.yellow().bold(),
                Severity::Info => underline.blue().bold(),
            };
            out.push_str(&format!(
                "{GUTTER}{} {} {}",
                " ".repeat(width),
                "│".bright_black(),
                carets
            ));
            if let Some(message) = label_message {
                out.push(' ');
                out.push_str(&format!("{}", message.red()));
            }
            out.push('\n');
        }
        Err(_) => {
            if let Some(message) = label_message {
                out.push_str(&format!("{GUTTER}{} {}\n", "│".bright_black(), message));
            }
        }
    }
}

fn render_secondary(out: &mut String, location: &DiagnosticLocation, label_message: Option<&str>) {
    let where_line = match &location.file {
        Some(file) => format!("{file}:{}:{}", location.line, location.column),
        None => format!("line {}:{}", location.line, location.column),
    };
    let message = label_message.unwrap_or("related");
    match source::load_source_line(location) {
        Ok(source_line) => {
            let number = location.line.to_string();
            let width = number.len();
            out.push_str(&format!(
                "{GUTTER}{} {}\n",
                "·".bright_black(),
                where_line.cyan()
            ));
            out.push_str(&format!(
                "{GUTTER}{} {} {}\n",
                number.bright_black(),
                "·".bright_black(),
                source_line.bright_black()
            ));
            let underline = source::primary_underline(location);
            out.push_str(&format!(
                "{GUTTER}{} {} {} {}\n",
                " ".repeat(width),
                "·".bright_black(),
                underline.bright_blue(),
                message.bright_blue()
            ));
        }
        Err(_) => {
            out.push_str(&format!(
                "{GUTTER}{} {} ({})\n",
                "·".bright_black(),
                message.bright_blue(),
                where_line.cyan()
            ));
        }
    }
}

fn render_summary(report: &DiagnosticReport) -> String {
    let mut parts = Vec::new();
    if report.error_count > 0 {
        let plural = if report.error_count == 1 { "" } else { "s" };
        let truncated = if report.diagnostics.len() >= 50 { "+" } else { "" };
        parts.push(format!(
            "{}",
            format!(
                "found {}{truncated} error{plural}",
                report.error_count
            )
            .red()
            .bold()
        ));
    }
    if report.warning_count > 0 {
        let plural = if report.warning_count == 1 { "" } else { "s" };
        parts.push(format!(
            "{}",
            format!("{} warning{plural}", report.warning_count)
                .yellow()
                .bold()
        ));
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fol_diagnostics::DiagnosticLocation;

    fn label_loc(line: usize, column: usize, length: usize) -> DiagnosticLocation {
        DiagnosticLocation {
            file: None,
            line,
            column,
            length: Some(length),
        }
    }

    // Family chips / message / code / footer are each rendered as whole styled
    // segments, so `contains` on their text is stable whether or not ANSI is on.

    #[test]
    fn pretty_header_carries_family_message_and_explain_hook() {
        let diagnostic = Diagnostic::error("P1001", "expected `;`, found `}`")
            .with_primary_label(label_loc(3, 14, 1));
        let out = render_diagnostic_pretty(&diagnostic);
        assert!(out.contains("PARSER"), "family chip: {out}");
        assert!(out.contains("expected `;`, found `}`"));
        assert!(out.contains("P1001"));
        assert!(out.contains("a syntax slip"));
        assert!(out.contains("run `fol explain P1001` for more"));
    }

    #[test]
    fn pretty_family_is_inferred_from_the_code_prefix() {
        for (code, expected) in [
            ("L1001", "LOWERING"),
            ("K1001", "PACKAGE"),
            ("P1001", "PARSER"),
            ("R1003", "NAMES"),
            ("T1002", "TYPES"),
            ("F1003", "BUILD"),
            ("B1001", "BACKEND"),
        ] {
            let out = render_diagnostic_pretty(&Diagnostic::error(code, "problem"));
            assert!(out.contains(expected), "code {code} should map to {expected}: {out}");
        }
    }

    #[test]
    fn pretty_uses_severity_families_for_warnings_and_info() {
        assert!(render_diagnostic_pretty(&Diagnostic::warning("W1001", "w")).contains("WARNING"));
        assert!(render_diagnostic_pretty(&Diagnostic::info("I1001", "i")).contains("NOTE"));
    }

    #[test]
    fn pretty_report_renders_notes_helps_and_summary() {
        let diagnostic = Diagnostic::error("R1003", "could not resolve name 'x'")
            .with_note("declared in another scope")
            .with_help("import it or declare it locally");
        let report = DiagnosticReport {
            diagnostics: vec![diagnostic],
            error_count: 1,
            warning_count: 0,
        };
        let out = render_report_pretty(&report);
        assert!(out.contains("note:"));
        assert!(out.contains("declared in another scope"));
        assert!(out.contains("help:"));
        assert!(out.contains("import it or declare it locally"));
        assert!(out.contains("found 1 error"));
    }
}

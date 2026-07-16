//! `fol code explain <CODE>` — render an extended explanation for a diagnostic code.
//!
//! The explanations and the code-family mapping live in `fol-diagnostics` (the
//! compiler-truth crate) so the family chip printed here matches the one the
//! pretty diagnostic renderer prints. This module only adapts that data to the
//! CLI's `human`/`plain`/`json` output modes and the in-house ANSI palette.

use crate::ansi::Colored;
use crate::{FrontendCommandResult, FrontendError, FrontendErrorKind, FrontendResult, OutputMode};
use fol_diagnostics::{explanation, family_for_code};

/// A rendered `explain` result plus whether the code was known, so callers can
/// pick an exit code (unknown codes exit nonzero).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainRendering {
    pub text: String,
    pub known: bool,
}

/// Render `fol code explain <code>` in the requested output mode.
pub fn render_explain(code: &str, mode: OutputMode) -> ExplainRendering {
    let normalized = code.trim().to_ascii_uppercase();
    let (family, hint) = family_for_code(&normalized);
    let known = explanation(&normalized).is_some();

    let text = match mode {
        OutputMode::Json => render_json(&normalized, family, hint),
        OutputMode::Plain => render_plain(&normalized, family, hint),
        OutputMode::Human => render_human(&normalized, family, hint),
    };
    ExplainRendering { text, known }
}

fn family_is_recognized(family: &str) -> bool {
    family != "ERROR"
}

fn chip(family: &str) -> String {
    let padded = format!(" {family} ");
    format!("{}", padded.black().on_red().bold())
}

fn render_human(code: &str, family: &str, hint: &str) -> String {
    let mut out = String::new();
    match explanation(code) {
        Some(explanation) => {
            // header: chip · title · code
            out.push_str(&chip(family));
            out.push(' ');
            out.push_str(&format!("{}", explanation.title.bold()));
            out.push_str("   ");
            out.push_str(&format!("{}", code.bright_black()));
            out.push('\n');
            // family hint
            out.push_str(&format!("  {}\n", hint.bright_black().italic()));
            out.push('\n');
            // body
            for line in explanation.body.lines() {
                out.push_str("  ");
                out.push_str(line);
                out.push('\n');
            }
        }
        None => {
            out.push_str(&format!(
                "{} {}\n",
                chip(family),
                format!("no extended explanation for {code} yet").bold()
            ));
            if family_is_recognized(family) {
                out.push_str(&format!(
                    "  {}\n",
                    format!("{code} is in the {family} family — {hint}")
                        .bright_black()
                        .italic()
                ));
            } else {
                out.push_str(&format!(
                    "  {}\n",
                    format!("{code} is not a recognized FOL diagnostic code")
                        .bright_black()
                        .italic()
                ));
            }
        }
    }
    out
}

fn render_plain(code: &str, family: &str, hint: &str) -> String {
    match explanation(code) {
        Some(explanation) => format!(
            "code: {code}\nfamily: {family}\ntitle: {title}\n\n{body}",
            title = explanation.title,
            body = explanation.body,
        ),
        None => {
            if family_is_recognized(family) {
                format!(
                    "code: {code}\nfamily: {family}\nno extended explanation for {code} yet ({hint})"
                )
            } else {
                format!("code: {code}\nno extended explanation for {code} yet (unrecognized code)")
            }
        }
    }
}

fn render_json(code: &str, family: &str, _hint: &str) -> String {
    let (title, body) = match explanation(code) {
        Some(explanation) => (
            Some(explanation.title.to_string()),
            Some(explanation.body.to_string()),
        ),
        None => (None, None),
    };
    let payload = serde_json::json!({
        "code": code,
        "family": if family_is_recognized(family) { serde_json::Value::from(family) } else { serde_json::Value::Null },
        "known": title.is_some(),
        "title": title,
        "explanation": body,
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// Library entrypoint used by `dispatch_cli`. The binary path renders `explain`
/// directly at the I/O boundary (so it can control the exact output shape and
/// exit code); this keeps the command usable through `run_command_from_args`
/// for library callers too.
pub fn explain_command(code: &str, mode: OutputMode) -> FrontendResult<FrontendCommandResult> {
    let rendering = render_explain(code, mode);
    if rendering.known {
        Ok(FrontendCommandResult::new("explain", rendering.text))
    } else {
        Err(
            FrontendError::new(FrontendErrorKind::InvalidInput, rendering.text).with_note(
                "run `fol code explain <CODE>` with a code from a diagnostic footer",
            ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_known_code_renders_family_title_and_body() {
        let rendering = render_explain("T1003", OutputMode::Human);
        assert!(rendering.known);
        assert!(rendering.text.contains("TYPES"));
        assert!(rendering.text.contains("incompatible types"));
        assert!(rendering.text.contains("T1003"));
        assert!(rendering.text.contains("How to fix"));
    }

    #[test]
    fn human_lookup_is_case_insensitive() {
        let lower = render_explain("t1003", OutputMode::Human);
        assert!(lower.known);
        assert!(lower.text.contains("incompatible types"));
    }

    #[test]
    fn human_unknown_but_recognized_prefix_points_at_family() {
        let rendering = render_explain("T9999", OutputMode::Human);
        assert!(!rendering.known);
        assert!(rendering.text.contains("no extended explanation for T9999"));
        assert!(rendering.text.contains("TYPES family"));
    }

    #[test]
    fn human_unknown_and_unrecognized_prefix_is_honest() {
        let rendering = render_explain("Z9999", OutputMode::Human);
        assert!(!rendering.known);
        assert!(rendering.text.contains("not a recognized FOL diagnostic code"));
    }

    #[test]
    fn json_known_code_carries_the_documented_shape() {
        let rendering = render_explain("T1003", OutputMode::Json);
        let value: serde_json::Value =
            serde_json::from_str(&rendering.text).expect("explain json should parse");
        assert_eq!(value["code"], "T1003");
        assert_eq!(value["family"], "TYPES");
        assert_eq!(value["known"], true);
        assert_eq!(value["title"], "incompatible types");
        assert!(value["explanation"].as_str().is_some());
    }

    #[test]
    fn json_unknown_code_reports_null_explanation() {
        let rendering = render_explain("Z9999", OutputMode::Json);
        let value: serde_json::Value =
            serde_json::from_str(&rendering.text).expect("explain json should parse");
        assert_eq!(value["code"], "Z9999");
        assert!(value["family"].is_null());
        assert_eq!(value["known"], false);
        assert!(value["title"].is_null());
        assert!(value["explanation"].is_null());
    }

    #[test]
    fn plain_known_code_is_script_friendly() {
        let rendering = render_explain("R1003", OutputMode::Plain);
        assert!(rendering.text.contains("code: R1003"));
        assert!(rendering.text.contains("family: NAMES"));
        assert!(rendering.text.contains("title: unresolved name"));
    }

    #[test]
    fn explain_command_errors_on_unknown_codes() {
        let error = explain_command("Z9999", OutputMode::Human)
            .expect_err("unknown codes should be a frontend error");
        assert_eq!(error.kind(), FrontendErrorKind::InvalidInput);
    }
}

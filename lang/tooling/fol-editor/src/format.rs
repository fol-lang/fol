use crate::{EditorError, EditorErrorKind, EditorResult, LspPosition, LspRange};

const INDENT_WIDTH: usize = 4;

pub fn format_document(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut depth = 0usize;
    let mut lines: Vec<String> = Vec::new();

    for raw_line in normalized.split('\n') {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if matches!(lines.last(), Some(previous) if !previous.is_empty()) {
                lines.push(String::new());
            }
            continue;
        }

        let indent_depth = depth.saturating_sub(leading_closing_brace_count(trimmed));
        let indent = " ".repeat(indent_depth * INDENT_WIDTH);
        lines.push(format!("{indent}{trimmed}"));
        depth = update_brace_depth(trimmed, depth);
    }

    while matches!(lines.last(), Some(line) if line.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        String::new()
    } else if depth > 0 {
        // The document is brace-unbalanced (for example mid-edit); keep the
        // truncated tail as-is instead of appending a declaration terminator.
        format!("{}\n", lines.join("\n"))
    } else {
        let mut joined = lines.join("\n");
        if joined.ends_with(';') {
            joined.pop();
        }
        format!("{};\n", joined)
    }
}

pub fn formatting_edit(text: &str) -> Option<crate::LspTextEdit> {
    let formatted = format_document(text);
    if formatted == text {
        None
    } else {
        Some(crate::LspTextEdit {
            range: whole_document_range(text),
            new_text: formatted,
        })
    }
}

pub fn format_document_in_place(path: &std::path::Path) -> EditorResult<FormatResult> {
    let canonical = path.canonicalize().map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to resolve '{}': {error}", path.display()),
        )
    })?;
    let source = std::fs::read_to_string(&canonical).map_err(|error| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("failed to read '{}': {error}", canonical.display()),
        )
    })?;
    let formatted = format_document(&source);
    let changed = formatted != source;
    if changed {
        std::fs::write(&canonical, &formatted).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!("failed to write '{}': {error}", canonical.display()),
            )
        })?;
    }
    Ok(FormatResult {
        canonical_path: canonical,
        original: source,
        formatted,
        changed,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    pub canonical_path: std::path::PathBuf,
    pub original: String,
    pub formatted: String,
    pub changed: bool,
}

impl FormatResult {
    pub fn line_count(&self) -> usize {
        self.formatted.lines().count()
    }

    pub fn changed_line_count(&self) -> usize {
        let original = self.original.lines().collect::<Vec<_>>();
        let formatted = self.formatted.lines().collect::<Vec<_>>();
        let shared = original.len().min(formatted.len());
        let mut changed = 0usize;
        for index in 0..shared {
            if original[index] != formatted[index] {
                changed += 1;
            }
        }
        changed + original.len().abs_diff(formatted.len())
    }
}

fn leading_closing_brace_count(line: &str) -> usize {
    let mut count = 0usize;
    for ch in line.chars() {
        if ch == '}' {
            count += 1;
        } else {
            break;
        }
    }
    count
}

fn update_brace_depth(line: &str, initial_depth: usize) -> usize {
    let mut depth = initial_depth;
    let mut single_quoted = false;
    let mut double_quoted = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if single_quoted || double_quoted => {
                escaped = true;
            }
            '"' if !single_quoted => {
                double_quoted = !double_quoted;
            }
            '\'' if !double_quoted => {
                single_quoted = !single_quoted;
            }
            '{' if !single_quoted && !double_quoted => {
                depth += 1;
            }
            '}' if !single_quoted && !double_quoted => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    depth
}

fn whole_document_range(text: &str) -> LspRange {
    let mut line = 0u32;
    let mut character = 0u32;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    LspRange {
        start: LspPosition {
            line: 0,
            character: 0,
        },
        end: LspPosition { line, character },
    }
}

#[cfg(test)]
mod tests {
    use super::{format_document, formatting_edit};
    use std::path::PathBuf;

    fn fixture(name: &str) -> String {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/formatter");
        std::fs::read_to_string(root.join(name)).expect("formatter fixture should load")
    }

    #[test]
    fn formatter_matches_record_fixture_snapshot() {
        let source = fixture("record.misformatted.fol");
        let expected = fixture("record.formatted.fol");

        assert_eq!(format_document(&source), expected);
    }

    #[test]
    fn formatter_matches_when_fixture_snapshot() {
        let source = fixture("when.misformatted.fol");
        let expected = fixture("when.formatted.fol");

        assert_eq!(format_document(&source), expected);
    }

    #[test]
    fn formatter_matches_build_fixture_snapshot() {
        let source = fixture("build.misformatted.fol");
        let expected = fixture("build.formatted.fol");

        assert_eq!(format_document(&source), expected);
    }

    #[test]
    fn formatter_matches_import_fixture_snapshot() {
        let source = fixture("imports.misformatted.fol");
        let expected = fixture("imports.formatted.fol");

        assert_eq!(format_document(&source), expected);
    }

    #[test]
    fn formatter_matches_alias_fixture_snapshot() {
        let source = fixture("alias.misformatted.fol");
        let expected = fixture("alias.formatted.fol");

        assert_eq!(format_document(&source), expected);
    }

    #[test]
    fn formatter_matches_nested_fixture_snapshot() {
        let source = fixture("nested.misformatted.fol");
        let expected = fixture("nested.formatted.fol");

        assert_eq!(format_document(&source), expected);
    }

    #[test]
    fn formatter_matches_broken_fixture_snapshot() {
        let source = fixture("broken.misformatted.fol");
        let expected = fixture("broken.formatted.fol");

        assert_eq!(format_document(&source), expected);
    }

    #[test]
    fn formatter_is_idempotent_on_formatted_output() {
        let fixtures = [
            "record.formatted.fol",
            "when.formatted.fol",
            "build.formatted.fol",
            "imports.formatted.fol",
            "alias.formatted.fol",
            "nested.formatted.fol",
            "broken.formatted.fol",
        ];

        for fixture_name in fixtures {
            let expected = fixture(fixture_name);

            assert_eq!(format_document(&expected), expected);
        }
    }

    #[test]
    fn formatting_edit_returns_full_document_edit_only_when_needed() {
        let source = fixture("when.misformatted.fol");
        let expected = fixture("when.formatted.fol");

        let edit = formatting_edit(&source).expect("misformatted source should need an edit");
        assert_eq!(edit.new_text, expected);
        assert_eq!(formatting_edit(&expected), None);
    }

    #[test]
    fn formatter_normalizes_crlf_trailing_space_and_final_newline() {
        let source = "fun[] main(): int = {\r\n    return 7;   \r\n};";

        assert_eq!(format_document(source), "fun[] main(): int = {\n    return 7;\n};\n");
    }

    #[test]
    fn formatter_collapses_blank_line_runs_and_leading_blank_lines() {
        let source = "\n\nfun[] helper(): int = {\n    return 7;\n};

\nfun[] main(): int = {\n    return helper();\n};\n";

        assert_eq!(
            format_document(source),
            "fun[] helper(): int = {\n    return 7;\n};

fun[] main(): int = {\n    return helper();\n};\n"
        );
    }

    #[test]
    fn formatting_edit_range_covers_the_original_whole_document() {
        let source = "fun[] main(): int = {\r\nreturn 7;\r\n};";
        let edit = formatting_edit(source).expect("source should need formatting");

        assert_eq!(edit.range.start.line, 0);
        assert_eq!(edit.range.start.character, 0);
        assert_eq!(edit.range.end.line, 2);
        assert_eq!(edit.range.end.character, 2);
    }
}

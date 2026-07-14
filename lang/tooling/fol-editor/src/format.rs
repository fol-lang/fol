use crate::source_scan::{mask_comments, scan_source, BraceEvent, BraceKind};
use crate::{EditorError, EditorErrorKind, EditorResult, LspPosition, LspRange};

const INDENT_WIDTH: usize = 4;

pub fn format_document(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let source_scan = scan_source(&normalized);
    let mut depth = 0usize;
    let mut lines: Vec<String> = Vec::new();

    for (line_index, raw_line) in normalized.split('\n').enumerate() {
        let protection = source_scan.lines[line_index];
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            if protection.starts_protected || protection.ends_protected {
                lines.push(raw_line.to_string());
                continue;
            }
            if matches!(lines.last(), Some(previous) if !previous.is_empty()) {
                lines.push(String::new());
            }
            continue;
        }

        let line_events = source_scan
            .braces
            .iter()
            .filter(|event| event.position.line == line_index as u32)
            .copied()
            .collect::<Vec<_>>();
        let indent_depth =
            depth.saturating_sub(leading_closing_brace_count(raw_line, &line_events));
        if protection.starts_protected {
            lines.push(raw_line.to_string());
        } else {
            let indent = " ".repeat(indent_depth * INDENT_WIDTH);
            let content = if protection.ends_protected {
                raw_line.trim_start()
            } else {
                trimmed
            };
            lines.push(format!("{indent}{content}"));
        }
        depth = update_brace_depth(&line_events, depth);
    }

    let ends_in_protected_content = source_scan.terminal_unclosed;
    if !ends_in_protected_content {
        while matches!(lines.last(), Some(line) if line.is_empty()) {
            lines.pop();
        }
    }

    if lines.is_empty() {
        String::new()
    } else if depth > 0 || ends_in_protected_content {
        // The document is structurally incomplete (for example mid-edit);
        // keep the truncated tail instead of appending a terminator inside it.
        let joined = lines.join("\n");
        if joined.ends_with('\n') {
            joined
        } else {
            format!("{joined}\n")
        }
    } else {
        let mut joined = lines.join("\n");
        let comment_masked = mask_comments(&joined);
        if let Some((index, last_code)) = comment_masked
            .char_indices()
            .rev()
            .find(|(_, ch)| !ch.is_whitespace())
        {
            if last_code != ';' {
                joined.insert(index + last_code.len_utf8(), ';');
            }
        }
        if joined.ends_with('\n') {
            joined
        } else {
            format!("{joined}\n")
        }
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

fn leading_closing_brace_count(line: &str, events: &[BraceEvent]) -> usize {
    let first_non_whitespace = line.chars().position(|ch| !ch.is_whitespace()).unwrap_or(0) as u32;
    let mut expected_character = first_non_whitespace;
    events
        .iter()
        .take_while(|event| {
            if event.kind == BraceKind::Close && event.position.character == expected_character {
                expected_character += 1;
                true
            } else {
                false
            }
        })
        .count()
}

fn update_brace_depth(events: &[BraceEvent], initial_depth: usize) -> usize {
    events
        .iter()
        .fold(initial_depth, |depth, event| match event.kind {
            BraceKind::Open => depth + 1,
            BraceKind::Close => depth.saturating_sub(1),
        })
}

fn whole_document_range(text: &str) -> LspRange {
    let mut line = 0u32;
    let mut character = 0u32;
    for ch in text.chars() {
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += ch.len_utf16() as u32;
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
    fn formatting_edits_end_at_utf16_document_positions() {
        let edit = formatting_edit("😀").expect("unterminated source should be formatted");
        assert_eq!(edit.range.end.line, 0);
        assert_eq!(edit.range.end.character, 2);
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
    fn formatter_preserves_v3_memory_and_processor_surfaces() {
        let source = "fun[] coordinate(counter[mux]: Counter): int / int = {\n\
        @var owner: Node = { value = 1 };\n\
        var[bor] view: Node = #owner;\n\
        var pointer: ptr[shared, Node] = &view;\n\
        var inner: ptr[int] = &seed;\n\
        var outer: ptr[ptr[int]] = &inner;\n\
        var extracted: ptr[int] = *outer;\n\
        var[bor] pointer_view: ptr[int] = extracted;\n\
        inspect(pointer_view);\n\
        inspect(pointer_view);\n\
        !pointer_view;\n\
        dfr {\n\
        !view;\n\
        };\n\
        edf {\n\
        var ignored: int = 0;\n\
        };\n\
        var channel: chn[int];\n\
        [>]std::io::echo_int(worker(channel[tx]));\n\
        var pending = std::fmt::double(work(1)) | async;\n\
        select {};\n\
        return pending | await;\n\
        };\n";
        let expected = concat!(
            "fun[] coordinate(counter[mux]: Counter): int / int = {\n",
            "    @var owner: Node = { value = 1 };\n",
            "    var[bor] view: Node = #owner;\n",
            "    var pointer: ptr[shared, Node] = &view;\n",
            "    var inner: ptr[int] = &seed;\n",
            "    var outer: ptr[ptr[int]] = &inner;\n",
            "    var extracted: ptr[int] = *outer;\n",
            "    var[bor] pointer_view: ptr[int] = extracted;\n",
            "    inspect(pointer_view);\n",
            "    inspect(pointer_view);\n",
            "    !pointer_view;\n",
            "    dfr {\n",
            "        !view;\n",
            "    };\n",
            "    edf {\n",
            "        var ignored: int = 0;\n",
            "    };\n",
            "    var channel: chn[int];\n",
            "    [>]std::io::echo_int(worker(channel[tx]));\n",
            "    var pending = std::fmt::double(work(1)) | async;\n",
            "    select {};\n",
            "    return pending | await;\n",
            "};\n",
        );

        assert_eq!(format_document(source), expected);
        assert_eq!(format_document(expected), expected);
    }

    #[test]
    fn formatter_ignores_braces_inside_comments_and_multiline_quotes() {
        let source = concat!(
            "fun[] main(): int = {\n",
            "` comment {\n",
            "  } still comment `\n",
            "/* compatibility {\n",
            "   } comment */\n",
            "// { } line comment\n",
            "var cooked = \"text {\n",
            " one } text\";\n",
            "var raw = 'text {\n",
            "  two } text';\n",
            "return 0;\n",
            "};\n",
        );
        let expected = concat!(
            "fun[] main(): int = {\n",
            "    ` comment {\n",
            "  } still comment `\n",
            "    /* compatibility {\n",
            "   } comment */\n",
            "    // { } line comment\n",
            "    var cooked = \"text {\n",
            " one } text\";\n",
            "    var raw = 'text {\n",
            "  two } text';\n",
            "    return 0;\n",
            "};\n",
        );

        assert_eq!(format_document(source), expected);
        assert_eq!(format_document(expected), expected);
    }

    #[test]
    fn formatter_preserves_unclosed_multiline_payload_tail() {
        let source = "fun[] main(): int = {\nvar text = \"one {\n\n";
        let expected = "fun[] main(): int = {\n    var text = \"one {\n\n";

        assert_eq!(format_document(source), expected);
        assert!(!format_document(source).ends_with(";\n"));
    }

    #[test]
    fn formatter_terminates_a_closed_multiline_quote_on_the_final_line() {
        let source = "var text: str = \"first\nsecond\"";
        let expected = "var text: str = \"first\nsecond\";\n";

        assert_eq!(format_document(source), expected);
        assert_eq!(format_document(expected), expected);
    }

    #[test]
    fn formatter_places_the_final_terminator_before_trailing_comments() {
        let cases = [
            (
                "var value: int = 1 // keep me",
                "var value: int = 1; // keep me\n",
            ),
            (
                "var value: int = 1 /* keep me */",
                "var value: int = 1; /* keep me */\n",
            ),
            (
                "var value: int = 1 ` keep me `",
                "var value: int = 1; ` keep me `\n",
            ),
            (
                "var value: int = 1; // keep me",
                "var value: int = 1; // keep me\n",
            ),
        ];

        for (source, expected) in cases {
            assert_eq!(format_document(source), expected, "{source:?}");
            assert_eq!(format_document(expected), expected, "{source:?}");
        }
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

        assert_eq!(
            format_document(source),
            "fun[] main(): int = {\n    return 7;\n};\n"
        );
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

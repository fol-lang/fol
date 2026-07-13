use crate::LspPosition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BraceKind {
    Open,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BraceEvent {
    pub kind: BraceKind,
    pub position: LspPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineProtection {
    pub starts_protected: bool,
    pub ends_protected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceScan {
    pub braces: Vec<BraceEvent>,
    pub lines: Vec<LineProtection>,
    pub masked_code: String,
    pub comment_masked_code: String,
    protected_bytes: Vec<bool>,
    pub terminal_protected: bool,
    pub terminal_unclosed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    Code,
    CookedQuote,
    RawQuote,
    BacktickComment,
    SlashLineComment,
    SlashBlockComment,
}

impl ScanState {
    fn protects_line_content(self) -> bool {
        matches!(
            self,
            Self::CookedQuote | Self::RawQuote | Self::BacktickComment | Self::SlashBlockComment
        )
    }

    fn protects_cursor(self) -> bool {
        self.protects_line_content() || self == Self::SlashLineComment
    }
}

/// Scan lexical protection boundaries and return only braces that are real
/// syntax. The recognized quote/comment forms match the compiler lexer.
pub(crate) fn scan_source(text: &str) -> SourceScan {
    let chars = text.chars().collect::<Vec<_>>();
    let mut braces = Vec::new();
    let mut lines = Vec::new();
    let mut masked_code = String::with_capacity(text.len());
    let mut comment_masked_code = String::with_capacity(text.len());
    let mut protected_bytes = Vec::with_capacity(text.len());
    let mut state = ScanState::Code;
    let mut line_start_state = state;
    let mut escaped = false;
    let mut line = 0u32;
    let mut character = 0u32;
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];
        let next = chars.get(index + 1).copied();
        let character_is_protected = state.protects_cursor()
            || matches!(
                (state, ch, next),
                (ScanState::Code, '"' | '\'' | '`', _) | (ScanState::Code, '/', Some('/' | '*'))
            );
        protected_bytes.extend(std::iter::repeat_n(character_is_protected, ch.len_utf8()));
        match state {
            ScanState::Code => match (ch, next) {
                ('"', _) => {
                    push_masked(&mut masked_code, ch);
                    comment_masked_code.push(ch);
                    state = ScanState::CookedQuote;
                    escaped = false;
                }
                ('\'', _) => {
                    push_masked(&mut masked_code, ch);
                    comment_masked_code.push(ch);
                    state = ScanState::RawQuote;
                }
                ('`', _) => {
                    push_masked(&mut masked_code, ch);
                    push_masked(&mut comment_masked_code, ch);
                    state = ScanState::BacktickComment;
                }
                ('/', Some('/')) => {
                    push_masked(&mut masked_code, ch);
                    push_masked(&mut comment_masked_code, ch);
                    state = ScanState::SlashLineComment;
                }
                ('/', Some('*')) => {
                    push_masked(&mut masked_code, ch);
                    push_masked(&mut comment_masked_code, ch);
                    state = ScanState::SlashBlockComment;
                }
                ('{', _) => {
                    masked_code.push(ch);
                    comment_masked_code.push(ch);
                    braces.push(BraceEvent {
                        kind: BraceKind::Open,
                        position: LspPosition { line, character },
                    });
                }
                ('}', _) => {
                    masked_code.push(ch);
                    comment_masked_code.push(ch);
                    braces.push(BraceEvent {
                        kind: BraceKind::Close,
                        position: LspPosition { line, character },
                    });
                }
                _ => {
                    masked_code.push(ch);
                    comment_masked_code.push(ch);
                }
            },
            ScanState::CookedQuote => {
                push_masked(&mut masked_code, ch);
                comment_masked_code.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    state = ScanState::Code;
                }
            }
            ScanState::RawQuote => {
                push_masked(&mut masked_code, ch);
                comment_masked_code.push(ch);
                if ch == '\'' {
                    state = ScanState::Code;
                }
            }
            ScanState::BacktickComment => {
                push_masked(&mut masked_code, ch);
                push_masked(&mut comment_masked_code, ch);
                if ch == '`' {
                    state = ScanState::Code;
                }
            }
            ScanState::SlashLineComment => {
                push_masked(&mut masked_code, ch);
                push_masked(&mut comment_masked_code, ch);
                if ch == '\n' {
                    state = ScanState::Code;
                }
            }
            ScanState::SlashBlockComment => {
                push_masked(&mut masked_code, ch);
                push_masked(&mut comment_masked_code, ch);
                if ch == '*' && next == Some('/') {
                    // Consume the closing slash here so it cannot be mistaken
                    // for the start of another comment after returning to code.
                    index += 1;
                    character += 1;
                    push_masked(&mut masked_code, '/');
                    push_masked(&mut comment_masked_code, '/');
                    protected_bytes.push(true);
                    state = ScanState::Code;
                }
            }
        }

        if ch == '\n' {
            lines.push(LineProtection {
                starts_protected: line_start_state.protects_line_content(),
                ends_protected: state.protects_line_content(),
            });
            if state == ScanState::SlashLineComment {
                state = ScanState::Code;
            }
            line_start_state = state;
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
        index += 1;
    }

    lines.push(LineProtection {
        starts_protected: line_start_state.protects_line_content(),
        ends_protected: state.protects_line_content(),
    });

    let terminal_protected = state.protects_cursor();
    let terminal_unclosed = state.protects_line_content();
    SourceScan {
        braces,
        lines,
        masked_code,
        comment_masked_code,
        protected_bytes,
        terminal_protected,
        terminal_unclosed,
    }
}

fn push_masked(output: &mut String, ch: char) {
    if ch == '\n' {
        output.push(ch);
    } else {
        output.extend(std::iter::repeat_n(' ', ch.len_utf8()));
    }
}

/// Return source braces that are real syntax, excluding braces inside every
/// quoted/comment form accepted by the compiler lexer.
pub(crate) fn brace_events(text: &str) -> Vec<BraceEvent> {
    scan_source(text).braces
}

/// Replace comments and quoted payloads with spaces while retaining newlines
/// and byte offsets. Text-only editor recovery paths can then inspect code
/// without inventing their own partial lexer.
pub(crate) fn mask_non_code(text: &str) -> String {
    scan_source(text).masked_code
}

/// Replace comments with spaces while retaining quoted literals, newlines,
/// and byte offsets. This lets the formatter inspect the final real code token
/// without mistaking comment payload for syntax or losing a quoted value.
pub(crate) fn mask_comments(text: &str) -> String {
    scan_source(text).comment_masked_code
}

/// Return whether the source character at an editor position belongs to a
/// compiler-recognized comment or quoted form. The scanner's mask preserves
/// byte offsets, while the position conversion follows the editor's existing
/// character-based convention.
pub(crate) fn position_is_protected(text: &str, position: LspPosition) -> bool {
    let scan = scan_source(text);
    let mut line_start = 0_usize;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        if line_index == position.line as usize {
            let content = line.strip_suffix('\n').unwrap_or(line);
            if let Some((byte_in_line, _)) = content.char_indices().nth(position.character as usize)
            {
                return scan
                    .protected_bytes
                    .get(line_start + byte_in_line)
                    .copied()
                    .unwrap_or(false);
            }
            if position.character as usize == content.chars().count() {
                return if line.ends_with('\n') {
                    scan.protected_bytes
                        .get(line_start + content.len())
                        .copied()
                        .unwrap_or(false)
                } else {
                    scan.terminal_protected
                };
            }
            return false;
        }
        line_start += line.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{
        brace_events, mask_comments, mask_non_code, position_is_protected, scan_source, BraceEvent,
        BraceKind, LineProtection,
    };
    use crate::LspPosition;

    #[test]
    fn scanner_ignores_braces_in_all_comment_and_quote_forms() {
        let source = concat!(
            "fun[] main(): int = {\n",
            "    ` backtick {\n",
            "      } comment `\n",
            "    /* block {\n",
            "       } comment */\n",
            "    // line { } comment\n",
            "    var cooked = \"quoted {\n",
            "        } text\";\n",
            "    var raw = 'quoted {\n",
            "        } text';\n",
            "    return 0;\n",
            "};\n",
        );

        assert_eq!(
            brace_events(source),
            vec![
                BraceEvent {
                    kind: BraceKind::Open,
                    position: LspPosition {
                        line: 0,
                        character: 20,
                    },
                },
                BraceEvent {
                    kind: BraceKind::Close,
                    position: LspPosition {
                        line: 11,
                        character: 0,
                    },
                },
            ]
        );
    }

    #[test]
    fn scanner_resumes_after_comments_and_quotes_close() {
        let source = "/* } */ { ` } ` } // {\n'{ }' { \"}\" }";
        let kinds = brace_events(source)
            .into_iter()
            .map(|event| event.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                BraceKind::Open,
                BraceKind::Close,
                BraceKind::Open,
                BraceKind::Close,
            ]
        );
    }

    #[test]
    fn scanner_marks_multiline_payload_lines_as_protected() {
        let source = "var text = \"first\nsecond\";\n`note\nbody`\n/* block\nbody */\n";
        let lines = scan_source(source).lines;

        assert_eq!(
            lines,
            vec![
                LineProtection {
                    starts_protected: false,
                    ends_protected: true,
                },
                LineProtection {
                    starts_protected: true,
                    ends_protected: false,
                },
                LineProtection {
                    starts_protected: false,
                    ends_protected: true,
                },
                LineProtection {
                    starts_protected: true,
                    ends_protected: false,
                },
                LineProtection {
                    starts_protected: false,
                    ends_protected: true,
                },
                LineProtection {
                    starts_protected: true,
                    ends_protected: false,
                },
                LineProtection {
                    starts_protected: false,
                    ends_protected: false,
                },
            ]
        );
    }

    #[test]
    fn scanner_mask_preserves_lines_and_byte_offsets() {
        let source = "alpha `µ {` beta // }\n/* { */ gamma \"}λ\" delta";
        let masked = mask_non_code(source);

        assert_eq!(masked.len(), source.len());
        assert_eq!(masked.matches('\n').count(), 1);
        assert_eq!(masked.find("alpha"), source.find("alpha"));
        assert_eq!(masked.find("beta"), source.find("beta"));
        assert_eq!(masked.find("gamma"), source.find("gamma"));
        assert_eq!(masked.find("delta"), source.find("delta"));
        assert!(!masked.contains('{'));
        assert!(!masked.contains('}'));
        assert!(!masked.contains('µ'));
        assert!(!masked.contains('λ'));
    }

    #[test]
    fn scanner_reports_terminal_comment_and_quote_state() {
        let line_comment = scan_source("value. // comment");
        assert!(line_comment.terminal_protected);
        assert!(!line_comment.terminal_unclosed);

        let closed_quote = scan_source("\"first\nsecond\"");
        assert!(!closed_quote.terminal_protected);
        assert!(!closed_quote.terminal_unclosed);

        for source in ["\"open", "'open", "` open", "/* open"] {
            let scan = scan_source(source);
            assert!(scan.terminal_protected, "{source:?}");
            assert!(scan.terminal_unclosed, "{source:?}");
        }
    }

    #[test]
    fn comment_mask_preserves_quotes_and_hides_comments() {
        let source = "var text = \"quoted // text\"; /* hidden */ // tail";
        let masked = mask_comments(source);

        assert_eq!(masked.len(), source.len());
        assert!(masked.contains("\"quoted // text\""));
        assert!(!masked.contains("hidden"));
        assert!(!masked.contains("tail"));
        assert!(masked.contains("var text"));
    }

    #[test]
    fn scanner_reports_protected_editor_positions_with_unicode_offsets() {
        let source = concat!(
            "µ code async\n",
            "// µ comment await\n",
            "var cooked = \"µ [>] edf\";\n",
            "var raw = 'µ *pointer @Node';\n",
            "` µ mux tx rx `\n",
            "/* µ dfr */\n",
        );
        let position = |line: u32, needle: &str| {
            let source_line = source.lines().nth(line as usize).unwrap();
            LspPosition {
                line,
                character: source_line[..source_line.find(needle).unwrap()]
                    .chars()
                    .count() as u32,
            }
        };

        assert!(!position_is_protected(source, position(0, "async")));
        assert!(position_is_protected(source, position(1, " await")));
        let end_of = |line: u32| LspPosition {
            line,
            character: source.lines().nth(line as usize).unwrap().chars().count() as u32,
        };
        assert!(!position_is_protected(source, end_of(0)));
        assert!(position_is_protected(source, end_of(1)));
        for (line, needle) in [
            (1, "await"),
            (2, "[>]"),
            (2, "edf"),
            (3, "*pointer"),
            (3, "@Node"),
            (4, "mux"),
            (4, "tx"),
            (4, "rx"),
            (5, "dfr"),
        ] {
            assert!(
                position_is_protected(source, position(line, needle)),
                "'{needle}' on line {line} should be protected"
            );
        }
    }
}

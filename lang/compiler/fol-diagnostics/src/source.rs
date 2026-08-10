use crate::DiagnosticLocation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSnippetError {
    MissingFilePath,
    ReadFailed,
    MissingLine,
}

pub fn load_source_line(location: &DiagnosticLocation) -> Result<String, SourceSnippetError> {
    let file = location
        .file
        .as_ref()
        .ok_or(SourceSnippetError::MissingFilePath)?;
    let contents = std::fs::read_to_string(file).map_err(|_| SourceSnippetError::ReadFailed)?;
    contents
        .lines()
        .nth(location.line.saturating_sub(1))
        .map(|line| line.to_string())
        .ok_or(SourceSnippetError::MissingLine)
}

/// Caret run under `source_line` for the location's span.
///
/// A span may run past the end of its first line (an unterminated literal runs
/// to end of file); the carets stop at the line end so the underline always
/// sits beneath real source text.
pub fn primary_underline(location: &DiagnosticLocation, source_line: &str) -> String {
    let start = location.column.saturating_sub(1);
    let line_width = source_line.chars().count();
    let available = line_width.saturating_sub(start).max(1);
    let width = location.length.unwrap_or(1).max(1).min(available);
    format!("{}{}", " ".repeat(start), "^".repeat(width))
}

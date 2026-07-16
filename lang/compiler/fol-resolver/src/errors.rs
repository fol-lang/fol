use crate::model::SymbolKind;
use fol_diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticLocation, DiagnosticSuggestion, ToDiagnostic,
    ToDiagnosticLocation,
};
use fol_package::PackageError;
use fol_parser::ast::SyntaxOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverErrorKind {
    InvalidInput,
    Unsupported,
    UnresolvedName,
    DuplicateSymbol,
    AmbiguousReference,
    ImportCycle,
    Internal,
}

impl ResolverErrorKind {
    pub fn diagnostic_code(self) -> DiagnosticCode {
        match self {
            Self::InvalidInput => DiagnosticCode::new("R1001"),
            Self::Unsupported => DiagnosticCode::new("R1002"),
            Self::UnresolvedName => DiagnosticCode::new("R1003"),
            Self::DuplicateSymbol => DiagnosticCode::new("R1004"),
            Self::AmbiguousReference => DiagnosticCode::new("R1005"),
            Self::ImportCycle => DiagnosticCode::new("R1006"),
            Self::Internal => DiagnosticCode::new("R1099"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverError {
    kind: ResolverErrorKind,
    message: String,
    origin: Option<Box<SyntaxOrigin>>,
    related_origins: Vec<(SyntaxOrigin, String)>,
    suggestions: Vec<DiagnosticSuggestion>,
}

impl ResolverError {
    pub fn new(kind: ResolverErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: None,
            related_origins: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn with_origin(
        kind: ResolverErrorKind,
        message: impl Into<String>,
        origin: SyntaxOrigin,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: Some(Box::new(origin)),
            related_origins: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn kind(&self) -> ResolverErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn origin(&self) -> Option<&SyntaxOrigin> {
        self.origin.as_deref()
    }

    pub fn diagnostic_location(&self) -> Option<DiagnosticLocation> {
        self.origin.as_ref().map(|origin| DiagnosticLocation {
            file: origin.file.clone(),
            line: origin.line,
            column: origin.column,
            length: Some(origin.length),
        })
    }

    pub fn with_related_origin(mut self, origin: SyntaxOrigin, message: impl Into<String>) -> Self {
        self.related_origins.push((origin, message.into()));
        self
    }

    pub fn with_suggestion(mut self, suggestion: DiagnosticSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }
}

impl std::fmt::Display for ResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ResolverError {}

impl ToDiagnosticLocation for ResolverError {
    fn to_diagnostic_location(&self, file: Option<String>) -> DiagnosticLocation {
        if let Some(origin) = &self.origin {
            DiagnosticLocation {
                file: file.or_else(|| origin.file.clone()),
                line: origin.line,
                column: origin.column,
                length: Some(origin.length),
            }
        } else {
            DiagnosticLocation {
                file,
                line: 1,
                column: 1,
                length: None,
            }
        }
    }
}

impl ToDiagnostic for ResolverError {
    fn to_diagnostic(&self) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(self.kind.diagnostic_code(), self.to_string());
        if let Some(location) = self.diagnostic_location() {
            diagnostic = diagnostic.with_primary_label(location);
        }
        for (origin, message) in &self.related_origins {
            diagnostic = diagnostic.with_secondary_label(
                DiagnosticLocation {
                    file: origin.file.clone(),
                    line: origin.line,
                    column: origin.column,
                    length: Some(origin.length),
                },
                message.clone(),
            );
        }
        for suggestion in &self.suggestions {
            diagnostic = diagnostic.with_suggestion(suggestion.clone());
        }
        if self.kind == ResolverErrorKind::Unsupported && self.message.contains("imports yet") {
            diagnostic = diagnostic.with_note("supported import source kinds are loc and pkg");
        }
        if self.message.contains("requires an explicit std root")
            || self.message.contains("requires bundled std at")
        {
            diagnostic = diagnostic.with_help("rerun with --std-root <DIR>");
        }
        if self
            .message
            .contains("requires an explicit package store root")
        {
            diagnostic = diagnostic.with_help("rerun with --package-store-root <DIR>");
        }
        if self.message.contains("pkg instead of loc") {
            diagnostic =
                diagnostic.with_help("replace the import source kind with pkg for formal packages");
        }
        diagnostic
    }
}

impl From<PackageError> for ResolverError {
    fn from(error: PackageError) -> Self {
        let kind = match error.kind() {
            fol_package::PackageErrorKind::InvalidInput => ResolverErrorKind::InvalidInput,
            fol_package::PackageErrorKind::Unsupported => ResolverErrorKind::Unsupported,
            fol_package::PackageErrorKind::ImportCycle => ResolverErrorKind::ImportCycle,
            fol_package::PackageErrorKind::Internal => ResolverErrorKind::Internal,
        };
        let message = resolver_package_message(error.message());
        match error.origin().cloned() {
            Some(origin) => ResolverError::with_origin(kind, message, origin),
            None => ResolverError::new(kind, message),
        }
    }
}

fn resolver_package_message(message: &str) -> String {
    if let Some(rest) = message.strip_prefix("package loader ") {
        return format!("resolver {rest}");
    }
    if let Some(rest) = message.strip_prefix("package loading ") {
        return format!("resolver {rest}");
    }
    if let Some(rest) = message.strip_prefix("package loc import target ") {
        return format!("resolver loc import target {rest}");
    }
    if let Some(rest) = message.strip_prefix("package std import target ") {
        return format!("resolver std import target {rest}");
    }
    if let Some(rest) = message.strip_prefix("package pkg import target ") {
        return format!("resolver pkg import target {rest}");
    }
    if let Some(rest) = message.strip_prefix("package entry import target ") {
        return format!("resolver entry import target {rest}");
    }
    message.to_string()
}

pub(crate) fn format_origin_brief(origin: &SyntaxOrigin) -> String {
    match &origin.file {
        Some(file) => format!("{file}:{}:{}", origin.line, origin.column),
        None => format!("line {}:{}", origin.line, origin.column),
    }
}

pub(crate) fn symbol_kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::ValueBinding => "value binding",
        SymbolKind::LabelBinding => "label binding",
        SymbolKind::DestructureBinding => "destructure binding",
        SymbolKind::Routine => "routine",
        SymbolKind::Type => "type",
        SymbolKind::Alias => "alias",
        SymbolKind::Definition => "definition",
        SymbolKind::Segment => "segment",
        SymbolKind::Standard => "standard",
        SymbolKind::ImportAlias => "import alias",
        SymbolKind::GenericParameter => "generic parameter",
        SymbolKind::Parameter => "parameter",
        SymbolKind::Capture => "capture",
        SymbolKind::LoopBinder => "loop binder",
        SymbolKind::RollingBinder => "rolling binder",
    }
}

#[cfg(test)]
mod tests {
    use super::{resolver_package_message, ResolverError, ResolverErrorKind};
    use fol_diagnostics::{DiagnosticCode, DiagnosticReport, ToDiagnostic};
    use fol_parser::ast::SyntaxOrigin;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn resolver_error_formats_without_kind_prefix() {
        let error = ResolverError::new(
            ResolverErrorKind::Unsupported,
            "import source kind is not implemented yet",
        );

        assert_eq!(
            error.to_string(),
            "import source kind is not implemented yet"
        );
    }

    #[test]
    fn resolver_error_exposes_diagnostic_location_from_origin() {
        let error = ResolverError::with_origin(
            ResolverErrorKind::UnresolvedName,
            "could not resolve `answer`",
            SyntaxOrigin {
                file: Some("pkg/main.fol".to_string()),
                line: 12,
                column: 4,
                length: 6,
            },
        );

        let location = error
            .diagnostic_location()
            .expect("Resolver errors with syntax origins should have diagnostic locations");

        assert_eq!(location.file.as_deref(), Some("pkg/main.fol"));
        assert_eq!(location.line, 12);
        assert_eq!(location.column, 4);
        assert_eq!(location.length, Some(6));
    }

    #[test]
    fn resolver_error_integrates_with_diagnostic_report() {
        let error = ResolverError::with_origin(
            ResolverErrorKind::DuplicateSymbol,
            "duplicate symbol `value`",
            SyntaxOrigin {
                file: Some("pkg/main.fol".to_string()),
                line: 7,
                column: 1,
                length: 5,
            },
        );
        let mut report = DiagnosticReport::new();

        report.add_error(error.to_string(), error.diagnostic_location());

        assert!(report.has_errors());
        let rendered = report.output(fol_diagnostics::OutputFormat::Json);
        assert!(rendered.contains("duplicate symbol `value`"));
        assert!(rendered.contains("pkg/main.fol"));
        assert!(rendered.contains("\"line\": 7"));
    }

    #[test]
    fn resolver_error_translation_rewrites_package_loader_prefixes() {
        assert_eq!(
            resolver_package_message(
                "package loader could not inspect loc import target '/tmp/x': nope"
            ),
            "resolver could not inspect loc import target '/tmp/x': nope"
        );
        assert_eq!(
            resolver_package_message("package std import target '/tmp/std/fmt' does not exist"),
            "resolver std import target '/tmp/std/fmt' does not exist"
        );
    }

    #[test]
    fn resolver_error_kinds_map_to_stable_diagnostic_codes() {
        assert_eq!(
            ResolverErrorKind::InvalidInput.diagnostic_code(),
            DiagnosticCode::new("R1001")
        );
        assert_eq!(
            ResolverErrorKind::Unsupported.diagnostic_code(),
            DiagnosticCode::new("R1002")
        );
        assert_eq!(
            ResolverErrorKind::UnresolvedName.diagnostic_code(),
            DiagnosticCode::new("R1003")
        );
        assert_eq!(
            ResolverErrorKind::DuplicateSymbol.diagnostic_code(),
            DiagnosticCode::new("R1004")
        );
        assert_eq!(
            ResolverErrorKind::AmbiguousReference.diagnostic_code(),
            DiagnosticCode::new("R1005")
        );
        assert_eq!(
            ResolverErrorKind::ImportCycle.diagnostic_code(),
            DiagnosticCode::new("R1006")
        );
        assert_eq!(
            ResolverErrorKind::Internal.diagnostic_code(),
            DiagnosticCode::new("R1099")
        );
    }

    #[test]
    fn resolver_error_human_output_keeps_snippet_shape() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be stable enough for temp file names")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fol_resolver_diagnostic_{stamp}.fol"));
        std::fs::write(&path, "return answer;\n")
            .expect("resolver diagnostic fixture should be writable");
        let error = ResolverError::with_origin(
            ResolverErrorKind::UnresolvedName,
            "could not resolve `answer`",
            SyntaxOrigin {
                file: Some(path.to_string_lossy().into_owned()),
                line: 1,
                column: 8,
                length: 6,
            },
        );
        let mut report = DiagnosticReport::new();

        report.add_error(error.to_string(), error.diagnostic_location());

        let rendered = report.output(fol_diagnostics::OutputFormat::Human);
        let _ = std::fs::remove_file(&path);

        assert!(rendered.contains("error: could not resolve `answer`"));
        assert!(rendered.contains("| return answer;"));
        assert!(rendered.contains("|        ^^^^^^"));
    }

    #[test]
    fn resolver_error_to_diagnostic_preserves_explicit_resolver_codes() {
        let error = ResolverError::with_origin(
            ResolverErrorKind::UnresolvedName,
            "could not resolve `answer`",
            SyntaxOrigin {
                file: Some("pkg/main.fol".to_string()),
                line: 2,
                column: 12,
                length: 6,
            },
        );
        let mut report = DiagnosticReport::new();

        report.add_from(&error);

        let rendered = report.output(fol_diagnostics::OutputFormat::Json);
        assert!(rendered.contains("\"code\": \"R1003\""));
        assert!(rendered.contains("could not resolve `answer`"));
    }

    #[test]
    fn resolver_error_to_diagnostic_keeps_related_origins_as_secondary_labels() {
        let diagnostic = ResolverError::with_origin(
            ResolverErrorKind::DuplicateSymbol,
            "duplicate symbol 'value'",
            SyntaxOrigin {
                file: Some("pkg/10_second.fol".to_string()),
                line: 1,
                column: 1,
                length: 5,
            },
        )
        .with_related_origin(
            SyntaxOrigin {
                file: Some("pkg/00_first.fol".to_string()),
                line: 1,
                column: 1,
                length: 5,
            },
            "first declaration",
        )
        .to_diagnostic();

        assert_eq!(diagnostic.labels.len(), 2);
        assert_eq!(
            diagnostic.labels[1].message.as_deref(),
            Some("first declaration")
        );
        assert_eq!(
            diagnostic.labels[1].location.file.as_deref(),
            Some("pkg/00_first.fol")
        );
    }

    #[test]
    fn resolver_error_to_diagnostic_adds_help_for_missing_bundled_std_or_override() {
        let diagnostic = ResolverError::new(
            ResolverErrorKind::InvalidInput,
            "resolver std import 'fmt' requires bundled std at '/tmp/std' or an explicit --std-root <DIR> override",
        )
        .to_diagnostic();

        assert_eq!(
            diagnostic.helps,
            vec!["rerun with --std-root <DIR>".to_string()]
        );
    }

    #[test]
    fn resolver_error_to_diagnostic_adds_note_for_unsupported_import_kinds() {
        let diagnostic = ResolverError::new(
            ResolverErrorKind::Unsupported,
            "resolver does not support 'url' imports yet",
        )
        .to_diagnostic();

        assert_eq!(
            diagnostic.notes,
            vec!["supported import source kinds are loc and pkg".to_string()]
        );
    }

    #[test]
    fn resolver_error_to_diagnostic_adds_help_for_formal_packages_imported_via_loc() {
        let diagnostic = ResolverError::new(
            ResolverErrorKind::InvalidInput,
            "resolver loc import target '/tmp/formal_pkg' contains '/tmp/formal_pkg/build.fol'; formal package roots must be imported with pkg instead of loc",
        )
        .to_diagnostic();

        assert_eq!(
            diagnostic.helps,
            vec!["replace the import source kind with pkg for formal packages".to_string()]
        );
    }
}

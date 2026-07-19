use fol_diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticLocation, ToDiagnostic, ToDiagnosticLocation,
};
use fol_parser::ast::SyntaxOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageErrorKind {
    InvalidInput,
    Unsupported,
    ImportCycle,
    Internal,
}

impl PackageErrorKind {
    pub fn diagnostic_code(self) -> DiagnosticCode {
        match self {
            Self::InvalidInput => DiagnosticCode::new("K1001"),
            Self::Unsupported => DiagnosticCode::new("K1002"),
            Self::ImportCycle => DiagnosticCode::new("K1003"),
            Self::Internal => DiagnosticCode::new("K1099"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageError {
    kind: PackageErrorKind,
    message: String,
    origin: Option<SyntaxOrigin>,
    related_origins: Vec<(SyntaxOrigin, String)>,
}

impl PackageError {
    pub fn new(kind: PackageErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: None,
            related_origins: Vec::new(),
        }
    }

    pub fn with_origin(
        kind: PackageErrorKind,
        message: impl Into<String>,
        origin: SyntaxOrigin,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: Some(origin),
            related_origins: Vec::new(),
        }
    }

    pub fn kind(&self) -> PackageErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn origin(&self) -> Option<&SyntaxOrigin> {
        self.origin.as_ref()
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
}

impl std::fmt::Display for PackageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PackageError {}

impl ToDiagnosticLocation for PackageError {
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

impl ToDiagnostic for PackageError {
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
        if self.message.contains("pkg instead of loc") {
            diagnostic =
                diagnostic.with_help("replace the import source kind with pkg for formal packages");
        }
        diagnostic
    }
}

#[cfg(test)]
mod tests {
    use super::{PackageError, PackageErrorKind};
    use fol_diagnostics::{DiagnosticCode, DiagnosticReport, ToDiagnostic};
    use fol_parser::ast::SyntaxOrigin;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn package_error_formats_without_kind_prefix() {
        let error = PackageError::new(
            PackageErrorKind::Unsupported,
            "package fetching is not implemented yet",
        );

        assert_eq!(error.to_string(), "package fetching is not implemented yet");
    }

    #[test]
    fn package_error_exposes_diagnostic_location_from_origin() {
        let error = PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            "invalid package metadata",
            SyntaxOrigin {
                file: Some("pkg/build.fol".to_string()),
                line: 4,
                column: 1,
                length: 4,
            },
        );

        let location = error
            .diagnostic_location()
            .expect("Package errors with syntax origins should expose diagnostic locations");

        assert_eq!(location.file.as_deref(), Some("pkg/build.fol"));
        assert_eq!(location.line, 4);
        assert_eq!(location.column, 1);
        assert_eq!(location.length, Some(4));
    }

    #[test]
    fn package_error_integrates_with_diagnostic_report() {
        let error = PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            "duplicate package metadata field",
            SyntaxOrigin {
                file: Some("pkg/build.fol".to_string()),
                line: 2,
                column: 1,
                length: 4,
            },
        );
        let mut report = DiagnosticReport::new();

        report.add_from(&error);

        assert!(report.has_errors());
        let rendered = report.output(fol_diagnostics::OutputFormat::Json);
        assert!(rendered.contains("duplicate package metadata field"));
        assert!(rendered.contains("pkg/build.fol"));
        assert!(rendered.contains("\"line\": 2"));
    }

    #[test]
    fn package_error_kinds_map_to_stable_diagnostic_codes() {
        assert_eq!(
            PackageErrorKind::InvalidInput.diagnostic_code(),
            DiagnosticCode::new("K1001")
        );
        assert_eq!(
            PackageErrorKind::Unsupported.diagnostic_code(),
            DiagnosticCode::new("K1002")
        );
        assert_eq!(
            PackageErrorKind::ImportCycle.diagnostic_code(),
            DiagnosticCode::new("K1003")
        );
        assert_eq!(
            PackageErrorKind::Internal.diagnostic_code(),
            DiagnosticCode::new("K1099")
        );
    }

    #[test]
    fn package_error_human_output_keeps_snippet_shape() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be stable enough for temp file names")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("fol_package_diagnostic_{stamp}.yaml"));
        std::fs::write(&path, "name: json\nversion: 1.0.0\n")
            .expect("package diagnostic fixture should be writable");
        let error = PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            "duplicate package metadata field",
            SyntaxOrigin {
                file: Some(path.to_string_lossy().into_owned()),
                line: 1,
                column: 1,
                length: 4,
            },
        );
        let mut report = DiagnosticReport::new();

        report.add_from(&error);

        let rendered = report.output(fol_diagnostics::OutputFormat::Human);
        let _ = std::fs::remove_file(&path);

        assert!(rendered.contains("error[K1001]: duplicate package metadata field"));
        assert!(rendered.contains("| name: json"));
        assert!(rendered.contains("| ^^^^"));
    }

    #[test]
    fn package_error_to_diagnostic_preserves_explicit_package_codes() {
        let error = PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            "duplicate package metadata field",
            SyntaxOrigin {
                file: Some("pkg/build.fol".to_string()),
                line: 2,
                column: 1,
                length: 4,
            },
        );
        let mut report = DiagnosticReport::new();

        report.add_from(&error);

        let rendered = report.output(fol_diagnostics::OutputFormat::Json);
        assert!(rendered.contains("\"code\": \"K1001\""));
        assert!(rendered.contains("duplicate package metadata field"));
    }

    #[test]
    fn package_error_to_diagnostic_keeps_related_origins_as_secondary_labels() {
        let diagnostic = PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            "duplicate package metadata field",
            SyntaxOrigin {
                file: Some("pkg/build.fol".to_string()),
                line: 3,
                column: 1,
                length: 4,
            },
        )
        .with_related_origin(
            SyntaxOrigin {
                file: Some("pkg/build.fol".to_string()),
                line: 1,
                column: 1,
                length: 4,
            },
            "first package metadata field declaration",
        )
        .to_diagnostic();

        assert_eq!(diagnostic.labels.len(), 2);
        assert_eq!(
            diagnostic.labels[1].message.as_deref(),
            Some("first package metadata field declaration")
        );
        assert_eq!(diagnostic.labels[1].location.line, 1);
    }

    #[test]
    fn package_error_to_diagnostic_adds_help_for_formal_packages_imported_via_loc() {
        let diagnostic = PackageError::new(
            PackageErrorKind::InvalidInput,
            "package loc import target '/tmp/formal_pkg' contains '/tmp/formal_pkg/build.fol'; formal package roots must be imported with pkg instead of loc",
        )
        .to_diagnostic();

        assert_eq!(
            diagnostic.helps,
            vec!["replace the import source kind with pkg for formal packages".to_string()]
        );
    }
}

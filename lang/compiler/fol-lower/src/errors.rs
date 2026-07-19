use fol_diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticLocation, ToDiagnostic, ToDiagnosticLocation,
};
use fol_parser::ast::SyntaxOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweringErrorKind {
    Unsupported,
    InvalidInput,
    Internal,
}

impl LoweringErrorKind {
    pub fn diagnostic_code(self) -> DiagnosticCode {
        match self {
            Self::Unsupported => DiagnosticCode::new("L1001"),
            Self::InvalidInput => DiagnosticCode::new("L1002"),
            Self::Internal => DiagnosticCode::new("L1099"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweringError {
    kind: LoweringErrorKind,
    message: String,
    origin: Option<SyntaxOrigin>,
}

impl LoweringError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: LoweringErrorKind::Unsupported,
            message: message.into(),
            origin: None,
        }
    }

    pub fn with_kind(kind: LoweringErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: None,
        }
    }

    pub fn with_origin(
        kind: LoweringErrorKind,
        message: impl Into<String>,
        origin: SyntaxOrigin,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: Some(origin),
        }
    }

    pub fn kind(&self) -> LoweringErrorKind {
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
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LoweringError {}

impl ToDiagnosticLocation for LoweringError {
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

impl ToDiagnostic for LoweringError {
    fn to_diagnostic(&self) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(self.kind.diagnostic_code(), self.to_string());
        if let Some(location) = self.diagnostic_location() {
            diagnostic = diagnostic.with_primary_label(location);
        }
        diagnostic
    }
}

#[cfg(test)]
mod tests {
    use super::{LoweringError, LoweringErrorKind};
    use fol_diagnostics::ToDiagnostic;
    use fol_parser::ast::SyntaxOrigin;

    #[test]
    fn lowering_errors_keep_kind_and_origin_information() {
        let error = LoweringError::with_origin(
            LoweringErrorKind::InvalidInput,
            "typed lowering input was incomplete",
            SyntaxOrigin {
                file: Some("pkg/main.fol".to_string()),
                line: 4,
                column: 7,
                length: 3,
            },
        );

        assert_eq!(error.kind(), LoweringErrorKind::InvalidInput);
        assert_eq!(
            error
                .diagnostic_location()
                .expect("lowering errors should expose diagnostic locations")
                .line,
            4
        );
        assert_eq!(error.to_string(), "typed lowering input was incomplete");
    }

    #[test]
    fn lowering_errors_lower_to_structured_diagnostics() {
        let diagnostic = LoweringError::with_origin(
            LoweringErrorKind::Unsupported,
            "generics are not yet supported",
            SyntaxOrigin {
                file: Some("pkg/build.fol".to_string()),
                line: 2,
                column: 1,
                length: 4,
            },
        )
        .to_diagnostic();

        assert_eq!(diagnostic.code.as_str(), "L1001");
        assert_eq!(
            diagnostic.primary_label().map(|label| label.location.line),
            Some(2)
        );
    }
}

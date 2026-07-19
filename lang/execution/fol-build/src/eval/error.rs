use super::capabilities::ForbiddenBuildTimeOperation;
use fol_diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticLocation, ToDiagnostic, ToDiagnosticLocation,
};
use fol_parser::ast::SyntaxOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEvaluationErrorKind {
    InvalidInput,
    Unsupported,
    ValidationFailed,
    Internal,
}

impl BuildEvaluationErrorKind {
    pub fn diagnostic_code(self) -> DiagnosticCode {
        match self {
            Self::InvalidInput => DiagnosticCode::new("K1101"),
            Self::Unsupported => DiagnosticCode::new("K1102"),
            Self::ValidationFailed => DiagnosticCode::new("K1103"),
            Self::Internal => DiagnosticCode::new("K1199"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEvaluationError {
    kind: BuildEvaluationErrorKind,
    message: String,
    origin: Option<SyntaxOrigin>,
}

impl BuildEvaluationError {
    pub fn new(kind: BuildEvaluationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: None,
        }
    }

    pub fn with_origin(
        kind: BuildEvaluationErrorKind,
        message: impl Into<String>,
        origin: SyntaxOrigin,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: Some(origin),
        }
    }

    pub fn kind(&self) -> BuildEvaluationErrorKind {
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

pub fn forbidden_capability_message(operation: ForbiddenBuildTimeOperation) -> &'static str {
    match operation {
        ForbiddenBuildTimeOperation::ArbitraryFilesystemRead => {
            "build evaluation forbids arbitrary filesystem reads"
        }
        ForbiddenBuildTimeOperation::ArbitraryFilesystemWrite => {
            "build evaluation forbids arbitrary filesystem writes"
        }
        ForbiddenBuildTimeOperation::ArbitraryNetworkAccess => {
            "build evaluation forbids arbitrary network access"
        }
        ForbiddenBuildTimeOperation::WallClockAccess => {
            "build evaluation forbids wall-clock access"
        }
        ForbiddenBuildTimeOperation::AmbientEnvironmentAccess => {
            "build evaluation forbids ambient environment access outside declared inputs"
        }
        ForbiddenBuildTimeOperation::UncontrolledProcessExecution => {
            "build evaluation forbids uncontrolled process execution"
        }
    }
}

pub fn forbidden_capability_error(
    operation: ForbiddenBuildTimeOperation,
    origin: Option<SyntaxOrigin>,
) -> BuildEvaluationError {
    evaluation_error(
        BuildEvaluationErrorKind::Unsupported,
        forbidden_capability_message(operation),
        origin,
    )
}

pub(super) fn evaluation_invalid_input(
    message: impl Into<String>,
    origin: Option<SyntaxOrigin>,
) -> BuildEvaluationError {
    evaluation_error(BuildEvaluationErrorKind::InvalidInput, message, origin)
}

pub(super) fn evaluation_api_error(
    error: crate::BuildApiError,
    origin: Option<SyntaxOrigin>,
) -> BuildEvaluationError {
    evaluation_invalid_input(error.to_string(), origin)
}

pub(super) fn evaluation_error(
    kind: BuildEvaluationErrorKind,
    message: impl Into<String>,
    origin: Option<SyntaxOrigin>,
) -> BuildEvaluationError {
    match origin {
        Some(origin) => BuildEvaluationError::with_origin(kind, message, origin),
        None => BuildEvaluationError::new(kind, message),
    }
}

impl std::fmt::Display for BuildEvaluationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for BuildEvaluationError {}

impl ToDiagnosticLocation for BuildEvaluationError {
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

impl ToDiagnostic for BuildEvaluationError {
    fn to_diagnostic(&self) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(self.kind.diagnostic_code(), self.to_string());
        if let Some(location) = self.diagnostic_location() {
            diagnostic = diagnostic.with_primary_label(location);
        }
        diagnostic
    }
}

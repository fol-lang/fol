use fol_diagnostics::{
    Diagnostic, DiagnosticCode, DiagnosticLocation, ToDiagnostic, ToDiagnosticLocation,
};
use fol_parser::ast::SyntaxOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypecheckErrorKind {
    InvalidInput,
    Unsupported,
    IncompatibleType,
    Internal,
    ScopeResolutionFailed,
    TypeImportFailed,
    SymbolTableCorrupted,
    UnsupportedSyntax,
    Ownership,
    OwnerBorrowed,
    BorrowConflict,
    BorrowMutability,
    BorrowReturned,
}

impl TypecheckErrorKind {
    pub fn diagnostic_code(self) -> DiagnosticCode {
        match self {
            Self::InvalidInput => DiagnosticCode::new("T1001"),
            Self::Unsupported => DiagnosticCode::new("T1002"),
            Self::IncompatibleType => DiagnosticCode::new("T1003"),
            Self::Internal => DiagnosticCode::new("T1099"),
            Self::ScopeResolutionFailed => DiagnosticCode::new("T1004"),
            Self::TypeImportFailed => DiagnosticCode::new("T1005"),
            Self::SymbolTableCorrupted => DiagnosticCode::new("T1006"),
            Self::UnsupportedSyntax => DiagnosticCode::new("T1007"),
            Self::Ownership => DiagnosticCode::new("O1001"),
            Self::OwnerBorrowed => DiagnosticCode::new("O2001"),
            Self::BorrowConflict => DiagnosticCode::new("O2002"),
            Self::BorrowMutability => DiagnosticCode::new("O2003"),
            Self::BorrowReturned => DiagnosticCode::new("O2004"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypecheckError {
    kind: TypecheckErrorKind,
    message: String,
    origin: Option<SyntaxOrigin>,
    related_origins: Vec<(SyntaxOrigin, String)>,
}

impl TypecheckError {
    pub fn new(kind: TypecheckErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: None,
            related_origins: Vec::new(),
        }
    }

    pub fn with_origin(
        kind: TypecheckErrorKind,
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

    pub fn kind(&self) -> TypecheckErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn origin(&self) -> Option<&SyntaxOrigin> {
        self.origin.as_ref()
    }

    pub fn with_fallback_origin(mut self, origin: SyntaxOrigin) -> Self {
        if self.origin.is_none() {
            self.origin = Some(origin);
        }
        self
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

impl std::fmt::Display for TypecheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TypecheckError {}

impl ToDiagnosticLocation for TypecheckError {
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

impl ToDiagnostic for TypecheckError {
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
        diagnostic
    }
}

use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::DiagnosticCode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticLocation {
    pub file: Option<String>,
    pub line: usize,
    pub column: usize,
    pub length: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiagnosticLabelKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticLabel {
    pub kind: DiagnosticLabelKind,
    pub location: DiagnosticLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticSuggestion {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<DiagnosticLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub labels: Vec<DiagnosticLabel>,
    pub notes: Vec<String>,
    pub helps: Vec<String>,
    pub suggestions: Vec<DiagnosticSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct DiagnosticWire {
    severity: Severity,
    code: DiagnosticCode,
    message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    location: Option<DiagnosticLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    help: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    labels: Vec<DiagnosticLabel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    helps: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    suggestions: Vec<DiagnosticSuggestion>,
}

impl Diagnostic {
    pub fn new(
        severity: Severity,
        code: impl Into<DiagnosticCode>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
            helps: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn error(code: impl Into<DiagnosticCode>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, code, message)
    }

    pub fn warning(code: impl Into<DiagnosticCode>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, code, message)
    }

    pub fn info(code: impl Into<DiagnosticCode>, message: impl Into<String>) -> Self {
        Self::new(Severity::Info, code, message)
    }

    pub fn from_message(
        message: impl Into<String>,
        severity: Severity,
        location: Option<DiagnosticLocation>,
    ) -> Self {
        let mut diagnostic = Self::new(severity, DiagnosticCode::unknown(), message);
        if let Some(location) = location {
            diagnostic.labels.push(DiagnosticLabel {
                kind: DiagnosticLabelKind::Primary,
                location,
                message: None,
            });
        }
        diagnostic
    }

    pub fn with_primary_label(mut self, location: DiagnosticLocation) -> Self {
        self.labels.push(DiagnosticLabel {
            kind: DiagnosticLabelKind::Primary,
            location,
            message: None,
        });
        self
    }

    pub fn with_optional_primary_label(mut self, location: Option<DiagnosticLocation>) -> Self {
        if let Some(location) = location {
            self = self.with_primary_label(location);
        }
        self
    }

    pub fn with_secondary_label(
        mut self,
        location: DiagnosticLocation,
        message: impl Into<String>,
    ) -> Self {
        self.labels.push(DiagnosticLabel {
            kind: DiagnosticLabelKind::Secondary,
            location,
            message: Some(message.into()),
        });
        self
    }

    pub fn with_optional_secondary_label(
        mut self,
        location: Option<DiagnosticLocation>,
        message: impl Into<String>,
    ) -> Self {
        if let Some(location) = location {
            self = self.with_secondary_label(location, message);
        }
        self
    }

    pub fn with_primary_label_message(
        mut self,
        location: DiagnosticLocation,
        message: impl Into<String>,
    ) -> Self {
        self.labels.push(DiagnosticLabel {
            kind: DiagnosticLabelKind::Primary,
            location,
            message: Some(message.into()),
        });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_note_if(mut self, condition: bool, note: impl Into<String>) -> Self {
        if condition {
            self = self.with_note(note);
        }
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.helps.push(help.into());
        self
    }

    pub fn with_help_if(mut self, condition: bool, help: impl Into<String>) -> Self {
        if condition {
            self = self.with_help(help);
        }
        self
    }

    pub fn with_suggestion(mut self, suggestion: DiagnosticSuggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    pub fn with_suggestion_if(mut self, condition: bool, suggestion: DiagnosticSuggestion) -> Self {
        if condition {
            self = self.with_suggestion(suggestion);
        }
        self
    }

    pub fn primary_label(&self) -> Option<&DiagnosticLabel> {
        self.labels
            .iter()
            .find(|label| label.kind == DiagnosticLabelKind::Primary)
    }

    pub fn primary_location(&self) -> Option<&DiagnosticLocation> {
        self.primary_label().map(|label| &label.location)
    }

    pub fn first_help(&self) -> Option<&str> {
        self.helps.first().map(|help| help.as_str())
    }

}

impl Serialize for Diagnostic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Diagnostic", 9)?;
        state.serialize_field("severity", &self.severity)?;
        state.serialize_field("code", &self.code)?;
        state.serialize_field("message", &self.message)?;
        state.serialize_field("location", &self.primary_location())?;
        state.serialize_field("help", &self.first_help())?;
        state.serialize_field("labels", &self.labels)?;
        state.serialize_field("notes", &self.notes)?;
        state.serialize_field("helps", &self.helps)?;
        state.serialize_field("suggestions", &self.suggestions)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Diagnostic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DiagnosticWire::deserialize(deserializer)?;
        let mut diagnostic = Diagnostic::new(wire.severity, wire.code, wire.message);
        diagnostic.labels = wire.labels;
        if diagnostic.labels.is_empty() {
            if let Some(location) = wire.location {
                diagnostic.labels.push(DiagnosticLabel {
                    kind: DiagnosticLabelKind::Primary,
                    location,
                    message: None,
                });
            }
        }
        diagnostic.notes = wire.notes;
        diagnostic.helps = wire.helps;
        if diagnostic.helps.is_empty() {
            if let Some(help) = wire.help {
                diagnostic.helps.push(help);
            }
        }
        diagnostic.suggestions = wire.suggestions;
        Ok(diagnostic)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticReport {
    pub diagnostics: Vec<Diagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
}

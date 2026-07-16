use crate::ansi::Colored;
use crate::{FrontendCommandResult, FrontendError, FrontendOutputConfig, OutputMode};
use fol_diagnostics::{DiagnosticReport, OutputFormat, ToDiagnostic};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontendOutput {
    config: FrontendOutputConfig,
}

impl FrontendOutput {
    pub fn new(config: FrontendOutputConfig) -> Self {
        crate::ansi::set_enabled(matches!(config.mode, OutputMode::Human));
        Self { config }
    }

    pub fn config(&self) -> FrontendOutputConfig {
        self.config
    }

    pub fn is_machine_readable(&self) -> bool {
        matches!(self.config.mode, OutputMode::Json)
    }

    pub fn should_use_color(&self) -> bool {
        matches!(self.config.mode, OutputMode::Human)
    }

    fn styled_section(&self, title: &str) -> String {
        format!("{}", title.cyan().bold())
    }

    fn styled_label(&self, label: &str, width: usize) -> String {
        format!("{}", format!("{label:<width$}").yellow().bold())
    }

    fn styled_action(&self, action: &str) -> String {
        format!("{}", action.green().bold())
    }

    fn styled_path(&self, path: &str) -> String {
        format!("{}", path.cyan())
    }


    pub fn render_human_header(&self, title: &str) -> String {
        self.styled_section(title)
    }

    pub fn render_human_status(&self, action: &str, detail: &str) -> String {
        format!(
            "{} {}",
            self.styled_label(action, 12),
            self.styled_path(detail)
        )
    }

    pub fn render_plain_section(&self, title: &str) -> String {
        format!("{title}:")
    }

    pub fn render_plain_field(&self, label: &str, value: impl std::fmt::Display) -> String {
        format!("{label}: {value}")
    }

    pub fn render_plain_status(&self, label: &str, fields: &[(&str, String)]) -> String {
        let rendered = fields
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("{label}: {rendered}")
    }

    pub fn render_json_result(
        &self,
        result: &FrontendCommandResult,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&serde_json::json!({
            "command": result.command,
            "summary": result.summary,
            "artifacts": result
                .artifacts
                .iter()
                .map(|artifact| serde_json::json!({
                    "kind": artifact.kind.as_str(),
                    "label": artifact.label,
                    "path": artifact.path.as_ref().map(|path| path.to_string_lossy().to_string()),
                }))
                .collect::<Vec<_>>(),
        }))
    }

    /// Build a diagnostic report from a frontend error. When the error
    /// carries real diagnostics it is a wrapper (e.g. "compilation failed
    /// with N error(s)") whose own message just summarizes those
    /// diagnostics; including it would double-count. Render only the real
    /// diagnostics in that case, and fall back to the error's own diagnostic
    /// when it carries none.
    fn report_for_error(error: &FrontendError) -> DiagnosticReport {
        let mut report = DiagnosticReport::new();
        if error.diagnostics().is_empty() {
            report.add_diagnostic(Self::annotate_diagnostic(error.to_diagnostic()));
        } else {
            for d in error.diagnostics() {
                report.add_diagnostic(Self::annotate_diagnostic(d.clone()));
            }
        }
        report
    }

    /// Attach an actionable frontend hint to a compiler diagnostic when it
    /// points at a declared-but-unmaterialized dependency. The compiler
    /// crates report the missing path; the `fol pack fetch` verb is a
    /// frontend concern, so the suggestion is added at this layer.
    fn annotate_diagnostic(mut diagnostic: fol_diagnostics::Diagnostic) -> fol_diagnostics::Diagnostic {
        let looks_unfetched = diagnostic.message.contains("/.fol/pkg/")
            && diagnostic.message.contains("does not exist");
        if looks_unfetched
            && !diagnostic
                .helps
                .iter()
                .any(|help| help.contains("fol pack fetch"))
        {
            diagnostic
                .helps
                .push("run 'fol pack fetch' to materialize declared dependencies".to_string());
        }
        diagnostic
    }

    pub fn render_json_error(&self, error: &FrontendError) -> Result<String, serde_json::Error> {
        Ok(Self::report_for_error(error).output(OutputFormat::Json))
    }

    pub fn render_human_error(&self, error: &FrontendError) -> String {
        crate::pretty::render_report_pretty(&Self::report_for_error(error))
    }

    pub fn render_plain_error(&self, error: &FrontendError) -> String {
        Self::report_for_error(error).output(OutputFormat::Human)
    }

    pub fn render_command_summary(
        &self,
        result: &FrontendCommandResult,
    ) -> Result<String, serde_json::Error> {
        match self.config.mode {
            OutputMode::Human => {
                let mut lines = vec![self.render_human_header(&result.command)];
                lines.push(format!(
                    "{} {}",
                    self.styled_action("Done:"),
                    result.summary
                ));
                for artifact in &result.artifacts {
                    let detail = artifact
                        .path
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| artifact.label.clone());
                    lines.push(self.render_human_status(artifact.kind.as_str(), &detail));
                }
                Ok(lines.join("\n"))
            }
            OutputMode::Plain => {
                let mut lines = vec![self.render_plain_field("command", &result.command)];
                lines.push(self.render_plain_field("summary", &result.summary));
                for artifact in &result.artifacts {
                    let detail = artifact
                        .path
                        .as_ref()
                        .map(|path| path.to_string_lossy().to_string())
                        .unwrap_or_else(|| artifact.label.clone());
                    lines.push(self.render_plain_field(artifact.kind.as_str(), detail));
                }
                Ok(lines.join("\n"))
            }
            OutputMode::Json => self.render_json_result(result),
        }
    }

    pub fn render_error(&self, error: &FrontendError) -> Result<String, serde_json::Error> {
        match self.config.mode {
            OutputMode::Human => Ok(self.render_human_error(error)),
            OutputMode::Plain => Ok(self.render_plain_error(error)),
            OutputMode::Json => self.render_json_error(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FrontendOutput;
    use crate::{
        FrontendArtifactKind, FrontendArtifactSummary, FrontendCommandResult, FrontendError,
        FrontendErrorKind, FrontendOutputConfig, OutputMode,
    };
    use std::path::PathBuf;

    #[test]
    fn output_helper_keeps_frontend_output_config() {
        let output = FrontendOutput::new(FrontendOutputConfig::default());

        assert_eq!(output.config().mode, OutputMode::Human);
        assert!(!output.is_machine_readable());
        assert!(output.should_use_color());
    }

    #[test]
    fn error_count_omits_the_wrapper_when_real_diagnostics_exist() {
        use fol_diagnostics::{Diagnostic, Severity};

        let real = Diagnostic::new(Severity::Error, "R1003", "could not resolve name 'x'");
        let error = FrontendError::from_errors(vec![real]);
        let output = FrontendOutput::new(FrontendOutputConfig {
            mode: OutputMode::Plain,
        });
        let rendered = output.render_plain_error(&error);
        assert!(rendered.contains("found 1 error"), "rendered: {rendered}");
        assert!(
            !rendered.contains("compilation failed with"),
            "the redundant wrapper should not appear: {rendered}"
        );
    }

    #[test]
    fn unfetched_dependency_errors_suggest_fol_pack_fetch() {
        use fol_diagnostics::{Diagnostic, Severity};

        let missing = Diagnostic::new(
            Severity::Error,
            "R1001",
            "resolver pkg import target '/w/.fol/pkg/std' does not exist",
        );
        let error = FrontendError::from_errors(vec![missing]);
        let output = FrontendOutput::new(FrontendOutputConfig {
            mode: OutputMode::Plain,
        });
        let rendered = output.render_plain_error(&error);
        assert!(
            rendered.contains("fol pack fetch"),
            "unfetched dependency errors should hint at fetch: {rendered}"
        );
    }

    #[test]
    fn human_helpers_render_colored_sections_and_rows() {
        let output = FrontendOutput::new(FrontendOutputConfig::default());

        let header = output.render_human_header("Build");
        let status = output.render_human_status("binary", "target/bin/demo");

        assert!(header.contains("Build"));
        assert!(status.contains("binary"));
        assert!(status.contains("target/bin/demo"));
    }

    #[test]
    fn plain_helpers_render_stable_script_friendly_lines() {
        let output = FrontendOutput::new(FrontendOutputConfig {
            mode: OutputMode::Plain,
        });

        assert_eq!(output.render_plain_section("build"), "build:");
        assert_eq!(
            output.render_plain_field("artifact", "target/bin/demo"),
            "artifact: target/bin/demo"
        );
        assert_eq!(
            output.render_plain_status(
                "status",
                &[
                    ("kind", "binary".to_string()),
                    ("path", "target/bin/demo".to_string())
                ]
            ),
            "status: kind=binary path=target/bin/demo"
        );
    }

    #[test]
    fn json_helpers_render_structured_result_and_error_payloads() {
        let output = FrontendOutput::new(FrontendOutputConfig {
            mode: OutputMode::Json,
        });
        let result = FrontendCommandResult::new("build", "built binary").with_artifact(
            FrontendArtifactSummary::new(
                FrontendArtifactKind::Binary,
                "demo",
                Some(PathBuf::from("target/bin/demo")),
            ),
        );
        let error =
            FrontendError::new(FrontendErrorKind::CommandFailed, "boom").with_note("note one");

        let rendered_result = output.render_json_result(&result).unwrap();
        let rendered_error = output.render_json_error(&error).unwrap();

        assert!(rendered_result.contains("\"command\": \"build\""));
        assert!(rendered_result.contains("\"kind\": \"binary\""));
        assert!(rendered_error.contains("\"boom\""));
        assert!(rendered_error.contains("\"note one\""));
    }

    #[test]
    fn color_rendering_only_applies_in_human_mode() {
        let human = FrontendOutput::new(FrontendOutputConfig::default());
        let json = FrontendOutput::new(FrontendOutputConfig {
            mode: OutputMode::Json,
        });

        assert!(human.should_use_color());
        assert!(!json.should_use_color());
    }

    #[test]
    fn human_error_rendering_uses_labeled_lines() {
        let output = FrontendOutput::new(FrontendOutputConfig::default());
        let error = FrontendError::new(FrontendErrorKind::WorkspaceNotFound, "missing root")
            .with_note("run `fol work init --bin`");

        let rendered = output.render_human_error(&error);

        assert!(rendered.contains("missing root"));
        assert!(rendered.contains("run `fol work init --bin`"));
    }

    #[test]
    fn human_command_summary_renders_done_and_artifacts() {
        let output = FrontendOutput::new(FrontendOutputConfig::default());
        let result = FrontendCommandResult::new("build", "built demo").with_artifact(
            FrontendArtifactSummary::new(
                FrontendArtifactKind::Binary,
                "demo",
                Some(PathBuf::from("target/bin/demo")),
            ),
        );

        let rendered = output.render_command_summary(&result).unwrap();

        assert!(rendered.contains("Done:"));
        assert!(rendered.contains("target/bin/demo"));
        assert!(rendered.contains("build"));
    }
}

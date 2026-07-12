use crate::artifact::BuildArtifactFolModel;
use crate::api::DependencySourceKind;
use crate::dependency::DependencyBuildEvaluationMode;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildExecutionRepresentation {
    RestrictedRuntimeIr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeProgram {
    representation: BuildExecutionRepresentation,
}

impl BuildRuntimeProgram {
    pub fn new(representation: BuildExecutionRepresentation) -> Self {
        Self { representation }
    }

    pub fn representation(&self) -> BuildExecutionRepresentation {
        self.representation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeArtifactKind {
    Executable,
    StaticLibrary,
    SharedLibrary,
    Test,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeGeneratedFileKind {
    Write,
    Copy,
    ToolOutput,
    CodegenOutput,
    GeneratedDir,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeArtifact {
    pub name: String,
    pub kind: BuildRuntimeArtifactKind,
    pub root_module: String,
    pub fol_model: BuildArtifactFolModel,
    pub target: Option<String>,
    pub optimize: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeGeneratedFile {
    pub name: String,
    pub relative_path: String,
    pub kind: BuildRuntimeGeneratedFileKind,
}

impl BuildRuntimeGeneratedFile {
    pub fn new(
        name: impl Into<String>,
        relative_path: impl Into<String>,
        kind: BuildRuntimeGeneratedFileKind,
    ) -> Self {
        Self {
            name: name.into(),
            relative_path: relative_path.into(),
            kind,
        }
    }
}

impl BuildRuntimeArtifact {
    pub fn new(
        name: impl Into<String>,
        kind: BuildRuntimeArtifactKind,
        root_module: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            root_module: root_module.into(),
            fol_model: BuildArtifactFolModel::Memo,
            target: None,
            optimize: None,
        }
    }

    pub fn with_fol_model(mut self, fol_model: BuildArtifactFolModel) -> Self {
        self.fol_model = fol_model;
        self
    }

    pub fn with_target_config(
        mut self,
        target: Option<impl Into<String>>,
        optimize: Option<impl Into<String>>,
    ) -> Self {
        self.target = target.map(|value| value.into());
        self.optimize = optimize.map(|value| value.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeStepBindingKind {
    DefaultBuild,
    DefaultRun,
    DefaultTest,
    NamedRun,
    NamedStep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeStepBinding {
    pub step_name: String,
    pub kind: BuildRuntimeStepBindingKind,
    pub artifact_name: Option<String>,
}

impl BuildRuntimeStepBinding {
    pub fn new(
        step_name: impl Into<String>,
        kind: BuildRuntimeStepBindingKind,
        artifact_name: Option<impl Into<String>>,
    ) -> Self {
        Self {
            step_name: step_name.into(),
            kind,
            artifact_name: artifact_name.map(|name| name.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeDependency {
    pub alias: String,
    pub source_kind: DependencySourceKind,
    pub package: String,
    pub args: BTreeMap<String, String>,
    pub evaluation_mode: Option<DependencyBuildEvaluationMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeDependencyExportKind {
    Module,
    Artifact,
    Step,
    File,
    Dir,
    Path,
    GeneratedOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeDependencyExport {
    pub name: String,
    pub target_name: String,
    pub kind: BuildRuntimeDependencyExportKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeDependencyQueryKind {
    Module,
    Artifact,
    Step,
    File,
    Dir,
    Path,
    GeneratedOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeDependencyQuery {
    pub dependency_alias: String,
    pub query_name: String,
    pub kind: BuildRuntimeDependencyQueryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeHandleKind {
    BuildContext,
    Graph,
    Artifact,
    GeneratedFile,
    Step,
    Run,
    Install,
    Dependency,
    DependencyModule,
    DependencyArtifact,
    DependencyStep,
    DependencyGeneratedOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeHandle {
    pub kind: BuildRuntimeHandleKind,
    pub identity: String,
}

impl BuildRuntimeHandle {
    pub fn new(kind: BuildRuntimeHandleKind, identity: impl Into<String>) -> Self {
        Self {
            kind,
            identity: identity.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildRuntimeValue {
    Void,
    Bool(bool),
    Int(i64),
    String(String),
    Path(String),
    Target(String),
    Optimize(String),
    Handle(BuildRuntimeHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildRuntimeLocalId(pub u32);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildRuntimeFrame {
    locals: BTreeMap<BuildRuntimeLocalId, BuildRuntimeValue>,
}

impl BuildRuntimeFrame {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, local: BuildRuntimeLocalId, value: BuildRuntimeValue) {
        self.locals.insert(local, value);
    }

    pub fn get(&self, local: BuildRuntimeLocalId) -> Option<&BuildRuntimeValue> {
        self.locals.get(&local)
    }

    pub fn alias(&mut self, target: BuildRuntimeLocalId, source: BuildRuntimeLocalId) -> bool {
        let Some(value) = self.locals.get(&source).cloned() else {
            return false;
        };
        self.locals.insert(target, value);
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildRuntimeExpr {
    Local(BuildRuntimeLocalId),
    Value(BuildRuntimeValue),
    Record(Vec<(String, BuildRuntimeExpr)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeRecordField {
    pub name: String,
    pub value: BuildRuntimeExpr,
}

impl BuildRuntimeRecordField {
    pub fn new(name: impl Into<String>, value: BuildRuntimeExpr) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }
}

pub fn find_record_field<'a>(
    fields: &'a [BuildRuntimeRecordField],
    name: &str,
) -> Option<&'a BuildRuntimeExpr> {
    fields
        .iter()
        .find(|field| field.name == name)
        .map(|field| &field.value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeReceiverKind {
    Graph,
    Handle(BuildRuntimeHandleKind),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeMethodCall {
    pub receiver: BuildRuntimeExpr,
    pub receiver_kind: BuildRuntimeReceiverKind,
    pub method: String,
    pub arguments: Vec<BuildRuntimeExpr>,
}

impl BuildRuntimeMethodCall {
    pub fn new(
        receiver: BuildRuntimeExpr,
        receiver_kind: BuildRuntimeReceiverKind,
        method: impl Into<String>,
        arguments: Vec<BuildRuntimeExpr>,
    ) -> Self {
        Self {
            receiver,
            receiver_kind,
            method: method.into(),
            arguments,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildRuntimeDiagnosticKind {
    MissingEntry,
    UnsupportedStatement,
    UnsupportedExpression,
    UnknownMethod,
    MissingField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRuntimeDiagnostic {
    pub kind: BuildRuntimeDiagnosticKind,
    pub message: String,
}

impl BuildRuntimeDiagnostic {
    pub fn new(kind: BuildRuntimeDiagnosticKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildRuntimeStmt {
    Bind {
        local: BuildRuntimeLocalId,
        value: BuildRuntimeExpr,
    },
    Expr(BuildRuntimeExpr),
    Return(BuildRuntimeExpr),
}

#[cfg(test)]
mod tests {
    use super::{
        find_record_field, BuildExecutionRepresentation, BuildRuntimeArtifact,
        BuildRuntimeArtifactKind, BuildRuntimeDependency, BuildRuntimeDependencyQuery,
        BuildRuntimeDependencyQueryKind, BuildRuntimeDiagnostic, BuildRuntimeDiagnosticKind,
        BuildRuntimeExpr, BuildRuntimeFrame, BuildRuntimeGeneratedFile,
        BuildRuntimeGeneratedFileKind, BuildRuntimeHandle, BuildRuntimeHandleKind,
        BuildRuntimeLocalId, BuildRuntimeMethodCall, BuildRuntimeProgram, BuildRuntimeReceiverKind,
        BuildRuntimeRecordField, BuildRuntimeStepBinding, BuildRuntimeStepBindingKind,
        BuildRuntimeStmt, BuildRuntimeValue,
    };
    use crate::artifact::BuildArtifactFolModel;
    use crate::dependency::DependencyBuildEvaluationMode;
    use std::collections::BTreeMap;

    #[test]
    fn runtime_programs_record_the_chosen_execution_representation() {
        let program = BuildRuntimeProgram::new(BuildExecutionRepresentation::RestrictedRuntimeIr);

        assert_eq!(
            program.representation(),
            BuildExecutionRepresentation::RestrictedRuntimeIr
        );
    }

    #[test]
    fn runtime_artifacts_cover_executable_test_and_library_outputs() {
        let exe =
            BuildRuntimeArtifact::new("app", BuildRuntimeArtifactKind::Executable, "src/app.fol");
        let test =
            BuildRuntimeArtifact::new("app_test", BuildRuntimeArtifactKind::Test, "test/app.fol");

        assert_eq!(exe.kind, BuildRuntimeArtifactKind::Executable);
        assert_eq!(exe.root_module, "src/app.fol");
        assert_eq!(exe.fol_model, BuildArtifactFolModel::Memo);
        assert_eq!(exe.target, None);
        assert_eq!(exe.optimize, None);
        assert_eq!(test.kind, BuildRuntimeArtifactKind::Test);
        assert_eq!(test.name, "app_test");
    }

    #[test]
    fn runtime_generated_files_cover_write_copy_tool_and_codegen_outputs() {
        let write = BuildRuntimeGeneratedFile::new(
            "version",
            "gen/version.fol",
            BuildRuntimeGeneratedFileKind::Write,
        );
        let tool = BuildRuntimeGeneratedFile::new(
            "bindings",
            "gen/bindings.fol",
            BuildRuntimeGeneratedFileKind::ToolOutput,
        );

        assert_eq!(write.name, "version");
        assert_eq!(write.relative_path, "gen/version.fol");
        assert_eq!(write.kind, BuildRuntimeGeneratedFileKind::Write);
        assert_eq!(tool.kind, BuildRuntimeGeneratedFileKind::ToolOutput);
    }

    #[test]
    fn runtime_artifacts_can_carry_fol_model_target_and_optimize_metadata() {
        let artifact =
            BuildRuntimeArtifact::new("app", BuildRuntimeArtifactKind::Executable, "src/app.fol")
                .with_fol_model(BuildArtifactFolModel::Core)
                .with_target_config(Some("x86_64-linux-gnu"), Some("release-fast"));

        assert_eq!(artifact.fol_model, BuildArtifactFolModel::Core);
        assert_eq!(artifact.target.as_deref(), Some("x86_64-linux-gnu"));
        assert_eq!(artifact.optimize.as_deref(), Some("release-fast"));
    }

    #[test]
    fn runtime_step_bindings_cover_default_and_named_artifact_steps() {
        let run = BuildRuntimeStepBinding::new(
            "run",
            BuildRuntimeStepBindingKind::DefaultRun,
            Some("app"),
        );
        let named = BuildRuntimeStepBinding::new(
            "serve",
            BuildRuntimeStepBindingKind::NamedRun,
            Some("app"),
        );

        assert_eq!(run.kind, BuildRuntimeStepBindingKind::DefaultRun);
        assert_eq!(run.artifact_name.as_deref(), Some("app"));
        assert_eq!(named.step_name, "serve");
        assert_eq!(named.kind, BuildRuntimeStepBindingKind::NamedRun);
    }

    #[test]
    fn runtime_values_cover_the_initial_build_handle_and_option_surface() {
        let build = BuildRuntimeValue::Handle(BuildRuntimeHandle::new(
            BuildRuntimeHandleKind::BuildContext,
            "build",
        ));
        let graph = BuildRuntimeValue::Handle(BuildRuntimeHandle::new(
            BuildRuntimeHandleKind::Graph,
            "graph",
        ));
        let generated = BuildRuntimeValue::Handle(BuildRuntimeHandle::new(
            BuildRuntimeHandleKind::GeneratedFile,
            "gen/version.fol",
        ));
        let target = BuildRuntimeValue::Target("x86_64-linux-gnu".to_string());
        let optimize = BuildRuntimeValue::Optimize("release-safe".to_string());

        assert!(matches!(
            build,
            BuildRuntimeValue::Handle(BuildRuntimeHandle {
                kind: BuildRuntimeHandleKind::BuildContext,
                ..
            })
        ));
        assert!(matches!(
            graph,
            BuildRuntimeValue::Handle(BuildRuntimeHandle {
                kind: BuildRuntimeHandleKind::Graph,
                ..
            })
        ));
        assert_eq!(
            target,
            BuildRuntimeValue::Target("x86_64-linux-gnu".to_string())
        );
        assert_eq!(
            optimize,
            BuildRuntimeValue::Optimize("release-safe".to_string())
        );
        assert!(matches!(
            generated,
            BuildRuntimeValue::Handle(BuildRuntimeHandle {
                kind: BuildRuntimeHandleKind::GeneratedFile,
                ..
            })
        ));
    }

    #[test]
    fn runtime_handle_kinds_cover_dependency_surface_queries() {
        assert_eq!(
            BuildRuntimeHandleKind::DependencyModule,
            BuildRuntimeHandleKind::DependencyModule
        );
        assert_eq!(
            BuildRuntimeHandleKind::DependencyArtifact,
            BuildRuntimeHandleKind::DependencyArtifact
        );
        assert_eq!(
            BuildRuntimeHandleKind::DependencyStep,
            BuildRuntimeHandleKind::DependencyStep
        );
        assert_eq!(
            BuildRuntimeHandleKind::DependencyGeneratedOutput,
            BuildRuntimeHandleKind::DependencyGeneratedOutput
        );
    }

    #[test]
    fn runtime_dependency_records_capture_alias_package_and_query_kind() {
        let dependency = BuildRuntimeDependency {
            alias: "core".to_string(),
            source_kind: crate::api::DependencySourceKind::PackageStore,
            package: "org/core".to_string(),
            args: BTreeMap::from([("target".to_string(), "wasm32-freestanding".to_string())]),
            evaluation_mode: Some(DependencyBuildEvaluationMode::Lazy),
        };
        let query = BuildRuntimeDependencyQuery {
            dependency_alias: "core".to_string(),
            query_name: "bindings".to_string(),
            kind: BuildRuntimeDependencyQueryKind::Path,
        };

        assert_eq!(dependency.alias, "core");
        assert_eq!(dependency.package, "org/core");
        assert_eq!(
            dependency.args.get("target").map(String::as_str),
            Some("wasm32-freestanding")
        );
        assert_eq!(
            dependency.evaluation_mode,
            Some(DependencyBuildEvaluationMode::Lazy)
        );
        assert_eq!(query.kind, BuildRuntimeDependencyQueryKind::Path);
        assert_eq!(query.query_name, "bindings");
    }

    #[test]
    fn runtime_frames_preserve_handle_aliases_across_repeated_local_flow() {
        let handle = BuildRuntimeValue::Handle(BuildRuntimeHandle::new(
            BuildRuntimeHandleKind::Artifact,
            "app",
        ));
        let mut frame = BuildRuntimeFrame::new();

        frame.bind(BuildRuntimeLocalId(0), handle.clone());
        assert!(frame.alias(BuildRuntimeLocalId(1), BuildRuntimeLocalId(0)));

        assert_eq!(frame.get(BuildRuntimeLocalId(0)), Some(&handle));
        assert_eq!(frame.get(BuildRuntimeLocalId(1)), Some(&handle));
        assert!(!frame.alias(BuildRuntimeLocalId(2), BuildRuntimeLocalId(99)));
    }

    #[test]
    fn runtime_expressions_cover_locals_literals_and_object_style_records() {
        let expression = BuildRuntimeExpr::Record(vec![
            (
                "name".to_string(),
                BuildRuntimeExpr::Value(BuildRuntimeValue::String("demo".to_string())),
            ),
            (
                "artifact".to_string(),
                BuildRuntimeExpr::Local(BuildRuntimeLocalId(3)),
            ),
        ]);

        assert!(matches!(
            expression,
            BuildRuntimeExpr::Record(fields)
            if fields[0].0 == "name"
                && matches!(fields[1].1, BuildRuntimeExpr::Local(BuildRuntimeLocalId(3)))
        ));
    }

    #[test]
    fn runtime_statements_cover_bind_effect_and_return_flow() {
        let bind = BuildRuntimeStmt::Bind {
            local: BuildRuntimeLocalId(0),
            value: BuildRuntimeExpr::Value(BuildRuntimeValue::Target(
                "x86_64-linux-gnu".to_string(),
            )),
        };
        let effect = BuildRuntimeStmt::Expr(BuildRuntimeExpr::Local(BuildRuntimeLocalId(0)));
        let ret = BuildRuntimeStmt::Return(BuildRuntimeExpr::Value(BuildRuntimeValue::Void));

        assert!(matches!(
            bind,
            BuildRuntimeStmt::Bind {
                local: BuildRuntimeLocalId(0),
                ..
            }
        ));
        assert!(matches!(
            effect,
            BuildRuntimeStmt::Expr(BuildRuntimeExpr::Local(BuildRuntimeLocalId(0)))
        ));
        assert!(matches!(
            ret,
            BuildRuntimeStmt::Return(BuildRuntimeExpr::Value(BuildRuntimeValue::Void))
        ));
    }

    #[test]
    fn runtime_record_helpers_find_named_fields_in_object_style_configs() {
        let fields = vec![
            BuildRuntimeRecordField::new(
                "name",
                BuildRuntimeExpr::Value(BuildRuntimeValue::String("demo".to_string())),
            ),
            BuildRuntimeRecordField::new(
                "root",
                BuildRuntimeExpr::Value(BuildRuntimeValue::Path("src/main.fol".to_string())),
            ),
        ];

        assert!(matches!(
            find_record_field(&fields, "root"),
            Some(BuildRuntimeExpr::Value(BuildRuntimeValue::Path(path)))
                if path == "src/main.fol"
        ));
        assert!(find_record_field(&fields, "missing").is_none());
    }

    #[test]
    fn runtime_method_calls_capture_graph_and_handle_receivers() {
        let graph_call = BuildRuntimeMethodCall::new(
            BuildRuntimeExpr::Local(BuildRuntimeLocalId(0)),
            BuildRuntimeReceiverKind::Graph,
            "add_exe",
            vec![BuildRuntimeExpr::Record(vec![(
                "name".to_string(),
                BuildRuntimeExpr::Value(BuildRuntimeValue::String("demo".to_string())),
            )])],
        );
        let handle_call = BuildRuntimeMethodCall::new(
            BuildRuntimeExpr::Local(BuildRuntimeLocalId(1)),
            BuildRuntimeReceiverKind::Handle(BuildRuntimeHandleKind::Step),
            "depend_on",
            vec![BuildRuntimeExpr::Local(BuildRuntimeLocalId(2))],
        );

        assert!(matches!(
            graph_call.receiver_kind,
            BuildRuntimeReceiverKind::Graph
        ));
        assert_eq!(graph_call.method, "add_exe");
        assert!(matches!(
            handle_call.receiver_kind,
            BuildRuntimeReceiverKind::Handle(BuildRuntimeHandleKind::Step)
        ));
        assert_eq!(handle_call.arguments.len(), 1);
    }

    #[test]
    fn runtime_diagnostics_cover_translation_and_evaluation_failures() {
        let unsupported_statement = BuildRuntimeDiagnostic::new(
            BuildRuntimeDiagnosticKind::UnsupportedStatement,
            "build runtime does not yet lower `when` statements",
        );
        let missing_field = BuildRuntimeDiagnostic::new(
            BuildRuntimeDiagnosticKind::MissingField,
            "artifact config is missing the required `root` field",
        );

        assert_eq!(
            unsupported_statement.kind,
            BuildRuntimeDiagnosticKind::UnsupportedStatement
        );
        assert!(unsupported_statement.message.contains("when"));
        assert_eq!(missing_field.kind, BuildRuntimeDiagnosticKind::MissingField);
        assert!(missing_field.message.contains("root"));
    }
}

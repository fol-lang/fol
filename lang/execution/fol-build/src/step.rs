#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildDefaultStepKind {
    Build,
    Run,
    Test,
    Install,
    Check,
}

impl BuildDefaultStepKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Run => "run",
            Self::Test => "test",
            Self::Install => "install",
            Self::Check => "check",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildRequestedStep {
    Default(BuildDefaultStepKind),
    Named(String),
}

impl BuildRequestedStep {
    pub fn name(&self) -> &str {
        match self {
            Self::Default(kind) => kind.as_str(),
            Self::Named(name) => name.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStepDefinition {
    pub name: String,
    pub default_kind: Option<BuildDefaultStepKind>,
    pub description: Option<String>,
    pub dependencies: Vec<String>,
}

impl BuildStepDefinition {
    pub fn custom(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            default_kind: None,
            description: None,
            dependencies: Vec::new(),
        }
    }

    pub fn default(kind: BuildDefaultStepKind) -> Self {
        Self {
            name: kind.as_str().to_string(),
            default_kind: Some(kind),
            description: None,
            dependencies: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildStepPlanError {
    UnknownStep(BuildStepId),
    DependencyCycle(BuildStepId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildStepCacheBoundary {
    Step,
    ArtifactInputs,
    Options,
    Dependencies,
    ProducedOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStepCacheKey {
    pub step_name: String,
    pub boundaries: Vec<BuildStepCacheBoundary>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildStepEventKind {
    Requested,
    Executed,
    SkippedFromCache,
    SkippedByForeignRunPolicy,
    ProducedOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStepEvent {
    pub step_name: String,
    pub kind: BuildStepEventKind,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildStepReport {
    pub requested_step: String,
    pub requested_step_description: Option<String>,
    pub events: Vec<BuildStepEvent>,
    pub produced_outputs: Vec<String>,
}

impl BuildStepReport {
    pub fn summary(&self) -> String {
        let requested = self
            .events
            .iter()
            .filter(|event| event.kind == BuildStepEventKind::Requested)
            .count();
        let executed = self
            .events
            .iter()
            .filter(|event| event.kind == BuildStepEventKind::Executed)
            .count();
        let cache_skips = self
            .events
            .iter()
            .filter(|event| event.kind == BuildStepEventKind::SkippedFromCache)
            .count();
        let foreign_skips = self
            .events
            .iter()
            .filter(|event| event.kind == BuildStepEventKind::SkippedByForeignRunPolicy)
            .count();
        format!(
            "step:{} requested={} executed={} cache-skips={} foreign-skips={} outputs={} description={}",
            self.requested_step,
            requested,
            executed,
            cache_skips,
            foreign_skips,
            self.produced_outputs.len(),
            self.requested_step_description.as_deref().unwrap_or("")
        )
    }

    pub fn primary_output(&self) -> Option<&str> {
        self.produced_outputs.first().map(String::as_str)
    }

    pub fn output_details(&self) -> &[String] {
        &self.produced_outputs
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildStepExecutionRequest {
    pub requested_step: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildStepExecutionResult {
    pub requested_step: String,
}

impl BuildStepExecutionResult {
    pub fn new(requested_step: impl Into<String>) -> Self {
        Self {
            requested_step: requested_step.into(),
        }
    }
}

pub fn compute_step_cache_keys(
    graph: &BuildGraph,
    resolved_options: &crate::option::ResolvedBuildOptionSet,
    dependency_args: &std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
) -> Vec<BuildStepCacheKey> {
    graph
        .steps()
        .iter()
        .map(|step| {
            let dependencies = graph
                .step_dependencies_for(step.id)
                .filter_map(|dependency| graph.steps().get(dependency.index()))
                .map(|dependency| dependency.name.clone())
                .collect::<Vec<_>>();
            let produced_outputs = project_step_output_names(graph, step.id);
            let option_fingerprint = resolved_options
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(",");
            let dependency_fingerprint = dependency_args
                .iter()
                .map(|(alias, args)| {
                    let rendered = args
                        .iter()
                        .map(|(key, value)| format!("{key}={value}"))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{alias}[{rendered}]")
                })
                .collect::<Vec<_>>()
                .join(";");
            let artifact_input_fingerprint = graph
                .artifacts()
                .iter()
                .map(|artifact| {
                    let inputs = graph
                        .artifact_inputs_for(artifact.id)
                        .map(|input| match input {
                            crate::graph::BuildArtifactInput::Module(module_id) => graph
                                .modules()
                                .get(module_id.index())
                                .map(|module| module.name.clone())
                                .unwrap_or_default(),
                            crate::graph::BuildArtifactInput::GeneratedFile(generated_file_id) => graph
                                .generated_files()
                                .get(generated_file_id.index())
                                .map(|generated| generated.name.clone())
                                .unwrap_or_default(),
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{}[{inputs}]", artifact.name)
                })
                .collect::<Vec<_>>()
                .join(";");
            BuildStepCacheKey {
                step_name: step.name.clone(),
                boundaries: vec![
                    BuildStepCacheBoundary::Step,
                    BuildStepCacheBoundary::ArtifactInputs,
                    BuildStepCacheBoundary::Options,
                    BuildStepCacheBoundary::Dependencies,
                    BuildStepCacheBoundary::ProducedOutputs,
                ],
                fingerprint: format!(
                    "step={};kind={:?};deps=[{}];artifacts=[{}];options=[{}];dep-args=[{}];outputs=[{}]",
                    step.name,
                    step.kind,
                    dependencies.join(","),
                    artifact_input_fingerprint,
                    option_fingerprint,
                    dependency_fingerprint,
                    produced_outputs.join(","),
                ),
            }
        })
        .collect()
}

fn project_step_output_names(graph: &BuildGraph, step: BuildStepId) -> Vec<String> {
    let mut outputs = Vec::new();
    if let Some(run_config) = graph.run_config_for(step) {
        if let Some(generated_id) = run_config.capture_stdout {
            if let Some(generated) = graph.generated_files().get(generated_id.index()) {
                outputs.push(generated.name.clone());
            }
        }
    }
    outputs.extend(
        graph.step_attachments_for(step)
            .filter_map(|generated_id| graph.generated_files().get(generated_id.index()))
            .map(|generated| generated.name.clone()),
    );
    outputs.extend(
        graph.installs()
            .iter()
            .filter(|install| install.name == graph.steps()[step.index()].name)
            .filter_map(|install| match &install.target {
                Some(crate::graph::BuildInstallTarget::GeneratedFile(generated_id)) => graph
                    .installs()
                    .iter()
                    .find(|install_output| install_output.id == install.id)
                    .map(|install_output| install_output.projected_destination.clone())
                    .or_else(|| {
                        graph.generated_files()
                            .get(generated_id.index())
                            .map(|generated| generated.name.clone())
                    }),
                Some(crate::graph::BuildInstallTarget::DirectoryPath(_)) => {
                    Some(install.projected_destination.clone())
                }
                Some(crate::graph::BuildInstallTarget::Artifact(_)) => {
                    Some(install.projected_destination.clone())
                }
                None => None,
            }),
    );
    outputs.sort();
    outputs.dedup();
    outputs
}

pub fn plan_step_order(
    graph: &BuildGraph,
    requested_step: BuildStepId,
) -> Result<Vec<BuildStepId>, BuildStepPlanError> {
    if requested_step.index() >= graph.steps().len() {
        return Err(BuildStepPlanError::UnknownStep(requested_step));
    }

    let mut visiting = vec![false; graph.steps().len()];
    let mut visited = vec![false; graph.steps().len()];
    let mut order = Vec::new();
    visit_step_order(
        graph,
        requested_step,
        &mut visiting,
        &mut visited,
        &mut order,
    )?;
    Ok(order)
}

fn visit_step_order(
    graph: &BuildGraph,
    step: BuildStepId,
    visiting: &mut [bool],
    visited: &mut [bool],
    order: &mut Vec<BuildStepId>,
) -> Result<(), BuildStepPlanError> {
    let index = step.index();
    if index >= graph.steps().len() {
        return Err(BuildStepPlanError::UnknownStep(step));
    }
    if visited[index] {
        return Ok(());
    }
    if visiting[index] {
        return Err(BuildStepPlanError::DependencyCycle(step));
    }

    visiting[index] = true;
    for dependency in graph.step_dependencies_for(step) {
        visit_step_order(graph, dependency, visiting, visited, order)?;
    }
    visiting[index] = false;
    visited[index] = true;
    order.push(step);
    Ok(())
}

pub fn project_graph_steps(graph: &BuildGraph) -> Vec<BuildStepDefinition> {
    graph
        .steps()
        .iter()
        .map(|step| BuildStepDefinition {
            name: step.name.clone(),
            default_kind: match step.kind {
                crate::graph::BuildStepKind::Default => Some(BuildDefaultStepKind::Build),
                crate::graph::BuildStepKind::Install => Some(BuildDefaultStepKind::Install),
                crate::graph::BuildStepKind::Run => Some(BuildDefaultStepKind::Run),
                crate::graph::BuildStepKind::Test => Some(BuildDefaultStepKind::Test),
                crate::graph::BuildStepKind::Check => Some(BuildDefaultStepKind::Check),
                crate::graph::BuildStepKind::CustomCommand => None,
            },
            description: step.description.clone(),
            dependencies: graph
                .step_dependencies_for(step.id)
                .filter_map(|dependency| {
                    graph
                        .steps()
                        .get(dependency.index())
                        .map(|dependency| dependency.name.clone())
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        compute_step_cache_keys, plan_step_order, project_graph_steps, BuildDefaultStepKind,
        BuildRequestedStep, BuildStepCacheBoundary, BuildStepCacheKey, BuildStepDefinition,
        BuildStepEvent, BuildStepEventKind, BuildStepExecutionRequest, BuildStepExecutionResult,
        BuildStepPlanError, BuildStepReport,
    };
    use crate::graph::{BuildGeneratedFileKind, BuildGraph, BuildStepId, BuildStepKind};

    #[test]
    fn build_default_step_kind_covers_phase_six_defaults() {
        assert_eq!(BuildDefaultStepKind::Build.as_str(), "build");
        assert_eq!(BuildDefaultStepKind::Run.as_str(), "run");
        assert_eq!(BuildDefaultStepKind::Test.as_str(), "test");
        assert_eq!(BuildDefaultStepKind::Install.as_str(), "install");
        assert_eq!(BuildDefaultStepKind::Check.as_str(), "check");
    }

    #[test]
    fn requested_steps_preserve_default_and_custom_names() {
        assert_eq!(
            BuildRequestedStep::Default(BuildDefaultStepKind::Build).name(),
            "build"
        );
        assert_eq!(BuildRequestedStep::Named("docs".to_string()).name(), "docs");
    }

    #[test]
    fn build_step_definitions_cover_default_and_custom_shapes() {
        let build = BuildStepDefinition::default(BuildDefaultStepKind::Build);
        let docs = BuildStepDefinition::custom("docs");

        assert_eq!(build.name, "build");
        assert_eq!(build.default_kind, Some(BuildDefaultStepKind::Build));
        assert!(build.dependencies.is_empty());

        assert_eq!(docs.name, "docs");
        assert_eq!(docs.default_kind, None);
        assert!(docs.dependencies.is_empty());
    }

    #[test]
    fn build_step_execution_request_defaults_to_an_empty_step_name() {
        let request = BuildStepExecutionRequest::default();

        assert!(request.requested_step.is_empty());
    }

    #[test]
    fn build_step_execution_result_keeps_the_requested_step_name() {
        let result = BuildStepExecutionResult::new("build");

        assert_eq!(result.requested_step, "build");
    }

    #[test]
    fn build_step_cache_keys_keep_boundaries_and_fingerprints() {
        let key = BuildStepCacheKey {
            step_name: "build".to_string(),
            boundaries: vec![
                BuildStepCacheBoundary::Step,
                BuildStepCacheBoundary::ArtifactInputs,
                BuildStepCacheBoundary::Options,
                BuildStepCacheBoundary::ProducedOutputs,
            ],
            fingerprint: "sha256:abc123".to_string(),
        };

        assert_eq!(key.step_name, "build");
        assert_eq!(key.boundaries.len(), 4);
        assert_eq!(key.boundaries[2], BuildStepCacheBoundary::Options);
        assert_eq!(key.boundaries[3], BuildStepCacheBoundary::ProducedOutputs);
        assert_eq!(key.fingerprint, "sha256:abc123");
    }

    #[test]
    fn build_step_reports_keep_execution_events_and_outputs() {
        let report = BuildStepReport {
            requested_step: "build".to_string(),
            requested_step_description: Some("Compile the app".to_string()),
            events: vec![
                BuildStepEvent {
                    step_name: "build".to_string(),
                    kind: BuildStepEventKind::Requested,
                    detail: "requested by cli".to_string(),
                },
                BuildStepEvent {
                    step_name: "build".to_string(),
                    kind: BuildStepEventKind::Executed,
                    detail: "compiled app".to_string(),
                },
                BuildStepEvent {
                    step_name: "build".to_string(),
                    kind: BuildStepEventKind::ProducedOutput,
                    detail: "zig-out/bin/app".to_string(),
                },
            ],
            produced_outputs: vec!["zig-out/bin/app".to_string()],
        };

        assert_eq!(report.requested_step, "build");
        assert_eq!(report.events.len(), 3);
        assert_eq!(report.events[0].kind, BuildStepEventKind::Requested);
        assert_eq!(report.produced_outputs, vec!["zig-out/bin/app".to_string()]);
        assert_eq!(
            report.summary(),
            "step:build requested=1 executed=1 cache-skips=0 foreign-skips=0 outputs=1 description=Compile the app"
        );
        assert_eq!(report.primary_output(), Some("zig-out/bin/app"));
        assert_eq!(report.output_details(), &["zig-out/bin/app".to_string()]);
    }

    #[test]
    fn step_reports_can_distinguish_cache_and_foreign_policy_skips() {
        let report = BuildStepReport {
            requested_step: "run".to_string(),
            requested_step_description: None,
            events: vec![
                BuildStepEvent {
                    step_name: "run".to_string(),
                    kind: BuildStepEventKind::Requested,
                    detail: "requested by cli".to_string(),
                },
                BuildStepEvent {
                    step_name: "build".to_string(),
                    kind: BuildStepEventKind::SkippedFromCache,
                    detail: "fingerprint match".to_string(),
                },
                BuildStepEvent {
                    step_name: "run".to_string(),
                    kind: BuildStepEventKind::SkippedByForeignRunPolicy,
                    detail: "non-std target".to_string(),
                },
            ],
            produced_outputs: Vec::new(),
        };

        assert_eq!(
            report.summary(),
            "step:run requested=1 executed=0 cache-skips=1 foreign-skips=1 outputs=0 description="
        );
    }

    #[test]
    fn step_order_planning_runs_dependencies_before_requested_step() {
        let mut graph = BuildGraph::new();
        let build = graph.add_step(BuildStepKind::Default, "build", None);
        let generate = graph.add_step(BuildStepKind::CustomCommand, "generate", None);
        let check = graph.add_step(BuildStepKind::Check, "check", None);
        graph.add_step_dependency(build, generate);
        graph.add_step_dependency(build, check);

        let order = plan_step_order(&graph, build).expect("build order should plan");

        assert_eq!(order, vec![generate, check, build]);
    }

    #[test]
    fn step_order_planning_reports_unknown_requested_steps() {
        let graph = BuildGraph::new();

        let error = plan_step_order(&graph, BuildStepId::from_index(0)).unwrap_err();

        assert_eq!(
            error,
            BuildStepPlanError::UnknownStep(BuildStepId::from_index(0))
        );
    }

    #[test]
    fn step_order_planning_reports_dependency_cycles() {
        let mut graph = BuildGraph::new();
        let build = graph.add_step(BuildStepKind::Default, "build", None);
        let check = graph.add_step(BuildStepKind::Check, "check", None);
        graph.add_step_dependency(build, check);
        graph.add_step_dependency(check, build);

        let error = plan_step_order(&graph, build).unwrap_err();

        assert_eq!(error, BuildStepPlanError::DependencyCycle(build));
    }

    #[test]
    fn graph_step_projection_keeps_default_and_custom_step_shapes() {
        let mut graph = BuildGraph::new();
        let build = graph.add_step(BuildStepKind::Default, "build", None);
        let docs = graph.add_step(
            BuildStepKind::CustomCommand,
            "docs",
            Some("Generate documentation".to_string()),
        );
        graph.add_step_dependency(docs, build);

        let projected = project_graph_steps(&graph);

        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].name, "build");
        assert_eq!(projected[0].default_kind, Some(BuildDefaultStepKind::Build));
        assert_eq!(projected[1].name, "docs");
        assert_eq!(projected[1].default_kind, None);
        assert_eq!(
            projected[1].description.as_deref(),
            Some("Generate documentation")
        );
        assert_eq!(projected[1].dependencies, vec!["build".to_string()]);
    }

    #[test]
    fn computed_step_cache_keys_include_outputs_options_and_dependency_args() {
        let mut graph = BuildGraph::new();
        let build = graph.add_step(BuildStepKind::Default, "build", None);
        let run = graph.add_step(BuildStepKind::Run, "run", None);
        graph.add_step_dependency(run, build);
        let generated = graph.add_generated_file(BuildGeneratedFileKind::CaptureOutput, "run-stdout");
        graph.run_config_mut(run).capture_stdout = Some(generated);
        let mut options = crate::ResolvedBuildOptionSet::new();
        options.insert("target", "wasm32-freestanding");
        options.insert("optimize", "release-safe");
        let dependency_args = std::collections::BTreeMap::from([(
            "json".to_string(),
            std::collections::BTreeMap::from([
                ("target".to_string(), "wasm32-freestanding".to_string()),
                ("flavor".to_string(), "strict".to_string()),
            ]),
        )]);

        let keys = compute_step_cache_keys(&graph, &options, &dependency_args);

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| key.boundaries.contains(&BuildStepCacheBoundary::ProducedOutputs)));
        let run_key = keys
            .iter()
            .find(|key| key.step_name == "run")
            .expect("run key should exist");
        assert!(run_key.fingerprint.contains("outputs=[run-stdout]"));
        assert!(run_key.fingerprint.contains("options=[optimize=release-safe,target=wasm32-freestanding]"));
        assert!(run_key
            .fingerprint
            .contains("dep-args=[json[flavor=strict,target=wasm32-freestanding]]"));
    }
}
use crate::graph::{BuildGraph, BuildStepId};

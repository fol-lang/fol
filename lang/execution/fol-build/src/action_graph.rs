//! The action graph and the checks that run before anything executes.
//!
//! Every validation here exists because the failure it prevents is silent or
//! destructive: two actions writing one path race and the winner is arbitrary,
//! two installs to one destination lose a file, and a path that escapes its
//! root writes outside the build tree entirely. None of these were checked
//! before M2; `BuildGraphValidationErrorKind` covered step cycles, missing
//! artifact inputs, and invalid install targets, and nothing else.

use crate::action::{BuildAction, BuildActionId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionValidationErrorKind {
    /// Actions form a cycle, so no execution order exists.
    DependencyCycle,
    /// An input names a producer that is not in the graph.
    UnknownProducer,
    /// An input has no producer and is not an existing source file.
    MissingProducer,
    /// Two actions declare the same output path.
    DuplicateOutput,
    /// Two installs target the same destination.
    DuplicateInstallDestination,
    /// A path leaves its allowed root.
    PathEscapesRoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionValidationError {
    pub kind: ActionValidationErrorKind,
    pub message: String,
}

impl ActionValidationError {
    fn new(kind: ActionValidationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ActionValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ActionValidationError {}

/// A set of actions plus the roots their paths must stay inside.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildActionGraph {
    actions: Vec<BuildAction>,
    /// Paths that already exist as sources and therefore need no producer.
    known_sources: BTreeSet<String>,
}

impl BuildActionGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_action(
        &mut self,
        name: impl Into<String>,
        payload: crate::action::BuildActionPayload,
    ) -> BuildActionId {
        let id = BuildActionId(self.actions.len());
        self.actions.push(BuildAction {
            id,
            name: name.into(),
            payload,
            inputs: Vec::new(),
            outputs: Vec::new(),
            depends_on: Vec::new(),
            target: None,
        });
        id
    }

    pub fn action_mut(&mut self, id: BuildActionId) -> Option<&mut BuildAction> {
        self.actions.get_mut(id.0)
    }

    pub fn action(&self, id: BuildActionId) -> Option<&BuildAction> {
        self.actions.get(id.0)
    }

    pub fn actions(&self) -> &[BuildAction] {
        &self.actions
    }

    /// Declare a path that exists before the build runs.
    pub fn declare_source(&mut self, path: impl Into<String>) {
        self.known_sources.insert(path.into());
    }

    /// Every check that must pass before execution.
    pub fn validate(&self, roots: &[&str]) -> Vec<ActionValidationError> {
        let mut errors = Vec::new();
        errors.extend(self.validate_producers());
        errors.extend(self.validate_unique_outputs());
        errors.extend(self.validate_install_destinations());
        errors.extend(self.validate_paths_stay_in_roots(roots));
        errors.extend(self.validate_acyclic());
        errors
    }

    fn validate_producers(&self) -> Vec<ActionValidationError> {
        let produced: BTreeSet<&str> = self
            .actions
            .iter()
            .flat_map(|action| action.outputs.iter())
            .map(|output| output.path.as_str())
            .collect();

        let mut errors = Vec::new();
        for action in &self.actions {
            for input in &action.inputs {
                match input.produced_by {
                    Some(producer) if self.action(producer).is_none() => {
                        errors.push(ActionValidationError::new(
                            ActionValidationErrorKind::UnknownProducer,
                            format!(
                                "action '{}' names producer {producer} for '{}', which is not in the graph",
                                action.name, input.path
                            ),
                        ));
                    }
                    Some(_) => {}
                    None => {
                        if !self.known_sources.contains(&input.path)
                            && !produced.contains(input.path.as_str())
                        {
                            errors.push(ActionValidationError::new(
                                ActionValidationErrorKind::MissingProducer,
                                format!(
                                    "action '{}' reads '{}', which no action produces and which is not a declared source",
                                    action.name, input.path
                                ),
                            ));
                        }
                    }
                }
            }
        }
        errors
    }

    fn validate_unique_outputs(&self) -> Vec<ActionValidationError> {
        let mut owner: BTreeMap<&str, &str> = BTreeMap::new();
        let mut errors = Vec::new();
        for action in &self.actions {
            for output in &action.outputs {
                if let Some(first) = owner.insert(output.path.as_str(), action.name.as_str()) {
                    errors.push(ActionValidationError::new(
                        ActionValidationErrorKind::DuplicateOutput,
                        format!(
                            "actions '{first}' and '{}' both produce '{}'; which one wins would \
                             depend on execution order",
                            action.name, output.path
                        ),
                    ));
                }
            }
        }
        errors
    }

    fn validate_install_destinations(&self) -> Vec<ActionValidationError> {
        let mut owner: BTreeMap<&str, &str> = BTreeMap::new();
        let mut errors = Vec::new();
        for action in &self.actions {
            if let crate::action::BuildActionPayload::Install { destination, .. } = &action.payload
            {
                if let Some(first) = owner.insert(destination.as_str(), action.name.as_str()) {
                    errors.push(ActionValidationError::new(
                        ActionValidationErrorKind::DuplicateInstallDestination,
                        format!(
                            "installs '{first}' and '{}' both target '{destination}'; one would \
                             silently overwrite the other",
                            action.name
                        ),
                    ));
                }
            }
        }
        errors
    }

    fn validate_paths_stay_in_roots(&self, roots: &[&str]) -> Vec<ActionValidationError> {
        let mut errors = Vec::new();
        for action in &self.actions {
            let paths = action
                .inputs
                .iter()
                .map(|input| input.path.as_str())
                .chain(action.outputs.iter().map(|output| output.path.as_str()));
            for path in paths {
                if let Err(reason) = canonical_relative_path(path) {
                    errors.push(ActionValidationError::new(
                        ActionValidationErrorKind::PathEscapesRoot,
                        format!("action '{}' uses '{path}': {reason}", action.name),
                    ));
                    continue;
                }
                if !roots.is_empty() && !roots.iter().any(|root| path_is_within(root, path)) {
                    errors.push(ActionValidationError::new(
                        ActionValidationErrorKind::PathEscapesRoot,
                        format!(
                            "action '{}' uses '{path}', which is outside every allowed root ({})",
                            action.name,
                            roots.join(", ")
                        ),
                    ));
                }
            }
        }
        errors
    }

    fn validate_acyclic(&self) -> Vec<ActionValidationError> {
        match self.execution_order() {
            Ok(_) => Vec::new(),
            Err(cycle) => vec![ActionValidationError::new(
                ActionValidationErrorKind::DependencyCycle,
                format!("actions form a dependency cycle: {cycle}"),
            )],
        }
    }

    /// A deterministic execution order, or the names on a cycle.
    ///
    /// Ties break by action id rather than by discovery order, so two runs of
    /// the same graph execute in the same sequence -- a build that reorders
    /// itself cannot be reproducible.
    pub fn execution_order(&self) -> Result<Vec<BuildActionId>, String> {
        let mut remaining: BTreeMap<usize, Vec<BuildActionId>> = self
            .actions
            .iter()
            .map(|action| (action.id.0, action.prerequisites()))
            .collect();
        let mut done: BTreeSet<usize> = BTreeSet::new();
        let mut order = Vec::with_capacity(self.actions.len());

        while !remaining.is_empty() {
            let ready: Vec<usize> = remaining
                .iter()
                .filter(|(_, prerequisites)| prerequisites.iter().all(|id| done.contains(&id.0)))
                .map(|(index, _)| *index)
                .collect();

            if ready.is_empty() {
                let stuck: Vec<&str> = remaining
                    .keys()
                    .filter_map(|index| self.actions.get(*index))
                    .map(|action| action.name.as_str())
                    .collect();
                return Err(stuck.join(" -> "));
            }
            for index in ready {
                remaining.remove(&index);
                done.insert(index);
                order.push(BuildActionId(index));
            }
        }
        Ok(order)
    }

    /// The actions a requested step needs, and nothing else.
    ///
    /// Executing the whole graph for a step that needs three actions is both
    /// slow and wrong: it runs work the user did not ask for and can fail on an
    /// unrelated action.
    pub fn closure_for(&self, requested: &[BuildActionId]) -> Vec<BuildActionId> {
        let mut needed: BTreeSet<usize> = BTreeSet::new();
        let mut pending: Vec<BuildActionId> = requested.to_vec();
        while let Some(id) = pending.pop() {
            if !needed.insert(id.0) {
                continue;
            }
            if let Some(action) = self.action(id) {
                pending.extend(action.prerequisites());
            }
        }
        self.execution_order()
            .unwrap_or_default()
            .into_iter()
            .filter(|id| needed.contains(&id.0))
            .collect()
    }
}

/// Reject a path that is absolute, empty, or climbs out of its root.
///
/// Checked on the string rather than the filesystem, because the path may not
/// exist yet -- an output is validated before it is produced.
pub fn canonical_relative_path(path: &str) -> Result<String, String> {
    if path.is_empty() {
        return Err("the path is empty".to_string());
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err("absolute paths are not allowed in a build action".to_string());
    }
    // A Windows drive letter is absolute without a leading separator.
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        return Err("drive-qualified paths are not allowed in a build action".to_string());
    }

    let mut parts: Vec<&str> = Vec::new();
    for part in path.split(['/', '\\']) {
        match part {
            "" | "." => continue,
            ".." => {
                if parts.pop().is_none() {
                    return Err("the path climbs above its root with '..'".to_string());
                }
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return Err("the path resolves to its own root".to_string());
    }
    Ok(parts.join("/"))
}

/// Whether `path` stays inside `root` once both are normalized.
pub fn path_is_within(root: &str, path: &str) -> bool {
    let (Ok(root), Ok(path)) = (canonical_relative_path(root), canonical_relative_path(path))
    else {
        return false;
    };
    path == root || path.starts_with(&format!("{root}/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{BuildActionInput, BuildActionOutput, BuildActionPayload};
    use crate::plan::OutputRole;

    fn write_action(graph: &mut BuildActionGraph, name: &str, path: &str) -> BuildActionId {
        let id = graph.add_action(
            name,
            BuildActionPayload::Write {
                contents: format!("contents of {path}"),
            },
        );
        graph
            .action_mut(id)
            .unwrap()
            .outputs
            .push(BuildActionOutput {
                path: path.to_string(),
                role: OutputRole::Object,
            });
        id
    }

    fn reads(
        graph: &mut BuildActionGraph,
        action: BuildActionId,
        path: &str,
        producer: Option<BuildActionId>,
    ) {
        graph
            .action_mut(action)
            .unwrap()
            .inputs
            .push(BuildActionInput {
                path: path.to_string(),
                produced_by: producer,
            });
    }

    #[test]
    fn a_valid_graph_reports_nothing() {
        let mut graph = BuildActionGraph::new();
        let first = write_action(&mut graph, "gen-a", "build/a.fol");
        let second = write_action(&mut graph, "gen-b", "build/b.fol");
        reads(&mut graph, second, "build/a.fol", Some(first));

        assert_eq!(graph.validate(&["build"]), Vec::new());
        assert_eq!(graph.execution_order().unwrap(), vec![first, second]);
    }

    /// Two actions writing one path race, and which one wins depends on
    /// execution order.
    #[test]
    fn two_actions_may_not_produce_the_same_path() {
        let mut graph = BuildActionGraph::new();
        write_action(&mut graph, "gen-a", "build/same.fol");
        write_action(&mut graph, "gen-b", "build/same.fol");

        let errors = graph.validate(&["build"]);
        assert!(errors
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::DuplicateOutput));
    }

    /// Two installs to one destination lose a file silently.
    #[test]
    fn two_installs_may_not_share_a_destination() {
        let mut graph = BuildActionGraph::new();
        for name in ["install-a", "install-b"] {
            graph.add_action(
                name,
                BuildActionPayload::Install {
                    source_role: OutputRole::Executable,
                    destination: "bin/app".to_string(),
                },
            );
        }

        let errors = graph.validate(&[]);
        assert!(errors
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::DuplicateInstallDestination));
    }

    #[test]
    fn an_input_with_no_producer_and_no_source_is_an_error() {
        let mut graph = BuildActionGraph::new();
        let action = write_action(&mut graph, "gen", "build/out.fol");
        reads(&mut graph, action, "build/missing.fol", None);

        let errors = graph.validate(&["build"]);
        assert!(errors
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::MissingProducer));

        // Declaring it as a source resolves it: the file exists before the
        // build starts.
        graph.declare_source("build/missing.fol");
        assert!(!graph
            .validate(&["build"])
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::MissingProducer));
    }

    #[test]
    fn a_producer_outside_the_graph_is_an_error() {
        let mut graph = BuildActionGraph::new();
        let action = write_action(&mut graph, "gen", "build/out.fol");
        reads(
            &mut graph,
            action,
            "build/other.fol",
            Some(BuildActionId(99)),
        );

        assert!(graph
            .validate(&["build"])
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::UnknownProducer));
    }

    #[test]
    fn a_cycle_has_no_execution_order() {
        let mut graph = BuildActionGraph::new();
        let first = write_action(&mut graph, "gen-a", "build/a.fol");
        let second = write_action(&mut graph, "gen-b", "build/b.fol");
        graph.action_mut(first).unwrap().depends_on.push(second);
        graph.action_mut(second).unwrap().depends_on.push(first);

        assert!(graph.execution_order().is_err());
        assert!(graph
            .validate(&["build"])
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::DependencyCycle));
    }

    #[test]
    fn traversal_and_absolute_paths_are_rejected() {
        for path in [
            "../outside.fol",
            "build/../../escape.fol",
            "/etc/passwd",
            "C:/windows/system32",
            "",
            ".",
        ] {
            assert!(
                canonical_relative_path(path).is_err(),
                "'{path}' should be rejected"
            );
        }

        // A `..` that stays inside is fine: it is normalized, not banned.
        assert_eq!(
            canonical_relative_path("build/gen/../out.fol").unwrap(),
            "build/out.fol"
        );
        assert_eq!(
            canonical_relative_path("./build/out.fol").unwrap(),
            "build/out.fol"
        );
    }

    #[test]
    fn an_action_path_outside_every_root_is_rejected() {
        let mut graph = BuildActionGraph::new();
        write_action(&mut graph, "gen", "somewhere-else/out.fol");

        let errors = graph.validate(&["build"]);
        assert!(errors
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::PathEscapesRoot));

        // The same action is fine when its root is allowed.
        assert!(!graph
            .validate(&["build", "somewhere-else"])
            .iter()
            .any(|error| error.kind == ActionValidationErrorKind::PathEscapesRoot));
    }

    #[test]
    fn containment_is_by_path_component_not_by_prefix() {
        assert!(path_is_within("build", "build/out.fol"));
        assert!(path_is_within("build", "build"));
        // `build-other` starts with `build` as a string and is a different
        // directory.
        assert!(!path_is_within("build", "build-other/out.fol"));
    }

    /// A step runs the actions it needs and no others.
    #[test]
    fn the_closure_covers_prerequisites_and_stops_there() {
        let mut graph = BuildActionGraph::new();
        let base = write_action(&mut graph, "gen-base", "build/base.fol");
        let middle = write_action(&mut graph, "gen-middle", "build/middle.fol");
        reads(&mut graph, middle, "build/base.fol", Some(base));
        let unrelated = write_action(&mut graph, "gen-unrelated", "build/unrelated.fol");

        let closure = graph.closure_for(&[middle]);
        assert_eq!(closure, vec![base, middle]);
        assert!(!closure.contains(&unrelated));
    }

    /// Execution order must be identical across runs, or a build cannot be
    /// reproducible.
    #[test]
    fn execution_order_is_deterministic_across_runs() {
        let build = || {
            let mut graph = BuildActionGraph::new();
            let a = write_action(&mut graph, "a", "build/a.fol");
            let b = write_action(&mut graph, "b", "build/b.fol");
            let c = write_action(&mut graph, "c", "build/c.fol");
            reads(&mut graph, c, "build/a.fol", Some(a));
            reads(&mut graph, c, "build/b.fol", Some(b));
            graph.execution_order().unwrap()
        };
        assert_eq!(build(), build());
    }

    #[test]
    fn cache_identity_changes_with_every_meaningful_field() {
        let mut graph = BuildActionGraph::new();
        let action = write_action(&mut graph, "gen", "build/out.fol");
        let baseline = graph.action(action).unwrap().cache_identity();

        graph.action_mut(action).unwrap().payload = BuildActionPayload::Write {
            contents: "different".to_string(),
        };
        assert_ne!(graph.action(action).unwrap().cache_identity(), baseline);

        let mut targeted = graph.clone();
        targeted.action_mut(action).unwrap().target =
            Some(fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-musl").unwrap());
        assert_ne!(
            targeted.action(action).unwrap().cache_identity(),
            graph.action(action).unwrap().cache_identity()
        );
    }

    /// An environment value can be a token, and a cache key travels with build
    /// metadata.
    #[test]
    fn cache_identity_never_contains_a_raw_environment_value() {
        let mut graph = BuildActionGraph::new();
        let action = graph.add_action(
            "tool",
            BuildActionPayload::SystemTool {
                tool: "/usr/bin/generator".to_string(),
                args: Vec::new(),
                env: vec![("TOKEN".to_string(), "ghp_secretvalue".to_string())],
                capture_stdout: None,
            },
        );
        let identity = graph.action(action).unwrap().cache_identity();
        assert!(!identity.contains("ghp_secretvalue"));

        // Two different secrets still give different identities.
        graph.action_mut(action).unwrap().payload = BuildActionPayload::SystemTool {
            tool: "/usr/bin/generator".to_string(),
            args: Vec::new(),
            env: vec![("TOKEN".to_string(), "ghp_othervalue".to_string())],
            capture_stdout: None,
        };
        assert_ne!(graph.action(action).unwrap().cache_identity(), identity);
    }
}

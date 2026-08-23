//! Typed build actions.
//!
//! Before this the graph recorded *declarations* -- a generated file, an
//! install, a system tool -- and the backend did the corresponding work through
//! its own side channel. Nothing tied the two together: a declaration named no
//! inputs, produced no identified output, and could not be ordered against
//! another declaration, so the graph could neither cache, report, nor validate
//! anything it had declared.
//!
//! An action is the declaration plus the four facts that make it executable:
//! what it reads, what it produces and in what role, what must happen first,
//! and which target it belongs to. Section 4.3 of `plan/V4_PLAN.md` calls the
//! result an operational action graph.

use crate::plan::OutputRole;

crate::define_graph_id!(BuildActionId, "action:");

/// What an action does. One variant per operation the build language can
/// declare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildActionPayload {
    /// Write literal contents to a file.
    Write { contents: String },
    /// Copy one file to another location.
    Copy { source: String },
    /// Run a tool found on the system, not one a dependency provided.
    SystemTool {
        /// Absolute or resolved path to the tool.
        tool: String,
        args: Vec<String>,
        /// Environment overrides. Values are fingerprinted, never printed.
        env: Vec<(String, String)>,
        /// Where the tool's stdout is captured, when it is.
        capture_stdout: Option<String>,
    },
    /// Run a code generator that produces FOL source.
    Codegen {
        generator: String,
        args: Vec<String>,
    },
    /// Compile one artifact.
    Compile { artifact: String },
    /// Place a produced output at its install destination.
    Install {
        source_role: OutputRole,
        destination: String,
    },
    /// Execute a produced binary.
    Run { artifact: String, args: Vec<String> },
}

impl BuildActionPayload {
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::Write { .. } => "write",
            Self::Copy { .. } => "copy",
            Self::SystemTool { .. } => "system-tool",
            Self::Codegen { .. } => "codegen",
            Self::Compile { .. } => "compile",
            Self::Install { .. } => "install",
            Self::Run { .. } => "run",
        }
    }

    /// Whether the action launches an external process.
    ///
    /// Used by the materializer to decide what needs a tool fingerprint, and by
    /// the trust policy, which refuses dependency-provided executables.
    pub const fn launches_a_process(&self) -> bool {
        matches!(
            self,
            Self::SystemTool { .. } | Self::Codegen { .. } | Self::Run { .. }
        )
    }
}

/// A file an action reads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildActionInput {
    /// Package- or build-relative. Canonicalized before execution.
    pub path: String,
    /// The action that produces this path, when one does. `None` means the
    /// input is expected to exist already, which validation checks.
    pub produced_by: Option<BuildActionId>,
}

/// A file an action produces, and what it is for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildActionOutput {
    pub path: String,
    pub role: OutputRole,
}

/// One executable node of the build graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildAction {
    pub id: BuildActionId,
    /// Stable, human-meaningful name used in diagnostics and reports.
    pub name: String,
    pub payload: BuildActionPayload,
    pub inputs: Vec<BuildActionInput>,
    pub outputs: Vec<BuildActionOutput>,
    /// Actions that must complete first, beyond those implied by inputs.
    pub depends_on: Vec<BuildActionId>,
    /// The target this action belongs to. `None` for target-independent work
    /// such as writing a config file.
    pub target: Option<fol_types::ResolvedTarget>,
}

impl BuildAction {
    /// Every action that must run before this one: explicit dependencies plus
    /// the producers of its inputs.
    pub fn prerequisites(&self) -> Vec<BuildActionId> {
        let mut prerequisites = self.depends_on.clone();
        for input in &self.inputs {
            if let Some(producer) = input.produced_by {
                if !prerequisites.contains(&producer) {
                    prerequisites.push(producer);
                }
            }
        }
        prerequisites
    }

    /// The cache identity of this action.
    ///
    /// Two actions with the same identity produce the same outputs from the
    /// same inputs, so one may be skipped when the other has run. Environment
    /// values are hashed rather than rendered, because an action's environment
    /// can hold a token and a cache key travels with build metadata.
    pub fn cache_identity(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str(self.payload.kind_name());
        rendered.push('\n');
        match &self.payload {
            BuildActionPayload::Write { contents } => {
                // The contents decide the output, so they are the identity --
                // hashed, because they can be large.
                rendered.push_str(&digest(contents.as_bytes()));
            }
            BuildActionPayload::Copy { source } => rendered.push_str(source),
            BuildActionPayload::SystemTool {
                tool,
                args,
                env,
                capture_stdout,
            } => {
                rendered.push_str(tool);
                rendered.push('\n');
                rendered.push_str(&args.join("\u{1f}"));
                rendered.push('\n');
                for (name, value) in env {
                    rendered.push_str(name);
                    rendered.push('=');
                    rendered.push_str(&digest(value.as_bytes()));
                    rendered.push('\n');
                }
                if let Some(path) = capture_stdout {
                    rendered.push_str(path);
                }
            }
            BuildActionPayload::Codegen { generator, args } => {
                rendered.push_str(generator);
                rendered.push('\n');
                rendered.push_str(&args.join("\u{1f}"));
            }
            BuildActionPayload::Compile { artifact } => rendered.push_str(artifact),
            BuildActionPayload::Install {
                source_role,
                destination,
            } => {
                rendered.push_str(source_role.as_str());
                rendered.push('\n');
                rendered.push_str(destination);
            }
            BuildActionPayload::Run { artifact, args } => {
                rendered.push_str(artifact);
                rendered.push('\n');
                rendered.push_str(&args.join("\u{1f}"));
            }
        }
        rendered.push('\n');

        // Inputs are sorted: two actions reading the same files in a different
        // declaration order are the same action.
        let mut inputs: Vec<&str> = self
            .inputs
            .iter()
            .map(|input| input.path.as_str())
            .collect();
        inputs.sort_unstable();
        rendered.push_str(&inputs.join("\u{1f}"));
        rendered.push('\n');

        let mut outputs: Vec<String> = self
            .outputs
            .iter()
            .map(|output| format!("{}:{}", output.role.as_str(), output.path))
            .collect();
        outputs.sort();
        rendered.push_str(&outputs.join("\u{1f}"));
        rendered.push('\n');

        if let Some(target) = &self.target {
            rendered.push_str(target.rust_target_triple());
        }
        digest(rendered.as_bytes())
    }
}

/// FNV-1a, matching `plan::identity`: stable across platforms and Rust
/// versions, which `std`'s hasher is not.
fn digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

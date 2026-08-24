use crate::OutputMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendProfile {
    Debug,
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionShellArg {
    Bash,
    Zsh,
    Fish,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontendOutputArgs {
    /// Command-local override. `None` preserves the root/global output mode.
    pub output: Option<OutputMode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontendProfileArgs {
    pub profile: Option<FrontendProfile>,
    pub debug: bool,
    pub release: bool,
}

impl FrontendProfileArgs {
    /// `None` when this position carried no profile flag, so callers can fall
    /// through to the next layer instead of overwriting it with a default.
    pub fn explicit_profile(&self) -> Option<FrontendProfile> {
        if self.release {
            Some(FrontendProfile::Release)
        } else if self.debug {
            Some(FrontendProfile::Debug)
        } else {
            self.profile
        }
    }

    pub fn selected_profile(&self) -> FrontendProfile {
        self.explicit_profile().unwrap_or(FrontendProfile::Debug)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontendCommand {
    Work(WorkCommand),
    Pack(PackCommand),
    Code(CodeCommand),
    Tool(ToolCommand),
    Complete(CompleteCommand),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitCommand;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompileRootArgs {
    pub std_root: Option<String>,
    pub package_store_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirectTargetArg {
    pub input: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildOptionArgs {
    pub build_target: Option<String>,
    pub build_optimize: Option<String>,
    pub build_options: Vec<String>,
    pub define: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildStepArgs {
    pub step: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FetchCommand {
    pub output: FrontendOutputArgs,
    pub roots: CompileRootArgs,
    pub locked: bool,
    pub offline: bool,
    pub refresh: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateCommand {
    pub output: FrontendOutputArgs,
    pub roots: CompileRootArgs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildCommand {
    pub output: FrontendOutputArgs,
    pub profile: FrontendProfileArgs,
    pub target: DirectTargetArg,
    pub roots: CompileRootArgs,
    pub options: BuildOptionArgs,
    pub step: BuildStepArgs,
    pub locked: bool,
    pub keep_build_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunCommand {
    pub output: FrontendOutputArgs,
    pub profile: FrontendProfileArgs,
    pub target: DirectTargetArg,
    pub roots: CompileRootArgs,
    pub options: BuildOptionArgs,
    pub step: BuildStepArgs,
    pub locked: bool,
    pub keep_build_dir: bool,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestCommand {
    pub output: FrontendOutputArgs,
    pub profile: FrontendProfileArgs,
    pub options: BuildOptionArgs,
    pub step: BuildStepArgs,
    pub path: Option<String>,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckCommand {
    pub output: FrontendOutputArgs,
    pub profile: FrontendProfileArgs,
    pub target: DirectTargetArg,
    pub roots: CompileRootArgs,
    pub options: BuildOptionArgs,
    pub step: BuildStepArgs,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkCommand {
    pub output: FrontendOutputArgs,
    pub path: Option<String>,
    pub command: WorkSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackCommand {
    pub output: FrontendOutputArgs,
    pub command: PackSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeCommand {
    pub output: FrontendOutputArgs,
    pub profile: FrontendProfileArgs,
    pub command: CodeSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCommand {
    pub output: FrontendOutputArgs,
    pub command: ToolSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCommand {
    pub shell: CompletionShellArg,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeCommand {
    pub command: TreeSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeGenerateCommand {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteCommand {
    pub tokens: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainCommand {
    pub code: String,
    /// Output mode set explicitly on the command (e.g. `code explain T1003
    /// --output json`). When `None`, the root-level `--output`/`--json` flags
    /// apply.
    pub output: Option<OutputMode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmitCommand {
    pub command: EmitSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorPathCommand {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorReferenceCommand {
    pub path: String,
    pub line: u32,
    pub character: u32,
    pub exclude_declaration: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCompletionCommand {
    pub path: String,
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorRenameCommand {
    pub path: String,
    pub line: u32,
    pub character: u32,
    pub new_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkSubcommand {
    Init(InitCommand),
    New(NewCommand),
    Info(UnitCommand),
    List(UnitCommand),
    Deps(UnitCommand),
    Status(UnitCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackSubcommand {
    Fetch(FetchCommand),
    Update(UpdateCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeSubcommand {
    Build(BuildCommand),
    Run(RunCommand),
    Test(TestCommand),
    Check(CheckCommand),
    Emit(EmitCommand),
    Explain(ExplainCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSubcommand {
    Lsp(UnitCommand),
    Format(EditorPathCommand),
    Parse(EditorPathCommand),
    Highlight(EditorPathCommand),
    Symbols(EditorPathCommand),
    References(EditorReferenceCommand),
    Rename(EditorRenameCommand),
    Complete(EditorCompletionCommand),
    SemanticTokens(EditorPathCommand),
    Tree(TreeCommand),
    Clean(UnitCommand),
    Completion(CompletionCommand),
    Bind(BindCommand),
    Abi(AbiCommand),
}

/// `fol tool bind c` and any future language lane.
///
/// Spelled as a subcommand rather than a `--language` flag because section
/// 4.13 makes the C lane deliberately C-specific: another language would need
/// its own options, not a different value for one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindSubcommand {
    C(BindCCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindCommand {
    pub output: FrontendOutputArgs,
    pub command: BindSubcommand,
}

/// Every fact the checked C pipeline needs, stated explicitly.
///
/// Nothing here is discovered from the environment: section 4.13 disables
/// ambient target, `CPATH`, SDK, compiler, and sysroot discovery in
/// reproducible mode, and this command is the reproducible mode.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BindCCommand {
    pub alias: String,
    pub target: Option<String>,
    pub header: String,
    pub provider: String,
    pub provider_kind: String,
    pub annotations: Option<String>,
    pub out: String,
    /// The capability model the import is checked against.
    pub fol_model: Option<String>,
}

/// `fol tool abi inspect` and `fol tool abi check`.
///
/// Both read written manifests and nothing else. Neither compiles, so neither
/// takes a package root: an installed prefix or a release archive is exactly
/// where these are useful, and requiring a source tree would make them useless
/// there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiSubcommand {
    Inspect(AbiInspectCommand),
    Check(AbiCheckCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiCommand {
    pub output: FrontendOutputArgs,
    pub command: AbiSubcommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AbiInspectCommand {
    /// The manifest to read.
    pub manifest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AbiCheckCommand {
    /// The checked-in baseline.
    pub baseline: String,
    /// The manifest just produced.
    pub candidate: String,
    /// Accept a breaking change because the major version was incremented on
    /// purpose. Spelled as a flag so the acceptance is in the command that ran,
    /// not in a file someone edited.
    pub allow_breaking: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeSubcommand {
    Generate(TreeGenerateCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitSubcommand {
    Rust(EmitRustCommand),
    Lowered(EmitLoweredCommand),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmitRustCommand {
    pub output: FrontendOutputArgs,
    pub profile: FrontendProfileArgs,
    pub target: DirectTargetArg,
    pub roots: CompileRootArgs,
    pub keep_build_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmitLoweredCommand {
    pub output: FrontendOutputArgs,
    pub profile: FrontendProfileArgs,
    pub target: DirectTargetArg,
    pub roots: CompileRootArgs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InitCommand {
    pub workspace: bool,
    pub bin: bool,
    pub lib: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCommand {
    pub name: String,
    pub workspace: bool,
    pub bin: bool,
    pub lib: bool,
}

//! User-facing frontend foundations for the FOL toolchain.
//!
//! `fol-frontend` will become the canonical command-line/workspace entrypoint
//! above `fol-package` and the compiler pipeline.

pub(crate) mod ansi;
mod build_route;
mod clean;
mod cli;
mod compile;
mod completion;
mod config;
mod direct;
mod discovery;
mod dispatch;
mod editor;
mod errors;
mod explain;
mod fetch;
mod interop;
mod output;
mod pretty;
mod result;
mod scaffold;
#[cfg(test)]
mod test_env;
mod ui;
mod work;
mod workspace;

pub use build_route::{
    execute_workspace_build_route, plan_workspace_build_route, requested_workspace_step,
    FrontendBuildStep, FrontendBuildWorkflowMode, FrontendMemberBuildRoute,
    FrontendWorkspaceBuildRequest, FrontendWorkspaceBuildRoute,
};
pub use clean::{clean_workspace, clean_workspace_with_config};
pub use cli::{
    BuildCommand, CheckCommand, CodeCommand, CodeSubcommand, CompleteCommand, CompletionCommand,
    CompletionShellArg, EditorPathCommand, EditorReferenceCommand, EditorRenameCommand,
    EmitCommand, EmitLoweredCommand, EmitRustCommand, EmitSubcommand, ExplainCommand, FetchCommand,
    FrontendCli, FrontendCommand, FrontendProfile, InitCommand, NewCommand, PackCommand,
    PackSubcommand, ParseError, ParseErrorKind, RunCommand, TestCommand, ToolCommand,
    ToolSubcommand, UnitCommand, UpdateCommand,
};
pub use compile::{
    build_workspace, build_workspace_for_profile_with_config, build_workspace_with_config,
    check_workspace, check_workspace_with_config, compile_member_workspace, emit_lowered,
    emit_lowered_with_config, emit_rust, emit_rust_with_config, profile_build_root, run_workspace,
    run_workspace_with_args_and_config, run_workspace_with_config, test_package,
    test_package_with_config, test_workspace, test_workspace_with_config,
};
pub use completion::{
    completion_command, generate_bash_completion_script, generate_completion_script,
    generate_fish_completion_script, generate_zsh_completion_script, internal_complete_command,
    internal_complete_command_with_tokens, internal_complete_matches, CompletionShell,
};
pub use config::FrontendConfig;
pub use direct::{
    run_direct_compile, run_direct_compile_with_io, DirectCompileConfig, DirectCompileMode,
};
pub use discovery::{
    discover_root_from_explicit_path, discover_root_upward, require_discovered_root,
    DiscoveredRoot, PackageRoot, WorkspaceRoot, PACKAGE_FILE_NAME, WORKSPACE_FILE_NAME,
};
pub use editor::{
    editor_completion_command, editor_format_command, editor_highlight_command, editor_lsp_command,
    editor_lsp_stdio, editor_parse_command, editor_references_command, editor_rename_command,
    editor_semantic_tokens_command, editor_symbols_command, editor_tree_generate_command,
};
pub use errors::{FrontendError, FrontendErrorKind, FrontendResult};
pub use explain::{explain_command, render_explain, ExplainRendering};
pub use fetch::{
    fetch_workspace, fetch_workspace_with_config, prepare_workspace_packages,
    select_package_store_root, update_workspace, update_workspace_with_config,
    FrontendPackagePreparation, FrontendPreparedPackage,
};
pub use output::{FrontendOutputConfig, OutputMode};
pub use result::{FrontendArtifactKind, FrontendArtifactSummary, FrontendCommandResult};
pub use scaffold::{
    init_current_dir, init_package_root, init_root, init_workspace_root, new_project,
    new_project_with_mode, package_target_kind, PackageTargetKind,
};
pub use ui::FrontendOutput;
pub use work::{work_deps, work_info, work_list, work_status};
pub use workspace::{
    enumerate_member_packages, load_frontend_workspace, load_workspace_config, FrontendWorkspace,
    FrontendWorkspaceConfig,
};

use std::io::Write;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Frontend;

impl Frontend {
    pub fn new() -> Self {
        Self
    }

    pub fn run(&self) -> FrontendResult<()> {
        let args = std::env::args_os().collect::<Vec<_>>();
        let (output, result) = run_command_from_args(args)?;
        let rendered = output
            .render_command_summary(&result)
            .map_err(|error| FrontendError::new(FrontendErrorKind::Internal, error.to_string()))?;
        writeln!(std::io::stdout(), "{rendered}")
            .map_err(|error| FrontendError::new(FrontendErrorKind::Internal, error.to_string()))?;
        Ok(())
    }
}

pub const CRATE_NAME: &str = "fol-frontend";

pub fn crate_name() -> &'static str {
    CRATE_NAME
}

pub fn run() -> FrontendResult<()> {
    Frontend::new().run()
}

pub fn run_from_args<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    run_from_args_with_io(args, &mut std::io::stdout(), &mut std::io::stderr())
}

pub fn run_from_args_with_io<I, T>(
    args: I,
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    dispatch::run_from_args_with_io_inner(args, stdout, stderr)
}

pub fn run_command_from_args<I, T>(
    args: I,
) -> FrontendResult<(FrontendOutput, FrontendCommandResult)>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|a| a.into().into_string().unwrap_or_default())
        .collect();
    let cli = FrontendCli::try_parse_from(args).map_err(|error| {
        FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string())
            .with_note("run `fol --help` to inspect the available workflow commands")
    })?;
    let config = frontend_config_from_cli(&cli, None);
    let output = FrontendOutput::new(config.output);
    let result = dispatch::dispatch_cli(&cli, &config)?;
    Ok((output, result))
}

pub fn run_command_from_args_in_dir<I, T>(
    args: I,
    working_directory: impl Into<std::path::PathBuf>,
) -> FrontendResult<(FrontendOutput, FrontendCommandResult)>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|a| a.into().into_string().unwrap_or_default())
        .collect();
    let cli = FrontendCli::try_parse_from(args).map_err(|error| {
        FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string())
            .with_note("run `fol --help` to inspect the available workflow commands")
    })?;
    let config = frontend_config_from_cli(&cli, Some(working_directory.into()));
    let output = FrontendOutput::new(config.output);
    let result = dispatch::dispatch_cli(&cli, &config)?;
    Ok((output, result))
}

pub(crate) fn frontend_config_from_cli(
    cli: &FrontendCli,
    working_directory: Option<std::path::PathBuf>,
) -> FrontendConfig {
    let mut config = FrontendConfig::from_env();
    if let Some(working_directory) = working_directory {
        config.working_directory = working_directory;
    }
    config.output.mode = if cli.json {
        OutputMode::Json
    } else {
        command_output_mode(cli).unwrap_or(cli.output)
    };
    config.profile_override = Some(command_profile(cli).unwrap_or_else(|| cli.selected_profile()));
    if let Some(std_root) = &cli.std_root {
        config.std_root_override = Some(std_root.into());
    }
    if let Some(package_store_root) = &cli.package_store_root {
        config.package_store_root_override = Some(package_store_root.into());
    }
    if cli.keep_build_dir {
        config.keep_build_dir = true;
    }
    match cli.command.as_ref() {
        Some(FrontendCommand::Pack(command)) => match &command.command {
            PackSubcommand::Fetch(command) => {
                config.locked_fetch = command.locked;
                config.offline_fetch = command.offline;
                config.refresh_fetch = command.refresh;
            }
            PackSubcommand::Update(_) => {
                config.refresh_fetch = true;
            }
        },
        Some(FrontendCommand::Code(command)) => match &command.command {
            CodeSubcommand::Build(command) => {
                config.locked_fetch = command.locked;
                apply_build_step_args(&mut config, &command.step);
                apply_build_option_args(&mut config, &command.options);
            }
            CodeSubcommand::Run(command) => {
                config.locked_fetch = command.locked;
                apply_build_step_args(&mut config, &command.step);
                apply_build_option_args(&mut config, &command.options);
            }
            CodeSubcommand::Test(command) => {
                config.locked_fetch = command.locked;
                apply_build_step_args(&mut config, &command.step);
                apply_build_option_args(&mut config, &command.options);
            }
            CodeSubcommand::Check(command) => {
                config.locked_fetch = command.locked;
                apply_build_step_args(&mut config, &command.step);
                apply_build_option_args(&mut config, &command.options);
            }
            CodeSubcommand::Emit(_) => {}
            CodeSubcommand::Explain(_) => {}
        },
        _ => {}
    }
    config
}

fn apply_build_option_args(config: &mut FrontendConfig, options: &cli::BuildOptionArgs) {
    if let Some(target) = &options.build_target {
        config.build_target_override = Some(target.clone());
    }
    if let Some(optimize) = &options.build_optimize {
        config.build_optimize_override = Some(optimize.clone());
    }
    let mut overrides = options.build_options.clone();
    // Merge -D shorthand into overrides; -Dtarget=X and -Doptimize=X take precedence
    for define in &options.define {
        if let Some(value) = define.strip_prefix("target=") {
            config.build_target_override = Some(value.to_string());
        } else if let Some(value) = define.strip_prefix("optimize=") {
            config.build_optimize_override = Some(value.to_string());
        } else {
            overrides.push(define.clone());
        }
    }
    if !overrides.is_empty() {
        config.build_option_overrides = overrides;
    }
}

fn apply_build_step_args(config: &mut FrontendConfig, step: &cli::BuildStepArgs) {
    if let Some(step) = &step.step {
        config.build_step_override = Some(step.clone());
    }
}

fn command_output_mode(cli: &FrontendCli) -> Option<OutputMode> {
    match cli.command.as_ref() {
        Some(FrontendCommand::Work(command)) => command.output.output,
        Some(FrontendCommand::Pack(command)) => {
            let local = match &command.command {
                PackSubcommand::Fetch(fetch) => fetch.output.output,
                PackSubcommand::Update(update) => update.output.output,
            };
            local.or(command.output.output)
        }
        Some(FrontendCommand::Code(command)) => {
            let local = match &command.command {
                CodeSubcommand::Build(build) => build.output.output,
                CodeSubcommand::Run(run) => run.output.output,
                CodeSubcommand::Test(test) => test.output.output,
                CodeSubcommand::Check(check) => check.output.output,
                CodeSubcommand::Emit(emit) => match &emit.command {
                    cli::EmitSubcommand::Rust(rust) => rust.output.output,
                    cli::EmitSubcommand::Lowered(lowered) => lowered.output.output,
                },
                CodeSubcommand::Explain(explain) => explain.output,
            };
            local.or(command.output.output)
        }
        Some(FrontendCommand::Tool(command)) => command.output.output,
        Some(FrontendCommand::Complete(_)) | None => None,
    }
}

fn command_profile(cli: &FrontendCli) -> Option<FrontendProfile> {
    match cli.command.as_ref() {
        Some(FrontendCommand::Code(command)) => Some(command.profile.selected_profile()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::{FrontendOutputArgs, FrontendProfileArgs};
    use crate::test_env::{EnvironmentGuard, FRONTEND_ENV_KEYS};

    fn semantic_dispatch_build() -> String {
        [
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({ name = \"app\", root = \"src/main.fol\" });\n",
            "    graph.install(app);\n",
            "    graph.add_run(app);\n",
            "    graph.add_test({ name = \"app_test\", root = \"src/main.fol\" });\n",
            "    return;\n",
            "};\n",
        ]
        .concat()
    }

    fn absorbed_build_dispatch_fixture_with_source(
        label: &str,
        build_source: &str,
    ) -> FrontendWorkspace {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_dispatch_route_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(root.join("build.fol"), build_source).unwrap();
        std::fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0\n};\n",
        )
        .unwrap();

        FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(root.clone())],
            std_root_override: None,
            package_store_root_override: None,
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        }
    }

    fn absorbed_build_dispatch_fixture(label: &str) -> FrontendWorkspace {
        absorbed_build_dispatch_fixture_with_source(label, &semantic_dispatch_build())
    }

    fn modern_dispatch_fixture(label: &str) -> FrontendWorkspace {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_dispatch_modern_{label}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before epoch")
                .as_nanos()
        ));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
        std::fs::write(root.join("build.fol"), semantic_dispatch_build()).unwrap();
        std::fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0\n};\n",
        )
        .unwrap();

        FrontendWorkspace {
            root: WorkspaceRoot::new(root.clone()),
            members: vec![PackageRoot::new(root.clone())],
            std_root_override: None,
            package_store_root_override: None,
            build_root: root.join(".fol/build"),
            cache_root: root.join(".fol/cache"),
            git_cache_root: root.join(".fol/cache/git"),
            install_prefix: root.join(".fol/install"),
        }
    }

    #[test]
    fn crate_name_matches_frontend_identity() {
        assert_eq!(crate_name(), "fol-frontend");
    }

    #[test]
    fn public_run_shell_is_callable() {
        let frontend = Frontend::new();
        let _ = frontend;
        let _run_ptr: fn() -> FrontendResult<()> = run;
    }

    #[test]
    fn graph_driven_build_route_surface_is_reexported_at_crate_root() {
        let workspace = absorbed_build_dispatch_fixture("public_build_route_surface");
        let requested_step =
            requested_workspace_step(&CodeSubcommand::Build(BuildCommand::default()), None);
        assert_eq!(requested_step, "build");

        let route = plan_workspace_build_route(&workspace, requested_step.clone())
            .expect("crate root route re-export should plan");
        assert_eq!(route.requested_step, requested_step);

        let result = execute_workspace_build_route(
            &workspace,
            &FrontendConfig::default(),
            &FrontendWorkspaceBuildRequest {
                requested_step,
                profile: FrontendProfile::Debug,
                run_args: Vec::new(),
            },
        )
        .expect("crate root route re-export should execute");
        assert_eq!(result.command, "build");

        std::fs::remove_dir_all(workspace.root.root).ok();
    }

    #[test]
    fn run_command_from_args_dispatches_buildable_frontend_commands() {
        let _env = EnvironmentGuard::removed(FRONTEND_ENV_KEYS);
        let root =
            std::env::temp_dir().join(format!("fol_frontend_dispatch_{}", std::process::id()));
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(root.join("build.fol"), "name: demo\nversion: 0.1.0\n").unwrap();
        std::fs::write(root.join("build.fol"), semantic_dispatch_build()).unwrap();
        std::fs::write(
            src.join("main.fol"),
            "fun[] main(): int = {\n    return 0\n};\n",
        )
        .unwrap();

        let (_, result) = run_command_from_args_in_dir(["fol", "code", "check"], &root).unwrap();

        assert_eq!(result.command, "check");
        assert!(result.summary.contains("checked 1 workspace package(s)"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn root_help_stays_root_owned() {
        let _env = EnvironmentGuard::removed(&["FOL_OUTPUT", "FOL_PROFILE"]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code = run_from_args_with_io(["fol", "--help"], &mut stdout, &mut stderr);
        let rendered = String::from_utf8(stdout).expect("help output should be utf8");

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(rendered.contains("User-facing frontend for the FOL toolchain"));
        assert!(!rendered.contains("Workflow Commands:"));
    }

    #[test]
    fn subcommand_help_is_not_swallowed_by_root_help() {
        let _env = EnvironmentGuard::removed(&["FOL_OUTPUT", "FOL_PROFILE"]);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let code =
            run_from_args_with_io(["fol", "code", "emit", "--help"], &mut stdout, &mut stderr);
        let rendered = String::from_utf8(stdout).expect("help output should be utf8");

        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(rendered.contains("Usage: fol code emit"));
        assert!(rendered.contains("rust"));
        assert!(rendered.contains("lowered"));
    }

    #[test]
    fn frontend_config_from_cli_keeps_build_option_overrides() {
        let _env = EnvironmentGuard::removed(FRONTEND_ENV_KEYS);
        let cli = FrontendCli::parse_from([
            "fol",
            "code",
            "build",
            "--target",
            "aarch64-macos-gnu",
            "--optimize",
            "release-fast",
            "--build-option",
            "jobs=16",
        ]);

        let config = frontend_config_from_cli(&cli, None);

        assert_eq!(
            config.build_target_override.as_deref(),
            Some("aarch64-macos-gnu")
        );
        assert_eq!(
            config.build_optimize_override.as_deref(),
            Some("release-fast")
        );
        assert_eq!(config.build_option_overrides, vec!["jobs=16".to_string()]);
    }

    #[test]
    fn frontend_config_from_cli_parses_define_shorthand_into_option_overrides() {
        let _env = EnvironmentGuard::removed(FRONTEND_ENV_KEYS);
        let cli = FrontendCli::parse_from([
            "fol",
            "code",
            "build",
            "-Dtarget=x86_64-linux-gnu",
            "-Doptimize=release-fast",
            "-Dstrip=true",
        ]);

        let config = frontend_config_from_cli(&cli, None);

        assert_eq!(
            config.build_target_override.as_deref(),
            Some("x86_64-linux-gnu")
        );
        assert_eq!(
            config.build_optimize_override.as_deref(),
            Some("release-fast")
        );
        assert_eq!(
            config.build_option_overrides,
            vec!["strip=true".to_string()]
        );
    }

    #[test]
    fn frontend_config_from_cli_keeps_selected_build_step_override() {
        let _env = EnvironmentGuard::removed(FRONTEND_ENV_KEYS);
        let cli = FrontendCli::parse_from(["fol", "code", "build", "--step", "docs"]);

        let config = frontend_config_from_cli(&cli, None);

        assert_eq!(config.build_step_override.as_deref(), Some("docs"));
    }

    #[test]
    fn frontend_config_resolves_output_overrides_from_most_specific_scope() {
        let _env = EnvironmentGuard::removed(FRONTEND_ENV_KEYS);
        let cases = [
            (
                vec!["fol", "--output", "json", "code", "check"],
                OutputMode::Json,
            ),
            (
                vec![
                    "fol", "--output", "json", "code", "--output", "plain", "check",
                ],
                OutputMode::Plain,
            ),
            (
                vec![
                    "fol", "--output", "json", "code", "--output", "plain", "check", "--output",
                    "human",
                ],
                OutputMode::Human,
            ),
            (
                vec![
                    "fol", "--output", "json", "pack", "--output", "plain", "fetch", "--output",
                    "human",
                ],
                OutputMode::Human,
            ),
            (
                vec![
                    "fol", "--output", "json", "code", "--output", "plain", "emit", "rust",
                    "--output", "human",
                ],
                OutputMode::Human,
            ),
            (
                vec![
                    "fol", "--output", "json", "work", "--output", "plain", "info",
                ],
                OutputMode::Plain,
            ),
            (
                vec!["fol", "--output", "json", "tool", "parse", "demo.fol"],
                OutputMode::Json,
            ),
            (
                vec![
                    "fol", "--output", "json", "code", "explain", "R1003", "--output", "plain",
                ],
                OutputMode::Plain,
            ),
            (
                vec![
                    "fol", "--json", "code", "--output", "plain", "check", "--output", "human",
                ],
                OutputMode::Json,
            ),
        ];

        for (args, expected) in cases {
            let cli = FrontendCli::try_parse_from(args).expect("output fixture should parse");
            let config = frontend_config_from_cli(&cli, None);
            assert_eq!(config.output.mode, expected);
        }
    }

    #[test]
    fn workspace_dispatch_routes_absorbed_build_steps_through_named_step_selection() {
        let workspace = absorbed_build_dispatch_fixture("check_step");
        let command = FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs::default(),
            profile: FrontendProfileArgs::default(),
            command: CodeSubcommand::Build(BuildCommand::default()),
        });
        let config = FrontendConfig {
            build_step_override: Some("check".to_string()),
            ..FrontendConfig::default()
        };

        let result = dispatch::dispatch_workspace_command(&command, &workspace, &config).unwrap();

        assert_eq!(result.command, "check");
        assert!(result.summary.contains("checked 1 workspace package(s)"));

        std::fs::remove_dir_all(&workspace.root.root).ok();
    }

    #[test]
    fn workspace_dispatch_keeps_build_artifacts_on_routed_absorbed_build_execution() {
        let workspace = absorbed_build_dispatch_fixture("build_artifacts");
        let command = FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs::default(),
            profile: FrontendProfileArgs::default(),
            command: CodeSubcommand::Build(BuildCommand::default()),
        });

        let result =
            dispatch::dispatch_workspace_command(&command, &workspace, &FrontendConfig::default())
                .unwrap();

        assert_eq!(result.command, "build");
        assert!(result
            .summary
            .contains("built 1 workspace package(s) into "));
        assert_eq!(result.artifacts.len(), 2);
        assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::BuildRoot);
        assert_eq!(result.artifacts[1].kind, FrontendArtifactKind::Binary);
        assert!(result.artifacts[1]
            .path
            .as_ref()
            .expect("binary path should be retained")
            .is_file());

        std::fs::remove_dir_all(&workspace.root.root).ok();
    }

    #[test]
    fn workspace_dispatch_keeps_run_summary_and_binary_artifact_on_routed_execution() {
        let workspace = absorbed_build_dispatch_fixture("run_artifacts");
        let command = FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs::default(),
            profile: FrontendProfileArgs::default(),
            command: CodeSubcommand::Run(RunCommand::default()),
        });

        let result =
            dispatch::dispatch_workspace_command(&command, &workspace, &FrontendConfig::default())
                .unwrap();

        assert_eq!(result.command, "run");
        assert!(result.summary.contains("ran "));
        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::Binary);

        std::fs::remove_dir_all(&workspace.root.root).ok();
    }

    #[test]
    fn workspace_dispatch_executes_modern_semantic_build_packages_through_workspace_route() {
        let workspace = modern_dispatch_fixture("modern_only");
        let command = FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs::default(),
            profile: FrontendProfileArgs::default(),
            command: CodeSubcommand::Build(BuildCommand::default()),
        });

        let result =
            dispatch::dispatch_workspace_command(&command, &workspace, &FrontendConfig::default())
                .expect("modern semantic build packages should execute");

        assert_eq!(result.command, "build");
        assert!(result
            .summary
            .contains("built 1 workspace package(s) into "));

        std::fs::remove_dir_all(&workspace.root.root).ok();
    }

    #[test]
    fn workspace_dispatch_executes_hybrid_semantic_build_packages_through_workspace_route() {
        let workspace = modern_dispatch_fixture("semantic");
        let command = FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs::default(),
            profile: FrontendProfileArgs::default(),
            command: CodeSubcommand::Check(CheckCommand::default()),
        });

        let result =
            dispatch::dispatch_workspace_command(&command, &workspace, &FrontendConfig::default())
                .expect("hybrid build entry packages should execute");

        assert_eq!(result.command, "check");
        assert!(result.summary.contains("checked 1 workspace package(s)"));

        std::fs::remove_dir_all(&workspace.root.root).ok();
    }
}

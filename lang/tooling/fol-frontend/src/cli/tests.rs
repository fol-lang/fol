use super::args::{
    BuildCommand, BuildOptionArgs, BuildStepArgs, CheckCommand, CodeCommand, CodeSubcommand,
    CompileRootArgs, CompleteCommand, CompletionCommand, CompletionShellArg, DirectTargetArg,
    EditorPathCommand, EditorReferenceCommand, EditorRenameCommand, EmitCommand,
    EmitLoweredCommand, EmitRustCommand, EmitSubcommand, ExplainCommand, FetchCommand,
    FrontendCommand, FrontendOutputArgs, FrontendProfile, FrontendProfileArgs, InitCommand,
    NewCommand, PackCommand, PackSubcommand, RunCommand, TestCommand, ToolCommand, ToolSubcommand,
    TreeCommand, TreeGenerateCommand, TreeSubcommand, UnitCommand, UpdateCommand, WorkCommand,
    WorkSubcommand,
};
use super::parser::{FrontendCli, ParseErrorKind};
use crate::OutputMode;
use std::sync::{Mutex, MutexGuard, OnceLock};

fn env_lock() -> MutexGuard<'static, ()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env test lock should not be poisoned")
}

fn parse_clean(args: &[&str]) -> FrontendCli {
    let _guard = env_lock();
    std::env::remove_var("FOL_OUTPUT");
    std::env::remove_var("FOL_PROFILE");
    FrontendCli::parse_from(args.iter().map(|s| s.to_string()))
}

fn try_parse_clean(args: &[&str]) -> Result<FrontendCli, super::parser::ParseError> {
    let _guard = env_lock();
    std::env::remove_var("FOL_OUTPUT");
    std::env::remove_var("FOL_PROFILE");
    FrontendCli::try_parse_from(args.iter().map(|s| s.to_string()))
}

fn default_output_args() -> FrontendOutputArgs {
    FrontendOutputArgs::default()
}

fn default_profile_args() -> FrontendProfileArgs {
    FrontendProfileArgs::default()
}

#[test]
fn derive_root_parser_accepts_empty_invocation() {
    let cli = parse_clean(&["fol"]);

    assert_eq!(cli.output, OutputMode::Human);
    assert_eq!(cli.selected_profile(), FrontendProfile::Debug);
    assert_eq!(cli.command, None);
}

#[test]
fn root_command_families_parse_through_derive_tree() {
    let cli = parse_clean(&["fol", "code", "build"]);

    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand::default()),
        }))
    );
}

#[test]
fn run_command_preserves_passthrough_args() {
    let cli = parse_clean(&["fol", "code", "run", "--", "--flag", "value"]);

    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Run(RunCommand {
                output: default_output_args(),
                profile: default_profile_args(),
                target: DirectTargetArg {
                    input: Some("--flag".to_string()),
                },
                roots: CompileRootArgs::default(),
                options: BuildOptionArgs::default(),
                step: BuildStepArgs::default(),
                locked: false,
                keep_build_dir: false,
                args: vec!["value".to_string()],
            }),
        }))
    );
}

#[test]
fn emit_subcommands_parse_through_derive_tree() {
    let rust = parse_clean(&["fol", "code", "emit", "rust"]);
    let lowered = parse_clean(&["fol", "code", "emit", "lowered"]);

    assert_eq!(
        rust.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Emit(EmitCommand {
                command: EmitSubcommand::Rust(EmitRustCommand::default()),
            }),
        }))
    );
    assert_eq!(
        lowered.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Emit(EmitCommand {
                command: EmitSubcommand::Lowered(EmitLoweredCommand::default()),
            }),
        }))
    );
}

#[test]
fn editor_subcommands_parse_through_derive_tree() {
    let lsp = parse_clean(&["fol", "tool", "lsp"]);
    let format = parse_clean(&["fol", "tool", "format", "demo/main.fol"]);
    let parse = parse_clean(&["fol", "tool", "parse", "demo/main.fol"]);
    let highlight = parse_clean(&["fol", "tool", "highlight", "demo/main.fol"]);
    let symbols = parse_clean(&["fol", "tool", "symbols", "demo/main.fol"]);
    let references = parse_clean(&[
        "fol",
        "tool",
        "references",
        "demo/main.fol",
        "--line",
        "5",
        "--character",
        "11",
    ]);
    let rename = parse_clean(&[
        "fol",
        "tool",
        "rename",
        "demo/main.fol",
        "--line",
        "5",
        "--character",
        "11",
        "count",
    ]);
    let semantic_tokens =
        parse_clean(&["fol", "tool", "semantic-tokens", "demo/main.fol"]);
    let tree = parse_clean(&["fol", "tool", "tree", "generate", "/tmp/fol-tree"]);

    assert_eq!(
        lsp.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Lsp(UnitCommand),
        }))
    );
    assert_eq!(
        format.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Format(EditorPathCommand {
                path: "demo/main.fol".to_string(),
            }),
        }))
    );
    assert_eq!(
        parse.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Parse(EditorPathCommand {
                path: "demo/main.fol".to_string(),
            }),
        }))
    );
    assert_eq!(
        highlight.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Highlight(EditorPathCommand {
                path: "demo/main.fol".to_string(),
            }),
        }))
    );
    assert_eq!(
        symbols.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Symbols(EditorPathCommand {
                path: "demo/main.fol".to_string(),
            }),
        }))
    );
    assert_eq!(
        references.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::References(EditorReferenceCommand {
                path: "demo/main.fol".to_string(),
                line: 5,
                character: 11,
                exclude_declaration: false,
            }),
        }))
    );
    assert_eq!(
        rename.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Rename(EditorRenameCommand {
                path: "demo/main.fol".to_string(),
                line: 5,
                character: 11,
                new_name: "count".to_string(),
            }),
        }))
    );
    assert_eq!(
        semantic_tokens.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::SemanticTokens(EditorPathCommand {
                path: "demo/main.fol".to_string(),
            }),
        }))
    );
    assert_eq!(
        tree.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Tree(TreeCommand {
                command: TreeSubcommand::Generate(TreeGenerateCommand {
                    path: "/tmp/fol-tree".to_string(),
                }),
            }),
        }))
    );
}

#[test]
fn editor_subcommands_parse_edge_flags_and_output_modes() {
    let references = parse_clean(&[
        "fol",
        "tool",
        "--output",
        "plain",
        "references",
        "demo/main.fol",
        "--line",
        "5",
        "--character",
        "11",
        "--exclude-declaration",
    ]);
    let rename = parse_clean(&[
        "fol",
        "tool",
        "--output",
        "json",
        "rename",
        "demo/main.fol",
        "--line",
        "5",
        "--character",
        "11",
        "count",
    ]);

    assert_eq!(
        references.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: FrontendOutputArgs {
                output: Some(OutputMode::Plain),
            },
            command: ToolSubcommand::References(EditorReferenceCommand {
                path: "demo/main.fol".to_string(),
                line: 5,
                character: 11,
                exclude_declaration: true,
            }),
        }))
    );
    assert_eq!(
        rename.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: FrontendOutputArgs {
                output: Some(OutputMode::Json),
            },
            command: ToolSubcommand::Rename(EditorRenameCommand {
                path: "demo/main.fol".to_string(),
                line: 5,
                character: 11,
                new_name: "count".to_string(),
            }),
        }))
    );
}

#[test]
fn unsupported_future_editor_commands_stay_rejected_by_cli() {
    for args in [
        vec!["fol", "tool", "workspace-symbols", "needle"],
        vec!["fol", "tool", "range-format", "demo/main.fol"],
        vec!["fol", "tool", "semanticTokens", "demo/main.fol"],
    ] {
        let error = try_parse_clean(&args).expect_err("future editor command should stay rejected");
        assert!(matches!(error.kind, ParseErrorKind::InvalidSubcommand(_)));
    }
}

#[test]
fn completion_command_parses_requested_shell() {
    let cli = parse_clean(&["fol", "tool", "completion", "bash"]);

    assert_eq!(
        cli.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Completion(CompletionCommand {
                shell: CompletionShellArg::Bash,
            }),
        }))
    );
}

#[test]
fn explain_command_parses_under_the_code_group() {
    let plain = parse_clean(&["fol", "code", "explain", "T1003"]);
    let cmd_json = parse_clean(&["fol", "code", "explain", "R1003", "--output", "json"]);
    let cmd_json_short = parse_clean(&["fol", "code", "explain", "--json", "P1001"]);

    assert_eq!(
        plain.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Explain(ExplainCommand {
                code: "T1003".to_string(),
                output: None,
            }),
        }))
    );
    assert_eq!(
        cmd_json.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Explain(ExplainCommand {
                code: "R1003".to_string(),
                output: Some(OutputMode::Json),
            }),
        }))
    );
    assert_eq!(
        cmd_json_short.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Explain(ExplainCommand {
                code: "P1001".to_string(),
                output: Some(OutputMode::Json),
            }),
        }))
    );
}

#[test]
fn explain_command_requires_a_code() {
    let error = try_parse_clean(&["fol", "code", "explain"]).expect_err("explain needs a code");
    assert!(matches!(error.kind, ParseErrorKind::MissingValue(_)));
}

#[test]
fn top_level_explain_command_is_removed() {
    // `fol explain` no longer exists; the token is treated as a direct input
    // target, not an explain command.
    let cli = parse_clean(&["fol", "explain", "T1003"]);
    assert!(
        cli.command.is_none(),
        "top-level explain must not resolve to a command"
    );
    assert_eq!(cli.input.as_deref(), Some("explain"));
}

#[test]
fn internal_complete_command_parses_optional_current_token() {
    let cli = parse_clean(&["fol", "_complete", "code", "emit", "ru"]);

    assert_eq!(
        cli.command,
        Some(FrontendCommand::Complete(CompleteCommand {
            tokens: vec!["code".to_string(), "emit".to_string(), "ru".to_string()],
        }))
    );
}

#[test]
fn visible_aliases_parse_to_the_same_root_commands() {
    let build = parse_clean(&["fol", "code", "make"]);
    let check = parse_clean(&["fol", "code", "verify"]);
    let work = parse_clean(&["fol", "w", "info"]);
    let pack = parse_clean(&["fol", "p", "fetch"]);
    let code = parse_clean(&["fol", "c", "build"]);
    let editor = parse_clean(&["fol", "t", "lsp"]);
    let tool = parse_clean(&["fol", "t", "clean"]);
    let fetch = parse_clean(&["fol", "pack", "sync"]);
    let update = parse_clean(&["fol", "pack", "upgrade"]);
    let emit = parse_clean(&["fol", "code", "gen", "rust"]);
    let clean = parse_clean(&["fol", "tool", "purge"]);

    assert_eq!(
        build.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand::default()),
        }))
    );
    assert_eq!(
        check.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Check(CheckCommand::default()),
        }))
    );
    assert_eq!(
        fetch.command,
        Some(FrontendCommand::Pack(PackCommand {
            output: default_output_args(),
            command: PackSubcommand::Fetch(FetchCommand::default()),
        }))
    );
    assert_eq!(
        update.command,
        Some(FrontendCommand::Pack(PackCommand {
            output: default_output_args(),
            command: PackSubcommand::Update(UpdateCommand::default()),
        }))
    );
    assert_eq!(
        emit.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Emit(EmitCommand {
                command: EmitSubcommand::Rust(EmitRustCommand::default()),
            }),
        }))
    );
    assert_eq!(
        clean.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Clean(UnitCommand),
        }))
    );
    assert_eq!(
        work.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::Info(UnitCommand),
        }))
    );
    assert_eq!(
        pack.command,
        Some(FrontendCommand::Pack(PackCommand {
            output: default_output_args(),
            command: PackSubcommand::Fetch(FetchCommand::default()),
        }))
    );
    assert_eq!(
        code.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand::default()),
        }))
    );
    assert_eq!(
        editor.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Lsp(UnitCommand),
        }))
    );
    assert_eq!(
        tool.command,
        Some(FrontendCommand::Tool(ToolCommand {
            output: default_output_args(),
            command: ToolSubcommand::Clean(UnitCommand),
        }))
    );
}

#[test]
fn output_flag_parses_global_output_mode() {
    let cli = parse_clean(&["fol", "code", "--output", "json", "build"]);

    assert_eq!(cli.output, OutputMode::Human);
    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs {
                output: Some(OutputMode::Json)
            },
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand::default()),
        }))
    );
}

#[test]
fn root_output_override_is_not_copied_into_command_defaults() {
    let cli = parse_clean(&["fol", "--output", "json", "code", "check"]);

    assert_eq!(cli.output, OutputMode::Json);
    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Check(CheckCommand::default()),
        }))
    );
}

#[test]
fn command_local_output_overrides_are_retained_explicitly() {
    let cli = parse_clean(&[
        "fol", "--output", "json", "code", "--output", "plain", "check", "--output", "human",
    ]);

    assert_eq!(cli.output, OutputMode::Json);
    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs {
                output: Some(OutputMode::Plain),
            },
            profile: default_profile_args(),
            command: CodeSubcommand::Check(CheckCommand {
                output: FrontendOutputArgs {
                    output: Some(OutputMode::Human),
                },
                ..CheckCommand::default()
            }),
        }))
    );
}

#[test]
fn profile_flags_normalize_to_frontend_profile_selection() {
    let profile = parse_clean(&["fol", "code", "--profile", "release", "build"]);
    let release = parse_clean(&["fol", "code", "--release", "build"]);

    assert_eq!(
        profile.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: FrontendProfileArgs {
                profile: Some(FrontendProfile::Release),
                debug: false,
                release: false,
            },
            command: CodeSubcommand::Build(BuildCommand::default()),
        }))
    );
    assert_eq!(
        release.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: FrontendProfileArgs {
                profile: None,
                debug: false,
                release: true,
            },
            command: CodeSubcommand::Build(BuildCommand::default()),
        }))
    );
}

#[test]
fn cli_env_values_feed_output_and_profile_defaults() {
    let _guard = env_lock();
    std::env::set_var("FOL_OUTPUT", "plain");
    std::env::set_var("FOL_PROFILE", "release");

    let cli = FrontendCli::parse_from(["fol", "code", "build"].iter().map(|s| s.to_string()));

    assert_eq!(cli.output, OutputMode::Plain);
    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: FrontendProfileArgs {
                profile: Some(FrontendProfile::Release),
                debug: false,
                release: false,
            },
            command: CodeSubcommand::Build(BuildCommand {
                output: default_output_args(),
                profile: FrontendProfileArgs {
                    profile: Some(FrontendProfile::Release),
                    debug: false,
                    release: false,
                },
                ..BuildCommand::default()
            }),
        }))
    );

    std::env::remove_var("FOL_OUTPUT");
    std::env::remove_var("FOL_PROFILE");
}

#[test]
fn explicit_flags_override_env_values() {
    let _guard = env_lock();
    std::env::set_var("FOL_OUTPUT", "plain");
    std::env::set_var("FOL_PROFILE", "release");

    let cli =
        FrontendCli::parse_from(["fol", "code", "--output", "json", "--profile", "debug", "build"].iter().map(|s| s.to_string()));

    assert_eq!(cli.output, OutputMode::Plain);
    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: FrontendOutputArgs {
                output: Some(OutputMode::Json)
            },
            profile: FrontendProfileArgs {
                profile: Some(FrontendProfile::Debug),
                debug: false,
                release: false,
            },
            command: CodeSubcommand::Build(BuildCommand {
                output: default_output_args(),
                profile: FrontendProfileArgs {
                    profile: Some(FrontendProfile::Release),
                    debug: false,
                    release: false,
                },
                ..BuildCommand::default()
            }),
        }))
    );

    std::env::remove_var("FOL_OUTPUT");
    std::env::remove_var("FOL_PROFILE");
}

#[test]
fn build_commands_parse_build_option_overrides() {
    let cli = parse_clean(&[
        "fol",
        "code",
        "build",
        "--target",
        "aarch64-macos-gnu",
        "--optimize",
        "release-fast",
        "--build-option",
        "jobs=16",
        "--build-option",
        "strip=true",
    ]);

    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand {
                output: default_output_args(),
                profile: default_profile_args(),
                target: DirectTargetArg::default(),
                roots: CompileRootArgs::default(),
                options: BuildOptionArgs {
                    build_target: Some("aarch64-macos-gnu".to_string()),
                    build_optimize: Some("release-fast".to_string()),
                    build_options: vec!["jobs=16".to_string(), "strip=true".to_string()],
                    define: Vec::new(),
                },
                step: BuildStepArgs::default(),
                locked: false,
                keep_build_dir: false,
            }),
        }))
    );
}

#[test]
fn workspace_code_commands_parse_explicit_step_selection() {
    let build = parse_clean(&["fol", "code", "build", "--step", "docs"]);
    let run = parse_clean(&["fol", "code", "run", "--step", "bench"]);
    let test = parse_clean(&["fol", "code", "test", "--step", "unit"]);
    let check = parse_clean(&["fol", "code", "check", "--step", "lint"]);

    assert_eq!(
        build.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand {
                step: BuildStepArgs {
                    step: Some("docs".to_string()),
                },
                ..BuildCommand::default()
            }),
        }))
    );
    assert_eq!(
        run.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Run(RunCommand {
                step: BuildStepArgs {
                    step: Some("bench".to_string()),
                },
                ..RunCommand::default()
            }),
        }))
    );
    assert_eq!(
        test.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Test(TestCommand {
                step: BuildStepArgs {
                    step: Some("unit".to_string()),
                },
                ..TestCommand::default()
            }),
        }))
    );
    assert_eq!(
        check.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Check(CheckCommand {
                step: BuildStepArgs {
                    step: Some("lint".to_string()),
                },
                ..CheckCommand::default()
            }),
        }))
    );
}

#[test]
fn help_output_points_users_to_subcommand_help() {
    let help = FrontendCli::root_help_text();

    assert!(help.contains("Run `fol <group> <command> --help` for command-specific usage."));
}

#[test]
fn help_output_keeps_global_mode_flags_hidden() {
    let help = FrontendCli::root_help_text();

    // Root help should not show internal flags
    assert!(!help.contains("--dump-lowered"));
    assert!(!help.contains("--emit-rust"));
    assert!(!help.contains("--build-option"));
    assert!(!help.contains("FILE_OR_FOLDER"));
}

#[test]
fn help_output_mentions_visible_aliases() {
    let help = FrontendCli::root_help_text();

    assert!(help.contains("work"));
    assert!(help.contains("[aliases: w]"));
    assert!(help.contains("pack"));
    assert!(help.contains("[aliases: p]"));
    assert!(help.contains("code"));
    assert!(help.contains("[aliases: c]"));
    assert!(help.contains("tool"));
    assert!(help.contains("[aliases: t]"));
}

#[test]
fn work_subcommands_parse_for_info_and_list() {
    let info = parse_clean(&["fol", "work", "info"]);
    let list = parse_clean(&["fol", "work", "list"]);
    let deps = parse_clean(&["fol", "work", "deps"]);
    let status = parse_clean(&["fol", "work", "status"]);

    assert_eq!(
        info.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::Info(UnitCommand),
        }))
    );
    assert_eq!(
        list.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::List(UnitCommand),
        }))
    );
    assert_eq!(
        deps.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::Deps(UnitCommand),
        }))
    );
    assert_eq!(
        status.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::Status(UnitCommand),
        }))
    );
}

#[test]
fn workspace_flags_parse_for_init_and_new_commands() {
    let init = parse_clean(&["fol", "work", "init", "--workspace"]);
    let new = parse_clean(&["fol", "work", "new", "demo", "--workspace"]);

    assert_eq!(
        init.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::Init(InitCommand {
                workspace: true,
                bin: false,
                lib: false
            }),
        }))
    );
    assert_eq!(
        new.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::New(NewCommand {
                name: "demo".to_string(),
                workspace: true,
                bin: false,
                lib: false,
            }),
        }))
    );
}

#[test]
fn bin_flags_parse_for_init_and_new_commands() {
    let init = parse_clean(&["fol", "work", "init", "--bin"]);
    let new = parse_clean(&["fol", "work", "new", "demo", "--bin"]);

    assert_eq!(
        init.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::Init(InitCommand {
                workspace: false,
                bin: true,
                lib: false
            }),
        }))
    );
    assert_eq!(
        new.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::New(NewCommand {
                name: "demo".to_string(),
                workspace: false,
                bin: true,
                lib: false,
            }),
        }))
    );
}

#[test]
fn lib_flags_parse_for_init_and_new_commands() {
    let init = parse_clean(&["fol", "work", "init", "--lib"]);
    let new = parse_clean(&["fol", "work", "new", "demo", "--lib"]);

    assert_eq!(
        init.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::Init(InitCommand {
                workspace: false,
                bin: false,
                lib: true
            }),
        }))
    );
    assert_eq!(
        new.command,
        Some(FrontendCommand::Work(WorkCommand {
            output: default_output_args(),
            path: None,
            command: WorkSubcommand::New(NewCommand {
                name: "demo".to_string(),
                workspace: false,
                bin: false,
                lib: true,
            }),
        }))
    );
}

#[test]
fn build_command_owns_direct_compile_flags() {
    let cli = parse_clean(&[
        "fol",
        "code",
        "build",
        "--std-root",
        "/tmp/std",
        "--package-store-root",
        "/tmp/pkg",
        "--keep-build-dir",
        "demo",
    ]);

    assert_eq!(
        cli.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand {
                output: default_output_args(),
                profile: default_profile_args(),
                target: DirectTargetArg {
                    input: Some("demo".to_string()),
                },
                roots: CompileRootArgs {
                    std_root: Some("/tmp/std".to_string()),
                    package_store_root: Some("/tmp/pkg".to_string()),
                },
                options: BuildOptionArgs::default(),
                step: BuildStepArgs::default(),
                locked: false,
                keep_build_dir: true,
            }),
        }))
    );
}

#[test]
fn fetch_and_locked_workflow_flags_parse_on_commands() {
    let fetch = parse_clean(&["fol", "pack", "fetch", "--locked", "--offline", "--refresh"]);
    let build = parse_clean(&["fol", "code", "build", "--locked"]);
    let run = parse_clean(&["fol", "code", "run", "--locked"]);
    let test = parse_clean(&["fol", "code", "test", "--locked"]);
    let check = parse_clean(&["fol", "code", "check", "--locked"]);

    assert_eq!(
        fetch.command,
        Some(FrontendCommand::Pack(PackCommand {
            output: default_output_args(),
            command: PackSubcommand::Fetch(FetchCommand {
                output: default_output_args(),
                roots: CompileRootArgs::default(),
                locked: true,
                offline: true,
                refresh: true,
            }),
        }))
    );
    assert_eq!(
        build.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Build(BuildCommand {
                output: default_output_args(),
                profile: default_profile_args(),
                target: DirectTargetArg::default(),
                roots: CompileRootArgs::default(),
                options: BuildOptionArgs::default(),
                step: BuildStepArgs::default(),
                locked: true,
                keep_build_dir: false,
            }),
        }))
    );
    assert_eq!(
        run.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Run(RunCommand {
                output: default_output_args(),
                profile: default_profile_args(),
                target: DirectTargetArg::default(),
                roots: CompileRootArgs::default(),
                options: BuildOptionArgs::default(),
                step: BuildStepArgs::default(),
                locked: true,
                keep_build_dir: false,
                args: Vec::new(),
            }),
        }))
    );
    assert_eq!(
        test.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Test(TestCommand {
                output: default_output_args(),
                profile: default_profile_args(),
                options: BuildOptionArgs::default(),
                step: BuildStepArgs::default(),
                path: None,
                locked: true,
            }),
        }))
    );
    assert_eq!(
        check.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Check(CheckCommand {
                output: default_output_args(),
                profile: default_profile_args(),
                target: DirectTargetArg::default(),
                roots: CompileRootArgs::default(),
                options: BuildOptionArgs::default(),
                step: BuildStepArgs::default(),
                locked: true,
            }),
        }))
    );
}

#[test]
fn emit_subcommands_own_their_specific_flags() {
    let rust = parse_clean(&["fol", "code", "emit", "rust", "--keep-build-dir", "demo"]);
    let lowered = parse_clean(&[
        "fol",
        "code",
        "emit",
        "lowered",
        "--std-root",
        "/tmp/std",
        "demo",
    ]);

    assert_eq!(
        rust.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Emit(EmitCommand {
                command: EmitSubcommand::Rust(EmitRustCommand {
                    output: default_output_args(),
                    profile: default_profile_args(),
                    target: DirectTargetArg {
                        input: Some("demo".to_string()),
                    },
                    roots: CompileRootArgs::default(),
                    keep_build_dir: true,
                }),
            }),
        }))
    );
    assert_eq!(
        lowered.command,
        Some(FrontendCommand::Code(CodeCommand {
            output: default_output_args(),
            profile: default_profile_args(),
            command: CodeSubcommand::Emit(EmitCommand {
                command: EmitSubcommand::Lowered(EmitLoweredCommand {
                    output: default_output_args(),
                    profile: default_profile_args(),
                    target: DirectTargetArg {
                        input: Some("demo".to_string()),
                    },
                    roots: CompileRootArgs {
                        std_root: Some("/tmp/std".to_string()),
                        package_store_root: None,
                    },
                }),
            }),
        }))
    );
}

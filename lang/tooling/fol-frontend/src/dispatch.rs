use crate::cli::parser::ParseErrorKind;
use crate::cli::{CodeSubcommand, EmitCommand, FrontendCli, FrontendCommand};
use crate::{
    build_route, cli, compile, CompletionShell, DirectCompileConfig, DirectCompileMode,
    DiscoveredRoot, EmitSubcommand, FrontendCommandResult, FrontendConfig, FrontendError,
    FrontendErrorKind, FrontendOutput, FrontendOutputConfig, FrontendProfile, FrontendResult,
    FrontendWorkspace, FrontendWorkspaceBuildRequest, PackSubcommand, ToolSubcommand,
};

pub fn dispatch_cli(
    cli: &FrontendCli,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    if let Some(input) = &cli.input {
        return crate::run_direct_compile(
            &DirectCompileConfig {
                input: input.clone(),
                std_root: cli.std_root.clone(),
                package_store_root: cli.package_store_root.clone(),
                mode: DirectCompileMode::Auto {
                    dump_lowered: cli.dump_lowered,
                    emit_rust: cli.emit_rust,
                    keep_build_dir: cli.keep_build_dir,
                },
            },
            config,
        );
    }

    match cli.command.as_ref() {
        None => Err(FrontendError::new(
            FrontendErrorKind::InvalidInput,
            "no frontend command was provided",
        )
        .with_note("run `fol --help` to inspect the frontend workflow")),
        Some(cmd @ FrontendCommand::Work(command)) => match &command.command {
            cli::WorkSubcommand::Init(command) => crate::init_root(
                &config.working_directory,
                command.workspace,
                crate::package_target_kind(command.bin, command.lib),
            ),
            cli::WorkSubcommand::New(command) => crate::new_project_with_mode(
                &config.working_directory,
                &command.name,
                command.workspace,
                crate::package_target_kind(command.bin, command.lib),
            ),
            _ => {
                let discovered = discovered_root_for_command(cmd, &config.working_directory)?;
                let workspace = crate::load_frontend_workspace(&discovered, config)?;
                dispatch_workspace_command(cmd, &workspace, config)
            }
        },
        Some(FrontendCommand::Code(command))
            if matches!(command.command, CodeSubcommand::Explain(_)) =>
        {
            let code = match &command.command {
                CodeSubcommand::Explain(explain) => explain.code.clone(),
                _ => unreachable!("guarded to Explain above"),
            };
            crate::explain_command(&code, config.output.mode)
        }
        Some(cmd @ (FrontendCommand::Pack(_) | FrontendCommand::Code(_))) => {
            let needs_direct = match cmd {
                FrontendCommand::Code(command) => code_has_direct_target(command),
                _ => false,
            };
            if needs_direct {
                dispatch_direct_grouped_command(cmd, config)
            } else {
                let discovered = discovered_root_for_command(cmd, &config.working_directory)?;
                let workspace = crate::load_frontend_workspace(&discovered, config)?;
                dispatch_workspace_command(cmd, &workspace, config)
            }
        }
        Some(cmd @ FrontendCommand::Tool(command)) => match &command.command {
            ToolSubcommand::Lsp(_) => crate::editor_lsp_command(config),
            ToolSubcommand::Format(command) => crate::editor_format_command(&command.path),
            ToolSubcommand::Parse(command) => crate::editor_parse_command(&command.path),
            ToolSubcommand::Highlight(command) => crate::editor_highlight_command(&command.path),
            ToolSubcommand::Symbols(command) => crate::editor_symbols_command(&command.path),
            ToolSubcommand::References(command) => crate::editor_references_command(
                &command.path,
                command.line,
                command.character,
                !command.exclude_declaration,
            ),
            ToolSubcommand::Rename(command) => crate::editor_rename_command(
                &command.path,
                command.line,
                command.character,
                &command.new_name,
            ),
            ToolSubcommand::Complete(command) => {
                crate::editor_completion_command(&command.path, command.line, command.character)
            }
            ToolSubcommand::SemanticTokens(command) => {
                crate::editor_semantic_tokens_command(&command.path)
            }
            ToolSubcommand::Tree(command) => match &command.command {
                cli::TreeSubcommand::Generate(command) => {
                    crate::editor_tree_generate_command(&command.path)
                }
            },
            ToolSubcommand::Completion(command) => {
                crate::completion_command(parse_completion_shell(command.shell))
            }
            ToolSubcommand::Clean(_) => {
                let discovered = discovered_root_for_command(cmd, &config.working_directory)?;
                let workspace = crate::load_frontend_workspace(&discovered, config)?;
                dispatch_workspace_command(cmd, &workspace, config)
            }
        },
        Some(FrontendCommand::Complete(command)) => {
            crate::internal_complete_command_with_tokens(&command.tokens)
        }
    }
}

pub fn dispatch_direct_grouped_command(
    command: &FrontendCommand,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    match command {
        FrontendCommand::Code(command) => match &command.command {
            CodeSubcommand::Build(command) => crate::run_direct_compile(
                &DirectCompileConfig {
                    input: command.target.input.clone().unwrap_or_default(),
                    std_root: command.roots.std_root.clone(),
                    package_store_root: command.roots.package_store_root.clone(),
                    mode: DirectCompileMode::Build {
                        keep_build_dir: command.keep_build_dir,
                    },
                },
                &config_for_roots_keep_build(config, &command.roots, command.keep_build_dir),
            ),
            CodeSubcommand::Check(command) => crate::run_direct_compile(
                &DirectCompileConfig {
                    input: command.target.input.clone().unwrap_or_default(),
                    std_root: command.roots.std_root.clone(),
                    package_store_root: command.roots.package_store_root.clone(),
                    mode: DirectCompileMode::Check,
                },
                &config_for_roots(config, &command.roots),
            ),
            CodeSubcommand::Run(command) => crate::run_direct_compile(
                &DirectCompileConfig {
                    input: command.target.input.clone().unwrap_or_default(),
                    std_root: command.roots.std_root.clone(),
                    package_store_root: command.roots.package_store_root.clone(),
                    mode: DirectCompileMode::Run {
                        keep_build_dir: command.keep_build_dir,
                        args: command.args.clone(),
                    },
                },
                &config_for_roots_keep_build(config, &command.roots, command.keep_build_dir),
            ),
            CodeSubcommand::Emit(command) => match &command.command {
                EmitSubcommand::Rust(emit) => crate::run_direct_compile(
                    &DirectCompileConfig {
                        input: emit.target.input.clone().unwrap_or_default(),
                        std_root: emit.roots.std_root.clone(),
                        package_store_root: emit.roots.package_store_root.clone(),
                        mode: DirectCompileMode::EmitRust {
                            keep_build_dir: emit.keep_build_dir,
                        },
                    },
                    &config_for_roots_keep_build(config, &emit.roots, emit.keep_build_dir),
                ),
                EmitSubcommand::Lowered(emit) => crate::run_direct_compile(
                    &DirectCompileConfig {
                        input: emit.target.input.clone().unwrap_or_default(),
                        std_root: emit.roots.std_root.clone(),
                        package_store_root: emit.roots.package_store_root.clone(),
                        mode: DirectCompileMode::EmitLowered,
                    },
                    &config_for_roots(config, &emit.roots),
                ),
            },
            CodeSubcommand::Test(_) => Err(FrontendError::new(
                FrontendErrorKind::Internal,
                "unexpected direct test dispatch",
            )),
            CodeSubcommand::Explain(_) => Err(FrontendError::new(
                FrontendErrorKind::Internal,
                "unexpected direct explain dispatch",
            )),
        },
        _ => Err(FrontendError::new(
            FrontendErrorKind::Internal,
            "unexpected grouped direct dispatch",
        )),
    }
}

pub fn dispatch_workspace_command(
    command: &FrontendCommand,
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    match command {
        FrontendCommand::Work(command) => Ok(match command.command {
            cli::WorkSubcommand::Init(_) | cli::WorkSubcommand::New(_) => {
                return Err(FrontendError::new(
                    FrontendErrorKind::Internal,
                    "unexpected work setup command reached workspace dispatcher",
                ))
            }
            cli::WorkSubcommand::Info(_) => crate::work_info(workspace),
            cli::WorkSubcommand::List(_) => crate::work_list(workspace),
            cli::WorkSubcommand::Deps(_) => crate::work_deps(workspace)?,
            cli::WorkSubcommand::Status(_) => crate::work_status(workspace, config)?,
        }),
        FrontendCommand::Pack(command) => match &command.command {
            PackSubcommand::Fetch(command) => crate::fetch_workspace_with_config(
                workspace,
                &config_for_roots(config, &command.roots),
            ),
            PackSubcommand::Update(command) => crate::update_workspace_with_config(
                workspace,
                &config_for_roots(config, &command.roots),
            ),
        },
        FrontendCommand::Code(code) => match &code.command {
            CodeSubcommand::Build(build) => {
                let routed_config =
                    config_for_roots_keep_build(config, &build.roots, build.keep_build_dir);
                dispatch_workspace_code_route(
                    workspace,
                    &routed_config,
                    &code.command,
                    FrontendProfile::Debug,
                    &[],
                )
            }
            CodeSubcommand::Check(check) => {
                let routed_config = config_for_roots(config, &check.roots);
                dispatch_workspace_code_route(
                    workspace,
                    &routed_config,
                    &code.command,
                    FrontendProfile::Debug,
                    &[],
                )
            }
            CodeSubcommand::Run(run) => {
                let routed_config =
                    config_for_roots_keep_build(config, &run.roots, run.keep_build_dir);
                dispatch_workspace_code_route(
                    workspace,
                    &routed_config,
                    &code.command,
                    FrontendProfile::Debug,
                    &run.args,
                )
            }
            CodeSubcommand::Test(_) => dispatch_workspace_code_route(
                workspace,
                config,
                &code.command,
                FrontendProfile::Debug,
                &[],
            ),
            CodeSubcommand::Emit(command) => match &command.command {
                EmitSubcommand::Rust(emit) => compile::emit_rust_with_config(
                    workspace,
                    &config_for_roots_keep_build(config, &emit.roots, emit.keep_build_dir),
                ),
                EmitSubcommand::Lowered(emit) => compile::emit_lowered_with_config(
                    workspace,
                    &config_for_roots(config, &emit.roots),
                ),
            },
            CodeSubcommand::Explain(_) => Err(FrontendError::new(
                FrontendErrorKind::Internal,
                "unexpected explain command reached workspace dispatcher",
            )),
        },
        FrontendCommand::Tool(command) => match &command.command {
            ToolSubcommand::Clean(_) => crate::clean_workspace_with_config(workspace, config),
            ToolSubcommand::Lsp(_)
            | ToolSubcommand::Format(_)
            | ToolSubcommand::Parse(_)
            | ToolSubcommand::Highlight(_)
            | ToolSubcommand::Symbols(_)
            | ToolSubcommand::References(_)
            | ToolSubcommand::Rename(_)
            | ToolSubcommand::Complete(_)
            | ToolSubcommand::SemanticTokens(_)
            | ToolSubcommand::Tree(_) => Err(FrontendError::new(
                FrontendErrorKind::Internal,
                "unexpected editor command reached workspace dispatcher",
            )),
            ToolSubcommand::Completion(_) => Err(FrontendError::new(
                FrontendErrorKind::Internal,
                "unexpected completion command reached workspace dispatcher",
            )),
        },
        FrontendCommand::Complete(_) => Err(FrontendError::new(
            FrontendErrorKind::Internal,
            "unexpected command reached workspace dispatcher",
        )),
    }
}

fn dispatch_workspace_code_route(
    workspace: &FrontendWorkspace,
    config: &FrontendConfig,
    command: &CodeSubcommand,
    default_profile: FrontendProfile,
    run_args: &[String],
) -> FrontendResult<FrontendCommandResult> {
    let requested_step =
        build_route::requested_workspace_step(command, config.build_step_override.as_deref());
    if matches!(command, CodeSubcommand::Emit(_)) {
        return Err(FrontendError::new(
            FrontendErrorKind::Internal,
            "emit commands do not participate in workspace build-step routing",
        ));
    }

    build_route::execute_workspace_build_route(
        workspace,
        config,
        &FrontendWorkspaceBuildRequest {
            requested_step,
            profile: config.profile_override.unwrap_or(default_profile),
            run_args: run_args.to_vec(),
        },
    )
}

fn emit_has_direct_target(command: &EmitCommand) -> bool {
    match &command.command {
        EmitSubcommand::Rust(emit) => emit.target.input.is_some(),
        EmitSubcommand::Lowered(emit) => emit.target.input.is_some(),
    }
}

pub fn code_has_direct_target(command: &cli::CodeCommand) -> bool {
    match &command.command {
        CodeSubcommand::Build(command) => command.target.input.is_some(),
        CodeSubcommand::Run(command) => command.target.input.is_some(),
        CodeSubcommand::Check(command) => command.target.input.is_some(),
        CodeSubcommand::Emit(command) => emit_has_direct_target(command),
        CodeSubcommand::Test(_) => false,
        CodeSubcommand::Explain(_) => false,
    }
}

pub fn config_for_roots(base: &FrontendConfig, roots: &cli::CompileRootArgs) -> FrontendConfig {
    let mut config = base.clone();
    if let Some(std_root) = &roots.std_root {
        config.std_root_override = Some(std_root.into());
    }
    if let Some(package_store_root) = &roots.package_store_root {
        config.package_store_root_override = Some(package_store_root.into());
    }
    config
}

pub fn config_for_roots_keep_build(
    base: &FrontendConfig,
    roots: &cli::CompileRootArgs,
    keep_build_dir: bool,
) -> FrontendConfig {
    let mut config = config_for_roots(base, roots);
    config.keep_build_dir = keep_build_dir;
    config
}

pub fn discovered_root_for_command(
    command: &FrontendCommand,
    working_directory: &std::path::Path,
) -> FrontendResult<DiscoveredRoot> {
    let explicit = match command {
        FrontendCommand::Work(command) => command.path.as_deref(),
        FrontendCommand::Code(command) => match &command.command {
            CodeSubcommand::Test(command) => command.path.as_deref(),
            _ => None,
        },
        _ => None,
    };
    if let Some(path) = explicit {
        crate::require_discovered_root(std::path::Path::new(path))
    } else {
        crate::require_discovered_root(working_directory)
    }
}

fn parse_completion_shell(shell: crate::CompletionShellArg) -> CompletionShell {
    match shell {
        crate::CompletionShellArg::Bash => CompletionShell::Bash,
        crate::CompletionShellArg::Zsh => CompletionShell::Zsh,
        crate::CompletionShellArg::Fish => CompletionShell::Fish,
    }
}

pub fn run_from_args_with_io_inner<I, T>(
    args: I,
    stdout: &mut impl std::io::Write,
    stderr: &mut impl std::io::Write,
) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|arg| arg.into().into_string().unwrap_or_default())
        .collect();

    // Enable ANSI colors when stdout is a terminal (before help text is rendered).
    crate::ansi::set_enabled(std::io::IsTerminal::is_terminal(&std::io::stdout()));

    match FrontendCli::try_parse_from(args.clone()) {
        Err(error)
            if matches!(
                error.kind,
                ParseErrorKind::Help(_) | ParseErrorKind::Version
            ) =>
        {
            match writeln!(stdout, "{error}") {
                Ok(()) => 0,
                Err(render_error) => {
                    let _ = writeln!(stderr, "FrontendInternal: {render_error}");
                    1
                }
            }
        }
        Err(error) => {
            let output = FrontendOutput::new(FrontendOutputConfig::default());
            let error = FrontendError::new(FrontendErrorKind::InvalidInput, error.to_string())
                .with_note("run `fol --help` to inspect the available workflow commands");
            match output.render_error(&error) {
                Ok(rendered) => {
                    let _ = writeln!(stderr, "{rendered}");
                }
                Err(render_error) => {
                    let _ = writeln!(stderr, "FrontendInternal: {render_error}");
                }
            }
            1
        }
        Ok(cli) if cli.input.is_some() => {
            let config = crate::frontend_config_from_cli(&cli, None);
            crate::run_direct_compile_with_io(
                &DirectCompileConfig {
                    input: cli.input.clone().unwrap_or_default(),
                    std_root: cli.std_root.clone(),
                    package_store_root: cli.package_store_root.clone(),
                    mode: DirectCompileMode::Auto {
                        dump_lowered: cli.dump_lowered,
                        emit_rust: cli.emit_rust,
                        keep_build_dir: cli.keep_build_dir,
                    },
                },
                &config,
                stdout,
            )
        }
        Ok(cli) if cli.command.is_none() => {
            let help = FrontendCli::root_help_text();
            match writeln!(stdout, "{help}") {
                Ok(()) => 0,
                Err(error) => {
                    let _ = writeln!(stderr, "FrontendInternal: {error}");
                    1
                }
            }
        }
        Ok(cli) if matches!(cli.command.as_ref(), Some(FrontendCommand::Tool(command)) if matches!(command.command, ToolSubcommand::Lsp(_))) =>
        {
            let config = crate::frontend_config_from_cli(&cli, None);
            match crate::editor_lsp_stdio(&config) {
                Ok(()) => 0,
                Err(error) => {
                    let output = FrontendOutput::new(FrontendOutputConfig::default());
                    match output.render_error(&error) {
                        Ok(rendered) => {
                            let _ = writeln!(stderr, "{rendered}");
                        }
                        Err(render_error) => {
                            let _ = writeln!(stderr, "FrontendInternal: {render_error}");
                        }
                    }
                    1
                }
            }
        }
        Ok(cli)
            if matches!(
                cli.command.as_ref(),
                Some(FrontendCommand::Code(command))
                    if matches!(command.command, CodeSubcommand::Explain(_))
            ) =>
        {
            let config = crate::frontend_config_from_cli(&cli, None);
            // JSON/plain output must stay free of ANSI escapes even on a TTY.
            if !matches!(config.output.mode, crate::OutputMode::Human) {
                crate::ansi::set_enabled(false);
            }
            let code = match cli.command.as_ref() {
                Some(FrontendCommand::Code(command)) => match &command.command {
                    CodeSubcommand::Explain(explain) => explain.code.clone(),
                    _ => unreachable!("guarded to code explain above"),
                },
                _ => unreachable!("guarded to code explain above"),
            };
            let rendering = crate::render_explain(&code, config.output.mode);
            match writeln!(stdout, "{}", rendering.text.trim_end()) {
                Ok(()) => {
                    if rendering.known {
                        0
                    } else {
                        1
                    }
                }
                Err(error) => {
                    let _ = writeln!(stderr, "FrontendInternal: {error}");
                    1
                }
            }
        }
        Ok(cli) => {
            let config = crate::frontend_config_from_cli(&cli, None);
            let output = FrontendOutput::new(config.output);
            match dispatch_cli(&cli, &config) {
                Ok(result) => match output.render_command_summary(&result) {
                    Ok(rendered) => match writeln!(stdout, "{rendered}") {
                        Ok(()) => 0,
                        Err(error) => {
                            let _ = writeln!(stderr, "FrontendInternal: {error}");
                            1
                        }
                    },
                    Err(error) => {
                        let _ = writeln!(stderr, "FrontendInternal: {error}");
                        1
                    }
                },
                Err(error) => {
                    match output.render_error(&error) {
                        Ok(rendered) => {
                            let _ = writeln!(stderr, "{rendered}");
                        }
                        Err(render_error) => {
                            let _ = writeln!(stderr, "FrontendInternal: {render_error}");
                        }
                    }
                    1
                }
            }
        }
    }
}

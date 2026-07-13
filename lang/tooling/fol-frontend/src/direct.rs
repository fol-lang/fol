use crate::{
    FrontendArtifactKind, FrontendArtifactSummary, FrontendCommandResult, FrontendConfig,
    FrontendError, FrontendErrorKind, FrontendResult, OutputMode,
};
use fol_backend::{
    emit_backend_artifact, summarize_emitted_artifact, BackendConfig, BackendMode, BackendSession,
};
use fol_diagnostics::{DiagnosticLocation, DiagnosticReport, OutputFormat};
use fol_lower::{render_lowered_workspace, LoweredWorkspace, Lowerer};
use fol_package::{PackageConfig, PackageSession};
use fol_parser::ast::AstParser;
use fol_resolver::ResolverConfig;
use fol_stream::FileStream;
use fol_typecheck::Typechecker;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectCompileConfig {
    pub input: String,
    pub std_root: Option<String>,
    pub package_store_root: Option<String>,
    pub mode: DirectCompileMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectCompileMode {
    Auto {
        dump_lowered: bool,
        emit_rust: bool,
        keep_build_dir: bool,
    },
    Check,
    Build {
        keep_build_dir: bool,
    },
    Run {
        keep_build_dir: bool,
        args: Vec<String>,
    },
    EmitRust {
        keep_build_dir: bool,
    },
    EmitLowered,
}

fn backend_profile_for_direct_compile(
    frontend_config: &FrontendConfig,
) -> fol_backend::BackendBuildProfile {
    match frontend_config
        .profile_override
        .unwrap_or(crate::FrontendProfile::Release)
    {
        crate::FrontendProfile::Debug => fol_backend::BackendBuildProfile::Debug,
        crate::FrontendProfile::Release => fol_backend::BackendBuildProfile::Release,
    }
}

fn ensure_direct_target_runs_on_host(frontend_config: &FrontendConfig) -> FrontendResult<()> {
    if frontend_config.machine_target_runs_on_host() {
        return Ok(());
    }

    let machine_target = frontend_config.backend_machine_target();
    let selected = machine_target
        .rust_target_triple()
        .unwrap_or_else(|| machine_target.display_name().to_string());
    let host = FrontendConfig::host_rust_target_triple().unwrap_or("unknown-host");
    Err(FrontendError::new(
        FrontendErrorKind::InvalidInput,
        format!("run command cannot execute target '{selected}' on host '{host}'"),
    ))
}

pub fn run_direct_compile(
    config: &DirectCompileConfig,
    frontend_config: &FrontendConfig,
) -> FrontendResult<FrontendCommandResult> {
    let mut diagnostics = DiagnosticReport::new();
    let lowered = match compile_file(
        &config.input,
        &ResolverConfig {
            std_root: config.std_root.clone(),
            package_store_root: config.package_store_root.clone(),
        },
        &mut diagnostics,
    ) {
        Ok(lowered) => lowered,
        Err(()) => return Err(FrontendError::from_errors(diagnostics.diagnostics)),
    };

    if diagnostics.has_errors() {
        return Err(FrontendError::from_errors(diagnostics.diagnostics));
    }

    match &config.mode {
        DirectCompileMode::Auto {
            dump_lowered,
            emit_rust,
            keep_build_dir,
        } => {
            let mut result =
                FrontendCommandResult::new("compile", format!("compiled {}", config.input));
            if *dump_lowered {
                let lowered_root = frontend_config
                    .working_directory
                    .join("target")
                    .join("lowered");
                std::fs::create_dir_all(&lowered_root).map_err(|error| {
                    FrontendError::new(
                        FrontendErrorKind::CommandFailed,
                        format!(
                            "failed to create lowered output root '{}': {error}",
                            lowered_root.display()
                        ),
                    )
                })?;
                let stem = Path::new(&config.input)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("input");
                let snapshot_path = lowered_root.join(format!("{stem}.lowered.txt"));
                std::fs::write(&snapshot_path, render_lowered_workspace(&lowered)).map_err(
                    |error| {
                        FrontendError::new(
                            FrontendErrorKind::CommandFailed,
                            format!(
                                "failed to write lowered snapshot '{}': {error}",
                                snapshot_path.display()
                            ),
                        )
                    },
                )?;
                result.artifacts.push(FrontendArtifactSummary::new(
                    FrontendArtifactKind::LoweredSnapshot,
                    "lowered-snapshot",
                    Some(snapshot_path),
                ));
                result.summary = format!("emitted lowered snapshot for {}", config.input);
                return Ok(result);
            }

            if lowered.entry_candidates().is_empty() {
                result.summary = format!("compiled {} without runnable entrypoint", config.input);
                return Ok(result);
            }

            let backend_session = BackendSession::new(lowered);
            let output_root = frontend_config.working_directory.join("target");
            let artifact = emit_backend_artifact(
                &backend_session,
                &BackendConfig {
                    machine_target: frontend_config.backend_machine_target(),
                    build_profile: backend_profile_for_direct_compile(frontend_config),
                    mode: if *emit_rust {
                        BackendMode::EmitSource
                    } else {
                        BackendMode::BuildArtifact
                    },
                    keep_build_dir: *keep_build_dir,
                    ..BackendConfig::default()
                },
                &output_root,
            )
            .map_err(|error| {
                FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string())
            })?;

            result.summary = summarize_emitted_artifact(&artifact);
            match artifact {
                fol_backend::BackendArtifact::RustSourceCrate { root, .. } => {
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::EmittedRust,
                        "emitted-rust",
                        Some(PathBuf::from(root)),
                    ));
                }
                fol_backend::BackendArtifact::CompiledBinary {
                    crate_root,
                    binary_path,
                } => {
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::EmittedRust,
                        "backend-crate",
                        Some(PathBuf::from(crate_root)),
                    ));
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::Binary,
                        "binary",
                        Some(PathBuf::from(binary_path)),
                    ));
                }
            }
            Ok(result)
        }
        DirectCompileMode::Check => {
            let mut result =
                FrontendCommandResult::new("check", format!("checked {}", config.input));
            if lowered.entry_candidates().is_empty() {
                result.summary = format!("checked {} without runnable entrypoint", config.input);
            }
            Ok(result)
        }
        DirectCompileMode::EmitLowered => {
            let lowered_root = frontend_config
                .working_directory
                .join("target")
                .join("lowered");
            std::fs::create_dir_all(&lowered_root).map_err(|error| {
                FrontendError::new(
                    FrontendErrorKind::CommandFailed,
                    format!(
                        "failed to create lowered output root '{}': {error}",
                        lowered_root.display()
                    ),
                )
            })?;
            let stem = Path::new(&config.input)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("input");
            let snapshot_path = lowered_root.join(format!("{stem}.lowered.txt"));
            std::fs::write(&snapshot_path, render_lowered_workspace(&lowered)).map_err(
                |error| {
                    FrontendError::new(
                        FrontendErrorKind::CommandFailed,
                        format!(
                            "failed to write lowered snapshot '{}': {error}",
                            snapshot_path.display()
                        ),
                    )
                },
            )?;
            let mut result = FrontendCommandResult::new(
                "emit lowered",
                format!("emitted lowered snapshot for {}", config.input),
            );
            result.artifacts.push(FrontendArtifactSummary::new(
                FrontendArtifactKind::LoweredSnapshot,
                "lowered-snapshot",
                Some(snapshot_path),
            ));
            Ok(result)
        }
        DirectCompileMode::Build { keep_build_dir }
        | DirectCompileMode::Run { keep_build_dir, .. }
        | DirectCompileMode::EmitRust { keep_build_dir } => {
            if matches!(config.mode, DirectCompileMode::Run { .. }) {
                ensure_direct_target_runs_on_host(frontend_config)?;
            }
            if lowered.entry_candidates().is_empty() {
                return Err(FrontendError::new(
                    FrontendErrorKind::InvalidInput,
                    format!("{} does not contain a runnable entrypoint", config.input),
                ));
            }

            let backend_session = BackendSession::new(lowered);
            let output_root = frontend_config.working_directory.join("target");
            let backend_mode = match config.mode {
                DirectCompileMode::EmitRust { .. } => BackendMode::EmitSource,
                _ => BackendMode::BuildArtifact,
            };
            let artifact = emit_backend_artifact(
                &backend_session,
                &BackendConfig {
                    machine_target: frontend_config.backend_machine_target(),
                    build_profile: backend_profile_for_direct_compile(frontend_config),
                    mode: backend_mode,
                    keep_build_dir: *keep_build_dir,
                    ..BackendConfig::default()
                },
                &output_root,
            )
            .map_err(|error| {
                FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string())
            })?;

            match (&config.mode, artifact) {
                (
                    DirectCompileMode::EmitRust { .. },
                    fol_backend::BackendArtifact::RustSourceCrate { root, .. },
                ) => {
                    let mut result = FrontendCommandResult::new(
                        "emit rust",
                        format!("emitted Rust backend for {}", config.input),
                    );
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::EmittedRust,
                        "emitted-rust",
                        Some(PathBuf::from(root)),
                    ));
                    Ok(result)
                }
                (
                    DirectCompileMode::Build { .. },
                    fol_backend::BackendArtifact::CompiledBinary {
                        crate_root,
                        binary_path,
                    },
                ) => {
                    let mut result = FrontendCommandResult::new(
                        "build",
                        summarize_emitted_artifact(&fol_backend::BackendArtifact::CompiledBinary {
                            crate_root: crate_root.clone(),
                            binary_path: binary_path.clone(),
                        }),
                    );
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::EmittedRust,
                        "backend-crate",
                        Some(PathBuf::from(crate_root)),
                    ));
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::Binary,
                        "binary",
                        Some(PathBuf::from(binary_path)),
                    ));
                    Ok(result)
                }
                (
                    DirectCompileMode::Run { args, .. },
                    fol_backend::BackendArtifact::CompiledBinary {
                        crate_root,
                        binary_path,
                    },
                ) => {
                    let output = std::process::Command::new(&binary_path)
                        .args(args)
                        .output()
                        .map_err(|error| {
                            FrontendError::new(FrontendErrorKind::CommandFailed, error.to_string())
                        })?;
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        if !stderr.is_empty() {
                            eprint!("{stderr}");
                        }
                        return Err(FrontendError::new(
                            FrontendErrorKind::CommandFailed,
                            format!(
                                "run command failed for '{}': status {}",
                                binary_path, output.status
                            ),
                        ));
                    }
                    let mut result =
                        FrontendCommandResult::new("run", format!("ran {}", binary_path));
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::EmittedRust,
                        "backend-crate",
                        Some(PathBuf::from(crate_root)),
                    ));
                    result.artifacts.push(FrontendArtifactSummary::new(
                        FrontendArtifactKind::Binary,
                        "binary",
                        Some(PathBuf::from(binary_path)),
                    ));
                    Ok(result)
                }
                _ => Err(FrontendError::new(
                    FrontendErrorKind::Internal,
                    "direct compile mode received an unexpected backend artifact",
                )),
            }
        }
    }
}

pub fn run_direct_compile_with_io(
    config: &DirectCompileConfig,
    frontend_config: &FrontendConfig,
    stdout: &mut impl std::io::Write,
) -> i32 {
    let resolver_config = ResolverConfig {
        std_root: config.std_root.clone(),
        package_store_root: config.package_store_root.clone(),
    };
    let mut diagnostics = DiagnosticReport::new();

    if frontend_config.output.mode != OutputMode::Json {
        let _ = writeln!(stdout, "=== FOL Compiler (Modular) ===");
        let _ = writeln!(stdout, "Compiling: {}", config.input);
    }

    match compile_file(&config.input, &resolver_config, &mut diagnostics) {
        Ok(lowered) => {
            if matches!(
                config.mode,
                DirectCompileMode::EmitLowered
                    | DirectCompileMode::Auto {
                        dump_lowered: true,
                        ..
                    }
            ) && frontend_config.output.mode != OutputMode::Json
                && !diagnostics.has_errors()
            {
                let _ = writeln!(stdout, "{}", render_lowered_workspace(&lowered));
            }
            if !diagnostics.has_errors() {
                if lowered.entry_candidates().is_empty()
                    && !matches!(
                        config.mode,
                        DirectCompileMode::Check | DirectCompileMode::EmitLowered
                    )
                {
                    if matches!(config.mode, DirectCompileMode::Auto { .. }) {
                        if frontend_config.output.mode != OutputMode::Json {
                            let _ = writeln!(stdout, "✓ Compilation successful!");
                        }
                    } else {
                        diagnostics.add_error(
                            format!(
                                "{} does not contain a runnable entrypoint",
                                config.input
                            ),
                            None,
                        );
                    }
                } else if matches!(
                    config.mode,
                    DirectCompileMode::Auto {
                        dump_lowered: true,
                        ..
                    }
                ) {
                    if frontend_config.output.mode != OutputMode::Json {
                        let _ = writeln!(stdout, "✓ Emitted lowered snapshot!");
                    }
                } else if !matches!(
                    config.mode,
                    DirectCompileMode::Check | DirectCompileMode::EmitLowered
                ) {
                    match matches!(config.mode, DirectCompileMode::Run { .. }) {
                        true => match ensure_direct_target_runs_on_host(frontend_config) {
                            Ok(()) => {
                                let backend_session = BackendSession::new(lowered);
                                let output_root = frontend_config.working_directory.join("target");
                                match emit_backend_artifact(
                                    &backend_session,
                                    &BackendConfig {
                                        machine_target: frontend_config.backend_machine_target(),
                                        build_profile: backend_profile_for_direct_compile(
                                            frontend_config,
                                        ),
                                        mode: match config.mode {
                                            DirectCompileMode::Auto {
                                                emit_rust: true, ..
                                            } => BackendMode::EmitSource,
                                            DirectCompileMode::EmitRust { .. } => {
                                                BackendMode::EmitSource
                                            }
                                            _ => BackendMode::BuildArtifact,
                                        },
                                        keep_build_dir: match &config.mode {
                                            DirectCompileMode::Auto {
                                                keep_build_dir, ..
                                            } => *keep_build_dir,
                                            DirectCompileMode::Build { keep_build_dir }
                                            | DirectCompileMode::Run { keep_build_dir, .. }
                                            | DirectCompileMode::EmitRust { keep_build_dir } => {
                                                *keep_build_dir
                                            }
                                            _ => false,
                                        },
                                        ..BackendConfig::default()
                                    },
                                    &output_root,
                                ) {
                                    Ok(artifact) => {
                                        if frontend_config.output.mode != OutputMode::Json {
                                            let _ = writeln!(
                                                stdout,
                                                "{}",
                                                summarize_emitted_artifact(&artifact)
                                            );
                                            let _ = writeln!(stdout, "✓ Compilation successful!");
                                        }
                                    }
                                    Err(error) => {
                                        diagnostics.add_error(error.to_string(), None);
                                    }
                                }
                            }
                            Err(error) => diagnostics.add_error(error.to_string(), None),
                        },
                        false => {
                            let backend_session = BackendSession::new(lowered);
                            let output_root = frontend_config.working_directory.join("target");
                            match emit_backend_artifact(
                                &backend_session,
                                &BackendConfig {
                                    machine_target: frontend_config.backend_machine_target(),
                                    build_profile: backend_profile_for_direct_compile(
                                        frontend_config,
                                    ),
                                    mode: match config.mode {
                                        DirectCompileMode::Auto {
                                            emit_rust: true, ..
                                        } => BackendMode::EmitSource,
                                        DirectCompileMode::EmitRust { .. } => BackendMode::EmitSource,
                                        _ => BackendMode::BuildArtifact,
                                    },
                                    keep_build_dir: match &config.mode {
                                        DirectCompileMode::Auto { keep_build_dir, .. } => {
                                            *keep_build_dir
                                        }
                                        DirectCompileMode::Build { keep_build_dir }
                                        | DirectCompileMode::Run { keep_build_dir, .. }
                                        | DirectCompileMode::EmitRust { keep_build_dir } => {
                                            *keep_build_dir
                                        }
                                        _ => false,
                                    },
                                    ..BackendConfig::default()
                                },
                                &output_root,
                            ) {
                                Ok(artifact) => {
                                    if frontend_config.output.mode != OutputMode::Json {
                                        let _ =
                                            writeln!(stdout, "{}", summarize_emitted_artifact(&artifact));
                                        let _ = writeln!(stdout, "✓ Compilation successful!");
                                    }
                                }
                                Err(error) => {
                                    diagnostics.add_error(error.to_string(), None);
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {}
    }

    let rendered = match frontend_config.output.mode {
        OutputMode::Human => crate::pretty::render_report_pretty(&diagnostics),
        OutputMode::Plain => diagnostics.output(OutputFormat::Human),
        OutputMode::Json => diagnostics.output(OutputFormat::Json),
    };
    if !rendered.trim().is_empty() {
        let _ = writeln!(stdout, "{rendered}");
    }

    if diagnostics.has_errors() {
        1
    } else {
        0
    }
}

fn compile_file(
    file_path: &str,
    resolver_config: &ResolverConfig,
    diagnostics: &mut DiagnosticReport,
) -> Result<LoweredWorkspace, ()> {
    let path = Path::new(file_path);
    if !path.exists() {
        diagnostics.add_error(format!("File not found: {}", file_path), None);
        return Err(());
    }

    let mut file_stream = if path.is_dir() {
        FileStream::from_folder(file_path).map_err(|e| {
            diagnostics.add_error(
                e.to_string(),
                Some(DiagnosticLocation {
                    file: Some(file_path.to_string()),
                    line: 1,
                    column: 1,
                    length: None,
                }),
            );
        })?
    } else {
        FileStream::from_file(file_path).map_err(|e| {
            diagnostics.add_error(
                e.to_string(),
                Some(DiagnosticLocation {
                    file: Some(file_path.to_string()),
                    line: 1,
                    column: 1,
                    length: None,
                }),
            );
        })?
    };

    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut file_stream);
    let mut ast_parser = AstParser::new();
    match ast_parser.parse_package(&mut lexer) {
        Ok(package) => {
            let package_session = PackageSession::with_config(PackageConfig {
                std_root: resolver_config.std_root.clone(),
                package_store_root: resolver_config.package_store_root.clone(),
                package_cache_root: None,
                package_git_cache_root: None,
            });
            let prepared = match package_session.prepare_entry_package(package) {
                Ok(prepared) => prepared,
                Err(error) => {
                    diagnostics.add_from(&error);
                    return Err(());
                }
            };
            match fol_resolver::resolve_prepared_workspace_with_config(
                prepared,
                resolver_config.clone(),
            ) {
                Ok(resolved) => match Typechecker::new().check_resolved_workspace(resolved) {
                    Ok(typed) => match Lowerer::new().lower_typed_workspace(typed) {
                        Ok(lowered) => Ok(lowered),
                        Err(errors) => {
                            for error in errors {
                                diagnostics.add_from(&error);
                            }
                            Err(())
                        }
                    },
                    Err(errors) => {
                        for error in errors {
                            diagnostics.add_from(&error);
                        }
                        Err(())
                    }
                },
                Err(errors) => {
                    for error in errors {
                        diagnostics.add_from(&error);
                    }
                    Err(())
                }
            }
        }
        Err(parser_diagnostics) => {
            for diagnostic in parser_diagnostics {
                diagnostics.add_diagnostic(diagnostic);
            }
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{run_direct_compile, DirectCompileConfig, DirectCompileMode};
    use crate::FrontendConfig;
    use std::fs;

    fn non_host_machine_target() -> String {
        if FrontendConfig::host_rust_target_triple() == Some("aarch64-apple-darwin") {
            "x86_64-unknown-linux-gnu".to_string()
        } else {
            "aarch64-apple-darwin".to_string()
        }
    }

    #[test]
    fn run_direct_compile_rejects_non_host_machine_targets() {
        let root = std::env::temp_dir().join(format!("fol_direct_cross_run_{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("main.fol");
        fs::write(&input, "fun[] main(): int = {\n    return 0\n};\n").unwrap();

        let config = DirectCompileConfig {
            input: input.display().to_string(),
            std_root: None,
            package_store_root: None,
            mode: DirectCompileMode::Run {
                keep_build_dir: false,
                args: Vec::new(),
            },
        };
        let frontend_config = FrontendConfig {
            build_target_override: Some(non_host_machine_target()),
            ..FrontendConfig::default()
        };

        let error = run_direct_compile(&config, &frontend_config).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("run command cannot execute target"),
            "{}",
            error
        );

        fs::remove_dir_all(root).ok();
    }
}

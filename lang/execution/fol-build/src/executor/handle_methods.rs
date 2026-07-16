use crate::api::{DependencyRequest, PathHandleClass, PathHandleProvenance};
use crate::eval::{
    BuildEvaluationError, BuildEvaluationOperation, BuildEvaluationOperationKind,
    BuildEvaluationRunArgKind,
};
use crate::runtime::{
    BuildRuntimeDependencyExport, BuildRuntimeDependencyExportKind, BuildRuntimeDependencyQuery,
    BuildRuntimeDependencyQueryKind, BuildRuntimeGeneratedFileKind,
};
use fol_parser::ast::AstNode;

use super::core::BuildBodyExecutor;
use super::types::ExecValue;

impl BuildBodyExecutor {
    fn invalid_config(&self, method: &str, detail: impl Into<String>) -> BuildEvaluationError {
        BuildEvaluationError::new(
            crate::eval::BuildEvaluationErrorKind::InvalidInput,
            format!("{method} config is invalid: {}", detail.into()),
        )
    }

    fn validate_export_name(&self, method: &str, name: &str) -> Result<(), BuildEvaluationError> {
        if !super::core::is_valid_identifier(name) {
            return Err(self.invalid_config(
                method,
                format!("export name '{}' must match [a-z][a-z0-9_-]*", name),
            ));
        }
        Ok(())
    }

    fn record_dependency_export(
        &mut self,
        method: &str,
        export: BuildRuntimeDependencyExport,
    ) -> Result<(), BuildEvaluationError> {
        if self
            .output
            .dependency_exports
            .iter()
            .any(|existing| existing.kind == export.kind && existing.name == export.name)
        {
            let kind = match export.kind {
                BuildRuntimeDependencyExportKind::Module => "module",
                BuildRuntimeDependencyExportKind::Artifact => "artifact",
                BuildRuntimeDependencyExportKind::Step => "step",
                BuildRuntimeDependencyExportKind::File => "file",
                BuildRuntimeDependencyExportKind::Dir => "dir",
                BuildRuntimeDependencyExportKind::Path => "path",
                BuildRuntimeDependencyExportKind::GeneratedOutput => "output",
            };
            return Err(self.invalid_config(
                method,
                format!("duplicate exported {kind} name '{}'", export.name),
            ));
        }
        self.output.dependency_exports.push(export);
        Ok(())
    }

    pub(super) fn eval_handle_method(
        &mut self,
        receiver: ExecValue,
        method: &str,
        args: &[AstNode],
    ) -> Result<Option<ExecValue>, BuildEvaluationError> {
        match &receiver {
            ExecValue::Build if method == "meta" => {
                let [AstNode::RecordInit { fields, .. }] = args else {
                    return Err(self.unsupported(method));
                };
                self.resolve_field_string(fields, "name").ok_or_else(|| {
                    self.invalid_config(method, "build.meta requires string field 'name'")
                })?;
                self.resolve_field_string(fields, "version")
                    .ok_or_else(|| {
                        self.invalid_config(method, "build.meta requires string field 'version'")
                    })?;
                Ok(Some(receiver))
            }
            ExecValue::Build if method == "add_dep" => {
                let [AstNode::RecordInit { fields, .. }] = args else {
                    return Err(self.unsupported(method));
                };
                let alias = self.resolve_field_string(fields, "alias").ok_or_else(|| {
                    self.invalid_config(method, "build.add_dep requires string field 'alias'")
                })?;
                if !super::core::is_valid_identifier(&alias) {
                    return Err(self.invalid_config(
                        method,
                        format!("dependency alias '{}' must match [a-z][a-z0-9_-]*", alias),
                    ));
                }
                let source = self.resolve_field_string(fields, "source").ok_or_else(|| {
                    self.invalid_config(method, "build.add_dep requires string field 'source'")
                })?;
                if !matches!(source.as_str(), "loc" | "pkg" | "git" | "internal") {
                    return Err(self.invalid_config(
                        method,
                        format!(
                            "dependency source must be one of: loc, pkg, git, internal (got '{}')",
                            source
                        ),
                    ));
                }
                let package = self.resolve_field_string(fields, "target").ok_or_else(|| {
                    self.invalid_config(method, "build.add_dep requires string field 'target'")
                })?;
                if package.trim().is_empty() {
                    return Err(
                        self.invalid_config(method, "dependency 'target' must not be empty")
                    );
                }
                if source == "internal" && package != "standard" {
                    return Err(self.invalid_config(
                        method,
                        format!(
                            "dependency source 'internal' currently requires target = 'standard' (got '{}')",
                            package
                        ),
                    ));
                }
                let git_version = match self.resolve_field_string(fields, "version") {
                    Some(version) => {
                        if source != "git" {
                            return Err(self.invalid_config(
                                method,
                                "dependency field 'version' is only valid for git dependencies",
                            ));
                        }
                        Some(
                            crate::GitDependencyVersionSelector::parse(version.as_str()).ok_or_else(
                                || {
                                    self.invalid_config(
                                        method,
                                        format!(
                                            "dependency 'version' must be one of: branch:<name>, tag:<name>, commit:<sha> (got '{}')",
                                            version
                                        ),
                                    )
                                },
                            )?,
                        )
                    }
                    None => None,
                };
                let git_hash = match self.resolve_field_string(fields, "hash") {
                    Some(hash) => {
                        if source != "git" {
                            return Err(self.invalid_config(
                                method,
                                "dependency field 'hash' is only valid for git dependencies",
                            ));
                        }
                        if hash.trim().is_empty() {
                            return Err(
                                self.invalid_config(method, "dependency 'hash' must not be empty")
                            );
                        }
                        Some(hash)
                    }
                    None => None,
                };
                let evaluation_mode = match self.resolve_field_string(fields, "mode") {
                    Some(mode) => crate::DependencyBuildEvaluationMode::parse(mode.as_str())
                        .ok_or_else(|| {
                            self.invalid_config(
                                method,
                                format!(
                                    "dependency mode must be one of: eager, lazy, on-demand (got '{}')",
                                    mode
                                ),
                            )
                        })?,
                    None => crate::DependencyBuildEvaluationMode::Eager,
                };
                let args = self.resolve_dependency_args(fields)?.unwrap_or_default();
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::Dependency(DependencyRequest {
                        alias: alias.clone(),
                        source_kind: match source.as_str() {
                            "loc" => crate::api::DependencySourceKind::Local,
                            "pkg" => crate::api::DependencySourceKind::PackageStore,
                            "git" => crate::api::DependencySourceKind::Git,
                            "internal" => crate::api::DependencySourceKind::Internal,
                            _ => unreachable!("validated dependency source"),
                        },
                        package,
                        args,
                        evaluation_mode: Some(evaluation_mode),
                        git_version,
                        git_hash,
                        surface: None,
                    }),
                });
                Ok(Some(ExecValue::Dependency { alias }))
            }
            ExecValue::Build
                if matches!(
                    method,
                    "export_module"
                        | "export_artifact"
                        | "export_step"
                        | "export_file"
                        | "export_dir"
                        | "export_path"
                        | "export_output"
                ) =>
            {
                let [AstNode::RecordInit { fields, .. }] = args else {
                    return Err(self.unsupported(method));
                };
                let export_name = self.resolve_field_string(fields, "name").ok_or_else(|| {
                    self.invalid_config(method, "export requires string field 'name'")
                })?;
                self.validate_export_name(method, &export_name)?;
                let export = match method {
                    "export_module" => {
                        let target_name = match fields.iter().find(|field| field.name == "module") {
                            Some(field) => match &field.value {
                                AstNode::Identifier { name, .. } => {
                                    match self.scope.get(name.as_str()) {
                                        Some(ExecValue::Module { name }) => name.clone(),
                                        _ => return Err(self.invalid_config(
                                            method,
                                            "build.export_module requires handle field 'module'",
                                        )),
                                    }
                                }
                                _ => {
                                    return Err(self.invalid_config(
                                        method,
                                        "build.export_module requires handle field 'module'",
                                    ))
                                }
                            },
                            None => {
                                return Err(self.invalid_config(
                                    method,
                                    "build.export_module requires handle field 'module'",
                                ))
                            }
                        };
                        BuildRuntimeDependencyExport {
                            name: export_name,
                            target_name,
                            kind: BuildRuntimeDependencyExportKind::Module,
                        }
                    }
                    "export_artifact" => {
                        let target_name = match fields.iter().find(|field| field.name == "artifact")
                        {
                            Some(field) => match &field.value {
                                AstNode::Identifier { name, .. } => match self
                                    .scope
                                    .get(name.as_str())
                                {
                                    Some(ExecValue::Artifact(artifact)) => artifact.name.clone(),
                                    _ => return Err(self.invalid_config(
                                        method,
                                        "build.export_artifact requires handle field 'artifact'",
                                    )),
                                },
                                _ => {
                                    return Err(self.invalid_config(
                                        method,
                                        "build.export_artifact requires handle field 'artifact'",
                                    ))
                                }
                            },
                            None => {
                                return Err(self.invalid_config(
                                    method,
                                    "build.export_artifact requires handle field 'artifact'",
                                ))
                            }
                        };
                        BuildRuntimeDependencyExport {
                            name: export_name,
                            target_name,
                            kind: BuildRuntimeDependencyExportKind::Artifact,
                        }
                    }
                    "export_step" => {
                        let target_name =
                            match fields.iter().find(|field| field.name == "step") {
                                Some(field) => match &field.value {
                                    AstNode::Identifier { name, .. } => {
                                        match self.scope.get(name.as_str()) {
                                            Some(ExecValue::Step { name })
                                            | Some(ExecValue::Run { name })
                                            | Some(ExecValue::Install { name }) => name.clone(),
                                            _ => return Err(self.invalid_config(
                                                method,
                                                "build.export_step requires handle field 'step'",
                                            )),
                                        }
                                    }
                                    _ => {
                                        return Err(self.invalid_config(
                                            method,
                                            "build.export_step requires handle field 'step'",
                                        ))
                                    }
                                },
                                None => {
                                    return Err(self.invalid_config(
                                        method,
                                        "build.export_step requires handle field 'step'",
                                    ))
                                }
                            };
                        BuildRuntimeDependencyExport {
                            name: export_name,
                            target_name,
                            kind: BuildRuntimeDependencyExportKind::Step,
                        }
                    }
                    "export_file" => {
                        let target_name =
                            match fields.iter().find(|field| field.name == "file") {
                                Some(field) => match &field.value {
                                    AstNode::Identifier { name, .. } => {
                                        match self.scope.get(name.as_str()) {
                                            Some(ExecValue::SourceFile { path }) => path.clone(),
                                            _ => return Err(self.invalid_config(
                                                method,
                                                "build.export_file requires handle field 'file'",
                                            )),
                                        }
                                    }
                                    _ => {
                                        return Err(self.invalid_config(
                                            method,
                                            "build.export_file requires handle field 'file'",
                                        ))
                                    }
                                },
                                None => {
                                    return Err(self.invalid_config(
                                        method,
                                        "build.export_file requires handle field 'file'",
                                    ))
                                }
                            };
                        BuildRuntimeDependencyExport {
                            name: export_name,
                            target_name,
                            kind: BuildRuntimeDependencyExportKind::File,
                        }
                    }
                    "export_dir" => {
                        let target_name = match fields.iter().find(|field| field.name == "dir") {
                            Some(field) => match &field.value {
                                AstNode::Identifier { name, .. } => {
                                    match self.scope.get(name.as_str()) {
                                        Some(ExecValue::SourceDir { path }) => path.clone(),
                                        Some(ExecValue::GeneratedFile {
                                            path,
                                            kind: BuildRuntimeGeneratedFileKind::GeneratedDir,
                                            ..
                                        }) => path.clone(),
                                        _ => {
                                            return Err(self.invalid_config(
                                                method,
                                                "build.export_dir requires handle field 'dir'",
                                            ))
                                        }
                                    }
                                }
                                _ => {
                                    return Err(self.invalid_config(
                                        method,
                                        "build.export_dir requires handle field 'dir'",
                                    ))
                                }
                            },
                            None => {
                                return Err(self.invalid_config(
                                    method,
                                    "build.export_dir requires handle field 'dir'",
                                ))
                            }
                        };
                        BuildRuntimeDependencyExport {
                            name: export_name,
                            target_name,
                            kind: BuildRuntimeDependencyExportKind::Dir,
                        }
                    }
                    "export_path" => {
                        let target_name = match fields.iter().find(|field| field.name == "path") {
                            Some(field) => match &field.value {
                                AstNode::Identifier { name, .. } => {
                                    match self.scope.get(name.as_str()) {
                                        Some(ExecValue::GeneratedFile { name, .. }) => name.clone(),
                                        _ => {
                                            return Err(self.invalid_config(
                                                method,
                                                "build.export_path requires handle field 'path'",
                                            ))
                                        }
                                    }
                                }
                                _ => {
                                    return Err(self.invalid_config(
                                        method,
                                        "build.export_path requires handle field 'path'",
                                    ))
                                }
                            },
                            None => {
                                return Err(self.invalid_config(
                                    method,
                                    "build.export_path requires handle field 'path'",
                                ))
                            }
                        };
                        BuildRuntimeDependencyExport {
                            name: export_name,
                            target_name,
                            kind: BuildRuntimeDependencyExportKind::Path,
                        }
                    }
                    "export_output" => {
                        let target_name = match fields.iter().find(|field| field.name == "output") {
                            Some(field) => match &field.value {
                                AstNode::Identifier { name, .. } => {
                                    match self.scope.get(name.as_str()) {
                                        Some(ExecValue::GeneratedFile { name, .. }) => name.clone(),
                                        _ => return Err(self.invalid_config(
                                            method,
                                            "build.export_output requires handle field 'output'",
                                        )),
                                    }
                                }
                                _ => {
                                    return Err(self.invalid_config(
                                        method,
                                        "build.export_output requires handle field 'output'",
                                    ))
                                }
                            },
                            None => {
                                return Err(self.invalid_config(
                                    method,
                                    "build.export_output requires handle field 'output'",
                                ))
                            }
                        };
                        BuildRuntimeDependencyExport {
                            name: export_name,
                            target_name,
                            kind: BuildRuntimeDependencyExportKind::GeneratedOutput,
                        }
                    }
                    _ => return Err(self.unsupported(method)),
                };
                self.record_dependency_export(method, export)?;
                Ok(Some(receiver))
            }
            ExecValue::Build if method == "graph" => {
                let [] = args else {
                    return Err(self.unsupported(method));
                };
                Ok(Some(ExecValue::Graph))
            }
            ExecValue::Build => Err(self.unsupported(method)),
            ExecValue::Dependency { alias }
                if matches!(
                    method,
                    "module" | "artifact" | "step" | "file" | "dir" | "path" | "generated"
                ) =>
            {
                let alias = alias.clone();
                let [name_arg] = args else {
                    return Err(self.unsupported(method));
                };
                let query_name = self
                    .resolve_string(name_arg)
                    .ok_or_else(|| self.unsupported(method))?;
                let kind = match method {
                    "module" => BuildRuntimeDependencyQueryKind::Module,
                    "artifact" => BuildRuntimeDependencyQueryKind::Artifact,
                    "step" => BuildRuntimeDependencyQueryKind::Step,
                    "file" => BuildRuntimeDependencyQueryKind::File,
                    "dir" => BuildRuntimeDependencyQueryKind::Dir,
                    "path" => BuildRuntimeDependencyQueryKind::Path,
                    "generated" => BuildRuntimeDependencyQueryKind::GeneratedOutput,
                    _ => return Err(self.unsupported(method)),
                };
                self.output
                    .dependency_queries
                    .push(BuildRuntimeDependencyQuery {
                        dependency_alias: alias.clone(),
                        query_name: query_name.clone(),
                        kind,
                    });
                let result = match method {
                    "module" => ExecValue::DependencyModule { alias, query_name },
                    "artifact" => ExecValue::DependencyArtifact { alias, query_name },
                    "step" => ExecValue::DependencyStep { alias, query_name },
                    "file" => ExecValue::SourceFile {
                        path: format!("$dep/{alias}/{query_name}"),
                    },
                    "dir" => ExecValue::SourceDir {
                        path: format!("$dep/{alias}/{query_name}"),
                    },
                    "path" => ExecValue::GeneratedFile {
                        name: format!("dep::{alias}::path::{query_name}"),
                        path: format!("$dep/{alias}/{query_name}"),
                        kind: BuildRuntimeGeneratedFileKind::ToolOutput,
                    },
                    "generated" => ExecValue::GeneratedFile {
                        name: format!("dep::{alias}::generated::{query_name}"),
                        path: format!("$dep/{alias}/{query_name}"),
                        kind: BuildRuntimeGeneratedFileKind::ToolOutput,
                    },
                    _ => return Err(self.unsupported(method)),
                };
                Ok(Some(result))
            }

            ExecValue::Step { name } | ExecValue::Run { name } | ExecValue::Install { name }
                if method == "depend_on" =>
            {
                let step_name = name.clone();
                let receiver_clone = receiver.clone();
                let depends_on = args
                    .iter()
                    .filter_map(|a| self.resolve_step_ref(a))
                    .collect::<Vec<_>>();
                if depends_on.is_empty() || depends_on.len() != args.len() {
                    return Err(self.unsupported(method));
                }
                self.append_step_dependencies(&step_name, &depends_on)?;
                Ok(Some(receiver_clone))
            }

            // Artifact handle methods
            ExecValue::Artifact(artifact) if method == "link" => {
                let artifact_name = artifact.name.clone();
                let [arg] = args else {
                    return Err(self.unsupported(method));
                };
                match arg {
                    AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                        Some(ExecValue::Artifact(a)) => {
                            self.output.operations.push(BuildEvaluationOperation {
                                origin: None,
                                kind: BuildEvaluationOperationKind::ArtifactLink {
                                    artifact: artifact_name,
                                    linked: a.name.clone(),
                                },
                            });
                        }
                        Some(ExecValue::SystemLibrary { request }) => {
                            self.output.operations.push(BuildEvaluationOperation {
                                origin: None,
                                kind: BuildEvaluationOperationKind::ArtifactLinkSystemLibrary {
                                    artifact: artifact_name,
                                    request: request.clone(),
                                },
                            });
                        }
                        _ => return Err(self.unsupported(method)),
                    },
                    _ => return Err(self.unsupported(method)),
                };
                Ok(Some(receiver))
            }

            ExecValue::Artifact(artifact) if method == "import" => {
                let artifact_name = artifact.name.clone();
                let [arg] = args else {
                    return Err(self.unsupported(method));
                };
                let module_name = match arg {
                    AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                        Some(ExecValue::Module { name }) => name.clone(),
                        Some(ExecValue::DependencyModule { alias, query_name }) => {
                            format!("dep::{alias}::module::{query_name}")
                        }
                        _ => return Err(self.unsupported(method)),
                    },
                    _ => return Err(self.unsupported(method)),
                };
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::ArtifactImport {
                        artifact: artifact_name,
                        module_name,
                    },
                });
                Ok(Some(receiver))
            }

            ExecValue::Artifact(artifact) if method == "add_generated" => {
                let artifact_name = artifact.name.clone();
                let [arg] = args else {
                    return Err(self.unsupported(method));
                };
                let resolved = self.resolve_path_handle(arg).ok_or_else(|| {
                    self.invalid_config(
                        method,
                        "artifact.add_generated requires a local generated-output handle",
                    )
                })?;
                let generated_name = match (
                    resolved.descriptor.class,
                    resolved.descriptor.provenance,
                    resolved.generated_name,
                ) {
                    (PathHandleClass::File, PathHandleProvenance::Generated, Some(name)) => name,
                    (PathHandleClass::File, PathHandleProvenance::Source, _) => {
                        return Err(self.invalid_config(
                            method,
                            "artifact.add_generated requires a local generated-output handle, not a source-file handle",
                        ))
                    }
                    (PathHandleClass::Dir, PathHandleProvenance::Source, _) => {
                        return Err(self.invalid_config(
                            method,
                            "artifact.add_generated requires a local generated-output handle, not a source-dir handle",
                        ))
                    }
                    (
                        PathHandleClass::File,
                        PathHandleProvenance::DependencyGenerated
                            | PathHandleProvenance::DependencyPath,
                        _,
                    ) => {
                        return Err(self.invalid_config(
                            method,
                            "artifact.add_generated requires a local generated-output handle, not a dependency path handle",
                        ))
                    }
                    _ => {
                        return Err(self.invalid_config(
                            method,
                            "artifact.add_generated requires a local generated-output handle",
                        ))
                    }
                };
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::ArtifactAddGenerated {
                        artifact: artifact_name,
                        generated_name,
                    },
                });
                Ok(Some(receiver))
            }

            ExecValue::Artifact { .. } => Err(self.unsupported(method)),

            // Run handle methods
            ExecValue::Run { name } if matches!(method, "add_arg" | "add_dir_arg") => {
                let run_name = name.clone();
                let [arg] = args else {
                    return Err(self.unsupported(method));
                };
                let value = self
                    .resolve_string(arg)
                    .ok_or_else(|| self.unsupported(method))?;
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::RunAddArg {
                        run_name,
                        kind: BuildEvaluationRunArgKind::Literal,
                        value,
                    },
                });
                Ok(Some(receiver))
            }

            ExecValue::Run { name } if method == "add_file_arg" => {
                let run_name = name.clone();
                let [arg] = args else {
                    return Err(self.unsupported(method));
                };
                let resolved = self.resolve_path_handle(arg).ok_or_else(|| {
                    self.invalid_config(
                        method,
                        "run.add_file_arg requires a source-file handle or generated-output handle",
                    )
                })?;
                let (kind, value) = match (resolved.descriptor.class, resolved.generated_name) {
                    (PathHandleClass::File, Some(generated_name)) => {
                        (BuildEvaluationRunArgKind::GeneratedFile, generated_name)
                    }
                    (PathHandleClass::File, None) => (
                        BuildEvaluationRunArgKind::Path,
                        resolved.descriptor.relative_path.clone(),
                    ),
                    (PathHandleClass::Dir, _) => {
                        return Err(self.invalid_config(
                            method,
                            "run.add_file_arg requires a source-file handle or generated-output handle, not a source-dir handle",
                        ))
                    }
                };
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::RunAddArg {
                        run_name,
                        kind,
                        value,
                    },
                });
                Ok(Some(receiver))
            }

            ExecValue::Run { name } if method == "capture_stdout" => {
                let run_name = name.clone();
                let output_name = format!("{run_name}-stdout");
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::RunCapture {
                        run_name,
                        output_name: output_name.clone(),
                    },
                });
                Ok(Some(ExecValue::GeneratedFile {
                    name: output_name.clone(),
                    path: output_name,
                    kind: BuildRuntimeGeneratedFileKind::ToolOutput,
                }))
            }

            ExecValue::Run { name } if method == "set_env" => {
                let run_name = name.clone();
                let (key, value) = match args {
                    [key_arg, val_arg] => {
                        let k = self
                            .resolve_string(key_arg)
                            .ok_or_else(|| self.unsupported(method))?;
                        let v = self
                            .resolve_string(val_arg)
                            .ok_or_else(|| self.unsupported(method))?;
                        (k, v)
                    }
                    _ => return Err(self.unsupported(method)),
                };
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::RunSetEnv {
                        run_name,
                        key,
                        value,
                    },
                });
                Ok(Some(receiver))
            }

            ExecValue::Run { .. } => Err(self.unsupported(method)),

            // Step handle method: attach
            ExecValue::Step { name } if method == "attach" => {
                let step_name = name.clone();
                let [arg] = args else {
                    return Err(self.unsupported(method));
                };
                let generated_name = match arg {
                    AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                        Some(ExecValue::GeneratedFile { name, .. }) => name.clone(),
                        _ => return Err(self.unsupported(method)),
                    },
                    _ => return Err(self.unsupported(method)),
                };
                self.output.operations.push(BuildEvaluationOperation {
                    origin: None,
                    kind: BuildEvaluationOperationKind::StepAttach {
                        step_name,
                        generated_name,
                    },
                });
                Ok(Some(receiver))
            }

            ExecValue::Step { .. } | ExecValue::Install { .. } | ExecValue::Dependency { .. } => {
                Err(self.unsupported(method))
            }

            _ => Ok(None),
        }
    }
}

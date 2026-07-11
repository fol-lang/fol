pub use fol_build::dependency::*;

use crate::{
    BuildEvaluationInputs, BuildEvaluationOperationKind, BuildEvaluationRequest, PackageError,
    PackageErrorKind, PreparedExportMount,
};
use fol_build::executor::BuildBodyExecutor;
use fol_parser::ast::{ParsedPackage, ParsedSourceUnitKind};
use std::path::Path;

pub fn dependency_modules_from_exports(
    alias: &str,
    exports: &[PreparedExportMount],
) -> Vec<DependencyModuleSurface> {
    exports
        .iter()
        .map(|export| DependencyModuleSurface {
            name: export
                .mounted_namespace_suffix
                .as_deref()
                .unwrap_or(alias)
                .to_string(),
            source_namespace: export.source_namespace.clone(),
        })
        .collect()
}

pub fn project_dependency_surface(
    alias: &str,
    package_root: &Path,
    syntax: &ParsedPackage,
) -> Result<DependencyBuildSurface, PackageError> {
    let build_path = package_root.join("build.fol");
    let (executor, body) = BuildBodyExecutor::from_file(&build_path)
        .map_err(|error| {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package dependency surface projection could not parse build '{}': {}",
                    build_path.display(),
                    error
                ),
            )
        })?
        .ok_or_else(|| {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package dependency surface projection requires canonical build entry in '{}'",
                    build_path.display()
                ),
            )
        })?;
    let exec_output = executor.execute(&body).map_err(|error| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package dependency surface projection could not evaluate build '{}': {}",
                build_path.display(),
                error
            ),
        )
    })?;
    let plan = fol_build::evaluate_build_plan(&BuildEvaluationRequest {
        package_root: package_root.display().to_string(),
        inputs: BuildEvaluationInputs {
            working_directory: package_root.display().to_string(),
            ..BuildEvaluationInputs::default()
        },
        operations: exec_output.operations.clone(),
    })
    .map_err(|error| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package dependency surface projection could not plan build '{}': {}",
                build_path.display(),
                error
            ),
        )
    })?;

    let mut source_roots = syntax
        .source_units
        .iter()
        .filter(|unit| unit.kind == ParsedSourceUnitKind::Ordinary)
        .map(|unit| {
            let relative_path = std::path::Path::new(&unit.path)
                .parent()
                .map(|parent| {
                    let rendered = parent.to_string_lossy().to_string();
                    if rendered.is_empty() {
                        ".".to_string()
                    } else {
                        rendered
                    }
                })
                .unwrap_or_else(|| ".".to_string());
            DependencySourceRootSurface {
                namespace_prefix: alias.to_string(),
                relative_path,
            }
        })
        .collect::<Vec<_>>();
    source_roots.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    source_roots.dedup_by(|left, right| left.relative_path == right.relative_path);

    let mut projected_modules = exec_output
        .operations
        .iter()
        .filter_map(|operation| match &operation.kind {
            BuildEvaluationOperationKind::AddModule(request) => Some(DependencyModuleSurface {
                name: request.name.clone(),
                source_namespace: request.root_module.clone(),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    projected_modules.sort_by(|left, right| left.name.cmp(&right.name));
    projected_modules.dedup_by(|left, right| left.name == right.name);

    let mut projected_artifacts = fol_build::project_graph_artifacts(&plan.graph)
        .into_iter()
        .map(|artifact| DependencyArtifactSurface {
            name: artifact.name,
            artifact_kind: match artifact.kind {
                fol_build::BuildArtifactModelKind::Executable => "exe".to_string(),
                fol_build::BuildArtifactModelKind::StaticLibrary => "static-lib".to_string(),
                fol_build::BuildArtifactModelKind::SharedLibrary => "shared-lib".to_string(),
                fol_build::BuildArtifactModelKind::TestBundle => "test".to_string(),
                fol_build::BuildArtifactModelKind::GeneratedSourceBundle => {
                    "generated-source".to_string()
                }
                fol_build::BuildArtifactModelKind::DocsBundle => "docs".to_string(),
            },
            fol_model: artifact.target.fol_model.as_str().to_string(),
        })
        .collect::<Vec<_>>();
    projected_artifacts.sort_by(|left, right| left.name.cmp(&right.name));
    projected_artifacts.dedup_by(|left, right| left.name == right.name);

    let mut projected_steps = fol_build::project_graph_steps(&plan.graph)
        .into_iter()
        .map(|step| DependencyStepSurface {
            step_kind: step
                .default_kind
                .map(|kind| kind.as_str().to_string())
                .unwrap_or_else(|| "custom".to_string()),
            name: step.name,
        })
        .collect::<Vec<_>>();
    projected_steps.sort_by(|left, right| left.name.cmp(&right.name));
    projected_steps.dedup_by(|left, right| left.name == right.name);

    let mut projected_generated_outputs = exec_output
        .generated_files
        .iter()
        .map(|generated| DependencyGeneratedOutputSurface {
            name: generated.name.clone(),
            relative_path: generated.relative_path.clone(),
        })
        .collect::<Vec<_>>();
    projected_generated_outputs.sort_by(|left, right| left.name.cmp(&right.name));
    projected_generated_outputs.dedup_by(|left, right| left.name == right.name);

    let modules = if exec_output
        .dependency_exports
        .iter()
        .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Module)
    {
        let projected = projected_modules
            .into_iter()
            .map(|module| (module.name.clone(), module))
            .collect::<std::collections::BTreeMap<_, _>>();
        exec_output
            .dependency_exports
            .iter()
            .filter(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Module)
            .map(|export| {
                let module = projected.get(&export.target_name).ok_or_else(|| {
                    PackageError::new(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency surface projection could not resolve exported module '{}' from '{}'",
                            export.target_name,
                            build_path.display()
                        ),
                    )
                })?;
                Ok(DependencyModuleSurface {
                    name: export.name.clone(),
                    source_namespace: module.source_namespace.clone(),
                })
            })
            .collect::<Result<Vec<_>, PackageError>>()?
    } else {
        Vec::new()
    };

    let artifacts = if exec_output
        .dependency_exports
        .iter()
        .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Artifact)
    {
        let projected = projected_artifacts
            .into_iter()
            .map(|artifact| (artifact.name.clone(), artifact))
            .collect::<std::collections::BTreeMap<_, _>>();
        exec_output
            .dependency_exports
            .iter()
            .filter(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Artifact)
            .map(|export| {
                let artifact = projected.get(&export.target_name).ok_or_else(|| {
                    PackageError::new(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency surface projection could not resolve exported artifact '{}' from '{}'",
                            export.target_name,
                            build_path.display()
                        ),
                    )
                })?;
                Ok(DependencyArtifactSurface {
                    name: export.name.clone(),
                    artifact_kind: artifact.artifact_kind.clone(),
                    fol_model: artifact.fol_model.clone(),
                })
            })
            .collect::<Result<Vec<_>, PackageError>>()?
    } else {
        Vec::new()
    };

    let steps = if exec_output
        .dependency_exports
        .iter()
        .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Step)
    {
        let projected = projected_steps
            .into_iter()
            .map(|step| (step.name.clone(), step))
            .collect::<std::collections::BTreeMap<_, _>>();
        exec_output
            .dependency_exports
            .iter()
            .filter(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Step)
            .map(|export| {
                let step = projected.get(&export.target_name).ok_or_else(|| {
                    PackageError::new(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency surface projection could not resolve exported step '{}' from '{}'",
                            export.target_name,
                            build_path.display()
                        ),
                    )
                })?;
                Ok(DependencyStepSurface {
                    name: export.name.clone(),
                    step_kind: step.step_kind.clone(),
                })
            })
            .collect::<Result<Vec<_>, PackageError>>()?
    } else {
        Vec::new()
    };

    let generated_outputs = if exec_output
        .dependency_exports
        .iter()
        .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::GeneratedOutput)
    {
        let projected = projected_generated_outputs
            .iter()
            .map(|output| (output.name.clone(), output))
            .collect::<std::collections::BTreeMap<_, _>>();
        exec_output
            .dependency_exports
            .iter()
            .filter(|export| {
                export.kind == fol_build::BuildRuntimeDependencyExportKind::GeneratedOutput
            })
            .map(|export| {
                let output = projected.get(&export.target_name).ok_or_else(|| {
                    PackageError::new(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency surface projection could not resolve exported output '{}' from '{}'",
                            export.target_name,
                            build_path.display()
                        ),
                    )
                })?;
                Ok(DependencyGeneratedOutputSurface {
                    name: export.name.clone(),
                    relative_path: output.relative_path.clone(),
                })
            })
            .collect::<Result<Vec<_>, PackageError>>()?
    } else {
        Vec::new()
    };

    let files = if exec_output
        .dependency_exports
        .iter()
        .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::File)
    {
        exec_output
            .dependency_exports
            .iter()
            .filter(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::File)
            .map(|export| DependencyFileSurface {
                name: export.name.clone(),
                relative_path: export.target_name.clone(),
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let dirs = if exec_output
        .dependency_exports
        .iter()
        .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Dir)
    {
        exec_output
            .dependency_exports
            .iter()
            .filter(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Dir)
            .map(|export| DependencyDirSurface {
                name: export.name.clone(),
                relative_path: export.target_name.clone(),
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let paths = if exec_output
        .dependency_exports
        .iter()
        .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Path)
    {
        let projected = projected_generated_outputs
            .iter()
            .map(|output| (output.name.clone(), output))
            .collect::<std::collections::BTreeMap<_, _>>();
        exec_output
            .dependency_exports
            .iter()
            .filter(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Path)
            .map(|export| {
                let path = projected.get(&export.target_name).ok_or_else(|| {
                    PackageError::new(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency surface projection could not resolve exported path '{}' from '{}'",
                            export.target_name,
                            build_path.display()
                        ),
                    )
                })?;
                Ok(DependencyPathSurface {
                    name: export.name.clone(),
                    relative_path: path.relative_path.clone(),
                })
            })
            .collect::<Result<Vec<_>, PackageError>>()?
    } else {
        Vec::new()
    };

    Ok(DependencyBuildSurface {
        alias: alias.to_string(),
        exposure: DependencyBuildExposure {
            modules_explicit: exec_output
                .dependency_exports
                .iter()
                .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Module),
            artifacts_explicit: exec_output
                .dependency_exports
                .iter()
                .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Artifact),
            steps_explicit: exec_output
                .dependency_exports
                .iter()
                .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Step),
            files_explicit: exec_output
                .dependency_exports
                .iter()
                .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::File),
            dirs_explicit: exec_output
                .dependency_exports
                .iter()
                .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Dir),
            paths_explicit: exec_output
                .dependency_exports
                .iter()
                .any(|export| export.kind == fol_build::BuildRuntimeDependencyExportKind::Path),
            generated_outputs_explicit: exec_output.dependency_exports.iter().any(|export| {
                export.kind == fol_build::BuildRuntimeDependencyExportKind::GeneratedOutput
            }),
        },
        modules,
        source_roots,
        artifacts,
        steps,
        files,
        dirs,
        paths,
        generated_outputs,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        dependency_modules_from_exports, project_dependency_surface, DependencyBuildExposure,
    };
    use crate::PreparedExportMount;
    use fol_parser::ast::AstParser;
    use fol_stream::FileStream;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn unique_temp_root(label: &str) -> std::path::PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "fol_pkg_dep_surface_{}_{}_{}",
            label,
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn dependency_module_bridge_projects_prepared_exports_into_module_surfaces() {
        let modules = dependency_modules_from_exports(
            "json",
            &[
                PreparedExportMount {
                    source_namespace: "json::src".to_string(),
                    mounted_namespace_suffix: None,
                },
                PreparedExportMount {
                    source_namespace: "json::src::codec".to_string(),
                    mounted_namespace_suffix: Some("codec".to_string()),
                },
            ],
        );

        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].name, "json");
        assert_eq!(modules[0].source_namespace, "json::src");
        assert_eq!(modules[1].name, "codec");
        assert_eq!(modules[1].source_namespace, "json::src::codec");
    }

    #[test]
    fn projected_dependency_surface_keeps_source_import_roots_without_build_exports() {
        let root = unique_temp_root("projected_surface");
        fs::create_dir_all(root.join("src/root")).expect("fixture root should exist");
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"json\", version = \"1.0.0\" });\n",
                "    var graph = build.graph();\n",
                "    var codec = graph.add_module({ name = \"codec\", root = \"src/root/codec.fol\" });\n",
                "    var app = graph.add_exe({ name = \"json\", root = \"src/root/main.fol\" });\n",
                "    var schema = graph.write_file({ name = \"schema\", path = \"gen/schema.fol\", contents = \"ok\" });\n",
                "    var docs = graph.step(\"docs\");\n",
                "    graph.install(app);\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("build fixture should be written");
        fs::write(
            root.join("src/root/main.fol"),
            "var[exp] answer: int = 42;\n",
        )
        .expect("source fixture should be written");
        fs::write(
            root.join("src/root/codec.fol"),
            "var[exp] codec: int = 7;\n",
        )
        .expect("module fixture should be written");

        let mut stream = FileStream::from_folder(root.to_str().expect("temp path should be utf-8"))
            .expect("folder stream should open");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("package syntax should parse");

        let surface =
            project_dependency_surface("json", &root, &syntax).expect("surface should project");

        assert_eq!(surface.alias, "json");
        assert_eq!(surface.exposure, DependencyBuildExposure::default());
        // The projected source-root path is derived from the parsed source
        // unit path, which is absolute here; assert it resolves to src/root.
        assert_eq!(surface.source_roots.len(), 1);
        assert!(
            std::path::Path::new(surface.source_roots[0].relative_path.as_str())
                .ends_with("src/root"),
            "source root should resolve to src/root, got {}",
            surface.source_roots[0].relative_path
        );
        assert!(surface.modules.is_empty());
        assert!(surface.artifacts.is_empty());
        assert!(surface.steps.is_empty());
        assert!(surface.generated_outputs.is_empty());

        fs::remove_dir_all(&root).expect("fixture root should be removable");
    }

    #[test]
    fn projected_dependency_surface_prefers_explicit_exports_when_declared() {
        let root = unique_temp_root("explicit_surface");
        fs::create_dir_all(root.join("src")).expect("fixture root should exist");
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"json\", version = \"1.0.0\" });\n",
                "    var graph = build.graph();\n",
                "    var codec = graph.add_module({ name = \"codec\", root = \"src/codec.fol\" });\n",
                "    var app = graph.add_static_lib({ name = \"json\", root = \"src/main.fol\" });\n",
                "    var schema = graph.write_file({ name = \"schema\", path = \"gen/schema.fol\", contents = \"ok\" });\n",
                "    var config = graph.file_from_root(\"config/defaults.toml\");\n",
                "    var assets = graph.dir_from_root(\"assets\");\n",
                "    var docs = graph.step(\"docs\");\n",
                "    build.export_module({ name = \"api\", module = codec });\n",
                "    build.export_artifact({ name = \"runtime\", artifact = app });\n",
                "    build.export_step({ name = \"check\", step = docs });\n",
                "    build.export_file({ name = \"defaults\", file = config });\n",
                "    build.export_dir({ name = \"public\", dir = assets });\n",
                "    build.export_path({ name = \"schema-path\", path = schema });\n",
                "    build.export_output({ name = \"schema-api\", output = schema });\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("build fixture should be written");
        fs::create_dir_all(root.join("config")).expect("config fixture root should exist");
        fs::create_dir_all(root.join("assets")).expect("assets fixture root should exist");
        fs::write(root.join("src/main.fol"), "var[exp] answer: int = 42;\n")
            .expect("source fixture should be written");
        fs::write(root.join("src/codec.fol"), "var[exp] codec: int = 7;\n")
            .expect("module fixture should be written");
        fs::write(root.join("config/defaults.toml"), "ok = true\n")
            .expect("file fixture should be written");
        fs::write(root.join("assets/logo.txt"), "logo\n").expect("dir fixture should be written");

        let mut stream = FileStream::from_folder(root.to_str().expect("temp path should be utf-8"))
            .expect("folder stream should open");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("package syntax should parse");

        let surface =
            project_dependency_surface("json", &root, &syntax).expect("surface should project");

        assert!(surface.exposure.modules_explicit);
        assert!(surface.exposure.artifacts_explicit);
        assert!(surface.exposure.steps_explicit);
        assert!(surface.exposure.files_explicit);
        assert!(surface.exposure.dirs_explicit);
        assert!(surface.exposure.paths_explicit);
        assert!(surface.exposure.generated_outputs_explicit);
        assert_eq!(
            surface
                .modules
                .iter()
                .map(|module| module.name.as_str())
                .collect::<Vec<_>>(),
            vec!["api"]
        );
        assert_eq!(
            surface
                .artifacts
                .iter()
                .map(|artifact| artifact.name.as_str())
                .collect::<Vec<_>>(),
            vec!["runtime"]
        );
        assert_eq!(
            surface
                .steps
                .iter()
                .map(|step| step.name.as_str())
                .collect::<Vec<_>>(),
            vec!["check"]
        );
        assert_eq!(
            surface
                .files
                .iter()
                .map(|file| file.name.as_str())
                .collect::<Vec<_>>(),
            vec!["defaults"]
        );
        assert_eq!(
            surface
                .dirs
                .iter()
                .map(|dir| dir.name.as_str())
                .collect::<Vec<_>>(),
            vec!["public"]
        );
        assert_eq!(
            surface
                .paths
                .iter()
                .map(|path| path.name.as_str())
                .collect::<Vec<_>>(),
            vec!["schema-path"]
        );
        assert_eq!(
            surface
                .generated_outputs
                .iter()
                .map(|output| output.name.as_str())
                .collect::<Vec<_>>(),
            vec!["schema-api"]
        );
        assert_eq!(surface.source_roots.len(), 1);
        assert!(
            std::path::Path::new(surface.source_roots[0].relative_path.as_str()).ends_with("src"),
            "source root should resolve to src, got {}",
            surface.source_roots[0].relative_path
        );

        fs::remove_dir_all(&root).expect("fixture root should be removable");
    }

    #[test]
    fn projected_dependency_surface_keeps_source_roots_even_when_build_exports_are_empty() {
        let root = unique_temp_root("source_imports_without_build_exports");
        fs::create_dir_all(root.join("src/root")).expect("fixture root should exist");
        fs::write(
            root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"json\", version = \"1.0.0\" });\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("build fixture should be written");
        fs::write(
            root.join("src/root/main.fol"),
            "var[exp] answer: int = 42;\n",
        )
        .expect("source fixture should be written");

        let mut stream = FileStream::from_folder(root.to_str().expect("temp path should be utf-8"))
            .expect("folder stream should open");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("package syntax should parse");

        let surface =
            project_dependency_surface("json", &root, &syntax).expect("surface should project");

        assert_eq!(surface.source_roots.len(), 1);
        assert!(
            std::path::Path::new(surface.source_roots[0].relative_path.as_str())
                .ends_with("src/root"),
            "source root should resolve to src/root, got {}",
            surface.source_roots[0].relative_path
        );
        assert!(surface.modules.is_empty());
        assert!(surface.artifacts.is_empty());
        assert!(surface.steps.is_empty());
        assert!(surface.generated_outputs.is_empty());

        fs::remove_dir_all(&root).expect("fixture root should be removable");
    }
}

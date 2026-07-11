use crate::{
    mangle_package_module_name, plan_generated_crate_layout, plan_namespace_layouts,
    plan_package_layouts, render_entry_definition, render_entry_trait_impl,
    render_global_declaration, render_record_definition, render_record_trait_impl,
    render_routine_definition, render_rust_type_in_workspace,
    BackendArtifact, BackendConfig,
    BackendError, BackendErrorKind, BackendResult, BackendRuntimeTier, BackendSession,
    EmittedRustFile,
};
use fol_lower::{LoweredBuiltinType, LoweredType, LoweredTypeId};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::runtime::backend_runtime_source_root;

pub fn emit_cargo_toml(session: &BackendSession) -> EmittedRustFile {
    let layout = plan_generated_crate_layout(session);
    let package_name = session.workspace_identity().crate_dir_name.clone();
    let runtime_path = runtime_dependency_path();

    EmittedRustFile {
        path: layout.cargo_toml_path,
        module_name: "cargo".to_string(),
        contents: format!(
            "[package]\nname = \"{package_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n\n[dependencies]\nfol-runtime = {{ path = \"{}\" }}\n",
            runtime_path.display()
        ),
    }
}

pub fn emit_main_rs_for_config(
    session: &BackendSession,
    config: &BackendConfig,
) -> BackendResult<EmittedRustFile> {
    let layout = plan_generated_crate_layout(session);
    let entry_candidate = session.select_buildable_entry_candidate()?;
    let entry_name = &session.entry_identity().display_name;
    let entry_wrapper = match resolve_entry_callable(session, entry_candidate, config.runtime_tier())? {
        EntryCallable {
            rust_path,
            call_args,
            recoverable: false,
        } => format!("    let _ = {rust_path}({call_args});"),
        EntryCallable {
            rust_path,
            call_args,
            recoverable: true,
        } => format!(
            "    let __fol_outcome = rt::outcome_from_recoverable({rust_path}({call_args}));\n    if let Some(__fol_message) = rt::printable_outcome_message(&__fol_outcome) {{\n        eprintln!(\"{{}}\", __fol_message);\n    }}\n    std::process::exit(__fol_outcome.exit_code());"
        ),
    };
    let runtime_tier = config.runtime_tier();

    Ok(EmittedRustFile {
        path: layout.main_rs_path,
        module_name: "main".to_string(),
        contents: format!(
            "{}\n\nmod packages;\n\nfn main() {{\n    let _runtime = rt::crate_name();\n    let _runtime_tier = rt_model::tier_name();\n    let _entry_package = \"{entry_name}\";\n    let _entry_name = \"{}\";\n    let _ = (&_runtime, &_runtime_tier, &_entry_package, &_entry_name);\n{entry_wrapper}\n}}\n",
            runtime_main_use_block(runtime_tier),
            entry_candidate.name
        ),
    })
}

pub fn emit_main_rs(session: &BackendSession) -> BackendResult<EmittedRustFile> {
    emit_main_rs_for_config(session, &BackendConfig::default())
}

pub fn emit_package_module_shells(session: &BackendSession) -> Vec<EmittedRustFile> {
    let package_plans = plan_package_layouts(session);
    let namespace_plans = plan_namespace_layouts(session);
    let mut direct_modules_by_path: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for namespace_plan in &namespace_plans {
        let package_module = mangle_package_module_name(&namespace_plan.package_identity);
        let relative_parts = namespace_plan
            .relative_file
            .split('/')
            .map(str::to_string)
            .collect::<Vec<_>>();
        if relative_parts.is_empty() {
            continue;
        }
        let root_child = module_name_from_relative_part(&relative_parts[0]);
        direct_modules_by_path
            .entry(format!("src/packages/{package_module}/mod.rs"))
            .or_default()
            .push(root_child);

        if relative_parts.len() <= 1 {
            continue;
        }

        for index in 0..(relative_parts.len() - 1) {
            let parent_dir = if index == 0 {
                format!("src/packages/{package_module}/{}", relative_parts[0])
            } else {
                format!(
                    "src/packages/{package_module}/{}",
                    relative_parts[..=index].join("/")
                )
            };
            let child = module_name_from_relative_part(&relative_parts[index + 1]);
            direct_modules_by_path
                .entry(format!("{parent_dir}/mod.rs"))
                .or_default()
                .push(child);
        }
    }

    let mut files = vec![EmittedRustFile {
        path: "src/packages/mod.rs".to_string(),
        module_name: "packages".to_string(),
        contents: package_plans
            .iter()
            .map(|plan| format!("pub mod {};", plan.module_name))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    }];

    for package_plan in package_plans {
        let mut namespace_modules = direct_modules_by_path
            .remove(&format!("{}/mod.rs", package_plan.relative_dir))
            .unwrap_or_default();
        namespace_modules.sort();
        namespace_modules.dedup();

        files.push(EmittedRustFile {
            path: format!("{}/mod.rs", package_plan.relative_dir),
            module_name: package_plan.module_name.clone(),
            contents: namespace_modules
                .iter()
                .map(|module_name| format!("pub mod {module_name};"))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        });
    }

    let mut nested_mod_paths = direct_modules_by_path.into_iter().collect::<Vec<_>>();
    nested_mod_paths.sort_by(|left, right| left.0.cmp(&right.0));
    for (path, mut module_names) in nested_mod_paths {
        module_names.sort();
        module_names.dedup();
        files.push(EmittedRustFile {
            module_name: "mod".to_string(),
            path,
            contents: module_names
                .iter()
                .map(|module_name| format!("pub mod {module_name};"))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));

    files
}

pub fn emit_namespace_module_shells_for_config(
    session: &BackendSession,
    config: &BackendConfig,
) -> BackendResult<Vec<EmittedRustFile>> {
    let mut files = Vec::new();
    let runtime_tier = config.runtime_tier();
    for namespace_plan in plan_namespace_layouts(session) {
        let emitted_items = render_namespace_items(session, &namespace_plan)?;
        let mut contents = format!(
                "{}\n\npub(crate) const NAMESPACE_NAME: &str = \"{}\";\npub(crate) const SOURCE_UNIT_IDS: &[usize] = &[{}];\n\npub(crate) fn namespace_runtime_marker() -> &'static str {{\n    let _ = rt::crate_name();\n    let _ = rt_model::tier_name();\n    NAMESPACE_NAME\n}}\n",
                runtime_use_block(runtime_tier),
                namespace_plan.full_namespace,
                namespace_plan
                    .source_unit_ids
                    .iter()
                    .map(|source_unit_id| source_unit_id.0.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        if !emitted_items.is_empty() {
            contents.push('\n');
            contents.push_str(&emitted_items);
        }
        files.push(EmittedRustFile {
            path: format!(
                "src/packages/{}/{}",
                mangle_package_module_name(&namespace_plan.package_identity),
                namespace_plan.relative_file
            ),
            module_name: namespace_plan.module_name.clone(),
            contents,
        });
    }
    Ok(files)
}

pub fn emit_namespace_module_shells(
    session: &BackendSession,
) -> BackendResult<Vec<EmittedRustFile>> {
    emit_namespace_module_shells_for_config(session, &BackendConfig::default())
}

pub fn emit_generated_crate_skeleton_for_config(
    session: &BackendSession,
    config: &BackendConfig,
) -> BackendResult<BackendArtifact> {
    let layout = plan_generated_crate_layout(session);
    let mut files = Vec::new();
    files.push(emit_cargo_toml(session));
    files.push(emit_main_rs_for_config(session, config)?);
    files.extend(emit_package_module_shells(session));
    files.extend(emit_namespace_module_shells_for_config(session, config)?);
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(BackendArtifact::RustSourceCrate {
        root: layout.crate_dir_name,
        files,
    })
}

pub fn emit_generated_crate_skeleton(session: &BackendSession) -> BackendResult<BackendArtifact> {
    emit_generated_crate_skeleton_for_config(session, &BackendConfig::default())
}

fn module_name_from_relative_part(relative_part: &str) -> String {
    relative_part
        .strip_suffix(".rs")
        .map(str::to_string)
        .unwrap_or_else(|| relative_part.to_string())
}

pub(super) fn runtime_dependency_path() -> PathBuf {
    backend_runtime_source_root()
}

fn runtime_use_block(runtime_tier: BackendRuntimeTier) -> String {
    format!(
        "use {} as rt;\nuse {} as rt_model;",
        runtime_tier.runtime_module_path(),
        runtime_tier.runtime_module_path()
    )
}

fn runtime_main_use_block(runtime_tier: BackendRuntimeTier) -> String {
    format!(
        "use {} as rt;\nuse {} as rt_model;\n\nfn __fol_cli_arg(index: usize) -> Option<String> {{\n    std::env::args().nth(index + 1)\n}}\n\nfn __fol_parse_bool(raw: &str) -> Option<rt::FolBool> {{\n    match raw {{\n        \"true\" | \"1\" | \"yes\" | \"on\" => Some(true),\n        \"false\" | \"0\" | \"no\" | \"off\" => Some(false),\n        _ => None,\n    }}\n}}\n\nfn __fol_parse_char(raw: &str) -> Option<rt::FolChar> {{\n    let mut chars = raw.chars();\n    let first = chars.next()?;\n    if chars.next().is_some() {{\n        None\n    }} else {{\n        Some(first)\n    }}\n}}\n",
        runtime_tier.runtime_module_path(),
        runtime_tier.runtime_module_path()
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EntryCallable {
    rust_path: String,
    call_args: String,
    recoverable: bool,
}

fn resolve_entry_callable(
    session: &BackendSession,
    entry_candidate: &fol_lower::LoweredEntryCandidate,
    runtime_tier: BackendRuntimeTier,
) -> BackendResult<EntryCallable> {
    let package = session
        .workspace()
        .package(&entry_candidate.package_identity)
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "entry candidate '{}' points at missing package '{}'",
                    entry_candidate.name, entry_candidate.package_identity.display_name
                ),
            )
        })?;
    let routine = package
        .routine_decls
        .get(&entry_candidate.routine_id)
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "entry candidate '{}' points at missing routine {:?}",
                    entry_candidate.name, entry_candidate.routine_id
                ),
            )
        })?;
    if routine.receiver_type.is_some() {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!(
                "entry routine '{}' must be a plain free routine",
                entry_candidate.name
            ),
        ));
    }
    let signature_id = routine.signature.ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("entry routine '{}' is missing a lowered signature", entry_candidate.name),
        )
    })?;
    let signature = match session.workspace().type_table().get(signature_id) {
        Some(LoweredType::Routine(signature)) => signature,
        _ => {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "entry routine '{}' signature does not resolve to a lowered routine type",
                    entry_candidate.name
                ),
            ))
        }
    };
    let source_unit_id = routine.source_unit_id.ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("entry routine '{}' is missing a source unit", entry_candidate.name),
        )
    })?;
    let namespace_plan = plan_namespace_layouts(session).into_iter().find(|plan| {
        plan.package_identity == entry_candidate.package_identity
            && plan.source_unit_ids.contains(&source_unit_id)
    }).ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            format!(
                "entry routine '{}' does not map to a namespace layout",
                entry_candidate.name
            ),
        )
    })?;
    render_routine_definition(
        session.workspace(),
        &entry_candidate.package_identity,
        routine,
        session.workspace().type_table(),
    )
    .map_err(|error| {
        BackendError::new(
            error.kind(),
            format!(
                "entry routine '{}' cannot be rendered for backend emission: {}",
                entry_candidate.name,
                error.message()
            ),
        )
    })?;
    // A namespace that owns child namespaces emits as `<dir>/mod.rs`, which
    // in Rust IS the `<dir>` module — the `mod` file stem must not become a
    // path segment.
    let namespace_path = namespace_plan
        .relative_file
        .trim_end_matches(".rs")
        .trim_end_matches("/mod")
        .replace('/', "::");
    Ok(EntryCallable {
        rust_path: format!(
            "packages::{}::{}::{}",
            mangle_package_module_name(&entry_candidate.package_identity),
            namespace_path,
            crate::mangle_routine_name(
                &entry_candidate.package_identity,
                entry_candidate.routine_id,
                &entry_candidate.name
            )
        ),
        call_args: render_entry_call_args(session, &signature.params, runtime_tier)?,
        recoverable: signature.error_type.is_some(),
    })
}

fn render_entry_call_args(
    session: &BackendSession,
    params: &[LoweredTypeId],
    runtime_tier: BackendRuntimeTier,
) -> BackendResult<String> {
    let type_table = session.workspace().type_table();
    let rendered = params
        .iter()
        .enumerate()
        .map(|(index, type_id)| render_entry_arg_expr(session, type_table, *type_id, index, runtime_tier))
        .collect::<BackendResult<Vec<_>>>()?;
    Ok(rendered.join(", "))
}

fn render_entry_arg_expr(
    session: &BackendSession,
    type_table: &fol_lower::LoweredTypeTable,
    type_id: LoweredTypeId,
    index: usize,
    runtime_tier: BackendRuntimeTier,
) -> BackendResult<String> {
    let Some(ty) = type_table.get(type_id) else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("entry parameter type {:?} is missing from the lowered type table", type_id),
        ));
    };
    let expr = match ty {
        LoweredType::Builtin(LoweredBuiltinType::Int) => {
            format!("__fol_cli_arg({index}).and_then(|raw| raw.parse::<rt::FolInt>().ok()).unwrap_or_default()")
        }
        LoweredType::Builtin(LoweredBuiltinType::Float) => {
            format!("__fol_cli_arg({index}).and_then(|raw| raw.parse::<rt::FolFloat>().ok()).unwrap_or_default()")
        }
        LoweredType::Builtin(LoweredBuiltinType::Bool) => {
            format!("__fol_cli_arg({index}).and_then(|raw| __fol_parse_bool(&raw)).unwrap_or_default()")
        }
        LoweredType::Builtin(LoweredBuiltinType::Char) => {
            format!("__fol_cli_arg({index}).and_then(|raw| __fol_parse_char(&raw)).unwrap_or_default()")
        }
        LoweredType::Builtin(LoweredBuiltinType::Str) => {
            format!("__fol_cli_arg({index}).map(rt_model::FolStr::from).unwrap_or_default()")
        }
        LoweredType::Builtin(LoweredBuiltinType::Never) => "rt::impossible()".to_string(),
        _ => {
            let rendered_type = render_rust_type_in_workspace(
                Some(session.workspace()),
                type_table,
                type_id,
            )?;
            let _ = runtime_tier;
            format!("<{rendered_type} as Default>::default()")
        }
    };
    Ok(expr)
}

fn render_namespace_items(
    session: &BackendSession,
    namespace_plan: &crate::NamespaceLayoutPlan,
) -> BackendResult<String> {
    let Some(package) = session
        .workspace()
        .package(&namespace_plan.package_identity)
    else {
        return Ok(String::new());
    };
    let mut items = Vec::new();

    let mut types = package
        .type_decls
        .values()
        .filter(|type_decl| {
            namespace_plan
                .source_unit_ids
                .contains(&type_decl.source_unit_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    types.sort_by_key(|type_decl| type_decl.runtime_type.0);
    for type_decl in &types {
        let rendered = match &type_decl.kind {
            fol_lower::LoweredTypeDeclKind::Record { .. } => render_record_definition(
                session.workspace(),
                &namespace_plan.package_identity,
                type_decl,
                session.workspace().type_table(),
            )
            .and_then(|definition| {
                Ok(format!(
                    "{definition}\n{}",
                    render_record_trait_impl(&namespace_plan.package_identity, type_decl)?
                ))
            }),
            fol_lower::LoweredTypeDeclKind::Entry { .. } => render_entry_definition(
                session.workspace(),
                &namespace_plan.package_identity,
                type_decl,
                session.workspace().type_table(),
            )
            .and_then(|definition| {
                Ok(format!(
                    "{definition}\n{}",
                    render_entry_trait_impl(&namespace_plan.package_identity, type_decl)?
                ))
            }),
            fol_lower::LoweredTypeDeclKind::Alias { .. } => Ok(String::new()),
        };
        let rendered = rendered?;
        if !rendered.is_empty() {
            items.push(rendered);
        }
    }

    let mut globals = package
        .global_decls
        .values()
        .filter(|global| {
            namespace_plan
                .source_unit_ids
                .contains(&global.source_unit_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    globals.sort_by_key(|global| global.id.0);
    for global in &globals {
        let rendered = render_global_declaration(
            session.workspace(),
            &namespace_plan.package_identity,
            global,
            session.workspace().type_table(),
        )?;
        items.push(rendered);
    }

    let mut routines = package
        .routine_decls
        .values()
        .filter(|routine| {
            routine.source_unit_id.is_some_and(|source_unit_id| {
                namespace_plan.source_unit_ids.contains(&source_unit_id)
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    routines.sort_by_key(|routine| routine.id.0);

    for routine in &routines {
        let rendered = render_routine_definition(
            session.workspace(),
            &namespace_plan.package_identity,
            routine,
            session.workspace().type_table(),
        )?;
        items.push(rendered);
    }

    Ok(items.join("\n"))
}

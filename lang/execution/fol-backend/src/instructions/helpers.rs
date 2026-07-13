use crate::{
    mangle_global_name, mangle_local_name, mangle_routine_name, mangle_type_name, BackendError,
    BackendErrorKind, BackendResult,
};
use fol_lower::{
    LoweredBuiltinType, LoweredGlobal, LoweredInstr, LoweredOperand, LoweredRoutine, LoweredType,
    LoweredTypeDecl, LoweredTypeId, LoweredTypeTable, LoweredWorkspace,
};
use fol_resolver::PackageIdentity;

pub fn resolve_global_decl(
    workspace: Option<&LoweredWorkspace>,
    global_id: fol_lower::LoweredGlobalId,
) -> BackendResult<(&PackageIdentity, &LoweredGlobal)> {
    let Some(workspace) = workspace else {
        return Err(BackendError::new(
            BackendErrorKind::Unsupported,
            format!(
                "workspace-aware global emission is required for global {:?}",
                global_id
            ),
        ));
    };
    workspace
        .packages()
        .find_map(|package| {
            package
                .global_decls
                .get(&global_id)
                .map(|global| (&package.identity, global))
        })
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "lowered global {:?} is missing from the workspace",
                    global_id
                ),
            )
        })
}

pub fn resolve_routine_decl(
    workspace: Option<&LoweredWorkspace>,
    routine_id: fol_lower::LoweredRoutineId,
) -> BackendResult<(&PackageIdentity, &LoweredRoutine)> {
    let Some(workspace) = workspace else {
        return Err(BackendError::new(
            BackendErrorKind::Unsupported,
            format!(
                "workspace-aware routine emission is required for routine {:?}",
                routine_id
            ),
        ));
    };
    workspace
        .packages()
        .find_map(|package| {
            package
                .routine_decls
                .get(&routine_id)
                .map(|routine| (&package.identity, routine))
        })
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "lowered routine {:?} is missing from the workspace",
                    routine_id
                ),
            )
        })
}

pub fn resolve_type_decl(
    workspace: Option<&LoweredWorkspace>,
    runtime_type: fol_lower::LoweredTypeId,
) -> BackendResult<(&PackageIdentity, &LoweredTypeDecl)> {
    let Some(workspace) = workspace else {
        return Err(BackendError::new(
            BackendErrorKind::Unsupported,
            format!(
                "workspace-aware aggregate emission is required for type {:?}",
                runtime_type
            ),
        ));
    };
    workspace
        .packages()
        .find_map(|package| {
            package
                .type_decls
                .values()
                .find(|type_decl| type_decl.runtime_type == runtime_type)
                .map(|type_decl| (&package.identity, type_decl))
        })
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "lowered type {:?} does not have a rendered declaration owner",
                    runtime_type
                ),
            )
        })
}

pub fn render_global_load(
    workspace: Option<&LoweredWorkspace>,
    type_table: &LoweredTypeTable,
    global_identity: &PackageIdentity,
    global: &LoweredGlobal,
) -> BackendResult<String> {
    let global_name = format!(
        "{}::{}",
        render_namespace_module_path(workspace, global_identity, global.source_unit_id)?,
        mangle_global_name(global_identity, global.id, &global.name)
    );
    if global.mutable {
        let default_expr =
            render_type_default_expr_in_workspace(workspace, type_table, global.type_id)?;
        Ok(format!(
            "{}.get_or_init(|| std::sync::Mutex::new({default_expr})).lock().unwrap_or_else(|e| e.into_inner()).clone()",
            global_name,
        ))
    } else {
        Ok(format!(
            "{global_name}.get_or_init(Default::default).clone()"
        ))
    }
}

pub fn render_routine_path(
    workspace: Option<&LoweredWorkspace>,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
) -> BackendResult<String> {
    Ok(format!(
        "{}::{}",
        render_namespace_module_path(
            workspace,
            package_identity,
            routine.source_unit_id.ok_or_else(|| BackendError::new(
                BackendErrorKind::InvalidInput,
                format!("routine '{}' is missing a source unit", routine.name),
            ))?,
        )?,
        mangle_routine_name(package_identity, routine.id, &routine.name)
    ))
}

pub fn render_type_path(
    workspace: Option<&LoweredWorkspace>,
    package_identity: &PackageIdentity,
    type_decl: &LoweredTypeDecl,
) -> BackendResult<String> {
    Ok(format!(
        "{}::{}",
        render_namespace_module_path(workspace, package_identity, type_decl.source_unit_id)?,
        mangle_type_name(package_identity, type_decl.runtime_type, &type_decl.name)
    ))
}

pub fn render_namespace_module_path(
    workspace: Option<&LoweredWorkspace>,
    package_identity: &PackageIdentity,
    source_unit_id: fol_resolver::SourceUnitId,
) -> BackendResult<String> {
    let Some(workspace) = workspace else {
        return Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "workspace-aware namespace emission is required",
        ));
    };
    let package = workspace.package(package_identity).ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            format!(
                "package '{}' is missing from workspace",
                package_identity.display_name
            ),
        )
    })?;
    let source_unit = package
        .source_units
        .iter()
        .find(|source_unit| source_unit.source_unit_id == source_unit_id)
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "source unit {:?} is missing from package '{}'",
                    source_unit_id, package_identity.display_name
                ),
            )
        })?;
    let mut segments = source_unit
        .namespace
        .split("::")
        .filter(|segment| !segment.is_empty())
        .map(crate::sanitize_backend_ident)
        .collect::<Vec<_>>();
    if segments.first().is_some_and(|segment| {
        segment == &crate::sanitize_backend_ident(&package_identity.display_name)
    }) {
        segments.remove(0);
    }
    let namespace_segment = match segments.as_slice() {
        [] => "root".to_string(),
        parts => parts.join("::"),
    };
    Ok(format!(
        "crate::packages::{}::{}",
        crate::mangle_package_module_name(package_identity),
        namespace_segment
    ))
}

pub fn render_native_intrinsic_expression(name: &str, args: &[String]) -> BackendResult<String> {
    match (name, args) {
        ("eq", [lhs, rhs]) => Ok(format!("{lhs} == {rhs}")),
        ("nq", [lhs, rhs]) => Ok(format!("{lhs} != {rhs}")),
        ("lt", [lhs, rhs]) => Ok(format!("{lhs} < {rhs}")),
        ("gt", [lhs, rhs]) => Ok(format!("{lhs} > {rhs}")),
        ("ge", [lhs, rhs]) => Ok(format!("{lhs} >= {rhs}")),
        ("le", [lhs, rhs]) => Ok(format!("{lhs} <= {rhs}")),
        ("not", [value]) => Ok(format!("!{value}")),
        (other, _) => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            format!("native Rust intrinsic emission is not implemented yet for '.{other}(...)'"),
        )),
    }
}

pub fn render_type_default_expr_in_workspace(
    workspace: Option<&LoweredWorkspace>,
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
) -> BackendResult<String> {
    let Some(ty) = type_table.get(type_id) else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("lowered type {:?} is missing from the type table", type_id),
        ));
    };

    match ty {
        LoweredType::Builtin(LoweredBuiltinType::Int) => Ok("0_i64".to_string()),
        LoweredType::Builtin(LoweredBuiltinType::Float) => Ok("0.0_f64".to_string()),
        LoweredType::Builtin(LoweredBuiltinType::Bool) => Ok("false".to_string()),
        LoweredType::Builtin(LoweredBuiltinType::Char) => Ok("'\\0'".to_string()),
        LoweredType::Builtin(LoweredBuiltinType::Str) => {
            Ok("rt_model::FolStr::new(\"\")".to_string())
        }
        LoweredType::Builtin(LoweredBuiltinType::Never) => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "never-typed globals cannot be default-initialized",
        )),
        LoweredType::GenericParameter { name } => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            format!("backend execution for generic parameter type '{name}' is not implemented yet"),
        )),
        LoweredType::Named { .. } => Ok(format!(
            "{}::default()",
            crate::render_rust_type_in_workspace(workspace, type_table, type_id)?
        )),
        LoweredType::Owned { inner } => Ok(format!(
            "Box::new({})",
            render_type_default_expr_in_workspace(workspace, type_table, *inner)?
        )),
        LoweredType::Borrowed { .. } => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "borrowed references cannot be default-initialized",
        )),
        LoweredType::Pointer { target, shared } => Ok(format!(
            "{}::new({})",
            if *shared { "std::rc::Rc" } else { "Box" },
            render_type_default_expr_in_workspace(workspace, type_table, *target)?
        )),
        LoweredType::Array {
            element_type,
            size: Some(_size),
        } => {
            let element_default =
                render_type_default_expr_in_workspace(workspace, type_table, *element_type)?;
            Ok(format!(
                "std::array::from_fn(|_| ({element_default}).clone())"
            ))
        }
        LoweredType::Array { size: None, .. } => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "unsized arrays are not supported; use vec[] for dynamic collections",
        )),
        LoweredType::Vector { .. } => Ok("rt_model::FolVec::new(vec![])".to_string()),
        LoweredType::Sequence { .. } => Ok("rt_model::FolSeq::new(vec![])".to_string()),
        LoweredType::Channel { .. } => Ok("rt::FolChannel::default()".to_string()),
        LoweredType::ChannelSender { .. } => Ok("rt::FolSender::default()".to_string()),
        LoweredType::Eventual { .. } => Ok("rt::FolEventual::default()".to_string()),
        LoweredType::Set { .. } => Ok("rt_model::FolSet::from_items(vec![])".to_string()),
        LoweredType::Map { .. } => Ok("rt_model::FolMap::from_pairs(vec![])".to_string()),
        LoweredType::Optional { .. } => Ok("rt::FolOption::nil()".to_string()),
        LoweredType::Error { inner } => Ok(match inner {
            Some(inner) => format!(
                "rt::FolError::new({})",
                render_type_default_expr_in_workspace(workspace, type_table, *inner)?
            ),
            None => "rt::FolError::new(())".to_string(),
        }),
        LoweredType::Record { .. } | LoweredType::Entry { .. } => Ok(format!(
            "{}::default()",
            crate::types::render_rust_type_in_workspace(workspace, type_table, type_id)?
        )),
        LoweredType::Routine(routine_type) => {
            let rendered_type =
                crate::types::render_rust_type_in_workspace(workspace, type_table, type_id)?;
            let dummy_params = routine_type
                .params
                .iter()
                .enumerate()
                .map(|(i, param_id)| {
                    crate::types::render_rust_type_in_workspace(workspace, type_table, *param_id)
                        .map(|ty| format!("_p{i}: {ty}"))
                })
                .collect::<BackendResult<Vec<_>>>()?;
            let return_clause = match (routine_type.return_type, routine_type.error_type) {
                (Some(ret), Some(err)) => format!(
                    " -> rt::FolRecover<{}, {}>",
                    crate::types::render_rust_type_in_workspace(workspace, type_table, ret)?,
                    crate::types::render_rust_type_in_workspace(workspace, type_table, err)?
                ),
                (Some(ret), None) => format!(
                    " -> {}",
                    crate::types::render_rust_type_in_workspace(workspace, type_table, ret)?
                ),
                _ => String::new(),
            };
            Ok(format!(
                "{{ fn __fol_uninit({}){return_clause} {{ unreachable!(\"uninitialized function pointer\") }} __fol_uninit as {rendered_type} }}",
                dummy_params.join(", ")
            ))
        }
    }
}

pub fn render_local_list(
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    locals: &[fol_lower::LoweredLocalId],
) -> BackendResult<String> {
    locals
        .iter()
        .map(|local| render_clone_expr(package_identity, routine, *local))
        .collect::<BackendResult<Vec<_>>>()
        .map(|items| items.join(", "))
}

pub fn render_clone_expr(
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> BackendResult<String> {
    let name = render_local_name(package_identity, routine, local_id)?;
    Ok(format!("{name}.clone()"))
}

pub fn render_transfer_expr(
    type_table: &LoweredTypeTable,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> BackendResult<String> {
    let name = render_local_name(package_identity, routine, local_id)?;
    let local_type = routine
        .locals
        .get(local_id)
        .and_then(|local| local.type_id)
        .and_then(|type_id| type_table.get(type_id));
    if let Some(LoweredType::Borrowed { mutable, .. }) = local_type {
        return Ok(if *mutable {
            format!("&mut *{name}")
        } else {
            name
        });
    }
    let moves = routine
        .locals
        .get(local_id)
        .and_then(|local| local.type_id)
        .is_some_and(|type_id| type_moves_on_transfer(type_table, type_id, 0));
    Ok(if moves {
        name
    } else {
        format!("{name}.clone()")
    })
}

fn type_moves_on_transfer(
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
    depth: usize,
) -> bool {
    if depth > 32 {
        return false;
    }
    match type_table.get(type_id) {
        Some(LoweredType::Owned { .. }) => true,
        Some(LoweredType::Pointer { shared: false, .. }) => true,
        Some(LoweredType::Eventual { .. }) => true,
        Some(LoweredType::Optional { inner }) | Some(LoweredType::Error { inner: Some(inner) }) => {
            type_moves_on_transfer(type_table, *inner, depth + 1)
        }
        _ => false,
    }
}

pub fn rendered_result_local(
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    instruction: &LoweredInstr,
) -> BackendResult<String> {
    let Some(local_id) = instruction.result else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!(
                "instruction {:?} does not have a result local",
                instruction.id
            ),
        ));
    };
    render_local_name(package_identity, routine, local_id)
}

pub fn render_local_name(
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> BackendResult<String> {
    let Some(local) = routine.locals.get(local_id) else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("lowered local {:?} is missing", local_id),
        ));
    };
    Ok(mangle_local_name(
        package_identity,
        routine.id,
        local_id,
        local.name.as_deref(),
    ))
}

pub fn render_operand(operand: &LoweredOperand) -> BackendResult<String> {
    match operand {
        LoweredOperand::Local(_) => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "unimplemented operand: Local",
        )),
        LoweredOperand::Global(_) => Err(BackendError::new(
            BackendErrorKind::Unsupported,
            "unimplemented operand: Global",
        )),
        LoweredOperand::Int(value) => Ok(format!("{value}_i64")),
        LoweredOperand::Float(bits) => Ok(format!("f64::from_bits({bits})")),
        LoweredOperand::Bool(value) => Ok(value.to_string()),
        LoweredOperand::Char(value) => Ok(format!("{value:?}")),
        LoweredOperand::Str(value) => Ok(format!("rt_model::FolStr::from({value:?})")),
        LoweredOperand::Nil => Ok("rt::FolOption::nil()".to_string()),
    }
}

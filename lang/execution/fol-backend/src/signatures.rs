use crate::{
    mangle_global_name, mangle_local_name, mangle_routine_name,
    render_core_instruction_in_workspace, render_rust_type_in_workspace, render_terminator,
    BackendError, BackendErrorKind, BackendResult,
};
use fol_lower::{
    LoweredBlockId, LoweredGlobal, LoweredRoutine, LoweredRoutineType, LoweredType, LoweredTypeId,
    LoweredTypeTable, LoweredWorkspace,
};
use fol_resolver::PackageIdentity;

fn collect_generic_params_from_type(
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
    params: &mut Vec<String>,
) {
    let Some(ty) = type_table.get(type_id) else {
        return;
    };
    match ty {
        LoweredType::GenericParameter { name } => {
            if !params.iter().any(|existing| existing == name) {
                params.push(name.clone());
            }
        }
        LoweredType::Array { element_type, .. }
        | LoweredType::Vector { element_type }
        | LoweredType::Sequence { element_type }
        | LoweredType::Optional { inner: element_type } => {
            collect_generic_params_from_type(type_table, *element_type, params);
        }
        LoweredType::Map {
            key_type,
            value_type,
        } => {
            collect_generic_params_from_type(type_table, *key_type, params);
            collect_generic_params_from_type(type_table, *value_type, params);
        }
        LoweredType::Error { inner } => {
            if let Some(inner) = inner {
                collect_generic_params_from_type(type_table, *inner, params);
            }
        }
        LoweredType::Set { member_types } => {
            for member in member_types {
                collect_generic_params_from_type(type_table, *member, params);
            }
        }
        LoweredType::Record { fields } => {
            for field in fields.values() {
                collect_generic_params_from_type(type_table, *field, params);
            }
        }
        LoweredType::Entry { variants } => {
            for variant in variants.values().flatten() {
                collect_generic_params_from_type(type_table, *variant, params);
            }
        }
        LoweredType::Routine(signature) => {
            for param in &signature.params {
                collect_generic_params_from_type(type_table, *param, params);
            }
            if let Some(ret) = signature.return_type {
                collect_generic_params_from_type(type_table, ret, params);
            }
            if let Some(err) = signature.error_type {
                collect_generic_params_from_type(type_table, err, params);
            }
        }
        LoweredType::Builtin(_) => {}
    }
}

fn render_generic_clause(signature: &LoweredRoutineType, type_table: &LoweredTypeTable) -> String {
    let mut params = Vec::new();
    for param in &signature.params {
        collect_generic_params_from_type(type_table, *param, &mut params);
    }
    if let Some(ret) = signature.return_type {
        collect_generic_params_from_type(type_table, ret, &mut params);
    }
    if let Some(err) = signature.error_type {
        collect_generic_params_from_type(type_table, err, &mut params);
    }
    if params.is_empty() {
        String::new()
    } else {
        format!(
            "<{}>",
            params
                .iter()
                .map(|name| format!(
                    "{}: Clone + Default",
                    crate::sanitize_backend_ident(name)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn recoverable_error_type_for_local(
    routine: &LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> Option<LoweredTypeId> {
    routine.instructions.iter().find_map(|instruction| match &instruction.kind {
        fol_lower::LoweredInstrKind::Call { error_type, .. }
        | fol_lower::LoweredInstrKind::CallIndirect { error_type, .. }
            if instruction.result == Some(local_id) =>
        {
            *error_type
        }
        _ => None,
    })
}

pub fn render_global_declaration(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    global: &LoweredGlobal,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let name = mangle_global_name(package_identity, global.id, &global.name);
    let value_type = render_rust_type_in_workspace(Some(workspace), type_table, global.type_id)?;

    Ok(if global.mutable {
        format!(
            "pub static {name}: std::sync::OnceLock<std::sync::Mutex<{value_type}>> = std::sync::OnceLock::new();\n"
        )
    } else {
        format!(
            "pub static {name}: std::sync::OnceLock<{value_type}> = std::sync::OnceLock::new();\n"
        )
    })
}

pub fn render_routine_signature(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let signature = routine_signature(type_table, routine.signature)?;

    let mut params = Vec::new();
    if let Some(receiver_type) = routine.receiver_type {
        let receiver_local_id = routine.params.first().copied().ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "receiver-qualified routine '{}' does not retain a receiver local slot",
                    routine.name
                ),
            )
        })?;
        let receiver_local = routine.locals.get(receiver_local_id).ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "receiver-qualified routine '{}' is missing its receiver local",
                    routine.name
                ),
            )
        })?;
        params.push(format!(
            "{}: {}",
            mangle_local_name(
                package_identity,
                routine.id,
                receiver_local_id,
                receiver_local.name.as_deref(),
            ),
            render_rust_type_in_workspace(Some(workspace), type_table, receiver_type)?
        ));
    }
    params.extend(render_param_list(
        workspace,
        package_identity,
        routine,
        signature,
        type_table,
    )?);

    let return_type = render_routine_return_type(workspace, signature, type_table)?;
    let generic_clause = render_generic_clause(signature, type_table);

    Ok(format!(
        "pub fn {}{}({}){}",
        mangle_routine_name(package_identity, routine.id, &routine.name),
        generic_clause,
        params.join(", "),
        return_type
    ))
}

pub fn render_routine_shell(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let header = render_routine_signature(workspace, package_identity, routine, type_table)?;
    let local_decls = routine
        .locals
        .iter_with_ids()
        .filter(|(local_id, _)| !routine.params.contains(local_id))
        .map(|(local_id, local)| {
            render_local_declaration(
                workspace,
                package_identity,
                routine,
                local_id,
                local,
                type_table,
            )
        })
        .collect::<BackendResult<Vec<_>>>()?
        .join("\n");
    let stub = format!(
        "    unreachable!(\"backend routine shell '{}' should not be executed\");",
        routine.name
    );

    Ok(if local_decls.is_empty() {
        format!("{header} {{\n{stub}\n}}\n")
    } else {
        format!("{header} {{\n{local_decls}\n{stub}\n}}\n")
    })
}

pub fn render_routine_definition(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let header = render_routine_signature(workspace, package_identity, routine, type_table)?;
    let local_decls = routine
        .locals
        .iter_with_ids()
        .filter(|(local_id, _)| !routine.params.contains(local_id))
        .map(|(local_id, local)| {
            render_local_declaration(
                workspace,
                package_identity,
                routine,
                local_id,
                local,
                type_table,
            )
        })
        .collect::<BackendResult<Vec<_>>>()?
        .join("\n");
    let rendered_blocks = routine
        .blocks
        .iter_with_ids()
        .map(|(block_id, block)| {
            render_block(
                workspace,
                package_identity,
                routine,
                block_id,
                block,
                type_table,
            )
        })
        .collect::<BackendResult<Vec<_>>>()?
        .join("\n");

    let local_section = if local_decls.is_empty() {
        String::new()
    } else {
        format!("{local_decls}\n")
    };

    Ok(format!(
        "{header} {{\n{local_section}    let mut __fol_next_block: usize = {};\n    loop {{\n        match __fol_next_block {{\n{rendered_blocks}\n            _ => unreachable!(\"invalid lowered block {{}}\", __fol_next_block),\n        }}\n    }}\n}}\n",
        routine.entry_block.0
    ))
}

fn routine_signature<'a>(
    type_table: &'a LoweredTypeTable,
    signature_id: Option<LoweredTypeId>,
) -> BackendResult<&'a LoweredRoutineType> {
    let Some(signature_id) = signature_id else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            "routine is missing a lowered signature",
        ));
    };
    match type_table.get(signature_id) {
        Some(LoweredType::Routine(signature)) => Ok(signature),
        Some(other) => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("routine signature id did not point to a routine type: {other:?}"),
        )),
        None => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("routine signature type {:?} is missing", signature_id),
        )),
    }
}

fn render_param_list(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    signature: &LoweredRoutineType,
    type_table: &LoweredTypeTable,
) -> BackendResult<Vec<String>> {
    let param_ids = if routine.receiver_type.is_some() {
        routine.params.get(1..).ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "receiver-qualified routine '{}' does not retain a receiver local slot",
                    routine.name
                ),
            )
        })?
    } else {
        routine.params.as_slice()
    };

    param_ids
        .iter()
        .enumerate()
        .map(|(index, local_id)| {
            let local = routine.locals.get(*local_id).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("routine parameter local {:?} is missing", local_id),
                )
            })?;
            let type_id = signature.params.get(index).copied().ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("routine parameter {} is missing a signature type", index),
                )
            })?;
            Ok(format!(
                "{}: {}",
                mangle_local_name(
                    package_identity,
                    routine.id,
                    *local_id,
                    local.name.as_deref()
                ),
                render_rust_type_in_workspace(Some(workspace), type_table, type_id)?
            ))
        })
        .collect()
}

fn render_local_declaration(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
    local: &fol_lower::LoweredLocal,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let rendered_type = match (local.type_id, recoverable_error_type_for_local(routine, local_id)) {
        (Some(type_id), Some(error_type)) => format!(
            "rt::FolRecover<{}, {}>",
            render_rust_type_in_workspace(Some(workspace), type_table, type_id)?,
            render_rust_type_in_workspace(Some(workspace), type_table, error_type)?,
        ),
        (Some(type_id), None) => {
            render_rust_type_in_workspace(Some(workspace), type_table, type_id)?
        }
        (None, _) => "_".to_string(),
    };
    let initializer = match local.type_id.and_then(|id| type_table.get(id)) {
        Some(fol_lower::LoweredType::Routine(routine_type)) => {
            let dummy_params: Vec<String> = routine_type
                .params
                .iter()
                .enumerate()
                .map(|(i, param_id)| {
                    render_rust_type_in_workspace(Some(workspace), type_table, *param_id)
                        .map(|ty| format!("_p{i}: {ty}"))
                })
                .collect::<BackendResult<Vec<_>>>()?;
            let return_clause = match (routine_type.return_type, routine_type.error_type) {
                (Some(ret), Some(err)) => format!(
                    " -> rt::FolRecover<{}, {}>",
                    render_rust_type_in_workspace(Some(workspace), type_table, ret)?,
                    render_rust_type_in_workspace(Some(workspace), type_table, err)?
                ),
                (Some(ret), None) => format!(
                    " -> {}",
                    render_rust_type_in_workspace(Some(workspace), type_table, ret)?
                ),
                _ => String::new(),
            };
            format!(
                "{{ fn __fol_uninit({}){return_clause} {{ unreachable!(\"uninitialized function pointer\") }} __fol_uninit as {rendered_type} }}",
                dummy_params.join(", ")
            )
        }
        _ => "Default::default()".to_string(),
    };
    Ok(format!(
        "    let mut {}: {} = {};",
        mangle_local_name(
            package_identity,
            routine.id,
            local_id,
            local.name.as_deref()
        ),
        rendered_type,
        initializer
    ))
}

fn render_block(
    workspace: &LoweredWorkspace,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    block_id: LoweredBlockId,
    block: &fol_lower::LoweredBlock,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let instructions = block
        .instructions
        .iter()
        .map(|instruction_id| {
            let instruction = routine.instructions.get(*instruction_id).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("lowered instruction {:?} is missing", instruction_id),
                )
            })?;
            Ok(format!(
                "                {}",
                render_core_instruction_in_workspace(
                    Some(workspace),
                    package_identity,
                    type_table,
                    routine,
                    instruction,
                )?
            ))
        })
        .collect::<BackendResult<Vec<_>>>()?
        .join("\n");
    let terminator = block.terminator.as_ref().ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("lowered block {:?} is missing a terminator", block_id),
        )
    })?;
    let rendered_terminator = format!(
        "                {}",
        render_terminator(package_identity, type_table, routine, terminator)?
    );
    let body = if instructions.is_empty() {
        rendered_terminator
    } else {
        format!("{instructions}\n{rendered_terminator}")
    };

    Ok(format!(
        "            {} => {{\n{body}\n            }},",
        block_id.0
    ))
}

fn render_routine_return_type(
    workspace: &LoweredWorkspace,
    signature: &LoweredRoutineType,
    type_table: &LoweredTypeTable,
) -> BackendResult<String> {
    let success_type = match signature.return_type {
        Some(return_type) => {
            render_rust_type_in_workspace(Some(workspace), type_table, return_type)?
        }
        None => "()".to_string(),
    };
    Ok(match signature.error_type {
        Some(error_type) => format!(
            " -> rt::FolRecover<{}, {}>",
            success_type,
            render_rust_type_in_workspace(Some(workspace), type_table, error_type)?
        ),
        None if signature.return_type.is_some() => format!(" -> {success_type}"),
        None => String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::{render_global_declaration, render_routine_shell, render_routine_signature};
    use crate::testing::{package_identity, sample_lowered_workspace};
    use fol_lower::{
        LoweredBlockId, LoweredBuiltinType, LoweredGlobal, LoweredGlobalId, LoweredLocal,
        LoweredLocalId, LoweredRoutine, LoweredRoutineId, LoweredRoutineType, LoweredType,
        LoweredTypeTable,
    };
    use fol_resolver::{PackageSourceKind, SourceUnitId, SymbolId};

    #[test]
    fn global_declaration_rendering_emits_lazy_shells_for_mutable_and_immutable_globals() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let immutable = LoweredGlobal {
            id: LoweredGlobalId(0),
            symbol_id: SymbolId(20),
            source_unit_id: SourceUnitId(0),
            name: "answer".to_string(),
            type_id: int_id,
            mutable: false,
        };
        let mutable = LoweredGlobal {
            id: LoweredGlobalId(1),
            symbol_id: SymbolId(21),
            source_unit_id: SourceUnitId(0),
            name: "counter".to_string(),
            type_id: int_id,
            mutable: true,
        };
        let workspace = sample_lowered_workspace();

        let immutable_rendered =
            render_global_declaration(&workspace, &package_identity, &immutable, &table)
                .expect("global");
        let mutable_rendered =
            render_global_declaration(&workspace, &package_identity, &mutable, &table)
                .expect("global");

        assert!(immutable_rendered.contains("pub static g__pkg__entry__app__g0__answer"));
        assert!(immutable_rendered.contains("std::sync::OnceLock<rt::FolInt>"));
        assert!(mutable_rendered.contains("pub static g__pkg__entry__app__g1__counter"));
        assert!(mutable_rendered.contains("std::sync::OnceLock<std::sync::Mutex<rt::FolInt>>"));
    }

    #[test]
    fn routine_signature_rendering_covers_plain_and_receiver_qualified_routines() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let signature_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![bool_id],
            return_type: Some(int_id),
            error_type: None,
        }));
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let workspace = sample_lowered_workspace();
        let mut plain = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
        plain.signature = Some(signature_id);
        let local_id = plain.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(bool_id),
            name: Some("flag".to_string()),
        });
        plain.params.push(local_id);

        let mut method = LoweredRoutine::new(LoweredRoutineId(1), "tick", LoweredBlockId(0));
        method.signature = Some(signature_id);
        method.receiver_type = Some(int_id);
        let receiver_slot = method.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(int_id),
            name: Some("self".to_string()),
        });
        method.params.push(receiver_slot);
        let method_param = method.locals.push(LoweredLocal {
            id: LoweredLocalId(1),
            type_id: Some(bool_id),
            name: Some("flag".to_string()),
        });
        method.params.push(method_param);

        let plain_rendered =
            render_routine_signature(&workspace, &package_identity, &plain, &table)
                .expect("signature");
        let method_rendered =
            render_routine_signature(&workspace, &package_identity, &method, &table)
                .expect("signature");

        assert!(plain_rendered.contains("pub fn r__pkg__entry__app__r0__main("));
        assert!(plain_rendered.contains("l__pkg__entry__app__r0__l0__flag: rt::FolBool"));
        assert!(plain_rendered.ends_with(" -> rt::FolInt"));
        assert!(method_rendered.contains("receiver: rt::FolInt"));
        assert!(method_rendered.contains("l__pkg__entry__app__r1__l1__flag: rt::FolBool"));
    }

    #[test]
    fn routine_signature_rendering_wraps_recoverable_returns_in_runtime_abi() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let signature_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![],
            return_type: Some(int_id),
            error_type: Some(str_id),
        }));
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let workspace = sample_lowered_workspace();
        let mut routine = LoweredRoutine::new(LoweredRoutineId(2), "load", LoweredBlockId(0));
        routine.signature = Some(signature_id);

        let rendered = render_routine_signature(&workspace, &package_identity, &routine, &table)
            .expect("signature");

        assert!(rendered.contains("pub fn r__pkg__entry__app__r2__load()"));
        assert!(rendered.ends_with(" -> rt::FolRecover<rt::FolInt, rt_model::FolStr>"));
    }

    #[test]
    fn routine_shell_rendering_adds_non_param_locals_as_backend_frame_slots() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let signature_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![bool_id],
            return_type: Some(int_id),
            error_type: None,
        }));
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let workspace = sample_lowered_workspace();
        let mut routine = LoweredRoutine::new(LoweredRoutineId(3), "compute", LoweredBlockId(0));
        routine.signature = Some(signature_id);
        let param_id = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(bool_id),
            name: Some("flag".to_string()),
        });
        let temp_id = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(1),
            type_id: Some(int_id),
            name: Some("temp".to_string()),
        });
        routine.params.push(param_id);

        let rendered = render_routine_shell(&workspace, &package_identity, &routine, &table)
            .expect("routine shell");

        assert!(rendered.contains("pub fn r__pkg__entry__app__r3__compute("));
        assert!(rendered.contains(
            "let mut l__pkg__entry__app__r3__l1__temp: rt::FolInt = Default::default();"
        ));
        assert!(!rendered
            .contains("l__pkg__entry__app__r3__l0__flag: rt::FolInt = Default::default();"));
        assert!(rendered.contains(
            "unreachable!(\"backend routine shell 'compute' should not be executed\")"
        ));
        assert_eq!(temp_id, LoweredLocalId(1));
    }

    #[test]
    fn combined_global_and_routine_signature_snapshot_stays_stable() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let signature_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![bool_id],
            return_type: Some(int_id),
            error_type: Some(str_id),
        }));
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let global = LoweredGlobal {
            id: LoweredGlobalId(0),
            symbol_id: SymbolId(20),
            source_unit_id: SourceUnitId(0),
            name: "answer".to_string(),
            type_id: int_id,
            mutable: false,
        };
        let mut routine = LoweredRoutine::new(LoweredRoutineId(4), "load", LoweredBlockId(0));
        routine.signature = Some(signature_id);
        let param_id = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(bool_id),
            name: Some("flag".to_string()),
        });
        let local_id = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(1),
            type_id: Some(int_id),
            name: Some("value".to_string()),
        });
        routine.params.push(param_id);

        let workspace = sample_lowered_workspace();
        let snapshot = [
            render_global_declaration(&workspace, &package_identity, &global, &table).expect("global"),
            render_routine_signature(&workspace, &package_identity, &routine, &table).expect("signature"),
            render_routine_shell(&workspace, &package_identity, &routine, &table).expect("shell"),
        ]
        .join("\n");

        assert!(snapshot.contains("g__pkg__entry__app__g0__answer"));
        assert!(snapshot.contains("r__pkg__entry__app__r4__load"));
        assert!(snapshot.contains("rt::FolRecover<rt::FolInt, rt_model::FolStr>"));
        assert!(snapshot.contains("l__pkg__entry__app__r4__l0__flag: rt::FolBool"));
        assert!(snapshot.contains(
            "let mut l__pkg__entry__app__r4__l1__value: rt::FolInt = Default::default();"
        ));
        assert_eq!(local_id, LoweredLocalId(1));
    }

    #[test]
    fn routine_shell_rendering_emits_unreachable_stub_for_function_pointer_locals() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let fn_type_id = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![int_id],
            return_type: Some(int_id),
            error_type: None,
        }));
        let outer_sig = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![],
            return_type: Some(int_id),
            error_type: None,
        }));
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let workspace = sample_lowered_workspace();
        let mut routine = LoweredRoutine::new(LoweredRoutineId(5), "caller", LoweredBlockId(0));
        routine.signature = Some(outer_sig);
        let _fn_local = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(fn_type_id),
            name: Some("callback".to_string()),
        });

        let rendered = render_routine_shell(&workspace, &package_identity, &routine, &table)
            .expect("routine shell with fn pointer");

        assert!(rendered.contains("l__pkg__entry__app__r5__l0__callback"));
        assert!(rendered.contains("unreachable!(\"uninitialized function pointer\")"));
        assert!(rendered.contains("__fol_uninit"));
    }
}

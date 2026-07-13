use crate::{BackendError, BackendErrorKind, BackendResult};
use fol_intrinsics::intrinsic_by_id;
use fol_lower::{
    control::{LoweredBinaryOp, LoweredLinearKind, LoweredUnaryOp},
    LoweredInstr, LoweredInstrKind, LoweredRoutine, LoweredType, LoweredTypeTable,
    LoweredWorkspace,
};
use fol_resolver::PackageIdentity;

use super::helpers::{
    render_global_load, render_local_list, render_local_name, render_mutex_guard_name,
    render_namespace_module_path, render_native_intrinsic_expression, render_operand,
    render_routine_path, render_transfer_expr, render_type_default_expr_in_workspace,
    render_type_path, rendered_result_local, resolve_global_decl, resolve_routine_decl,
    resolve_type_decl,
};

pub fn render_core_instruction(
    package_identity: &PackageIdentity,
    type_table: &LoweredTypeTable,
    routine: &LoweredRoutine,
    instruction: &LoweredInstr,
) -> BackendResult<String> {
    render_core_instruction_in_workspace(None, package_identity, type_table, routine, instruction)
}

fn render_call_arguments(
    type_table: &LoweredTypeTable,
    package_identity: &PackageIdentity,
    caller: &LoweredRoutine,
    callee: &LoweredRoutine,
    args: &[fol_lower::LoweredLocalId],
) -> BackendResult<String> {
    args.iter()
        .enumerate()
        .map(|(index, local_id)| {
            let callee_param = callee.params.get(index).copied();
            if callee_param.is_some_and(|param| callee.mutex_params.contains(&param)) {
                let name = render_local_name(package_identity, caller, *local_id)?;
                if caller.mutex_params.contains(local_id) {
                    Ok(format!("{name}.clone()"))
                } else {
                    Ok(format!("rt::FolMutex::from_value({name}.clone())"))
                }
            } else {
                render_transfer_expr(type_table, package_identity, caller, *local_id)
            }
        })
        .collect::<BackendResult<Vec<_>>>()
        .map(|args| args.join(", "))
}

pub fn render_core_instruction_in_workspace(
    workspace: Option<&LoweredWorkspace>,
    package_identity: &PackageIdentity,
    type_table: &LoweredTypeTable,
    routine: &LoweredRoutine,
    instruction: &LoweredInstr,
) -> BackendResult<String> {
    match &instruction.kind {
        LoweredInstrKind::ConstraintCall { method, .. } => {
            return Err(BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                "constraint call '{method}' reached backend emission without being monomorphized"
            ),
            ))
        }
        LoweredInstrKind::Const(operand) => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            Ok(format!("{result} = {};", render_operand(operand)?))
        }
        LoweredInstrKind::LoadLocal { local } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let source = render_transfer_expr(type_table, package_identity, routine, *local)?;
            Ok(format!("{result} = {source};"))
        }
        LoweredInstrKind::StoreLocal { local, value } => {
            let target = render_local_name(package_identity, routine, *local)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            Ok(format!("{target} = {value};"))
        }
        LoweredInstrKind::DropLocal { local } => {
            let local = render_local_name(package_identity, routine, *local)?;
            Ok(format!("drop({local});"))
        }
        LoweredInstrKind::StoreField { base, field, value } => {
            let base_id = *base;
            let base = render_local_name(package_identity, routine, base_id)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            let field = crate::escape_rust_field_ident(field);
            if routine.mutex_params.contains(&base_id) {
                let guard = render_mutex_guard_name(base_id);
                Ok(format!(
                    "{guard}.as_mut().expect(\"mutex field assignment requires .lock()\").{field} = {value};"
                ))
            } else {
                Ok(format!("{base}.{field} = {value};"))
            }
        }
        LoweredInstrKind::LoadGlobal { global } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let (global_identity, global_decl) = resolve_global_decl(workspace, *global)?;
            Ok(format!(
                "{result} = {};",
                render_global_load(workspace, type_table, global_identity, global_decl)?
            ))
        }
        LoweredInstrKind::StoreGlobal { global, value } => {
            let (global_identity, global_decl) = resolve_global_decl(workspace, *global)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            if !global_decl.mutable {
                return Err(BackendError::new(
                    BackendErrorKind::Unsupported,
                    format!(
                        "store emission is not implemented for immutable global '{}'",
                        global_decl.name
                    ),
                ));
            }
            let global_path = format!(
                "{}::{}",
                render_namespace_module_path(
                    workspace,
                    global_identity,
                    global_decl.source_unit_id
                )?,
                crate::mangle_global_name(global_identity, *global, &global_decl.name)
            );
            let default_expr =
                render_type_default_expr_in_workspace(workspace, type_table, global_decl.type_id)?;
            Ok(format!(
                "*{global_path}.get_or_init(|| std::sync::Mutex::new({default_expr})).lock().unwrap_or_else(|e| e.into_inner()) = {value};",
            ))
        }
        LoweredInstrKind::Call {
            callee,
            args,
            error_type: None,
        } => {
            let (callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let rendered_args =
                render_call_arguments(type_table, package_identity, routine, callee_decl, args)?;
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            match instruction.result {
                Some(_) => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = {callee_name}({rendered_args});"))
                }
                None => Ok(format!("{callee_name}({rendered_args});")),
            }
        }
        LoweredInstrKind::Call {
            callee,
            args,
            error_type: Some(_),
        } => {
            let (callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let rendered_args =
                render_call_arguments(type_table, package_identity, routine, callee_decl, args)?;
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            match instruction.result {
                Some(_) => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = {callee_name}({rendered_args});"))
                }
                None => Ok(format!("{callee_name}({rendered_args});")),
            }
        }
        LoweredInstrKind::SpawnCall { callee, args } => {
            let (callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let rendered_args =
                render_call_arguments(type_table, package_identity, routine, callee_decl, args)?;
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            Ok(format!(
                "rt::spawn_task(move || {{ let _ = {callee_name}({rendered_args}); }});"
            ))
        }
        LoweredInstrKind::AsyncCall { callee, args, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let (callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let rendered_args =
                render_call_arguments(type_table, package_identity, routine, callee_decl, args)?;
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            Ok(format!(
                "{result} = rt::spawn_eventual(move || {callee_name}({rendered_args}));"
            ))
        }
        LoweredInstrKind::AwaitEventual { eventual, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let eventual = render_local_name(package_identity, routine, *eventual)?;
            Ok(format!("{result} = {eventual}.await_value();"))
        }
        LoweredInstrKind::ChannelSender { channel } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let channel = render_local_name(package_identity, routine, *channel)?;
            Ok(format!(
                "{result} = {channel}.acquire_sender().expect(\"channel transmitter must be acquired before receiver use\");"
            ))
        }
        LoweredInstrKind::ChannelSend { channel, value } => {
            let channel = render_local_name(package_identity, routine, *channel)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            Ok(format!(
                "{channel}.send({value}).unwrap_or_else(|_| panic!(\"channel send requires an open receiver\"));"
            ))
        }
        LoweredInstrKind::ChannelReceive { channel } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let channel = render_local_name(package_identity, routine, *channel)?;
            Ok(format!("{result} = {channel}.receive();"))
        }
        LoweredInstrKind::ChannelReceiveOptional { channel } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let channel = render_local_name(package_identity, routine, *channel)?;
            Ok(format!("{result} = {channel}.receive_optional();"))
        }
        LoweredInstrKind::ChannelTryReceive { channel } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let channel = render_local_name(package_identity, routine, *channel)?;
            Ok(format!("{result} = {channel}.try_receive();"))
        }
        LoweredInstrKind::ChannelIsClosed { channel } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let channel = render_local_name(package_identity, routine, *channel)?;
            Ok(format!("{result} = {channel}.is_closed();"))
        }
        LoweredInstrKind::ProcessorYield => Ok("rt::yield_processor();".to_string()),
        LoweredInstrKind::MutexLock { mutex } => {
            let mutex_id = *mutex;
            let mutex = render_local_name(package_identity, routine, mutex_id)?;
            let guard = render_mutex_guard_name(mutex_id);
            Ok(format!("{guard} = Some({mutex}.lock());"))
        }
        LoweredInstrKind::MutexUnlock { mutex } => {
            let guard = render_mutex_guard_name(*mutex);
            Ok(format!("drop({guard}.take());"))
        }
        LoweredInstrKind::OptionalHasValue { operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand = render_local_name(package_identity, routine, *operand)?;
            Ok(format!("{result} = {operand}.is_some();"))
        }
        LoweredInstrKind::FieldAccess { base, field } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let result_moves = instruction
                .result
                .and_then(|local_id| routine.locals.get(local_id))
                .and_then(|local| local.type_id)
                .is_some_and(|type_id| type_table.moves_on_transfer(type_id));
            let base_id = *base;
            let base = render_local_name(package_identity, routine, base_id)?;
            let field = crate::escape_rust_field_ident(field);
            if routine.mutex_params.contains(&base_id) {
                if result_moves {
                    return Err(BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "move-only fields cannot be transferred out of a mutex guard",
                    ));
                }
                let guard = render_mutex_guard_name(base_id);
                Ok(format!(
                    "{result} = {guard}.as_ref().expect(\"mutex field access requires .lock()\").{field}.clone();"
                ))
            } else if result_moves {
                Ok(format!("{result} = {base}.{field};"))
            } else {
                Ok(format!("{result} = {base}.{field}.clone();"))
            }
        }
        LoweredInstrKind::IntrinsicCall { intrinsic, args } => {
            let entry = intrinsic_by_id(*intrinsic).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("intrinsic id {:?} is not registered", intrinsic),
                )
            })?;
            let rendered_args = args
                .iter()
                .map(|local_id| render_local_name(package_identity, routine, *local_id))
                .collect::<BackendResult<Vec<_>>>()?;
            let expression = render_native_intrinsic_expression(entry.name, &rendered_args)?;
            match instruction.result {
                Some(_) => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = {expression};"))
                }
                None => Ok(format!("{expression};")),
            }
        }
        LoweredInstrKind::LengthOf { operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand = render_local_name(package_identity, routine, *operand)?;
            Ok(format!("{result} = rt::len(&{operand});"))
        }
        LoweredInstrKind::RuntimeHook { intrinsic, args } => {
            let entry = intrinsic_by_id(*intrinsic).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("intrinsic id {:?} is not registered", intrinsic),
                )
            })?;
            match (entry.name, args.as_slice()) {
                ("echo", [value]) => {
                    let value =
                        render_transfer_expr(type_table, package_identity, routine, *value)?;
                    let rendered = format!("rt::echo({value})");
                    match instruction.result {
                        Some(_) => {
                            let result =
                                rendered_result_local(package_identity, routine, instruction)?;
                            Ok(format!("{result} = {rendered};"))
                        }
                        None => Ok(format!("{rendered};")),
                    }
                }
                (other, _) => Err(BackendError::new(
                    BackendErrorKind::Unsupported,
                    format!("runtime hook emission is not implemented yet for '.{other}(...)'"),
                )),
            }
        }
        LoweredInstrKind::CheckRecoverable { operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand = render_local_name(package_identity, routine, *operand)?;
            Ok(format!("{result} = rt::check_recoverable(&{operand});"))
        }
        LoweredInstrKind::UnwrapRecoverable { operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand = render_local_name(package_identity, routine, *operand)?;
            Ok(format!(
                "{result} = std::mem::take(&mut {operand}).into_value().expect(\"unwrap of recoverable value failed: result contains an error\");"
            ))
        }
        LoweredInstrKind::ExtractRecoverableError { operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand = render_local_name(package_identity, routine, *operand)?;
            Ok(format!(
                "{result} = std::mem::take(&mut {operand}).into_error().expect(\"extract of recoverable error failed: result contains a value\");"
            ))
        }
        LoweredInstrKind::ConstructOptional { value, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let expression = match value {
                Some(value) => {
                    let value =
                        render_transfer_expr(type_table, package_identity, routine, *value)?;
                    format!("rt::FolOption::some({value})")
                }
                None => "rt::FolOption::nil()".to_string(),
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::ConstructOwned { value, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let value = render_local_name(package_identity, routine, *value)?;
            Ok(format!("{result} = Box::new({value});"))
        }
        LoweredInstrKind::ConsumeOwned { value } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let value = render_local_name(package_identity, routine, *value)?;
            Ok(format!("{result} = *{value};"))
        }
        LoweredInstrKind::ConstructBorrow { owner, mutable, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let owner = render_local_name(package_identity, routine, *owner)?;
            Ok(format!(
                "{result} = &{}{owner};",
                if *mutable { "mut " } else { "" }
            ))
        }
        LoweredInstrKind::ReadBorrow { borrow } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let borrow = render_local_name(package_identity, routine, *borrow)?;
            Ok(format!("{result} = (*{borrow}).clone();"))
        }
        LoweredInstrKind::ConstructPointer { value, shared, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            Ok(format!(
                "{result} = {}::new({value});",
                if *shared { "std::rc::Rc" } else { "Box" }
            ))
        }
        LoweredInstrKind::DerefPointer { pointer } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let pointer = render_local_name(package_identity, routine, *pointer)?;
            Ok(format!("{result} = (*{pointer}).clone();"))
        }
        LoweredInstrKind::StoreDeref { pointer, value } => {
            let pointer = render_local_name(package_identity, routine, *pointer)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            Ok(format!("*{pointer} = {value};"))
        }
        LoweredInstrKind::GiveBackBorrow { borrow } => {
            let borrow = render_local_name(package_identity, routine, *borrow)?;
            Ok(format!("drop({borrow});"))
        }
        LoweredInstrKind::ConstructError { value, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let expression = match value {
                Some(value) => {
                    let value =
                        render_transfer_expr(type_table, package_identity, routine, *value)?;
                    format!("rt::FolError::new({value})")
                }
                // Leave the payload type to inference from the assignment
                // target, exactly like FolOption::nil().
                None => "rt::FolError::default()".to_string(),
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::UnwrapShell { operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand_name = render_local_name(package_identity, routine, *operand)?;
            let operand_local = routine.locals.get(*operand).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("lowered local {:?} is missing", operand),
                )
            })?;
            let Some(type_id) = operand_local.type_id else {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "shell operand local {:?} does not have a lowered type",
                        operand
                    ),
                ));
            };
            let operand = if type_table.moves_on_transfer(type_id) {
                operand_name
            } else {
                format!("{operand_name}.clone()")
            };
            let expression = match type_table.get(type_id) {
                Some(LoweredType::Optional { .. }) => {
                    format!("rt::unwrap_optional_shell({operand}).unwrap()")
                }
                Some(LoweredType::Error { .. }) => {
                    format!("rt::unwrap_error_shell({operand})")
                }
                other => {
                    return Err(BackendError::new(
                        BackendErrorKind::InvalidInput,
                        format!(
                        "shell unwrap emission expected optional/error local but found {other:?}"
                    ),
                    ))
                }
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::ConstructLinear { kind, elements, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let elements = render_local_list(type_table, package_identity, routine, elements)?;
            let expression = match kind {
                LoweredLinearKind::Array => format!("[{elements}]"),
                LoweredLinearKind::Vector => {
                    format!("rt_model::FolVec::from_items(vec![{elements}])")
                }
                LoweredLinearKind::Sequence => {
                    format!("rt_model::FolSeq::from_items(vec![{elements}])")
                }
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::ConstructSet { members, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let members = render_local_list(type_table, package_identity, routine, members)?;
            Ok(format!(
                "{result} = rt_model::FolSet::from_items(vec![{members}]);"
            ))
        }
        LoweredInstrKind::ConstructMap { entries, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let entries = entries
                .iter()
                .map(|(key, value)| {
                    Ok(format!(
                        "({}, {})",
                        render_transfer_expr(type_table, package_identity, routine, *key)?,
                        render_transfer_expr(type_table, package_identity, routine, *value)?
                    ))
                })
                .collect::<BackendResult<Vec<_>>>()?
                .join(", ");
            Ok(format!(
                "{result} = rt_model::FolMap::from_pairs(vec![{entries}]);"
            ))
        }
        LoweredInstrKind::IndexAccess { container, index } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let container_name = render_local_name(package_identity, routine, *container)?;
            let index_name = render_local_name(package_identity, routine, *index)?;
            let container_local = routine.locals.get(*container).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("lowered local {:?} is missing", container),
                )
            })?;
            let Some(type_id) = container_local.type_id else {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "index container local {:?} does not have a lowered type",
                        container
                    ),
                ));
            };
            let expression = match type_table.get(type_id) {
                Some(LoweredType::Array { .. }) => format!(
                    "rt::index_array(&{container_name}, {index_name}.clone()).unwrap().clone()"
                ),
                Some(LoweredType::Vector { .. }) => format!(
                    "rt::index_vec(&{container_name}, {index_name}.clone()).unwrap().clone()"
                ),
                Some(LoweredType::Sequence { .. }) => format!(
                    "rt::index_seq(&{container_name}, {index_name}.clone()).unwrap().clone()"
                ),
                Some(LoweredType::Map { .. }) => format!(
                    "rt::lookup_map(&{container_name}, &{index_name}).unwrap().clone()"
                ),
                other => {
                    return Err(BackendError::new(
                        BackendErrorKind::InvalidInput,
                        format!("index emission expected array/vector/sequence/map local but found {other:?}"),
                    ))
                }
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::SliceAccess {
            container,
            start,
            end,
        } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let container_name = render_local_name(package_identity, routine, *container)?;
            let start_name = render_local_name(package_identity, routine, *start)?;
            let end_name = render_local_name(package_identity, routine, *end)?;
            let container_local = routine.locals.get(*container).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("lowered local {:?} is missing", container),
                )
            })?;
            let Some(type_id) = container_local.type_id else {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "slice container local {:?} does not have a lowered type",
                        container
                    ),
                ));
            };
            let expression = match type_table.get(type_id) {
                Some(LoweredType::Vector { .. }) => format!(
                    "rt::slice_vec(&{container_name}, {start_name}.clone(), {end_name}.clone()).unwrap()"
                ),
                Some(LoweredType::Sequence { .. }) => format!(
                    "rt::slice_seq(&{container_name}, {start_name}.clone(), {end_name}.clone()).unwrap()"
                ),
                other => {
                    return Err(BackendError::new(
                        BackendErrorKind::InvalidInput,
                        format!("slice emission expected vector/sequence local but found {other:?}"),
                    ))
                }
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::ConstructRecord { type_id, fields } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let (type_identity, type_decl) = resolve_type_decl(workspace, *type_id)?;
            let type_name = render_type_path(workspace, type_identity, type_decl)?;
            let rendered_fields = fields
                .iter()
                .map(|(field, local)| {
                    Ok(format!(
                        "{}: {}",
                        crate::escape_rust_field_ident(field),
                        render_transfer_expr(type_table, package_identity, routine, *local)?
                    ))
                })
                .collect::<BackendResult<Vec<_>>>()?
                .join(", ");
            Ok(format!("{result} = {type_name} {{ {rendered_fields} }};"))
        }
        LoweredInstrKind::ConstructEntry {
            type_id,
            variant,
            payload,
        } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let (type_identity, type_decl) = resolve_type_decl(workspace, *type_id)?;
            let type_name = render_type_path(workspace, type_identity, type_decl)?;
            let variant = crate::escape_rust_field_ident(variant);
            let expression = match payload {
                Some(payload) => format!(
                    "{type_name}::{variant}({})",
                    render_transfer_expr(type_table, package_identity, routine, *payload)?
                ),
                None => format!("{type_name}::{variant}"),
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::BinaryOp { op, left, right } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let left_id = *left;
            let left = render_local_name(package_identity, routine, left_id)?;
            let right = render_local_name(package_identity, routine, *right)?;
            let expression = match op {
                LoweredBinaryOp::Add => format!("{left} + {right}"),
                LoweredBinaryOp::Sub => format!("{left} - {right}"),
                LoweredBinaryOp::Mul => format!("{left} * {right}"),
                LoweredBinaryOp::Div => format!("{left} / {right}"),
                LoweredBinaryOp::Mod => format!("{left} % {right}"),
                LoweredBinaryOp::Pow => {
                    let left_local = routine.locals.get(left_id).ok_or_else(|| {
                        BackendError::new(
                            BackendErrorKind::InvalidInput,
                            format!("lowered local {:?} is missing", left_id),
                        )
                    })?;
                    if let Some(type_id) = left_local.type_id {
                        if matches!(
                            type_table.get(type_id),
                            Some(LoweredType::Builtin(fol_lower::LoweredBuiltinType::Float))
                        ) {
                            format!("rt::pow_float({left}, {right})")
                        } else {
                            format!("rt::pow({left}, {right})")
                        }
                    } else {
                        format!("rt::pow({left}, {right})")
                    }
                }
                LoweredBinaryOp::Eq => format!("{left} == {right}"),
                LoweredBinaryOp::Ne => format!("{left} != {right}"),
                LoweredBinaryOp::Lt => format!("{left} < {right}"),
                LoweredBinaryOp::Le => format!("{left} <= {right}"),
                LoweredBinaryOp::Gt => format!("{left} > {right}"),
                LoweredBinaryOp::Ge => format!("{left} >= {right}"),
                LoweredBinaryOp::And => format!("{left} && {right}"),
                LoweredBinaryOp::Or => format!("{left} || {right}"),
                LoweredBinaryOp::Xor => format!("{left} ^ {right}"),
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::UnaryOp { op, operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand = render_local_name(package_identity, routine, *operand)?;
            let expression = match op {
                LoweredUnaryOp::Neg => format!("-{operand}"),
                LoweredUnaryOp::Not => format!("!{operand}"),
            };
            Ok(format!("{result} = {expression};"))
        }
        LoweredInstrKind::Cast {
            operand,
            target_type,
        } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand = render_local_name(package_identity, routine, *operand)?;
            let target =
                crate::types::render_rust_type_in_workspace(workspace, type_table, *target_type)?;
            Ok(format!("{result} = {operand} as {target};"))
        }
        LoweredInstrKind::RoutineRef { routine: callee } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let (callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            let callee_signature = callee_decl.signature.ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "routine '{}' is missing a lowered signature",
                        callee_decl.name
                    ),
                )
            })?;
            let fn_type = crate::types::render_rust_type_in_workspace(
                workspace,
                type_table,
                callee_signature,
            )?;
            Ok(format!("{result} = {callee_name} as {fn_type};"))
        }
        LoweredInstrKind::CallIndirect {
            callee,
            args,
            error_type: _,
        } => {
            let callee_name = render_local_name(package_identity, routine, *callee)?;
            let rendered_args = args
                .iter()
                .map(|local_id| render_local_name(package_identity, routine, *local_id))
                .collect::<BackendResult<Vec<_>>>()?
                .join(", ");
            match instruction.result {
                Some(_) => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = {callee_name}({rendered_args});"))
                }
                None => Ok(format!("{callee_name}({rendered_args});")),
            }
        }
    }
}

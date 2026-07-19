use crate::{BackendError, BackendErrorKind, BackendResult};
use fol_intrinsics::intrinsic_by_id;
use fol_lower::{
    control::{LoweredBinaryOp, LoweredLinearKind, LoweredUnaryOp},
    LoweredInstr, LoweredInstrKind, LoweredRoutine, LoweredType, LoweredTypeId, LoweredTypeTable,
    LoweredWorkspace,
};
use fol_resolver::PackageIdentity;

use super::helpers::{
    render_global_load, render_local_list, render_local_name, render_mutex_guard_name,
    render_namespace_module_path, render_native_intrinsic_expression, render_operand,
    render_routine_path, render_transfer_expr, render_type_path, rendered_result_local,
    resolve_global_decl, resolve_routine_decl, resolve_type_decl, validate_global_storage_type,
};

pub fn render_core_instruction(
    package_identity: &PackageIdentity,
    type_table: &LoweredTypeTable,
    routine: &LoweredRoutine,
    instruction: &LoweredInstr,
) -> BackendResult<String> {
    render_core_instruction_in_workspace(None, package_identity, type_table, routine, instruction)
}

fn observed_storage_reference(
    type_table: &LoweredTypeTable,
    mut type_id: LoweredTypeId,
    name: &str,
) -> (LoweredTypeId, String) {
    let mut dereferences = 0usize;
    while let Some(LoweredType::Owned { inner } | LoweredType::Borrowed { inner, .. }) =
        type_table.get(type_id)
    {
        type_id = *inner;
        dereferences += 1;
    }
    let reference = if dereferences == 0 {
        format!("&{name}")
    } else {
        format!("&{}{name}", "*".repeat(dereferences))
    };
    (type_id, reference)
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
                    let value =
                        render_transfer_expr(type_table, package_identity, caller, *local_id)?;
                    Ok(format!("rt::FolMutex::from_value({value})"))
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
        LoweredInstrKind::ConstraintCall { method, .. } => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!(
                "constraint call '{method}' reached backend emission without being monomorphized"
            ),
        )),
        LoweredInstrKind::Const(operand) => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            Ok(format!("{result} = {};", render_operand(operand)?))
        }
        LoweredInstrKind::LoadLocal { local } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let source_name = render_local_name(package_identity, routine, *local)?;
            let source_is_mutex = routine.mutex_params.contains(local);
            let source_moves = !source_is_mutex
                && routine
                    .locals
                    .get(*local)
                    .and_then(|local| local.type_id)
                    .is_some_and(|type_id| type_table.moves_on_transfer(type_id));
            // Generated control flow is a Rust dispatch loop. Leaving a named
            // move-only slot uninitialized after a semantic move prevents
            // rustc from proving later FOL reinitialization across blocks.
            // Replace it with its backend-only default sentinel; typecheck is
            // still the authority that forbids reading a moved slot.
            let source = if source_moves {
                format!("std::mem::take(&mut {source_name})")
            } else {
                // A `[mux]` parameter's lowered type describes the protected
                // value, while the rendered local is an `Arc`-backed
                // `FolMutex<T>` handle. Even when `T` is move-only, forwarding
                // the handle must preserve the same mutex identity rather than
                // moving it out and replacing the caller's binding with a new
                // default mutex.
                if source_is_mutex {
                    format!("{source_name}.clone()")
                } else {
                    render_transfer_expr(type_table, package_identity, routine, *local)?
                }
            };
            Ok(format!("{result} = {source};"))
        }
        LoweredInstrKind::StoreLocal { local, value } => {
            let target = render_local_name(package_identity, routine, *local)?;
            // Target-directed construction of a `mux[T]` local wraps the inner
            // value in a fresh managed mutex (V3_MEM §8.3). Storing an existing
            // mutex handle (already a `FolMutex`) is passed through unchanged.
            let wrap_mutex =
                routine.mutex_params.contains(local) && !routine.mutex_params.contains(value);
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            if wrap_mutex {
                Ok(format!("{target} = rt::FolMutex::from_value({value});"))
            } else {
                Ok(format!("{target} = {value};"))
            }
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
            validate_global_storage_type(type_table, global_decl.type_id)?;
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
            let init_expr =
                super::helpers::render_global_init_expr(workspace, type_table, global_decl)?;
            Ok(format!(
                "*{global_path}.get_or_init(|| std::sync::Mutex::new({init_expr})).lock().unwrap_or_else(|e| e.into_inner()) = {value};",
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
        LoweredInstrKind::SpawnCall {
            callee,
            args,
            detached,
        } => {
            let (callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let rendered_args =
                render_call_arguments(type_table, package_identity, routine, callee_decl, args)?;
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            // A detached task is spawned without a join handle, so it is not
            // joined at scope or process exit (V3_PROC).
            let spawn_fn = if *detached {
                "spawn_detached"
            } else {
                "spawn_task"
            };
            Ok(format!(
                "rt::{spawn_fn}(move || {{ let _ = {callee_name}({rendered_args}); }});"
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
        LoweredInstrKind::ChannelReceiver { channel } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let channel = render_local_name(package_identity, routine, *channel)?;
            Ok(format!(
                "{result} = {channel}.acquire_receiver().expect(\"channel receiver must be transferred before it is received on again\");"
            ))
        }
        LoweredInstrKind::ChannelSend { channel, value } => {
            // A send yields a must-handle `err[T]`: `nil` on delivery, or the
            // unsent payload wrapped as an error when the receiver has closed.
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let channel = render_local_name(package_identity, routine, *channel)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            Ok(format!(
                "{result} = match {channel}.send({value}) {{ Ok(()) => rt::FolError::nil(), Err(__fol_unsent) => rt::FolError::new(__fol_unsent) }};"
            ))
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
            // This is the shell-present test behind `when ... on ... *`. An
            // `opt[T]` is present when it is `some`; an `err[T]` shell is present
            // when it holds a stored error (`nil` means no error).
            let is_error_shell = routine
                .locals
                .get(*operand)
                .and_then(|local| local.type_id)
                .and_then(|type_id| type_table.get(type_id))
                .is_some_and(|ty| matches!(ty, LoweredType::Error { .. }));
            let operand = render_local_name(package_identity, routine, *operand)?;
            let probe = if is_error_shell { "is_err" } else { "is_some" };
            Ok(format!("{result} = {operand}.{probe}();"))
        }
        LoweredInstrKind::FieldAccess { base, field } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let result_moves = instruction
                .result
                .and_then(|local_id| routine.locals.get(local_id))
                .and_then(|local| local.type_id)
                .is_some_and(|type_id| type_table.moves_on_transfer(type_id));
            let base_id = *base;
            let borrowed_base = routine
                .locals
                .get(base_id)
                .and_then(|local| local.type_id)
                .and_then(|type_id| type_table.get(type_id))
                .is_some_and(|ty| matches!(ty, LoweredType::Borrowed { .. }));
            if result_moves && borrowed_base {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "move-only fields cannot be transferred out of a borrowed base",
                ));
            }
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
                // A move-only field transfer must leave the containing local
                // structurally initialized. Lexical cleanup still drops that
                // local after the FOL move, and a native Rust field move would
                // make the later whole-value drop illegal. As with LoadLocal,
                // replace the transferred field with its backend-only default
                // sentinel; typecheck remains responsible for rejecting any
                // semantic read of the moved field.
                Ok(format!("{result} = std::mem::take(&mut {base}.{field});"))
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
            let operand_id = *operand;
            let operand = render_local_name(package_identity, routine, operand_id)?;
            let operand_type = routine
                .locals
                .get(operand_id)
                .and_then(|local| local.type_id)
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "length operand local does not retain a lowered type",
                    )
                })?;
            let (_, observed) = observed_storage_reference(type_table, operand_type, &operand);
            Ok(format!("{result} = rt::len({observed});"))
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
        LoweredInstrKind::ConstructBorrow {
            owner: owner_id,
            mutable,
            ..
        } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let owner = render_local_name(package_identity, routine, *owner_id)?;
            // A reborrow (owner is itself a borrow / Rust reference) must
            // reborrow through it (`&*owner`), not take a reference to the
            // reference (`&owner`).
            let owner_is_borrow = routine
                .locals
                .get(*owner_id)
                .and_then(|local| local.type_id)
                .and_then(|type_id| type_table.get(type_id))
                .is_some_and(|ty| matches!(ty, LoweredType::Borrowed { .. }));
            let deref = if owner_is_borrow { "*" } else { "" };
            Ok(format!(
                "{result} = &{}{deref}{owner};",
                if *mutable { "mut " } else { "" }
            ))
        }
        LoweredInstrKind::ReadBorrow { borrow } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let borrow = render_local_name(package_identity, routine, *borrow)?;
            Ok(format!("{result} = (*{borrow}).clone();"))
        }
        LoweredInstrKind::ConstructPointer {
            value,
            shared,
            type_id,
        } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            let sync = matches!(
                type_table.get(*type_id),
                Some(LoweredType::Pointer { sync: true, .. })
            );
            let constructor = if *shared && sync {
                "std::sync::Arc"
            } else if *shared {
                "std::rc::Rc"
            } else {
                "Box"
            };
            Ok(format!("{result} = {constructor}::new({value});"))
        }
        LoweredInstrKind::WeakDowngrade { pointer, type_id } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let pointer = render_local_name(package_identity, routine, *pointer)?;
            // The weak handle's own type is `ptr[weak, ...]`; whether it is an
            // `Arc`/`Rc` downgrade follows its `sync` flag.
            let sync = matches!(
                type_table.get(*type_id),
                Some(LoweredType::Pointer { sync: true, .. })
            );
            let origin = if sync {
                "std::sync::Arc"
            } else {
                "std::rc::Rc"
            };
            Ok(format!("{result} = {origin}::downgrade(&{pointer});"))
        }
        LoweredInstrKind::WeakUpgrade { pointer, .. } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let pointer = render_local_name(package_identity, routine, *pointer)?;
            Ok(format!(
                "{result} = match {pointer}.upgrade() {{ std::option::Option::Some(v) => rt::FolOption::Some(v), std::option::Option::None => rt::FolOption::Nil }};"
            ))
        }
        LoweredInstrKind::DerefPointer { pointer, consuming } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let pointer_id = *pointer;
            let pointer = render_local_name(package_identity, routine, pointer_id)?;
            let mut pointer_type = routine
                .locals
                .get(pointer_id)
                .and_then(|local| local.type_id)
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "pointer dereference operand does not retain a lowered type",
                    )
                })?;
            let mut wrapper_dereferences = 0usize;
            while let Some(LoweredType::Owned { inner } | LoweredType::Borrowed { inner, .. }) =
                type_table.get(pointer_type)
            {
                pointer_type = *inner;
                wrapper_dereferences += 1;
            }
            if !matches!(
                type_table.get(pointer_type),
                Some(LoweredType::Pointer { .. })
            ) {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "pointer dereference operand is not pointer-backed storage",
                ));
            }
            let dereferences = "*".repeat(wrapper_dereferences + 1);
            if *consuming {
                // Generated control flow keeps named locals structurally
                // initialized across dispatch blocks. Replace the consumed
                // unique pointer with its backend-only default sentinel before
                // moving T out of the allocation.
                Ok(format!(
                    "{result} = {dereferences}std::mem::take(&mut {pointer});"
                ))
            } else {
                Ok(format!("{result} = ({dereferences}{pointer}).clone();"))
            }
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
                // `nil` error shell: no stored error (the success state).
                None => "rt::FolError::nil()".to_string(),
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
                    format!("rt::require(rt::unwrap_optional_shell({operand}))")
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
            let result_type = instruction
                .result
                .and_then(|result| routine.locals.get(result))
                .and_then(|local| local.type_id)
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "index result local does not retain a lowered type",
                    )
                })?;
            if type_table.moves_on_transfer(result_type) {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "move-only index results require an explicit removal operation; clone-based index reads are not supported",
                ));
            }
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
            let (runtime_type, container_ref) =
                observed_storage_reference(type_table, type_id, &container_name);
            let expression = match type_table.get(runtime_type) {
                Some(LoweredType::Array { .. }) => format!(
                    "rt::require(rt::index_array({container_ref}, {index_name}.clone())).clone()"
                ),
                Some(LoweredType::Vector { .. }) => format!(
                    "rt::require(rt::index_vec({container_ref}, {index_name}.clone())).clone()"
                ),
                Some(LoweredType::Sequence { .. }) => format!(
                    "rt::require(rt::index_seq({container_ref}, {index_name}.clone())).clone()"
                ),
                Some(LoweredType::Set { .. }) => format!(
                    "rt::require(rt::index_set({container_ref}, {index_name}.clone())).clone()"
                ),
                Some(LoweredType::Map { .. }) => format!(
                    "rt::require(rt::lookup_map({container_ref}, &{index_name})).clone()"
                ),
                other => {
                    return Err(BackendError::new(
                        BackendErrorKind::InvalidInput,
                        format!("index emission expected array/vector/sequence/set/map local but found {other:?}"),
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
            let result_type = instruction
                .result
                .and_then(|result| routine.locals.get(result))
                .and_then(|local| local.type_id)
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "slice result local does not retain a lowered type",
                    )
                })?;
            if type_table.moves_on_transfer(result_type) {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "move-only slice results are not supported in V3; slice emission would clone unique ownership",
                ));
            }
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
            let (runtime_type, container_ref) =
                observed_storage_reference(type_table, type_id, &container_name);
            let expression = match type_table.get(runtime_type) {
                Some(LoweredType::Vector { .. }) => format!(
                    "rt::require(rt::slice_vec({container_ref}, {start_name}.clone(), {end_name}.clone()))"
                ),
                Some(LoweredType::Sequence { .. }) => format!(
                    "rt::require(rt::slice_seq({container_ref}, {start_name}.clone(), {end_name}.clone()))"
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
            if !callee_decl.mutex_params.is_empty() {
                return Err(BackendError::new(
                    BackendErrorKind::Unsupported,
                    "routines with mux[T] parameters cannot be emitted as first-class routine references",
                ));
            }
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            // The result local is declared with the routine's `Rc<dyn Fn>`
            // type, so wrapping the fn item unsize-coerces on assignment.
            Ok(format!("{result} = std::rc::Rc::new({callee_name});"))
        }
        LoweredInstrKind::ClosureRef {
            routine: callee,
            env,
        } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let (callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let callee_name = render_routine_path(workspace, callee_identity, callee_decl)?;
            let signature_id = callee_decl.signature.ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "closure routine '{}' is missing a lowered signature",
                        callee_decl.name
                    ),
                )
            })?;
            let Some(fol_lower::LoweredType::Routine(signature)) = type_table.get(signature_id)
            else {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "closure routine '{}' signature is not a routine type",
                        callee_decl.name
                    ),
                ));
            };
            let env_names = env
                .iter()
                .map(|local_id| render_local_name(package_identity, routine, *local_id))
                .collect::<BackendResult<Vec<_>>>()?;
            let visible_params = signature.params.get(env.len()..).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "closure routine '{}' has fewer parameters than captured values",
                        callee_decl.name
                    ),
                )
            })?;
            let closure_params = visible_params
                .iter()
                .enumerate()
                .map(|(index, type_id)| {
                    crate::types::render_rust_type_in_workspace(workspace, type_table, *type_id)
                        .map(|rendered| format!("__p{index}: {rendered}"))
                })
                .collect::<BackendResult<Vec<_>>>()?
                .join(", ");
            // The environment values move into the closure and are re-cloned
            // on every invocation, matching FOL's per-call value semantics.
            let call_args = env_names
                .iter()
                .map(|name| format!("{name}.clone()"))
                .chain((0..visible_params.len()).map(|index| format!("__p{index}")))
                .collect::<Vec<_>>()
                .join(", ");
            Ok(format!(
                "{result} = std::rc::Rc::new(move |{closure_params}| {callee_name}({call_args}));"
            ))
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
                    Ok(format!(
                        "{result} = ({callee_name}.as_ref())({rendered_args});"
                    ))
                }
                None => Ok(format!("({callee_name}.as_ref())({rendered_args});")),
            }
        }
    }
}

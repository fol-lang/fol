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

/// The `&mut` a store site writes a local through. Mirrors
/// `observed_storage_reference`, but refuses a shared loan: writing through one
/// would emit `&mut` on a `&T` and fail in rustc rather than in FOL.
fn mutable_storage_reference(
    type_table: &LoweredTypeTable,
    mut type_id: LoweredTypeId,
    name: &str,
) -> BackendResult<(LoweredTypeId, String)> {
    let mut dereferences = 0usize;
    loop {
        match type_table.get(type_id) {
            Some(LoweredType::Borrowed { mutable: false, .. }) => {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    "indexed assignment cannot write through a shared borrow",
                ))
            }
            Some(LoweredType::Owned { inner } | LoweredType::Borrowed { inner, .. }) => {
                type_id = *inner;
                dereferences += 1;
            }
            _ => break,
        }
    }
    let reference = if dereferences == 0 {
        format!("&mut {name}")
    } else {
        format!("&mut {}{name}", "*".repeat(dereferences))
    };
    Ok((type_id, reference))
}

/// The name an operator site reads a local through. Rust has no arithmetic,
/// comparison or cast on `&T`, so a borrowed slot is peeled down to the value
/// it points at.
fn render_operator_operand(
    type_table: &LoweredTypeTable,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> BackendResult<String> {
    let name = render_local_name(package_identity, routine, local_id)?;
    let mut type_id = routine.locals.get(local_id).and_then(|local| local.type_id);
    let mut depth = 0usize;
    while let Some(LoweredType::Borrowed { inner, .. }) = type_id.and_then(|id| type_table.get(id))
    {
        depth += 1;
        type_id = Some(*inner);
    }
    if depth == 0 {
        Ok(name)
    } else {
        Ok(format!("({}{name}).clone()", "*".repeat(depth)))
    }
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

/// Resolve the mutable place a container mutation writes through: the binding's
/// own local, or one field hop into a record it holds. Shared by `StoreIndex`
/// and `ContainerMutate`, which differ only in the runtime call they then emit.
fn mutable_container_place(
    type_table: &LoweredTypeTable,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    base_id: fol_lower::LoweredLocalId,
    field: &Option<String>,
    what: &str,
) -> BackendResult<(LoweredTypeId, String)> {
    // A guard binding aliases its mutex local, so the container lives behind the
    // held guard rather than on the handle -- addressing the handle directly
    // emits a field access on `FolMutex` (E0609).
    let base_name = if routine.mutex_params.contains(&base_id) {
        format!(
            "{}.as_mut().expect(\"mutex {what} requires .lock()\")",
            render_mutex_guard_name(base_id)
        )
    } else {
        render_local_name(package_identity, routine, base_id)?
    };
    let base_type = routine
        .locals
        .get(base_id)
        .and_then(|local| local.type_id)
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!("{what} base does not retain a lowered type"),
            )
        })?;
    // A container held in a record field is reached through the field; the base
    // itself is the record.
    match field {
        Some(field) => {
            let (record_type, _) = observed_storage_reference(type_table, base_type, &base_name);
            let Some(LoweredType::Record { fields, .. }) = type_table.get(record_type) else {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("{what} through a field requires a record base"),
                ));
            };
            let field_type = fields.get(field).copied().ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("{what} names unknown field '{field}'"),
                )
            })?;
            let place = format!("{base_name}.{}", crate::escape_rust_field_ident(field));
            mutable_storage_reference(type_table, field_type, &place)
        }
        None => mutable_storage_reference(type_table, base_type, &base_name),
    }
}

/// The shared reference a read-only container operation goes through. Same
/// base/field walk as `mutable_container_place`, but yields `&` so a `[bor]`
/// receiver stays legal.
fn shared_container_place(
    type_table: &LoweredTypeTable,
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    base_id: fol_lower::LoweredLocalId,
    field: &Option<String>,
    what: &str,
) -> BackendResult<(LoweredTypeId, String)> {
    let base_name = if routine.mutex_params.contains(&base_id) {
        format!(
            "{}.as_ref().expect(\"mutex {what} requires .lock()\")",
            render_mutex_guard_name(base_id)
        )
    } else {
        render_local_name(package_identity, routine, base_id)?
    };
    let base_type = routine
        .locals
        .get(base_id)
        .and_then(|local| local.type_id)
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!("{what} base does not retain a lowered type"),
            )
        })?;
    match field {
        Some(field) => {
            let (record_type, _) = observed_storage_reference(type_table, base_type, &base_name);
            let Some(LoweredType::Record { fields, .. }) = type_table.get(record_type) else {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("{what} through a field requires a record base"),
                ));
            };
            let field_type = fields.get(field).copied().ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("{what} names unknown field '{field}'"),
                )
            })?;
            let place = format!("{base_name}.{}", crate::escape_rust_field_ident(field));
            Ok(observed_storage_reference(type_table, field_type, &place))
        }
        None => Ok(observed_storage_reference(
            type_table, base_type, &base_name,
        )),
    }
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
            // A guard binding reads the protected value out of the held guard,
            // never out of the `FolMutex` handle. The result slot of a handle
            // forward carries the `mux[T]` marking; a guard read does not.
            if source_is_mutex
                && instruction
                    .result
                    .is_some_and(|result| !routine.mutex_params.contains(&result))
            {
                let guard = render_mutex_guard_name(*local);
                return Ok(format!(
                    "{result} = (**{guard}.as_ref().expect(\"mutex guard read requires .lock()\")).clone();"
                ));
            }
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
                Ok(format!("{target}.replace_value({value});"))
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
        LoweredInstrKind::ForeignCall {
            alias,
            adapter,
            symbol,
            args,
            error_type,
            callback_arg,
            buffer_arg,
        } => {
            // The callee is a FOL-generated safe adapter, not the raw provider
            // symbol. Section 4.13 keeps validation, the error convention, and
            // the capability check inside that adapter, so the emitted call
            // looks exactly like an ordinary one from here.
            // A handle crosses into the adapter as a bare address. Which
            // conversion applies is read off the local's own lowered type:
            // an owned handle gives the address up (`into_raw` takes `self`,
            // so the FOL value cannot be used again), and a loan only lends it.
            let mut rendered_args = args
                .iter()
                .enumerate()
                .map(|(position, arg)| {
                    let rendered =
                        render_local_list(type_table, package_identity, routine, &[*arg])?;
                    // A callback argument is not passed as a value at all: C
                    // takes a bare function pointer, so the closure stays in
                    // this frame and the provider receives a trampoline plus
                    // the closure's address.
                    if *callback_arg == Some(position) {
                        return Ok("Some(__fol_trampoline)".to_string());
                    }
                    // A paired buffer is one FOL value and two C arguments.
                    // The length is derived from the value rather than passed
                    // beside it, which is the point of the pairing: there is no
                    // second number a caller could get wrong. `as _` takes the
                    // provider's own width from the adapter's signature.
                    if *buffer_arg == Some(position) {
                        // The loan's own rendering is peeled off first. A
                        // mutable loan arrives as `&mut *local`, and taking a
                        // slice of that would be dereferencing the length.
                        let base = rendered
                            .trim_end_matches(".clone()")
                            .trim_start_matches("&mut *")
                            .trim_start_matches("&*")
                            .to_string();
                        let slice = match buffer_is_mutable(type_table, routine, *arg) {
                            true => format!("{base}.as_mut_slice()"),
                            false => format!("{base}.as_slice()"),
                        };
                        let address = match buffer_is_mutable(type_table, routine, *arg) {
                            true => format!("{slice}.as_mut_ptr()"),
                            false => format!("{slice}.as_ptr()"),
                        };
                        return Ok(format!("{address}, {slice}.len() as _"));
                    }
                    // A record is passed as its fields, matching the adapter,
                    // which rebuilds the provider's struct from them. FOL's
                    // struct has FOL's layout, so handing it over directly
                    // would be handing C a value it cannot read.
                    if let Some(fol_lower::LoweredType::ForeignRecord { fields, .. }) = routine
                        .locals
                        .get(*arg)
                        .and_then(|local| local.type_id)
                        .and_then(|type_id| type_table.get(type_id))
                    {
                        let base = rendered.trim_end_matches(".clone()").to_string();
                        return Ok(fields
                            .iter()
                            .map(|(field, _)| {
                                format!("{base}.{}", crate::escape_rust_field_ident(field))
                            })
                            .collect::<Vec<_>>()
                            .join(", "));
                    }
                    Ok(match handle_passing(type_table, routine, *arg) {
                        Some(HandlePassing::Owned) => format!("{rendered}.into_raw()"),
                        Some(HandlePassing::Borrowed) => format!("{rendered}.as_raw()"),
                        None => rendered,
                    })
                })
                .collect::<BackendResult<Vec<_>>>()?;

            // The context follows every declared argument, matching the adapter,
            // which appends it for the same reason: FOL owns that slot.
            let mut trampoline = String::new();
            let mut installed_closure = None;
            if let Some(position) = *callback_arg {
                let closure =
                    render_local_list(type_table, package_identity, routine, &[args[position]])?;
                let closure_local = closure.trim_end_matches(".clone()").to_string();
                trampoline =
                    render_callback_trampoline(type_table, routine, args[position], symbol)?;
                installed_closure = Some(closure_local.clone());
                // Still the closure's address, so a provider that logs or
                // compares its context sees something meaningful. It is never
                // dereferenced: the trampoline reads the thread-local slot.
                rendered_args.push(format!(
                    "(&{closure_local}) as *const _ as *mut core::ffi::c_void"
                ));
            }
            let rendered_args = rendered_args.join(", ");
            let module = crate::mangle::foreign_adapter_module_name(alias);
            let callee_name = format!(
                "{}::{module}::{}",
                crate::mangle::FOREIGN_ADAPTER_CRATE,
                crate::mangle::escape_rust_field_ident(adapter)
            );
            // A fallible adapter returns a plain `Result`, because the adapter
            // crate is linked without the runtime. The conversion into FOL's
            // carrier happens here, where `rt` is in scope.
            let expression = if error_type.is_some() {
                format!(
                    "match {callee_name}({rendered_args}) {{ \
                     Ok(__fol_ok) => rt::FolRecover::Ok(__fol_ok), \
                     Err(__fol_err) => rt::FolRecover::Err(__fol_err) }}"
                )
            } else {
                format!("{callee_name}({rendered_args})")
            };
            // The symbol is not in the emitted expression -- the adapter owns
            // it -- so it is recorded as a comment, which is what makes a
            // generated file greppable back to the provider it calls.
            let call = match instruction.result {
                Some(result_id) => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    // A producer hands back a bare address; adopting it is what
                    // makes the FOL value the thing that owes the release.
                    let expression = match handle_passing(type_table, routine, result_id) {
                        Some(HandlePassing::Owned) => {
                            // Checked rather than adopted: a producer that
                            // returns NULL owes no resource, and a handle that
                            // owes a release on nothing would call destroy on
                            // NULL later.
                            format!(
                                "{{ let __fol_handle = rt::FolHandle::from_raw({expression});                                  if __fol_handle.is_null() {{                                  rt::handle_produced_null(\"{symbol}\"); }} __fol_handle }}"
                            )
                        }
                        _ => expression,
                    };
                    format!("{result} = {expression};")
                }
                None => format!("{expression};"),
            };
            // The trampoline is a nested item, so it is scoped to this one
            // call: two call sites passing different closures cannot collide,
            // and the provider cannot reach one from anywhere else.
            if trampoline.is_empty() {
                Ok(format!("{call} // c: {symbol}"))
            } else {
                let closure =
                    installed_closure.expect("a rendered trampoline always installs its closure");
                // The slot is live for exactly the duration of the call. The
                // previous value is restored rather than cleared so a nested
                // call through the same site leaves the outer one usable.
                Ok(format!(
                    "{{\n{trampoline}\
                     \x20   let __fol_previous = __FOL_CALLBACK\n\
                     \x20       .with(|__fol_slot| __fol_slot.borrow_mut().replace({closure}.clone()));\n\
                     {call} // c: {symbol}\n\
                     \x20   __FOL_CALLBACK.with(|__fol_slot| {{\n\
                     \x20       *__fol_slot.borrow_mut() = __fol_previous;\n\
                     \x20   }});\n\
                     }}"
                ))
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
        LoweredInstrKind::StoreIndex {
            base,
            field,
            index,
            value,
        } => {
            let (container_type, container_ref) = mutable_container_place(
                type_table,
                package_identity,
                routine,
                *base,
                field,
                "indexed assignment",
            )?;
            let index_name = render_local_name(package_identity, routine, *index)?;
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            let call = match type_table.get(container_type) {
                Some(LoweredType::Array { .. }) => {
                    format!("rt::store_array({container_ref}, {index_name}.clone(), {value})")
                }
                Some(LoweredType::Vector { .. }) => {
                    format!("rt::store_vec({container_ref}, {index_name}.clone(), {value})")
                }
                other => {
                    return Err(BackendError::new(
                        BackendErrorKind::InvalidInput,
                        format!(
                        "indexed assignment expected an array or vector local but found {other:?}"
                    ),
                    ))
                }
            };
            Ok(format!("rt::require({call});"))
        }
        LoweredInstrKind::ContainerMutate {
            base,
            field,
            op,
            args,
        } => {
            use fol_lower::ContainerMutateOp;
            // A read-only operation still addresses the binding's own local, but
            // takes a shared reference: `&mut` on a `[bor]` receiver would fail
            // in rustc rather than in FOL.
            let (_container_type, container_ref) = if op.is_read_only() {
                shared_container_place(
                    type_table,
                    package_identity,
                    routine,
                    *base,
                    field,
                    "container read",
                )?
            } else {
                mutable_container_place(
                    type_table,
                    package_identity,
                    routine,
                    *base,
                    field,
                    "container mutation",
                )?
            };
            if args.len() != op.arity() {
                return Err(BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!(
                        "'.{}' expects {} lowered argument(s), got {}",
                        op.method_name(),
                        op.arity(),
                        args.len()
                    ),
                ));
            }
            let rendered = args
                .iter()
                .map(|arg| render_transfer_expr(type_table, package_identity, routine, *arg))
                .collect::<BackendResult<Vec<_>>>()?;
            match op {
                ContainerMutateOp::VecPush => {
                    Ok(format!("rt::push_vec({container_ref}, {});", rendered[0]))
                }
                ContainerMutateOp::VecClear => Ok(format!("rt::clear_vec({container_ref});")),
                ContainerMutateOp::VecSort => Ok(format!("rt::sort_vec({container_ref});")),
                ContainerMutateOp::VecReserve => Ok(format!(
                    "rt::reserve_vec({container_ref}, {});",
                    rendered[0]
                )),
                ContainerMutateOp::VecSwap => Ok(format!(
                    "rt::require(rt::swap_vec({container_ref}, {}, {}));",
                    rendered[0], rendered[1]
                )),
                ContainerMutateOp::VecTruncate => Ok(format!(
                    "rt::require(rt::truncate_vec({container_ref}, {}));",
                    rendered[0]
                )),
                ContainerMutateOp::VecInsertAt => Ok(format!(
                    "rt::require(rt::insert_vec({container_ref}, {}, {}));",
                    rendered[0], rendered[1]
                )),
                ContainerMutateOp::VecPop => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = rt::pop_vec({container_ref});"))
                }
                ContainerMutateOp::VecRemoveAt => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    // `remove_at` faults on a bad index like a bad read does, then
                    // wraps the removed element so both value-yielding operations
                    // share one `opt[T]` surface.
                    Ok(format!(
                        "{result} = rt::FolOption::some(rt::require(rt::remove_vec({container_ref}, {})));",
                        rendered[0]
                    ))
                }
                ContainerMutateOp::MapClear => Ok(format!("rt::clear_map({container_ref});")),
                ContainerMutateOp::MapInsert => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!(
                        "{result} = rt::insert_map({container_ref}, {}, {});",
                        rendered[0], rendered[1]
                    ))
                }
                // The lookup family borrows its key, so a key that is itself a
                // move-only value stays usable after the call.
                ContainerMutateOp::MapGet => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!(
                        "{result} = rt::get_map({container_ref}, &{});",
                        rendered[0]
                    ))
                }
                ContainerMutateOp::MapRemove => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!(
                        "{result} = rt::remove_map({container_ref}, &{});",
                        rendered[0]
                    ))
                }
                ContainerMutateOp::MapContains => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!(
                        "{result} = rt::contains_map({container_ref}, &{});",
                        rendered[0]
                    ))
                }
                ContainerMutateOp::MapKeys => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = rt::keys_map({container_ref});"))
                }
                ContainerMutateOp::MapValues => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = rt::values_map({container_ref});"))
                }
            }
        }
        LoweredInstrKind::StoreMutexValue { mutex, value } => {
            let guard = render_mutex_guard_name(*mutex);
            let value = render_transfer_expr(type_table, package_identity, routine, *value)?;
            Ok(format!(
                "**{guard}.as_mut().expect(\"mutex assignment requires .lock()\") = {value};"
            ))
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
        LoweredInstrKind::FinalizeEach {
            container,
            callee,
            form,
        } => {
            use fol_lower::FinalizeEachForm;
            let (_callee_identity, callee_decl) = resolve_routine_decl(workspace, *callee)?;
            let callee_name = render_routine_path(workspace, _callee_identity, callee_decl)?;
            let source = render_local_name(package_identity, routine, *container)?;
            // Consuming the container is what scope exit means here: nothing
            // reads it afterwards, and each element has to be owned to be
            // handed to a finalizer that consumes it.
            let each = match form {
                FinalizeEachForm::Linear => format!("{source}.into_vec()"),
                FinalizeEachForm::Array => format!("{source}.into_iter()"),
                FinalizeEachForm::Set => format!("{source}.into_set()"),
                FinalizeEachForm::MapKey | FinalizeEachForm::MapValue => {
                    format!("{source}.into_map()")
                }
                FinalizeEachForm::OptionalPayload | FinalizeEachForm::ErrorPayload => String::new(),
            };
            Ok(match form {
                FinalizeEachForm::MapKey => format!(
                    "for (__fol_fin, _) in {each} {{ {callee_name}(__fol_fin); }}"
                ),
                FinalizeEachForm::MapValue => format!(
                    "for (_, __fol_fin) in {each} {{ {callee_name}(__fol_fin); }}"
                ),
                FinalizeEachForm::OptionalPayload => format!(
                    "if let rt::FolOption::Some(__fol_fin) = {source} {{ {callee_name}(__fol_fin); }}"
                ),
                FinalizeEachForm::ErrorPayload => format!(
                    "if let rt::FolError::Err(__fol_fin) = {source} {{ {callee_name}(__fol_fin); }}"
                ),
                _ => format!("for __fol_fin in {each} {{ {callee_name}(__fol_fin); }}"),
            })
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
        // Both peel `@` and `bor` first, so `.type_name` of a loaned Point says
        // `Point` and `.size_of` measures the Point rather than the loan.
        // `ptr[...]` is deliberately not peeled: a pointer is a value you hold.
        LoweredInstrKind::TypeNameOf { operand } | LoweredInstrKind::SizeOfValue { operand } => {
            let result = rendered_result_local(package_identity, routine, instruction)?;
            let operand_id = *operand;
            let operand_name = render_local_name(package_identity, routine, operand_id)?;
            let operand_type = routine
                .locals
                .get(operand_id)
                .and_then(|local| local.type_id)
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::InvalidInput,
                        "introspection operand local does not retain a lowered type",
                    )
                })?;
            let (peeled, observed) =
                observed_storage_reference(type_table, operand_type, &operand_name);
            if matches!(instruction.kind, LoweredInstrKind::TypeNameOf { .. }) {
                // Resolved here, not at lowering time: monomorphization has
                // already replaced a templated copy's generic locals, so this
                // reads the concrete type.
                let spelling = type_table.render_type(peeled);
                Ok(format!("{result} = rt_model::FolStr::from({spelling:?});"))
            } else {
                Ok(format!(
                    "{result} = std::mem::size_of_val({observed}) as i64;"
                ))
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
        LoweredInstrKind::RuntimeHook {
            intrinsic, args, ..
        } => {
            let entry = intrinsic_by_id(*intrinsic).ok_or_else(|| {
                BackendError::new(
                    BackendErrorKind::InvalidInput,
                    format!("intrinsic id {:?} is not registered", intrinsic),
                )
            })?;
            let rendered = match (entry.name, args.as_slice()) {
                // The only hook whose emission depends on its destination
                // rather than its name: the target width comes from the local
                // it writes into, so no runtime function could know it.
                ("widen", [value]) => {
                    let width = result_int_width(type_table, routine, instruction)?;
                    let value =
                        render_transfer_expr(type_table, package_identity, routine, *value)?;
                    format!("({value} as {})", width.rust_primitive())
                }
                // `try_from` is the checked conversion. The reported error
                // carries the value that did not fit, so a caller can say which
                // one it was rather than only that something failed.
                ("narrow", [value]) => {
                    let width = result_int_width(type_table, routine, instruction)?;
                    let value =
                        render_transfer_expr(type_table, package_identity, routine, *value)?;
                    format!(
                        "{{ let __fol_narrowed = {value}; match <{target}>::try_from(__fol_narrowed) {{ Ok(fitted) => rt::FolRecover::Ok(fitted), Err(_) => rt::FolRecover::Err(__fol_narrowed as rt::FolInt) }} }}",
                        target = width.rust_primitive()
                    )
                }
                ("echo", [value])
                | ("write", [value])
                | ("int_to_str", [value])
                | ("int_to_flt", [value])
                | ("flt_to_int", [value])
                | ("flt_floor", [value])
                | ("flt_ceil", [value])
                | ("flt_round", [value])
                | ("chr_to_int", [value])
                | ("int_to_chr", [value])
                | ("chr_to_str", [value])
                | ("sqrt", [value])
                | ("flt_abs", [value])
                | ("sin", [value])
                | ("cos", [value])
                | ("tan", [value])
                | ("ln", [value])
                | ("log10", [value])
                | ("exp", [value])
                | ("is_nan", [value])
                | ("is_inf", [value])
                | ("pop_count", [value])
                | ("bit_not", [value])
                | ("clz", [value])
                | ("ctz", [value])
                | ("abs", [value])
                | ("asin", [value])
                | ("acos", [value])
                | ("atan", [value])
                | ("log2", [value])
                | ("file_exists", [value])
                | ("is_file", [value])
                | ("is_dir", [value])
                | ("file_mtime", [value])
                | ("file_size", [value])
                | ("make_dir", [value])
                | ("remove_file", [value])
                | ("exit_process", [value])
                | ("shell_out", [value])
                | ("tcp_listen", [value])
                | ("tcp_accept", [value])
                | ("tcp_connect", [value])
                | ("tcp_read", [value])
                | ("tcp_close", [value])
                | ("tcp_local_addr", [value])
                | ("str_char_len", [value])
                | ("str_byte_len", [value])
                | ("str_valid_utf8", [value])
                | ("str_chars", [value])
                | ("str_from_chars", [value])
                | ("chr_upper", [value])
                | ("chr_lower", [value])
                | ("str_upper", [value])
                | ("str_lower", [value])
                | ("chr_is_alpha", [value])
                | ("chr_is_digit", [value])
                | ("chr_is_space", [value])
                | ("str_trim", [value])
                | ("random_bytes", [value])
                | ("sleep_ns", [value])
                | ("time_parts", [value])
                | ("time_from_parts", [value])
                | ("read_bytes", [value])
                | ("dir_entries", [value])
                | ("remove_dir_all", [value])
                | ("file_is_link", [value])
                | ("read_link", [value])
                | ("permissions", [value])
                | ("flt_bits", [value])
                | ("flt_from_bits", [value])
                | ("flt_is_finite", [value])
                | ("tcp_try_read", [value])
                | ("tcp_peer_addr", [value])
                | ("udp_bind", [value])
                | ("udp_recv_from", [value])
                | ("dns_resolve", [value])
                | ("atomic_new", [value])
                | ("atomic_load", [value])
                | ("hash_bytes", [value])
                | ("tz_offset_sec", [value])
                | ("realpath", [value])
                | ("temp_file", [value])
                | ("file_unlock", [value])
                | ("unset_env_var", [value])
                | ("child_pid", [value])
                | ("child_try_wait", [value])
                | ("child_wait", [value])
                | ("str_from_bytes", [value])
                | ("str_bytes", [value])
                | ("bytes_valid_utf8", [value])
                | ("utf8_prefix_len", [value])
                | ("str_width", [value])
                | ("chr_width", [value])
                | ("file_flush", [value])
                | ("file_close", [value])
                | ("signal_trap", [value])
                | ("flt_to_str_exact", [value])
                | ("set_current_dir", [value])
                | ("raw_mode", [value])
                | ("sleep_ms", [value])
                | ("byte_to_str", [value])
                | ("read_key_ms", [value])
                | ("env_var", [value])
                | ("shell", [value])
                | ("dir_list", [value])
                | ("read_file", [value])
                | ("arg_at", [value])
                | ("write_err", [value]) => {
                    let value =
                        render_transfer_expr(type_table, package_identity, routine, *value)?;
                    format!("rt::{}({value})", entry.name)
                }
                ("arg_count", []) => "rt::arg_count()".to_string(),
                ("read_line", []) => "rt::read_line()".to_string(),
                ("read_all", []) => "rt::read_all()".to_string(),
                ("current_dir", []) => "rt::current_dir()".to_string(),
                ("random_flt", []) => "rt::random_flt()".to_string(),
                ("now_ns", []) => "rt::now_ns()".to_string(),
                ("mono_ns", []) => "rt::mono_ns()".to_string(),
                ("temp_dir", []) => "rt::temp_dir()".to_string(),
                ("home_dir", []) => "rt::home_dir()".to_string(),
                ("env_vars", []) => "rt::env_vars()".to_string(),
                ("process_id", []) => "rt::process_id()".to_string(),
                // Two runtime entry points rather than one optional message:
                // the message is a `str`, which does not exist below `memo`,
                // so the bare form has to stay callable at `core`.
                ("assert", [condition]) => {
                    let condition =
                        render_transfer_expr(type_table, package_identity, routine, *condition)?;
                    format!("rt::assert_that({condition})")
                }
                ("assert", [condition, message]) => {
                    let condition =
                        render_transfer_expr(type_table, package_identity, routine, *condition)?;
                    let message =
                        render_transfer_expr(type_table, package_identity, routine, *message)?;
                    format!("rt::assert_message({condition}, {message})")
                }
                ("backtrace", []) => "rt::backtrace()".to_string(),
                ("os_error", []) => "rt::os_error()".to_string(),
                ("os_error_kind", []) => "rt::os_error_kind()".to_string(),
                ("arg_program", []) => "rt::arg_program()".to_string(),
                ("signal_pending", []) => "rt::signal_pending()".to_string(),
                ("cpu_count", []) => "rt::cpu_count()".to_string(),
                ("thread_yield", []) => "rt::thread_yield()".to_string(),
                ("thread_id", []) => "rt::thread_id()".to_string(),
                ("write_file", [path, contents])
                | ("str_find", [path, contents])
                | ("parse_int", [path, contents])
                | ("parse_flt", [path, contents])
                | ("atan2", [path, contents])
                | ("hypot", [path, contents])
                | ("bit_and", [path, contents])
                | ("bit_or", [path, contents])
                | ("bit_xor", [path, contents])
                | ("shl", [path, contents])
                | ("shr", [path, contents])
                | ("rotl", [path, contents])
                | ("rotr", [path, contents])
                | ("min", [path, contents])
                | ("max", [path, contents])
                | ("checked_add", [path, contents])
                | ("checked_sub", [path, contents])
                | ("wrapping_add", [path, contents])
                | ("wrapping_sub", [path, contents])
                | ("saturating_add", [path, contents])
                | ("saturating_sub", [path, contents])
                | ("rename_file", [path, contents])
                | ("copy_file", [path, contents])
                | ("append_file", [path, contents])
                | ("tcp_write", [path, contents])
                | ("str_char", [path, contents])
                | ("str_char_index", [path, contents])
                | ("random_int", [path, contents])
                | ("write_bytes", [path, contents])
                | ("set_permissions", [path, contents])
                | ("set_env_var", [path, contents])
                | ("run_capture", [path, contents])
                | ("run_status", [path, contents])
                | ("flt_copysign", [path, contents])
                | ("flt_rem", [path, contents])
                | ("flt_next_after", [path, contents])
                | ("checked_mul", [path, contents])
                | ("wrapping_mul", [path, contents])
                | ("saturating_mul", [path, contents])
                | ("checked_div", [path, contents])
                | ("tcp_set_timeout", [path, contents])
                | ("tcp_set_nodelay", [path, contents])
                | ("tcp_shutdown", [path, contents])
                | ("atomic_store", [path, contents])
                | ("atomic_add", [path, contents])
                | ("bytes_equal_ct", [path, contents])
                | ("make_symlink", [path, contents])
                | ("child_kill", [path, contents])
                | ("child_spawn", [path, contents])
                | ("poll_read", [path, contents])
                | ("str_normalize", [path, contents])
                | ("str_is_normalized", [path, contents])
                | ("file_open", [path, contents])
                | ("file_read", [path, contents])
                | ("file_write", [path, contents])
                | ("float_to_str", [path, contents]) => {
                    let first = render_transfer_expr(type_table, package_identity, routine, *path)?;
                    let second =
                        render_transfer_expr(type_table, package_identity, routine, *contents)?;
                    format!("rt::{}({first}, {second})", entry.name)
                }
                ("str_replace", [text, from, to])
                | ("flt_mul_add", [text, from, to])
                | ("udp_send_to", [text, from, to])
                | ("atomic_cas", [text, from, to])
                | ("file_seek", [text, from, to])
                | ("file_lock", [text, from, to])
                | ("run_input", [text, from, to]) => {
                    let text = render_transfer_expr(type_table, package_identity, routine, *text)?;
                    let from = render_transfer_expr(type_table, package_identity, routine, *from)?;
                    let to = render_transfer_expr(type_table, package_identity, routine, *to)?;
                    format!("rt::{}({text}, {from}, {to})", entry.name)
                }
                ("str_sub", [text, start, count]) => {
                    let text = render_transfer_expr(type_table, package_identity, routine, *text)?;
                    let start =
                        render_transfer_expr(type_table, package_identity, routine, *start)?;
                    let count =
                        render_transfer_expr(type_table, package_identity, routine, *count)?;
                    format!("rt::str_sub({text}, {start}, {count})")
                }
                ("str_byte", [text, index]) => {
                    let text = render_transfer_expr(type_table, package_identity, routine, *text)?;
                    let index =
                        render_transfer_expr(type_table, package_identity, routine, *index)?;
                    format!("rt::str_byte({text}, {index})")
                }
                ("read_key" | "now_ms" | "term_cols" | "term_rows", []) => {
                    format!("rt::{}()", entry.name)
                }
                (other, _) => {
                    return Err(BackendError::new(
                        BackendErrorKind::Unsupported,
                        format!("runtime hook emission is not implemented yet for '.{other}(...)'"),
                    ))
                }
            };
            match instruction.result {
                Some(_) => {
                    let result = rendered_result_local(package_identity, routine, instruction)?;
                    Ok(format!("{result} = {rendered};"))
                }
                None => Ok(format!("{rendered};")),
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
            // A guard binding aliases its mutex local, so borrowing the owner
            // directly hands out `&mut FolMutex<T>` where `&mut T` is wanted.
            // The guarded value is reached through the held guard, exactly as
            // field and element stores already do.
            if routine.mutex_params.contains(owner_id) {
                let guard = render_mutex_guard_name(*owner_id);
                let accessor = if *mutable {
                    "as_mut().expect(\"mutex receiver requires .lock()\")"
                } else {
                    "as_ref().expect(\"mutex receiver requires .lock()\")"
                };
                return Ok(format!(
                    "{result} = &{}*{guard}.{accessor};",
                    if *mutable { "mut " } else { "" }
                ));
            }
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
            let left = render_operator_operand(type_table, package_identity, routine, left_id)?;
            let right = render_operator_operand(type_table, package_identity, routine, *right)?;
            let expression = match op {
                LoweredBinaryOp::Add => format!("{left} + {right}"),
                LoweredBinaryOp::Sub => format!("{left} - {right}"),
                LoweredBinaryOp::Mul => format!("{left} * {right}"),
                // Integer division/modulo fault on a zero divisor; the rt
                // helpers present that as a fol runtime fault instead of a
                // raw Rust panic pointing into generated code. Float forms
                // keep the plain operators (they yield inf/NaN, no fault).
                LoweredBinaryOp::Div => {
                    if operand_is_float(type_table, routine, left_id) {
                        format!("{left} / {right}")
                    } else {
                        format!("rt::div_int({left}, {right})")
                    }
                }
                LoweredBinaryOp::Mod => {
                    if operand_is_float(type_table, routine, left_id) {
                        format!("{left} % {right}")
                    } else {
                        format!("rt::mod_int({left}, {right})")
                    }
                }
                LoweredBinaryOp::Pow => {
                    if operand_is_float(type_table, routine, left_id) {
                        format!("rt::pow_float({left}, {right})")
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
            let operand = render_operator_operand(type_table, package_identity, routine, *operand)?;
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
            let operand = render_operator_operand(type_table, package_identity, routine, *operand)?;
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

/// Whether a lowered local carries the builtin float type. Arithmetic render
/// arms use this to pick float operators over the faulting integer helpers;
/// an untyped local falls back to the integer form, matching the historical
/// `pow` dispatch.
fn operand_is_float(
    type_table: &LoweredTypeTable,
    routine: &LoweredRoutine,
    local: fol_lower::LoweredLocalId,
) -> bool {
    routine
        .locals
        .get(local)
        .and_then(|local| local.type_id)
        .is_some_and(|type_id| {
            matches!(
                type_table.get(type_id),
                Some(LoweredType::Builtin(fol_lower::LoweredBuiltinType::Float(
                    _
                )))
            )
        })
}

/// The integer width an instruction writes into. `.widen(...)` emits a cast to
/// its destination type, which the intrinsic name alone does not carry.
fn result_int_width(
    type_table: &LoweredTypeTable,
    routine: &LoweredRoutine,
    instruction: &LoweredInstr,
) -> BackendResult<fol_types::IntWidth> {
    instruction
        .result
        .and_then(|local_id| routine.locals.get(local_id))
        .and_then(|local| local.type_id)
        .and_then(|type_id| type_table.get(type_id))
        .and_then(|lowered| match lowered {
            LoweredType::Builtin(fol_lower::LoweredBuiltinType::Int(width)) => Some(*width),
            _ => None,
        })
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                "a width conversion must write into an integer local",
            )
        })
}

/// How one local carries a foreign handle across the adapter boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HandlePassing {
    /// The FOL value owns the handle and gives the address up.
    Owned,
    /// The FOL value is a loan and only lends the address.
    Borrowed,
}

/// Whether a local is a foreign handle, and in which of the two forms.
///
/// Read off the lowered type rather than the routine's ABI record, so the
/// answer is the same one the emitted Rust type is built from and the two
/// cannot disagree.
fn handle_passing(
    type_table: &fol_lower::LoweredTypeTable,
    routine: &fol_lower::LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> Option<HandlePassing> {
    let type_id = routine.locals.get(local_id)?.type_id?;
    match type_table.get(type_id)? {
        fol_lower::LoweredType::ForeignHandle { .. } => Some(HandlePassing::Owned),
        fol_lower::LoweredType::Borrowed { inner, .. } => matches!(
            type_table.get(*inner),
            Some(fol_lower::LoweredType::ForeignHandle { .. })
        )
        .then_some(HandlePassing::Borrowed),
        _ => None,
    }
}

/// Whether a paired buffer is lent mutably.
///
/// A provider that writes through the address needs a mutable loan on FOL's
/// side too; the const-ness the header declared is what decided which.
fn buffer_is_mutable(
    type_table: &fol_lower::LoweredTypeTable,
    routine: &fol_lower::LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> bool {
    let Some(Some(type_id)) = routine.locals.get(local_id).map(|local| local.type_id) else {
        return false;
    };
    matches!(
        type_table.get(type_id),
        Some(fol_lower::LoweredType::Borrowed { mutable: true, .. })
    )
}

/// The `extern "C"` shim a provider calls back through.
///
/// C takes a bare function pointer, and a Rust closure is not one: it carries
/// an environment. The shim is the bridge -- a monomorphic `extern "C" fn` that
/// recovers the closure from the context pointer FOL passed alongside it and
/// calls it.
///
/// Two rules the plan names are enforced here rather than documented:
///
/// - **The context is validated.** A null one means the provider called back
///   outside the call that lent the closure, and there is no value that would
///   be a true answer, so the process ends.
/// - **A panic is contained.** Unwinding out of `extern "C"` is undefined, and
///   a callback has no status channel to report through, so a panic also ends
///   the process rather than returning something nobody computed.
fn render_callback_trampoline(
    type_table: &fol_lower::LoweredTypeTable,
    routine: &fol_lower::LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
    symbol: &str,
) -> BackendResult<String> {
    let type_id = routine
        .locals
        .get(local_id)
        .and_then(|local| local.type_id)
        .ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!("the callback passed to '{symbol}' lost its lowered type"),
            )
        })?;
    let Some(fol_lower::LoweredType::Routine(signature)) = type_table.get(type_id) else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("the callback passed to '{symbol}' is not a routine value"),
        ));
    };

    let closure_type = crate::types::render_rust_type(type_table, type_id)?;
    let mut params = Vec::new();
    let mut forwarded = Vec::new();
    for (index, param) in signature.params.iter().enumerate() {
        let rendered = crate::types::render_rust_type(type_table, *param)?;
        params.push(format!("__fol_a{index}: {rendered}"));
        forwarded.push(format!("__fol_a{index}"));
    }
    let params = params.join(", ");
    let forwarded = forwarded.join(", ");
    let result = match signature.return_type {
        Some(return_type) => format!(
            " -> {}",
            crate::types::render_rust_type(type_table, return_type)?
        ),
        None => String::new(),
    };

    // The symbol as a Rust string literal, so the runtime message names the
    // provider routine that called back.
    let named = format!("{symbol:?}");
    let separator = if params.is_empty() { "" } else { ", " };
    // The closure reaches the trampoline through a thread-local slot rather
    // than through the context pointer, and that is the whole safety property
    // here. The context is a pointer to a stack local: dereferencing it works
    // while the call is on the stack and reads reused memory afterwards, so a
    // provider that stashed the callback and invoked it later would run a
    // closure that no longer exists, silently.
    //
    // The slot is filled for the duration of the call and restored after, so
    // an invocation from outside that window finds it empty and is refused.
    // Being thread-local covers the other half at no extra cost: another
    // thread's slot was never filled, so a cross-thread invocation is refused
    // by the same check.
    //
    // Saved and restored rather than cleared, because the FOL closure may
    // itself call back into the same foreign routine; clearing would leave the
    // outer call's slot empty while it is still running.
    Ok(format!(
        "    thread_local! {{\n\
         \x20       static __FOL_CALLBACK: std::cell::RefCell<Option<{closure_type}>> =\n\
         \x20           const {{ std::cell::RefCell::new(None) }};\n\
         \x20   }}\n\
         \x20   unsafe extern \"C\" fn __fol_trampoline(\
         __fol_context: *mut core::ffi::c_void{separator}{params}){result} {{\n\
         \x20       if __fol_context.is_null() {{ rt::callback_context_invalid({named}); }}\n\
         \x20       // Cloned out rather than borrowed across the call: the\n\
         \x20       // closure may re-enter, and a live borrow would panic.\n\
         \x20       let __fol_closure = __FOL_CALLBACK.with(|__fol_slot| {{\n\
         \x20           __fol_slot.borrow().clone()\n\
         \x20       }});\n\
         \x20       let Some(__fol_closure) = __fol_closure else {{\n\
         \x20           rt::callback_invoked_out_of_scope({named});\n\
         \x20       }};\n\
         \x20       match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| \
         __fol_closure({forwarded}))) {{\n\
         \x20           Ok(__fol_value) => __fol_value,\n\
         \x20           Err(_) => rt::callback_panicked({named}),\n\
         \x20       }}\n\
         \x20   }}\n"
    ))
}

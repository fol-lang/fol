use crate::{LoweredGlobalId, LoweredRoutineId, LoweredWorkspace, LoweringError, LoweringErrorKind};
use std::collections::BTreeSet;

use super::helpers::{verify_local_reference, verify_type_reference};

fn recoverable_error_type_for_local(
    routine: &crate::LoweredRoutine,
    local_id: crate::LoweredLocalId,
) -> Option<crate::LoweredTypeId> {
    recoverable_error_type_for_local_inner(routine, local_id, 0)
}

fn recoverable_error_type_for_local_inner(
    routine: &crate::LoweredRoutine,
    local_id: crate::LoweredLocalId,
    depth: usize,
) -> Option<crate::LoweredTypeId> {
    if depth > 8 {
        return None;
    }
    routine.instructions.iter().find_map(|instr| match &instr.kind {
        crate::LoweredInstrKind::Call { error_type, .. }
        | crate::LoweredInstrKind::CallIndirect { error_type, .. }
            if instr.result == Some(local_id) =>
        {
            *error_type
        }
        // Chained fallbacks join a still-wrapped recoverable through a
        // StoreLocal; follow the stored source.
        crate::LoweredInstrKind::StoreLocal { local, value } if *local == local_id => {
            recoverable_error_type_for_local_inner(routine, *value, depth + 1)
        }
        _ => None,
    })
}

pub(super) fn verify_instruction(
    workspace: &LoweredWorkspace,
    package: &crate::LoweredPackage,
    routine: &crate::LoweredRoutine,
    instr: &crate::LoweredInstr,
    valid_global_ids: &BTreeSet<LoweredGlobalId>,
    valid_routine_ids: &BTreeSet<LoweredRoutineId>,
    errors: &mut Vec<LoweringError>,
) {
    match &instr.kind {
        crate::LoweredInstrKind::ConstraintCall { .. } => {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "constraint call in routine '{}' was not resolved during monomorphization",
                    routine.name
                ),
            ));
        }
        crate::LoweredInstrKind::Const(_) => {}
        crate::LoweredInstrKind::LoadGlobal { global } => {
            if !valid_global_ids.contains(global) {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' loads missing global {}",
                        routine.name, global.0
                    ),
                ));
            }
        }
        crate::LoweredInstrKind::LoadLocal { local }
        | crate::LoweredInstrKind::UnwrapShell { operand: local }
        | crate::LoweredInstrKind::CheckRecoverable { operand: local }
        | crate::LoweredInstrKind::UnwrapRecoverable { operand: local }
        | crate::LoweredInstrKind::ExtractRecoverableError { operand: local } => {
            verify_local_reference(routine, instr.id.0, "operand", *local, errors);
        }
        crate::LoweredInstrKind::StoreLocal { local, value } => {
            verify_local_reference(routine, instr.id.0, "store target", *local, errors);
            verify_local_reference(routine, instr.id.0, "store value", *value, errors);
        }
        crate::LoweredInstrKind::StoreField { base, value, .. } => {
            verify_local_reference(routine, instr.id.0, "field store base", *base, errors);
            verify_local_reference(routine, instr.id.0, "field store value", *value, errors);
        }
        crate::LoweredInstrKind::StoreGlobal { global, value } => {
            if !valid_global_ids.contains(global) {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' stores to missing global {}",
                        routine.name, global.0
                    ),
                ));
            }
            verify_local_reference(routine, instr.id.0, "store value", *value, errors);
        }
        crate::LoweredInstrKind::Call {
            callee,
            args,
            error_type,
        } => {
            if !valid_routine_ids.contains(callee) {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' calls missing routine {}",
                        routine.name, callee.0
                    ),
                ));
            }
            for arg in args {
                verify_local_reference(routine, instr.id.0, "call arg", *arg, errors);
            }
            if let Some(error_type) = error_type {
                verify_type_reference(
                    workspace,
                    package,
                    routine,
                    instr.id.0,
                    "call error type",
                    *error_type,
                    errors,
                );
            }
        }
        crate::LoweredInstrKind::IntrinsicCall { intrinsic, args } => {
            if fol_intrinsics::intrinsic_by_id(*intrinsic).is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' uses missing intrinsic {}",
                        routine.name,
                        intrinsic.index()
                    ),
                ));
            } else if fol_intrinsics::backend_role_for_intrinsic(*intrinsic)
                != Some(fol_intrinsics::IntrinsicBackendRole::PureOp)
            {
                let intrinsic_name = fol_intrinsics::intrinsic_by_id(*intrinsic)
                    .map(|entry| entry.name)
                    .unwrap_or("<missing>");
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' instruction {} uses intrinsic '.{}' as an IntrinsicCall even though it is not a pure-op intrinsic",
                        routine.name, instr.id.0, intrinsic_name
                    ),
                ));
            }
            if instr.result.is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' intrinsic instruction {} must write a result local",
                        routine.name, instr.id.0
                    ),
                ));
            }
            for arg in args {
                verify_local_reference(routine, instr.id.0, "intrinsic arg", *arg, errors);
            }
        }
        crate::LoweredInstrKind::RuntimeHook { intrinsic, args } => {
            if fol_intrinsics::intrinsic_by_id(*intrinsic).is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' uses missing runtime hook intrinsic {}",
                        routine.name,
                        intrinsic.index()
                    ),
                ));
            } else if fol_intrinsics::backend_role_for_intrinsic(*intrinsic)
                != Some(fol_intrinsics::IntrinsicBackendRole::RuntimeHook)
            {
                let intrinsic_name = fol_intrinsics::intrinsic_by_id(*intrinsic)
                    .map(|entry| entry.name)
                    .unwrap_or("<missing>");
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' instruction {} uses intrinsic '.{}' as a RuntimeHook even though it is not a runtime-hook intrinsic",
                        routine.name, instr.id.0, intrinsic_name
                    ),
                ));
            }
            if let Some(result) = instr.result {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' runtime hook instruction {} must not write result local {}",
                        routine.name, instr.id.0, result.0
                    ),
                ));
            }
            for arg in args {
                verify_local_reference(routine, instr.id.0, "runtime hook arg", *arg, errors);
            }
        }
        crate::LoweredInstrKind::LengthOf { operand } => {
            if instr.result.is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' length helper instruction {} must write a result local",
                        routine.name, instr.id.0
                    ),
                ));
            }
            verify_local_reference(routine, instr.id.0, "length operand", *operand, errors);
        }
        crate::LoweredInstrKind::ConstructRecord { type_id, fields } => {
            verify_type_reference(
                workspace,
                package,
                routine,
                instr.id.0,
                "record type",
                *type_id,
                errors,
            );
            for (_, value) in fields {
                verify_local_reference(routine, instr.id.0, "record field", *value, errors);
            }
        }
        crate::LoweredInstrKind::ConstructEntry {
            type_id, payload, ..
        } => {
            verify_type_reference(
                workspace,
                package,
                routine,
                instr.id.0,
                "entry type",
                *type_id,
                errors,
            );
            if let Some(payload) = payload {
                verify_local_reference(routine, instr.id.0, "entry payload", *payload, errors);
            }
        }
        crate::LoweredInstrKind::ConstructLinear {
            type_id, elements, ..
        } => {
            verify_type_reference(
                workspace,
                package,
                routine,
                instr.id.0,
                "linear type",
                *type_id,
                errors,
            );
            for element in elements {
                verify_local_reference(routine, instr.id.0, "linear element", *element, errors);
            }
        }
        crate::LoweredInstrKind::ConstructSet { type_id, members } => {
            verify_type_reference(
                workspace, package, routine, instr.id.0, "set type", *type_id, errors,
            );
            for member in members {
                verify_local_reference(routine, instr.id.0, "set member", *member, errors);
            }
        }
        crate::LoweredInstrKind::ConstructMap { type_id, entries } => {
            verify_type_reference(
                workspace, package, routine, instr.id.0, "map type", *type_id, errors,
            );
            for (key, value) in entries {
                verify_local_reference(routine, instr.id.0, "map key", *key, errors);
                verify_local_reference(routine, instr.id.0, "map value", *value, errors);
            }
        }
        crate::LoweredInstrKind::ConstructOptional { type_id, value }
        | crate::LoweredInstrKind::ConstructError { type_id, value } => {
            verify_type_reference(
                workspace,
                package,
                routine,
                instr.id.0,
                "shell type",
                *type_id,
                errors,
            );
            if let Some(value) = value {
                verify_local_reference(routine, instr.id.0, "shell value", *value, errors);
            }
        }
        crate::LoweredInstrKind::FieldAccess { base, .. } => {
            verify_local_reference(routine, instr.id.0, "field base", *base, errors);
        }
        crate::LoweredInstrKind::IndexAccess { container, index } => {
            verify_local_reference(routine, instr.id.0, "index container", *container, errors);
            verify_local_reference(routine, instr.id.0, "index value", *index, errors);
        }
        crate::LoweredInstrKind::SliceAccess {
            container,
            start,
            end,
        } => {
            verify_local_reference(routine, instr.id.0, "slice container", *container, errors);
            verify_local_reference(routine, instr.id.0, "slice start", *start, errors);
            verify_local_reference(routine, instr.id.0, "slice end", *end, errors);
        }
        crate::LoweredInstrKind::Cast {
            operand,
            target_type,
        } => {
            verify_local_reference(routine, instr.id.0, "cast operand", *operand, errors);
            verify_type_reference(
                workspace,
                package,
                routine,
                instr.id.0,
                "cast type",
                *target_type,
                errors,
            );
        }
        crate::LoweredInstrKind::BinaryOp { left, right, .. } => {
            verify_local_reference(routine, instr.id.0, "binary left", *left, errors);
            verify_local_reference(routine, instr.id.0, "binary right", *right, errors);
        }
        crate::LoweredInstrKind::UnaryOp { operand, .. } => {
            verify_local_reference(routine, instr.id.0, "unary operand", *operand, errors);
        }
        crate::LoweredInstrKind::RoutineRef { routine: callee } => {
            if !valid_routine_ids.contains(callee) {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' references missing routine {}",
                        routine.name, callee.0
                    ),
                ));
            }
        }
        crate::LoweredInstrKind::CallIndirect {
            callee,
            args,
            error_type,
        } => {
            verify_local_reference(routine, instr.id.0, "indirect callee", *callee, errors);
            for arg in args {
                verify_local_reference(routine, instr.id.0, "indirect call arg", *arg, errors);
            }
            if let Some(error_type) = error_type {
                verify_type_reference(
                    workspace,
                    package,
                    routine,
                    instr.id.0,
                    "indirect call error type",
                    *error_type,
                    errors,
                );
            }
        }
    }

    match &instr.kind {
        crate::LoweredInstrKind::ConstraintCall { .. } => {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "constraint call in routine '{}' was not resolved during monomorphization",
                    routine.name
                ),
            ));
        }
        crate::LoweredInstrKind::CheckRecoverable { operand }
        | crate::LoweredInstrKind::UnwrapRecoverable { operand }
        | crate::LoweredInstrKind::ExtractRecoverableError { operand } => {
            let operand_effect = recoverable_error_type_for_local(routine, *operand);
            if operand_effect.is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' instruction {} expects a recoverable call-result operand local {}",
                        routine.name, instr.id.0, operand.0
                    ),
                ));
            }
        }
        _ => {}
    }
}

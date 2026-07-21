use crate::{LoweredTerminator, LoweredWorkspace, LoweringError, LoweringErrorKind};
use std::collections::{BTreeSet, VecDeque};

mod helpers;
mod instruction;

#[cfg(test)]
mod tests;

use helpers::enqueue_target;
use instruction::verify_instruction;

pub(crate) fn verify_workspace(workspace: &LoweredWorkspace) -> Result<(), Vec<LoweringError>> {
    let mut errors = Vec::new();
    let valid_global_ids = workspace
        .packages()
        .flat_map(|package| package.global_decls.keys().copied())
        .collect::<BTreeSet<_>>();
    let valid_routine_ids = workspace
        .packages()
        .flat_map(|package| package.routine_decls.keys().copied())
        .collect::<BTreeSet<_>>();

    for package in workspace.packages() {
        for ownership in package.symbol_ownership.values() {
            match ownership.mounted_from.as_ref() {
                Some(provenance) => {
                    if ownership.owning_package != provenance.package_identity {
                        errors.push(LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!(
                                "lowered package '{}' recorded mounted symbol {} with conflicting owning package '{}'",
                                package.identity.display_name,
                                ownership.symbol_id.0,
                                ownership.owning_package.display_name,
                            ),
                        ));
                    }
                }
                None => {
                    if ownership.owning_package != package.identity {
                        errors.push(LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!(
                                "lowered package '{}' recorded local symbol {} as owned by foreign package '{}'",
                                package.identity.display_name,
                                ownership.symbol_id.0,
                                ownership.owning_package.display_name,
                            ),
                        ));
                    }
                }
            }
        }

        for global in package.global_decls.values() {
            if workspace.type_table().get(global.type_id).is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered global '{}' references missing type {}",
                        global.name, global.type_id.0
                    ),
                ));
            }
        }

        for type_decl in package.type_decls.values() {
            if workspace.type_table().get(type_decl.runtime_type).is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered type '{}' references missing runtime type {}",
                        type_decl.name, type_decl.runtime_type.0
                    ),
                ));
            }
        }

        for routine in package.routine_decls.values() {
            verify_routine(
                workspace,
                package,
                routine,
                &valid_global_ids,
                &valid_routine_ids,
                &mut errors,
            );
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn verify_routine(
    workspace: &LoweredWorkspace,
    package: &crate::LoweredPackage,
    routine: &crate::LoweredRoutine,
    valid_global_ids: &BTreeSet<crate::LoweredGlobalId>,
    valid_routine_ids: &BTreeSet<crate::LoweredRoutineId>,
    errors: &mut Vec<LoweringError>,
) {
    if let Some(signature) = routine.signature {
        if workspace.type_table().get(signature).is_none() {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' references missing signature type {}",
                    routine.name, signature.0
                ),
            ));
        }
    }
    if let Some(receiver_type) = routine.receiver_type {
        if workspace.type_table().get(receiver_type).is_none() {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' references missing receiver type {}",
                    routine.name, receiver_type.0
                ),
            ));
        }
    }
    for local in routine.locals.iter() {
        if let Some(type_id) = local.type_id {
            if workspace.type_table().get(type_id).is_none() {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' local {:?} references missing type {}",
                        routine.name, local.name, type_id.0
                    ),
                ));
            }
        }
    }
    for param in &routine.params {
        if routine.locals.get(*param).is_none() {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' parameter local {} is missing",
                    routine.name, param.0
                ),
            ));
        }
    }
    if let Some(body_result) = routine.body_result {
        if routine.locals.get(body_result).is_none() {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' body result local {} is missing",
                    routine.name, body_result.0
                ),
            ));
        }
    }
    for (symbol_id, local_id) in &routine.local_symbols {
        if routine.locals.get(*local_id).is_none() {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' maps symbol {} to missing local {}",
                    routine.name, symbol_id.0, local_id.0
                ),
            ));
        }
    }

    verify_no_generic_parameter_leak(workspace, routine, errors);

    verify_routine_blocks(
        workspace,
        package,
        routine,
        valid_global_ids,
        valid_routine_ids,
        errors,
    );
}

/// The backend derives a routine's Rust generic clause solely from its
/// signature (parameters, return, error) and receiver. Any generic parameter
/// that survives lowering in a *local* or instruction type without appearing in
/// that clause would emit as an unbound Rust type parameter (`FolSeq<t>` with
/// `t` never declared). Monomorphization is supposed to erase such leaks; this
/// is the safety net that rejects a routine before the backend ever sees it.
fn verify_no_generic_parameter_leak(
    workspace: &LoweredWorkspace,
    routine: &crate::LoweredRoutine,
    errors: &mut Vec<LoweringError>,
) {
    let type_table = workspace.type_table();
    let mut declared = BTreeSet::new();
    if let Some(signature) = routine.signature {
        type_table.collect_generic_parameter_names(signature, &mut declared);
    }
    if let Some(receiver_type) = routine.receiver_type {
        type_table.collect_generic_parameter_names(receiver_type, &mut declared);
    }

    let mut leaked = BTreeSet::new();
    let check = |type_id: crate::LoweredTypeId, leaked: &mut BTreeSet<String>| {
        let mut names = BTreeSet::new();
        type_table.collect_generic_parameter_names(type_id, &mut names);
        for name in names {
            if !declared.contains(&name) {
                leaked.insert(name);
            }
        }
    };

    for local in routine.locals.iter() {
        if let Some(type_id) = local.type_id {
            check(type_id, &mut leaked);
        }
    }
    for instr in routine.instructions.iter() {
        match &instr.kind {
            crate::LoweredInstrKind::Call { error_type, .. }
            | crate::LoweredInstrKind::CallIndirect { error_type, .. } => {
                if let Some(error_type) = error_type {
                    check(*error_type, &mut leaked);
                }
            }
            crate::LoweredInstrKind::ConstructRecord { type_id, .. }
            | crate::LoweredInstrKind::ConstructEntry { type_id, .. }
            | crate::LoweredInstrKind::ConstructLinear { type_id, .. }
            | crate::LoweredInstrKind::ConstructSet { type_id, .. }
            | crate::LoweredInstrKind::ConstructMap { type_id, .. }
            | crate::LoweredInstrKind::ConstructOptional { type_id, .. }
            | crate::LoweredInstrKind::ConstructError { type_id, .. }
            | crate::LoweredInstrKind::Cast {
                target_type: type_id,
                ..
            } => check(*type_id, &mut leaked),
            _ => {}
        }
    }

    for name in leaked {
        errors.push(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "lowered routine '{}' leaks generic parameter '{}' not bound by its signature; \
                 monomorphization must erase it before backend emission",
                routine.name, name
            ),
        ));
    }
}

fn verify_routine_blocks(
    workspace: &LoweredWorkspace,
    package: &crate::LoweredPackage,
    routine: &crate::LoweredRoutine,
    valid_global_ids: &BTreeSet<crate::LoweredGlobalId>,
    valid_routine_ids: &BTreeSet<crate::LoweredRoutineId>,
    errors: &mut Vec<LoweringError>,
) {
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::from([routine.entry_block]);

    while let Some(block_id) = queue.pop_front() {
        if !reachable.insert(block_id) {
            continue;
        }
        let Some(block) = routine.blocks.get(block_id) else {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' references a missing block {}",
                    routine.name, block_id.0
                ),
            ));
            continue;
        };

        for instr_id in &block.instructions {
            let Some(instr) = routine.instructions.get(*instr_id) else {
                errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' block {} references missing instruction {}",
                        routine.name, block_id.0, instr_id.0
                    ),
                ));
                continue;
            };
            if let Some(result) = instr.result {
                if routine.locals.get(result).is_none() {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "lowered routine '{}' instruction {} writes to missing local {}",
                            routine.name, instr_id.0, result.0
                        ),
                    ));
                }
            }
            verify_instruction(
                workspace,
                package,
                routine,
                instr,
                valid_global_ids,
                valid_routine_ids,
                errors,
            );
        }

        match block.terminator.as_ref() {
            Some(LoweredTerminator::Jump { target }) => {
                enqueue_target(
                    routine.name.as_str(),
                    &routine.blocks,
                    errors,
                    &mut queue,
                    *target,
                );
            }
            Some(LoweredTerminator::Branch {
                condition,
                then_block,
                else_block,
            }) => {
                if routine.locals.get(*condition).is_none() {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "lowered routine '{}' branch uses missing local {}",
                            routine.name, condition.0
                        ),
                    ));
                }
                enqueue_target(
                    routine.name.as_str(),
                    &routine.blocks,
                    errors,
                    &mut queue,
                    *then_block,
                );
                enqueue_target(
                    routine.name.as_str(),
                    &routine.blocks,
                    errors,
                    &mut queue,
                    *else_block,
                );
            }
            Some(LoweredTerminator::Return { value: Some(local) }) => {
                if routine.locals.get(*local).is_none() {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "lowered routine '{}' return uses missing local {}",
                            routine.name, local.0
                        ),
                    ));
                }
            }
            Some(LoweredTerminator::Return { value: None }) => {}
            Some(LoweredTerminator::Report { value: Some(local) }) => {
                if routine.locals.get(*local).is_none() {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "lowered routine '{}' report uses missing local {}",
                            routine.name, local.0
                        ),
                    ));
                }
            }
            Some(LoweredTerminator::Report { value: None }) => {}
            Some(LoweredTerminator::Panic { value: Some(local) }) => {
                if routine.locals.get(*local).is_none() {
                    errors.push(LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "lowered routine '{}' panic uses missing local {}",
                            routine.name, local.0
                        ),
                    ));
                }
            }
            Some(LoweredTerminator::Panic { value: None }) => {}
            Some(LoweredTerminator::Unreachable) => {}
            None => {}
        }
    }

    let reachable_unterminated = reachable
        .iter()
        .filter(|block_id| {
            routine
                .blocks
                .get(**block_id)
                .map(|block| block.terminator.is_none())
                .unwrap_or(false)
        })
        .count();
    if reachable_unterminated > 1 {
        errors.push(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "lowered routine '{}' retains {} reachable unterminated blocks; at most one fallthrough block is allowed",
                routine.name, reachable_unterminated
            ),
        ));
    }

    for (block_id, _block) in routine.blocks.iter_with_ids() {
        if !reachable.contains(&block_id) {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' produced unreachable block {}",
                    routine.name, block_id.0
                ),
            ));
        }
    }
}

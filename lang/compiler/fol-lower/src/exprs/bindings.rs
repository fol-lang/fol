use super::cursor::{LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::lower_expression_expected;
use crate::{control::LoweredInstrKind, ids::LoweredTypeId, LoweringError, LoweringErrorKind};
use fol_parser::ast::AstNode;
use fol_resolver::{PackageIdentity, ScopeId, SourceUnitId, SymbolKind};
use std::collections::BTreeMap;

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_local_binding(
    typed_package: &fol_typecheck::TypedPackage,
    type_table: &crate::LoweredTypeTable,
    checked_type_map: &BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    current_identity: &PackageIdentity,
    decl_index: &WorkspaceDeclIndex,
    cursor: &mut RoutineCursor<'_>,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    name: &str,
    kind: SymbolKind,
    value: Option<&AstNode>,
) -> Result<Option<LoweredValue>, LoweringError> {
    let Some(symbol_id) = crate::decls::find_symbol_for_declaration(
        &typed_package.program,
        source_unit_id,
        kind,
        name,
        syntax_id,
    ) else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("binding '{name}' does not retain its exact syntax-anchored lowering symbol"),
        ));
    };
    // Guard-VALUE binding (V3_MEM §8.3): `var[mut, bor] guard: T = ([bor]state).lock()`
    // aliases `guard` to the mutex local and acquires its guard, reusing the
    // existing mutex-guard machinery — `guard.value` then resolves to the guarded
    // mutex field (backend renders through the synthetic guard) and the lock
    // releases at scope exit. No fresh local / value copy is created.
    if let Some(value_node) = value {
        let mut unwrapped = value_node;
        while let AstNode::Commented { node, .. } = unwrapped {
            unwrapped = node.as_ref();
        }
        if let AstNode::MethodCall {
            object,
            method,
            args,
            ..
        } = unwrapped
        {
            if method == "lock" && args.is_empty() {
                if let AstNode::OwnershipOp {
                    options, operand, ..
                } = object.as_ref()
                {
                    if matches!(
                        options.as_slice(),
                        [fol_parser::ast::options::OwnershipOption::Borrow]
                    ) {
                        if let Some(mutex) = super::expressions::direct_local_identifier_value(
                            typed_package,
                            cursor,
                            operand.as_ref(),
                        )
                        .filter(|value| cursor.routine.mutex_params.contains(&value.local_id))
                        {
                            cursor
                                .routine
                                .local_symbols
                                .insert(symbol_id, mutex.local_id);
                            cursor.register_mutex_guard(mutex.local_id)?;
                            cursor.push_instr(
                                None,
                                LoweredInstrKind::MutexLock {
                                    mutex: mutex.local_id,
                                },
                            )?;
                            return Ok(None);
                        }
                    }
                }
            }
        }
    }
    let type_id = typed_package
        .program
        .typed_symbol(symbol_id)
        .and_then(|symbol| symbol.declared_type)
        .and_then(|checked_type| checked_type_map.get(&checked_type).copied())
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("binding '{name}' does not retain a lowered storage type"),
            )
        })?;
    let local_id = cursor.allocate_local(type_id, Some(name.to_string()));
    cursor.routine.local_symbols.insert(symbol_id, local_id);
    // A `var state: mux[T]` local is a first-class managed mutex (V3_MEM §8.3):
    // register it like a `mux[T]` parameter so `.lock()`/`.unlock()`/guarded
    // field access and the backend `rt::FolMutex<T>` rendering apply to it.
    if typed_package
        .program
        .typed_symbol(symbol_id)
        .is_some_and(|symbol| symbol.is_mutex)
    {
        cursor.routine.mutex_params.insert(local_id);
    }
    // A `fin` local runs its custom finalizer at scope exit (V3_MEM §6.1). A fin
    // record is affine (move-only) so its finalizer sees the complete value; the
    // finalize call consumes the local, so it takes the place of the structural
    // lexical drop rather than being emitted alongside it.
    let claims_fin = typed_package
        .program
        .typed_symbol(symbol_id)
        .and_then(|symbol| symbol.declared_type)
        .is_some_and(|checked_type| typed_package.program.type_resolves_to_fin(checked_type));
    if type_table.moves_on_transfer(type_id) && !claims_fin {
        // A binding can be moved on one continuing branch and reinitialized
        // on another. Typecheck conservatively records the merged binding as
        // moved so later reads are rejected, but the reinitialized branch
        // still owns a value that must be released at lexical exit. Backend
        // moves leave the named slot holding its default sentinel, so one
        // unconditional lexical drop is valid on both paths.
        cursor.register_lexical_drop(local_id)?;
    }

    if claims_fin {
        let (_callee_identity, callee) = super::calls::resolve_method_target(
            typed_package,
            checked_type_map,
            current_identity,
            decl_index,
            "finalize",
            type_id,
            None,
        )?;
        cursor.register_finalization(super::cursor::FinalizeEntry {
            local: local_id,
            symbol: symbol_id,
            callee,
        })?;
    }

    if let Some(value) = value {
        let lowered_value = lower_expression_expected(
            typed_package,
            type_table,
            checked_type_map,
            current_identity,
            decl_index,
            cursor,
            source_unit_id,
            scope_id,
            Some(type_id),
            value,
        )?;
        cursor.push_instr(
            None,
            LoweredInstrKind::StoreLocal {
                local: local_id,
                value: lowered_value.local_id,
            },
        )?;
        Ok(Some(LoweredValue {
            local_id,
            type_id,
            recoverable_error_type: None,
        }))
    } else {
        Ok(Some(LoweredValue {
            local_id,
            type_id,
            recoverable_error_type: None,
        }))
    }
}

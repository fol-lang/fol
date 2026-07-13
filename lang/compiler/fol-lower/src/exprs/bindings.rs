use super::cursor::{LoweredValue, RoutineCursor, WorkspaceDeclIndex};
use super::expressions::lower_expression_expected;
use crate::{control::LoweredInstrKind, ids::LoweredTypeId, LoweringError, LoweringErrorKind};
use fol_parser::ast::AstNode;
use fol_resolver::{PackageIdentity, ScopeId, SourceUnitId, SymbolKind};
use std::collections::BTreeMap;

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
    if type_needs_lexical_drop(type_table, type_id, 0)
        && typed_package
            .program
            .moved_binding_origin(symbol_id)
            .is_none()
    {
        cursor.register_lexical_drop(local_id)?;
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

fn type_needs_lexical_drop(
    type_table: &crate::LoweredTypeTable,
    type_id: LoweredTypeId,
    depth: usize,
) -> bool {
    if depth > 32 {
        return false;
    }
    match type_table.get(type_id) {
        Some(crate::LoweredType::Owned { .. }) => true,
        Some(crate::LoweredType::Pointer { shared: false, .. }) => true,
        Some(crate::LoweredType::Optional { inner })
        | Some(crate::LoweredType::Error { inner: Some(inner) }) => {
            type_needs_lexical_drop(type_table, *inner, depth + 1)
        }
        _ => false,
    }
}

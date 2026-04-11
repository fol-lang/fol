use crate::{
    LoweredBlock, LoweredLocal, LoweredPackage, LoweredRoutine, LoweringError,
    LoweringErrorKind, LoweringResult,
};
use fol_parser::ast::{AstNode, ParsedSourceUnitKind};
use fol_resolver::{SourceUnitId, SymbolId, SymbolKind};
use fol_typecheck::CheckedType;

use super::symbol_lookup::{find_local_symbol_id, find_symbol_in_scope_or_descendants};
use super::type_decls::lower_symbol_signature;

pub fn lower_routine_signatures(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &mut LoweredPackage,
) -> LoweringResult<()> {
    let mut errors = Vec::new();

    for (source_unit_index, source_unit) in typed_package
        .program
        .resolved()
        .syntax()
        .source_units
        .iter()
        .enumerate()
    {
        if source_unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit_id = SourceUnitId(source_unit_index);
        for item in &source_unit.items {
            let Some(name) = routine_name(&item.node) else {
                continue;
            };

            match find_local_symbol_id(
                &typed_package.program,
                source_unit_id,
                SymbolKind::Routine,
                name,
            ) {
                Some(symbol_id) => {
                    match lower_symbol_signature(typed_package, lowered_package, symbol_id) {
                        Ok(signature_type) => {
                            lowered_package
                                .routine_signatures
                                .insert(symbol_id, signature_type);
                        }
                        Err(error) => errors.push(error),
                    }
                }
                None => errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("top-level routine '{name}' does not retain a resolved symbol"),
                )),
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn lower_routine_declarations(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &mut LoweredPackage,
    next_routine_index: &mut usize,
) -> LoweringResult<()> {
    let mut errors = Vec::new();

    for (source_unit_index, source_unit) in typed_package
        .program
        .resolved()
        .syntax()
        .source_units
        .iter()
        .enumerate()
    {
        if source_unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit_id = SourceUnitId(source_unit_index);
        for item in &source_unit.items {
            let (name, syntax_id, params) = match &item.node {
                AstNode::FunDecl {
                    name,
                    syntax_id,
                    params,
                    ..
                }
                | AstNode::ProDecl {
                    name,
                    syntax_id,
                    params,
                    ..
                }
                | AstNode::LogDecl {
                    name,
                    syntax_id,
                    params,
                    ..
                } => (name.as_str(), *syntax_id, params.as_slice()),
                _ => continue,
            };

            match find_local_symbol_id(
                &typed_package.program,
                source_unit_id,
                SymbolKind::Routine,
                name,
            ) {
                Some(symbol_id) => match lower_routine_decl(
                    typed_package,
                    lowered_package,
                    symbol_id,
                    source_unit_id,
                    name,
                    syntax_id,
                    params,
                    next_routine_index,
                ) {
                    Ok(routine) => {
                        lowered_package.routines.push(routine.id);
                        lowered_package.routine_decls.insert(routine.id, routine);
                    }
                    Err(error) => errors.push(error),
                },
                None => errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("top-level routine '{name}' does not retain a resolved symbol"),
                )),
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn lower_routine_decl(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &LoweredPackage,
    symbol_id: SymbolId,
    source_unit_id: SourceUnitId,
    name: &str,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    params: &[fol_parser::ast::Parameter],
    next_routine_index: &mut usize,
) -> Result<LoweredRoutine, LoweringError> {
    let signature = lowered_package
        .routine_signatures
        .get(&symbol_id)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "routine symbol {} does not retain a lowered signature",
                    symbol_id.0
                ),
            )
        })?;
    let typed_symbol = typed_package
        .program
        .typed_symbol(symbol_id)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "routine symbol {} disappeared from typed facts",
                    symbol_id.0
                ),
            )
        })?;

    let mut routine = LoweredRoutine::new(
        crate::LoweredRoutineId(*next_routine_index),
        name,
        crate::LoweredBlockId(0),
    );
    *next_routine_index += 1;
    routine.symbol_id = Some(symbol_id);
    routine.source_unit_id = Some(source_unit_id);
    routine.signature = Some(signature);
    routine.receiver_type = typed_symbol.receiver_type.and_then(|receiver_type| {
        lowered_package
            .checked_type_map
            .get(&receiver_type)
            .copied()
    });
    let entry_block = routine.blocks.push(LoweredBlock {
        id: crate::LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: None,
    });
    routine.entry_block = entry_block;

    let mut next_local_index = 0;
    if let Some(receiver_type) = routine.receiver_type {
        let local_id = routine.locals.push(LoweredLocal {
            id: crate::LoweredLocalId(next_local_index),
            type_id: Some(receiver_type),
            name: Some("self".to_string()),
        });
        routine.params.push(local_id);
        next_local_index += 1;
    }

    let routine_scope_id = syntax_id
        .and_then(|syntax_id| typed_package.program.resolved().scope_for_syntax(syntax_id))
        .unwrap_or(typed_symbol.scope_id);
    let checked_signature = typed_symbol.declared_type.ok_or_else(|| {
        LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "routine symbol {} does not retain a checked signature",
                symbol_id.0
            ),
        )
    })?;
    let checked_param_types = match typed_package
        .program
        .type_table()
        .get(checked_signature)
        .cloned()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "routine symbol {} lost its checked signature shape",
                    symbol_id.0
                ),
            )
        })? {
        CheckedType::Routine(signature) => signature.params,
        _ => {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "routine symbol {} no longer lowers to a routine shape",
                    symbol_id.0
                ),
            ))
        }
    };

    for (param, checked_param_type) in params.iter().zip(checked_param_types.into_iter()) {
        let param_type = lowered_package
            .checked_type_map
            .get(&checked_param_type)
            .copied()
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "routine parameter '{}' on symbol {} does not map to a lowered type",
                        param.name, symbol_id.0
                    ),
                )
            })?;
        let local_id = routine.locals.push(LoweredLocal {
            id: crate::LoweredLocalId(next_local_index),
            type_id: Some(param_type),
            name: Some(param.name.clone()),
        });
        let Some(param_symbol_id) = find_symbol_in_scope_or_descendants(
            &typed_package.program,
            source_unit_id,
            routine_scope_id,
            SymbolKind::Parameter,
            &param.name,
        ) else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "routine parameter '{}' does not map to a resolved symbol in its lowered scope",
                    param.name
                ),
            ));
        };
        routine.local_symbols.insert(param_symbol_id, local_id);
        routine.params.push(local_id);
        next_local_index += 1;
    }

    Ok(routine)
}

fn routine_name(node: &AstNode) -> Option<&str> {
    match node {
        AstNode::FunDecl { name, .. }
        | AstNode::ProDecl { name, .. }
        | AstNode::LogDecl { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

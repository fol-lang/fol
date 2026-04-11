use crate::{
    LoweredFieldLayout, LoweredGlobal, LoweredPackage, LoweredTypeDecl, LoweredTypeDeclKind,
    LoweredVariantLayout, LoweringError, LoweringErrorKind, LoweringResult,
};
use fol_parser::ast::{AstNode, ParsedSourceUnitKind, TypeDefinition};
use fol_resolver::{SourceUnitId, SymbolId, SymbolKind};
use fol_typecheck::CheckedType;
use std::collections::BTreeSet;

use super::symbol_lookup::find_local_symbol_id;

pub fn lower_alias_declarations(
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
            let AstNode::AliasDecl { name, .. } = &item.node else {
                continue;
            };

            match find_local_symbol_id(
                &typed_package.program,
                source_unit_id,
                SymbolKind::Alias,
                name,
            ) {
                Some(symbol_id) => {
                    if symbol_has_generic_params(&typed_package.program, symbol_id) {
                        continue;
                    }
                    match lower_symbol_signature(typed_package, lowered_package, symbol_id) {
                        Ok(target_type) => {
                            lowered_package.type_decls.insert(
                                symbol_id,
                                LoweredTypeDecl {
                                    symbol_id,
                                    source_unit_id,
                                    name: name.clone(),
                                    runtime_type: target_type,
                                    kind: LoweredTypeDeclKind::Alias { target_type },
                                },
                            );
                        }
                        Err(error) => errors.push(error),
                    }
                }
                None => errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("alias '{name}' does not retain a resolved symbol"),
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

pub fn lower_record_declarations(
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
            let AstNode::TypeDecl {
                name,
                type_def: TypeDefinition::Record { .. },
                ..
            } = &item.node
            else {
                continue;
            };

            match find_local_symbol_id(
                &typed_package.program,
                source_unit_id,
                SymbolKind::Type,
                name,
            ) {
                Some(symbol_id) => {
                    if symbol_has_generic_params(&typed_package.program, symbol_id) {
                        continue;
                    }
                    match lower_record_decl(
                        typed_package,
                        lowered_package,
                        symbol_id,
                        source_unit_id,
                        name,
                    ) {
                        Ok(type_decl) => {
                            lowered_package.type_decls.insert(symbol_id, type_decl);
                        }
                        Err(error) => errors.push(error),
                    }
                }
                None => errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("record type '{name}' does not retain a resolved symbol"),
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

pub fn lower_entry_declarations(
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
            let AstNode::TypeDecl {
                name,
                type_def: TypeDefinition::Entry { .. },
                ..
            } = &item.node
            else {
                continue;
            };

            match find_local_symbol_id(
                &typed_package.program,
                source_unit_id,
                SymbolKind::Type,
                name,
            ) {
                Some(symbol_id) => {
                    if symbol_has_generic_params(&typed_package.program, symbol_id) {
                        continue;
                    }
                    match lower_entry_decl(
                        typed_package,
                        lowered_package,
                        symbol_id,
                        source_unit_id,
                        name,
                    ) {
                        Ok(type_decl) => {
                            lowered_package.type_decls.insert(symbol_id, type_decl);
                        }
                        Err(error) => errors.push(error),
                    }
                }
                None => errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("entry type '{name}' does not retain a resolved symbol"),
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

pub fn synthesize_structural_runtime_type_declarations(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &mut LoweredPackage,
) -> LoweringResult<()> {
    let Some(source_unit_id) = typed_package
        .program
        .ordinary_source_units()
        .next()
        .map(|unit| unit.source_unit_id)
    else {
        return Ok(());
    };

    let mut existing_runtime_types = lowered_package
        .type_decls
        .values()
        .map(|type_decl| type_decl.runtime_type)
        .collect::<BTreeSet<_>>();
    let mut errors = Vec::new();

    for (checked_type_id, runtime_type) in lowered_package.checked_type_map.clone() {
        if existing_runtime_types.contains(&runtime_type) {
            continue;
        }

        let Some(checked_type) = typed_package.program.type_table().get(checked_type_id).cloned() else {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "checked type {} disappeared while synthesizing structural runtime declarations",
                    checked_type_id.0
                ),
            ));
            continue;
        };
        if checked_type_contains_generic_parameter(&checked_type, &typed_package.program) {
            continue;
        }

        let Some(type_decl) = synthesize_structural_type_decl(
            lowered_package,
            checked_type,
            runtime_type,
            source_unit_id,
        ) else {
            continue;
        };
        existing_runtime_types.insert(runtime_type);
        lowered_package
            .type_decls
            .insert(type_decl.symbol_id, type_decl);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn lower_global_declarations(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &mut LoweredPackage,
    next_global_index: &mut usize,
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
            let (name, kind, mutable) = match &item.node {
                AstNode::VarDecl { name, .. } => (name.as_str(), SymbolKind::ValueBinding, true),
                AstNode::LabDecl { name, .. } => (name.as_str(), SymbolKind::LabelBinding, false),
                _ => continue,
            };

            match find_local_symbol_id(&typed_package.program, source_unit_id, kind, name) {
                Some(symbol_id) => match lower_global_decl(
                    typed_package,
                    lowered_package,
                    symbol_id,
                    source_unit_id,
                    name,
                    mutable,
                    next_global_index,
                ) {
                    Ok(global) => {
                        lowered_package.globals.push(global.id);
                        lowered_package.global_decls.insert(global.id, global);
                    }
                    Err(error) => errors.push(error),
                },
                None => errors.push(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!("top-level binding '{name}' does not retain a resolved symbol"),
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

pub(super) fn lower_symbol_signature(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &LoweredPackage,
    symbol_id: SymbolId,
) -> Result<crate::LoweredTypeId, LoweringError> {
    let checked_signature = typed_package
        .program
        .typed_symbol(symbol_id)
        .and_then(|typed_symbol| typed_symbol.declared_type)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "routine symbol {} does not retain a typed signature",
                    symbol_id.0
                ),
            )
        })?;

    lowered_package
        .checked_type_map
        .get(&checked_signature)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "routine symbol {} does not map to a lowering-owned signature type",
                    symbol_id.0
                ),
            )
        })
}

fn lower_record_decl(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &LoweredPackage,
    symbol_id: SymbolId,
    source_unit_id: SourceUnitId,
    name: &str,
) -> Result<LoweredTypeDecl, LoweringError> {
    let checked_type = typed_package
        .program
        .typed_symbol(symbol_id)
        .and_then(|typed_symbol| typed_symbol.declared_type)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "record symbol {} does not retain a typed runtime shape",
                    symbol_id.0
                ),
            )
        })?;
    let runtime_type = lowered_package
        .checked_type_map
        .get(&checked_type)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "record symbol {} does not map to a lowered runtime type",
                    symbol_id.0
                ),
            )
        })?;
    let CheckedType::Record { fields } = typed_package
        .program
        .type_table()
        .get(checked_type)
        .cloned()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "record symbol {} lost its typed runtime definition",
                    symbol_id.0
                ),
            )
        })?
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "record symbol {} no longer lowers to a record shape",
                symbol_id.0
            ),
        ));
    };
    let mut lowered_fields = Vec::new();
    for (field_name, field_type) in fields {
        let lowered_field_type = lowered_package
            .checked_type_map
            .get(&field_type)
            .copied()
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "record field '{field_name}' on symbol {} does not map to a lowered type",
                        symbol_id.0
                    ),
                )
            })?;
        lowered_fields.push(LoweredFieldLayout {
            name: field_name,
            type_id: lowered_field_type,
        });
    }

    Ok(LoweredTypeDecl {
        symbol_id,
        source_unit_id,
        name: name.to_string(),
        runtime_type,
        kind: LoweredTypeDeclKind::Record {
            fields: lowered_fields,
        },
    })
}

fn synthesize_structural_type_decl(
    lowered_package: &LoweredPackage,
    checked_type: CheckedType,
    runtime_type: crate::LoweredTypeId,
    source_unit_id: SourceUnitId,
) -> Option<LoweredTypeDecl> {
    let symbol_id = synthetic_type_decl_symbol_id(runtime_type);
    match checked_type {
        CheckedType::Record { fields } => {
            let mut lowered_fields = Vec::new();
            for (field_name, field_type) in fields {
                let lowered_field_type = lowered_package.checked_type_map.get(&field_type).copied()?;
                lowered_fields.push(LoweredFieldLayout {
                    name: field_name,
                    type_id: lowered_field_type,
                });
            }
            Some(LoweredTypeDecl {
                symbol_id,
                source_unit_id,
                name: format!("record_t{}", runtime_type.0),
                runtime_type,
                kind: LoweredTypeDeclKind::Record {
                    fields: lowered_fields,
                },
            })
        }
        CheckedType::Entry { variants } => {
            let mut lowered_variants = Vec::new();
            for (variant_name, variant_type) in variants {
                let lowered_variant_type = match variant_type {
                    Some(variant_type) => Some(
                        lowered_package
                            .checked_type_map
                            .get(&variant_type)
                            .copied()?,
                    ),
                    None => None,
                };
                lowered_variants.push(LoweredVariantLayout {
                    name: variant_name,
                    payload_type: lowered_variant_type,
                });
            }
            Some(LoweredTypeDecl {
                symbol_id,
                source_unit_id,
                name: format!("entry_t{}", runtime_type.0),
                runtime_type,
                kind: LoweredTypeDeclKind::Entry {
                    variants: lowered_variants,
                },
            })
        }
        _ => None,
    }
}

fn synthetic_type_decl_symbol_id(runtime_type: crate::LoweredTypeId) -> SymbolId {
    SymbolId(usize::MAX - runtime_type.0)
}

fn symbol_has_generic_params(program: &fol_typecheck::TypedProgram, symbol_id: SymbolId) -> bool {
    program
        .typed_symbol(symbol_id)
        .is_some_and(|typed_symbol| !typed_symbol.generic_params.is_empty())
}

fn checked_type_contains_generic_parameter(
    checked_type: &CheckedType,
    program: &fol_typecheck::TypedProgram,
) -> bool {
    match checked_type {
        CheckedType::Declared {
            kind: fol_typecheck::DeclaredTypeKind::GenericParameter,
            ..
        } => true,
        CheckedType::Builtin(_) => false,
        CheckedType::Declared { symbol, .. } => program
            .typed_symbol(*symbol)
            .and_then(|typed_symbol| typed_symbol.declared_type)
            .and_then(|type_id| program.type_table().get(type_id))
            .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program)),
        CheckedType::Array { element_type, .. }
        | CheckedType::Vector { element_type }
        | CheckedType::Sequence { element_type }
        | CheckedType::Optional { inner: element_type } => program
            .type_table()
            .get(*element_type)
            .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program)),
        CheckedType::Set { member_types } => member_types.iter().any(|member| {
            program
                .type_table()
                .get(*member)
                .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
        }),
        CheckedType::Map {
            key_type,
            value_type,
        } => [key_type, value_type].iter().any(|type_id| {
            program
                .type_table()
                .get(**type_id)
                .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
        }),
        CheckedType::Error { inner } => inner.is_some_and(|inner| {
            program
                .type_table()
                .get(inner)
                .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
        }),
        CheckedType::Record { fields } => fields.values().any(|field| {
            program
                .type_table()
                .get(*field)
                .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
        }),
        CheckedType::Entry { variants } => variants.values().flatten().any(|variant| {
            program
                .type_table()
                .get(*variant)
                .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
        }),
        CheckedType::Routine(signature) => {
            signature.params.iter().any(|param| {
                program
                    .type_table()
                    .get(*param)
                    .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
            }) || signature.return_type.is_some_and(|return_type| {
                program
                    .type_table()
                    .get(return_type)
                    .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
            }) || signature.error_type.is_some_and(|error_type| {
                program
                    .type_table()
                    .get(error_type)
                    .is_some_and(|checked| checked_type_contains_generic_parameter(checked, program))
            })
        }
    }
}

fn lower_entry_decl(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &LoweredPackage,
    symbol_id: SymbolId,
    source_unit_id: SourceUnitId,
    name: &str,
) -> Result<LoweredTypeDecl, LoweringError> {
    let checked_type = typed_package
        .program
        .typed_symbol(symbol_id)
        .and_then(|typed_symbol| typed_symbol.declared_type)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "entry symbol {} does not retain a typed runtime shape",
                    symbol_id.0
                ),
            )
        })?;
    let runtime_type = lowered_package
        .checked_type_map
        .get(&checked_type)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "entry symbol {} does not map to a lowered runtime type",
                    symbol_id.0
                ),
            )
        })?;
    let CheckedType::Entry { variants } = typed_package
        .program
        .type_table()
        .get(checked_type)
        .cloned()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "entry symbol {} lost its typed runtime definition",
                    symbol_id.0
                ),
            )
        })?
    else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "entry symbol {} no longer lowers to an entry shape",
                symbol_id.0
            ),
        ));
    };
    let mut lowered_variants = Vec::new();
    for (variant_name, variant_type) in variants {
        let lowered_variant_type = variant_type
            .map(|variant_type| {
                lowered_package
                    .checked_type_map
                    .get(&variant_type)
                    .copied()
                    .ok_or_else(|| {
                        LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!(
                                "entry variant '{variant_name}' on symbol {} does not map to a lowered type",
                                symbol_id.0
                            ),
                        )
                    })
            })
            .transpose()?;
        lowered_variants.push(LoweredVariantLayout {
            name: variant_name,
            payload_type: lowered_variant_type,
        });
    }

    Ok(LoweredTypeDecl {
        symbol_id,
        source_unit_id,
        name: name.to_string(),
        runtime_type,
        kind: LoweredTypeDeclKind::Entry {
            variants: lowered_variants,
        },
    })
}

pub(super) fn lower_global_decl(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &LoweredPackage,
    symbol_id: SymbolId,
    source_unit_id: SourceUnitId,
    name: &str,
    mutable: bool,
    next_global_index: &mut usize,
) -> Result<LoweredGlobal, LoweringError> {
    let checked_type = typed_package
        .program
        .typed_symbol(symbol_id)
        .and_then(|typed_symbol| typed_symbol.declared_type)
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "global symbol {} does not retain a checked type",
                    symbol_id.0
                ),
            )
        })?;
    let type_id = lowered_package
        .checked_type_map
        .get(&checked_type)
        .copied()
        .ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "global symbol {} does not map to a lowered type",
                    symbol_id.0
                ),
            )
        })?;
    let global = LoweredGlobal {
        id: crate::LoweredGlobalId(*next_global_index),
        symbol_id,
        source_unit_id,
        name: name.to_string(),
        type_id,
        mutable,
    };
    *next_global_index += 1;
    Ok(global)
}

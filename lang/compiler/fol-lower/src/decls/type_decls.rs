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

        let Some(mut checked_type) = typed_package
            .program
            .type_table()
            .get(checked_type_id)
            .cloned()
        else {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "checked type {} disappeared while synthesizing structural runtime declarations",
                    checked_type_id.0
                ),
            ));
            continue;
        };
        // A generic instantiation (`Box[int]`) is a nominal `Declared` node;
        // its runtime struct is its substituted structural record, reached via
        // the apparent override. Synthesize the decl from that shape.
        if matches!(&checked_type, CheckedType::Declared { args, .. } if !args.is_empty()) {
            if let Some(apparent_id) = typed_package
                .program
                .apparent_type_override(checked_type_id)
            {
                if let Some(apparent) = typed_package.program.type_table().get(apparent_id).cloned()
                {
                    checked_type = apparent;
                }
            }
        }
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
            let (name, kind, mutable, value) = match &item.node {
                // `con` parses to a VarDecl carrying the Immutable option; a
                // constant global must not classify (and render) as mutable.
                AstNode::VarDecl {
                    name,
                    options,
                    value,
                    ..
                } => (
                    name.as_str(),
                    SymbolKind::ValueBinding,
                    !options.contains(&fol_parser::ast::VarOption::Immutable),
                    value.as_deref(),
                ),
                AstNode::LabDecl { name, value, .. } => (
                    name.as_str(),
                    SymbolKind::LabelBinding,
                    false,
                    value.as_deref(),
                ),
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
                    value,
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
                let lowered_field_type =
                    lowered_package.checked_type_map.get(&field_type).copied()?;
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
    fn contains_type_id(
        type_id: fol_typecheck::CheckedTypeId,
        program: &fol_typecheck::TypedProgram,
        visiting: &mut BTreeSet<fol_typecheck::CheckedTypeId>,
    ) -> bool {
        if !visiting.insert(type_id) {
            return false;
        }
        let contains = program
            .type_table()
            .get(type_id)
            .is_some_and(|checked| contains(checked, program, visiting));
        visiting.remove(&type_id);
        contains
    }

    fn contains(
        checked_type: &CheckedType,
        program: &fol_typecheck::TypedProgram,
        visiting: &mut BTreeSet<fol_typecheck::CheckedTypeId>,
    ) -> bool {
        match checked_type {
            CheckedType::Declared {
                kind: fol_typecheck::DeclaredTypeKind::GenericParameter,
                ..
            } => true,
            CheckedType::Builtin(_) => false,
            // A generic INSTANTIATION (`Box[int]`, args non-empty) is generic iff
            // one of its concrete type arguments is — checking the declaration's
            // template (which always mentions `T`) would wrongly skip every
            // concrete instance's runtime decl.
            CheckedType::Declared { args, .. } if !args.is_empty() => args
                .iter()
                .any(|arg| contains_type_id(*arg, program, visiting)),
            CheckedType::Declared { symbol, .. } => program
                .typed_symbol(*symbol)
                .and_then(|typed_symbol| typed_symbol.declared_type)
                .is_some_and(|type_id| contains_type_id(type_id, program, visiting)),
            CheckedType::Array { element_type, .. }
            | CheckedType::Vector { element_type }
            | CheckedType::Sequence { element_type }
            | CheckedType::Channel { element_type }
            | CheckedType::ChannelSender { element_type }
            | CheckedType::ChannelReceiver { element_type }
            | CheckedType::Optional {
                inner: element_type,
            }
            | CheckedType::Owned {
                inner: element_type,
            }
            | CheckedType::Borrowed {
                inner: element_type,
                ..
            }
            | CheckedType::Pointer {
                target: element_type,
                ..
            } => contains_type_id(*element_type, program, visiting),
            CheckedType::Eventual {
                value_type,
                error_type,
            } => {
                contains_type_id(*value_type, program, visiting)
                    || error_type
                        .is_some_and(|error_type| contains_type_id(error_type, program, visiting))
            }
            CheckedType::Set { member_types } => member_types
                .iter()
                .any(|member| contains_type_id(*member, program, visiting)),
            CheckedType::Map {
                key_type,
                value_type,
            } => [key_type, value_type]
                .iter()
                .any(|type_id| contains_type_id(**type_id, program, visiting)),
            CheckedType::Error { inner } => {
                inner.is_some_and(|inner| contains_type_id(inner, program, visiting))
            }
            CheckedType::Record { fields } => fields
                .values()
                .any(|field| contains_type_id(*field, program, visiting)),
            CheckedType::Entry { variants } => variants
                .values()
                .flatten()
                .any(|variant| contains_type_id(*variant, program, visiting)),
            CheckedType::Routine(signature) => {
                signature
                    .params
                    .iter()
                    .any(|param| contains_type_id(*param, program, visiting))
                    || signature
                        .return_type
                        .is_some_and(|return_type| contains_type_id(return_type, program, visiting))
                    || signature
                        .error_type
                        .is_some_and(|error_type| contains_type_id(error_type, program, visiting))
            }
        }
    }

    contains(checked_type, program, &mut BTreeSet::new())
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

pub(super) #[allow(clippy::too_many_arguments)]
fn lower_global_decl(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &LoweredPackage,
    symbol_id: SymbolId,
    source_unit_id: SourceUnitId,
    name: &str,
    mutable: bool,
    value: Option<&AstNode>,
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
    // Global reads materialize through the declared initializer; without it a
    // `con LIMIT: int = 9` silently read the type default. V3 restricts global
    // initializers to literal values.
    let mut stripped = value;
    while let Some(AstNode::Commented { node, .. }) = stripped {
        stripped = Some(node);
    }
    fn literal_operand(
        literal: &fol_parser::ast::Literal,
    ) -> Option<crate::control::LoweredOperand> {
        Some(match literal {
            fol_parser::ast::Literal::Integer(value) => crate::control::LoweredOperand::Int(*value),
            fol_parser::ast::Literal::Float(value) => {
                crate::control::LoweredOperand::Float(value.to_bits())
            }
            fol_parser::ast::Literal::String(value) => {
                crate::control::LoweredOperand::Str(value.clone())
            }
            fol_parser::ast::Literal::Character(value) => {
                crate::control::LoweredOperand::Char(*value)
            }
            fol_parser::ast::Literal::Boolean(value) => {
                crate::control::LoweredOperand::Bool(*value)
            }
            fol_parser::ast::Literal::Nil => crate::control::LoweredOperand::Nil,
        })
    }
    let unsupported_initializer = || {
        LoweringError::with_kind(
            LoweringErrorKind::Unsupported,
            format!(
                "global binding '{name}' requires a literal initializer (a scalar, or a record of scalars) in V3; construct other values inside a routine"
            ),
        )
    };
    let initializer = match stripped {
        Some(AstNode::Literal(literal)) => Some(crate::model::LoweredGlobalInit::Operand(
            literal_operand(literal).ok_or_else(unsupported_initializer)?,
        )),
        Some(AstNode::RecordInit { fields, .. }) => {
            let mut lowered_fields = Vec::with_capacity(fields.len());
            for field in fields {
                let mut field_value = &field.value;
                while let AstNode::Commented { node, .. } = field_value {
                    field_value = node;
                }
                let AstNode::Literal(literal) = field_value else {
                    return Err(unsupported_initializer());
                };
                let operand = literal_operand(literal).ok_or_else(unsupported_initializer)?;
                lowered_fields.push((field.name.clone(), operand));
            }
            Some(crate::model::LoweredGlobalInit::Record {
                fields: lowered_fields,
            })
        }
        Some(reference @ (AstNode::Identifier { .. } | AstNode::QualifiedIdentifier { .. })) => {
            let syntax_id = match reference {
                AstNode::Identifier { syntax_id, .. } => *syntax_id,
                AstNode::QualifiedIdentifier { path } => path.syntax_id(),
                _ => None,
            };
            let referenced = syntax_id.and_then(|syntax_id| {
                typed_package
                    .program
                    .resolved()
                    .references
                    .iter()
                    .find(|candidate| candidate.syntax_id == Some(syntax_id))
                    .and_then(|candidate| candidate.resolved)
            });
            let Some(referenced) = referenced else {
                return Err(unsupported_initializer());
            };
            let (identity, origin_symbol) = typed_package
                .program
                .resolved()
                .symbol(referenced)
                .and_then(|symbol| symbol.mounted_from.as_ref())
                .map(|provenance| {
                    (
                        provenance.package_identity.clone(),
                        provenance.foreign_symbol,
                    )
                })
                .unwrap_or_else(|| (lowered_package.identity.clone(), referenced));
            Some(crate::model::LoweredGlobalInit::GlobalRef {
                package: identity,
                symbol: origin_symbol,
            })
        }
        Some(_) => {
            return Err(unsupported_initializer());
        }
        None => None,
    };
    let global = LoweredGlobal {
        id: crate::LoweredGlobalId(*next_global_index),
        symbol_id,
        source_unit_id,
        name: name.to_string(),
        type_id,
        mutable,
        initializer,
    };
    *next_global_index += 1;
    Ok(global)
}

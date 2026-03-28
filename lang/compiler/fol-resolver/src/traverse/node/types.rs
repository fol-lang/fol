use crate::{
    model::ResolvedProgram, ResolverError, ResolverSession, ScopeId, SourceUnitId,
};
use fol_parser::ast::{FolType, TypeDefinition};

use super::super::references::record_named_type_reference;
use super::super::references::record_qualified_type_reference;
use super::super::references::record_contract_reference;

pub fn resolve_type_reference(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    typ: &FolType,
) -> Result<(), ResolverError> {
    match typ {
        typ if typ.is_builtin_str() => {}
        FolType::Named { name, syntax_id } => {
            record_named_type_reference(
                program,
                source_unit_id,
                scope_id,
                name,
                *syntax_id,
                syntax_id
                    .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
                    .cloned(),
            )?;
        }
        FolType::Array { element_type, .. }
        | FolType::Vector { element_type }
        | FolType::Sequence { element_type }
        | FolType::Channel { element_type } => {
            resolve_type_reference(session, program, source_unit_id, scope_id, element_type)?;
        }
        FolType::Matrix { element_type, .. } => {
            resolve_type_reference(session, program, source_unit_id, scope_id, element_type)?;
        }
        FolType::Set { types } | FolType::Multiple { types } | FolType::Union { types } => {
            for part in types {
                resolve_type_reference(session, program, source_unit_id, scope_id, part)?;
            }
        }
        FolType::Map {
            key_type,
            value_type,
        } => {
            resolve_type_reference(session, program, source_unit_id, scope_id, key_type)?;
            resolve_type_reference(session, program, source_unit_id, scope_id, value_type)?;
        }
        FolType::Record { fields } => {
            for field_type in fields.values() {
                resolve_type_reference(session, program, source_unit_id, scope_id, field_type)?;
            }
        }
        FolType::Entry { variants } => {
            for variant in variants.values().flatten() {
                resolve_type_reference(session, program, source_unit_id, scope_id, variant)?;
            }
        }
        FolType::Optional { inner } | FolType::Pointer { target: inner } => {
            resolve_type_reference(session, program, source_unit_id, scope_id, inner)?;
        }
        FolType::Error { inner } => {
            if let Some(inner) = inner {
                resolve_type_reference(session, program, source_unit_id, scope_id, inner)?;
            }
        }
        FolType::Limited { base, limits } => {
            resolve_type_reference(session, program, source_unit_id, scope_id, base)?;
            for limit in limits {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    limit,
                    false,
                    None,
                )?;
            }
        }
        FolType::Function {
            params,
            return_type,
        } => {
            for param in params {
                resolve_type_reference(session, program, source_unit_id, scope_id, param)?;
            }
            resolve_type_reference(session, program, source_unit_id, scope_id, return_type)?;
        }
        FolType::Generic { constraints, .. } => {
            for constraint in constraints {
                resolve_type_reference(session, program, source_unit_id, scope_id, constraint)?;
            }
        }
        FolType::QualifiedNamed { path } => {
            record_qualified_type_reference(program, source_unit_id, scope_id, path)?;
        }
        FolType::Int { .. }
        | FolType::Float { .. }
        | FolType::Char { .. }
        | FolType::Bool
        | FolType::Never
        | FolType::Any
        | FolType::None
        | FolType::Package { .. }
        | FolType::Module { .. }
        | FolType::Block { .. }
        | FolType::Test { .. }
        | FolType::Location { .. } => {}
    }

    Ok(())
}

pub fn resolve_contract_reference(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    typ: &FolType,
) -> Result<(), ResolverError> {
    match typ {
        FolType::Named { name, syntax_id } => {
            record_contract_reference(
                program,
                source_unit_id,
                scope_id,
                name,
                *syntax_id,
                syntax_id
                    .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
                    .cloned(),
            )?;
        }
        FolType::QualifiedNamed { path } => {
            record_qualified_type_reference(program, source_unit_id, scope_id, path)?;
        }
        _ => resolve_type_reference(session, program, source_unit_id, scope_id, typ)?,
    }

    Ok(())
}

pub fn resolve_type_definition(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    type_def: &TypeDefinition,
) -> Result<(), ResolverError> {
    match type_def {
        TypeDefinition::Record {
            fields, members, ..
        } => {
            for field_type in fields.values() {
                resolve_type_reference(session, program, source_unit_id, scope_id, field_type)?;
            }
            for member in members {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    member,
                    false,
                    None,
                )?;
            }
        }
        TypeDefinition::Entry {
            variants, members, ..
        } => {
            for variant_type in variants.values().flatten() {
                resolve_type_reference(session, program, source_unit_id, scope_id, variant_type)?;
            }
            for member in members {
                super::traverse_node(
                    session,
                    program,
                    source_unit_id,
                    scope_id,
                    member,
                    false,
                    None,
                )?;
            }
        }
        TypeDefinition::Alias { target } => {
            resolve_type_reference(session, program, source_unit_id, scope_id, target)?;
        }
    }

    Ok(())
}

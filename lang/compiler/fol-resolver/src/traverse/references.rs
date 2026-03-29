use crate::{
    model::{ReferenceKind, ResolvedProgram, ResolvedReference, SymbolKind},
    ReferenceId, ResolverError, ScopeId, SourceUnitId, SymbolId,
};
use fol_parser::ast::QualifiedPath;

use super::resolve::{
    qualified_path_origin, resolve_qualified_symbol, resolve_visible_or_imported_symbol_of_kinds,
    resolve_visible_symbol,
};

pub fn record_identifier_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<ReferenceId, ResolverError> {
    let symbol_id = resolve_visible_symbol(program, scope_id, name, origin)?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::Identifier,
        syntax_id,
        name: name.to_string(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    Ok(reference_id)
}

pub fn is_builtin_diagnostic_call(name: &str) -> bool {
    matches!(name, "panic" | "report" | "check" | "assert")
}

pub fn record_function_call_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<ReferenceId, ResolverError> {
    let symbol_id = resolve_visible_or_imported_symbol_of_kinds(
        program,
        scope_id,
        name,
        &[SymbolKind::Routine, SymbolKind::ValueBinding, SymbolKind::Parameter],
        Some("callable routine"),
        origin,
    )?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::FunctionCall,
        syntax_id,
        name: name.to_string(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    Ok(reference_id)
}

pub fn record_qualified_identifier_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    path: &QualifiedPath,
) -> Result<ReferenceId, ResolverError> {
    let symbol_id = resolve_qualified_symbol(
        program,
        scope_id,
        path,
        &[],
        "qualified identifier",
        qualified_path_origin(program, path),
    )?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::QualifiedIdentifier,
        syntax_id: path.syntax_id(),
        name: path.joined(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    Ok(reference_id)
}

pub fn record_qualified_function_call_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    path: &QualifiedPath,
) -> Result<ReferenceId, ResolverError> {
    let symbol_id = resolve_qualified_symbol(
        program,
        scope_id,
        path,
        &[SymbolKind::Routine],
        "qualified callable routine",
        qualified_path_origin(program, path),
    )?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::QualifiedFunctionCall,
        syntax_id: path.syntax_id(),
        name: path.joined(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    Ok(reference_id)
}

pub fn record_named_type_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<ReferenceId, ResolverError> {
    let symbol_id = resolve_visible_or_imported_symbol_of_kinds(
        program,
        scope_id,
        name,
        &[
            SymbolKind::Type,
            SymbolKind::Alias,
            SymbolKind::GenericParameter,
            SymbolKind::Standard,
        ],
        Some("type"),
        origin,
    )?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::TypeName,
        syntax_id,
        name: name.to_string(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    Ok(reference_id)
}

pub fn record_contract_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    origin: Option<fol_parser::ast::SyntaxOrigin>,
) -> Result<ReferenceId, ResolverError> {
    let symbol_id = resolve_visible_or_imported_symbol_of_kinds(
        program,
        scope_id,
        name,
        &[SymbolKind::Standard],
        Some("standard"),
        origin,
    )?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::TypeName,
        syntax_id,
        name: name.to_string(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    Ok(reference_id)
}

pub fn record_inquiry_target_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    resolved: SymbolId,
) -> ReferenceId {
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::InquiryTarget,
        syntax_id: None,
        name: name.to_string(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(resolved),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    reference_id
}

pub fn record_qualified_type_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    path: &QualifiedPath,
) -> Result<ReferenceId, ResolverError> {
    let symbol_id = resolve_qualified_symbol(
        program,
        scope_id,
        path,
        &[SymbolKind::Type, SymbolKind::Alias, SymbolKind::Standard],
        "qualified type",
        qualified_path_origin(program, path),
    )?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::QualifiedTypeName,
        syntax_id: path.syntax_id(),
        name: path.joined(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
    Ok(reference_id)
}

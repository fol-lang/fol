use crate::{
    model::{ReferenceKind, ResolvedProgram, ResolvedReference, SymbolKind},
    ReferenceId, ResolverError, ScopeId, SourceUnitId, SymbolId,
};
use fol_parser::ast::QualifiedPath;

use super::resolve::{
    qualified_path_origin, resolve_qualified_symbol, resolve_visible_or_imported_symbol_of_kinds,
    resolve_visible_symbol,
};

fn instantiated_type_base(name: &str) -> Option<&str> {
    let mut square_depth = 0usize;

    for (idx, ch) in name.char_indices() {
        match ch {
            '[' => {
                if square_depth == 0 {
                    return Some(name[..idx].trim_end());
                }
                square_depth += 1;
            }
            ']' => {
                square_depth = square_depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    None
}

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
        anchor_syntax_id: None,
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
        &[
            SymbolKind::Routine,
            SymbolKind::ValueBinding,
            SymbolKind::Parameter,
            SymbolKind::Capture,
        ],
        Some("callable routine"),
        origin,
    )?;
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::FunctionCall,
        syntax_id,
        anchor_syntax_id: None,
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
        anchor_syntax_id: path.final_syntax_id(),
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
        anchor_syntax_id: path.final_syntax_id(),
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
    let resolved_name = instantiated_type_base(name).unwrap_or(name);
    let (kind, symbol_id) = if resolved_name.contains("::") {
        let symbol_id = resolve_qualified_symbol(
            program,
            scope_id,
            &QualifiedPath::with_syntax_id(
                resolved_name
                    .split("::")
                    .map(|segment| segment.to_string())
                    .collect(),
                syntax_id,
            ),
            &[SymbolKind::Type, SymbolKind::Alias, SymbolKind::Standard],
            "qualified type",
            origin,
        )?;
        (ReferenceKind::QualifiedTypeName, symbol_id)
    } else {
        let symbol_id = resolve_visible_or_imported_symbol_of_kinds(
            program,
            scope_id,
            resolved_name,
            &[
                SymbolKind::Type,
                SymbolKind::Alias,
                SymbolKind::GenericParameter,
                SymbolKind::Standard,
            ],
            Some("type"),
            origin,
        )?;
        (ReferenceKind::TypeName, symbol_id)
    };
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind,
        syntax_id,
        anchor_syntax_id: None,
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
        anchor_syntax_id: None,
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
        anchor_syntax_id: None,
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
        anchor_syntax_id: None,
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

/// Link a dfr/edf capture-list entry to the outer binding it captures: the
/// deferred block runs in-frame and binds no new symbol, so the entry is a
/// plain use of the enclosing binding. Recording it keeps editor
/// rename/references covering the capture site. Resolution failures are left
/// for typecheck to report; the reference is purely navigational.
pub(crate) fn record_deferred_capture_reference(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    capture: &fol_parser::ast::RoutineCapture,
) {
    let Some(syntax_id) = capture.syntax_id else {
        return;
    };
    let origin = program.syntax_index().origin(syntax_id).cloned();
    let Ok(symbol_id) = super::resolve::resolve_visible_symbol_of_kinds(
        program,
        scope_id,
        &capture.name,
        &[
            SymbolKind::ValueBinding,
            SymbolKind::Parameter,
            SymbolKind::Capture,
        ],
        Some("captured binding"),
        origin,
    ) else {
        return;
    };
    let reference_id = program.references.push(ResolvedReference {
        id: ReferenceId(0),
        kind: ReferenceKind::Identifier,
        syntax_id: Some(syntax_id),
        anchor_syntax_id: None,
        name: capture.name.clone(),
        scope: scope_id,
        source_unit: source_unit_id,
        resolved: Some(symbol_id),
    });
    if let Some(reference) = program.references.get_mut(reference_id) {
        reference.id = reference_id;
    }
}

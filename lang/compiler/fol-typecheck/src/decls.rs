use crate::{
    CheckedType, CheckedTypeId, DeclaredTypeKind, RoutineType, TypeTable, TypecheckError,
    TypecheckErrorKind, TypecheckResult, TypedProgram,
};
use fol_parser::ast::{
    AstNode, BindingPattern, FolType, Generic, Parameter, ParsedSourceUnitKind, ParsedTopLevel,
    SyntaxNodeId, SyntaxOrigin, TypeDefinition, TypeOption, VarOption,
};
use fol_resolver::{ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::BTreeMap;

pub fn lower_declaration_signatures(typed: &mut TypedProgram) -> TypecheckResult<()> {
    let resolved = typed.resolved().clone();
    let syntax = resolved.syntax().clone();
    let mut errors = Vec::new();

    for (source_unit_index, source_unit) in syntax.source_units.iter().enumerate() {
        if source_unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit_id = SourceUnitId(source_unit_index);
        for item in &source_unit.items {
            if let Err(error) = lower_top_level_declaration(typed, &resolved, source_unit_id, item)
            {
                errors.push(error);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn lower_top_level_declaration(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    item: &ParsedTopLevel,
) -> Result<(), TypecheckError> {
    if let Some(error) = unsupported_v1_top_level_decl(resolved, item) {
        return Err(error);
    }

    match &item.node {
        AstNode::VarDecl {
            name, type_hint, ..
        }
        | AstNode::LabDecl {
            name, type_hint, ..
        } => {
            if let Some(type_hint) = type_hint {
                let symbol_id = find_symbol_id(
                    resolved,
                    source_unit_id,
                    &[symbol_kind_for_node(&item.node)],
                    name,
                )?;
                let symbol_scope = resolved
                    .symbol(symbol_id)
                    .map(|symbol| symbol.scope)
                    .ok_or_else(|| internal_error("resolved binding symbol disappeared", None))?;
                let type_id = lower_type(typed, resolved, symbol_scope, type_hint)?;
                record_symbol_type(typed, symbol_id, type_id)?;
            }
        }
        AstNode::DestructureDecl {
            pattern, type_hint, ..
        } => {
            if let Some(type_hint) = type_hint {
                let binding_names = binding_names(pattern);
                let symbol_scope = binding_names
                    .first()
                    .and_then(|name| {
                        find_symbol_id(
                            resolved,
                            source_unit_id,
                            &[SymbolKind::DestructureBinding],
                            name,
                        )
                        .ok()
                    })
                    .and_then(|symbol_id| resolved.symbol(symbol_id).map(|symbol| symbol.scope))
                    .ok_or_else(|| {
                        internal_error("resolved destructure binding symbol disappeared", None)
                    })?;
                let type_id = lower_type(typed, resolved, symbol_scope, type_hint)?;
                for name in binding_names {
                    let symbol_id = find_symbol_id(
                        resolved,
                        source_unit_id,
                        &[SymbolKind::DestructureBinding],
                        &name,
                    )?;
                    record_symbol_type(typed, symbol_id, type_id)?;
                }
            }
        }
        AstNode::FunDecl {
            syntax_id,
            name,
            generics,
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::ProDecl {
            syntax_id,
            name,
            generics,
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::LogDecl {
            syntax_id,
            name,
            generics,
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            let signature_scope = lower_named_routine_signature(
                typed,
                resolved,
                source_unit_id,
                name,
                *syntax_id,
                generics,
                receiver_type.as_ref(),
                params,
                return_type.as_ref(),
                error_type.as_ref(),
            )?;
            lower_nested_declarations_in_nodes(
                typed,
                resolved,
                source_unit_id,
                signature_scope,
                body,
            )?;
            lower_nested_declarations_in_nodes(
                typed,
                resolved,
                source_unit_id,
                signature_scope,
                inquiries,
            )?;
        }
        AstNode::TypeDecl { name, type_def, .. } => {
            let symbol_id = find_symbol_id(resolved, source_unit_id, &[SymbolKind::Type], name)?;
            let symbol_scope = resolved
                .symbol(symbol_id)
                .map(|symbol| symbol.scope)
                .ok_or_else(|| internal_error("resolved type symbol disappeared", None))?;
            let type_id = match type_def {
                TypeDefinition::Alias { target } => {
                    lower_type(typed, resolved, symbol_scope, target)?
                }
                TypeDefinition::Record { fields, .. } => {
                    let mut lowered = BTreeMap::new();
                    for (field_name, field_type) in fields {
                        lowered.insert(
                            field_name.clone(),
                            lower_type(typed, resolved, symbol_scope, field_type)?,
                        );
                    }
                    typed
                        .type_table_mut()
                        .intern(CheckedType::Record { fields: lowered })
                }
                TypeDefinition::Entry { variants, .. } => {
                    let mut lowered = BTreeMap::new();
                    for (variant_name, variant_type) in variants {
                        lowered.insert(
                            variant_name.clone(),
                            variant_type
                                .as_ref()
                                .map(|variant| lower_type(typed, resolved, symbol_scope, variant))
                                .transpose()?,
                        );
                    }
                    typed
                        .type_table_mut()
                        .intern(CheckedType::Entry { variants: lowered })
                }
            };
            record_symbol_type(typed, symbol_id, type_id)?;
        }
        AstNode::AliasDecl { name, target } => {
            let symbol_id = find_symbol_id(resolved, source_unit_id, &[SymbolKind::Alias], name)?;
            let symbol_scope = resolved
                .symbol(symbol_id)
                .map(|symbol| symbol.scope)
                .ok_or_else(|| internal_error("resolved alias symbol disappeared", None))?;
            let target_type = lower_type(typed, resolved, symbol_scope, target)?;
            record_symbol_type(typed, symbol_id, target_type)?;
        }
        _ => {}
    }

    Ok(())
}

fn lower_nested_declarations_in_nodes(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    current_scope: ScopeId,
    nodes: &[AstNode],
) -> Result<(), TypecheckError> {
    for node in nodes {
        lower_nested_declarations_in_node(typed, resolved, source_unit_id, current_scope, node)?;
    }
    Ok(())
}

fn lower_nested_declarations_in_node(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    current_scope: ScopeId,
    node: &AstNode,
) -> Result<(), TypecheckError> {
    if let Some(error) = unsupported_v1_nested_decl(resolved, node) {
        return Err(error);
    }

    match node {
        AstNode::VarDecl {
            name, type_hint, ..
        }
        | AstNode::LabDecl {
            name, type_hint, ..
        } => {
            if let Some(type_hint) = type_hint {
                let symbol_id = find_symbol_id_in_scope(
                    resolved,
                    source_unit_id,
                    current_scope,
                    &[symbol_kind_for_node(node)],
                    name,
                )?;
                let type_id = lower_type(typed, resolved, current_scope, type_hint)?;
                record_symbol_type(typed, symbol_id, type_id)?;
            }
        }
        AstNode::DestructureDecl {
            pattern, type_hint, ..
        } => {
            if let Some(type_hint) = type_hint {
                let type_id = lower_type(typed, resolved, current_scope, type_hint)?;
                for name in binding_names(pattern) {
                    let symbol_id = find_symbol_id_in_scope(
                        resolved,
                        source_unit_id,
                        current_scope,
                        &[SymbolKind::DestructureBinding],
                        &name,
                    )?;
                    record_symbol_type(typed, symbol_id, type_id)?;
                }
            }
        }
        AstNode::FunDecl {
            syntax_id,
            name,
            generics,
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::ProDecl {
            syntax_id,
            name,
            generics,
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::LogDecl {
            syntax_id,
            name,
            generics,
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            let routine_scope = lower_named_routine_signature(
                typed,
                resolved,
                source_unit_id,
                name,
                *syntax_id,
                generics,
                receiver_type.as_ref(),
                params,
                return_type.as_ref(),
                error_type.as_ref(),
            )?;
            lower_nested_declarations_in_nodes(
                typed,
                resolved,
                source_unit_id,
                routine_scope,
                body,
            )?;
            lower_nested_declarations_in_nodes(
                typed,
                resolved,
                source_unit_id,
                routine_scope,
                inquiries,
            )?;
        }
        AstNode::AnonymousFun {
            params,
            return_type,
            error_type,
            ..
        }
        | AstNode::AnonymousPro {
            params,
            return_type,
            error_type,
            ..
        }
        | AstNode::AnonymousLog {
            params,
            return_type,
            error_type,
            ..
        } => {
            for param in params {
                let _ = lower_type(typed, resolved, current_scope, &param.param_type)?;
            }
            if let Some(return_type) = return_type {
                let _ = lower_type(typed, resolved, current_scope, return_type)?;
            }
            if let Some(error_type) = error_type {
                let _ = lower_type(typed, resolved, current_scope, error_type)?;
            }
        }
        AstNode::Defer {
            syntax_id, body, ..
        } => {
            let deferred_scope =
                nested_scope_for_syntax(resolved, current_scope, *syntax_id, "defer block")?;
            lower_nested_declarations_in_nodes(
                typed,
                resolved,
                source_unit_id,
                deferred_scope,
                body,
            )?;
        }
        AstNode::Block {
            syntax_id,
            statements,
            ..
        } => {
            let block_scope =
                nested_scope_for_syntax(resolved, current_scope, *syntax_id, "block")?;
            lower_nested_declarations_in_nodes(
                typed,
                resolved,
                source_unit_id,
                block_scope,
                statements,
            )?;
        }
        AstNode::Inquiry { body, .. } => {
            lower_nested_declarations_in_nodes(
                typed,
                resolved,
                source_unit_id,
                current_scope,
                body,
            )?;
        }
        AstNode::Commented { node, .. } => {
            lower_nested_declarations_in_node(
                typed,
                resolved,
                source_unit_id,
                current_scope,
                node,
            )?;
        }
        _ => {
            for child in node.children() {
                lower_nested_declarations_in_node(
                    typed,
                    resolved,
                    source_unit_id,
                    current_scope,
                    child,
                )?;
            }
        }
    }

    Ok(())
}

fn lower_named_routine_signature(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    name: &str,
    syntax_id: Option<SyntaxNodeId>,
    generics: &[Generic],
    receiver_type: Option<&FolType>,
    params: &[fol_parser::ast::Parameter],
    return_type: Option<&FolType>,
    error_type: Option<&FolType>,
) -> Result<ScopeId, TypecheckError> {
    let symbol_id = find_routine_symbol_id(resolved, source_unit_id, name, receiver_type, params)?;
    let signature_scope = syntax_id
        .and_then(|id| resolved.scope_for_syntax(id))
        .or_else(|| resolved.symbol(symbol_id).map(|symbol| symbol.scope))
        .ok_or_else(|| internal_error("resolved routine scope disappeared", None))?;
    let generic_params =
        lower_routine_generic_params(typed, resolved, source_unit_id, signature_scope, generics)?;
    let mut lowered_params = Vec::new();
    for param in params {
        let param_type = lower_type(typed, resolved, signature_scope, &param.param_type)?;
        let param_symbol_id = find_symbol_id_in_scope(
            resolved,
            source_unit_id,
            signature_scope,
            &[SymbolKind::Parameter],
            &param.name,
        )?;
        record_symbol_type(typed, param_symbol_id, param_type)?;
        lowered_params.push(param_type);
    }
    let lowered_return = match return_type {
        None | Some(FolType::None) => None,
        Some(ty) => Some(lower_type(typed, resolved, signature_scope, ty)?),
    };
    let lowered_error = error_type
        .as_ref()
        .map(|ty| lower_type(typed, resolved, signature_scope, ty))
        .transpose()?;
    let lowered_receiver = receiver_type
        .as_ref()
        .map(|ty| lower_type(typed, resolved, signature_scope, ty))
        .transpose()?;
    reject_generic_types_in_position(
        typed,
        lowered_receiver,
        receiver_type.and_then(|ty| type_origin(resolved, ty)),
        "generic receiver types are not yet supported in V2 Milestone 1",
    )?;
    reject_generic_types_in_position(
        typed,
        lowered_error,
        error_type.and_then(|ty| type_origin(resolved, ty)),
        "generic error types are not yet supported in V2 Milestone 1",
    )?;
    let routine_type = typed
        .type_table_mut()
        .intern(CheckedType::Routine(RoutineType {
            generic_params,
            param_names: params.iter().map(|param| param.name.clone()).collect(),
            param_defaults: params.iter().map(|param| param.default.clone()).collect(),
            variadic_index: params.iter().enumerate().find_map(|(index, param)| {
                (index + 1 == params.len()
                    && matches!(param.param_type, FolType::Sequence { .. }))
                .then_some(index)
            }),
            params: lowered_params,
            return_type: lowered_return,
            error_type: lowered_error,
        }));
    record_symbol_type(typed, symbol_id, routine_type)?;
    record_symbol_receiver_type(typed, symbol_id, lowered_receiver)?;
    Ok(signature_scope)
}

fn lower_routine_generic_params(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    signature_scope: ScopeId,
    generics: &[Generic],
) -> Result<Vec<SymbolId>, TypecheckError> {
    let mut generic_params = Vec::new();
    for generic in generics {
        if !generic.constraints.is_empty() {
            return Err(match generic
                .constraints
                .first()
                .and_then(|constraint| type_origin(resolved, constraint))
            {
                Some(origin) => TypecheckError::with_origin(
                    TypecheckErrorKind::Unsupported,
                    "generic routine constraints are not yet supported in V2 Milestone 1",
                    origin,
                ),
                None => TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "generic routine constraints are not yet supported in V2 Milestone 1",
                ),
            });
        }

        let symbol_id = find_symbol_id_in_scope(
            resolved,
            source_unit_id,
            signature_scope,
            &[SymbolKind::GenericParameter],
            &generic.name,
        )?;
        let generic_type = typed.type_table_mut().intern(CheckedType::Declared {
            symbol: symbol_id,
            name: generic.name.clone(),
            kind: DeclaredTypeKind::GenericParameter,
        });
        record_symbol_type(typed, symbol_id, generic_type)?;
        generic_params.push(symbol_id);
    }

    Ok(generic_params)
}

fn reject_generic_types_in_position(
    typed: &TypedProgram,
    type_id: Option<CheckedTypeId>,
    origin: Option<SyntaxOrigin>,
    message: &'static str,
) -> Result<(), TypecheckError> {
    let Some(type_id) = type_id else {
        return Ok(());
    };
    if checked_type_contains_generic_param(typed, type_id) {
        return Err(match origin {
            Some(origin) => TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
            None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        });
    }
    Ok(())
}

pub(crate) fn checked_type_contains_generic_param(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> bool {
    match typed.type_table().get(type_id) {
        Some(CheckedType::Declared {
            kind: DeclaredTypeKind::GenericParameter,
            ..
        }) => true,
        Some(CheckedType::Array { element_type, .. })
        | Some(CheckedType::Vector {
            element_type,
        })
        | Some(CheckedType::Sequence {
            element_type,
        })
        | Some(CheckedType::Optional { inner: element_type }) => {
            checked_type_contains_generic_param(typed, *element_type)
        }
        Some(CheckedType::Error { inner }) => inner
            .is_some_and(|inner| checked_type_contains_generic_param(typed, inner)),
        Some(CheckedType::Set { member_types }) => member_types
            .iter()
            .any(|member| checked_type_contains_generic_param(typed, *member)),
        Some(CheckedType::Map {
            key_type,
            value_type,
        }) => {
            checked_type_contains_generic_param(typed, *key_type)
                || checked_type_contains_generic_param(typed, *value_type)
        }
        Some(CheckedType::Record { fields }) => fields
            .values()
            .any(|field| checked_type_contains_generic_param(typed, *field)),
        Some(CheckedType::Entry { variants }) => variants
            .values()
            .flatten()
            .any(|variant| checked_type_contains_generic_param(typed, *variant)),
        Some(CheckedType::Routine(signature)) => {
            signature
                .params
                .iter()
                .any(|param| checked_type_contains_generic_param(typed, *param))
                || signature
                    .return_type
                    .is_some_and(|ret| checked_type_contains_generic_param(typed, ret))
                || signature
                    .error_type
                    .is_some_and(|err| checked_type_contains_generic_param(typed, err))
        }
        _ => false,
    }
}

fn nested_scope_for_syntax(
    resolved: &ResolvedProgram,
    parent_scope: ScopeId,
    syntax_id: Option<SyntaxNodeId>,
    construct_name: &str,
) -> Result<ScopeId, TypecheckError> {
    let Some(syntax_id) = syntax_id else {
        return Err(internal_error(
            format!("{construct_name} is missing syntax identity during type lowering"),
            None,
        ));
    };
    let Some(scope_id) = resolved.scope_for_syntax(syntax_id) else {
        return Err(internal_error(
            format!("{construct_name} is missing a resolved child scope during type lowering"),
            None,
        ));
    };
    let Some(scope) = resolved.scope(scope_id) else {
        return Err(internal_error(
            format!("{construct_name} resolved to unknown scope {}", scope_id.0),
            None,
        ));
    };
    if scope.parent != Some(parent_scope) {
        return Err(internal_error(
            format!(
                "{construct_name} resolved scope {} does not belong to parent scope {}",
                scope_id.0, parent_scope.0
            ),
            None,
        ));
    }
    Ok(scope_id)
}

pub(crate) fn lower_type(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    scope_id: ScopeId,
    typ: &FolType,
) -> Result<CheckedTypeId, TypecheckError> {
    match typ {
        FolType::Int { .. } => Ok(typed.builtin_types().int),
        FolType::Float { .. } => Ok(typed.builtin_types().float),
        FolType::Bool => Ok(typed.builtin_types().bool_),
        FolType::Char { .. } => Ok(typed.builtin_types().char_),
        typ if typ.is_builtin_str() => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "str")?;
            Ok(typed.builtin_types().str_)
        }
        FolType::Never => Ok(typed.builtin_types().never),
        FolType::Named { name, syntax_id } => {
            let symbol_id = resolved_symbol_for_syntax(
                resolved,
                *syntax_id,
                name,
                SymbolReferenceShape::Named,
            )?;
            lower_declared_symbol(typed.type_table_mut(), resolved, symbol_id)
        }
        FolType::QualifiedNamed { path } => {
            let symbol_id = resolved_symbol_for_syntax(
                resolved,
                path.syntax_id(),
                &path.joined(),
                SymbolReferenceShape::Qualified,
            )?;
            lower_declared_symbol(typed.type_table_mut(), resolved, symbol_id)
        }
        FolType::Array { element_type, size } => {
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed.type_table_mut().intern(CheckedType::Array {
                element_type,
                size: *size,
            }))
        }
        FolType::Vector { element_type } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "vec[...]")?;
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Vector { element_type }))
        }
        FolType::Sequence { element_type } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "seq[...]")?;
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Sequence { element_type }))
        }
        FolType::Set { types } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "set[...]")?;
            let mut member_types = Vec::new();
            for member in types {
                member_types.push(lower_type(typed, resolved, scope_id, member)?);
            }
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Set { member_types }))
        }
        FolType::Map {
            key_type,
            value_type,
        } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "map[...]")?;
            let key_type = lower_type(typed, resolved, scope_id, key_type)?;
            let value_type = lower_type(typed, resolved, scope_id, value_type)?;
            Ok(typed.type_table_mut().intern(CheckedType::Map {
                key_type,
                value_type,
            }))
        }
        FolType::Optional { inner } => {
            let inner = lower_type(typed, resolved, scope_id, inner)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Optional { inner }))
        }
        FolType::Error { inner } => {
            let inner = inner
                .as_ref()
                .map(|inner| lower_type(typed, resolved, scope_id, inner))
                .transpose()?;
            Ok(typed.type_table_mut().intern(CheckedType::Error { inner }))
        }
        FolType::Record { fields } => {
            let mut lowered = BTreeMap::new();
            for (field_name, field_type) in fields {
                lowered.insert(
                    field_name.clone(),
                    lower_type(typed, resolved, scope_id, field_type)?,
                );
            }
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Record { fields: lowered }))
        }
        FolType::Entry { variants } => {
            let mut lowered = BTreeMap::new();
            for (variant_name, variant_type) in variants {
                lowered.insert(
                    variant_name.clone(),
                    variant_type
                        .as_ref()
                        .map(|variant| lower_type(typed, resolved, scope_id, variant))
                        .transpose()?,
                );
            }
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Entry { variants: lowered }))
        }
        FolType::Function {
            params,
            return_type,
        } => {
            let lowered_params = params
                .iter()
                .map(|p| lower_type(typed, resolved, scope_id, p))
                .collect::<Result<Vec<_>, _>>()?;
            let lowered_return = lower_type(typed, resolved, scope_id, return_type)?;
            Ok(typed.type_table_mut().intern(CheckedType::Routine(
                crate::types::RoutineType {
                    generic_params: Vec::new(),
                    param_names: vec![String::new(); lowered_params.len()],
                    param_defaults: vec![None; lowered_params.len()],
                    variadic_index: None,
                    params: lowered_params,
                    return_type: Some(lowered_return),
                    error_type: None,
                },
            )))
        }
        unsupported => Err(unsupported_type_error(resolved, unsupported)),
    }
}

fn reject_heap_backed_type_in_core(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    typ: &FolType,
    label: &str,
) -> Result<(), TypecheckError> {
    if typed.capability_model() != crate::TypecheckCapabilityModel::Core {
        return Ok(());
    }

    let message = format!(
        "{label} requires heap support and is unavailable in 'fol_model = core'"
    );
    Err(match type_origin(resolved, typ) {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
    })
}

fn lower_declared_symbol(
    table: &mut TypeTable,
    resolved: &ResolvedProgram,
    symbol_id: SymbolId,
) -> Result<CheckedTypeId, TypecheckError> {
    let symbol = resolved
        .symbol(symbol_id)
        .ok_or_else(|| internal_error("resolved type symbol disappeared", None))?;
    let kind = match symbol.kind {
        SymbolKind::Type => DeclaredTypeKind::Type,
        SymbolKind::Alias => DeclaredTypeKind::Alias,
        SymbolKind::GenericParameter => DeclaredTypeKind::GenericParameter,
        _ => {
            return Err(internal_error(
                "type reference resolved to a non-type symbol",
                symbol.origin.clone(),
            ));
        }
    };

    Ok(table.intern(CheckedType::Declared {
        symbol: symbol_id,
        name: symbol.name.clone(),
        kind,
    }))
}

fn resolved_symbol_for_syntax(
    resolved: &ResolvedProgram,
    syntax_id: Option<SyntaxNodeId>,
    display_name: &str,
    shape: SymbolReferenceShape,
) -> Result<SymbolId, TypecheckError> {
    let syntax_id = syntax_id.ok_or_else(|| {
        invalid_input_error(
            format!("type reference '{display_name}' does not retain a syntax id"),
            None,
        )
    })?;

    resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(syntax_id)
                && match shape {
                    SymbolReferenceShape::Named => {
                        reference.kind == fol_resolver::ReferenceKind::TypeName
                    }
                    SymbolReferenceShape::Qualified => {
                        reference.kind == fol_resolver::ReferenceKind::QualifiedTypeName
                    }
                }
        })
        .and_then(|reference| reference.resolved)
        .ok_or_else(|| {
            invalid_input_error(
                format!("type reference '{display_name}' does not have a resolved symbol"),
                resolved.syntax_index().origin(syntax_id).cloned(),
            )
        })
}

fn find_symbol_id(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    allowed_kinds: &[SymbolKind],
    name: &str,
) -> Result<SymbolId, TypecheckError> {
    resolved
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.name == name
                && allowed_kinds.contains(&symbol.kind)
        })
        .map(|(symbol_id, _)| symbol_id)
        .ok_or_else(|| {
            internal_error(
                format!("resolved declaration symbol '{name}' is missing from typed lowering"),
                None,
            )
        })
}

fn find_routine_symbol_id(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    name: &str,
    receiver_type: Option<&FolType>,
    params: &[Parameter],
) -> Result<SymbolId, TypecheckError> {
    let canonical_name = canonical_identifier_key(name);
    let receiver = receiver_type
        .map(routine_type_key)
        .unwrap_or_else(|| "_".to_string());
    let params = params
        .iter()
        .map(|param| routine_type_key(&param.param_type))
        .collect::<Vec<_>>()
        .join(",");
    let duplicate_key = format!("routine#{canonical_name}#{receiver}#{params}");

    resolved
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.kind == SymbolKind::Routine
                && symbol.duplicate_key == duplicate_key
        })
        .map(|(symbol_id, _)| symbol_id)
        .ok_or_else(|| {
            internal_error(
                format!(
                    "resolved routine symbol '{name}' with duplicate key '{duplicate_key}' is missing from typed lowering"
                ),
                None,
            )
        })
}

pub(crate) fn find_symbol_id_in_scope(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    allowed_kinds: &[SymbolKind],
    name: &str,
) -> Result<SymbolId, TypecheckError> {
    resolved
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.scope == scope_id
                && symbol.name == name
                && allowed_kinds.contains(&symbol.kind)
        })
        .map(|(symbol_id, _)| symbol_id)
        .ok_or_else(|| {
            internal_error(
                format!(
                    "resolved declaration symbol '{name}' is missing from typed lowering for scope {}",
                    scope_id.0
                ),
                None,
            )
        })
}

fn routine_type_key(typ: &FolType) -> String {
    match typ {
        FolType::Named { name, .. } => canonical_identifier_key(name),
        FolType::QualifiedNamed { path } => path
            .segments
            .iter()
            .map(|segment| canonical_identifier_key(segment))
            .collect::<Vec<_>>()
            .join("::"),
        other => other
            .named_text()
            .map(|text| canonical_identifier_key(&text))
            .unwrap_or_else(|| format!("{other:?}")),
    }
}

fn canonical_identifier_key(name: &str) -> String {
    name.chars()
        .filter(|ch| *ch != '_')
        .map(|ch| {
            if ch.is_ascii() {
                ch.to_ascii_lowercase()
            } else {
                ch
            }
        })
        .collect()
}

pub(crate) fn record_symbol_type(
    typed: &mut TypedProgram,
    symbol_id: SymbolId,
    type_id: CheckedTypeId,
) -> Result<(), TypecheckError> {
    let symbol = typed.typed_symbol_mut(symbol_id).ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::SymbolTableCorrupted,
            format!(
                "symbol table corrupted: symbol {} is missing while recording declared type {}",
                symbol_id.0, type_id.0,
            ),
        )
    })?;
    symbol.declared_type = Some(type_id);
    Ok(())
}

fn record_symbol_receiver_type(
    typed: &mut TypedProgram,
    symbol_id: SymbolId,
    type_id: Option<CheckedTypeId>,
) -> Result<(), TypecheckError> {
    let symbol = typed.typed_symbol_mut(symbol_id).ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::SymbolTableCorrupted,
            format!(
                "symbol table corrupted: symbol {} is missing while recording receiver type",
                symbol_id.0,
            ),
        )
    })?;
    symbol.receiver_type = type_id;
    Ok(())
}

fn binding_names(pattern: &BindingPattern) -> Vec<String> {
    match pattern {
        BindingPattern::Name(name) | BindingPattern::Rest(name) => vec![name.clone()],
        BindingPattern::Sequence(parts) => parts.iter().flat_map(binding_names).collect(),
    }
}

fn symbol_kind_for_node(node: &AstNode) -> SymbolKind {
    match node {
        AstNode::VarDecl { .. } => SymbolKind::ValueBinding,
        AstNode::LabDecl { .. } => SymbolKind::LabelBinding,
        _ => SymbolKind::ValueBinding,
    }
}

fn unsupported_type_error(resolved: &ResolvedProgram, typ: &FolType) -> TypecheckError {
    let label = match typ {
        FolType::Matrix { .. } => "matrix types are not yet supported",
        FolType::Pointer { .. } => "pointer types are planned for a future release",
        FolType::Channel { .. } => "channel types are planned for a future release",
        FolType::Multiple { .. } => "multiple-return types are not yet supported",
        FolType::Union { .. } => "union types are not yet supported",
        FolType::Limited { .. } => "limited/constrained types are not yet supported",
        FolType::Any => "'any' type is not yet supported",
        FolType::None => "'none' type is not yet supported",
        FolType::Generic { .. } => "generic type parameters are not yet supported",
        FolType::Package { .. }
        | FolType::Module { .. }
        | FolType::Block { .. }
        | FolType::Test { .. }
        | FolType::Location { .. } => {
            "package/build-specific type surfaces are not yet supported"
        }
        _ => "this type surface is not yet supported",
    };
    match type_origin(resolved, typ) {
        Some(origin) => TypecheckError::with_origin(TypecheckErrorKind::Unsupported, label, origin),
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, label),
    }
}

fn unsupported_v1_top_level_decl(
    resolved: &ResolvedProgram,
    item: &ParsedTopLevel,
) -> Option<TypecheckError> {
    let origin = resolved.syntax_index().origin(item.node_id).cloned();
    unsupported_v1_decl_with_origin(&item.node, origin)
}

fn unsupported_v1_nested_decl(
    resolved: &ResolvedProgram,
    node: &AstNode,
) -> Option<TypecheckError> {
    unsupported_v1_decl_with_origin(node, node_origin(resolved, node))
}

fn unsupported_v1_decl_with_origin(
    node: &AstNode,
    origin: Option<SyntaxOrigin>,
) -> Option<TypecheckError> {
    let message = match node {
        AstNode::VarDecl { options, .. } | AstNode::LabDecl { options, .. } => {
            unsupported_binding_surface_message(options)
        }
        AstNode::FunDecl { generics, .. }
        | AstNode::ProDecl { generics, .. }
        | AstNode::LogDecl { generics, .. }
            if !generics.is_empty() && generics.iter().any(|generic| !generic.constraints.is_empty()) =>
        {
            Some("generic routine constraints are not yet supported in V2 Milestone 1")
        }
        AstNode::FunDecl { params, .. }
        | AstNode::ProDecl { params, .. }
        | AstNode::LogDecl { params, .. } => unsupported_routine_param_surface_message(params),
        AstNode::TypeDecl { contracts, .. } if !contracts.is_empty() => {
            Some("type contract conformance is planned for a future release")
        }
        AstNode::TypeDecl { options, .. }
            if options
                .iter()
                .any(|option| matches!(option, TypeOption::Extension)) =>
        {
            Some("type extension declarations are planned for a future release")
        }
        AstNode::TypeDecl { generics, .. } if !generics.is_empty() => {
            Some("generic types are not yet supported")
        }
        AstNode::DefDecl { .. } => {
            Some("definition/meta declarations are planned for a future release")
        }
        AstNode::SegDecl { .. } => {
            Some("segment declarations are planned for a future release")
        }
        AstNode::ImpDecl { .. } => {
            Some("implementation declarations are planned for a future release")
        }
        AstNode::StdDecl { kind, .. } => Some(match kind {
            fol_parser::ast::StandardKind::Protocol => {
                "protocol standards are planned for a future release"
            }
            fol_parser::ast::StandardKind::Blueprint => {
                "blueprint standards are planned for a future release"
            }
            fol_parser::ast::StandardKind::Extended => {
                "extended standards are planned for a future release"
            }
        }),
        _ => None,
    }?;

    Some(match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
    })
}

pub(crate) fn unsupported_routine_param_surface_message(
    params: &[Parameter],
) -> Option<&'static str> {
    if params.iter().any(|param| param.is_mutex) {
        Some("mutex parameter semantics are planned for a future release")
    } else if params.iter().any(|param| param.is_borrowable) {
        Some("borrowable parameter semantics are planned for a future release")
    } else {
        None
    }
}

fn unsupported_binding_surface_message(options: &[VarOption]) -> Option<&'static str> {
    if options
        .iter()
        .any(|option| matches!(option, VarOption::Borrowing))
    {
        Some("borrowing binding semantics are planned for a future release")
    } else if options
        .iter()
        .any(|option| matches!(option, VarOption::New))
    {
        Some("heap/new binding semantics are planned for a future release")
    } else if options
        .iter()
        .any(|option| matches!(option, VarOption::Static))
    {
        Some("static binding semantics are not yet supported")
    } else if options
        .iter()
        .any(|option| matches!(option, VarOption::Reactive))
    {
        Some("reactive binding semantics are not yet supported")
    } else {
        None
    }
}

fn type_origin(resolved: &ResolvedProgram, typ: &FolType) -> Option<SyntaxOrigin> {
    match typ {
        FolType::Named { syntax_id, .. } => syntax_id
            .and_then(|syntax_id| resolved.syntax_index().origin(syntax_id))
            .cloned(),
        FolType::QualifiedNamed { path } => path
            .syntax_id()
            .and_then(|syntax_id| resolved.syntax_index().origin(syntax_id))
            .cloned(),
        _ => None,
    }
}

fn node_origin(resolved: &ResolvedProgram, node: &AstNode) -> Option<SyntaxOrigin> {
    if let Some(syntax_id) = node.syntax_id() {
        return resolved.syntax_index().origin(syntax_id).cloned();
    }

    for child in node.children() {
        if let Some(origin) = node_origin(resolved, child) {
            return Some(origin);
        }
    }

    None
}

fn invalid_input_error(message: impl Into<String>, origin: Option<SyntaxOrigin>) -> TypecheckError {
    match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
    }
}

fn internal_error(message: impl Into<String>, origin: Option<SyntaxOrigin>) -> TypecheckError {
    match origin {
        Some(origin) => TypecheckError::with_origin(TypecheckErrorKind::Internal, message, origin),
        None => TypecheckError::new(TypecheckErrorKind::Internal, message),
    }
}

enum SymbolReferenceShape {
    Named,
    Qualified,
}

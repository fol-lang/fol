use crate::{
    CheckedType, CheckedTypeId, DeclaredTypeKind, RoutineType, TypeTable, TypecheckError,
    TypecheckErrorKind, TypecheckResult, TypedConformance, TypedProgram, TypedStandard,
    TypedStandardRoutine,
};
use fol_parser::ast::{
    AstNode, BindingPattern, FolType, Generic, Parameter, ParsedSourceUnitKind, ParsedTopLevel,
    StandardKind, SyntaxNodeId, SyntaxOrigin, TypeDefinition, TypeOption, VarOption,
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

    if let Err(mut conformance_errors) = check_standard_conformance(typed, &resolved, &syntax) {
        errors.append(&mut conformance_errors);
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
        AstNode::TypeDecl {
            generics,
            name,
            explicit_contracts,
            type_def,
            ..
        } => {
            let symbol_id = find_symbol_id(resolved, source_unit_id, &[SymbolKind::Type], name)?;
            let type_scope =
                find_top_level_type_decl_scope(resolved, source_unit_id, item, symbol_id)?;
            let generic_params =
                generic_params_in_scope(resolved, type_scope, item, generics)?;
            let type_id = match type_def {
                TypeDefinition::Alias { target } => {
                    lower_type(typed, resolved, type_scope, target)?
                }
                TypeDefinition::Record { fields, .. } => {
                    let mut lowered = BTreeMap::new();
                    for (field_name, field_type) in fields {
                        lowered.insert(
                            field_name.clone(),
                            lower_type(typed, resolved, type_scope, field_type)?,
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
                                .map(|variant| lower_type(typed, resolved, type_scope, variant))
                                .transpose()?,
                        );
                    }
                    typed
                        .type_table_mut()
                        .intern(CheckedType::Entry { variants: lowered })
                }
            };
            record_symbol_generic_params(typed, symbol_id, generic_params)?;
            record_symbol_type(typed, symbol_id, type_id)?;
            if !explicit_contracts.is_empty() {
                let standard_symbol_ids = explicit_contracts
                    .iter()
                    .map(|contract| lower_standard_symbol_for_contract(resolved, contract))
                    .collect::<Result<Vec<_>, _>>()?;
                typed.record_typed_conformance(TypedConformance {
                    type_symbol_id: symbol_id,
                    standard_symbol_ids,
                });
            }
        }
        AstNode::StdDecl {
            syntax_id,
            name,
            kind,
            body,
            ..
        } => {
            if *kind != StandardKind::Protocol {
                let message = match kind {
                    StandardKind::Protocol => unreachable!(),
                    StandardKind::Blueprint => {
                        "blueprint standards are planned for a future release"
                    }
                    StandardKind::Extended => {
                        "extended standards are planned for a future release"
                    }
                };
                return Err(match node_origin(resolved, &item.node) {
                    Some(origin) => {
                        TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
                    }
                    None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
                });
            }

            let standard_symbol_id =
                find_symbol_id(resolved, source_unit_id, &[SymbolKind::Standard], name)?;
            let standard_scope = syntax_id
                .and_then(|id| resolved.scope_for_syntax(id))
                .ok_or_else(|| {
                    internal_error(
                        format!(
                            "resolved standard scope disappeared for protocol standard '{}'",
                            name
                        ),
                        node_origin(resolved, &item.node),
                    )
                })?;
            let mut required_routines = Vec::new();
            for member in body {
                let required = lower_protocol_standard_member(
                    typed,
                    resolved,
                    source_unit_id,
                    standard_scope,
                    member,
                )?;
                required_routines.push(required);
            }
            typed.record_typed_standard(TypedStandard {
                symbol_id: standard_symbol_id,
                scope_id: standard_scope,
                kind: *kind,
                required_routines,
            });
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
    let source_unit = resolved
        .source_unit(source_unit_id)
        .ok_or_else(|| internal_error("resolved source unit disappeared", None))?;
    if let AstNode::StdDecl { syntax_id, .. } = node {
        let is_top_level_standard = syntax_id
            .is_some_and(|id| source_unit.top_level_nodes.contains(&id));
        if !is_top_level_standard {
            return Err(match node_origin(resolved, node) {
                Some(origin) => TypecheckError::with_origin(
                    TypecheckErrorKind::Unsupported,
                    "standard declarations are not supported in executable bodies",
                    origin,
                ),
                None => TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "standard declarations are not supported in executable bodies",
                ),
            });
        }
    } else if current_scope != source_unit.scope_id {
        if let Some(error) = unsupported_v1_nested_decl(resolved, node) {
            return Err(error);
        }
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

fn lower_protocol_standard_member(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    standard_scope: ScopeId,
    member: &AstNode,
) -> Result<TypedStandardRoutine, TypecheckError> {
    let origin = node_origin(resolved, member);
    let unsupported = |message: &'static str| match origin.clone() {
        Some(origin) => TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin),
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
    };

    match member {
        AstNode::FunDecl {
            name,
            syntax_id,
            generics,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::ProDecl {
            name,
            syntax_id,
            generics,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::LogDecl {
            name,
            syntax_id,
            generics,
            receiver_type,
            captures,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            if !generics.is_empty() {
                return Err(unsupported(
                    "generic standard routine requirements are not yet supported in V2 Milestone 2",
                ));
            }
            if receiver_type.is_some() {
                return Err(unsupported(
                    "receiver-qualified standard requirements are not yet supported in V2 Milestone 2",
                ));
            }
            if !captures.is_empty() {
                return Err(unsupported(
                    "capturing standard routine requirements are not yet supported in V2 Milestone 2",
                ));
            }
            if !body.is_empty() || !inquiries.is_empty() {
                return Err(unsupported(
                    "default standard routine implementations are not yet supported in V2 Milestone 2",
                ));
            }

            let symbol_id = find_routine_symbol_id_in_scope(
                resolved,
                source_unit_id,
                standard_scope,
                name,
                params,
            )?;
            let _ = lower_named_routine_signature(
                typed,
                resolved,
                source_unit_id,
                name,
                *syntax_id,
                generics,
                None,
                params,
                return_type.as_ref(),
                error_type.as_ref(),
            )?;
            let signature = typed
                .typed_symbol(symbol_id)
                .and_then(|symbol| symbol.declared_type)
                .and_then(|type_id| typed.type_table().get(type_id))
                .and_then(|checked| match checked {
                    CheckedType::Routine(signature) => Some(signature.clone()),
                    _ => None,
                })
                .ok_or_else(|| {
                    internal_error(
                        format!(
                            "typed standard routine '{}' is missing its lowered routine signature",
                            name
                        ),
                        origin.clone(),
                    )
                })?;

            Ok(TypedStandardRoutine {
                symbol_id,
                name: name.clone(),
                params: signature.params,
                return_type: signature.return_type,
                error_type: signature.error_type,
            })
        }
        _ => Err(unsupported(
            "protocol standards currently support only required routine signatures in V2 Milestone 2",
        )),
    }
}

fn lower_standard_symbol_for_contract(
    resolved: &ResolvedProgram,
    contract: &FolType,
) -> Result<SymbolId, TypecheckError> {
    let syntax_id = match contract {
        FolType::Named { syntax_id, .. } => *syntax_id,
        FolType::QualifiedNamed { path } => path.syntax_id(),
        _ => None,
    };
    let display_name = contract
        .named_text()
        .unwrap_or_else(|| format!("{contract:?}"));
    let symbol_id = resolved_symbol_for_syntax(
        resolved,
        syntax_id,
        &display_name,
        match contract {
            FolType::QualifiedNamed { .. } => SymbolReferenceShape::Qualified,
            _ => SymbolReferenceShape::Named,
        },
    )?;
    let symbol = resolved.symbol(symbol_id).ok_or_else(|| {
        internal_error(
            format!("resolved contract symbol '{}' disappeared", display_name),
            type_origin(resolved, contract),
        )
    })?;
    if symbol.kind != SymbolKind::Standard {
        return Err(match type_origin(resolved, contract) {
            Some(origin) => TypecheckError::with_origin(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "type contract '{}' must resolve to a standard declaration",
                    display_name
                ),
                origin,
            ),
            None => TypecheckError::new(
                TypecheckErrorKind::InvalidInput,
                format!(
                    "type contract '{}' must resolve to a standard declaration",
                    display_name
                ),
            ),
        });
    }
    Ok(symbol_id)
}

fn check_standard_conformance(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    syntax: &fol_parser::ast::ParsedPackage,
) -> TypecheckResult<()> {
    let mut errors = Vec::new();
    for (source_unit_index, source_unit) in syntax.source_units.iter().enumerate() {
        if source_unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit_id = SourceUnitId(source_unit_index);
        for item in &source_unit.items {
            let AstNode::TypeDecl {
                name,
                explicit_contracts,
                ..
            } = &item.node
            else {
                continue;
            };
            if explicit_contracts.is_empty() {
                continue;
            }

            let type_symbol_id =
                match find_symbol_id(resolved, source_unit_id, &[SymbolKind::Type], name) {
                    Ok(symbol_id) => symbol_id,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
            let receiver_type =
                match lower_declared_symbol(typed.type_table_mut(), resolved, type_symbol_id) {
                    Ok(type_id) => type_id,
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                };
            let Some(conformance) = typed.typed_conformance(type_symbol_id).cloned() else {
                errors.push(internal_error(
                    format!(
                        "typed conformance metadata disappeared for type '{}'",
                        name
                    ),
                    node_origin(resolved, &item.node),
                ));
                continue;
            };
            for standard_symbol_id in conformance.standard_symbol_ids {
                let Some(standard) = typed.typed_standard(standard_symbol_id).cloned() else {
                    let standard_name = resolved
                        .symbol(standard_symbol_id)
                        .map(|symbol| symbol.name.clone())
                        .unwrap_or_else(|| format!("#{}", standard_symbol_id.0));
                    errors.push(match node_origin(resolved, &item.node) {
                        Some(origin) => TypecheckError::with_origin(
                            TypecheckErrorKind::Unsupported,
                            format!(
                                "type '{}' claims unsupported standard '{}'; only protocol standards are supported in V2 Milestone 2",
                                name, standard_name
                            ),
                            origin,
                        ),
                        None => TypecheckError::new(
                            TypecheckErrorKind::Unsupported,
                            format!(
                                "type '{}' claims unsupported standard '{}'; only protocol standards are supported in V2 Milestone 2",
                                name, standard_name
                            ),
                        ),
                    });
                    continue;
                };
                for requirement in &standard.required_routines {
                    let candidates = typed
                        .all_typed_symbols()
                        .filter(|symbol| {
                            symbol.kind == SymbolKind::Routine
                                && symbol.receiver_type == Some(receiver_type)
                                && resolved
                                    .symbol(symbol.symbol_id)
                                    .is_some_and(|resolved_symbol| {
                                        resolved_symbol.name == requirement.name
                                    })
                        })
                        .filter_map(|symbol| {
                            symbol
                                .declared_type
                                .and_then(|type_id| typed.type_table().get(type_id))
                                .and_then(|checked| match checked {
                                    CheckedType::Routine(signature) => {
                                        Some((symbol.symbol_id, signature.clone()))
                                    }
                                    _ => None,
                                })
                        })
                        .collect::<Vec<_>>();
                    let exact_matches = candidates
                        .iter()
                        .filter(|(_, signature)| {
                            signature.params == requirement.params
                                && signature.return_type == requirement.return_type
                                && signature.error_type == requirement.error_type
                        })
                        .collect::<Vec<_>>();
                    if exact_matches.len() == 1 {
                        continue;
                    }

                    let standard_name = resolved
                        .symbol(standard.symbol_id)
                        .map(|symbol| symbol.name.clone())
                        .unwrap_or_else(|| format!("#{}", standard.symbol_id.0));
                    let mut error = if exact_matches.len() > 1 {
                        match node_origin(resolved, &item.node) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                format!(
                                    "type '{}' satisfies standard '{}' ambiguously: multiple routines match required routine '{}'",
                                    name, standard_name, requirement.name
                                ),
                                origin,
                            ),
                            None => TypecheckError::new(
                                TypecheckErrorKind::InvalidInput,
                                format!(
                                    "type '{}' satisfies standard '{}' ambiguously: multiple routines match required routine '{}'",
                                    name, standard_name, requirement.name
                                ),
                            ),
                        }
                    } else if candidates.is_empty() {
                        match node_origin(resolved, &item.node) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy standard '{}': missing required routine '{}'",
                                    name, standard_name, requirement.name
                                ),
                                origin,
                            ),
                            None => TypecheckError::new(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy standard '{}': missing required routine '{}'",
                                    name, standard_name, requirement.name
                                ),
                            ),
                        }
                    } else {
                        let actual_signatures = candidates
                            .iter()
                            .map(|(_, signature)| {
                                render_standard_signature(
                                    typed,
                                    &requirement.name,
                                    &signature.params,
                                    signature.return_type,
                                    signature.error_type,
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        match node_origin(resolved, &item.node) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy standard '{}': routine '{}' has incompatible signature; expected {}, found {}",
                                    name,
                                    standard_name,
                                    requirement.name,
                                    render_standard_signature(
                                        typed,
                                        &requirement.name,
                                        &requirement.params,
                                        requirement.return_type,
                                        requirement.error_type,
                                    ),
                                    actual_signatures,
                                ),
                                origin,
                            ),
                            None => TypecheckError::new(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy standard '{}': routine '{}' has incompatible signature; expected {}, found {}",
                                    name,
                                    standard_name,
                                    requirement.name,
                                    render_standard_signature(
                                        typed,
                                        &requirement.name,
                                        &requirement.params,
                                        requirement.return_type,
                                        requirement.error_type,
                                    ),
                                    actual_signatures,
                                ),
                            ),
                        }
                    };
                    if let Some(origin) = resolved
                        .symbol(requirement.symbol_id)
                        .and_then(|symbol| symbol.origin.clone())
                    {
                        error = error.with_related_origin(origin, "required by this standard routine");
                    }
                    for (symbol_id, _) in exact_matches {
                        if let Some(origin) =
                            resolved.symbol(*symbol_id).and_then(|symbol| symbol.origin.clone())
                        {
                            error = error.with_related_origin(
                                origin,
                                "matching routine contributing to ambiguity",
                            );
                        }
                    }
                    errors.push(error);
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn render_standard_signature(
    typed: &TypedProgram,
    name: &str,
    params: &[CheckedTypeId],
    return_type: Option<CheckedTypeId>,
    error_type: Option<CheckedTypeId>,
) -> String {
    let params = params
        .iter()
        .map(|type_id| typed.type_table().render_type(*type_id))
        .collect::<Vec<_>>()
        .join(", ");
    let mut signature = format!("fun {name}({params})");
    if let Some(return_type) = return_type {
        signature.push_str(": ");
        signature.push_str(&typed.type_table().render_type(return_type));
    }
    if let Some(error_type) = error_type {
        signature.push_str(" / ");
        signature.push_str(&typed.type_table().render_type(error_type));
    }
    signature
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
            if let Some(instantiated) = parse_instantiated_type_args(name, type_origin(resolved, typ))? {
                let symbol_id = if let Some(syntax_id) = *syntax_id {
                    resolved_symbol_for_syntax(
                        resolved,
                        Some(syntax_id),
                        name,
                        SymbolReferenceShape::Named,
                    )
                    .or_else(|_| {
                        resolve_declared_symbol_by_text(
                            resolved,
                            scope_id,
                            &instantiated.base_name,
                            type_origin(resolved, typ),
                        )
                    })?
                } else {
                    resolve_declared_symbol_by_text(
                        resolved,
                        scope_id,
                        &instantiated.base_name,
                        type_origin(resolved, typ),
                    )?
                };
                let arg_types = instantiated
                    .args
                    .iter()
                    .map(|arg| lower_type(typed, resolved, scope_id, arg))
                    .collect::<Result<Vec<_>, _>>()?;
                return instantiate_declared_generic_type(
                    typed,
                    resolved,
                    symbol_id,
                    &arg_types,
                    type_origin(resolved, typ),
                );
            }
            let symbol_id = if syntax_id.is_some() {
                resolved_symbol_for_syntax(
                    resolved,
                    *syntax_id,
                    name,
                    SymbolReferenceShape::Named,
                )
                .or_else(|_| {
                    resolve_declared_symbol_by_text(
                        resolved,
                        scope_id,
                        name,
                        type_origin(resolved, typ),
                    )
                })?
            } else {
                resolve_declared_symbol_by_text(resolved, scope_id, name, type_origin(resolved, typ))?
            };
            lower_declared_symbol(typed.type_table_mut(), resolved, symbol_id)
        }
        FolType::QualifiedNamed { path } => {
            let joined = path.joined();
            if let Some(instantiated) =
                parse_instantiated_type_args(&joined, type_origin(resolved, typ))?
            {
                let symbol_id = if path.syntax_id().is_some() {
                    resolved_symbol_for_syntax(
                        resolved,
                        path.syntax_id(),
                        &joined,
                        SymbolReferenceShape::Qualified,
                    )
                    .or_else(|_| {
                        resolve_declared_symbol_by_text(
                            resolved,
                            scope_id,
                            &instantiated.base_name,
                            type_origin(resolved, typ),
                        )
                    })?
                } else {
                    resolve_declared_symbol_by_text(
                        resolved,
                        scope_id,
                        &instantiated.base_name,
                        type_origin(resolved, typ),
                    )?
                };
                let arg_types = instantiated
                    .args
                    .iter()
                    .map(|arg| lower_type(typed, resolved, scope_id, arg))
                    .collect::<Result<Vec<_>, _>>()?;
                return instantiate_declared_generic_type(
                    typed,
                    resolved,
                    symbol_id,
                    &arg_types,
                    type_origin(resolved, typ),
                );
            }
            let symbol_id = if path.syntax_id().is_some() {
                resolved_symbol_for_syntax(
                    resolved,
                    path.syntax_id(),
                    &joined,
                    SymbolReferenceShape::Qualified,
                )
                .or_else(|_| {
                    resolve_declared_symbol_by_text(
                        resolved,
                        scope_id,
                        &joined,
                        type_origin(resolved, typ),
                    )
                })?
            } else {
                resolve_declared_symbol_by_text(
                    resolved,
                    scope_id,
                    &joined,
                    type_origin(resolved, typ),
                )?
            };
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

#[derive(Debug)]
struct ParsedInstantiatedType {
    base_name: String,
    args: Vec<FolType>,
}

fn parse_instantiated_type_args(
    raw: &str,
    origin: Option<SyntaxOrigin>,
) -> Result<Option<ParsedInstantiatedType>, TypecheckError> {
    let Some(open_index) = raw.find('[') else {
        return Ok(None);
    };
    if !raw.ends_with(']') {
        return Err(invalid_input_error(
            format!("instantiated type '{raw}' is missing a closing ']'"),
            origin,
        ));
    }
    let base_name = raw[..open_index].trim().to_string();
    let inner = &raw[open_index + 1..raw.len() - 1];
    let args = split_type_argument_text(inner)
        .into_iter()
        .map(|arg| {
            fol_parser::parse_type_reference_text(&arg).map_err(|diagnostic| {
                invalid_input_error(
                    format!("could not parse generic type argument '{arg}': {}", diagnostic.message),
                    origin.clone(),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(ParsedInstantiatedType { base_name, args }))
}

fn split_type_argument_text(raw: &str) -> Vec<String> {
    let mut depth = 0usize;
    let mut current = String::new();
    let mut args = Vec::new();

    for ch in raw.chars() {
        match ch {
            '[' => {
                depth += 1;
                current.push(ch);
            }
            ']' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' | ';' if depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    args.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        args.push(trimmed.to_string());
    }

    args
}

fn resolve_declared_symbol_by_text(
    resolved: &ResolvedProgram,
    scope_id: ScopeId,
    display_name: &str,
    origin: Option<SyntaxOrigin>,
) -> Result<SymbolId, TypecheckError> {
    if display_name.contains("::") {
        let segments = display_name
            .split("::")
            .map(|segment| segment.trim())
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        if segments.len() < 2 {
            return Err(invalid_input_error(
                format!("qualified generic type argument '{display_name}' is malformed"),
                origin,
            ));
        }
        let (mut current_scope, mut current_namespace) = resolve_qualified_type_root_by_text(
            resolved,
            scope_id,
            segments[0],
            display_name,
            origin.clone(),
        )?;
        for segment in &segments[1..segments.len() - 1] {
            current_namespace.push_str("::");
            current_namespace.push_str(segment);
            current_scope = resolved.namespace_scope(&current_namespace).ok_or_else(|| {
                invalid_input_error(
                    format!("could not resolve generic type argument '{display_name}'"),
                    origin.clone(),
                )
            })?;
        }
        return resolve_symbol_in_scope_by_text(
            resolved,
            current_scope,
            segments.last().copied().unwrap_or_default(),
            display_name,
            origin,
        );
    }

    let mut current_scope = Some(scope_id);
    let canonical_name = canonical_identifier_key(display_name);
    while let Some(scope_id) = current_scope {
        let matches = resolved
            .symbols_named_in_scope(scope_id, &canonical_name)
            .into_iter()
            .filter(|symbol| {
                matches!(
                    symbol.kind,
                    SymbolKind::Type
                        | SymbolKind::Alias
                        | SymbolKind::GenericParameter
                        | SymbolKind::Standard
                )
            })
            .collect::<Vec<_>>();
        match matches.len() {
            1 => return Ok(matches[0].id),
            0 => {
                current_scope = resolved.scope(scope_id).and_then(|scope| scope.parent);
            }
            _ => {
                return Err(invalid_input_error(
                    format!("generic type argument '{display_name}' is ambiguous in the current scope"),
                    origin,
                ));
            }
        }
    }

    Err(invalid_input_error(
        format!("could not resolve generic type argument '{display_name}'"),
        origin,
    ))
}

fn resolve_qualified_type_root_by_text(
    resolved: &ResolvedProgram,
    starting_scope: ScopeId,
    root_segment: &str,
    full_path: &str,
    origin: Option<SyntaxOrigin>,
) -> Result<(ScopeId, String), TypecheckError> {
    if root_segment == resolved.package_name() {
        return Ok((resolved.program_scope, resolved.package_name().to_string()));
    }

    let canonical_root = canonical_identifier_key(root_segment);
    let mut current_scope = Some(starting_scope);
    while let Some(scope_id) = current_scope {
        let import_aliases = resolved
            .symbols_named_in_scope(scope_id, &canonical_root)
            .into_iter()
            .filter(|symbol| symbol.kind == SymbolKind::ImportAlias)
            .collect::<Vec<_>>();
        match import_aliases.len() {
            1 => {
                let target_scope = resolved
                    .imports_in_scope(scope_id)
                    .into_iter()
                    .find(|import| import.alias_symbol == import_aliases[0].id)
                    .and_then(|import| import.target_scope)
                    .ok_or_else(|| {
                        invalid_input_error(
                            format!("could not resolve generic type argument '{full_path}'"),
                            origin.clone(),
                        )
                    })?;
                let namespace = resolved
                    .scope(target_scope)
                    .and_then(|scope| match &scope.kind {
                        fol_resolver::ScopeKind::ProgramRoot { package } => Some(package.clone()),
                        fol_resolver::ScopeKind::NamespaceRoot { namespace } => {
                            Some(namespace.clone())
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        TypecheckError::new(
                            TypecheckErrorKind::Internal,
                            "qualified type lookup lost its namespace root",
                        )
                    })?;
                return Ok((target_scope, namespace));
            }
            0 => current_scope = resolved.scope(scope_id).and_then(|scope| scope.parent),
            _ => {
                return Err(invalid_input_error(
                    format!("generic type argument '{full_path}' is ambiguous in the current scope"),
                    origin,
                ))
            }
        }
    }

    let namespace = format!("{}::{}", resolved.package_name(), root_segment);
    resolved
        .namespace_scope(&namespace)
        .map(|scope_id| (scope_id, namespace))
        .ok_or_else(|| {
            invalid_input_error(
                format!("could not resolve generic type argument '{full_path}'"),
                origin,
            )
        })
}

fn resolve_symbol_in_scope_by_text(
    resolved: &ResolvedProgram,
    scope_id: ScopeId,
    name: &str,
    full_path: &str,
    origin: Option<SyntaxOrigin>,
) -> Result<SymbolId, TypecheckError> {
    let canonical_name = canonical_identifier_key(name);
    let matches = resolved
        .symbols_named_in_scope(scope_id, &canonical_name)
        .into_iter()
        .filter(|symbol| {
            matches!(
                symbol.kind,
                SymbolKind::Type
                    | SymbolKind::Alias
                    | SymbolKind::GenericParameter
                    | SymbolKind::Standard
            )
        })
        .collect::<Vec<_>>();
    match matches.len() {
        1 => Ok(matches[0].id),
        0 => Err(invalid_input_error(
            format!("could not resolve generic type argument '{full_path}'"),
            origin,
        )),
        _ => Err(invalid_input_error(
            format!("generic type argument '{full_path}' is ambiguous in the current scope"),
            origin,
        )),
    }
}

fn instantiate_declared_generic_type(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    symbol_id: SymbolId,
    arg_types: &[CheckedTypeId],
    origin: Option<SyntaxOrigin>,
) -> Result<CheckedTypeId, TypecheckError> {
    let typed_symbol = typed.typed_symbol(symbol_id).ok_or_else(|| {
        internal_error("instantiated type symbol disappeared during type lowering", origin.clone())
    })?;
    let template = typed_symbol.declared_type.ok_or_else(|| {
        match origin.clone() {
            Some(origin) => TypecheckError::with_origin(
                TypecheckErrorKind::Unsupported,
                "generic recursive type instantiation is not yet supported",
                origin,
            ),
            None => TypecheckError::new(
                TypecheckErrorKind::Unsupported,
                "generic recursive type instantiation is not yet supported",
            ),
        }
    })?;
    if typed_symbol.generic_params.is_empty() {
        return lower_declared_symbol(typed.type_table_mut(), resolved, symbol_id);
    }
    if typed_symbol.generic_params.len() != arg_types.len() {
        return Err(invalid_input_error(
            format!(
                "generic type '{}' expects {} type argument(s) but got {}",
                resolved
                    .symbol(symbol_id)
                    .map(|symbol| symbol.name.as_str())
                    .unwrap_or("?"),
                typed_symbol.generic_params.len(),
                arg_types.len()
            ),
            origin,
        ));
    }
    let bindings = typed_symbol
        .generic_params
        .iter()
        .copied()
        .zip(arg_types.iter().copied())
        .collect::<BTreeMap<_, _>>();
    substitute_generic_checked_type(typed, template, &bindings, origin)
}

fn substitute_generic_checked_type(
    typed: &mut TypedProgram,
    type_id: CheckedTypeId,
    bindings: &BTreeMap<SymbolId, CheckedTypeId>,
    origin: Option<SyntaxOrigin>,
) -> Result<CheckedTypeId, TypecheckError> {
    let checked = typed
        .type_table()
        .get(type_id)
        .cloned()
        .ok_or_else(|| internal_error("generic type substitution lost a checked type", origin.clone()))?;
    match checked {
        CheckedType::Declared {
            symbol,
            kind: DeclaredTypeKind::GenericParameter,
            ..
        } => bindings.get(&symbol).copied().ok_or_else(|| {
            invalid_input_error(
                format!("generic type substitution left parameter '{}' unbound", symbol.0),
                origin.clone(),
            )
        }),
        CheckedType::Declared { .. } | CheckedType::Builtin(_) => Ok(type_id),
        CheckedType::Array { element_type, size } => {
            let element_type = substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Array { element_type, size }))
        }
        CheckedType::Vector { element_type } => {
            let element_type = substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Vector { element_type }))
        }
        CheckedType::Sequence { element_type } => {
            let element_type = substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Sequence { element_type }))
        }
        CheckedType::Set { member_types } => {
            let member_types = member_types
                .into_iter()
                .map(|member| substitute_generic_checked_type(typed, member, bindings, origin.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(typed.type_table_mut().intern(CheckedType::Set { member_types }))
        }
        CheckedType::Map { key_type, value_type } => {
            let key_type = substitute_generic_checked_type(typed, key_type, bindings, origin.clone())?;
            let value_type =
                substitute_generic_checked_type(typed, value_type, bindings, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Map { key_type, value_type }))
        }
        CheckedType::Optional { inner } => {
            let inner = substitute_generic_checked_type(typed, inner, bindings, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Optional { inner }))
        }
        CheckedType::Error { inner } => {
            let inner = inner
                .map(|inner| substitute_generic_checked_type(typed, inner, bindings, origin.clone()))
                .transpose()?;
            Ok(typed.type_table_mut().intern(CheckedType::Error { inner }))
        }
        CheckedType::Record { fields } => {
            let fields = fields
                .into_iter()
                .map(|(field_name, field_type)| {
                    substitute_generic_checked_type(typed, field_type, bindings, origin.clone())
                        .map(|field_type| (field_name, field_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok(typed.type_table_mut().intern(CheckedType::Record { fields }))
        }
        CheckedType::Entry { variants } => {
            let variants = variants
                .into_iter()
                .map(|(variant_name, variant_type)| {
                    variant_type
                        .map(|variant_type| {
                            substitute_generic_checked_type(typed, variant_type, bindings, origin.clone())
                        })
                        .transpose()
                        .map(|variant_type| (variant_name, variant_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok(typed.type_table_mut().intern(CheckedType::Entry { variants }))
        }
        CheckedType::Routine(signature) => {
            let params = signature
                .params
                .into_iter()
                .map(|param| substitute_generic_checked_type(typed, param, bindings, origin.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            let return_type = signature
                .return_type
                .map(|return_type| {
                    substitute_generic_checked_type(typed, return_type, bindings, origin.clone())
                })
                .transpose()?;
            let error_type = signature
                .error_type
                .map(|error_type| {
                    substitute_generic_checked_type(typed, error_type, bindings, origin.clone())
                })
                .transpose()?;
            Ok(typed.type_table_mut().intern(CheckedType::Routine(RoutineType {
                generic_params: Vec::new(),
                param_names: signature.param_names,
                param_defaults: signature.param_defaults,
                variadic_index: signature.variadic_index,
                params,
                return_type,
                error_type,
            })))
        }
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
        SymbolKind::Standard => {
            return Err(match symbol.origin.clone() {
                Some(origin) => TypecheckError::with_origin(
                    TypecheckErrorKind::Unsupported,
                    format!(
                        "standard '{}' cannot be used as an ordinary type in V2 Milestone 2",
                        symbol.name
                    ),
                    origin,
                ),
                None => TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    format!(
                        "standard '{}' cannot be used as an ordinary type in V2 Milestone 2",
                        symbol.name
                    ),
                ),
            });
        }
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

fn find_routine_symbol_id_in_scope(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    name: &str,
    params: &[Parameter],
) -> Result<SymbolId, TypecheckError> {
    let canonical_name = canonical_identifier_key(name);
    let params = params
        .iter()
        .map(|param| routine_type_key(&param.param_type))
        .collect::<Vec<_>>()
        .join(",");
    let duplicate_key = format!("routine#{canonical_name}#_#{params}");

    resolved
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit_id
                && symbol.scope == scope_id
                && symbol.kind == SymbolKind::Routine
                && symbol.duplicate_key == duplicate_key
        })
        .map(|(symbol_id, _)| symbol_id)
        .ok_or_else(|| {
            internal_error(
                format!(
                    "resolved standard routine symbol '{name}' with duplicate key '{duplicate_key}' is missing from typed lowering"
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

fn record_symbol_generic_params(
    typed: &mut TypedProgram,
    symbol_id: SymbolId,
    generic_params: Vec<SymbolId>,
) -> Result<(), TypecheckError> {
    let symbol = typed.typed_symbol_mut(symbol_id).ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::SymbolTableCorrupted,
            format!(
                "symbol table corrupted: symbol {} is missing while recording generic params",
                symbol_id.0
            ),
        )
    })?;
    symbol.generic_params = generic_params;
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
        AstNode::TypeDecl { options, .. }
            if options
                .iter()
                .any(|option| matches!(option, TypeOption::Extension)) =>
        {
            Some("type extension declarations are planned for a future release")
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
            fol_parser::ast::StandardKind::Protocol => return None,
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

fn find_top_level_type_decl_scope(
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    item: &ParsedTopLevel,
    _symbol_id: SymbolId,
) -> Result<ScopeId, TypecheckError> {
    let parent_scope = resolved
        .source_unit(source_unit_id)
        .map(|source_unit| source_unit.scope_id)
        .ok_or_else(|| internal_error("resolved source unit disappeared during type lowering", None))?;
    let decl_origin = resolved.syntax_index().origin(item.node_id).cloned();
    let source_unit = resolved
        .syntax()
        .source_units
        .get(source_unit_id.0)
        .ok_or_else(|| internal_error("resolved source unit disappeared during type lowering", None))?;
    let type_scope_index = source_unit
        .items
        .iter()
        .filter(|candidate| {
            matches!(
                candidate.node,
                AstNode::TypeDecl { .. } | AstNode::ImpDecl { .. }
            )
        })
        .position(|candidate| candidate.node_id == item.node_id)
        .ok_or_else(|| {
            internal_error(
                "type declaration disappeared from source unit while lowering signatures",
                decl_origin.clone(),
            )
        })?;

    let candidate_scopes = resolved
        .scopes
        .iter_with_ids()
        .filter_map(|(scope_id, scope)| {
            (matches!(scope.kind, fol_resolver::ScopeKind::TypeDeclaration)
                && scope.parent == Some(parent_scope)
                && scope.source_unit == Some(source_unit_id))
                .then_some(scope_id)
        })
        .collect::<Vec<_>>();

    candidate_scopes
        .get(type_scope_index)
        .copied()
        .ok_or_else(|| {
            internal_error(
                "type declaration lost its resolver-owned declaration scope",
                decl_origin,
            )
        })
}

fn generic_params_in_scope(
    resolved: &ResolvedProgram,
    scope_id: ScopeId,
    item: &ParsedTopLevel,
    generics: &[Generic],
) -> Result<Vec<SymbolId>, TypecheckError> {
    let decl_origin = resolved.syntax_index().origin(item.node_id).cloned();
    let generic_symbols = resolved
        .scope(scope_id)
        .ok_or_else(|| {
            internal_error(
                "type declaration resolved to an unknown declaration scope",
                decl_origin.clone(),
            )
        })?
        .symbols
        .iter()
        .filter_map(|symbol_id| resolved.symbol(*symbol_id))
        .filter(|symbol| symbol.kind == SymbolKind::GenericParameter)
        .map(|symbol| symbol.id)
        .collect::<Vec<_>>();

    if generic_symbols.len() != generics.len() {
        return Err(internal_error(
            "generic type declaration lost its resolver-owned parameter scope",
            decl_origin,
        ));
    }

    Ok(generic_symbols)
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

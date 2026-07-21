use crate::{
    CheckedType, CheckedTypeId, DeclaredTypeKind, GenericConstraint, RoutineType, TypeTable,
    TypecheckCapabilityModel, TypecheckError, TypecheckErrorKind, TypecheckResult,
    TypedConformance, TypedConformanceClaim, TypedProgram, TypedStandard, TypedStandardField,
    TypedStandardRoutine,
};
use fol_parser::ast::{
    AstNode, BindingPattern, FolType, Generic, Parameter, ParsedSourceUnitKind, ParsedTopLevel,
    RecordFieldMeta, StandardKind, SyntaxNodeId, SyntaxOrigin, TypeDefinition, TypeOption,
    VarOption,
};
use fol_resolver::{ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::{BTreeMap, HashMap};

type LoweredRoutineGenerics = (Vec<SymbolId>, BTreeMap<SymbolId, Vec<GenericConstraint>>);

pub fn lower_declaration_signatures(typed: &mut TypedProgram) -> TypecheckResult<()> {
    let resolved = typed.resolved().clone();
    let syntax = resolved.syntax().clone();
    let mut errors = Vec::new();

    // Type declarations are lowered for every source unit before any other
    // declarations. Routine signatures and eager binding hints may instantiate
    // generic type templates declared in a later-ordered source unit, so the
    // templates must be recorded first regardless of unit ordering.
    for type_decls_only in [true, false] {
        for (source_unit_index, source_unit) in syntax.source_units.iter().enumerate() {
            if source_unit.kind == ParsedSourceUnitKind::Build {
                continue;
            }
            let source_unit_id = SourceUnitId(source_unit_index);
            for item in &source_unit.items {
                if is_type_decl_item(&item.node) != type_decls_only {
                    continue;
                }
                if let Err(error) =
                    lower_top_level_declaration(typed, &resolved, source_unit_id, item)
                {
                    errors.push(error);
                }
            }
        }
    }

    if let Err(mut conformance_errors) = check_standard_conformance(typed, &resolved, &syntax) {
        errors.append(&mut conformance_errors);
    }

    if errors.is_empty() {
        if let Err(error) = crate::channel_analysis::refine_channel_parameters(typed) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn is_type_decl_item(node: &AstNode) -> bool {
    match node {
        AstNode::TypeDecl { .. } => true,
        AstNode::Commented { node, .. } => is_type_decl_item(node),
        _ => false,
    }
}

/// Record the ordered field layout (declaration order plus per-field default
/// initializers) for a record type and validate that each default expression
/// is assignable to its field's declared type.
#[allow(clippy::too_many_arguments)]
fn lower_record_field_layout(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    type_scope: ScopeId,
    record_type_id: CheckedTypeId,
    fields: &HashMap<String, FolType>,
    field_meta: &HashMap<String, RecordFieldMeta>,
    field_order: &[String],
    decl_origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    use crate::model::RecordFieldLayout;

    let mut layout = Vec::with_capacity(field_order.len());
    for field_name in field_order {
        let Some(field_type) = fields.get(field_name) else {
            continue;
        };
        let field_type_id = lower_type(typed, resolved, type_scope, field_type)?;
        let default = field_meta
            .get(field_name)
            .and_then(|meta| meta.default.clone());

        // A field default must be assignable to the field's declared type;
        // reject a mismatch at the declaration with a located diagnostic.
        if let Some(default_expr) = &default {
            let default_origin =
                node_origin(resolved, default_expr).or_else(|| decl_origin.clone());
            let context = crate::exprs::TypeContext {
                source_unit_id,
                scope_id: type_scope,
                routine_return_type: None,
                routine_error_type: None,
                error_call_mode: crate::exprs::ErrorCallMode::Propagate,
                processor_task_call: None,
                allow_mutex_handle: false,
                repeating_loop_scope: None,
                inside_deferred_block: false,
                inside_error_deferred_block: false,
                field_projection_root: false,
                direct_spawn_anonymous: false,
                direct_binding_anonymous: false,
            };
            let typed_default = crate::exprs::type_node_with_expectation(
                typed,
                resolved,
                context,
                default_expr,
                Some(field_type_id),
            )
            .map_err(|error| {
                default_origin
                    .clone()
                    .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
            })?;
            if let Some(actual) = typed_default.value_type {
                crate::exprs::helpers::ensure_assignable(
                    typed,
                    field_type_id,
                    actual,
                    format!("default for record field '{field_name}'"),
                    default_origin,
                )?;
            }
        }

        layout.push(RecordFieldLayout {
            name: field_name.clone(),
            type_id: field_type_id,
            default,
        });
    }
    typed.set_record_layout(record_type_id, layout);
    Ok(())
}

/// A custom finalizer (`finalize` on a `fin` type) runs foreign-resource cleanup
/// effects, so it must be declared `pro`, not `fun`/`log` (plan §4.2, §6.1).
/// Types are lowered before routines, so the `fin` claim is already recorded.
fn reject_fun_finalizer(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    scope: ScopeId,
    node: &AstNode,
    name: &str,
    receiver_type: Option<&fol_parser::ast::FolType>,
) -> Result<(), TypecheckError> {
    if name != "finalize" {
        return Ok(());
    }
    let Some(receiver_ast) = receiver_type else {
        return Ok(());
    };
    let receiver_checked = lower_type(typed, resolved, scope, receiver_ast)?;
    if !typed.type_resolves_to_fin(receiver_checked) {
        return Ok(());
    }
    if matches!(node, AstNode::ProDecl { .. }) {
        return Ok(());
    }
    let message =
        "a custom finalizer 'finalize' must be declared 'pro', not 'fun'; finalization performs foreign-resource cleanup effects";
    Err(node_origin(resolved, node).map_or_else(
        || TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin),
    ))
}

/// A `fun` may not accept a mutable input loan (V3_MEM §4.2), so a method with a
/// `[mut, bor]` receiver must be a `pro` — mutating a receiver through the loan
/// is an effect. Shared `[bor]` receivers stay allowed on `fun`.
fn reject_fun_mutable_receiver(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    scope: ScopeId,
    node: &AstNode,
    receiver_type: Option<&fol_parser::ast::FolType>,
) -> Result<(), TypecheckError> {
    let Some(receiver_ast) = receiver_type else {
        return Ok(());
    };
    if matches!(node, AstNode::ProDecl { .. }) {
        return Ok(());
    }
    let receiver_checked = lower_type(typed, resolved, scope, receiver_ast)?;
    if !matches!(
        typed.type_table().get(receiver_checked),
        Some(CheckedType::Borrowed { mutable: true, .. })
    ) {
        return Ok(());
    }
    let message =
        "a 'fun' cannot take a mutable '[mut, bor]' receiver; declare it 'pro' (V3_MEM §4.2: a fun may not accept mutable input loans). A shared '[bor]' receiver is allowed on a fun";
    Err(node_origin(resolved, node).map_or_else(
        || TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
        |origin| TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin),
    ))
}

fn lower_top_level_declaration(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    item: &ParsedTopLevel,
) -> Result<(), TypecheckError> {
    if let Some(error) = unsupported_v1_top_level_decl(resolved, item, typed.capability_model()) {
        return Err(error);
    }

    match &item.node {
        AstNode::VarDecl {
            name,
            type_hint,
            options,
            ..
        }
        | AstNode::LabDecl {
            name,
            type_hint,
            options,
            ..
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
                let mut type_id = lower_type(typed, resolved, symbol_scope, type_hint)?;
                if options
                    .iter()
                    .any(|option| matches!(option, VarOption::New))
                {
                    reject_heap_backed_type_in_core(
                        typed,
                        resolved,
                        type_hint,
                        "heap allocation binding",
                        resolved
                            .symbol(symbol_id)
                            .and_then(|symbol| symbol.origin.clone())
                            .or_else(|| node_origin(resolved, &item.node)),
                    )?;
                    type_id = typed
                        .type_table_mut()
                        .intern(CheckedType::Owned { inner: type_id });
                }
                crate::exprs::bindings::reject_unsupported_top_level_binding_type(
                    typed,
                    resolved,
                    symbol_id,
                    type_id,
                    resolved
                        .symbol(symbol_id)
                        .and_then(|symbol| symbol.origin.clone())
                        .or_else(|| node_origin(resolved, &item.node)),
                )?;
                record_symbol_type(typed, symbol_id, type_id)?;
                // A `var state: mux[T]` local is a first-class managed mutex
                // (V3_MEM §8.3). It lowers to the guarded inner `T` plus the
                // `is_mutex` flag, so `.lock()`/`.unlock()`/guarded field access
                // reuse the same machinery as a `mux[T]` parameter.
                if matches!(type_hint, FolType::Mutex { .. }) {
                    if let Some(symbol) = typed.typed_symbol_mut(symbol_id) {
                        symbol.is_mutex = true;
                    }
                }
            }
        }
        AstNode::DestructureDecl {
            pattern,
            type_hint: Some(type_hint),
            ..
        } => {
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
                crate::exprs::bindings::reject_unsupported_top_level_binding_type(
                    typed,
                    resolved,
                    symbol_id,
                    type_id,
                    resolved
                        .symbol(symbol_id)
                        .and_then(|symbol| symbol.origin.clone())
                        .or_else(|| node_origin(resolved, &item.node)),
                )?;
                record_symbol_type(typed, symbol_id, type_id)?;
            }
        }
        AstNode::DestructureDecl { .. } => {}
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
            reject_fun_finalizer(
                typed,
                resolved,
                signature_scope,
                &item.node,
                name,
                receiver_type.as_ref(),
            )?;
            reject_fun_mutable_receiver(
                typed,
                resolved,
                signature_scope,
                &item.node,
                receiver_type.as_ref(),
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
            let generic_params = generic_params_in_scope(resolved, type_scope, item, generics)?;
            let generic_constraints = lower_generic_constraints_for_params(
                typed,
                resolved,
                type_scope,
                generics,
                &generic_params,
            )?;
            // Record the generic parameters/constraints before lowering the
            // body so a self-referential field (`typ Node(T) = { next: Node[T] }`)
            // sees `Node` as a generic type and interns a nominal `Node[T]` node
            // instead of re-lowering the declaration and recursing forever.
            record_symbol_generic_params(typed, symbol_id, generic_params)?;
            record_symbol_generic_constraints(typed, symbol_id, generic_constraints)?;
            let type_id = match type_def {
                TypeDefinition::Alias { target } => {
                    // §8.1: same rule as `ali` — an eventual cannot hide
                    // behind a type name; the lifetime is spelled per
                    // signature.
                    if matches!(target, FolType::Eventual { .. }) {
                        return Err(TypecheckError::new(
                            TypecheckErrorKind::Unsupported,
                            format!(
                                "an eventual type cannot be named through type declaration '{name}'; spell 'evt[L, T]' directly in each signature"
                            ),
                        ));
                    }
                    lower_type(typed, resolved, type_scope, target)?
                }
                TypeDefinition::Record {
                    fields,
                    field_meta,
                    field_order,
                    ..
                } => {
                    let mut lowered = BTreeMap::new();
                    for (field_name, field_type) in fields {
                        lowered.insert(
                            field_name.clone(),
                            lower_type(typed, resolved, type_scope, field_type)?,
                        );
                    }
                    let record_type_id = typed
                        .type_table_mut()
                        .intern(CheckedType::Record { fields: lowered });
                    let decl_origin = node_origin(resolved, &item.node)
                        .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                    lower_record_field_layout(
                        typed,
                        resolved,
                        source_unit_id,
                        type_scope,
                        record_type_id,
                        fields,
                        field_meta,
                        field_order,
                        decl_origin,
                    )?;
                    record_type_id
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
            crate::exprs::helpers::reject_embedded_full_channel(
                typed,
                type_id,
                node_origin(resolved, &item.node)
                    .or_else(|| resolved.syntax_index().origin(item.node_id).cloned()),
            )?;
            record_symbol_type(typed, symbol_id, type_id)?;
            // Directly value-recursive definitions (`typ Node(T) = { next:
            // Node[T] }`, `typ Tree = { kids: vec[Tree] }`) have no finite
            // shape. V3's shipped recursive path uses explicit `@`-owned
            // indirection, which the check below permits. Reject only the
            // unguarded value-recursive form with a located diagnostic instead
            // of looping forever in lowering.
            let recursion_origin = node_origin(resolved, &item.node)
                .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
            reject_recursive_type_definition(
                typed,
                symbol_id,
                type_id,
                recursion_origin,
                resolved,
            )?;
            if !explicit_contracts.is_empty() {
                let mut standard_symbol_ids = Vec::new();
                let mut claims = Vec::new();
                for contract in explicit_contracts {
                    // Compiler-owned capability standards (`copy`, `clone`,
                    // `fin`, `send`, `share`) are recognized directly rather than
                    // resolved to a user `std` symbol. Record the claim on the
                    // type without a standard symbol; structural verification of
                    // the claim is a later slice.
                    let contract_base = contract
                        .named_text()
                        .map(|name| name.split('[').next().unwrap_or(&name).to_string());
                    if let Some(capability) = contract_base
                        .as_deref()
                        .filter(|name| fol_parser::ast::is_capability_standard(name))
                    {
                        typed.record_capability_claim(symbol_id, capability.to_string());
                        // `copy` implies `clone`: a value that duplicates freely
                        // can always produce an independent clone.
                        if capability == "copy" {
                            typed.record_capability_claim(symbol_id, "clone".to_string());
                        }
                        continue;
                    }
                    let standard_symbol_id =
                        lower_standard_symbol_for_contract(resolved, contract)?;
                    standard_symbol_ids.push(standard_symbol_id);
                    // Pull explicit type arguments out of
                    // `Name[args]`-shaped contract references and lower
                    // them in the type declaration scope.
                    let type_args =
                        extract_contract_type_args(typed, resolved, type_scope, contract)?;
                    claims.push(TypedConformanceClaim {
                        standard_symbol_id,
                        type_args,
                    });
                }
                // A `copy` value is duplicated with no ownership bookkeeping, so
                // it cannot also carry custom finalization: `copy` and `fin`
                // cannot coexist on the same type.
                if typed
                    .capability_claims(symbol_id)
                    .is_some_and(|capabilities| capabilities.contains("fin"))
                {
                    typed.record_fin_type(type_id);
                }
                if let Some(capabilities) = typed.capability_claims(symbol_id) {
                    if capabilities.contains("copy") && capabilities.contains("fin") {
                        let message =
                            "a type cannot claim both 'copy' and 'fin'; a copyable value cannot have custom finalization";
                        let origin = node_origin(resolved, &item.node)
                            .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                        return Err(origin.map_or_else(
                            || TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
                            |origin| {
                                TypecheckError::with_origin(
                                    TypecheckErrorKind::InvalidInput,
                                    message,
                                    origin,
                                )
                            },
                        ));
                    }
                    // `copy` duplicates a value bit-for-bit, so every field must
                    // itself be copy-safe. Reject the claim when a field is a
                    // known move-only/heap type (`str`, owned heap, pointer,
                    // channel, eventual, borrow). Record/entry fields are allowed
                    // here (their own `copy` claim is verified on their own
                    // declaration) to avoid declaration-order false rejections.
                    if capabilities.contains("copy") {
                        if let Some(offending) = first_known_non_copy_field(typed, type_id)? {
                            let message = format!(
                                "'copy' requires every field to be copy-safe, but field '{offending}' is not"
                            );
                            let origin = node_origin(resolved, &item.node)
                                .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                            return Err(match origin {
                                Some(origin) => TypecheckError::with_origin(
                                    TypecheckErrorKind::InvalidInput,
                                    message,
                                    origin,
                                ),
                                None => {
                                    TypecheckError::new(TypecheckErrorKind::InvalidInput, message)
                                }
                            });
                        }
                    }
                    // `send` lets a value cross a task/thread boundary and
                    // `share` lets shared access cross one, so every field must
                    // itself be thread-safe. Reject either claim when a field
                    // owns a `fin` foreign resource or an `Rc` pointer.
                    let thread_safety_claim = if capabilities.contains("send") {
                        Some("send")
                    } else if capabilities.contains("share") {
                        Some("share")
                    } else {
                        None
                    };
                    if let Some(standard) = thread_safety_claim {
                        if let Some(offending) = first_known_non_thread_safe_field(typed, type_id)?
                        {
                            let message = format!(
                                "'{standard}' requires every field to be {standard}-safe, but field '{offending}' is not"
                            );
                            let origin = node_origin(resolved, &item.node)
                                .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                            return Err(match origin {
                                Some(origin) => TypecheckError::with_origin(
                                    TypecheckErrorKind::InvalidInput,
                                    message,
                                    origin,
                                ),
                                None => {
                                    TypecheckError::new(TypecheckErrorKind::InvalidInput, message)
                                }
                            });
                        }
                    }
                }
                typed.record_typed_conformance(TypedConformance {
                    type_symbol_id: symbol_id,
                    standard_symbol_ids,
                    claims,
                });
            }
        }
        AstNode::StdDecl {
            syntax_id,
            name,
            generics,
            kind,
            body,
            ..
        } => {
            let standard_symbol_id =
                find_symbol_id(resolved, source_unit_id, &[SymbolKind::Standard], name)?;
            let standard_scope = syntax_id
                .and_then(|id| resolved.scope_for_syntax(id))
                .ok_or_else(|| {
                    internal_error(
                        format!(
                            "resolved standard scope disappeared for standard '{}'",
                            name
                        ),
                        node_origin(resolved, &item.node)
                            .or_else(|| resolved.syntax_index().origin(item.node_id).cloned()),
                    )
                })?;
            // Bind the standard's generic parameters as declared types in
            // the standard scope so routine signatures inside the body
            // see `T` as a proper generic parameter.
            let mut generic_params: Vec<SymbolId> = Vec::new();
            for generic in generics {
                let symbol_id = find_symbol_id_in_scope(
                    resolved,
                    source_unit_id,
                    standard_scope,
                    &[SymbolKind::GenericParameter],
                    &generic.name,
                )?;
                let generic_type = typed.type_table_mut().intern(CheckedType::Declared {
                    symbol: symbol_id,
                    name: generic.name.clone(),
                    kind: DeclaredTypeKind::GenericParameter,
                    args: Vec::new(),
                });
                record_symbol_type(typed, symbol_id, generic_type)?;
                generic_params.push(symbol_id);
            }
            let mut required_routines = Vec::new();
            let mut required_fields = Vec::new();
            match kind {
                StandardKind::Protocol => {
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
                }
                StandardKind::Blueprint => {
                    for member in body {
                        let required = lower_blueprint_standard_member(
                            typed,
                            resolved,
                            source_unit_id,
                            standard_scope,
                            member,
                        )?;
                        required_fields.push(required);
                    }
                }
                StandardKind::Extended => {
                    // Extended standards combine protocol and blueprint
                    // requirements. Routine members lower through the
                    // protocol path; field members lower through the
                    // blueprint path. Each member is routed to the path
                    // matching its AST shape.
                    for member in body {
                        match member {
                            AstNode::FunDecl { .. }
                            | AstNode::ProDecl { .. }
                            | AstNode::LogDecl { .. } => {
                                let required = lower_protocol_standard_member(
                                    typed,
                                    resolved,
                                    source_unit_id,
                                    standard_scope,
                                    member,
                                )?;
                                required_routines.push(required);
                            }
                            AstNode::VarDecl { .. } => {
                                let required = lower_blueprint_standard_member(
                                    typed,
                                    resolved,
                                    source_unit_id,
                                    standard_scope,
                                    member,
                                )?;
                                required_fields.push(required);
                            }
                            _ => {
                                return Err(match node_origin(resolved, member) {
                                    Some(origin) => TypecheckError::with_origin(
                                        TypecheckErrorKind::Unsupported,
                                        "extended standards currently support only required routines and `var` field declarations",
                                        origin,
                                    ),
                                    None => TypecheckError::new(
                                        TypecheckErrorKind::Unsupported,
                                        "extended standards currently support only required routines and `var` field declarations",
                                    ),
                                });
                            }
                        }
                    }
                }
            }
            typed.record_typed_standard(TypedStandard {
                symbol_id: standard_symbol_id,
                scope_id: standard_scope,
                kind: *kind,
                generic_params,
                required_routines,
                required_fields,
            });
        }
        AstNode::AliasDecl { name, target, .. } => {
            // §8.1: an eventual's parent-scope lifetime is spelled at each
            // signature; hiding `evt[...]` behind an alias would evade that
            // spelling (and alias-typed handles cannot await anyway).
            if matches!(target, FolType::Eventual { .. }) {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    format!(
                        "an eventual type cannot be aliased as '{name}'; spell 'evt[L, T]' directly in each signature"
                    ),
                ));
            }
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
        let is_top_level_standard =
            syntax_id.is_some_and(|id| source_unit.top_level_nodes.contains(&id));
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
        if let Some(error) = unsupported_v1_nested_decl(resolved, node, typed.capability_model()) {
            return Err(error);
        }
    }

    match node {
        AstNode::VarDecl {
            name,
            type_hint,
            options,
            ..
        }
        | AstNode::LabDecl {
            name,
            type_hint,
            options,
            ..
        } => {
            if let Some(type_hint) = type_hint {
                let symbol_id = find_symbol_id_in_scope(
                    resolved,
                    source_unit_id,
                    current_scope,
                    &[symbol_kind_for_node(node)],
                    name,
                )?;
                let mut type_id =
                    lower_type(typed, resolved, current_scope, type_hint).map_err(|error| {
                        resolved
                            .symbol(symbol_id)
                            .and_then(|symbol| symbol.origin.clone())
                            .or_else(|| node_origin(resolved, node))
                            .map_or(error.clone(), |origin| error.with_fallback_origin(origin))
                    })?;
                if options
                    .iter()
                    .any(|option| matches!(option, VarOption::New))
                {
                    reject_heap_backed_type_in_core(
                        typed,
                        resolved,
                        type_hint,
                        "heap allocation binding",
                        resolved
                            .symbol(symbol_id)
                            .and_then(|symbol| symbol.origin.clone())
                            .or_else(|| node_origin(resolved, node)),
                    )?;
                    type_id = typed
                        .type_table_mut()
                        .intern(CheckedType::Owned { inner: type_id });
                }
                record_symbol_type(typed, symbol_id, type_id)?;
                // A `var state: mux[T]` local is a first-class managed mutex
                // (V3_MEM §8.3): mark the binding so `.lock()`/`.unlock()` and
                // guarded field access reuse the `mux[T]` parameter machinery.
                if matches!(type_hint, FolType::Mutex { .. }) {
                    if let Some(symbol) = typed.typed_symbol_mut(symbol_id) {
                        symbol.is_mutex = true;
                    }
                }
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
            reject_fun_finalizer(
                typed,
                resolved,
                routine_scope,
                node,
                name,
                receiver_type.as_ref(),
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
        AstNode::Dfr {
            syntax_id, body, ..
        }
        | AstNode::Edf {
            syntax_id, body, ..
        } => {
            let deferred_scope =
                nested_scope_for_syntax(resolved, current_scope, *syntax_id, "dfr block")?;
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
        AstNode::When { cases, default, .. } => {
            // Case and default bodies live in their own resolver Block
            // scopes; nested bindings must be lowered against those scopes.
            let mut bodies: Vec<&[fol_parser::ast::AstNode]> = Vec::new();
            for case in cases {
                match case {
                    fol_parser::ast::WhenCase::Case { body, .. }
                    | fol_parser::ast::WhenCase::Is { body, .. }
                    | fol_parser::ast::WhenCase::In { body, .. }
                    | fol_parser::ast::WhenCase::Has { body, .. }
                    | fol_parser::ast::WhenCase::On { body, .. }
                    | fol_parser::ast::WhenCase::Of { body, .. } => bodies.push(body),
                }
            }
            if let Some(default) = default {
                bodies.push(default);
            }
            for body in bodies {
                let body_scope = crate::exprs::inline_body_block_scope(
                    resolved,
                    source_unit_id,
                    current_scope,
                    body,
                )
                .unwrap_or(current_scope);
                lower_nested_declarations_in_nodes(
                    typed,
                    resolved,
                    source_unit_id,
                    body_scope,
                    body,
                )?;
            }
        }
        AstNode::Loop {
            syntax_id,
            condition,
            body,
        } => {
            // Every loop body is a lexical scope. Iteration loops use that
            // same scope for their binder; condition loops use an ordinary
            // block scope.
            let body_scope = crate::exprs::loop_body_scope(resolved, *syntax_id)?;
            match condition.as_ref() {
                fol_parser::ast::LoopCondition::Condition(cond) => {
                    lower_nested_declarations_in_node(
                        typed,
                        resolved,
                        source_unit_id,
                        current_scope,
                        cond,
                    )?;
                    lower_nested_declarations_in_nodes(
                        typed,
                        resolved,
                        source_unit_id,
                        body_scope,
                        body,
                    )?;
                }
                fol_parser::ast::LoopCondition::Iteration {
                    var: _,
                    iterable,
                    condition: guard,
                    ..
                } => {
                    lower_nested_declarations_in_node(
                        typed,
                        resolved,
                        source_unit_id,
                        current_scope,
                        iterable,
                    )?;
                    let binder_scope = body_scope;
                    if let Some(guard) = guard.as_deref() {
                        lower_nested_declarations_in_node(
                            typed,
                            resolved,
                            source_unit_id,
                            binder_scope,
                            guard,
                        )?;
                    }
                    lower_nested_declarations_in_nodes(
                        typed,
                        resolved,
                        source_unit_id,
                        binder_scope,
                        body,
                    )?;
                }
            }
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

/// §8.1: an eventual crossing a routine signature must spell its parent-scope
/// lifetime as `evt[L, T]` with `L` declared `L: lif` in the routine's generic
/// list. Applies to parameters and the return type alike; local bindings may
/// elide `L`.
fn require_signature_eventual_lifetime(
    resolved: &ResolvedProgram,
    generics: &[Generic],
    fol_type: &FolType,
    surface: &str,
    type_word: &str,
    syntax_id: Option<SyntaxNodeId>,
) -> Result<(), TypecheckError> {
    let FolType::Eventual { lifetime, .. } = fol_type else {
        return Ok(());
    };
    let routine_origin = syntax_id.and_then(|id| resolved.syntax_index().origin(id).cloned());
    match lifetime {
        None => {
            let message = format!(
                "an eventual {surface} must name its parent-scope lifetime; declare 'L: lif' and spell the {type_word} 'evt[L, T]'"
            );
            Err(routine_origin.map_or_else(
                || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
                |origin| {
                    TypecheckError::with_origin(
                        TypecheckErrorKind::Ownership,
                        message.clone(),
                        origin,
                    )
                },
            ))
        }
        Some(name) => {
            let declared_lif = generics.iter().any(|generic| {
                generic.name.eq_ignore_ascii_case(name)
                    && generic.constraints.iter().any(|constraint| {
                        matches!(constraint, FolType::Named { name, .. }
                            if name.split('[').next().unwrap_or(name) == "lif")
                    })
            });
            if declared_lif {
                Ok(())
            } else {
                let message = format!(
                    "eventual lifetime '{name}' is not a declared lifetime parameter; declare '{name}: lif' in the routine's generic list"
                );
                Err(routine_origin.map_or_else(
                    || TypecheckError::new(TypecheckErrorKind::Ownership, message.clone()),
                    |origin| {
                        TypecheckError::with_origin(
                            TypecheckErrorKind::Ownership,
                            message.clone(),
                            origin,
                        )
                    },
                ))
            }
        }
    }
}

// Signature lowering receives the routine AST fields alongside its semantic
// context; bundling them would duplicate the parser's routine representation.
#[allow(clippy::too_many_arguments)]
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
    let (generic_params, generic_constraints) =
        lower_routine_generic_params(typed, resolved, source_unit_id, signature_scope, generics)?;
    let mut lowered_params = Vec::new();
    for param in params {
        let mut param_type = lower_type(typed, resolved, signature_scope, &param.param_type)?;
        if param.is_borrowable {
            param_type = typed.type_table_mut().intern(CheckedType::Borrowed {
                inner: param_type,
                mutable: false,
            });
        }
        let param_symbol_id = find_symbol_id_in_scope(
            resolved,
            source_unit_id,
            signature_scope,
            &[SymbolKind::Parameter],
            &param.name,
        )?;
        record_symbol_type(typed, param_symbol_id, param_type)?;
        if param.is_mutex {
            if let Some(symbol) = typed.typed_symbol_mut(param_symbol_id) {
                symbol.is_mutex = true;
            }
        }
        // §5.3: a parameter typed `{fun (...): T}[bor=L]` receives a closure
        // whose environment may hold loans tied to the caller's region; inside
        // this routine it obeys the same nonescaping rules as a local
        // borrowed-environment closure.
        if matches!(
            &param.param_type,
            FolType::Function {
                env_lifetime: Some(_),
                ..
            }
        ) {
            typed.mark_bor_env_closure(param_symbol_id);
        }
        lowered_params.push(param_type);
    }
    // §8.1: an eventual that crosses a routine signature spells its
    // parent-scope lifetime (`evt[L, T]` with a declared `L: lif`); the
    // elided `evt[T]` form is local-declaration shorthand only. With every
    // storage escape (fields, containers, channels, globals, closures,
    // detached tasks) rejected elsewhere, a handle can only travel through
    // signatures, so requiring `L` on BOTH sides makes outliving `L`
    // structurally impossible — this is the conservative region model.
    for param in params {
        require_signature_eventual_lifetime(
            resolved,
            generics,
            &param.param_type,
            &format!("received as parameter '{}'", param.name),
            "parameter type",
            syntax_id,
        )?;
    }
    // An eventual has no method surface: its whole API is `| await` plus the
    // signature flows above. A receiver would smuggle a handle past the
    // lifetime spelling, so reject the declaration outright.
    if let Some(FolType::Eventual { .. }) = receiver_type {
        let message = "an eventual cannot be a method receiver; await it, or pass it to a named routine through an 'evt[L, T]' parameter";
        let routine_origin = syntax_id.and_then(|id| resolved.syntax_index().origin(id).cloned());
        return Err(routine_origin.map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Ownership, message),
            |origin| TypecheckError::with_origin(TypecheckErrorKind::Ownership, message, origin),
        ));
    }
    if let Some(return_type) = return_type {
        require_signature_eventual_lifetime(
            resolved,
            generics,
            return_type,
            "returned from a routine",
            "return type",
            syntax_id,
        )?;
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
    if let Some(receiver_checked) = lowered_receiver {
        let self_symbol_id = find_symbol_id_in_scope(
            resolved,
            source_unit_id,
            signature_scope,
            &[SymbolKind::Parameter],
            "self",
        )?;
        record_symbol_type(typed, self_symbol_id, receiver_checked)?;
    }
    let param_defaults = params
        .iter()
        .map(|param| param.default.clone())
        .collect::<Vec<_>>();
    let routine_type = typed
        .type_table_mut()
        .intern(CheckedType::Routine(RoutineType {
            generic_params,
            generic_constraints: generic_constraints.clone(),
            param_names: params.iter().map(|param| param.name.clone()).collect(),
            param_defaults: param_defaults.clone(),
            variadic_index: params.iter().position(|param| param.is_variadic),
            mutex_params: params
                .iter()
                .enumerate()
                .filter_map(|(index, param)| param.is_mutex.then_some(index))
                .collect(),
            params: lowered_params,
            return_type: lowered_return,
            error_type: lowered_error,
            env_lifetime: false,
        }));
    record_symbol_generic_constraints(typed, symbol_id, generic_constraints)?;
    record_symbol_type(typed, symbol_id, routine_type)?;
    record_symbol_receiver_type(typed, symbol_id, lowered_receiver)?;
    record_symbol_param_defaults(typed, symbol_id, param_defaults)?;
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
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
        }
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
            let _ = (generics, receiver_type, captures);
            let has_default_body = !body.is_empty() || !inquiries.is_empty();

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
                has_default_body,
            })
        }
        _ => Err(unsupported(
            "protocol standards currently support only required routine signatures in V2 Milestone 2",
        )),
    }
}

/// Given a type-contract reference like `Iterator[int]`, lower the
/// inner type arguments into `CheckedTypeId`s for later substitution
/// into the standard's required routine/field signatures. Returns an
/// empty list when the contract has no explicit type arguments.
fn extract_contract_type_args(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    scope_id: ScopeId,
    contract: &FolType,
) -> Result<Vec<CheckedTypeId>, TypecheckError> {
    let raw_name = match contract {
        FolType::Named { name, .. } => name.clone(),
        _ => return Ok(Vec::new()),
    };
    let Some(open_index) = raw_name.find('[') else {
        return Ok(Vec::new());
    };
    let Some(parsed) = parse_instantiated_type_args(&raw_name, type_origin(resolved, contract))?
    else {
        let _ = open_index;
        return Ok(Vec::new());
    };
    let mut lowered = Vec::with_capacity(parsed.args.len());
    for arg in parsed.args {
        lowered.push(lower_type(typed, resolved, scope_id, &arg)?);
    }
    Ok(lowered)
}

fn lower_blueprint_standard_member(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    source_unit_id: SourceUnitId,
    standard_scope: ScopeId,
    member: &AstNode,
) -> Result<TypedStandardField, TypecheckError> {
    let origin = node_origin(resolved, member);
    let unsupported = |message: &'static str| match origin.clone() {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
    };

    let (name, type_hint, value) = match member {
        AstNode::VarDecl {
            name,
            type_hint,
            value,
            ..
        } => (name.as_str(), type_hint.clone(), value.clone()),
        AstNode::LabDecl { .. } | AstNode::DestructureDecl { .. } => {
            return Err(unsupported(
                "blueprint standards currently support only required `var` field declarations",
            ));
        }
        _ => {
            return Err(unsupported(
                "blueprint standards currently support only required `var` field declarations",
            ));
        }
    };

    // Required field declarations are static contracts — they must not
    // carry an initializer at the standard site. The conformer declares
    // its own initializer.
    if value.is_some() {
        return Err(unsupported(
            "blueprint required fields must not provide an initializer; conformers supply the value",
        ));
    }
    let Some(type_hint) = type_hint else {
        return Err(unsupported(
            "blueprint required fields must declare an explicit type",
        ));
    };

    let symbol_id = find_symbol_id_in_scope(
        resolved,
        source_unit_id,
        standard_scope,
        &[SymbolKind::ValueBinding],
        name,
    )?;
    let field_type = lower_type(typed, resolved, standard_scope, &type_hint)?;
    record_symbol_type(typed, symbol_id, field_type)?;

    Ok(TypedStandardField {
        symbol_id,
        name: name.to_string(),
        field_type,
    })
}

/// Lower a generic-parameter constraint `T: Std` or `T: Std[args]` into a
/// `GenericConstraint` carrying the standard symbol and its lowered type
/// arguments. For a non-generic standard `args` is empty; for a generic
/// standard the args let a constraint call substitute the standard's own
/// parameters on demand (mirroring the conformance-header path).
fn lower_standard_constraint_for_contract(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    scope_id: ScopeId,
    constraint: &FolType,
) -> Result<GenericConstraint, TypecheckError> {
    let standard = lower_standard_symbol_for_contract(resolved, constraint)?;
    let args = extract_contract_type_args(typed, resolved, scope_id, constraint)?;
    Ok(GenericConstraint { standard, args })
}

fn lower_standard_symbol_for_contract(
    resolved: &ResolvedProgram,
    contract: &FolType,
) -> Result<SymbolId, TypecheckError> {
    let display_name = contract
        .named_text()
        .unwrap_or_else(|| format!("{contract:?}"));
    let syntax_id = match contract {
        FolType::Named { syntax_id, .. } => *syntax_id,
        FolType::QualifiedNamed { path } => path.syntax_id(),
        _ => {
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
            })
        }
    };
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
            // §4.1 recursive `copy` verification. `copy` has no structural
            // default (unlike `clone`), so a `copy` type's nominal aggregate
            // fields must each also claim `copy`. This runs after the main type
            // pass, so every type's capability claims are already recorded and
            // the check is declaration-order independent.
            if typed
                .capability_claims(type_symbol_id)
                .is_some_and(|caps| caps.contains("copy"))
            {
                match first_field_lacking_declared_copy(typed, receiver_type) {
                    Ok(Some(offending)) => {
                        let message = format!(
                            "'copy' is verified recursively: field '{offending}' has a type that does not claim 'copy'"
                        );
                        let origin = node_origin(resolved, &item.node)
                            .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                        errors.push(match origin {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                message,
                                origin,
                            ),
                            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
                        });
                        continue;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                }
            }
            // §4.1 recursive `send`/`share` verification: a thread-safe claim on
            // an aggregate is unsound if any TRANSITIVELY nested field is a `fin`
            // value, `Rc` shared/weak pointer, or borrow (the emitted Rust would
            // fail its own `Send`/`Sync` bound). The main type pass only checks
            // direct fields; recurse here where every type is lowered.
            let thread_safe_claim = typed.capability_claims(type_symbol_id).and_then(|caps| {
                if caps.contains("send") {
                    Some("send")
                } else if caps.contains("share") {
                    Some("share")
                } else {
                    None
                }
            });
            if let Some(claim) = thread_safe_claim {
                // The claiming type must not ITSELF be a non-thread-safe leaf
                // (e.g. a `fin` value is neither `Send` nor `Sync`).
                match type_is_known_non_thread_safe(typed, receiver_type) {
                    Ok(true) => {
                        let message = format!(
                            "a type that claims '{claim}' cannot itself be a non-{claim}-safe value (a 'fin' value or 'Rc' pointer is neither 'send' nor 'share')"
                        );
                        let origin = node_origin(resolved, &item.node)
                            .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                        errors.push(match origin {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                message,
                                origin,
                            ),
                            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
                        });
                        continue;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                }
                match first_field_transitively_non_thread_safe(typed, receiver_type) {
                    Ok(Some(offending)) => {
                        let message = format!(
                            "'{claim}' is verified recursively: field '{offending}' transitively contains a value that is not {claim}-safe"
                        );
                        let origin = node_origin(resolved, &item.node)
                            .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                        errors.push(match origin {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                message,
                                origin,
                            ),
                            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
                        });
                        continue;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                }
            }
            // §4.1 recursive `clone` verification: `clone` gets a structural
            // default only when EVERY field supports clone, so a `clone` claim is
            // unsound if any TRANSITIVELY nested field is a unique handle
            // (channel/receiver/eventual) or a `fin` value (whose structural
            // clone would fail to emit). Recurse here where every type is lowered.
            if typed
                .capability_claims(type_symbol_id)
                .is_some_and(|caps| caps.contains("clone"))
            {
                // The claiming type must not ITSELF be a non-clonable value. A
                // `fin` value in particular has no structural clone: `fin + clone`
                // requires a custom clone (§4.1) that FOL does not yet implement,
                // so structurally cloning it (double-finalizing the resource) is
                // rejected.
                match type_is_known_non_clone(typed, receiver_type) {
                    Ok(true) => {
                        let message =
                            "a type that claims 'clone' cannot itself be a non-clonable value; a 'fin' value needs a custom clone (not the structural default), which is not yet supported".to_string();
                        let origin = node_origin(resolved, &item.node)
                            .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                        errors.push(match origin {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                message,
                                origin,
                            ),
                            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
                        });
                        continue;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                }
                match first_field_transitively_non_clone(typed, receiver_type) {
                    Ok(Some(offending)) => {
                        let message = format!(
                            "'clone' is verified recursively: field '{offending}' transitively contains a value that cannot be cloned"
                        );
                        let origin = node_origin(resolved, &item.node)
                            .or_else(|| resolved.syntax_index().origin(item.node_id).cloned());
                        errors.push(match origin {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                message,
                                origin,
                            ),
                            None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
                        });
                        continue;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                }
            }
            let Some(conformance) = typed.typed_conformance(type_symbol_id).cloned() else {
                errors.push(internal_error(
                    format!("typed conformance metadata disappeared for type '{}'", name),
                    node_origin(resolved, &item.node)
                        .or_else(|| resolved.syntax_index().origin(item.node_id).cloned()),
                ));
                continue;
            };
            for claim in &conformance.claims {
                let standard_symbol_id = claim.standard_symbol_id;
                let Some(standard) = typed.typed_standard(standard_symbol_id).cloned() else {
                    let standard_name = resolved
                        .symbol(standard_symbol_id)
                        .map(|symbol| symbol.name.clone())
                        .unwrap_or_else(|| format!("#{}", standard_symbol_id.0));
                    errors.push(match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                        Some(origin) => TypecheckError::with_origin(
                            TypecheckErrorKind::Unsupported,
                            format!(
                                "type '{}' claims standard '{}' whose kind is not part of the shipped V2 contract",
                                name, standard_name
                            ),
                            origin,
                        ),
                        None => TypecheckError::new(
                            TypecheckErrorKind::Unsupported,
                            format!(
                                "type '{}' claims standard '{}' whose kind is not part of the shipped V2 contract",
                                name, standard_name
                            ),
                        ),
                    });
                    continue;
                };

                // Build the generic-parameter substitution table for this
                // claim. For non-generic standards the map is empty and
                // substitution is a no-op; for generic standards the
                // table binds each parameter to the type arg supplied in
                // the conformance header.
                let bindings: BTreeMap<SymbolId, CheckedTypeId> = if standard
                    .generic_params
                    .is_empty()
                {
                    BTreeMap::new()
                } else {
                    if claim.type_args.len() != standard.generic_params.len() {
                        let standard_name = resolved
                            .symbol(standard_symbol_id)
                            .map(|symbol| symbol.name.clone())
                            .unwrap_or_else(|| format!("#{}", standard_symbol_id.0));
                        errors.push(match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                format!(
                                    "type '{}' claims generic standard '{}' with {} type argument(s) but the standard expects {}",
                                    name,
                                    standard_name,
                                    claim.type_args.len(),
                                    standard.generic_params.len(),
                                ),
                                origin,
                            ),
                            None => TypecheckError::new(
                                TypecheckErrorKind::InvalidInput,
                                format!(
                                    "type '{}' claims generic standard '{}' with {} type argument(s) but the standard expects {}",
                                    name,
                                    standard_name,
                                    claim.type_args.len(),
                                    standard.generic_params.len(),
                                ),
                            ),
                        });
                        continue;
                    }
                    standard
                        .generic_params
                        .iter()
                        .copied()
                        .zip(claim.type_args.iter().copied())
                        .collect()
                };

                // Substituted requirements: each routine/field has its
                // declared types rewritten through `bindings`. For a
                // non-generic standard this is a cheap clone.
                let substituted_routines: Vec<TypedStandardRoutine> = standard
                    .required_routines
                    .iter()
                    .map(|req| {
                        let params = req
                            .params
                            .iter()
                            .map(|type_id| {
                                substitute_generic_checked_type(typed, *type_id, &bindings, None)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        let return_type = req
                            .return_type
                            .map(|type_id| {
                                substitute_generic_checked_type(typed, type_id, &bindings, None)
                            })
                            .transpose()?;
                        let error_type = req
                            .error_type
                            .map(|type_id| {
                                substitute_generic_checked_type(typed, type_id, &bindings, None)
                            })
                            .transpose()?;
                        Ok::<_, TypecheckError>(TypedStandardRoutine {
                            symbol_id: req.symbol_id,
                            name: req.name.clone(),
                            params,
                            return_type,
                            error_type,
                            has_default_body: req.has_default_body,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap_or_default();
                let substituted_fields: Vec<TypedStandardField> = standard
                    .required_fields
                    .iter()
                    .map(|req| {
                        let field_type = substitute_generic_checked_type(
                            typed,
                            req.field_type,
                            &bindings,
                            None,
                        )?;
                        Ok::<_, TypecheckError>(TypedStandardField {
                            symbol_id: req.symbol_id,
                            name: req.name.clone(),
                            field_type,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap_or_default();

                for requirement in &substituted_routines {
                    let candidates = typed
                        .all_typed_symbols()
                        .filter(|symbol| {
                            // A standard requirement is satisfied by a method on
                            // the conforming type regardless of its receiver
                            // ownership: `fun (T[bor])size()` and
                            // `pro (T[mut, bor])clear()` are the canonical V3
                            // method forms and must count alongside `(T)`.
                            let receiver_matches =
                                symbol.receiver_type.is_some_and(|declared_receiver| {
                                    declared_receiver == receiver_type
                                        || matches!(
                                            typed.type_table().get(declared_receiver),
                                            Some(CheckedType::Borrowed { inner, .. })
                                                if *inner == receiver_type
                                        )
                                });
                            symbol.kind == SymbolKind::Routine
                                && receiver_matches
                                && resolved.symbol(symbol.symbol_id).is_some_and(
                                    |resolved_symbol| resolved_symbol.name == requirement.name,
                                )
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
                    // When the standard ships a default body, missing the
                    // routine on the conformer is fine — the default is
                    // inherited. Multiple matches still fail as ambiguous,
                    // and signature mismatches still fail as incompatible.
                    if candidates.is_empty() && requirement.has_default_body {
                        continue;
                    }

                    let standard_name = resolved
                        .symbol(standard.symbol_id)
                        .map(|symbol| symbol.name.clone())
                        .unwrap_or_else(|| format!("#{}", standard.symbol_id.0));
                    let expected_signature = render_standard_signature(
                        typed,
                        &requirement.name,
                        &requirement.params,
                        requirement.return_type,
                        requirement.error_type,
                    );
                    let mut error = if exact_matches.len() > 1 {
                        match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::InvalidInput,
                                format!(
                                    "type '{}' satisfies standard '{}' ambiguously: multiple routines match required routine '{}'; expected exactly one routine with signature {}",
                                    name, standard_name, requirement.name, expected_signature
                                ),
                                origin,
                            ),
                            None => TypecheckError::new(
                                TypecheckErrorKind::InvalidInput,
                                format!(
                                    "type '{}' satisfies standard '{}' ambiguously: multiple routines match required routine '{}'; expected exactly one routine with signature {}",
                                    name, standard_name, requirement.name, expected_signature
                                ),
                            ),
                        }
                    } else if candidates.is_empty() {
                        match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy standard '{}': missing required routine '{}'; expected {}",
                                    name, standard_name, requirement.name, expected_signature
                                ),
                                origin,
                            ),
                            None => TypecheckError::new(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy standard '{}': missing required routine '{}'; expected {}",
                                    name, standard_name, requirement.name, expected_signature
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
                        match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy standard '{}': routine '{}' has incompatible signature; expected {}, found {}",
                                    name,
                                    standard_name,
                                    requirement.name,
                                    expected_signature,
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
                                    expected_signature,
                                    actual_signatures,
                                ),
                            ),
                        }
                    };
                    if let Some(origin) = resolved
                        .symbol(requirement.symbol_id)
                        .and_then(|symbol| symbol.origin.clone())
                    {
                        error =
                            error.with_related_origin(origin, "required by this standard routine");
                    }
                    for (symbol_id, _) in exact_matches {
                        if let Some(origin) = resolved
                            .symbol(*symbol_id)
                            .and_then(|symbol| symbol.origin.clone())
                        {
                            error = error.with_related_origin(
                                origin,
                                "matching routine contributing to ambiguity",
                            );
                        }
                    }
                    errors.push(error);
                }

                // Blueprint field checks: walk each blueprint requirement
                // and match it against the conformer's declared record
                // fields. The checks are purely structural — a matching
                // name with a compatible type is enough.
                // The conformer receiver type is a `Declared { kind: Type }`
                // wrapper — resolve it through the typed symbol to the
                // underlying record definition.
                let conformer_record_fields = typed
                    .typed_symbol(type_symbol_id)
                    .and_then(|typed_symbol| typed_symbol.declared_type)
                    .and_then(|type_id| typed.type_table().get(type_id))
                    .and_then(|checked| match checked {
                        CheckedType::Record { fields } => Some(fields.clone()),
                        _ => None,
                    });
                for requirement in &substituted_fields {
                    let standard_name = resolved
                        .symbol(standard.symbol_id)
                        .map(|symbol| symbol.name.clone())
                        .unwrap_or_else(|| format!("#{}", standard.symbol_id.0));
                    let expected_type_name = typed.type_table().render_type(requirement.field_type);
                    let Some(fields) = conformer_record_fields.as_ref() else {
                        errors.push(match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                            Some(origin) => TypecheckError::with_origin(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy blueprint standard '{}': it is not a record type and cannot carry the required field '{}: {}'",
                                    name, standard_name, requirement.name, expected_type_name
                                ),
                                origin,
                            ),
                            None => TypecheckError::new(
                                TypecheckErrorKind::IncompatibleType,
                                format!(
                                    "type '{}' does not satisfy blueprint standard '{}': it is not a record type and cannot carry the required field '{}: {}'",
                                    name, standard_name, requirement.name, expected_type_name
                                ),
                            ),
                        });
                        continue;
                    };
                    match fields.get(&requirement.name) {
                        None => {
                            errors.push(match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                                Some(origin) => TypecheckError::with_origin(
                                    TypecheckErrorKind::IncompatibleType,
                                    format!(
                                        "type '{}' does not satisfy blueprint standard '{}': missing required field '{}: {}'",
                                        name, standard_name, requirement.name, expected_type_name
                                    ),
                                    origin,
                                ),
                                None => TypecheckError::new(
                                    TypecheckErrorKind::IncompatibleType,
                                    format!(
                                        "type '{}' does not satisfy blueprint standard '{}': missing required field '{}: {}'",
                                        name, standard_name, requirement.name, expected_type_name
                                    ),
                                ),
                            });
                        }
                        Some(actual_type) if *actual_type != requirement.field_type => {
                            let actual_type_name = typed.type_table().render_type(*actual_type);
                            errors.push(match node_origin(resolved, &item.node).or_else(|| resolved.syntax_index().origin(item.node_id).cloned()) {
                                Some(origin) => TypecheckError::with_origin(
                                    TypecheckErrorKind::IncompatibleType,
                                    format!(
                                        "type '{}' does not satisfy blueprint standard '{}': field '{}' has incompatible type; expected {}, found {}",
                                        name, standard_name, requirement.name, expected_type_name, actual_type_name
                                    ),
                                    origin,
                                ),
                                None => TypecheckError::new(
                                    TypecheckErrorKind::IncompatibleType,
                                    format!(
                                        "type '{}' does not satisfy blueprint standard '{}': field '{}' has incompatible type; expected {}, found {}",
                                        name, standard_name, requirement.name, expected_type_name, actual_type_name
                                    ),
                                ),
                            });
                        }
                        Some(_) => {}
                    }
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

pub(crate) fn validate_generic_bindings_against_constraints(
    typed: &TypedProgram,
    bindings: &BTreeMap<SymbolId, CheckedTypeId>,
    generic_constraints: &BTreeMap<SymbolId, Vec<GenericConstraint>>,
    surface: String,
    origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    for (generic_symbol_id, constraints) in generic_constraints {
        let Some(actual_type) = bindings.get(generic_symbol_id).copied() else {
            continue;
        };
        for constraint in constraints {
            if checked_type_satisfies_standard(
                typed,
                actual_type,
                constraint.standard,
                &constraint.args,
            ) {
                continue;
            }
            let generic_name = typed
                .resolved()
                .symbol(*generic_symbol_id)
                .map(|symbol| symbol.name.as_str())
                .unwrap_or("T");
            let standard_name = typed
                .resolved()
                .symbol(constraint.standard)
                .map(|symbol| symbol.name.as_str())
                .unwrap_or("standard");
            let actual_name = typed.type_table().render_type(actual_type);
            let message = format!(
                "{surface} requires type '{actual_name}' to satisfy standard '{standard_name}' for generic parameter '{generic_name}'; add an explicit conformance header for '{standard_name}' on '{actual_name}' and implement the required routines"
            );
            return Err(match origin.clone() {
                Some(origin) => TypecheckError::with_origin(
                    TypecheckErrorKind::IncompatibleType,
                    message,
                    origin,
                ),
                None => TypecheckError::new(TypecheckErrorKind::IncompatibleType, message),
            });
        }
    }

    // Compiler-owned capability bounds (`T: copy`/`clone`/`send`/`share`/`fin`)
    // are conditional obligations: the actual type bound to `T` must have the
    // capability (V3_MEM §4.1).
    for (generic_symbol_id, actual_type) in bindings {
        let Some(capabilities) = typed.generic_capability_constraints(*generic_symbol_id) else {
            continue;
        };
        for capability in capabilities {
            if type_satisfies_capability(typed, *actual_type, capability)? {
                continue;
            }
            let generic_name = typed
                .resolved()
                .symbol(*generic_symbol_id)
                .map(|symbol| symbol.name.as_str())
                .unwrap_or("T");
            let actual_name = typed.type_table().render_type(*actual_type);
            let message = format!(
                "{surface} requires type '{actual_name}' to satisfy the '{capability}' capability for generic parameter '{generic_name}'; the type does not"
            );
            return Err(match origin.clone() {
                Some(origin) => TypecheckError::with_origin(
                    TypecheckErrorKind::IncompatibleType,
                    message,
                    origin,
                ),
                None => TypecheckError::new(TypecheckErrorKind::IncompatibleType, message),
            });
        }
    }

    Ok(())
}

/// Whether `type_id` has the compiler-owned `capability` (V3_MEM §4.1). Uses the
/// same conservative predicates as operand/claim conformance: only a type that
/// definitively lacks the capability fails.
fn type_satisfies_capability(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    capability: &str,
) -> Result<bool, TypecheckError> {
    // Generic capability bounds are verified at call sites, after every type is
    // lowered, so the `clone`/`send`/`share` checks recurse transitively through
    // nested aggregate fields (V3_MEM §4.1) — mirroring the recursive claim
    // verification. (`copy` stays structural here, matching the still-structural
    // `[cpy]` operand check; requiring the declared claim is a later step.)
    Ok(match capability {
        "copy" => !type_lacks_copy(typed, type_id)?,
        "clone" => {
            !type_is_known_non_clone(typed, type_id)?
                && first_field_transitively_non_clone(typed, type_id)?.is_none()
        }
        "send" | "share" => {
            !type_is_known_non_thread_safe(typed, type_id)?
                && first_field_transitively_non_thread_safe(typed, type_id)?.is_none()
        }
        "fin" => {
            let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
            typed.type_resolves_to_fin(apparent)
        }
        _ => true,
    })
}

fn checked_type_satisfies_standard(
    typed: &TypedProgram,
    checked_type_id: CheckedTypeId,
    standard_symbol_id: SymbolId,
    standard_args: &[CheckedTypeId],
) -> bool {
    // A generic parameter satisfies a standard when its own declared bound
    // already includes that standard at the same arguments: passing `T`
    // (bound by `geo`) into a `Box[T: geo]` slot is legal without a fresh
    // conformance header, and `T: Holder[int]` satisfies a `Holder[int]` slot.
    if let Some(CheckedType::Declared {
        symbol,
        kind: DeclaredTypeKind::GenericParameter,
        ..
    }) = typed.type_table().get(checked_type_id)
    {
        return typed
            .typed_symbol(*symbol)
            .and_then(|param_symbol| param_symbol.generic_constraints.get(symbol))
            .is_some_and(|constraints| {
                constraints.iter().any(|constraint| {
                    constraint.standard == standard_symbol_id && constraint.args == standard_args
                })
            });
    }

    let Some(type_symbol_id) = conformance_subject_symbol(typed, checked_type_id) else {
        return false;
    };
    // The conformer must claim this exact standard at these exact arguments:
    // a type claiming `Holder[str]` does not satisfy a `Holder[int]` bound.
    typed
        .typed_conformance(type_symbol_id)
        .is_some_and(|conformance| {
            conformance.claims.iter().any(|claim| {
                claim.standard_symbol_id == standard_symbol_id && claim.type_args == standard_args
            })
        })
}

pub(crate) fn conformance_subject_symbol(
    typed: &TypedProgram,
    checked_type_id: CheckedTypeId,
) -> Option<SymbolId> {
    match typed.type_table().get(checked_type_id)? {
        CheckedType::Declared {
            symbol,
            kind: DeclaredTypeKind::Type,
            ..
        } => Some(*symbol),
        CheckedType::Declared {
            symbol,
            kind: DeclaredTypeKind::Alias,
            ..
        } => typed
            .typed_symbol(*symbol)
            .and_then(|typed_symbol| typed_symbol.declared_type)
            .and_then(|target| conformance_subject_symbol(typed, target)),
        _ => None,
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
) -> Result<LoweredRoutineGenerics, TypecheckError> {
    let mut generic_params = Vec::new();
    let mut generic_constraints = BTreeMap::new();
    for generic in generics {
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
            args: Vec::new(),
        });
        record_symbol_type(typed, symbol_id, generic_type)?;
        // Record compiler-owned capability bounds (`T: copy`/`clone`/`send`/
        // `share`/`fin`) so call sites can verify the actual type (V3_MEM §4.1).
        for constraint in &generic.constraints {
            if let FolType::Named { name, .. } = constraint {
                let base = name.split('[').next().unwrap_or(name);
                if fol_parser::ast::is_compiler_owned_generic_constraint(base)
                    && fol_parser::ast::is_capability_standard(base)
                {
                    typed.record_generic_capability_constraint(symbol_id, base.to_string());
                }
            }
        }
        let lowered_constraints = generic
            .constraints
            .iter()
            // Compiler-owned capability standards carry no standard symbol.
            .filter(|constraint| {
                !matches!(constraint, FolType::Named { name, .. }
                    if fol_parser::ast::is_compiler_owned_generic_constraint(
                        name.split('[').next().unwrap_or(name)))
            })
            .map(|constraint| {
                lower_standard_constraint_for_contract(typed, resolved, signature_scope, constraint)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !lowered_constraints.is_empty() {
            // Record the parameter's own bound on its own typed symbol (keyed by
            // itself) so a later constraint-satisfaction check can answer "does
            // generic parameter T carry standard geo?" directly. This must land
            // before the routine's parameter types are lowered, since an
            // instantiation like `Box[T]` in the parameter list is validated
            // immediately.
            if let Some(param_symbol) = typed.typed_symbol_mut(symbol_id) {
                param_symbol
                    .generic_constraints
                    .insert(symbol_id, lowered_constraints.clone());
            }
            generic_constraints.insert(symbol_id, lowered_constraints);
        }
        generic_params.push(symbol_id);
    }

    Ok((generic_params, generic_constraints))
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
        // A generic instantiation like `Box[T]` mentions a generic parameter
        // when any of its type arguments does.
        Some(CheckedType::Declared { args, .. }) => args
            .iter()
            .any(|arg| checked_type_contains_generic_param(typed, *arg)),
        Some(CheckedType::Array { element_type, .. })
        | Some(CheckedType::Vector { element_type })
        | Some(CheckedType::Sequence { element_type })
        | Some(CheckedType::Channel { element_type })
        | Some(CheckedType::ChannelSender { element_type })
        | Some(CheckedType::Optional {
            inner: element_type,
        })
        | Some(CheckedType::Owned {
            inner: element_type,
        })
        | Some(CheckedType::Borrowed {
            inner: element_type,
            ..
        })
        | Some(CheckedType::Pointer {
            target: element_type,
            ..
        }) => checked_type_contains_generic_param(typed, *element_type),
        Some(CheckedType::Eventual {
            value_type,
            error_type,
        }) => {
            checked_type_contains_generic_param(typed, *value_type)
                || error_type.is_some_and(|error| checked_type_contains_generic_param(typed, error))
        }
        Some(CheckedType::Error { inner }) => {
            inner.is_some_and(|inner| checked_type_contains_generic_param(typed, inner))
        }
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
    let type_id = lower_type_inner(typed, resolved, scope_id, typ)?;
    crate::exprs::helpers::reject_embedded_full_channel(
        typed,
        type_id,
        type_origin(resolved, typ),
    )?;
    Ok(type_id)
}

fn lower_type_inner(
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
            reject_heap_backed_type_in_core(typed, resolved, typ, "str", None)?;
            Ok(typed.builtin_types().str_)
        }
        FolType::Never => Ok(typed.builtin_types().never),
        FolType::Named { name, syntax_id } => {
            if name == "evt" {
                return Err(TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    "an eventual type needs a value type, e.g. 'evt[T]' or 'evt[T / E]'",
                ));
            }
            if let Some(instantiated) =
                parse_instantiated_type_args(name, type_origin(resolved, typ))?
            {
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
                resolved_symbol_for_syntax(resolved, *syntax_id, name, SymbolReferenceShape::Named)
                    .or_else(|_| {
                        resolve_declared_symbol_by_text(
                            resolved,
                            scope_id,
                            name,
                            type_origin(resolved, typ),
                        )
                    })?
            } else {
                resolve_declared_symbol_by_text(
                    resolved,
                    scope_id,
                    name,
                    type_origin(resolved, typ),
                )?
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
            reject_heap_backed_type_in_core(typed, resolved, typ, "vec[...]", None)?;
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Vector { element_type }))
        }
        FolType::Sequence { element_type } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "seq[...]", None)?;
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Sequence { element_type }))
        }
        FolType::Channel { element_type } => {
            if !typed.capability_model().supports_processor() {
                return Err(match type_origin(resolved, typ) {
                    Some(origin) => TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        "channel types require hosted std support; declare the bundled internal standard dependency",
                        origin,
                    ),
                    None => TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "channel types require hosted std support; declare the bundled internal standard dependency",
                    ),
                });
            }
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Channel { element_type }))
        }
        FolType::Mutex { inner } => {
            // `mux[T]` marks a mutex-guarded parameter (V3_MEM §8.3). A `mux[T]`
            // parameter is desugared to `is_mutex` + inner `T` at parse time, so
            // this arm only sees `mux[T]` in other positions; it lowers to the
            // guarded inner type and, like channels, requires hosted std.
            if !typed.capability_model().supports_processor() {
                return Err(match type_origin(resolved, typ) {
                    Some(origin) => TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        "mutex parameters require hosted std support; declare the bundled internal standard dependency",
                        origin,
                    ),
                    None => TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "mutex parameters require hosted std support; declare the bundled internal standard dependency",
                    ),
                });
            }
            lower_type(typed, resolved, scope_id, inner)
        }
        FolType::ChannelSender { element_type } => {
            // `chn[tx, T]` names a first-class, clone-capable sender endpoint
            // value (V3_MEM §8.2). Like channels, it requires hosted std.
            if !typed.capability_model().supports_processor() {
                return Err(match type_origin(resolved, typ) {
                    Some(origin) => TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        "channel types require hosted std support; declare the bundled internal standard dependency",
                        origin,
                    ),
                    None => TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "channel types require hosted std support; declare the bundled internal standard dependency",
                    ),
                });
            }
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::ChannelSender { element_type }))
        }
        FolType::ChannelReceiver { element_type } => {
            // `chn[rx, T]` names a first-class, move-only unique receiver
            // endpoint value (V3_MEM §8.2). Like channels, it requires hosted std.
            if !typed.capability_model().supports_processor() {
                return Err(match type_origin(resolved, typ) {
                    Some(origin) => TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        "channel types require hosted std support; declare the bundled internal standard dependency",
                        origin,
                    ),
                    None => TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "channel types require hosted std support; declare the bundled internal standard dependency",
                    ),
                });
            }
            let element_type = lower_type(typed, resolved, scope_id, element_type)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::ChannelReceiver { element_type }))
        }
        FolType::Owned { inner } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "owned heap type '@T'", None)?;
            let inner = lower_type(typed, resolved, scope_id, inner)?;
            Ok(typed.type_table_mut().intern(CheckedType::Owned { inner }))
        }
        FolType::Borrowed { inner, mutable, .. } => {
            // A borrowed type annotation `T[bor]` / `T[mut, bor]` / `T[bor=L]`
            // lowers to a borrow of the inner type. Named lifetimes are parsed
            // but not yet region-checked (later Slice C work).
            let inner = lower_type(typed, resolved, scope_id, inner)?;
            Ok(typed.type_table_mut().intern(CheckedType::Borrowed {
                inner,
                mutable: *mutable,
            }))
        }
        FolType::Pointer { qualifier, target } => {
            if matches!(qualifier, fol_parser::ast::PointerQualifier::Raw) {
                return Err(match type_origin(resolved, typ) {
                    Some(origin) => TypecheckError::with_origin(
                        TypecheckErrorKind::Unsupported,
                        "raw pointers are a V4 interop surface",
                        origin,
                    ),
                    None => TypecheckError::new(
                        TypecheckErrorKind::Unsupported,
                        "raw pointers are a V4 interop surface",
                    ),
                });
            }
            let weak = matches!(qualifier, fol_parser::ast::PointerQualifier::Weak);
            let sync = matches!(qualifier, fol_parser::ast::PointerQualifier::SharedSync);
            let shared = sync || matches!(qualifier, fol_parser::ast::PointerQualifier::Shared);
            let target = lower_type(typed, resolved, scope_id, target)?;
            Ok(typed.type_table_mut().intern(CheckedType::Pointer {
                target,
                weak,
                shared,
                sync,
            }))
        }
        FolType::Set { types } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "set[...]", None)?;
            let mut member_types = Vec::new();
            for member in types {
                member_types.push(lower_type(typed, resolved, scope_id, member)?);
            }
            for member in &member_types {
                if checked_type_blocks_ordering(
                    typed,
                    *member,
                    &mut std::collections::BTreeSet::new(),
                ) {
                    return Err(unorderable_container_member_error(
                        "a set member",
                        type_origin(resolved, typ),
                    ));
                }
            }
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Set { member_types }))
        }
        FolType::Map {
            key_type,
            value_type,
        } => {
            reject_heap_backed_type_in_core(typed, resolved, typ, "map[...]", None)?;
            let key_type = lower_type(typed, resolved, scope_id, key_type)?;
            let value_type = lower_type(typed, resolved, scope_id, value_type)?;
            // A `map[K, V]` is `BTreeMap`-backed, so only the KEY must be `Ord`.
            if checked_type_blocks_ordering(typed, key_type, &mut std::collections::BTreeSet::new())
            {
                return Err(unorderable_container_member_error(
                    "a map key",
                    type_origin(resolved, typ),
                ));
            }
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
        FolType::Eventual {
            value_type,
            error_type,
            // The public lifetime `L` from `evt[L, T]` names the parent scope
            // for public APIs (V3_MEM §8.1); like `[bor=L]`, it is accepted but
            // not yet region-checked, so it does not affect the checked type.
            ..
        } => {
            // Namable eventual: `evt[T]`, `evt[T / E]`, `evt[L, T]`, and
            // `evt[L, T / E]` all denote the same one-shot eventual that a
            // `| async` stage produces.
            let value_type = lower_type(typed, resolved, scope_id, value_type)?;
            let error_type = error_type
                .as_ref()
                .map(|error_type| lower_type(typed, resolved, scope_id, error_type))
                .transpose()?;
            Ok(typed.type_table_mut().intern(CheckedType::Eventual {
                value_type,
                error_type,
            }))
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
            env_lifetime,
        } => {
            // §5.3: the environment lifetime must name a declared generic
            // parameter (`L: lif`) reachable from this signature scope.
            if let Some(lifetime) = env_lifetime {
                let source_unit_id = resolved
                    .scope(scope_id)
                    .and_then(|scope| scope.source_unit)
                    .unwrap_or(SourceUnitId(0));
                if crate::exprs::helpers::find_symbol_in_scope_chain(
                    resolved,
                    source_unit_id,
                    scope_id,
                    lifetime,
                    SymbolKind::GenericParameter,
                )
                .is_none()
                {
                    return Err(TypecheckError::new(
                        TypecheckErrorKind::Ownership,
                        format!(
                            "routine type environment lifetime '{lifetime}' is not a declared lifetime parameter; declare '{lifetime}: lif' in the routine's generic list"
                        ),
                    ));
                }
            }
            let lowered_params = params
                .iter()
                .map(|p| lower_type(typed, resolved, scope_id, p))
                .collect::<Result<Vec<_>, _>>()?;
            let lowered_return = match return_type.as_ref() {
                FolType::None => None,
                return_type => Some(lower_type(typed, resolved, scope_id, return_type)?),
            };
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Routine(crate::types::RoutineType {
                    generic_params: Vec::new(),
                    generic_constraints: BTreeMap::new(),
                    param_names: vec![String::new(); lowered_params.len()],
                    param_defaults: vec![None; lowered_params.len()],
                    variadic_index: None,
                    mutex_params: Default::default(),
                    params: lowered_params,
                    return_type: lowered_return,
                    error_type: None,
                    env_lifetime: env_lifetime.is_some(),
                })))
        }
        unsupported => Err(unsupported_type_error(
            resolved,
            unsupported,
            typed.capability_model(),
        )),
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
                    format!(
                        "could not parse generic type argument '{arg}': {}",
                        diagnostic.message
                    ),
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
            current_scope = resolved
                .namespace_scope(&current_namespace)
                .ok_or_else(|| {
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
                    format!(
                        "generic type argument '{display_name}' is ambiguous in the current scope"
                    ),
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
                    format!(
                        "generic type argument '{full_path}' is ambiguous in the current scope"
                    ),
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
        internal_error(
            "instantiated type symbol disappeared during type lowering",
            origin.clone(),
        )
    })?;
    if typed_symbol.generic_params.is_empty() {
        return lower_declared_symbol(typed.type_table_mut(), resolved, symbol_id);
    }
    let generic_params = typed_symbol.generic_params.clone();
    if generic_params.len() != arg_types.len() {
        return Err(invalid_input_error(
            format!(
                "generic type '{}' expects {} type argument(s) but got {}",
                resolved
                    .symbol(symbol_id)
                    .map(|symbol| symbol.name.as_str())
                    .unwrap_or("?"),
                generic_params.len(),
                arg_types.len()
            ),
            origin,
        ));
    }
    let generic_constraints = typed_symbol.generic_constraints.clone();
    // The template (the declaration's structural body) may not be recorded yet
    // when a generic type references itself inside its own body
    // (`typ Node(T): rec = { next: Node[T] }`): its `declared_type` lands only
    // after the body is fully lowered.
    let template = typed_symbol.declared_type;

    // Nominal identity: intern the instantiation as a `Declared` node carrying
    // its type arguments, so `Box[int]` and `Cup[int]` are distinct types
    // (different `symbol`) and `Box[int]`/`Box[str]` differ (different `args`).
    let display_name = resolved
        .symbol(symbol_id)
        .map(|symbol| symbol.name.clone())
        .unwrap_or_else(|| "?".to_string());
    let kind = match resolved.symbol(symbol_id).map(|symbol| symbol.kind) {
        Some(SymbolKind::Alias) => DeclaredTypeKind::Alias,
        _ => DeclaredTypeKind::Type,
    };
    let instance = typed.type_table_mut().intern(CheckedType::Declared {
        symbol: symbol_id,
        name: display_name,
        kind,
        args: arg_types.to_vec(),
    });

    // No template yet (self-reference while the type's own body is still being
    // lowered), or this instance is already being expanded higher on the stack
    // (a recursive instantiation): the interned nominal node is the answer. Its
    // concrete structural shape is filled in when the type is instantiated at a
    // real use site, and the cycle guard keeps that expansion finite.
    let Some(template) = template else {
        return Ok(instance);
    };
    if !typed.begin_instantiation(instance) {
        return Ok(instance);
    }

    let bindings = generic_params
        .iter()
        .copied()
        .zip(arg_types.iter().copied())
        .collect::<BTreeMap<_, _>>();
    let result = (|| {
        validate_generic_bindings_against_constraints(
            typed,
            &bindings,
            &generic_constraints,
            format!(
                "generic type instantiation '{}'",
                resolved
                    .symbol(symbol_id)
                    .map(|symbol| symbol.name.as_str())
                    .unwrap_or("?")
            ),
            origin.clone(),
        )?;
        // The substituted structural shape is computed once and registered as
        // the node's apparent type, so every consumer that resolves through
        // `apparent_type_id` (field access, record-literal checking,
        // assignability, dispatch) transparently sees the concrete record/entry.
        substitute_generic_checked_type(typed, template, &bindings, origin.clone())
    })();
    typed.end_instantiation(instance);
    let structural = result?;
    if instance != structural {
        typed.record_apparent_type_override(instance, structural);
    }
    Ok(instance)
}

/// Reject value-recursive types while allowing recursion guarded by owned heap
/// indirection (`@T`). A bare self-reference has infinite size; `@T` lowers to
/// a finite `Box<T>` edge and is therefore legal.
fn reject_recursive_type_definition(
    typed: &TypedProgram,
    symbol_id: SymbolId,
    type_id: CheckedTypeId,
    origin: Option<SyntaxOrigin>,
    resolved: &ResolvedProgram,
) -> Result<(), TypecheckError> {
    let mut visited = std::collections::BTreeSet::new();
    if !checked_type_references_symbol(typed, type_id, symbol_id, &mut visited) {
        return Ok(());
    }
    let name = resolved
        .symbol(symbol_id)
        .map(|symbol| symbol.name.clone())
        .unwrap_or_else(|| "?".to_string());
    let message = format!(
        "recursive value type '{name}' has no finite runtime shape; guard the recursive edge \
         with owned heap indirection such as 'opt @{name}'"
    );
    Err(match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
    })
}

/// Walk a checked type's structure looking for a reference back to `target`.
/// Follows record fields, entry variants, container element/member/key/value
/// types, optional/error inner types, generic arguments, and — for a nested
/// declared type — that type's own structural body (for mutual recursion). The
/// `visited` set keeps the walk finite.
fn checked_type_references_symbol(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    target: SymbolId,
    visited: &mut std::collections::BTreeSet<CheckedTypeId>,
) -> bool {
    if !visited.insert(type_id) {
        return false;
    }
    let Some(checked) = typed.type_table().get(type_id).cloned() else {
        return false;
    };
    let refers = |inner: CheckedTypeId, visited: &mut std::collections::BTreeSet<CheckedTypeId>| {
        checked_type_references_symbol(typed, inner, target, visited)
    };
    match checked {
        CheckedType::Declared {
            symbol, kind, args, ..
        } => {
            if matches!(kind, DeclaredTypeKind::Type | DeclaredTypeKind::Alias) && symbol == target
            {
                return true;
            }
            if args.iter().any(|arg| refers(*arg, visited)) {
                return true;
            }
            // Descend into the referenced type's structural body (mutual
            // recursion), preferring the concrete apparent shape when present.
            let inner = typed
                .apparent_type_override(type_id)
                .or_else(|| typed.typed_symbol(symbol).and_then(|s| s.declared_type));
            inner.is_some_and(|inner| refers(inner, visited))
        }
        CheckedType::Record { fields } => fields.values().any(|field| refers(*field, visited)),
        CheckedType::Entry { variants } => variants
            .values()
            .filter_map(|variant| *variant)
            .any(|variant| refers(variant, visited)),
        CheckedType::Array { element_type, .. }
        | CheckedType::Vector { element_type }
        | CheckedType::Sequence { element_type } => refers(element_type, visited),
        // A channel stores transport handles rather than embedding a message
        // value inline, so it also breaks recursive value layout.
        CheckedType::Channel { .. }
        | CheckedType::ChannelSender { .. }
        | CheckedType::ChannelReceiver { .. }
        | CheckedType::Eventual { .. } => false,
        CheckedType::Set { member_types } => {
            member_types.iter().any(|member| refers(*member, visited))
        }
        CheckedType::Map {
            key_type,
            value_type,
        } => refers(key_type, visited) || refers(value_type, visited),
        CheckedType::Optional { inner } => refers(inner, visited),
        // Owned heap indirection gives recursion a finite runtime shape.
        CheckedType::Owned { .. } => false,
        // Unique/shared pointers are heap indirection too; their target is not
        // embedded inline in the enclosing value.
        CheckedType::Pointer { .. } => false,
        CheckedType::Borrowed { inner, .. } => refers(inner, visited),
        CheckedType::Error { inner } => inner.is_some_and(|inner| refers(inner, visited)),
        CheckedType::Builtin(_) | CheckedType::Routine(_) => false,
    }
}

/// Whether `type_id` cannot be a set member or map key because it transitively
/// contains a value with no Rust `Ord` — a float (`f64: !Ord`) or a weak pointer
/// (`Weak<T>: !Ord`). Set/map storage is `BTreeSet`/`BTreeMap`-backed, so such a
/// member/key typechecks but fails the emitted Rust build. The walk descends
/// through aggregates, nominal bodies, and non-weak pointers (`Rc<T>`/`Box<T>`
/// are `Ord` only when `T` is), with a `visited` cycle guard.
fn unorderable_container_member_error(role: &str, origin: Option<SyntaxOrigin>) -> TypecheckError {
    let message = format!(
        "{role} must be orderable, but this type has (or transitively contains) a \
         'flt' or 'ptr[weak, T]', which have no ordering; sets and maps are \
         'BTreeSet'/'BTreeMap'-backed and require orderable members/keys"
    );
    match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
    }
}

fn checked_type_blocks_ordering(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    visited: &mut std::collections::BTreeSet<CheckedTypeId>,
) -> bool {
    if !visited.insert(type_id) {
        return false;
    }
    let Some(checked) = typed.type_table().get(type_id).cloned() else {
        return false;
    };
    let blocks = |inner: CheckedTypeId, visited: &mut std::collections::BTreeSet<CheckedTypeId>| {
        checked_type_blocks_ordering(typed, inner, visited)
    };
    match checked {
        CheckedType::Builtin(crate::BuiltinType::Float) => true,
        CheckedType::Pointer { weak: true, .. } => true,
        CheckedType::Pointer { target, .. } => blocks(target, visited),
        CheckedType::Owned { inner }
        | CheckedType::Optional { inner }
        | CheckedType::Borrowed { inner, .. } => blocks(inner, visited),
        CheckedType::Array { element_type, .. }
        | CheckedType::Vector { element_type }
        | CheckedType::Sequence { element_type } => blocks(element_type, visited),
        CheckedType::Set { member_types } => {
            member_types.iter().any(|member| blocks(*member, visited))
        }
        CheckedType::Map {
            key_type,
            value_type,
        } => blocks(key_type, visited) || blocks(value_type, visited),
        CheckedType::Error { inner } => inner.is_some_and(|inner| blocks(inner, visited)),
        CheckedType::Record { fields } => fields.values().any(|field| blocks(*field, visited)),
        CheckedType::Entry { variants } => variants
            .values()
            .filter_map(|variant| *variant)
            .any(|variant| blocks(variant, visited)),
        CheckedType::Declared { symbol, args, .. } => {
            args.iter().any(|arg| blocks(*arg, visited)) || {
                let inner = typed
                    .apparent_type_override(type_id)
                    .or_else(|| typed.typed_symbol(symbol).and_then(|s| s.declared_type));
                inner.is_some_and(|inner| blocks(inner, visited))
            }
        }
        // Channels/eventuals/routines are not ordinary set members; leave them to
        // the emitted-Rust bound rather than over-rejecting here.
        CheckedType::Channel { .. }
        | CheckedType::ChannelSender { .. }
        | CheckedType::ChannelReceiver { .. }
        | CheckedType::Eventual { .. }
        | CheckedType::Builtin(_)
        | CheckedType::Routine(_) => false,
    }
}

pub(crate) fn substitute_generic_checked_type(
    typed: &mut TypedProgram,
    type_id: CheckedTypeId,
    bindings: &BTreeMap<SymbolId, CheckedTypeId>,
    origin: Option<SyntaxOrigin>,
) -> Result<CheckedTypeId, TypecheckError> {
    let checked = typed.type_table().get(type_id).cloned().ok_or_else(|| {
        internal_error(
            "generic type substitution lost a checked type",
            origin.clone(),
        )
    })?;
    match checked {
        CheckedType::Declared {
            symbol,
            kind: DeclaredTypeKind::GenericParameter,
            ..
        } => bindings.get(&symbol).copied().ok_or_else(|| {
            invalid_input_error(
                format!(
                    "generic type substitution left parameter '{}' unbound",
                    symbol.0
                ),
                origin.clone(),
            )
        }),
        // A generic instantiation (`Box[T]`, args non-empty) inside a
        // template must substitute its arguments and re-instantiate so the
        // apparent structural shape is recomputed for the concrete args.
        // Without this, an alias `typ MaybeBox(T): Box[T]` never turns `T`
        // into `int` when instantiated.
        CheckedType::Declared {
            symbol,
            name,
            kind,
            args,
        } if !args.is_empty() => {
            let substituted_args = args
                .iter()
                .map(|arg| substitute_generic_checked_type(typed, *arg, bindings, origin.clone()))
                .collect::<Result<Vec<_>, _>>()?;
            let template = typed.typed_symbol(symbol).and_then(|s| s.declared_type);
            let generic_params = typed
                .typed_symbol(symbol)
                .map(|s| s.generic_params.clone())
                .unwrap_or_default();
            let instance = typed.type_table_mut().intern(CheckedType::Declared {
                symbol,
                name,
                kind,
                args: substituted_args.clone(),
            });
            // Recompute the concrete structural shape unless this instance is
            // already being expanded higher on the stack — a recursive type
            // (`typ Node(T) = { next: Node[T] }`) reaches its own instantiation
            // while substituting its body, and the interned nominal node is the
            // fixpoint the recursion resolves to.
            if let Some(template) = template {
                if generic_params.len() == substituted_args.len()
                    && typed.begin_instantiation(instance)
                {
                    let inner: BTreeMap<SymbolId, CheckedTypeId> =
                        generic_params.into_iter().zip(substituted_args).collect();
                    let structural =
                        substitute_generic_checked_type(typed, template, &inner, origin.clone());
                    typed.end_instantiation(instance);
                    let structural = structural?;
                    if instance != structural {
                        typed.record_apparent_type_override(instance, structural);
                    }
                }
            }
            Ok(instance)
        }
        CheckedType::Declared { .. } | CheckedType::Builtin(_) => Ok(type_id),
        CheckedType::Array { element_type, size } => {
            let element_type =
                substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Array { element_type, size }))
        }
        CheckedType::Vector { element_type } => {
            let element_type =
                substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Vector { element_type }))
        }
        CheckedType::Sequence { element_type } => {
            let element_type =
                substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Sequence { element_type }))
        }
        CheckedType::Channel { element_type } => {
            let element_type =
                substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Channel { element_type }))
        }
        CheckedType::ChannelSender { element_type } => {
            let element_type =
                substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::ChannelSender { element_type }))
        }
        CheckedType::ChannelReceiver { element_type } => {
            let element_type =
                substitute_generic_checked_type(typed, element_type, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::ChannelReceiver { element_type }))
        }
        CheckedType::Eventual {
            value_type,
            error_type,
        } => {
            let value_type =
                substitute_generic_checked_type(typed, value_type, bindings, origin.clone())?;
            let error_type = error_type
                .map(|error_type| {
                    substitute_generic_checked_type(typed, error_type, bindings, origin.clone())
                })
                .transpose()?;
            Ok(typed.type_table_mut().intern(CheckedType::Eventual {
                value_type,
                error_type,
            }))
        }
        CheckedType::Set { member_types } => {
            let member_types = member_types
                .into_iter()
                .map(|member| {
                    substitute_generic_checked_type(typed, member, bindings, origin.clone())
                })
                .collect::<Result<Vec<_>, _>>()?;
            // A generic `set[T]` becomes concrete here; re-validate orderability
            // so `set[T]` instantiated with `flt`/`ptr[weak,_]` is rejected.
            for member in &member_types {
                if checked_type_blocks_ordering(
                    typed,
                    *member,
                    &mut std::collections::BTreeSet::new(),
                ) {
                    return Err(unorderable_container_member_error(
                        "a set member",
                        origin.clone(),
                    ));
                }
            }
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Set { member_types }))
        }
        CheckedType::Map {
            key_type,
            value_type,
        } => {
            let key_type =
                substitute_generic_checked_type(typed, key_type, bindings, origin.clone())?;
            let value_type =
                substitute_generic_checked_type(typed, value_type, bindings, origin.clone())?;
            // Only the key must be orderable (BTreeMap); re-check the concrete key.
            if checked_type_blocks_ordering(typed, key_type, &mut std::collections::BTreeSet::new())
            {
                return Err(unorderable_container_member_error("a map key", origin));
            }
            Ok(typed.type_table_mut().intern(CheckedType::Map {
                key_type,
                value_type,
            }))
        }
        CheckedType::Optional { inner } => {
            let inner = substitute_generic_checked_type(typed, inner, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Optional { inner }))
        }
        CheckedType::Owned { inner } => {
            let inner = substitute_generic_checked_type(typed, inner, bindings, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Owned { inner }))
        }
        CheckedType::Borrowed { inner, mutable } => {
            let inner = substitute_generic_checked_type(typed, inner, bindings, origin)?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Borrowed { inner, mutable }))
        }
        CheckedType::Pointer {
            target,
            shared,
            weak,
            sync,
        } => {
            let target = substitute_generic_checked_type(typed, target, bindings, origin)?;
            Ok(typed.type_table_mut().intern(CheckedType::Pointer {
                target,
                shared,
                weak,
                sync,
            }))
        }
        CheckedType::Error { inner } => {
            let inner = inner
                .map(|inner| {
                    substitute_generic_checked_type(typed, inner, bindings, origin.clone())
                })
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
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Record { fields }))
        }
        CheckedType::Entry { variants } => {
            let variants = variants
                .into_iter()
                .map(|(variant_name, variant_type)| {
                    variant_type
                        .map(|variant_type| {
                            substitute_generic_checked_type(
                                typed,
                                variant_type,
                                bindings,
                                origin.clone(),
                            )
                        })
                        .transpose()
                        .map(|variant_type| (variant_name, variant_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Entry { variants }))
        }
        CheckedType::Routine(signature) => {
            let params = signature
                .params
                .into_iter()
                .map(|param| {
                    substitute_generic_checked_type(typed, param, bindings, origin.clone())
                })
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
            Ok(typed
                .type_table_mut()
                .intern(CheckedType::Routine(RoutineType {
                    generic_params: Vec::new(),
                    generic_constraints: BTreeMap::new(),
                    param_names: signature.param_names,
                    param_defaults: signature.param_defaults,
                    variadic_index: signature.variadic_index,
                    mutex_params: signature.mutex_params,
                    params,
                    return_type,
                    error_type,
                    env_lifetime: signature.env_lifetime,
                })))
        }
    }
}

/// Unify a routine's receiver template against a concrete object type,
/// binding any generic parameters from `generic_params` along the way.
///
/// Returns `Some(bindings)` iff the object type is structurally compatible
/// with the template and every generic parameter that appears in the
/// template has a consistent binding. Used by method resolution to match
/// a `fun (Box[T])...` receiver against a call-site `Box[int]`.
pub(crate) fn unify_receiver_with_object(
    typed: &TypedProgram,
    template: CheckedTypeId,
    object: CheckedTypeId,
    generic_params: &[SymbolId],
) -> Option<BTreeMap<SymbolId, CheckedTypeId>> {
    let mut bindings: BTreeMap<SymbolId, CheckedTypeId> = BTreeMap::new();
    if unify_checked_type(typed, template, object, generic_params, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

fn unify_checked_type(
    typed: &TypedProgram,
    template: CheckedTypeId,
    object: CheckedTypeId,
    generic_params: &[SymbolId],
    bindings: &mut BTreeMap<SymbolId, CheckedTypeId>,
) -> bool {
    if template == object {
        return true;
    }
    let template_checked = match typed.type_table().get(template) {
        Some(checked) => checked,
        None => return false,
    };
    if let CheckedType::Declared {
        symbol,
        kind: DeclaredTypeKind::GenericParameter,
        ..
    } = template_checked
    {
        if generic_params.contains(symbol) {
            match bindings.get(symbol) {
                Some(existing) => return *existing == object,
                None => {
                    bindings.insert(*symbol, object);
                    return true;
                }
            }
        }
    }
    let object_checked = match typed.type_table().get(object) {
        Some(checked) => checked,
        None => return false,
    };
    match (template_checked.clone(), object_checked.clone()) {
        (CheckedType::Builtin(a), CheckedType::Builtin(b)) => a == b,
        (
            CheckedType::Declared {
                symbol: a,
                kind: DeclaredTypeKind::Type,
                args: template_args,
                ..
            },
            CheckedType::Declared {
                symbol: b,
                kind: DeclaredTypeKind::Type,
                args: object_args,
                ..
            },
        )
        | (
            CheckedType::Declared {
                symbol: a,
                kind: DeclaredTypeKind::Alias,
                args: template_args,
                ..
            },
            CheckedType::Declared {
                symbol: b,
                kind: DeclaredTypeKind::Alias,
                args: object_args,
                ..
            },
        ) => {
            // Nominal unification: same base declaration, and the type
            // arguments unify pairwise (binds `T` from `Box[T]` vs `Box[int]`).
            a == b
                && template_args.len() == object_args.len()
                && template_args
                    .iter()
                    .zip(object_args.iter())
                    .all(|(t, o)| unify_checked_type(typed, *t, *o, generic_params, bindings))
        }
        (
            CheckedType::Array {
                element_type: t,
                size: ts,
            },
            CheckedType::Array {
                element_type: o,
                size: os,
            },
        ) => ts == os && unify_checked_type(typed, t, o, generic_params, bindings),
        (CheckedType::Vector { element_type: t }, CheckedType::Vector { element_type: o })
        | (CheckedType::Sequence { element_type: t }, CheckedType::Sequence { element_type: o })
        | (CheckedType::Optional { inner: t }, CheckedType::Optional { inner: o }) => {
            unify_checked_type(typed, t, o, generic_params, bindings)
        }
        (CheckedType::Error { inner: t }, CheckedType::Error { inner: o }) => match (t, o) {
            (Some(t), Some(o)) => unify_checked_type(typed, t, o, generic_params, bindings),
            (None, None) => true,
            _ => false,
        },
        (
            CheckedType::Map {
                key_type: tk,
                value_type: tv,
            },
            CheckedType::Map {
                key_type: ok,
                value_type: ov,
            },
        ) => {
            unify_checked_type(typed, tk, ok, generic_params, bindings)
                && unify_checked_type(typed, tv, ov, generic_params, bindings)
        }
        (CheckedType::Set { member_types: t }, CheckedType::Set { member_types: o }) => {
            t.len() == o.len()
                && t.iter()
                    .zip(o.iter())
                    .all(|(a, b)| unify_checked_type(typed, *a, *b, generic_params, bindings))
        }
        (CheckedType::Record { fields: t }, CheckedType::Record { fields: o }) => {
            t.len() == o.len()
                && t.iter().all(|(name, t_ty)| match o.get(name) {
                    Some(o_ty) => unify_checked_type(typed, *t_ty, *o_ty, generic_params, bindings),
                    None => false,
                })
        }
        (CheckedType::Entry { variants: t }, CheckedType::Entry { variants: o }) => {
            t.len() == o.len()
                && t.iter().all(|(name, t_ty)| match o.get(name) {
                    Some(o_ty) => match (t_ty, o_ty) {
                        (Some(t_ty), Some(o_ty)) => {
                            unify_checked_type(typed, *t_ty, *o_ty, generic_params, bindings)
                        }
                        (None, None) => true,
                        _ => false,
                    },
                    None => false,
                })
        }
        _ => false,
    }
}

fn reject_heap_backed_type_in_core(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    typ: &FolType,
    label: &str,
    fallback_origin: Option<SyntaxOrigin>,
) -> Result<(), TypecheckError> {
    if typed.capability_model() != crate::TypecheckCapabilityModel::Core {
        return Ok(());
    }

    let message = format!("{label} requires heap support and is unavailable in 'fol_model = core'");
    Err(match type_origin(resolved, typ).or(fallback_origin) {
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
                        "standard '{}' is a static contract, not a value type; use it as a generic constraint instead",
                        symbol.name
                    ),
                    origin,
                ),
                None => TypecheckError::new(
                    TypecheckErrorKind::Unsupported,
                    format!(
                        "standard '{}' is a static contract, not a value type; use it as a generic constraint instead",
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
        args: Vec::new(),
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

pub(crate) fn find_routine_symbol_id(
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

fn record_symbol_param_defaults(
    typed: &mut TypedProgram,
    symbol_id: SymbolId,
    param_defaults: Vec<Option<AstNode>>,
) -> Result<(), TypecheckError> {
    let symbol = typed.typed_symbol_mut(symbol_id).ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::SymbolTableCorrupted,
            format!(
                "symbol table corrupted: symbol {} is missing while recording parameter defaults",
                symbol_id.0,
            ),
        )
    })?;
    symbol.param_defaults = param_defaults;
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

fn record_symbol_generic_constraints(
    typed: &mut TypedProgram,
    symbol_id: SymbolId,
    generic_constraints: BTreeMap<SymbolId, Vec<GenericConstraint>>,
) -> Result<(), TypecheckError> {
    let symbol = typed.typed_symbol_mut(symbol_id).ok_or_else(|| {
        TypecheckError::new(
            TypecheckErrorKind::SymbolTableCorrupted,
            format!(
                "symbol table corrupted: generic constraint owner {} disappeared",
                symbol_id.0
            ),
        )
    })?;
    symbol.generic_constraints = generic_constraints;
    Ok(())
}

/// The first record field whose type is a known move-only/heap type, and thus
/// cannot satisfy a `copy` claim. Record/entry fields are treated as acceptable
/// here; their own `copy` obligation is verified on their own declaration.
fn first_known_non_copy_field(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<Option<String>, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    if let Some(CheckedType::Record { fields }) = typed.type_table().get(apparent) {
        let fields = fields.clone();
        for (name, field_type) in &fields {
            if type_is_known_non_copy(typed, *field_type)? {
                return Ok(Some(name.clone()));
            }
        }
    }
    Ok(None)
}

/// The first record field that is a nominal user aggregate NOT itself claiming
/// `copy` (V3_MEM §4.1: "the compiler verifies each claim recursively"). Unlike
/// `clone`, `copy` has no structural default, so a `copy` type's user-declared
/// aggregate fields must each declare `copy` too. Primitives and non-nominal
/// fields are structurally copy-safe and handled by `first_known_non_copy_field`.
/// Decl-order-safe: run only after every type's capability claims are recorded.
fn first_field_lacking_declared_copy(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<Option<String>, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    let Some(CheckedType::Record { fields }) = typed.type_table().get(apparent) else {
        return Ok(None);
    };
    let fields = fields.clone();
    for (name, field_type) in &fields {
        // Resolve the field's nominal declaring symbol, if any.
        let Some(CheckedType::Declared { symbol, .. }) = typed.type_table().get(*field_type) else {
            continue;
        };
        let symbol = *symbol;
        // Only nominal aggregates (records/entries) need an explicit `copy`
        // claim; a `Declared` alias of a primitive is structurally copy-safe.
        let field_apparent = crate::exprs::helpers::apparent_type_id(typed, *field_type)?;
        let is_aggregate = matches!(
            typed.type_table().get(field_apparent),
            Some(CheckedType::Record { .. }) | Some(CheckedType::Entry { .. })
        );
        if !is_aggregate {
            continue;
        }
        let claims_copy = typed
            .capability_claims(symbol)
            .is_some_and(|caps| caps.contains("copy"));
        if !claims_copy {
            return Ok(Some(name.clone()));
        }
    }
    Ok(None)
}

/// Whether `type_id` is a nominal user aggregate (record/entry) that does NOT
/// declare `copy`. `copy` has no structural default (§4.1), so `[cpy]value`
/// requires the value's own type to claim `copy` — an all-copy-safe record
/// without a `(copy)` header is still move/clone-only. Primitives, containers,
/// and non-nominal types return `false` (their copy-ness is structural and
/// handled by `type_lacks_copy`). Decl-order-safe: claims are recorded before
/// any routine body is typed.
pub(crate) fn type_is_nominal_aggregate_lacking_copy(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    let Some(CheckedType::Declared { symbol, .. }) = typed.type_table().get(type_id) else {
        return Ok(false);
    };
    let symbol = *symbol;
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    let is_aggregate = matches!(
        typed.type_table().get(apparent),
        Some(CheckedType::Record { .. }) | Some(CheckedType::Entry { .. })
    );
    if !is_aggregate {
        return Ok(false);
    }
    Ok(!typed
        .capability_claims(symbol)
        .is_some_and(|caps| caps.contains("copy")))
}

/// Whether a type is a known move-only / heap-backed type that cannot be `copy`.
/// Conservative: aggregates whose copy-ness depends on other declarations return
/// `false` so a copy claim is never rejected by declaration order.
pub(crate) fn type_is_known_non_copy(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    Ok(match typed.type_table().get(apparent) {
        Some(CheckedType::Builtin(crate::BuiltinType::Str)) => true,
        Some(CheckedType::Owned { .. })
        | Some(CheckedType::Pointer { .. })
        | Some(CheckedType::Channel { .. })
        | Some(CheckedType::ChannelSender { .. })
        | Some(CheckedType::ChannelReceiver { .. })
        | Some(CheckedType::Eventual { .. })
        | Some(CheckedType::Borrowed { .. }) => true,
        Some(CheckedType::Optional { inner }) => {
            let inner = *inner;
            type_is_known_non_copy(typed, inner)?
        }
        _ => false,
    })
}

/// The first field of a record `type_id` that is not thread-safe, if any. A
/// field is not thread-safe when it owns a `fin` foreign resource or an
/// `Rc`-based shared/weak pointer, or is a borrow — none are proven `send` or
/// `share` (V3_MEM §4.1). Record/entry fields are allowed here; their own
/// capability claim is verified on their own declaration, avoiding
/// declaration-order false rejects. Used for both `send` and `share` claims:
/// FOL's non-safe types (`fin`, `Rc` pointers) are neither `Send` nor `Sync`.
fn first_known_non_thread_safe_field(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<Option<String>, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    if let Some(CheckedType::Record { fields }) = typed.type_table().get(apparent) {
        let fields = fields.clone();
        for (name, field_type) in &fields {
            if type_is_known_non_thread_safe(typed, *field_type)? {
                return Ok(Some(name.clone()));
            }
        }
    }
    Ok(None)
}

/// Whether `type_id` transitively contains a non-thread-safe leaf, recursing
/// through record/entry/container fields (V3_MEM §4.1: "verifies each claim
/// recursively"). A `send`/`share` claim on an aggregate is unsound if any
/// nested field is a `fin` value, `Rc` shared/weak pointer, or borrow — the
/// emitted Rust would fail its own `Send`/`Sync` bound. Cycle-guarded for
/// recursive types. Decl-order-safe only after all types are lowered, so this
/// is used from `check_standard_conformance`, not the main type pass.
fn type_transitively_non_thread_safe(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    visiting: &mut std::collections::BTreeSet<CheckedTypeId>,
) -> Result<bool, TypecheckError> {
    if type_is_known_non_thread_safe(typed, type_id)? {
        return Ok(true);
    }
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    if !visiting.insert(apparent) {
        return Ok(false);
    }
    let mut nested: Vec<CheckedTypeId> = Vec::new();
    match typed.type_table().get(apparent) {
        Some(CheckedType::Record { fields }) => nested.extend(fields.values().copied()),
        Some(CheckedType::Entry { variants }) => {
            nested.extend(variants.values().flatten().copied())
        }
        Some(CheckedType::Array { element_type, .. })
        | Some(CheckedType::Vector { element_type })
        | Some(CheckedType::Sequence { element_type })
        | Some(CheckedType::Optional {
            inner: element_type,
        }) => nested.push(*element_type),
        Some(CheckedType::Set { member_types }) => nested.extend(member_types.iter().copied()),
        Some(CheckedType::Map {
            key_type,
            value_type,
        }) => nested.extend([*key_type, *value_type]),
        _ => {}
    }
    let mut result = false;
    for field_type in nested {
        if type_transitively_non_thread_safe(typed, field_type, visiting)? {
            result = true;
            break;
        }
    }
    visiting.remove(&apparent);
    Ok(result)
}

/// The first direct record field of `type_id` that transitively contains a
/// non-thread-safe leaf. Reports the direct field name for the `send`/`share`
/// claim error.
fn first_field_transitively_non_thread_safe(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<Option<String>, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    let Some(CheckedType::Record { fields }) = typed.type_table().get(apparent) else {
        return Ok(None);
    };
    let fields = fields.clone();
    for (name, field_type) in &fields {
        let mut visiting = std::collections::BTreeSet::new();
        if type_transitively_non_thread_safe(typed, *field_type, &mut visiting)? {
            return Ok(Some(name.clone()));
        }
    }
    Ok(None)
}

/// Whether a type is a known non-thread-safe type — neither `send` nor `share`:
/// a `fin` value, an `Rc`-based shared/weak pointer, or a borrow. Conservative —
/// aggregates whose thread-safety depends on other declarations return `false`.
fn type_is_known_non_thread_safe(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    if typed.type_resolves_to_fin(apparent) {
        return Ok(true);
    }
    Ok(match typed.type_table().get(apparent) {
        // An `Rc`-backed shared/weak pointer is not thread-safe; the `Arc`-backed
        // `ptr[shared, sync, T]` (sync == true) is.
        Some(CheckedType::Pointer {
            shared: true,
            sync: false,
            ..
        })
        | Some(CheckedType::Pointer {
            weak: true,
            sync: false,
            ..
        })
        | Some(CheckedType::Borrowed { .. }) => true,
        Some(CheckedType::Optional { inner }) => {
            let inner = *inner;
            type_is_known_non_thread_safe(typed, inner)?
        }
        _ => false,
    })
}

/// Whether a value of `type_id` definitively lacks the `copy` capability: it is
/// either a known move-only leaf type, or a record with at least one field that
/// is not copy-safe. Used to reject `[cpy]` operands (V3_MEM §4.1). Conservative
/// — an all-copy-safe record is allowed even without an explicit `copy` claim;
/// requiring the claim is a stricter later Slice B step.
pub(crate) fn type_lacks_copy(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    if type_is_known_non_copy(typed, type_id)? {
        return Ok(true);
    }
    Ok(first_known_non_copy_field(typed, type_id)?.is_some())
}

/// Whether a type is a known unique/one-shot handle that cannot be `clone`d.
/// These are the runtime handles with no `Clone` impl: a full channel, a unique
/// `chn[rx, T]` receiver, and a one-shot eventual. A `chn[tx, T]` sender is
/// deliberately excluded — senders are clone-capable (V3_MEM §8.2). Conservative:
/// everything whose clone-ability is structural or capability-declared returns
/// `false` so a `[cln]` is never rejected by declaration order.
pub(crate) fn type_is_known_non_clone(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    // A `fin` value has no structural `clone`: `copy` and `fin` cannot coexist,
    // and `fin + clone` requires a custom clone (not the structural default), so
    // `[cln]` on it is rejected until such a custom clone exists (V3_MEM §4.1).
    if typed.type_resolves_to_fin(apparent) {
        return Ok(true);
    }
    Ok(matches!(
        typed.type_table().get(apparent),
        Some(CheckedType::Channel { .. })
            | Some(CheckedType::ChannelReceiver { .. })
            | Some(CheckedType::Eventual { .. })
    ))
}

/// Whether a value of `type_id` definitively lacks the `clone` capability: it is
/// a known non-clone leaf (a unique channel/eventual handle or a `fin` value),
/// or a record with at least one field that is not clonable. Used to reject
/// `[cln]` operands (V3_MEM §4.1). Conservative — an all-clonable record is
/// allowed even without an explicit `clone` claim (structural default).
pub(crate) fn type_lacks_clone(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<bool, TypecheckError> {
    if type_is_known_non_clone(typed, type_id)? {
        return Ok(true);
    }
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    if let Some(CheckedType::Record { fields }) = typed.type_table().get(apparent) {
        let fields = fields.clone();
        for field_type in fields.values() {
            if type_is_known_non_clone(typed, *field_type)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether `type_id` transitively contains a non-clonable leaf (a unique
/// channel/eventual handle or a `fin` value), recursing through record/entry/
/// container fields. A `clone` claim on an aggregate is unsound if any nested
/// field cannot be cloned — the emitted Rust `#[derive(Clone)]` / `.clone()`
/// would fail to compile (V3_MEM §4.1: "clone receives a structural default
/// when every field supports it"; `fin + clone` needs an unimplemented custom
/// clone). Cycle-guarded; decl-order-safe only after all types are lowered.
fn type_transitively_non_clone(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
    visiting: &mut std::collections::BTreeSet<CheckedTypeId>,
) -> Result<bool, TypecheckError> {
    if type_is_known_non_clone(typed, type_id)? {
        return Ok(true);
    }
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    if !visiting.insert(apparent) {
        return Ok(false);
    }
    let mut nested: Vec<CheckedTypeId> = Vec::new();
    match typed.type_table().get(apparent) {
        Some(CheckedType::Record { fields }) => nested.extend(fields.values().copied()),
        Some(CheckedType::Entry { variants }) => {
            nested.extend(variants.values().flatten().copied())
        }
        Some(CheckedType::Array { element_type, .. })
        | Some(CheckedType::Vector { element_type })
        | Some(CheckedType::Sequence { element_type })
        | Some(CheckedType::Optional {
            inner: element_type,
        }) => nested.push(*element_type),
        Some(CheckedType::Set { member_types }) => nested.extend(member_types.iter().copied()),
        Some(CheckedType::Map {
            key_type,
            value_type,
        }) => nested.extend([*key_type, *value_type]),
        _ => {}
    }
    let mut result = false;
    for field_type in nested {
        if type_transitively_non_clone(typed, field_type, visiting)? {
            result = true;
            break;
        }
    }
    visiting.remove(&apparent);
    Ok(result)
}

/// The first direct record field of `type_id` that transitively contains a
/// non-clonable leaf. Reports the direct field name for the `clone` claim error.
fn first_field_transitively_non_clone(
    typed: &TypedProgram,
    type_id: CheckedTypeId,
) -> Result<Option<String>, TypecheckError> {
    let apparent = crate::exprs::helpers::apparent_type_id(typed, type_id)?;
    let Some(CheckedType::Record { fields }) = typed.type_table().get(apparent) else {
        return Ok(None);
    };
    let fields = fields.clone();
    for (name, field_type) in &fields {
        let mut visiting = std::collections::BTreeSet::new();
        if type_transitively_non_clone(typed, *field_type, &mut visiting)? {
            return Ok(Some(name.clone()));
        }
    }
    Ok(None)
}

fn lower_generic_constraints_for_params(
    typed: &mut TypedProgram,
    resolved: &ResolvedProgram,
    scope_id: ScopeId,
    generics: &[Generic],
    generic_params: &[SymbolId],
) -> Result<BTreeMap<SymbolId, Vec<GenericConstraint>>, TypecheckError> {
    let mut generic_constraints = BTreeMap::new();
    for (generic, symbol_id) in generics.iter().zip(generic_params.iter().copied()) {
        // Compiler-owned capability standards (`copy`/`clone`/`fin`/`send`/
        // `share`) carry no standard symbol; record them as capability bounds on
        // the generic parameter so call sites can verify the actual type has the
        // capability (V3_MEM §4.1), then drop them from the symbol-based list.
        for constraint in &generic.constraints {
            if let FolType::Named { name, .. } = constraint {
                let base = name.split('[').next().unwrap_or(name);
                if fol_parser::ast::is_compiler_owned_generic_constraint(base) {
                    typed.record_generic_capability_constraint(symbol_id, base.to_string());
                }
            }
        }
        let lowered_constraints = generic
            .constraints
            .iter()
            .filter(|constraint| {
                !matches!(constraint, FolType::Named { name, .. }
                    if fol_parser::ast::is_compiler_owned_generic_constraint(
                        name.split('[').next().unwrap_or(name)))
            })
            .map(|constraint| {
                lower_standard_constraint_for_contract(typed, resolved, scope_id, constraint)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if !lowered_constraints.is_empty() {
            generic_constraints.insert(symbol_id, lowered_constraints);
        }
    }
    Ok(generic_constraints)
}

fn binding_names(pattern: &BindingPattern) -> Vec<String> {
    match pattern {
        BindingPattern::Name(name, _) | BindingPattern::Rest(name) => vec![name.clone()],
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

fn unsupported_type_error(
    resolved: &ResolvedProgram,
    typ: &FolType,
    model: TypecheckCapabilityModel,
) -> TypecheckError {
    let label = match typ {
        FolType::Matrix { .. } => "matrix types are not yet supported",
        FolType::Channel { .. } if !model.supports_processor() => {
            "channel types require hosted std support; declare the bundled internal standard dependency"
        }
        FolType::Channel { .. } => {
            "channel types require hosted std support; declare the bundled internal standard dependency"
        }
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
        | FolType::Location { .. } => "package/build-specific type surfaces are not yet supported",
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
    model: TypecheckCapabilityModel,
) -> Option<TypecheckError> {
    let origin = resolved.syntax_index().origin(item.node_id).cloned();
    unsupported_v1_decl_with_origin(&item.node, origin, model)
}

fn unsupported_v1_nested_decl(
    resolved: &ResolvedProgram,
    node: &AstNode,
    model: TypecheckCapabilityModel,
) -> Option<TypecheckError> {
    unsupported_v1_decl_with_origin(node, node_origin(resolved, node), model)
}

fn unsupported_v1_decl_with_origin(
    node: &AstNode,
    origin: Option<SyntaxOrigin>,
    model: TypecheckCapabilityModel,
) -> Option<TypecheckError> {
    let message = match node {
        AstNode::VarDecl { options, .. } | AstNode::LabDecl { options, .. } => {
            unsupported_binding_surface_message(options)
        }
        AstNode::FunDecl { params, .. }
        | AstNode::ProDecl { params, .. }
        | AstNode::LogDecl { params, .. } => {
            unsupported_routine_param_surface_message(params, model)
        }
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
        AstNode::SegDecl { .. } => Some("segment declarations are planned for a future release"),
        AstNode::StdDecl { .. } => None,
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
        .ok_or_else(|| {
            internal_error(
                "resolved source unit disappeared during type lowering",
                None,
            )
        })?;
    let decl_origin = resolved.syntax_index().origin(item.node_id).cloned();
    let source_unit = resolved
        .syntax()
        .source_units
        .get(source_unit_id.0)
        .ok_or_else(|| {
            internal_error(
                "resolved source unit disappeared during type lowering",
                None,
            )
        })?;
    let type_scope_index = source_unit
        .items
        .iter()
        .filter(|candidate| matches!(candidate.node, AstNode::TypeDecl { .. }))
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
    model: TypecheckCapabilityModel,
) -> Option<&'static str> {
    if params.iter().any(|param| param.is_mutex) {
        if model.supports_processor() {
            None
        } else {
            Some(
            "mutex parameters require hosted std support; declare the bundled internal standard dependency"
            )
        }
    } else {
        None
    }
}

fn unsupported_binding_surface_message(options: &[VarOption]) -> Option<&'static str> {
    if options
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
        FolType::Owned { inner } | FolType::Optional { inner } => type_origin(resolved, inner),
        FolType::Pointer { target, .. } => type_origin(resolved, target),
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

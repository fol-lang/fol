//! FOL-side monomorphization of generic receiver routines.
//!
//! Generic receiver routines such as `fun (Box[T])unwrap(T)(fallback: T): T`
//! cannot ride the Rust-generics emission path: their receiver names a generic
//! record template that never lowers to a backend type declaration. Instead,
//! after routine bodies are lowered, this pass rewrites every call to such a
//! template into a call to a synthesized concrete clone. The clone's types are
//! produced by substituting the generic parameters with the concrete types
//! observed at the call site, so the backend only ever sees ordinary concrete
//! routines. The templates themselves are removed once every call is rewritten.

use crate::{
    control::{LoweredInstrKind, LoweredRoutine},
    ids::{LoweredInstrId, LoweredRoutineId, LoweredTypeId},
    types::{LoweredRoutineType, LoweredType, LoweredTypeTable},
    LoweredFieldLayout, LoweredPackage, LoweredTypeDecl, LoweredTypeDeclKind, LoweredVariantLayout,
    LoweringError, LoweringErrorKind, LoweringResult,
};
use fol_resolver::{PackageIdentity, SourceUnitId, SymbolId};
use std::collections::{BTreeMap, BTreeSet};

type GenericBindings = BTreeMap<String, LoweredTypeId>;
type InstantiationKey = (LoweredRoutineId, Vec<(String, LoweredTypeId)>);

pub(crate) fn monomorphize_generic_receiver_routines(
    packages: &mut BTreeMap<PackageIdentity, LoweredPackage>,
    type_table: &mut LoweredTypeTable,
    next_routine_index: &mut usize,
) -> LoweringResult<()> {
    let templates = collect_templates(packages, type_table);
    if templates.is_empty() {
        return Ok(());
    }

    let mut worklist: Vec<(PackageIdentity, LoweredRoutineId)> = packages
        .iter()
        .flat_map(|(identity, package)| {
            package
                .routine_decls
                .keys()
                .filter(|routine_id| !templates.contains_key(routine_id))
                .map(|routine_id| (identity.clone(), *routine_id))
                .collect::<Vec<_>>()
        })
        .collect();
    let mut instantiations: BTreeMap<InstantiationKey, LoweredRoutineId> = BTreeMap::new();
    let mut errors = Vec::new();

    while let Some((identity, routine_id)) = worklist.pop() {
        let call_sites = match collect_template_call_sites(
            packages,
            &identity,
            routine_id,
            &templates,
        ) {
            Ok(call_sites) => call_sites,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };

        for (instr_id, template_id, arg_types) in call_sites {
            let template = &templates[&template_id];
            let bindings = match derive_call_bindings(type_table, template, &arg_types) {
                Ok(bindings) => bindings,
                Err(error) => {
                    errors.push(error);
                    continue;
                }
            };
            let key = (
                template_id,
                bindings
                    .iter()
                    .map(|(name, type_id)| (name.clone(), *type_id))
                    .collect(),
            );
            let concrete_id = match instantiations.get(&key) {
                Some(existing) => *existing,
                None => match instantiate_template(
                    packages,
                    type_table,
                    next_routine_index,
                    template,
                    &bindings,
                ) {
                    Ok(concrete_id) => {
                        instantiations.insert(key, concrete_id);
                        worklist.push((template.owner.clone(), concrete_id));
                        concrete_id
                    }
                    Err(error) => {
                        errors.push(error);
                        continue;
                    }
                },
            };
            patch_call_target(packages, &identity, routine_id, instr_id, concrete_id);
        }
    }

    for template in templates.values() {
        if let Some(package) = packages.get_mut(&template.owner) {
            package.routine_decls.remove(&template.routine.id);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

struct Template {
    owner: PackageIdentity,
    routine: LoweredRoutine,
}

fn collect_templates(
    packages: &BTreeMap<PackageIdentity, LoweredPackage>,
    type_table: &LoweredTypeTable,
) -> BTreeMap<LoweredRoutineId, Template> {
    let mut templates = BTreeMap::new();
    for (identity, package) in packages {
        for routine in package.routine_decls.values() {
            let generic_receiver = routine
                .receiver_type
                .is_some_and(|receiver_type| {
                    type_contains_generic_parameter(type_table, receiver_type)
                });
            // Routines that call required constraint routines on their
            // generic parameters cannot ride Rust-generics emission either:
            // the callee only exists after substitution.
            let has_constraint_calls = routine
                .instructions
                .iter()
                .any(|instr| matches!(instr.kind, LoweredInstrKind::ConstraintCall { .. }));
            // A free generic routine that builds or returns a generic-parameterized
            // record/entry (e.g. `Box[T]`) cannot ride the Rust-generics path: the
            // structural type would need a generic Rust struct declaration, which the
            // monomorphization model forbids. Template it so the structural type is
            // concrete before backend emission.
            let uses_generic_structural = routine
                .locals
                .iter()
                .filter_map(|local| local.type_id)
                .any(|type_id| type_table.contains_generic_structural_type(type_id))
                || routine.signature.is_some_and(|signature| {
                    type_table.contains_generic_structural_type(signature)
                });
            if generic_receiver || has_constraint_calls || uses_generic_structural {
                templates.insert(
                    routine.id,
                    Template {
                        owner: identity.clone(),
                        routine: routine.clone(),
                    },
                );
            }
        }
    }

    // Propagate templating across generic -> template call chains. A generic
    // routine that calls a template routine cannot ride Rust-generics emission:
    // the callee only exists as monomorphized copies once its own generic
    // parameters are concrete, so the caller must itself be instantiated first
    // to substitute those parameters before the nested template call is
    // resolved. This fixpoint handles arbitrarily deep two-level (and beyond)
    // constraint propagation.
    loop {
        let mut added = false;
        for (identity, package) in packages {
            for routine in package.routine_decls.values() {
                if templates.contains_key(&routine.id) {
                    continue;
                }
                if !routine_is_generic(type_table, routine) {
                    continue;
                }
                let calls_template = routine.instructions.iter().any(|instr| {
                    matches!(
                        &instr.kind,
                        LoweredInstrKind::Call { callee, .. } if templates.contains_key(callee)
                    )
                });
                if calls_template {
                    templates.insert(
                        routine.id,
                        Template {
                            owner: identity.clone(),
                            routine: routine.clone(),
                        },
                    );
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
    }

    templates
}

/// A routine is generic when any of its parameter/local types, its signature,
/// or its receiver still mentions a generic parameter. Such routines can only
/// be realized by monomorphization once their generic parameters are fixed.
fn routine_is_generic(type_table: &LoweredTypeTable, routine: &LoweredRoutine) -> bool {
    routine
        .locals
        .iter()
        .filter_map(|local| local.type_id)
        .any(|type_id| type_contains_generic_parameter(type_table, type_id))
        || routine
            .signature
            .is_some_and(|signature| type_contains_generic_parameter(type_table, signature))
        || routine
            .receiver_type
            .is_some_and(|receiver_type| type_contains_generic_parameter(type_table, receiver_type))
}

type TemplateCallSite = (LoweredInstrId, LoweredRoutineId, Vec<LoweredTypeId>);

fn collect_template_call_sites(
    packages: &BTreeMap<PackageIdentity, LoweredPackage>,
    identity: &PackageIdentity,
    routine_id: LoweredRoutineId,
    templates: &BTreeMap<LoweredRoutineId, Template>,
) -> Result<Vec<TemplateCallSite>, LoweringError> {
    let Some(routine) = packages
        .get(identity)
        .and_then(|package| package.routine_decls.get(&routine_id))
    else {
        return Ok(Vec::new());
    };

    let mut call_sites = Vec::new();
    for (instr_id, instr) in routine.instructions.iter_with_ids() {
        match &instr.kind {
            LoweredInstrKind::Call { callee, args, .. } if templates.contains_key(callee) => {
                let arg_types = args
                    .iter()
                    .map(|arg| {
                        routine
                            .locals
                            .get(*arg)
                            .and_then(|local| local.type_id)
                            .ok_or_else(|| {
                                LoweringError::with_kind(
                                    LoweringErrorKind::InvalidInput,
                                    format!(
                                        "call to generic receiver routine '{}' passes an untyped argument",
                                        templates[callee].routine.name
                                    ),
                                )
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                call_sites.push((instr_id, *callee, arg_types));
            }
            LoweredInstrKind::RoutineRef { routine } if templates.contains_key(routine) => {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::Unsupported,
                    format!(
                        "generic receiver routine '{}' cannot be referenced as a value",
                        templates[routine].routine.name
                    ),
                ));
            }
            _ => {}
        }
    }
    Ok(call_sites)
}

fn derive_call_bindings(
    type_table: &LoweredTypeTable,
    template: &Template,
    arg_types: &[LoweredTypeId],
) -> Result<GenericBindings, LoweringError> {
    let param_types = template
        .routine
        .params
        .iter()
        .map(|param| {
            template
                .routine
                .locals
                .get(*param)
                .and_then(|local| local.type_id)
                .ok_or_else(|| {
                    LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "generic receiver routine '{}' has an untyped parameter",
                            template.routine.name
                        ),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if param_types.len() != arg_types.len() {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "call to generic receiver routine '{}' passes {} argument(s) but the routine declares {} parameter(s)",
                template.routine.name,
                arg_types.len(),
                param_types.len()
            ),
        ));
    }

    let mut bindings = GenericBindings::new();
    for (param_type, arg_type) in param_types.iter().zip(arg_types.iter()) {
        unify_template_type(type_table, *param_type, *arg_type, &mut bindings).map_err(
            |detail| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "call to generic receiver routine '{}' does not determine its generic parameters: {detail}",
                        template.routine.name
                    ),
                )
            },
        )?;
    }

    for bound in bindings.values() {
        if type_contains_generic_parameter(type_table, *bound) {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::Unsupported,
                format!(
                    "calling generic receiver routine '{}' with a generic receiver is not supported; \
                     generic receiver routines must be called on concrete instantiations",
                    template.routine.name
                ),
            ));
        }
    }

    Ok(bindings)
}

fn unify_template_type(
    type_table: &LoweredTypeTable,
    template_type: LoweredTypeId,
    concrete_type: LoweredTypeId,
    bindings: &mut GenericBindings,
) -> Result<(), String> {
    if template_type == concrete_type {
        return Ok(());
    }

    let Some(template) = type_table.get(template_type) else {
        return Err(format!("template type {} is missing", template_type.0));
    };
    let Some(concrete) = type_table.get(concrete_type) else {
        return Err(format!("concrete type {} is missing", concrete_type.0));
    };

    match (template, concrete) {
        (LoweredType::GenericParameter { name }, _) => {
            if let Some(existing) = bindings.get(name) {
                if *existing != concrete_type {
                    return Err(format!(
                        "generic parameter '{name}' binds to conflicting types"
                    ));
                }
                return Ok(());
            }
            bindings.insert(name.clone(), concrete_type);
            Ok(())
        }
        (
            LoweredType::Array {
                element_type: template_element,
                size: template_size,
            },
            LoweredType::Array {
                element_type: concrete_element,
                size: concrete_size,
            },
        ) if template_size == concrete_size => {
            unify_template_type(type_table, *template_element, *concrete_element, bindings)
        }
        (
            LoweredType::Vector {
                element_type: template_element,
            },
            LoweredType::Vector {
                element_type: concrete_element,
            },
        )
        | (
            LoweredType::Sequence {
                element_type: template_element,
            },
            LoweredType::Sequence {
                element_type: concrete_element,
            },
        )
        | (
            LoweredType::Optional {
                inner: template_element,
            },
            LoweredType::Optional {
                inner: concrete_element,
            },
        ) => unify_template_type(type_table, *template_element, *concrete_element, bindings),
        (
            LoweredType::Set {
                member_types: template_members,
            },
            LoweredType::Set {
                member_types: concrete_members,
            },
        ) if template_members.len() == concrete_members.len() => {
            for (template_member, concrete_member) in
                template_members.iter().zip(concrete_members.iter())
            {
                unify_template_type(type_table, *template_member, *concrete_member, bindings)?;
            }
            Ok(())
        }
        (
            LoweredType::Map {
                key_type: template_key,
                value_type: template_value,
            },
            LoweredType::Map {
                key_type: concrete_key,
                value_type: concrete_value,
            },
        ) => {
            unify_template_type(type_table, *template_key, *concrete_key, bindings)?;
            unify_template_type(type_table, *template_value, *concrete_value, bindings)
        }
        (
            LoweredType::Error {
                inner: template_inner,
            },
            LoweredType::Error {
                inner: concrete_inner,
            },
        ) => match (template_inner, concrete_inner) {
            (Some(template_inner), Some(concrete_inner)) => {
                unify_template_type(type_table, *template_inner, *concrete_inner, bindings)
            }
            (None, None) => Ok(()),
            _ => Err("error shells disagree about their payload".to_string()),
        },
        (
            LoweredType::Record {
                fields: template_fields,
            },
            LoweredType::Record {
                fields: concrete_fields,
            },
        ) if template_fields.len() == concrete_fields.len() => {
            for ((template_name, template_field), (concrete_name, concrete_field)) in
                template_fields.iter().zip(concrete_fields.iter())
            {
                if template_name != concrete_name {
                    return Err(format!(
                        "record fields '{template_name}' and '{concrete_name}' do not match"
                    ));
                }
                unify_template_type(type_table, *template_field, *concrete_field, bindings)?;
            }
            Ok(())
        }
        (
            LoweredType::Entry {
                variants: template_variants,
            },
            LoweredType::Entry {
                variants: concrete_variants,
            },
        ) if template_variants.len() == concrete_variants.len() => {
            for ((template_name, template_payload), (concrete_name, concrete_payload)) in
                template_variants.iter().zip(concrete_variants.iter())
            {
                if template_name != concrete_name {
                    return Err(format!(
                        "entry variants '{template_name}' and '{concrete_name}' do not match"
                    ));
                }
                match (template_payload, concrete_payload) {
                    (Some(template_payload), Some(concrete_payload)) => {
                        unify_template_type(
                            type_table,
                            *template_payload,
                            *concrete_payload,
                            bindings,
                        )?;
                    }
                    (None, None) => {}
                    _ => {
                        return Err(format!(
                            "entry variant '{template_name}' disagrees about its payload"
                        ))
                    }
                }
            }
            Ok(())
        }
        (LoweredType::Routine(template_signature), LoweredType::Routine(concrete_signature))
            if template_signature.params.len() == concrete_signature.params.len() =>
        {
            for (template_param, concrete_param) in template_signature
                .params
                .iter()
                .zip(concrete_signature.params.iter())
            {
                unify_template_type(type_table, *template_param, *concrete_param, bindings)?;
            }
            match (template_signature.return_type, concrete_signature.return_type) {
                (Some(template_return), Some(concrete_return)) => {
                    unify_template_type(type_table, template_return, concrete_return, bindings)?;
                }
                (None, None) => {}
                _ => return Err("routine types disagree about their return type".to_string()),
            }
            match (template_signature.error_type, concrete_signature.error_type) {
                (Some(template_error), Some(concrete_error)) => {
                    unify_template_type(type_table, template_error, concrete_error, bindings)
                }
                (None, None) => Ok(()),
                _ => Err("routine types disagree about their error type".to_string()),
            }
        }
        _ => Err(format!(
            "template type {} does not match concrete type {}",
            template_type.0, concrete_type.0
        )),
    }
}

fn instantiate_template(
    packages: &mut BTreeMap<PackageIdentity, LoweredPackage>,
    type_table: &mut LoweredTypeTable,
    next_routine_index: &mut usize,
    template: &Template,
    bindings: &GenericBindings,
) -> Result<LoweredRoutineId, LoweringError> {
    let mut routine = template.routine.clone();
    routine.id = LoweredRoutineId(*next_routine_index);
    *next_routine_index += 1;

    let mut memo = BTreeMap::new();
    for local in routine.locals.iter_mut() {
        local.type_id = local
            .type_id
            .map(|type_id| substitute_type(type_table, bindings, &mut memo, type_id))
            .transpose()?;
    }
    routine.signature = routine
        .signature
        .map(|signature| substitute_type(type_table, bindings, &mut memo, signature))
        .transpose()?;
    routine.receiver_type = routine
        .receiver_type
        .map(|receiver_type| substitute_type(type_table, bindings, &mut memo, receiver_type))
        .transpose()?;
    for instr in routine.instructions.iter_mut() {
        match &mut instr.kind {
            LoweredInstrKind::Call { error_type, .. }
            | LoweredInstrKind::CallIndirect { error_type, .. } => {
                *error_type = error_type
                    .map(|type_id| substitute_type(type_table, bindings, &mut memo, type_id))
                    .transpose()?;
            }
            LoweredInstrKind::ConstructRecord { type_id, .. }
            | LoweredInstrKind::ConstructEntry { type_id, .. }
            | LoweredInstrKind::ConstructLinear { type_id, .. }
            | LoweredInstrKind::ConstructSet { type_id, .. }
            | LoweredInstrKind::ConstructMap { type_id, .. }
            | LoweredInstrKind::ConstructOptional { type_id, .. }
            | LoweredInstrKind::ConstructError { type_id, .. }
            | LoweredInstrKind::Cast {
                target_type: type_id,
                ..
            } => {
                *type_id = substitute_type(type_table, bindings, &mut memo, *type_id)?;
            }
            _ => {}
        }
    }

    resolve_constraint_calls(packages, type_table, template, &mut routine)?;
    synthesize_missing_structural_decls(packages, type_table, template, &routine)?;

    let routine_id = routine.id;
    let Some(package) = packages.get_mut(&template.owner) else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "generic receiver routine '{}' lost its owning package during monomorphization",
                template.routine.name
            ),
        ));
    };
    package.routine_decls.insert(routine_id, routine);
    Ok(routine_id)
}

fn substitute_type(
    type_table: &mut LoweredTypeTable,
    bindings: &GenericBindings,
    memo: &mut BTreeMap<LoweredTypeId, LoweredTypeId>,
    type_id: LoweredTypeId,
) -> Result<LoweredTypeId, LoweringError> {
    if let Some(existing) = memo.get(&type_id) {
        return Ok(*existing);
    }

    let Some(lowered_type) = type_table.get(type_id).cloned() else {
        return Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!("lowered type {} disappeared during monomorphization", type_id.0),
        ));
    };

    let substituted = match lowered_type {
        LoweredType::GenericParameter { name } => {
            *bindings.get(&name).ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "generic parameter '{name}' could not be monomorphized: no concrete type \
                         was inferred for it. It must be fixed by the call arguments (directly or \
                         through a nested generic call whose arguments determine it)"
                    ),
                )
            })?
        }
        LoweredType::Builtin(_) => type_id,
        LoweredType::Array { element_type, size } => {
            let element_type = substitute_type(type_table, bindings, memo, element_type)?;
            type_table.intern(LoweredType::Array { element_type, size })
        }
        LoweredType::Vector { element_type } => {
            let element_type = substitute_type(type_table, bindings, memo, element_type)?;
            type_table.intern(LoweredType::Vector { element_type })
        }
        LoweredType::Sequence { element_type } => {
            let element_type = substitute_type(type_table, bindings, memo, element_type)?;
            type_table.intern(LoweredType::Sequence { element_type })
        }
        LoweredType::Set { member_types } => {
            let member_types = member_types
                .into_iter()
                .map(|member_type| substitute_type(type_table, bindings, memo, member_type))
                .collect::<Result<Vec<_>, _>>()?;
            type_table.intern(LoweredType::Set { member_types })
        }
        LoweredType::Map {
            key_type,
            value_type,
        } => {
            let key_type = substitute_type(type_table, bindings, memo, key_type)?;
            let value_type = substitute_type(type_table, bindings, memo, value_type)?;
            type_table.intern(LoweredType::Map {
                key_type,
                value_type,
            })
        }
        LoweredType::Optional { inner } => {
            let inner = substitute_type(type_table, bindings, memo, inner)?;
            type_table.intern(LoweredType::Optional { inner })
        }
        LoweredType::Error { inner } => {
            let inner = inner
                .map(|inner| substitute_type(type_table, bindings, memo, inner))
                .transpose()?;
            type_table.intern(LoweredType::Error { inner })
        }
        LoweredType::Record { fields } => {
            let fields = fields
                .into_iter()
                .map(|(field_name, field_type)| {
                    substitute_type(type_table, bindings, memo, field_type)
                        .map(|field_type| (field_name, field_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            type_table.intern(LoweredType::Record { fields })
        }
        LoweredType::Entry { variants } => {
            let variants = variants
                .into_iter()
                .map(|(variant_name, variant_type)| {
                    variant_type
                        .map(|variant_type| {
                            substitute_type(type_table, bindings, memo, variant_type)
                        })
                        .transpose()
                        .map(|variant_type| (variant_name, variant_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            type_table.intern(LoweredType::Entry { variants })
        }
        LoweredType::Routine(signature) => {
            let params = signature
                .params
                .into_iter()
                .map(|param| substitute_type(type_table, bindings, memo, param))
                .collect::<Result<Vec<_>, _>>()?;
            let return_type = signature
                .return_type
                .map(|return_type| substitute_type(type_table, bindings, memo, return_type))
                .transpose()?;
            let error_type = signature
                .error_type
                .map(|error_type| substitute_type(type_table, bindings, memo, error_type))
                .transpose()?;
            type_table.intern(LoweredType::Routine(LoweredRoutineType {
                params,
                return_type,
                error_type,
            }))
        }
    };

    memo.insert(type_id, substituted);
    Ok(substituted)
}


/// Rewrite every deferred constraint call in a freshly instantiated clone
/// into an ordinary direct call: the receiver argument's substituted type is
/// now concrete, so the conformer's receiver routine (or the standard's
/// default body) can be looked up directly.
fn resolve_constraint_calls(
    packages: &BTreeMap<PackageIdentity, LoweredPackage>,
    _type_table: &LoweredTypeTable,
    template: &Template,
    routine: &mut LoweredRoutine,
) -> Result<(), LoweringError> {
    let mut rewrites: Vec<(usize, LoweredRoutineId, Vec<crate::ids::LoweredLocalId>, Option<LoweredTypeId>)> = Vec::new();
    for (index, instr) in routine.instructions.iter().enumerate() {
        let LoweredInstrKind::ConstraintCall { method, args, error_type } = &instr.kind else {
            continue;
        };
        let receiver_local = args.first().copied().ok_or_else(|| {
            LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "constraint call '{method}' in routine '{}' has no receiver argument",
                    template.routine.name
                ),
            )
        })?;
        let receiver_type = routine
            .locals
            .get(receiver_local)
            .and_then(|local| local.type_id)
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "constraint call '{method}' in routine '{}' has an untyped receiver",
                        template.routine.name
                    ),
                )
            })?;

        // Explicit receiver routines on the concrete conformer win; the
        // standard's default body (a receiver-less routine) is the fallback
        // and takes the call without the receiver argument.
        let mut explicit_target: Option<LoweredRoutineId> = None;
        let mut default_target: Option<LoweredRoutineId> = None;
        for package in packages.values() {
            for candidate in package.routine_decls.values() {
                if candidate.name != *method {
                    continue;
                }
                match candidate.receiver_type {
                    Some(candidate_receiver) if candidate_receiver == receiver_type => {
                        explicit_target = Some(candidate.id);
                    }
                    None => {
                        default_target.get_or_insert(candidate.id);
                    }
                    _ => {}
                }
            }
        }
        let (callee, call_args) = match (explicit_target, default_target) {
            (Some(callee), _) => (callee, args.clone()),
            (None, Some(callee)) => (callee, args[1..].to_vec()),
            (None, None) => {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "constraint call '{method}' in routine '{}' does not resolve to a conformer routine or standard default body after monomorphization",
                        template.routine.name
                    ),
                ))
            }
        };
        rewrites.push((index, callee, call_args, *error_type));
    }

    for (index, callee, args, error_type) in rewrites {
        let instr_id = crate::ids::LoweredInstrId(index);
        if let Some(instr) = routine.instructions.get_mut(instr_id) {
            instr.kind = LoweredInstrKind::Call { callee, args, error_type };
        }
    }
    Ok(())
}

/// Substitution can produce concrete record/entry shapes that no call site
/// mentioned directly (for example a record only constructed inside the
/// template body). The backend requires every named runtime type to map to a
/// lowered type declaration, so mint the missing structural declarations the
/// same way `synthesize_structural_runtime_type_declarations` does.
fn synthesize_missing_structural_decls(
    packages: &mut BTreeMap<PackageIdentity, LoweredPackage>,
    type_table: &LoweredTypeTable,
    template: &Template,
    routine: &LoweredRoutine,
) -> Result<(), LoweringError> {
    let mut used_types = BTreeSet::new();
    for local in routine.locals.iter() {
        if let Some(type_id) = local.type_id {
            collect_named_structural_types(type_table, type_id, &mut used_types);
        }
    }

    let known_runtime_types = packages
        .values()
        .flat_map(|package| {
            package
                .type_decls
                .values()
                .map(|type_decl| type_decl.runtime_type)
        })
        .collect::<BTreeSet<_>>();

    let source_unit_id = routine
        .source_unit_id
        .or(template.routine.source_unit_id)
        .unwrap_or(SourceUnitId(0));
    let Some(package) = packages.get_mut(&template.owner) else {
        return Ok(());
    };

    for type_id in used_types {
        if known_runtime_types.contains(&type_id) {
            continue;
        }
        let Some(lowered_type) = type_table.get(type_id) else {
            continue;
        };
        let (name, kind) = match lowered_type {
            LoweredType::Record { fields } => (
                format!("record_t{}", type_id.0),
                LoweredTypeDeclKind::Record {
                    fields: fields
                        .iter()
                        .map(|(field_name, field_type)| LoweredFieldLayout {
                            name: field_name.clone(),
                            type_id: *field_type,
                        })
                        .collect(),
                },
            ),
            LoweredType::Entry { variants } => (
                format!("entry_t{}", type_id.0),
                LoweredTypeDeclKind::Entry {
                    variants: variants
                        .iter()
                        .map(|(variant_name, variant_type)| LoweredVariantLayout {
                            name: variant_name.clone(),
                            payload_type: *variant_type,
                        })
                        .collect(),
                },
            ),
            _ => continue,
        };
        let symbol_id = SymbolId(usize::MAX - type_id.0);
        package.type_decls.insert(
            symbol_id,
            LoweredTypeDecl {
                symbol_id,
                source_unit_id,
                name,
                runtime_type: type_id,
                kind,
            },
        );
    }

    Ok(())
}

fn collect_named_structural_types(
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
    out: &mut BTreeSet<LoweredTypeId>,
) {
    let Some(lowered_type) = type_table.get(type_id) else {
        return;
    };
    match lowered_type {
        LoweredType::Record { fields } => {
            if out.insert(type_id) {
                for field_type in fields.values() {
                    collect_named_structural_types(type_table, *field_type, out);
                }
            }
        }
        LoweredType::Entry { variants } => {
            if out.insert(type_id) {
                for variant_type in variants.values().flatten() {
                    collect_named_structural_types(type_table, *variant_type, out);
                }
            }
        }
        LoweredType::Array { element_type, .. }
        | LoweredType::Vector { element_type }
        | LoweredType::Sequence { element_type }
        | LoweredType::Optional {
            inner: element_type,
        } => collect_named_structural_types(type_table, *element_type, out),
        LoweredType::Error { inner } => {
            if let Some(inner) = inner {
                collect_named_structural_types(type_table, *inner, out);
            }
        }
        LoweredType::Set { member_types } => {
            for member_type in member_types {
                collect_named_structural_types(type_table, *member_type, out);
            }
        }
        LoweredType::Map {
            key_type,
            value_type,
        } => {
            collect_named_structural_types(type_table, *key_type, out);
            collect_named_structural_types(type_table, *value_type, out);
        }
        LoweredType::Routine(signature) => {
            for param in &signature.params {
                collect_named_structural_types(type_table, *param, out);
            }
            if let Some(return_type) = signature.return_type {
                collect_named_structural_types(type_table, return_type, out);
            }
            if let Some(error_type) = signature.error_type {
                collect_named_structural_types(type_table, error_type, out);
            }
        }
        LoweredType::Builtin(_) | LoweredType::GenericParameter { .. } => {}
    }
}

fn patch_call_target(
    packages: &mut BTreeMap<PackageIdentity, LoweredPackage>,
    identity: &PackageIdentity,
    routine_id: LoweredRoutineId,
    instr_id: LoweredInstrId,
    concrete_id: LoweredRoutineId,
) {
    let Some(instr) = packages
        .get_mut(identity)
        .and_then(|package| package.routine_decls.get_mut(&routine_id))
        .and_then(|routine| routine.instructions.get_mut(instr_id))
    else {
        return;
    };
    if let LoweredInstrKind::Call { callee, .. } = &mut instr.kind {
        *callee = concrete_id;
    }
}

fn type_contains_generic_parameter(
    type_table: &LoweredTypeTable,
    type_id: LoweredTypeId,
) -> bool {
    let Some(lowered_type) = type_table.get(type_id) else {
        return false;
    };
    match lowered_type {
        LoweredType::GenericParameter { .. } => true,
        LoweredType::Builtin(_) => false,
        LoweredType::Array { element_type, .. }
        | LoweredType::Vector { element_type }
        | LoweredType::Sequence { element_type }
        | LoweredType::Optional {
            inner: element_type,
        } => type_contains_generic_parameter(type_table, *element_type),
        LoweredType::Error { inner } => inner
            .is_some_and(|inner| type_contains_generic_parameter(type_table, inner)),
        LoweredType::Set { member_types } => member_types
            .iter()
            .any(|member_type| type_contains_generic_parameter(type_table, *member_type)),
        LoweredType::Map {
            key_type,
            value_type,
        } => {
            type_contains_generic_parameter(type_table, *key_type)
                || type_contains_generic_parameter(type_table, *value_type)
        }
        LoweredType::Record { fields } => fields
            .values()
            .any(|field_type| type_contains_generic_parameter(type_table, *field_type)),
        LoweredType::Entry { variants } => variants
            .values()
            .flatten()
            .any(|variant_type| type_contains_generic_parameter(type_table, *variant_type)),
        LoweredType::Routine(signature) => {
            signature
                .params
                .iter()
                .any(|param| type_contains_generic_parameter(type_table, *param))
                || signature
                    .return_type
                    .is_some_and(|return_type| {
                        type_contains_generic_parameter(type_table, return_type)
                    })
                || signature
                    .error_type
                    .is_some_and(|error_type| {
                        type_contains_generic_parameter(type_table, error_type)
                    })
        }
    }
}

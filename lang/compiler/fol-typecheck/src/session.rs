use crate::{
    decls, exprs, CheckedType, CheckedTypeId, TypecheckConfig, TypecheckError, TypecheckErrorKind,
    TypecheckResult, TypedExportMount, TypedPackage, TypedProgram, TypedWorkspace,
};
use fol_resolver::{MountedSymbolProvenance, PackageIdentity, ScopeKind, SymbolId, SymbolKind};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default)]
pub struct TypecheckSession {
    config: TypecheckConfig,
}

impl TypecheckSession {
    pub fn new() -> Self {
        Self::with_config(TypecheckConfig::default())
    }

    pub fn with_config(config: TypecheckConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> TypecheckConfig {
        self.config
    }

    pub fn check_resolved_program(
        &mut self,
        resolved: fol_resolver::ResolvedProgram,
    ) -> TypecheckResult<TypedProgram> {
        validate_import_capability_model(&resolved, self.config.capability_model)?;
        validate_implicit_closure_captures(&resolved)?;
        let mut typed =
            TypedProgram::from_resolved_with_model(resolved, self.config.capability_model);
        decls::lower_declaration_signatures(&mut typed)?;
        exprs::type_program(&mut typed)?;
        Ok(typed)
    }

    pub fn check_resolved_workspace(
        &mut self,
        resolved: fol_resolver::ResolvedWorkspace,
    ) -> TypecheckResult<TypedWorkspace> {
        let mut typed_packages = BTreeMap::new();
        let mut in_progress = BTreeSet::new();
        let identities = resolved
            .packages()
            .map(|package| package.identity.clone())
            .collect::<Vec<_>>();
        let mut errors = Vec::new();

        for identity in identities {
            if let Err(mut package_errors) = self.check_workspace_package(
                &resolved,
                &identity,
                &mut typed_packages,
                &mut in_progress,
            ) {
                errors.append(&mut package_errors);
            }
        }

        if errors.is_empty() {
            Ok(TypedWorkspace::new(
                self.config.capability_model,
                resolved.entry_identity().clone(),
                typed_packages,
            ))
        } else {
            Err(errors)
        }
    }

    fn check_workspace_package(
        &mut self,
        workspace: &fol_resolver::ResolvedWorkspace,
        identity: &fol_resolver::PackageIdentity,
        typed_packages: &mut BTreeMap<fol_resolver::PackageIdentity, TypedPackage>,
        in_progress: &mut BTreeSet<fol_resolver::PackageIdentity>,
    ) -> TypecheckResult<()> {
        if typed_packages.contains_key(identity) {
            return Ok(());
        }

        if !in_progress.insert(identity.clone()) {
            return Err(vec![TypecheckError::new(
                TypecheckErrorKind::Internal,
                format!(
                    "typecheck workspace entered a package cycle at '{}'",
                    identity.canonical_root
                ),
            )]);
        }

        let package = workspace.package(identity).ok_or_else(|| {
            vec![TypecheckError::new(
                TypecheckErrorKind::Internal,
                format!(
                    "resolved workspace lost package '{}'",
                    identity.canonical_root
                ),
            )]
        })?;

        let dependency_identities = package
            .program
            .symbols
            .iter()
            .filter_map(|symbol| symbol.mounted_from.as_ref())
            .map(|provenance| provenance.package_identity.clone())
            .filter(|dependency| dependency != identity)
            .collect::<BTreeSet<_>>();

        let mut errors = Vec::new();
        for dependency in dependency_identities {
            if workspace.package(&dependency).is_none() {
                continue;
            }
            if let Err(mut dependency_errors) =
                self.check_workspace_package(workspace, &dependency, typed_packages, in_progress)
            {
                errors.append(&mut dependency_errors);
            }
        }

        // Import capability legality belongs to the importing package and is
        // independent of whether a dependency also failed to typecheck. Keep
        // the source-file diagnostic available instead of letting an earlier
        // dependency error suppress it (and, in editor routing, get filtered
        // out because it points at another package).
        if let Err(mut package_errors) =
            validate_import_capability_model(&package.program, self.config.capability_model)
        {
            errors.append(&mut package_errors);
        }
        if let Err(mut package_errors) = validate_implicit_closure_captures(&package.program) {
            errors.append(&mut package_errors);
        }

        if errors.is_empty() {
            let mut typed = TypedProgram::from_resolved_with_model(
                package.program.clone(),
                self.config.capability_model,
            );
            if let Err(mut package_errors) =
                self.hydrate_mounted_symbol_types(&mut typed, typed_packages)
            {
                errors.append(&mut package_errors);
            } else if let Err(mut package_errors) = decls::lower_declaration_signatures(&mut typed)
            {
                errors.append(&mut package_errors);
            } else if let Err(mut package_errors) = exprs::type_program(&mut typed) {
                errors.append(&mut package_errors);
            } else {
                typed_packages.insert(
                    identity.clone(),
                    TypedPackage::new(
                        identity.clone(),
                        package
                            .prepared
                            .exports
                            .iter()
                            .map(|mount| TypedExportMount {
                                source_namespace: mount.source_namespace.clone(),
                                mounted_namespace_suffix: mount.mounted_namespace_suffix.clone(),
                            })
                            .collect(),
                        typed,
                    ),
                );
            }
        }

        in_progress.remove(identity);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn hydrate_mounted_symbol_types(
        &mut self,
        typed: &mut TypedProgram,
        typed_packages: &BTreeMap<PackageIdentity, TypedPackage>,
    ) -> TypecheckResult<()> {
        let mounted_symbols = typed
            .resolved()
            .symbols
            .iter_with_ids()
            .filter_map(|(symbol_id, symbol)| {
                symbol
                    .mounted_from
                    .as_ref()
                    .map(|provenance| (symbol_id, provenance.clone()))
            })
            .collect::<Vec<_>>();

        if mounted_symbols.is_empty() {
            return Ok(());
        }

        let mounted_symbol_map = mounted_symbols
            .iter()
            .map(|(local_symbol_id, provenance)| {
                (
                    (
                        provenance.package_identity.clone(),
                        provenance.foreign_symbol,
                    ),
                    *local_symbol_id,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut imported_cache = BTreeMap::new();
        let mut errors = Vec::new();

        for (local_symbol_id, provenance) in mounted_symbols {
            match self.import_mounted_symbol_type(
                typed,
                typed_packages,
                &mounted_symbol_map,
                &mut imported_cache,
                local_symbol_id,
                &provenance,
            ) {
                Ok(()) => {}
                Err(error) => errors.push(error),
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn import_mounted_symbol_type(
        &mut self,
        typed: &mut TypedProgram,
        typed_packages: &BTreeMap<PackageIdentity, TypedPackage>,
        mounted_symbol_map: &BTreeMap<(PackageIdentity, SymbolId), SymbolId>,
        imported_cache: &mut BTreeMap<(PackageIdentity, CheckedTypeId), CheckedTypeId>,
        local_symbol_id: SymbolId,
        provenance: &MountedSymbolProvenance,
    ) -> Result<(), TypecheckError> {
        let foreign_package = typed_packages
            .get(&provenance.package_identity)
            .ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::TypeImportFailed,
                    format!(
                        "type import failed: package '{}' not found in typed workspace \
                         while importing symbol {} for local symbol {}",
                        provenance.package_identity.canonical_root,
                        provenance.foreign_symbol.0,
                        local_symbol_id.0,
                    ),
                )
            })?;
        let foreign_type = foreign_package
            .program
            .typed_symbol(provenance.foreign_symbol)
            .ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::TypeImportFailed,
                    format!(
                        "type import failed: symbol {} in package '{}' has no typed entry \
                         (local symbol {})",
                        provenance.foreign_symbol.0,
                        provenance.package_identity.canonical_root,
                        local_symbol_id.0,
                    ),
                )
            })?;
        let foreign_declared_type = foreign_type.declared_type.ok_or_else(|| {
            TypecheckError::new(
                TypecheckErrorKind::TypeImportFailed,
                format!(
                    "type import failed: symbol {} in package '{}' has no declared type \
                     (local symbol {})",
                    provenance.foreign_symbol.0,
                    provenance.package_identity.canonical_root,
                    local_symbol_id.0,
                ),
            )
        })?;
        let translated = self.import_type_id(
            typed,
            &foreign_package.identity,
            &foreign_package.program,
            foreign_declared_type,
            mounted_symbol_map,
            imported_cache,
        )?;
        let translated_receiver = foreign_type
            .receiver_type
            .map(|receiver_type| {
                self.import_type_id(
                    typed,
                    &foreign_package.identity,
                    &foreign_package.program,
                    receiver_type,
                    mounted_symbol_map,
                    imported_cache,
                )
            })
            .transpose()?;
        let typed_symbol = typed.typed_symbol_mut(local_symbol_id).ok_or_else(|| {
            TypecheckError::new(
                TypecheckErrorKind::SymbolTableCorrupted,
                format!(
                    "symbol table corrupted: local symbol {} (imported from package '{}' symbol {}) \
                     is missing from the typed program",
                    local_symbol_id.0,
                    provenance.package_identity.canonical_root,
                    provenance.foreign_symbol.0,
                ),
            )
        })?;
        typed_symbol.declared_type = Some(translated);
        typed_symbol.receiver_type = translated_receiver;
        typed_symbol.param_defaults = foreign_type.param_defaults.clone();
        typed_symbol.generic_params = foreign_type.generic_params.clone();
        typed_symbol.generic_constraints = foreign_type.generic_constraints.clone();
        typed_symbol.is_channel_sender_capture = foreign_type.is_channel_sender_capture;
        typed_symbol.channel_receiver_params = foreign_type.channel_receiver_params.clone();
        Ok(())
    }

    fn import_type_id(
        &mut self,
        target_program: &mut TypedProgram,
        source_identity: &PackageIdentity,
        source_program: &TypedProgram,
        source_type_id: CheckedTypeId,
        mounted_symbol_map: &BTreeMap<(PackageIdentity, SymbolId), SymbolId>,
        imported_cache: &mut BTreeMap<(PackageIdentity, CheckedTypeId), CheckedTypeId>,
    ) -> Result<CheckedTypeId, TypecheckError> {
        if let Some(existing) = imported_cache.get(&(source_identity.clone(), source_type_id)) {
            return Ok(*existing);
        }

        let source_type = source_program
            .type_table()
            .get(source_type_id)
            .cloned()
            .ok_or_else(|| {
                TypecheckError::new(
                    TypecheckErrorKind::TypeImportFailed,
                    format!(
                        "type import failed: type {} is missing from package '{}' type table",
                        source_type_id.0, source_identity.canonical_root,
                    ),
                )
            })?;

        let translated = match source_type {
            CheckedType::Builtin(builtin) => {
                target_program.type_table_mut().intern_builtin(builtin)
            }
            CheckedType::Declared {
                symbol,
                name,
                kind,
                args,
            } => {
                // A generic instantiation carries type args in the SOURCE
                // program; translate each into the target program so the
                // imported instance keeps its nominal `(symbol, args)` identity.
                let translated_args = args
                    .iter()
                    .map(|arg| {
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            *arg,
                            mounted_symbol_map,
                            imported_cache,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if kind == crate::DeclaredTypeKind::GenericParameter {
                    // Generic parameters are opaque placeholders; expanding
                    // their declared type would chase a self-reference.
                    target_program
                        .type_table_mut()
                        .intern(CheckedType::Declared {
                            symbol,
                            name,
                            kind,
                            args: translated_args,
                        })
                } else if let Some(translated_symbol) = translated_symbol_id(
                    source_identity,
                    source_program,
                    symbol,
                    mounted_symbol_map,
                ) {
                    target_program
                        .type_table_mut()
                        .intern(CheckedType::Declared {
                            symbol: translated_symbol,
                            name,
                            kind,
                            args: translated_args,
                        })
                } else if let Some(expanded_type) = source_program
                    .typed_symbol(symbol)
                    .and_then(|typed_symbol| typed_symbol.declared_type)
                {
                    let shell_type =
                        target_program
                            .type_table_mut()
                            .intern(CheckedType::Declared {
                                symbol,
                                name,
                                kind,
                                args: translated_args,
                            });
                    // Guard against cyclic declared types: cache the shell
                    // before expanding so re-entry terminates.
                    imported_cache.insert((source_identity.clone(), source_type_id), shell_type);
                    // A generic instantiation's apparent shape is the source's
                    // own apparent override (its substituted record), not the
                    // generic template; import that if present, else the body.
                    let source_apparent = source_program
                        .apparent_type_override(source_type_id)
                        .unwrap_or(expanded_type);
                    let apparent_type = self.import_type_id(
                        target_program,
                        source_identity,
                        source_program,
                        source_apparent,
                        mounted_symbol_map,
                        imported_cache,
                    )?;
                    if apparent_type != shell_type {
                        target_program.record_apparent_type_override(shell_type, apparent_type);
                    }
                    shell_type
                } else {
                    target_program
                        .type_table_mut()
                        .intern(CheckedType::Declared {
                            symbol,
                            name,
                            kind,
                            args: translated_args,
                        })
                }
            }
            CheckedType::Array { element_type, size } => {
                let element_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    element_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Array { element_type, size })
            }
            CheckedType::Vector { element_type } => {
                let element_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    element_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Vector { element_type })
            }
            CheckedType::Sequence { element_type } => {
                let element_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    element_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Sequence { element_type })
            }
            CheckedType::Channel { element_type } => {
                let element_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    element_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Channel { element_type })
            }
            CheckedType::ChannelSender { element_type } => {
                let element_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    element_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::ChannelSender { element_type })
            }
            CheckedType::Eventual {
                value_type,
                error_type,
            } => {
                let value_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    value_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                let error_type = error_type
                    .map(|error_type| {
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            error_type,
                            mounted_symbol_map,
                            imported_cache,
                        )
                    })
                    .transpose()?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Eventual {
                        value_type,
                        error_type,
                    })
            }
            CheckedType::Set { member_types } => {
                let member_types = member_types
                    .into_iter()
                    .map(|member| {
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            member,
                            mounted_symbol_map,
                            imported_cache,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Set { member_types })
            }
            CheckedType::Map {
                key_type,
                value_type,
            } => {
                let key_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    key_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                let value_type = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    value_type,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program.type_table_mut().intern(CheckedType::Map {
                    key_type,
                    value_type,
                })
            }
            CheckedType::Optional { inner } => {
                let inner = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    inner,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Optional { inner })
            }
            CheckedType::Owned { inner } => {
                let inner = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    inner,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Owned { inner })
            }
            CheckedType::Borrowed { inner, mutable } => {
                let inner = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    inner,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Borrowed { inner, mutable })
            }
            CheckedType::Pointer { target, shared } => {
                let target = self.import_type_id(
                    target_program,
                    source_identity,
                    source_program,
                    target,
                    mounted_symbol_map,
                    imported_cache,
                )?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Pointer { target, shared })
            }
            CheckedType::Error { inner } => {
                let inner = inner
                    .map(|inner| {
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            inner,
                            mounted_symbol_map,
                            imported_cache,
                        )
                    })
                    .transpose()?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Error { inner })
            }
            CheckedType::Record { fields } => {
                let mut translated_fields = BTreeMap::new();
                for (field_name, field_type) in fields {
                    translated_fields.insert(
                        field_name,
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            field_type,
                            mounted_symbol_map,
                            imported_cache,
                        )?,
                    );
                }
                target_program.type_table_mut().intern(CheckedType::Record {
                    fields: translated_fields,
                })
            }
            CheckedType::Entry { variants } => {
                let mut translated_variants = BTreeMap::new();
                for (variant_name, variant_type) in variants {
                    translated_variants.insert(
                        variant_name,
                        variant_type
                            .map(|variant| {
                                self.import_type_id(
                                    target_program,
                                    source_identity,
                                    source_program,
                                    variant,
                                    mounted_symbol_map,
                                    imported_cache,
                                )
                            })
                            .transpose()?,
                    );
                }
                target_program.type_table_mut().intern(CheckedType::Entry {
                    variants: translated_variants,
                })
            }
            CheckedType::Routine(signature) => {
                let params = signature
                    .params
                    .into_iter()
                    .map(|param| {
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            param,
                            mounted_symbol_map,
                            imported_cache,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let return_type = signature
                    .return_type
                    .map(|return_type| {
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            return_type,
                            mounted_symbol_map,
                            imported_cache,
                        )
                    })
                    .transpose()?;
                let error_type = signature
                    .error_type
                    .map(|error_type| {
                        self.import_type_id(
                            target_program,
                            source_identity,
                            source_program,
                            error_type,
                            mounted_symbol_map,
                            imported_cache,
                        )
                    })
                    .transpose()?;
                target_program
                    .type_table_mut()
                    .intern(CheckedType::Routine(crate::RoutineType {
                        generic_params: Vec::new(),
                        generic_constraints: BTreeMap::new(),
                        param_names: signature.param_names.clone(),
                        param_defaults: signature.param_defaults.clone(),
                        variadic_index: signature.variadic_index,
                        mutex_params: signature.mutex_params.clone(),
                        params,
                        return_type,
                        error_type,
                    }))
            }
        };

        imported_cache.insert((source_identity.clone(), source_type_id), translated);
        Ok(translated)
    }
}

fn validate_implicit_closure_captures(
    resolved: &fol_resolver::ResolvedProgram,
) -> TypecheckResult<()> {
    fn nearest_routine_scope(
        resolved: &fol_resolver::ResolvedProgram,
        mut scope: fol_resolver::ScopeId,
    ) -> Option<fol_resolver::ScopeId> {
        loop {
            let resolved_scope = resolved.scope(scope)?;
            if resolved_scope.kind == ScopeKind::Routine {
                return Some(scope);
            }
            scope = resolved_scope.parent?;
        }
    }

    let mut errors = Vec::new();
    for reference in resolved.references.iter() {
        let Some(symbol_id) = reference.resolved else {
            continue;
        };
        let Some(symbol) = resolved.symbol(symbol_id) else {
            continue;
        };
        if !matches!(
            symbol.kind,
            SymbolKind::ValueBinding
                | SymbolKind::LabelBinding
                | SymbolKind::DestructureBinding
                | SymbolKind::Parameter
                | SymbolKind::Capture
                | SymbolKind::LoopBinder
                | SymbolKind::RollingBinder
        ) {
            continue;
        }
        let declaration_routine = nearest_routine_scope(resolved, symbol.scope);
        let reference_routine = nearest_routine_scope(resolved, reference.scope);
        if declaration_routine.is_none() || declaration_routine == reference_routine {
            continue;
        }
        let message = format!(
            "implicit closure capture of outer local '{}' is not supported; pass the value as a routine parameter instead",
            symbol.name
        );
        let origin = reference
            .syntax_id
            .and_then(|syntax_id| resolved.syntax_index().origin(syntax_id).cloned());
        errors.push(origin.map_or_else(
            || TypecheckError::new(TypecheckErrorKind::Unsupported, message.clone()),
            |origin| {
                TypecheckError::with_origin(
                    TypecheckErrorKind::Unsupported,
                    message.clone(),
                    origin,
                )
            },
        ));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_import_capability_model(
    resolved: &fol_resolver::ResolvedProgram,
    capability_model: crate::TypecheckCapabilityModel,
) -> TypecheckResult<()> {
    if capability_model != crate::TypecheckCapabilityModel::Core {
        return Ok(());
    }

    let mut errors = Vec::new();
    for import in resolved.imports.iter() {
        let Some(target_scope) = import.target_scope else {
            continue;
        };
        let Some(scope) = resolved.scope(target_scope) else {
            continue;
        };
        let fol_resolver::ScopeKind::ProgramRoot { package } = &scope.kind else {
            continue;
        };
        if package != "std" {
            continue;
        }
        let origin = resolved
            .symbol(import.alias_symbol)
            .and_then(|symbol| symbol.origin.clone());
        let message = format!(
            "bundled std imports require 'fol_model = memo'; current artifact model is '{}'",
            capability_model.as_str()
        );
        errors.push(match origin {
            Some(origin) => {
                TypecheckError::with_origin(TypecheckErrorKind::Unsupported, message, origin)
            }
            None => TypecheckError::new(TypecheckErrorKind::Unsupported, message),
        });
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn translated_symbol_id(
    source_identity: &PackageIdentity,
    source_program: &TypedProgram,
    symbol_id: SymbolId,
    mounted_symbol_map: &BTreeMap<(PackageIdentity, SymbolId), SymbolId>,
) -> Option<SymbolId> {
    let resolved_symbol = source_program.resolved().symbol(symbol_id)?;
    let translation_key = resolved_symbol
        .mounted_from
        .as_ref()
        .map(|provenance| {
            (
                provenance.package_identity.clone(),
                provenance.foreign_symbol,
            )
        })
        .unwrap_or_else(|| (source_identity.clone(), symbol_id));

    mounted_symbol_map.get(&translation_key).copied()
}

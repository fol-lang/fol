use crate::{
    control::{LoweredInstr, LoweredInstrKind, LoweredLocal, LoweredOperand},
    ids::{LoweredBlockId, LoweredInstrId, LoweredLocalId, LoweredTypeId},
    LoweredGlobalId, LoweredPackage, LoweredRoutine, LoweredRoutineId, LoweringError,
    LoweringErrorKind,
};
use fol_parser::ast::{AstNode, Literal};
use fol_resolver::{
    MountedSymbolProvenance, PackageIdentity, ResolvedSymbol, SourceUnitId, SymbolId, SymbolKind,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LoweredValue {
    pub local_id: LoweredLocalId,
    pub type_id: LoweredTypeId,
    pub recoverable_error_type: Option<LoweredTypeId>,
}

#[derive(Debug, Clone)]
pub(crate) struct EntryVariantLowering {
    pub type_id: LoweredTypeId,
    pub payload_type: Option<LoweredTypeId>,
    pub default: Option<AstNode>,
}

#[derive(Debug, Clone)]
pub(crate) struct RoutineDefaultLowering {
    pub package_identity: PackageIdentity,
    pub source_unit_id: SourceUnitId,
    pub scope_id: fol_resolver::ScopeId,
    pub defaults: Vec<Option<AstNode>>,
    pub variadic_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeferScopeKind {
    Ordinary,
    Loop,
}

#[derive(Debug, Clone)]
pub(crate) struct DeferredBody {
    pub source_unit_id: SourceUnitId,
    pub scope_id: fol_resolver::ScopeId,
    pub body: Vec<AstNode>,
    pub error_only: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ActiveDeferScope {
    pub kind: DeferScopeKind,
    pub entries: Vec<DeferredBody>,
}

#[derive(Debug, Default)]
pub(crate) struct WorkspaceDeclIndex {
    typed_packages: BTreeMap<PackageIdentity, fol_typecheck::TypedPackage>,
    globals: BTreeMap<(PackageIdentity, SymbolId), LoweredGlobalId>,
    routines: BTreeMap<(PackageIdentity, SymbolId), LoweredRoutineId>,
    routine_params: BTreeMap<LoweredRoutineId, Vec<LoweredTypeId>>,
    routine_param_names: BTreeMap<LoweredRoutineId, Vec<String>>,
    routine_param_defaults: BTreeMap<LoweredRoutineId, RoutineDefaultLowering>,
    routine_has_receiver: BTreeMap<LoweredRoutineId, bool>,
    entry_variants: BTreeMap<(PackageIdentity, SymbolId, String), EntryVariantLowering>,
    record_fields: BTreeMap<(PackageIdentity, SymbolId, String), LoweredTypeId>,
    record_runtime_types: BTreeMap<(PackageIdentity, SymbolId), LoweredTypeId>,
}

impl WorkspaceDeclIndex {
    pub(crate) fn from_workspace(
        typed: &fol_typecheck::TypedWorkspace,
        packages: &BTreeMap<PackageIdentity, LoweredPackage>,
    ) -> Self {
        let mut index = Self::default();
        for typed_package in typed.packages() {
            index
                .typed_packages
                .insert(typed_package.identity.clone(), typed_package.clone());
            let Some(lowered_package) = packages.get(&typed_package.identity) else {
                continue;
            };
            index.extend_package(typed_package, lowered_package);
        }
        index
    }

    #[cfg(test)]
    pub(crate) fn build(workspace: &crate::LoweredWorkspace) -> Self {
        let mut index = Self::default();
        for package in workspace.packages() {
            for global in package.global_decls.values() {
                index
                    .globals
                    .insert((package.identity.clone(), global.symbol_id), global.id);
            }
            for routine in package.routine_decls.values() {
                if let Some(symbol_id) = routine.symbol_id {
                    index
                        .routines
                        .insert((package.identity.clone(), symbol_id), routine.id);
                }
                let params = routine
                    .params
                    .iter()
                    .filter_map(|param| routine.locals.get(*param).and_then(|local| local.type_id))
                    .collect::<Vec<_>>();
                let param_names = routine
                    .params
                    .iter()
                    .filter_map(|param| {
                        routine
                            .locals
                            .get(*param)
                            .and_then(|local| local.name.clone())
                    })
                    .collect::<Vec<_>>();
                index.routine_params.insert(routine.id, params);
                index.routine_param_names.insert(routine.id, param_names);
            }
        }
        index
    }

    pub(crate) fn extend_package(
        &mut self,
        typed_package: &fol_typecheck::TypedPackage,
        package: &LoweredPackage,
    ) {
        for global in package.global_decls.values() {
            self.globals
                .insert((package.identity.clone(), global.symbol_id), global.id);
        }
        for routine in package.routine_decls.values() {
            if let Some(symbol_id) = routine.symbol_id {
                self.routines
                    .insert((package.identity.clone(), symbol_id), routine.id);
                if let Some(typed_symbol) = typed_package.program.typed_symbol(symbol_id) {
                    if let Some(signature_type) = typed_symbol
                        .declared_type
                        .and_then(|type_id| typed_package.program.type_table().get(type_id))
                    {
                        if let fol_typecheck::CheckedType::Routine(signature) = signature_type {
                            let mut defaults = signature.param_defaults.clone();
                            let mut variadic_index = signature.variadic_index;
                            if typed_symbol.receiver_type.is_some() {
                                defaults.insert(0, None);
                                variadic_index = variadic_index.map(|index| index + 1);
                            }
                            self.routine_param_defaults.insert(
                                routine.id,
                                RoutineDefaultLowering {
                                    package_identity: package.identity.clone(),
                                    source_unit_id: typed_symbol.source_unit_id,
                                    scope_id: typed_symbol.scope_id,
                                    defaults,
                                    variadic_index,
                                },
                            );
                        }
                    }
                }
            }
            let params = routine
                .params
                .iter()
                .filter_map(|param| routine.locals.get(*param).and_then(|local| local.type_id))
                .collect::<Vec<_>>();
            let param_names = routine
                .params
                .iter()
                .filter_map(|param| {
                    routine
                        .locals
                        .get(*param)
                        .and_then(|local| local.name.clone())
                })
                .collect::<Vec<_>>();
            self.routine_params.insert(routine.id, params);
            self.routine_param_names.insert(routine.id, param_names);
            self.routine_has_receiver
                .insert(routine.id, routine.receiver_type.is_some());
        }
        self.extend_entry_variants(typed_package, package);
        for (symbol_id, type_decl) in &package.type_decls {
            if let crate::LoweredTypeDeclKind::Record { fields } = &type_decl.kind {
                self.record_runtime_types.insert(
                    (package.identity.clone(), *symbol_id),
                    type_decl.runtime_type,
                );
                for field in fields {
                    self.record_fields.insert(
                        (package.identity.clone(), *symbol_id, field.name.clone()),
                        field.type_id,
                    );
                }
            }
        }
    }

    pub(crate) fn routine_has_receiver(&self, routine_id: LoweredRoutineId) -> bool {
        self.routine_has_receiver
            .get(&routine_id)
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn global_id_for_symbol(
        &self,
        identity: &PackageIdentity,
        symbol_id: SymbolId,
    ) -> Option<LoweredGlobalId> {
        self.globals.get(&(identity.clone(), symbol_id)).copied()
    }

    pub(crate) fn routine_id_for_symbol(
        &self,
        identity: &PackageIdentity,
        symbol_id: SymbolId,
    ) -> Option<LoweredRoutineId> {
        self.routines.get(&(identity.clone(), symbol_id)).copied()
    }

    pub(crate) fn routine_param_types(
        &self,
        routine_id: LoweredRoutineId,
    ) -> Option<&[LoweredTypeId]> {
        self.routine_params.get(&routine_id).map(Vec::as_slice)
    }

    pub(crate) fn routine_param_names(&self, routine_id: LoweredRoutineId) -> Option<&[String]> {
        self.routine_param_names.get(&routine_id).map(Vec::as_slice)
    }

    pub(crate) fn routine_param_defaults(
        &self,
        routine_id: LoweredRoutineId,
    ) -> Option<&RoutineDefaultLowering> {
        self.routine_param_defaults.get(&routine_id)
    }

    pub(crate) fn typed_package(
        &self,
        identity: &PackageIdentity,
    ) -> Option<&fol_typecheck::TypedPackage> {
        self.typed_packages.get(identity)
    }

    pub(crate) fn entry_variant(
        &self,
        identity: &PackageIdentity,
        symbol_id: SymbolId,
        variant: &str,
    ) -> Option<&EntryVariantLowering> {
        self.entry_variants
            .get(&(identity.clone(), symbol_id, variant.to_string()))
    }

    pub(crate) fn record_field(
        &self,
        identity: &PackageIdentity,
        symbol_id: SymbolId,
        field: &str,
    ) -> Option<LoweredTypeId> {
        self.record_fields
            .get(&(identity.clone(), symbol_id, field.to_string()))
            .copied()
    }

    pub(crate) fn record_runtime_type(
        &self,
        identity: &PackageIdentity,
        symbol_id: SymbolId,
    ) -> Option<LoweredTypeId> {
        self.record_runtime_types
            .get(&(identity.clone(), symbol_id))
            .copied()
    }

    pub(crate) fn record_fields(
        &self,
        identity: &PackageIdentity,
        symbol_id: SymbolId,
    ) -> BTreeMap<String, LoweredTypeId> {
        self.record_fields
            .iter()
            .filter_map(|((package, symbol, field), type_id)| {
                (package == identity && *symbol == symbol_id).then_some((field.clone(), *type_id))
            })
            .collect()
    }

    fn extend_entry_variants(
        &mut self,
        typed_package: &fol_typecheck::TypedPackage,
        package: &LoweredPackage,
    ) {
        for (source_unit_index, source_unit) in typed_package
            .program
            .resolved()
            .syntax()
            .source_units
            .iter()
            .enumerate()
        {
            if source_unit.kind == fol_parser::ast::ParsedSourceUnitKind::Build {
                continue;
            }
            let source_unit_id = SourceUnitId(source_unit_index);
            for item in &source_unit.items {
                let AstNode::TypeDecl {
                    name,
                    type_def: fol_parser::ast::TypeDefinition::Entry { variant_meta, .. },
                    ..
                } = &item.node
                else {
                    continue;
                };
                let Some(symbol_id) = crate::decls::find_local_symbol_id(
                    &typed_package.program,
                    source_unit_id,
                    SymbolKind::Type,
                    name,
                ) else {
                    continue;
                };
                let Some(type_decl) = package.type_decls.get(&symbol_id) else {
                    continue;
                };
                let crate::LoweredTypeDeclKind::Entry { variants } = &type_decl.kind else {
                    continue;
                };
                for variant in variants {
                    let default = variant_meta
                        .get(&variant.name)
                        .and_then(|meta| meta.default.clone());
                    self.entry_variants.insert(
                        (package.identity.clone(), symbol_id, variant.name.clone()),
                        EntryVariantLowering {
                            type_id: type_decl.runtime_type,
                            payload_type: variant.payload_type,
                            default,
                        },
                    );
                }
            }
        }
    }
}

pub(crate) struct RoutineCursor<'a> {
    pub(crate) routine: &'a mut LoweredRoutine,
    pub(crate) block_id: LoweredBlockId,
    next_local_index: usize,
    next_instr_index: usize,
    next_block_index: usize,
    pub(crate) loop_exit_blocks: Vec<LoweredBlockId>,
    pub(crate) defer_scopes: Vec<ActiveDeferScope>,
    pub(crate) anonymous_routines: Vec<LoweredRoutine>,
    pub(crate) next_routine_index: usize,
}

impl<'a> RoutineCursor<'a> {
    pub(crate) fn new(routine: &'a mut LoweredRoutine, block_id: LoweredBlockId) -> Self {
        Self {
            next_local_index: routine.locals.len(),
            next_instr_index: routine.instructions.len(),
            next_block_index: routine.blocks.len(),
            routine,
            block_id,
            loop_exit_blocks: Vec::new(),
            defer_scopes: Vec::new(),
            anonymous_routines: Vec::new(),
            next_routine_index: 0,
        }
    }

    pub(crate) fn switch_block(&mut self, block_id: LoweredBlockId) -> Result<(), LoweringError> {
        if self.routine.blocks.get(block_id).is_none() {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' lost block {}",
                    self.routine.name, block_id.0
                ),
            ));
        }
        self.block_id = block_id;
        Ok(())
    }

    pub(crate) fn create_block(&mut self) -> LoweredBlockId {
        let block_id = self.routine.blocks.push(crate::LoweredBlock {
            id: LoweredBlockId(self.next_block_index),
            instructions: Vec::new(),
            terminator: None,
        });
        self.next_block_index += 1;
        block_id
    }

    pub(crate) fn push_loop_exit(&mut self, block_id: LoweredBlockId) {
        self.loop_exit_blocks.push(block_id);
    }

    pub(crate) fn pop_loop_exit(&mut self) -> Option<LoweredBlockId> {
        self.loop_exit_blocks.pop()
    }

    pub(crate) fn current_loop_exit(&self) -> Option<LoweredBlockId> {
        self.loop_exit_blocks.last().copied()
    }

    pub(crate) fn push_defer_scope(&mut self, kind: DeferScopeKind) {
        self.defer_scopes.push(ActiveDeferScope {
            kind,
            entries: Vec::new(),
        });
    }

    pub(crate) fn pop_defer_scope(&mut self) -> Option<ActiveDeferScope> {
        self.defer_scopes.pop()
    }

    pub(crate) fn defer_scope_depth(&self) -> usize {
        self.defer_scopes.len()
    }

    pub(crate) fn defer_scopes_snapshot(&self) -> Vec<ActiveDeferScope> {
        self.defer_scopes.clone()
    }

    pub(crate) fn nearest_loop_defer_depth(&self) -> Option<usize> {
        self.defer_scopes
            .iter()
            .rposition(|scope| scope.kind == DeferScopeKind::Loop)
    }

    pub(crate) fn register_defer(
        &mut self,
        source_unit_id: SourceUnitId,
        scope_id: fol_resolver::ScopeId,
        body: &[AstNode],
        error_only: bool,
    ) -> Result<(), LoweringError> {
        let Some(scope) = self.defer_scopes.last_mut() else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                "dfr registration requires an active dfr scope",
            ));
        };
        scope.entries.push(DeferredBody {
            source_unit_id,
            scope_id,
            body: body.to_vec(),
            error_only,
        });
        Ok(())
    }

    pub(crate) fn current_block_terminated(&self) -> Result<bool, LoweringError> {
        self.routine
            .blocks
            .get(self.block_id)
            .map(|block| block.is_terminated())
            .ok_or_else(|| {
                LoweringError::with_kind(
                    LoweringErrorKind::InvalidInput,
                    format!(
                        "lowered routine '{}' lost block {}",
                        self.routine.name, self.block_id.0
                    ),
                )
            })
    }

    pub(crate) fn terminate_current_block(
        &mut self,
        terminator: crate::LoweredTerminator,
    ) -> Result<(), LoweringError> {
        let Some(block) = self.routine.blocks.get_mut(self.block_id) else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' lost block {}",
                    self.routine.name, self.block_id.0
                ),
            ));
        };
        if block.terminator.is_some() {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' attempted to terminate block {} twice",
                    self.routine.name, self.block_id.0
                ),
            ));
        }
        block.terminator = Some(terminator);
        Ok(())
    }

    pub(crate) fn allocate_local(
        &mut self,
        type_id: LoweredTypeId,
        name: Option<String>,
    ) -> LoweredLocalId {
        let local_id = self.routine.locals.push(LoweredLocal {
            id: LoweredLocalId(self.next_local_index),
            type_id: Some(type_id),
            name,
        });
        self.next_local_index += 1;
        local_id
    }

    pub(crate) fn push_instr(
        &mut self,
        result: Option<LoweredLocalId>,
        kind: LoweredInstrKind,
    ) -> Result<LoweredInstrId, LoweringError> {
        if self.current_block_terminated()? {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' attempted to append instructions after block {} terminated",
                    self.routine.name, self.block_id.0
                ),
            ));
        }
        let instr_id = self.routine.instructions.push(LoweredInstr {
            id: LoweredInstrId(self.next_instr_index),
            result,
            kind,
        });
        self.next_instr_index += 1;
        let Some(block) = self.routine.blocks.get_mut(self.block_id) else {
            return Err(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "lowered routine '{}' lost block {}",
                    self.routine.name, self.block_id.0
                ),
            ));
        };
        block.instructions.push(instr_id);
        Ok(instr_id)
    }

    pub(crate) fn lower_literal(
        &mut self,
        literal: &Literal,
        type_id: LoweredTypeId,
    ) -> Result<LoweredValue, LoweringError> {
        let operand = match literal {
            Literal::Integer(value) => LoweredOperand::Int(*value),
            Literal::Float(value) => LoweredOperand::Float(value.to_bits()),
            Literal::String(value) => LoweredOperand::Str(value.clone()),
            Literal::Character(value) => LoweredOperand::Char(*value),
            Literal::Boolean(value) => LoweredOperand::Bool(*value),
            Literal::Nil => {
                return Err(LoweringError::with_kind(
                    LoweringErrorKind::Unsupported,
                    "nil lowering is part of the shell-lowering phase, not the core expression phase",
                ));
            }
        };
        let local_id = self.allocate_local(type_id, None);
        self.push_instr(Some(local_id), LoweredInstrKind::Const(operand))?;
        Ok(LoweredValue {
            local_id,
            type_id,
            recoverable_error_type: None,
        })
    }

    pub(crate) fn lower_identifier_reference(
        &mut self,
        current_identity: &PackageIdentity,
        decl_index: &WorkspaceDeclIndex,
        resolved_symbol: &ResolvedSymbol,
        result_type: LoweredTypeId,
    ) -> Result<LoweredValue, LoweringError> {
        if let Some(local_id) = self.routine.local_symbols.get(&resolved_symbol.id).copied() {
            let result_local = self.allocate_local(result_type, None);
            if self.routine.mutex_params.contains(&local_id) {
                self.routine.mutex_params.insert(result_local);
            }
            self.push_instr(
                Some(result_local),
                LoweredInstrKind::LoadLocal { local: local_id },
            )?;
            return Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: None,
            });
        }

        let (owning_identity, owning_symbol_id) = canonical_symbol_key(
            current_identity,
            resolved_symbol.mounted_from.as_ref(),
            resolved_symbol.id,
        );
        if let Some(global_id) = decl_index.global_id_for_symbol(&owning_identity, owning_symbol_id)
        {
            let result_local = self.allocate_local(result_type, None);
            self.push_instr(
                Some(result_local),
                LoweredInstrKind::LoadGlobal { global: global_id },
            )?;
            return Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: None,
            });
        }

        // Fall back to routine reference — the symbol may be a named routine
        // used as a function value (e.g. passed as an argument)
        if let Some(routine_id) =
            decl_index.routine_id_for_symbol(&owning_identity, owning_symbol_id)
        {
            let result_local = self.allocate_local(result_type, None);
            self.push_instr(
                Some(result_local),
                LoweredInstrKind::RoutineRef {
                    routine: routine_id,
                },
            )?;
            return Ok(LoweredValue {
                local_id: result_local,
                type_id: result_type,
                recoverable_error_type: None,
            });
        }

        Err(LoweringError::with_kind(
            LoweringErrorKind::InvalidInput,
            format!(
                "value symbol '{}' does not map to a lowered local, global, or routine definition",
                resolved_symbol.name
            ),
        ))
    }
}

pub(crate) fn canonical_symbol_key(
    current_identity: &PackageIdentity,
    mounted_from: Option<&MountedSymbolProvenance>,
    symbol_id: SymbolId,
) -> (PackageIdentity, SymbolId) {
    mounted_from
        .map(|provenance| {
            (
                provenance.package_identity.clone(),
                provenance.foreign_symbol,
            )
        })
        .unwrap_or_else(|| (current_identity.clone(), symbol_id))
}

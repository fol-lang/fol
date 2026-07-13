use crate::types::GenericConstraint;
use crate::{BuiltinTypeIds, CheckedTypeId, TypeTable, TypecheckCapabilityModel};
use fol_intrinsics::IntrinsicId;
use fol_parser::ast::{AstNode, ParsedSourceUnitKind, StandardKind, SyntaxNodeId, SyntaxOrigin};
use fol_resolver::{PackageIdentity, ReferenceKind, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoverableCallEffect {
    pub error_type: CheckedTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedExportMount {
    pub source_namespace: String,
    pub mounted_namespace_suffix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedSourceUnit {
    pub source_unit_id: SourceUnitId,
    pub path: String,
    pub package: String,
    pub namespace: String,
    pub kind: ParsedSourceUnitKind,
    pub scope_id: ScopeId,
    pub top_level_nodes: Vec<SyntaxNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedSymbol {
    pub symbol_id: SymbolId,
    pub kind: SymbolKind,
    pub scope_id: ScopeId,
    pub source_unit_id: SourceUnitId,
    pub declared_type: Option<CheckedTypeId>,
    pub receiver_type: Option<CheckedTypeId>,
    pub generic_params: Vec<SymbolId>,
    pub generic_constraints: BTreeMap<SymbolId, Vec<GenericConstraint>>,
    /// Mirrors the resolver's binding mutability (`var[mut]`/`lab[mut]`).
    /// Drives field-assignment legality.
    pub is_mutable: bool,
    pub is_mutex: bool,
    /// A channel capture written as `c[tx]` exposes only the sender endpoint
    /// inside its anonymous routine, even though lowering still carries the
    /// enclosing channel handle.
    pub is_channel_sender_capture: bool,
    /// Parameter positions that perform a receive through `c[rx]`. Direct
    /// spawn rejects these routines so the owning receiver stays single.
    pub channel_receiver_params: BTreeSet<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedNode {
    pub syntax_id: SyntaxNodeId,
    pub source_unit_id: SourceUnitId,
    pub inferred_type: Option<CheckedTypeId>,
    pub recoverable_effect: Option<RecoverableCallEffect>,
    pub intrinsic_id: Option<IntrinsicId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedReference {
    pub reference_id: fol_resolver::ReferenceId,
    pub kind: ReferenceKind,
    pub source_unit_id: SourceUnitId,
    pub resolved_type: Option<CheckedTypeId>,
    pub recoverable_effect: Option<RecoverableCallEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStandardRoutine {
    pub symbol_id: SymbolId,
    pub name: String,
    pub params: Vec<CheckedTypeId>,
    pub return_type: Option<CheckedTypeId>,
    pub error_type: Option<CheckedTypeId>,
    /// True when the required routine ships a default body that conformers
    /// inherit if they do not provide their own receiver routine with the
    /// exact signature.
    pub has_default_body: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStandardField {
    pub symbol_id: SymbolId,
    pub name: String,
    pub field_type: CheckedTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedStandard {
    pub symbol_id: SymbolId,
    pub scope_id: ScopeId,
    pub kind: StandardKind,
    /// Generic parameters of the standard itself. Empty when the
    /// standard is not parameterized. At each conformance site these
    /// parameters are substituted with the types supplied in the
    /// conformance header.
    pub generic_params: Vec<SymbolId>,
    pub required_routines: Vec<TypedStandardRoutine>,
    pub required_fields: Vec<TypedStandardField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedConformanceClaim {
    pub standard_symbol_id: SymbolId,
    /// Type arguments supplied at the conformance header, e.g. the
    /// `int` in `typ IntIter()(Iterator[int]): rec`. Empty when the
    /// conformer claims a non-generic standard.
    pub type_args: Vec<CheckedTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedConformance {
    pub type_symbol_id: SymbolId,
    pub standard_symbol_ids: Vec<SymbolId>,
    /// Claim-with-args list, parallel to `standard_symbol_ids` but
    /// carrying the concrete type arguments supplied at each
    /// conformance header.
    pub claims: Vec<TypedConformanceClaim>,
}

/// One field of a record type, in source declaration order, carrying the
/// optional default initializer expression. Records store their fields in
/// order-losing maps for identity; this side table preserves declaration
/// order and defaults so record initialization (positional binding and
/// default filling) can be checked and lowered.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordFieldLayout {
    pub name: String,
    pub type_id: CheckedTypeId,
    pub default: Option<AstNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveBorrow {
    pub owner: SymbolId,
    pub binding: SymbolId,
    pub scope: ScopeId,
    pub mutable: bool,
    pub origin: SyntaxOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMutexGuard {
    pub scope: ScopeId,
    pub origin: SyntaxOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventualMoveKind {
    Transfer,
    Await,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedProgram {
    capability_model: TypecheckCapabilityModel,
    resolved: fol_resolver::ResolvedProgram,
    type_table: TypeTable,
    builtins: BuiltinTypeIds,
    source_units: Vec<TypedSourceUnit>,
    symbols: BTreeMap<SymbolId, TypedSymbol>,
    nodes: BTreeMap<SyntaxNodeId, TypedNode>,
    references: BTreeMap<fol_resolver::ReferenceId, TypedReference>,
    standards: BTreeMap<SymbolId, TypedStandard>,
    conformances: BTreeMap<SymbolId, TypedConformance>,
    apparent_type_overrides: BTreeMap<CheckedTypeId, CheckedTypeId>,
    method_call_targets: BTreeMap<SyntaxNodeId, SymbolId>,
    constraint_call_sites: std::collections::BTreeSet<SyntaxNodeId>,
    record_layouts: BTreeMap<CheckedTypeId, Vec<RecordFieldLayout>>,
    /// Generic instantiations whose structural shape is currently being
    /// computed. Recursive value instantiation reaches its own node while
    /// expanding; this guard breaks the cycle so the checker can issue the
    /// finite-layout diagnostic. Transient — empty outside active lowering.
    active_instantiations: std::collections::BTreeSet<CheckedTypeId>,
    moved_bindings: BTreeMap<SymbolId, SyntaxOrigin>,
    eventual_moves: BTreeMap<SymbolId, EventualMoveKind>,
    active_borrows: BTreeMap<SymbolId, Vec<ActiveBorrow>>,
    borrow_bindings: BTreeMap<SymbolId, ActiveBorrow>,
    borrow_history: BTreeMap<SymbolId, ActiveBorrow>,
    owner_borrow_history: BTreeMap<SymbolId, ActiveBorrow>,
    returned_borrows: BTreeMap<SymbolId, SyntaxOrigin>,
    active_mutex_guards: BTreeMap<SymbolId, ActiveMutexGuard>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedPackage {
    pub identity: PackageIdentity,
    pub export_mounts: Vec<TypedExportMount>,
    pub program: TypedProgram,
}

impl TypedPackage {
    pub fn new(
        identity: PackageIdentity,
        export_mounts: Vec<TypedExportMount>,
        program: TypedProgram,
    ) -> Self {
        Self {
            identity,
            export_mounts,
            program,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypedWorkspace {
    capability_model: TypecheckCapabilityModel,
    entry_identity: PackageIdentity,
    packages: BTreeMap<PackageIdentity, TypedPackage>,
}

impl TypedWorkspace {
    pub fn single(entry_identity: PackageIdentity, entry_program: TypedProgram) -> Self {
        let mut packages = BTreeMap::new();
        packages.insert(
            entry_identity.clone(),
            TypedPackage::new(entry_identity.clone(), Vec::new(), entry_program),
        );
        Self {
            capability_model: TypecheckCapabilityModel::Std,
            entry_identity,
            packages,
        }
    }

    pub(crate) fn new(
        capability_model: TypecheckCapabilityModel,
        entry_identity: PackageIdentity,
        packages: BTreeMap<PackageIdentity, TypedPackage>,
    ) -> Self {
        Self {
            capability_model,
            entry_identity,
            packages,
        }
    }

    pub fn capability_model(&self) -> TypecheckCapabilityModel {
        self.capability_model
    }

    pub fn entry_identity(&self) -> &PackageIdentity {
        &self.entry_identity
    }

    pub fn entry_package(&self) -> &TypedPackage {
        self.packages
            .get(&self.entry_identity)
            .expect("typed workspace should always retain the entry package")
    }

    pub fn entry_program(&self) -> &TypedProgram {
        &self.entry_package().program
    }

    pub fn package(&self, identity: &PackageIdentity) -> Option<&TypedPackage> {
        self.packages.get(identity)
    }

    pub fn packages(&self) -> impl Iterator<Item = &TypedPackage> {
        self.packages.values()
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }
}

impl TypedProgram {
    pub(crate) fn mark_binding_moved(&mut self, symbol: SymbolId, origin: SyntaxOrigin) {
        self.moved_bindings.entry(symbol).or_insert(origin);
    }

    pub(crate) fn mark_eventual_transferred(&mut self, symbol: SymbolId, origin: SyntaxOrigin) {
        if !self.moved_bindings.contains_key(&symbol) {
            self.eventual_moves
                .insert(symbol, EventualMoveKind::Transfer);
        }
        self.mark_binding_moved(symbol, origin);
    }

    pub(crate) fn mark_eventual_awaited(&mut self, symbol: SymbolId, origin: SyntaxOrigin) {
        if !self.moved_bindings.contains_key(&symbol) {
            self.eventual_moves.insert(symbol, EventualMoveKind::Await);
        }
        self.mark_binding_moved(symbol, origin);
    }

    pub(crate) fn eventual_move_kind(&self, symbol: SymbolId) -> Option<EventualMoveKind> {
        self.eventual_moves.get(&symbol).copied()
    }

    pub fn moved_binding_origin(&self, symbol: SymbolId) -> Option<&SyntaxOrigin> {
        self.moved_bindings.get(&symbol)
    }

    pub fn active_borrow_for_owner(&self, owner: SymbolId) -> Option<&ActiveBorrow> {
        self.active_borrows
            .get(&owner)
            .and_then(|borrows| borrows.first())
    }

    pub fn active_borrow_binding(&self, binding: SymbolId) -> Option<&ActiveBorrow> {
        self.borrow_bindings.get(&binding)
    }

    pub fn borrow_for_binding(&self, binding: SymbolId) -> Option<&ActiveBorrow> {
        self.borrow_history.get(&binding)
    }

    pub fn borrow_for_owner(&self, owner: SymbolId) -> Option<&ActiveBorrow> {
        self.owner_borrow_history.get(&owner)
    }

    pub fn returned_borrow_origin(&self, binding: SymbolId) -> Option<&SyntaxOrigin> {
        self.returned_borrows.get(&binding)
    }

    pub(crate) fn register_borrow(&mut self, borrow: ActiveBorrow) -> Option<ActiveBorrow> {
        let conflict = self.active_borrows.get(&borrow.owner).and_then(|active| {
            active
                .iter()
                .find(|existing| borrow.mutable || existing.mutable)
                .cloned()
        });
        if conflict.is_some() {
            return conflict;
        }
        self.active_borrows
            .entry(borrow.owner)
            .or_default()
            .push(borrow.clone());
        self.borrow_history.insert(borrow.binding, borrow.clone());
        self.owner_borrow_history
            .entry(borrow.owner)
            .or_insert_with(|| borrow.clone());
        self.borrow_bindings.insert(borrow.binding, borrow);
        None
    }

    pub(crate) fn give_back_borrow(&mut self, binding: SymbolId, origin: SyntaxOrigin) -> bool {
        let Some(borrow) = self.borrow_bindings.remove(&binding) else {
            return false;
        };
        if let Some(active) = self.active_borrows.get_mut(&borrow.owner) {
            active.retain(|entry| entry.binding != binding);
            if active.is_empty() {
                self.active_borrows.remove(&borrow.owner);
            }
        }
        self.returned_borrows.insert(binding, origin);
        true
    }

    pub(crate) fn release_borrows_in_scope(&mut self, scope: ScopeId) {
        let bindings = self
            .borrow_bindings
            .iter()
            .filter_map(|(binding, borrow)| (borrow.scope == scope).then_some(*binding))
            .collect::<Vec<_>>();
        for binding in bindings {
            if let Some(borrow) = self.borrow_bindings.remove(&binding) {
                if let Some(active) = self.active_borrows.get_mut(&borrow.owner) {
                    active.retain(|entry| entry.binding != binding);
                    if active.is_empty() {
                        self.active_borrows.remove(&borrow.owner);
                    }
                }
            }
        }
    }

    pub fn active_mutex_guard(&self, mutex: SymbolId) -> Option<&ActiveMutexGuard> {
        self.active_mutex_guards.get(&mutex)
    }

    pub(crate) fn register_mutex_guard(
        &mut self,
        mutex: SymbolId,
        guard: ActiveMutexGuard,
    ) -> Option<ActiveMutexGuard> {
        if let Some(active) = self.active_mutex_guards.get(&mutex) {
            return Some(active.clone());
        }
        self.active_mutex_guards.insert(mutex, guard);
        None
    }

    pub(crate) fn release_mutex_guard(
        &mut self,
        mutex: SymbolId,
        scope: ScopeId,
    ) -> Option<ActiveMutexGuard> {
        let guard = self.active_mutex_guards.get(&mutex)?;
        if guard.scope != scope {
            return None;
        }
        self.active_mutex_guards.remove(&mutex)
    }

    pub(crate) fn release_mutex_guards_in_scope(&mut self, scope: ScopeId) {
        self.active_mutex_guards
            .retain(|_, guard| guard.scope != scope);
    }

    pub fn from_resolved(resolved: fol_resolver::ResolvedProgram) -> Self {
        Self::from_resolved_with_model(resolved, TypecheckCapabilityModel::Std)
    }

    pub(crate) fn from_resolved_with_model(
        resolved: fol_resolver::ResolvedProgram,
        capability_model: TypecheckCapabilityModel,
    ) -> Self {
        let mut type_table = TypeTable::new();
        let builtins = BuiltinTypeIds::install(&mut type_table);
        let source_units = resolved
            .source_units
            .iter_with_ids()
            .map(|(source_unit_id, unit)| TypedSourceUnit {
                source_unit_id,
                path: unit.path.clone(),
                package: unit.package.clone(),
                namespace: unit.namespace.clone(),
                kind: unit.kind,
                scope_id: unit.scope_id,
                top_level_nodes: unit.top_level_nodes.clone(),
            })
            .collect::<Vec<_>>();
        let symbols = resolved
            .symbols
            .iter_with_ids()
            .map(|(symbol_id, symbol)| {
                (
                    symbol_id,
                    TypedSymbol {
                        symbol_id,
                        kind: symbol.kind,
                        scope_id: symbol.scope,
                        source_unit_id: symbol.source_unit,
                        declared_type: None,
                        receiver_type: None,
                        generic_params: Vec::new(),
                        generic_constraints: BTreeMap::new(),
                        is_mutable: symbol.is_mutable,
                        is_mutex: false,
                        is_channel_sender_capture: false,
                        channel_receiver_params: BTreeSet::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let nodes = source_units
            .iter()
            .flat_map(|unit| {
                unit.top_level_nodes.iter().copied().map(move |syntax_id| {
                    (
                        syntax_id,
                        TypedNode {
                            syntax_id,
                            source_unit_id: unit.source_unit_id,
                            inferred_type: None,
                            recoverable_effect: None,
                            intrinsic_id: None,
                        },
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        let references = resolved
            .references
            .iter_with_ids()
            .map(|(reference_id, reference)| {
                (
                    reference_id,
                    TypedReference {
                        reference_id,
                        kind: reference.kind,
                        source_unit_id: reference.source_unit,
                        resolved_type: None,
                        recoverable_effect: None,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        Self {
            capability_model,
            resolved,
            type_table,
            builtins,
            source_units,
            symbols,
            nodes,
            references,
            standards: BTreeMap::new(),
            conformances: BTreeMap::new(),
            apparent_type_overrides: BTreeMap::new(),
            active_instantiations: std::collections::BTreeSet::new(),
            method_call_targets: BTreeMap::new(),
            constraint_call_sites: std::collections::BTreeSet::new(),
            record_layouts: BTreeMap::new(),
            moved_bindings: BTreeMap::new(),
            eventual_moves: BTreeMap::new(),
            active_borrows: BTreeMap::new(),
            borrow_bindings: BTreeMap::new(),
            borrow_history: BTreeMap::new(),
            owner_borrow_history: BTreeMap::new(),
            returned_borrows: BTreeMap::new(),
            active_mutex_guards: BTreeMap::new(),
        }
    }

    /// Store the ordered field layout (with defaults) for a record type. Keyed
    /// by the interned record `CheckedTypeId` so record-initializer checking and
    /// lowering can recover declaration order and per-field defaults.
    pub(crate) fn set_record_layout(
        &mut self,
        type_id: CheckedTypeId,
        layout: Vec<RecordFieldLayout>,
    ) {
        self.record_layouts.insert(type_id, layout);
    }

    /// The ordered field layout for a record type, if one was recorded.
    pub fn record_layout(&self, type_id: CheckedTypeId) -> Option<&[RecordFieldLayout]> {
        self.record_layouts
            .get(&type_id)
            .map(|fields| fields.as_slice())
    }

    pub fn record_method_call_target(&mut self, syntax_id: SyntaxNodeId, symbol_id: SymbolId) {
        self.method_call_targets.insert(syntax_id, symbol_id);
    }

    pub fn method_call_target(&self, syntax_id: SyntaxNodeId) -> Option<SymbolId> {
        self.method_call_targets.get(&syntax_id).copied()
    }

    pub fn record_constraint_call_site(&mut self, syntax_id: SyntaxNodeId) {
        self.constraint_call_sites.insert(syntax_id);
    }

    /// True when the method call at `syntax_id` targets a required routine of
    /// a generic-parameter constraint; the concrete callee is only known
    /// after monomorphization.
    pub fn is_constraint_call_site(&self, syntax_id: SyntaxNodeId) -> bool {
        self.constraint_call_sites.contains(&syntax_id)
    }

    pub fn method_call_targets(&self) -> impl Iterator<Item = (SyntaxNodeId, SymbolId)> + '_ {
        self.method_call_targets
            .iter()
            .map(|(syntax_id, symbol_id)| (*syntax_id, *symbol_id))
    }

    pub fn package_name(&self) -> &str {
        self.resolved.package_name()
    }

    pub fn capability_model(&self) -> TypecheckCapabilityModel {
        self.capability_model
    }

    pub fn resolved(&self) -> &fol_resolver::ResolvedProgram {
        &self.resolved
    }

    pub fn type_table(&self) -> &TypeTable {
        &self.type_table
    }

    pub fn builtin_types(&self) -> BuiltinTypeIds {
        self.builtins
    }

    pub(crate) fn type_table_mut(&mut self) -> &mut TypeTable {
        &mut self.type_table
    }

    pub fn source_units(&self) -> &[TypedSourceUnit] {
        &self.source_units
    }

    pub fn ordinary_source_units(&self) -> impl Iterator<Item = &TypedSourceUnit> {
        self.source_units
            .iter()
            .filter(|unit| unit.kind == ParsedSourceUnitKind::Ordinary)
    }

    pub fn build_source_units(&self) -> impl Iterator<Item = &TypedSourceUnit> {
        self.source_units
            .iter()
            .filter(|unit| unit.kind == ParsedSourceUnitKind::Build)
    }

    pub fn typed_symbol(&self, symbol_id: SymbolId) -> Option<&TypedSymbol> {
        self.symbols.get(&symbol_id)
    }

    pub(crate) fn typed_symbol_mut(&mut self, symbol_id: SymbolId) -> Option<&mut TypedSymbol> {
        self.symbols.get_mut(&symbol_id)
    }

    pub fn typed_node(&self, syntax_id: SyntaxNodeId) -> Option<&TypedNode> {
        self.nodes.get(&syntax_id)
    }

    pub fn typed_reference(
        &self,
        reference_id: fol_resolver::ReferenceId,
    ) -> Option<&TypedReference> {
        self.references.get(&reference_id)
    }

    pub fn all_typed_symbols(&self) -> impl Iterator<Item = &TypedSymbol> {
        self.symbols.values()
    }

    pub fn all_typed_references(&self) -> impl Iterator<Item = &TypedReference> {
        self.references.values()
    }

    pub fn typed_standard(&self, symbol_id: SymbolId) -> Option<&TypedStandard> {
        self.standards.get(&symbol_id)
    }

    pub fn all_typed_standards(&self) -> impl Iterator<Item = &TypedStandard> {
        self.standards.values()
    }

    pub fn typed_conformance(&self, symbol_id: SymbolId) -> Option<&TypedConformance> {
        self.conformances.get(&symbol_id)
    }

    pub fn all_typed_conformances(&self) -> impl Iterator<Item = &TypedConformance> {
        self.conformances.values()
    }

    pub(crate) fn typed_reference_mut(
        &mut self,
        reference_id: fol_resolver::ReferenceId,
    ) -> Option<&mut TypedReference> {
        self.references.get_mut(&reference_id)
    }

    pub(crate) fn record_typed_standard(&mut self, standard: TypedStandard) {
        self.standards.insert(standard.symbol_id, standard);
    }

    pub(crate) fn record_typed_conformance(&mut self, conformance: TypedConformance) {
        self.conformances
            .insert(conformance.type_symbol_id, conformance);
    }

    pub(crate) fn record_node_type(
        &mut self,
        syntax_id: SyntaxNodeId,
        source_unit_id: SourceUnitId,
        type_id: CheckedTypeId,
    ) -> Result<(), crate::TypecheckError> {
        self.nodes
            .entry(syntax_id)
            .or_insert(TypedNode {
                syntax_id,
                source_unit_id,
                inferred_type: None,
                recoverable_effect: None,
                intrinsic_id: None,
            })
            .inferred_type = Some(type_id);
        Ok(())
    }

    pub(crate) fn record_node_recoverable_effect(
        &mut self,
        syntax_id: SyntaxNodeId,
        source_unit_id: SourceUnitId,
        effect: RecoverableCallEffect,
    ) -> Result<(), crate::TypecheckError> {
        self.nodes
            .entry(syntax_id)
            .or_insert(TypedNode {
                syntax_id,
                source_unit_id,
                inferred_type: None,
                recoverable_effect: None,
                intrinsic_id: None,
            })
            .recoverable_effect = Some(effect);
        Ok(())
    }

    pub(crate) fn record_node_intrinsic(
        &mut self,
        syntax_id: SyntaxNodeId,
        source_unit_id: SourceUnitId,
        intrinsic_id: IntrinsicId,
    ) -> Result<(), crate::TypecheckError> {
        self.nodes
            .entry(syntax_id)
            .or_insert(TypedNode {
                syntax_id,
                source_unit_id,
                inferred_type: None,
                recoverable_effect: None,
                intrinsic_id: None,
            })
            .intrinsic_id = Some(intrinsic_id);
        Ok(())
    }

    pub(crate) fn record_reference_recoverable_effect(
        &mut self,
        reference_id: fol_resolver::ReferenceId,
        effect: RecoverableCallEffect,
    ) -> Result<(), crate::TypecheckError> {
        let reference = self.typed_reference_mut(reference_id).ok_or_else(|| {
            crate::TypecheckError::new(
                crate::TypecheckErrorKind::Internal,
                "typed reference disappeared while recording a recoverable call effect",
            )
        })?;
        reference.recoverable_effect = Some(effect);
        Ok(())
    }

    pub(crate) fn record_apparent_type_override(
        &mut self,
        shell_type: CheckedTypeId,
        apparent_type: CheckedTypeId,
    ) {
        self.apparent_type_overrides
            .insert(shell_type, apparent_type);
    }

    /// Mark a generic instantiation as being expanded. Returns `false` if it is
    /// already in progress (a recursive self-reference), so the caller can stop
    /// expanding and use the interned nominal node instead.
    pub(crate) fn begin_instantiation(&mut self, instance: CheckedTypeId) -> bool {
        self.active_instantiations.insert(instance)
    }

    pub(crate) fn end_instantiation(&mut self, instance: CheckedTypeId) {
        self.active_instantiations.remove(&instance);
    }

    pub fn apparent_type_override(&self, type_id: CheckedTypeId) -> Option<CheckedTypeId> {
        self.apparent_type_overrides.get(&type_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::{TypedProgram, TypedWorkspace};
    use crate::{BuiltinType, CheckedType, TypecheckCapabilityModel};
    use fol_parser::ast::{AstParser, ParsedSourceUnitKind};
    use fol_resolver::{resolve_package, PackageIdentity, PackageSourceKind};
    use fol_stream::FileStream;
    use std::collections::BTreeMap;

    fn package_identity(name: &str) -> PackageIdentity {
        PackageIdentity {
            source_kind: PackageSourceKind::Entry,
            canonical_root: format!("/tmp/{name}"),
            display_name: name.to_string(),
        }
    }

    #[test]
    fn typed_workspace_retains_capability_model() {
        let identity = package_identity("demo");
        let workspace = TypedWorkspace::new(
            TypecheckCapabilityModel::Core,
            identity.clone(),
            BTreeMap::new(),
        );

        assert_eq!(workspace.capability_model(), TypecheckCapabilityModel::Core);
        assert_eq!(workspace.entry_identity(), &identity);
        assert_eq!(workspace.package_count(), 0);
    }

    #[test]
    fn typed_program_defaults_to_std_capability_model() {
        let program = TypedProgram::from_resolved(fol_resolver::ResolvedProgram::new(
            fol_parser::ast::ParsedPackage {
                package: "demo".to_string(),
                source_units: Vec::new(),
                syntax_index: fol_parser::ast::SyntaxIndex::default(),
            },
        ));

        assert_eq!(program.capability_model(), TypecheckCapabilityModel::Std);
    }

    #[test]
    fn typed_program_shell_installs_builtin_types_for_resolved_programs() {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream =
            FileStream::from_file(fixture_path).expect("Should open typecheck fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Typecheck fixture should parse");
        let resolved = resolve_package(syntax).expect("Typecheck fixture should resolve");

        let typed = TypedProgram::from_resolved(resolved);

        assert_eq!(typed.package_name(), "parser");
        assert_eq!(
            typed.type_table().get(typed.builtin_types().str_),
            Some(&CheckedType::Builtin(BuiltinType::Str))
        );
        assert_eq!(typed.source_units().len(), 1);
        assert_eq!(
            typed.source_units()[0].top_level_nodes,
            typed
                .resolved()
                .source_units
                .get(fol_resolver::SourceUnitId(0))
                .expect("resolved source unit should exist")
                .top_level_nodes
        );
    }

    #[test]
    fn typed_workspace_single_package_shell_exposes_entry_program() {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream =
            FileStream::from_file(fixture_path).expect("Should open typecheck fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Typecheck fixture should parse");
        let resolved = resolve_package(syntax).expect("Typecheck fixture should resolve");
        let entry_identity = fol_resolver::PackageIdentity {
            source_kind: fol_resolver::PackageSourceKind::Entry,
            canonical_root: resolved.package_name().to_string(),
            display_name: resolved.package_name().to_string(),
        };

        let workspace = TypedWorkspace::single(
            entry_identity.clone(),
            TypedProgram::from_resolved(resolved),
        );

        assert_eq!(workspace.package_count(), 1);
        assert_eq!(workspace.entry_identity(), &entry_identity);
        assert_eq!(workspace.entry_program().package_name(), "parser");
    }

    #[test]
    fn typed_program_filters_build_and_ordinary_source_units() {
        let root = std::env::temp_dir().join(format!(
            "fol_typecheck_build_units_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).expect("should create temp source dir");
        std::fs::write(root.join("build.fol"), "`build`\n").expect("should write build file");
        std::fs::write(root.join("src/main.fol"), "var value: int = 1;\n")
            .expect("should write ordinary source");

        let mut stream =
            FileStream::from_folder(root.to_str().expect("utf8 temp path")).expect("open temp pkg");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("temp pkg should parse");
        let resolved = resolve_package(syntax).expect("temp pkg should resolve");
        let typed = TypedProgram::from_resolved(resolved);

        assert_eq!(typed.build_source_units().count(), 1);
        assert_eq!(typed.ordinary_source_units().count(), 1);
        assert_eq!(typed.source_units()[0].kind, ParsedSourceUnitKind::Build);

        std::fs::remove_dir_all(root).ok();
    }
}

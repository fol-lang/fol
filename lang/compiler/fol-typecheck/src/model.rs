use crate::types::GenericConstraint;
use crate::{BuiltinTypeIds, CheckedTypeId, RoutineType, TypeTable, TypecheckCapabilityModel};
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

#[derive(Debug, Clone, PartialEq)]
pub struct TypedSymbol {
    pub symbol_id: SymbolId,
    pub kind: SymbolKind,
    pub scope_id: ScopeId,
    pub source_unit_id: SourceUnitId,
    pub declared_type: Option<CheckedTypeId>,
    pub receiver_type: Option<CheckedTypeId>,
    /// Declaration-owned default expressions. Routine types are interned by
    /// callable shape, so the concrete AST must remain attached to the symbol
    /// instead of relying on whichever equal signature was interned first.
    pub param_defaults: Vec<Option<AstNode>>,
    pub generic_params: Vec<SymbolId>,
    pub generic_constraints: BTreeMap<SymbolId, Vec<GenericConstraint>>,
    /// Mirrors the resolver's binding mutability (`var[mut]`/`lab[mut]`).
    /// Drives field-assignment legality.
    pub is_mutable: bool,
    pub is_mutex: bool,
    /// A `var[mut, bor] guard = ([bor]mux).lock()` binding. The lifetime-scoped
    /// guard value cannot be moved, copied, or cloned as a whole (V3_MEM §8.3);
    /// field reads (`guard.value`) remain the way to snapshot protected data.
    pub is_mutex_guard: bool,
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
    /// True when the lock was bound to a lifetime-scoped guard value
    /// (`var[mut, bor] guard = ([bor]mux).lock()`), as opposed to the older
    /// handle-lock statement form (`mux.lock(); ...; mux.unlock();`). Only the
    /// guard-VALUE form is forbidden from crossing spawn/await/blocking-receive/
    /// blocking-select boundaries (V3_MEM §8.3); handle-lock crossing stays
    /// allowed for concurrent access.
    pub bound: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeferredBindingUse {
    pub(crate) scope: ScopeId,
    pub(crate) origin: SyntaxOrigin,
}

/// A shared loan held on an outer binding by a spawned scoped task
/// (`[>]fun()[state[bor]] = ...`) or by a local closure value
/// (`var f = fun()[state[bor]] ...`). The owner stays readable but cannot be
/// mutated or moved until the registering scope exits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskBorrow {
    pub(crate) scope: ScopeId,
    pub(crate) origin: SyntaxOrigin,
    pub(crate) kind: TaskBorrowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TaskBorrowKind {
    SpawnTask,
    LocalClosure,
}

impl TaskBorrowKind {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::SpawnTask => "a spawned task",
            Self::LocalClosure => "a local closure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeferredTransferConflict {
    pub(crate) symbol: SymbolId,
    pub(crate) transfer_origin: SyntaxOrigin,
    pub(crate) deferred_origin: SyntaxOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnershipFlowState {
    moved_bindings: BTreeMap<SymbolId, SyntaxOrigin>,
    eventual_moves: BTreeMap<SymbolId, EventualMoveKind>,
    recoverable_eventual_obligations: BTreeMap<SymbolId, RecoverableEventualObligation>,
    // Loan state is flow-sensitive. It must be snapshotted, restored, and merged
    // at branch boundaries exactly like the move set; otherwise a borrow ended on
    // one branch leaks that give-back onto the other branch, making conditional
    // giveback path-unsound (Slice A soundness repair).
    active_borrows: BTreeMap<SymbolId, Vec<ActiveBorrow>>,
    borrow_bindings: BTreeMap<SymbolId, ActiveBorrow>,
    returned_borrows: BTreeMap<SymbolId, SyntaxOrigin>,
    // Static-place move tracking (Slice C §3.1). Moving a single named field of
    // an owned aggregate invalidates only that field, not the whole binding, so
    // the surviving fields stay readable. Keyed by binding then field name; a
    // whole-binding move (moved_bindings) subsumes every field.
    moved_fields: BTreeMap<SymbolId, BTreeMap<String, SyntaxOrigin>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventualMoveKind {
    Transfer,
    Await,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecoverableEventualObligation {
    pub(crate) owner_scope: ScopeId,
    pub(crate) activation_scope: ScopeId,
    pub(crate) origin: Option<SyntaxOrigin>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OwnershipEventKind {
    Move,
    Reinitialize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnershipEvent {
    kind: OwnershipEventKind,
    origin: SyntaxOrigin,
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
    /// Fully instantiated signatures at direct call sites. Processor boundary
    /// validation needs the concrete parameter types after generic inference,
    /// including parameters filled by omitted defaults.
    call_signatures: BTreeMap<SyntaxNodeId, RoutineType>,
    constraint_call_sites: std::collections::BTreeSet<SyntaxNodeId>,
    record_layouts: BTreeMap<CheckedTypeId, Vec<RecordFieldLayout>>,
    /// Generic instantiations whose structural shape is currently being
    /// computed. Recursive value instantiation reaches its own node while
    /// expanding; this guard breaks the cycle so the checker can issue the
    /// finite-layout diagnostic. Transient — empty outside active lowering.
    active_instantiations: std::collections::BTreeSet<CheckedTypeId>,
    moved_bindings: BTreeMap<SymbolId, SyntaxOrigin>,
    moved_fields: BTreeMap<SymbolId, BTreeMap<String, SyntaxOrigin>>,
    eventual_moves: BTreeMap<SymbolId, EventualMoveKind>,
    recoverable_eventual_obligations: BTreeMap<SymbolId, RecoverableEventualObligation>,
    ownership_history: BTreeMap<SymbolId, Vec<OwnershipEvent>>,
    active_borrows: BTreeMap<SymbolId, Vec<ActiveBorrow>>,
    borrow_bindings: BTreeMap<SymbolId, ActiveBorrow>,
    borrow_history: BTreeMap<SymbolId, ActiveBorrow>,
    owner_borrow_history: BTreeMap<SymbolId, ActiveBorrow>,
    returned_borrows: BTreeMap<SymbolId, SyntaxOrigin>,
    active_mutex_guards: BTreeMap<SymbolId, ActiveMutexGuard>,
    deferred_binding_uses: BTreeMap<SymbolId, Vec<DeferredBindingUse>>,
    task_borrowed_bindings: BTreeMap<SymbolId, Vec<TaskBorrow>>,
    bor_env_closures: BTreeSet<SymbolId>,
    deferred_transfer_conflict: Option<DeferredTransferConflict>,
    /// Compiler-owned capability standards (`copy`/`clone`/`fin`/`send`/`share`)
    /// claimed by each type declaration via its conformance list. Structural
    /// verification of these claims is a later slice.
    capability_claims: BTreeMap<SymbolId, std::collections::BTreeSet<String>>,
    /// Compiler-owned capability bounds on each generic parameter symbol (from
    /// `fun f(T: copy)(...)`). Verified against the actual type at every call
    /// site (V3_MEM §4.1).
    generic_capability_constraints: BTreeMap<SymbolId, std::collections::BTreeSet<String>>,
    /// Record/entry type ids whose declaration claims custom finalization
    /// (`fin`). Kept keyed by CheckedTypeId so containment checks (e.g. rejecting
    /// a top-level `fin` value) do not need to reverse a type back to its symbol.
    fin_types: std::collections::BTreeSet<CheckedTypeId>,
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
    pub(crate) fn ownership_flow_state(&self) -> OwnershipFlowState {
        OwnershipFlowState {
            moved_bindings: self.moved_bindings.clone(),
            moved_fields: self.moved_fields.clone(),
            eventual_moves: self.eventual_moves.clone(),
            recoverable_eventual_obligations: self.recoverable_eventual_obligations.clone(),
            active_borrows: self.active_borrows.clone(),
            borrow_bindings: self.borrow_bindings.clone(),
            returned_borrows: self.returned_borrows.clone(),
        }
    }

    pub(crate) fn restore_ownership_flow(&mut self, state: &OwnershipFlowState) {
        self.moved_bindings.clone_from(&state.moved_bindings);
        self.moved_fields.clone_from(&state.moved_fields);
        self.eventual_moves.clone_from(&state.eventual_moves);
        self.recoverable_eventual_obligations
            .clone_from(&state.recoverable_eventual_obligations);
        self.active_borrows.clone_from(&state.active_borrows);
        self.borrow_bindings.clone_from(&state.borrow_bindings);
        self.returned_borrows.clone_from(&state.returned_borrows);
    }

    pub(crate) fn merge_ownership_flows(
        &mut self,
        baseline: &OwnershipFlowState,
        branches: &[OwnershipFlowState],
    ) {
        if branches.is_empty() {
            self.restore_ownership_flow(baseline);
            return;
        }
        self.moved_bindings.clear();
        self.moved_fields.clear();
        self.eventual_moves.clear();
        self.recoverable_eventual_obligations.clear();
        self.active_borrows.clear();
        self.borrow_bindings.clear();
        self.returned_borrows.clear();
        for branch in branches {
            for (symbol, origin) in &branch.moved_bindings {
                self.moved_bindings
                    .entry(*symbol)
                    .or_insert_with(|| origin.clone());
            }
            // A field moved on any branch is treated as moved after the merge,
            // matching whole-binding move semantics: a later use must be
            // rejected because a runtime path could have consumed the field.
            for (symbol, fields) in &branch.moved_fields {
                let entry = self.moved_fields.entry(*symbol).or_default();
                for (field, origin) in fields {
                    entry.entry(field.clone()).or_insert_with(|| origin.clone());
                }
            }
            for (symbol, kind) in &branch.eventual_moves {
                self.eventual_moves.entry(*symbol).or_insert(*kind);
            }
            // Recoverable-eventual obligations are must-handle state. If any
            // continuing branch still owns the obligation, it remains live
            // after the merge. This intentionally differs from a normal move:
            // handling the eventual on only one runtime path is insufficient.
            for (symbol, obligation) in &branch.recoverable_eventual_obligations {
                self.recoverable_eventual_obligations
                    .entry(*symbol)
                    .or_insert_with(|| obligation.clone());
            }
            // A give-back on this path is the authoritative "loan ended"
            // signal. Union it: if any branch returned the borrow, a later
            // reuse or a second give-back must be rejected, which prevents the
            // emitted double-drop when giveback is conditional.
            for (binding, origin) in &branch.returned_borrows {
                self.returned_borrows
                    .entry(*binding)
                    .or_insert_with(|| origin.clone());
            }
        }
        // Loan liveness is anchored on the baseline set of loans entering the
        // branch (the loans live before the split), not on each branch's
        // active_borrows: a branch body may collapse to the enclosing routine
        // scope and prematurely drop an outer loan from its own table, which
        // would otherwise make the merge unsound. A give-back inside a branch is
        // recorded in returned_borrows, so we use that as the reliable
        // per-branch signal.
        //
        // Rule: a loan created before the branch is still active after the merge
        // unless it was given back on *every* branch. Giving it back on only some
        // branches leaves it live on the others, so the owner stays inaccessible
        // (sound: matches the runtime path that never ran the give-back).
        let returned_on_every_branch = |binding: SymbolId| -> bool {
            branches
                .iter()
                .all(|branch| branch.returned_borrows.contains_key(&binding))
        };
        let returned_on_any_branch = |binding: SymbolId| -> bool {
            branches
                .iter()
                .any(|branch| branch.returned_borrows.contains_key(&binding))
        };
        for (owner, borrows) in &baseline.active_borrows {
            for borrow in borrows {
                if returned_on_every_branch(borrow.binding) {
                    continue;
                }
                self.active_borrows
                    .entry(*owner)
                    .or_default()
                    .push(borrow.clone());
            }
        }
        // A borrower binding stays usable after the merge only if it was live at
        // the baseline and was not given back on any branch: a give-back on even
        // one path leaves the borrower gone on that path.
        for (binding, borrow) in &baseline.borrow_bindings {
            if !returned_on_any_branch(*binding) {
                self.borrow_bindings.insert(*binding, borrow.clone());
            }
        }
    }

    pub(crate) fn mark_binding_moved(&mut self, symbol: SymbolId, origin: SyntaxOrigin) {
        if self.record_deferred_transfer_conflict(symbol, origin.clone()) {
            return;
        }
        self.record_ownership_event(symbol, OwnershipEventKind::Move, origin.clone());
        self.moved_bindings.entry(symbol).or_insert(origin);
    }

    pub(crate) fn mark_binding_reinitialized(
        &mut self,
        symbol: SymbolId,
        origin: Option<SyntaxOrigin>,
    ) {
        if let Some(origin) = origin {
            self.record_ownership_event(symbol, OwnershipEventKind::Reinitialize, origin);
        }
        self.moved_bindings.remove(&symbol);
        self.moved_fields.remove(&symbol);
        self.eventual_moves.remove(&symbol);
    }

    /// Mark a single static field of an owned aggregate as moved (Slice C
    /// §3.1). Only that field becomes inaccessible; the surviving fields of the
    /// binding stay readable. A subsequent whole-binding move or reinitialize
    /// subsumes the per-field state.
    pub(crate) fn mark_field_moved(&mut self, symbol: SymbolId, field: &str, origin: SyntaxOrigin) {
        if self.record_deferred_transfer_conflict(symbol, origin.clone()) {
            return;
        }
        self.record_ownership_event(symbol, OwnershipEventKind::Move, origin.clone());
        self.moved_fields
            .entry(symbol)
            .or_default()
            .entry(field.to_string())
            .or_insert(origin);
    }

    /// Origin at which `symbol.field` was moved, if it is currently moved.
    pub fn moved_field_origin(&self, symbol: SymbolId, field: &str) -> Option<&SyntaxOrigin> {
        self.moved_fields
            .get(&symbol)
            .and_then(|fields| fields.get(field))
    }

    /// The first (by field name) moved field of `symbol`, if the binding is
    /// partially moved. Used to reject reading the aggregate as a whole value.
    pub fn first_moved_field(&self, symbol: SymbolId) -> Option<(&String, &SyntaxOrigin)> {
        self.moved_fields
            .get(&symbol)
            .and_then(|fields| fields.iter().next())
    }

    pub(crate) fn mark_eventual_transferred(&mut self, symbol: SymbolId, origin: SyntaxOrigin) {
        if self.record_deferred_transfer_conflict(symbol, origin.clone()) {
            return;
        }
        if !self.moved_bindings.contains_key(&symbol) {
            self.eventual_moves
                .insert(symbol, EventualMoveKind::Transfer);
        }
        self.record_ownership_event(symbol, OwnershipEventKind::Move, origin.clone());
        self.moved_bindings.entry(symbol).or_insert(origin);
        self.recoverable_eventual_obligations.remove(&symbol);
    }

    pub(crate) fn mark_eventual_awaited(&mut self, symbol: SymbolId, origin: SyntaxOrigin) {
        if self.record_deferred_transfer_conflict(symbol, origin.clone()) {
            return;
        }
        if !self.moved_bindings.contains_key(&symbol) {
            self.eventual_moves.insert(symbol, EventualMoveKind::Await);
        }
        self.record_ownership_event(symbol, OwnershipEventKind::Move, origin.clone());
        self.moved_bindings.entry(symbol).or_insert(origin);
        self.recoverable_eventual_obligations.remove(&symbol);
    }

    pub(crate) fn register_recoverable_eventual_obligation(
        &mut self,
        symbol: SymbolId,
        owner_scope: ScopeId,
        activation_scope: ScopeId,
        origin: Option<SyntaxOrigin>,
    ) {
        self.recoverable_eventual_obligations.insert(
            symbol,
            RecoverableEventualObligation {
                owner_scope,
                activation_scope,
                origin,
            },
        );
    }

    pub(crate) fn recoverable_eventual_obligation(
        &self,
        symbol: SymbolId,
    ) -> Option<&RecoverableEventualObligation> {
        self.recoverable_eventual_obligations.get(&symbol)
    }

    pub(crate) fn recoverable_eventual_obligations(
        &self,
    ) -> impl Iterator<Item = (SymbolId, &RecoverableEventualObligation)> {
        self.recoverable_eventual_obligations
            .iter()
            .map(|(symbol, obligation)| (*symbol, obligation))
    }

    pub(crate) fn release_recoverable_eventual_obligations_in_scope(&mut self, scope: ScopeId) {
        self.recoverable_eventual_obligations
            .retain(|_, obligation| obligation.owner_scope != scope);
    }

    fn record_ownership_event(
        &mut self,
        symbol: SymbolId,
        kind: OwnershipEventKind,
        origin: SyntaxOrigin,
    ) {
        let event = OwnershipEvent { kind, origin };
        let history = self.ownership_history.entry(symbol).or_default();
        if !history.contains(&event) {
            history.push(event);
        }
    }

    fn record_deferred_transfer_conflict(
        &mut self,
        symbol: SymbolId,
        transfer_origin: SyntaxOrigin,
    ) -> bool {
        let Some(deferred_use) = self
            .deferred_binding_uses
            .get(&symbol)
            .and_then(|uses| uses.first())
            .cloned()
        else {
            return false;
        };
        self.deferred_transfer_conflict
            .get_or_insert(DeferredTransferConflict {
                symbol,
                transfer_origin,
                deferred_origin: deferred_use.origin,
            });
        true
    }

    pub(crate) fn register_deferred_binding_use(
        &mut self,
        symbol: SymbolId,
        deferred_use: DeferredBindingUse,
    ) {
        let uses = self.deferred_binding_uses.entry(symbol).or_default();
        if !uses.contains(&deferred_use) {
            uses.push(deferred_use);
        }
    }

    pub(crate) fn register_task_borrow(&mut self, symbol: SymbolId, task_borrow: TaskBorrow) {
        let borrows = self.task_borrowed_bindings.entry(symbol).or_default();
        if !borrows.contains(&task_borrow) {
            borrows.push(task_borrow);
        }
    }

    /// Mark `symbol` as a closure value holding borrowed captures; such a
    /// value cannot escape the scope its loans are tied to (V3_MEM section
    /// 5.3 local nonescaping closures).
    pub(crate) fn mark_bor_env_closure(&mut self, symbol: SymbolId) {
        self.bor_env_closures.insert(symbol);
    }

    pub(crate) fn is_bor_env_closure(&self, symbol: SymbolId) -> bool {
        self.bor_env_closures.contains(&symbol)
    }

    pub(crate) fn first_task_borrow(&self, symbol: SymbolId) -> Option<&TaskBorrow> {
        self.task_borrowed_bindings
            .get(&symbol)
            .and_then(|borrows| borrows.first())
    }

    pub(crate) fn take_deferred_transfer_conflict(&mut self) -> Option<DeferredTransferConflict> {
        self.deferred_transfer_conflict.take()
    }

    /// Record a compiler-owned capability standard claimed by a type.
    pub(crate) fn record_capability_claim(&mut self, type_symbol: SymbolId, capability: String) {
        self.capability_claims
            .entry(type_symbol)
            .or_default()
            .insert(capability);
    }

    /// The compiler-owned capability standards a type declaration claims.
    pub fn capability_claims(
        &self,
        type_symbol: SymbolId,
    ) -> Option<&std::collections::BTreeSet<String>> {
        self.capability_claims.get(&type_symbol)
    }

    pub(crate) fn record_generic_capability_constraint(
        &mut self,
        generic_symbol: SymbolId,
        capability: String,
    ) {
        self.generic_capability_constraints
            .entry(generic_symbol)
            .or_default()
            .insert(capability);
    }

    /// The compiler-owned capability bounds on a generic parameter symbol.
    pub fn generic_capability_constraints(
        &self,
        generic_symbol: SymbolId,
    ) -> Option<&std::collections::BTreeSet<String>> {
        self.generic_capability_constraints.get(&generic_symbol)
    }

    /// Record that a type (by its checked id) claims custom finalization.
    pub(crate) fn record_fin_type(&mut self, type_id: CheckedTypeId) {
        self.fin_types.insert(type_id);
    }

    /// Whether the type's own declaration claims `fin` (non-transitive, exact id).
    pub fn type_claims_fin(&self, type_id: CheckedTypeId) -> bool {
        self.fin_types.contains(&type_id)
    }

    /// Whether `type_id` resolves to a `fin`-claiming type after peeling
    /// owned/borrowed shells and following `Declared` references to the
    /// underlying declaration. A binding or receiver type is often a `Declared`
    /// reference rather than the interned record id recorded at declaration.
    pub fn type_resolves_to_fin(&self, type_id: CheckedTypeId) -> bool {
        fn resolve(program: &TypedProgram, type_id: CheckedTypeId, depth: u8) -> bool {
            if depth > 8 {
                return false;
            }
            if program.type_claims_fin(type_id) {
                return true;
            }
            match program.type_table().get(type_id) {
                Some(crate::CheckedType::Owned { inner })
                | Some(crate::CheckedType::Borrowed { inner, .. }) => {
                    resolve(program, *inner, depth + 1)
                }
                Some(crate::CheckedType::Declared { symbol, .. }) => program
                    .typed_symbol(*symbol)
                    .and_then(|declared| declared.declared_type)
                    .is_some_and(|declared| {
                        declared != type_id && resolve(program, declared, depth + 1)
                    }),
                _ => false,
            }
        }
        resolve(self, type_id, 0)
    }

    /// A deferred (`dfr`/`edf`) block that reads a binding pins that binding's
    /// lifetime to the scope exit where the block runs. Used to reject an early
    /// give-back of a borrow the deferred block still needs.
    pub(crate) fn first_deferred_binding_use(
        &self,
        symbol: SymbolId,
    ) -> Option<&DeferredBindingUse> {
        self.deferred_binding_uses
            .get(&symbol)
            .and_then(|uses| uses.first())
    }

    pub(crate) fn release_deferred_binding_uses_in_scope(&mut self, scope: ScopeId) {
        self.deferred_binding_uses.retain(|_, uses| {
            uses.retain(|deferred_use| deferred_use.scope != scope);
            !uses.is_empty()
        });
    }

    pub(crate) fn release_task_borrows_in_scope(&mut self, scope: ScopeId) {
        self.task_borrowed_bindings.retain(|_, borrows| {
            borrows.retain(|task_borrow| task_borrow.scope != scope);
            !borrows.is_empty()
        });
    }

    pub(crate) fn eventual_move_kind(&self, symbol: SymbolId) -> Option<EventualMoveKind> {
        self.eventual_moves.get(&symbol).copied()
    }

    pub fn moved_binding_origin(&self, symbol: SymbolId) -> Option<&SyntaxOrigin> {
        self.moved_bindings.get(&symbol)
    }

    pub fn moved_binding_origin_at(
        &self,
        symbol: SymbolId,
        file: &str,
        line: usize,
        column: usize,
    ) -> Option<&SyntaxOrigin> {
        let event = self
            .ownership_history
            .get(&symbol)?
            .iter()
            .filter(|event| {
                event.origin.file.as_deref() == Some(file)
                    && (event.origin.line, event.origin.column) <= (line, column)
            })
            .max_by_key(|event| (event.origin.line, event.origin.column))?;
        match event.kind {
            OwnershipEventKind::Move => Some(&event.origin),
            OwnershipEventKind::Reinitialize => None,
        }
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

    pub fn borrows_for_owner(&self, owner: SymbolId) -> impl Iterator<Item = &ActiveBorrow> {
        self.borrow_history
            .values()
            .filter(move |borrow| borrow.owner == owner)
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

    /// Remove every active loan created by `binding` across all owners it
    /// borrows. A single borrow binding may borrow more than one owner — for
    /// example a borrow-returning call `pick(#a, #b)` locks both `a` and `b`
    /// (Slice C multi-owner borrows) — so release must sweep every owner, not
    /// just the representative recorded in `borrow_bindings`.
    fn sweep_active_borrows_for_binding(&mut self, binding: SymbolId) {
        let owners = self
            .active_borrows
            .iter()
            .filter(|(_, borrows)| borrows.iter().any(|entry| entry.binding == binding))
            .map(|(owner, _)| *owner)
            .collect::<Vec<_>>();
        for owner in owners {
            if let Some(active) = self.active_borrows.get_mut(&owner) {
                active.retain(|entry| entry.binding != binding);
                if active.is_empty() {
                    self.active_borrows.remove(&owner);
                }
            }
        }
    }

    pub(crate) fn give_back_borrow(&mut self, binding: SymbolId, origin: SyntaxOrigin) -> bool {
        if self.borrow_bindings.remove(&binding).is_none() {
            return false;
        }
        self.sweep_active_borrows_for_binding(binding);
        self.returned_borrows.insert(binding, origin);
        true
    }

    /// Non-lexical borrow release (Slice C): a borrow binding declared in
    /// `scope` whose last use is at or before the just-completed statement
    /// index is dead, so it is released early and its owner becomes accessible
    /// again — rather than staying locked until the lexical scope ends. A
    /// binding with no recorded later use (absent from `last_use`) is released
    /// as soon as it is active. This only ends loans; it does not mark them as
    /// explicitly given back, so it never interferes with give-back diagnostics.
    pub(crate) fn release_scope_borrows_dead_after(
        &mut self,
        scope: ScopeId,
        stmt_index: usize,
        last_use: &BTreeMap<SymbolId, usize>,
    ) {
        let dead = self
            .borrow_bindings
            .iter()
            .filter(|(binding, borrow)| {
                borrow.scope == scope
                    && last_use.get(binding).is_none_or(|last| *last <= stmt_index)
            })
            .map(|(binding, _)| *binding)
            .collect::<Vec<_>>();
        for binding in dead {
            if self.borrow_bindings.remove(&binding).is_some() {
                self.sweep_active_borrows_for_binding(binding);
            }
        }
    }

    pub(crate) fn release_borrows_in_scope(&mut self, scope: ScopeId) {
        let bindings = self
            .borrow_bindings
            .iter()
            .filter_map(|(binding, borrow)| (borrow.scope == scope).then_some(*binding))
            .collect::<Vec<_>>();
        for binding in bindings {
            if self.borrow_bindings.remove(&binding).is_some() {
                self.sweep_active_borrows_for_binding(binding);
            }
        }
    }

    pub fn active_mutex_guard(&self, mutex: SymbolId) -> Option<&ActiveMutexGuard> {
        self.active_mutex_guards.get(&mutex)
    }

    /// Promote the active lock for `mutex` to a lifetime-scoped guard value.
    /// Called when a `.lock()` result is bound with `var[mut, bor]` (V3_MEM §8.3).
    pub(crate) fn mark_guard_bound(&mut self, mutex: SymbolId) {
        if let Some(guard) = self.active_mutex_guards.get_mut(&mutex) {
            guard.bound = true;
        }
    }

    /// The first active guard held as a lifetime-scoped guard value, if any.
    /// Used to reject crossing a processor boundary while such a guard is live.
    pub(crate) fn active_bound_guard(&self) -> Option<&ActiveMutexGuard> {
        self.active_mutex_guards.values().find(|guard| guard.bound)
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
                        param_defaults: Vec::new(),
                        generic_params: Vec::new(),
                        generic_constraints: BTreeMap::new(),
                        is_mutable: symbol.is_mutable,
                        is_mutex: false,
                        is_mutex_guard: false,
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
            call_signatures: BTreeMap::new(),
            constraint_call_sites: std::collections::BTreeSet::new(),
            record_layouts: BTreeMap::new(),
            moved_bindings: BTreeMap::new(),
            moved_fields: BTreeMap::new(),
            eventual_moves: BTreeMap::new(),
            recoverable_eventual_obligations: BTreeMap::new(),
            ownership_history: BTreeMap::new(),
            active_borrows: BTreeMap::new(),
            borrow_bindings: BTreeMap::new(),
            borrow_history: BTreeMap::new(),
            owner_borrow_history: BTreeMap::new(),
            returned_borrows: BTreeMap::new(),
            active_mutex_guards: BTreeMap::new(),
            deferred_binding_uses: BTreeMap::new(),
            task_borrowed_bindings: BTreeMap::new(),
            bor_env_closures: BTreeSet::new(),
            deferred_transfer_conflict: None,
            capability_claims: BTreeMap::new(),
            generic_capability_constraints: BTreeMap::new(),
            fin_types: std::collections::BTreeSet::new(),
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

    pub(crate) fn record_call_signature(
        &mut self,
        syntax_id: SyntaxNodeId,
        signature: RoutineType,
    ) {
        self.call_signatures.insert(syntax_id, signature);
    }

    pub(crate) fn call_signature(&self, syntax_id: SyntaxNodeId) -> Option<&RoutineType> {
        self.call_signatures.get(&syntax_id)
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

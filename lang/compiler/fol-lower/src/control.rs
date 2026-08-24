use crate::ids::{
    IdTable, LoweredBlockId, LoweredGlobalId, LoweredInstrId, LoweredLocalId, LoweredRoutineId,
    LoweredTypeId,
};
use fol_intrinsics::IntrinsicId;
use fol_resolver::{SourceUnitId, SymbolId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredOperand {
    Local(LoweredLocalId),
    Global(LoweredGlobalId),
    Int(i64),
    Float(u64),
    Bool(bool),
    Char(char),
    Str(String),
    Nil,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredLocal {
    pub id: LoweredLocalId,
    pub type_id: Option<LoweredTypeId>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweredLinearKind {
    Array,
    Vector,
    Sequence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweredBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoweredUnaryOp {
    Neg,
    Not,
}

/// How a container yields the `fin` values it owns.
/// Growable-container operations reachable through method syntax. Growth is
/// allocation, so every variant here lands in the `memo` runtime tier and above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerMutateOp {
    VecPush,
    VecPop,
    VecInsertAt,
    VecRemoveAt,
    VecClear,
    VecTruncate,
    VecSort,
    VecSwap,
    VecReserve,
    MapInsert,
    MapGet,
    MapRemove,
    MapContains,
    MapClear,
    MapKeys,
    MapValues,
}

impl ContainerMutateOp {
    pub fn method_name(self) -> &'static str {
        match self {
            Self::VecPush => "push",
            Self::VecPop => "pop",
            Self::VecInsertAt => "insert_at",
            Self::VecRemoveAt => "remove_at",
            Self::VecClear | Self::MapClear => "clear",
            Self::VecTruncate => "truncate",
            Self::VecSort => "sort",
            Self::VecSwap => "swap",
            Self::VecReserve => "reserve",
            Self::MapInsert => "insert",
            Self::MapGet => "get",
            Self::MapRemove => "remove",
            Self::MapContains => "contains",
            Self::MapKeys => "keys",
            Self::MapValues => "values",
        }
    }

    /// Number of explicit arguments the method takes, receiver excluded.
    pub fn arity(self) -> usize {
        match self {
            Self::VecPop
            | Self::VecClear
            | Self::VecSort
            | Self::MapClear
            | Self::MapKeys
            | Self::MapValues => 0,
            Self::VecPush
            | Self::VecRemoveAt
            | Self::VecTruncate
            | Self::VecReserve
            | Self::MapGet
            | Self::MapRemove
            | Self::MapContains => 1,
            Self::VecInsertAt | Self::VecSwap | Self::MapInsert => 2,
        }
    }

    /// Whether the operation yields a value. The rest are statements.
    pub fn yields_value(self) -> bool {
        !matches!(
            self,
            Self::VecPush
                | Self::VecInsertAt
                | Self::VecClear
                | Self::VecTruncate
                | Self::VecSort
                | Self::VecSwap
                | Self::VecReserve
                | Self::MapClear
        )
    }

    /// Whether the operation only reads. Reads still route through this
    /// instruction because they address the binding's own local, but they do not
    /// require a mutable place.
    pub fn is_read_only(self) -> bool {
        matches!(
            self,
            Self::MapGet | Self::MapContains | Self::MapKeys | Self::MapValues
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizeEachForm {
    /// `vec`/`seq`: `into_vec()`.
    Linear,
    /// `arr`: a plain Rust array.
    Array,
    /// `set`: `into_set()`.
    Set,
    /// `map`, finalizing the key side.
    MapKey,
    /// `map`, finalizing the value side.
    MapValue,
    /// `opt`: the present payload.
    OptionalPayload,
    /// `err`: the error payload.
    ErrorPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredInstrKind {
    Const(LoweredOperand),
    LoadGlobal {
        global: LoweredGlobalId,
    },
    LoadLocal {
        local: LoweredLocalId,
    },
    CheckRecoverable {
        operand: LoweredLocalId,
    },
    /// Consume a checked recoverable carrier and extract its success value.
    /// The carrier is single-use even when both payload types are clone-safe.
    UnwrapRecoverable {
        operand: LoweredLocalId,
    },
    /// Consume a checked recoverable carrier and extract its error value.
    /// The carrier is single-use even when both payload types are clone-safe.
    ExtractRecoverableError {
        operand: LoweredLocalId,
    },
    StoreLocal {
        local: LoweredLocalId,
        value: LoweredLocalId,
    },
    StoreGlobal {
        global: LoweredGlobalId,
        value: LoweredLocalId,
    },
    /// End a move-only local's lexical lifetime after deferred bodies have
    /// run. Backend transfers leave the named slot holding a default sentinel,
    /// so this drops either the live value or that inert moved-from sentinel.
    DropLocal {
        local: LoweredLocalId,
    },
    /// Assign into a field of a mutable record local, e.g. `counter.total = 5`.
    /// `base` is the record binding's own local (not a cloned copy) so the
    /// store is observed by later reads.
    StoreField {
        base: LoweredLocalId,
        field: String,
        value: LoweredLocalId,
    },
    /// Replace one element of a positional container in place, e.g.
    /// `cells[i] = 7` or `holder.cells[i] = 7`. `base` is the binding's own
    /// local (not a cloned copy) so the store is observed by later reads, and
    /// `field` names the container when it is held in a record field.
    ///
    /// Only `arr[T,N]` and `vec[T]` reach here; typecheck rejects the other
    /// families. Containers are therefore no longer immutable once built, so a
    /// later pass must not memoize or common-subexpression an `IndexAccess`
    /// result across one of these.
    StoreIndex {
        base: LoweredLocalId,
        field: Option<String>,
        index: LoweredLocalId,
        value: LoweredLocalId,
    },
    /// Mutate a growable container in place, e.g. `values.push(7)` or
    /// `self.items.push(7)`. `base` is the binding's own local (not a cloned
    /// copy) so the mutation is observed by later reads, and `field` names the
    /// container when it is held in a record field.
    ///
    /// Carries no `LoweredTypeId` for the same reason `StoreIndex` does not:
    /// the backend derives the container type from the base local plus the
    /// field name, which keeps `mono.rs` out of this instruction entirely.
    ContainerMutate {
        base: LoweredLocalId,
        field: Option<String>,
        op: ContainerMutateOp,
        args: Vec<LoweredLocalId>,
    },
    Call {
        callee: LoweredRoutineId,
        args: Vec<LoweredLocalId>,
        error_type: Option<LoweredTypeId>,
    },
    /// A call into an imported C provider.
    ///
    /// Deliberately not a `Call`: there is no `LoweredRoutineId` because there
    /// is no FOL routine, and the backend has to reach a generated adapter
    /// rather than an internal path. Section 4.13 keeps the C error convention
    /// inside that adapter, so this instruction carries only which adapter to
    /// call and whether the result is recoverable.
    ForeignCall {
        /// The import's namespace alias, which names the adapter module.
        alias: String,
        /// The safe adapter function inside that module.
        adapter: String,
        /// The exact provider symbol, kept for diagnostics and traceability.
        symbol: String,
        args: Vec<LoweredLocalId>,
        error_type: Option<LoweredTypeId>,
        /// The position in `args` holding a routine value the provider will
        /// invoke during the call.
        ///
        /// Carried here because the backend cannot tell a callback from any
        /// other routine-valued argument by its lowered type alone: both are
        /// `Rc<dyn Fn>`. Only the import manifest knows, and this is where that
        /// knowledge reaches codegen.
        callback_arg: Option<usize>,
    },
    SpawnCall {
        callee: LoweredRoutineId,
        args: Vec<LoweredLocalId>,
        /// True for a `[spn, det]` detached task: the backend spawns it without
        /// registering a join handle, so it is not joined at scope/process exit.
        detached: bool,
    },
    AsyncCall {
        callee: LoweredRoutineId,
        args: Vec<LoweredLocalId>,
        error_type: Option<LoweredTypeId>,
    },
    AwaitEventual {
        eventual: LoweredLocalId,
        error_type: Option<LoweredTypeId>,
    },
    ChannelSender {
        channel: LoweredLocalId,
    },
    /// Transfer a channel's unique receiver as a first-class `chn[rx, T]` value.
    ChannelReceiver {
        channel: LoweredLocalId,
    },
    ChannelSend {
        channel: LoweredLocalId,
        value: LoweredLocalId,
    },
    ChannelReceiveOptional {
        channel: LoweredLocalId,
    },
    ChannelTryReceive {
        channel: LoweredLocalId,
    },
    ChannelIsClosed {
        channel: LoweredLocalId,
    },
    ProcessorYield,
    MutexLock {
        mutex: LoweredLocalId,
    },
    MutexUnlock {
        mutex: LoweredLocalId,
    },
    /// Replace the whole guarded value through a held guard. A guard binding is
    /// an alias of its mutex local, so assigning to it looks exactly like
    /// initialising the mutex -- and initialising takes the lock, which a held
    /// guard already owns. Writing through the guard is the only safe form.
    StoreMutexValue {
        mutex: LoweredLocalId,
        value: LoweredLocalId,
    },
    OptionalHasValue {
        operand: LoweredLocalId,
    },
    IntrinsicCall {
        intrinsic: IntrinsicId,
        args: Vec<LoweredLocalId>,
    },
    RuntimeHook {
        intrinsic: IntrinsicId,
        args: Vec<LoweredLocalId>,
        /// Set when the hook reports through the error channel rather than
        /// always producing a value, which makes its result local a
        /// `FolRecover` exactly as a fallible call does.
        error_type: Option<LoweredTypeId>,
    },
    LengthOf {
        operand: LoweredLocalId,
    },
    /// `.type_name(x)` and `.size_of(x)`. Neither carries a `LoweredTypeId`:
    /// the backend reads the operand local's type instead, which is what makes
    /// them correct after monomorphization — `instantiate_template` substitutes
    /// every local's type, so a templated copy sees its concrete type with no
    /// arm needed in `mono.rs`. Baking the answer in at lowering time would
    /// freeze the generic parameter's spelling instead.
    TypeNameOf {
        operand: LoweredLocalId,
    },
    SizeOfValue {
        operand: LoweredLocalId,
    },
    ConstructRecord {
        type_id: LoweredTypeId,
        fields: Vec<(String, LoweredLocalId)>,
    },
    ConstructEntry {
        type_id: LoweredTypeId,
        variant: String,
        payload: Option<LoweredLocalId>,
    },
    ConstructLinear {
        kind: LoweredLinearKind,
        type_id: LoweredTypeId,
        elements: Vec<LoweredLocalId>,
    },
    ConstructSet {
        type_id: LoweredTypeId,
        members: Vec<LoweredLocalId>,
    },
    ConstructMap {
        type_id: LoweredTypeId,
        entries: Vec<(LoweredLocalId, LoweredLocalId)>,
    },
    ConstructOptional {
        type_id: LoweredTypeId,
        value: Option<LoweredLocalId>,
    },
    ConstructOwned {
        type_id: LoweredTypeId,
        value: LoweredLocalId,
    },
    ConsumeOwned {
        value: LoweredLocalId,
    },
    ConstructBorrow {
        type_id: LoweredTypeId,
        owner: LoweredLocalId,
        mutable: bool,
    },
    ReadBorrow {
        borrow: LoweredLocalId,
    },
    ConstructPointer {
        type_id: LoweredTypeId,
        value: LoweredLocalId,
        shared: bool,
    },
    /// `[weak]shared`: downgrade a shared pointer to a weak handle
    /// (`std::rc::Rc::downgrade`).
    WeakDowngrade {
        type_id: LoweredTypeId,
        pointer: LoweredLocalId,
    },
    /// `[upg]weak`: upgrade a weak handle to an optional shared pointer
    /// (`std::rc::Weak::upgrade`).
    WeakUpgrade {
        type_id: LoweredTypeId,
        pointer: LoweredLocalId,
    },
    DerefPointer {
        pointer: LoweredLocalId,
        /// True when dereferencing transfers a move-only pointee out of its
        /// unique pointer. False is an observational, clone-safe read.
        consuming: bool,
    },
    StoreDeref {
        pointer: LoweredLocalId,
        value: LoweredLocalId,
    },
    GiveBackBorrow {
        borrow: LoweredLocalId,
    },
    ConstructError {
        type_id: LoweredTypeId,
        value: Option<LoweredLocalId>,
    },
    FieldAccess {
        base: LoweredLocalId,
        field: String,
    },
    /// Run a `fin` element's finalizer for every value a container holds, at
    /// scope exit. A record field can be named and called directly; a container
    /// holds a runtime number of values, so releasing them needs iteration the
    /// rest of the instruction set does not express.
    FinalizeEach {
        container: LoweredLocalId,
        callee: LoweredRoutineId,
        form: FinalizeEachForm,
    },
    IndexAccess {
        container: LoweredLocalId,
        index: LoweredLocalId,
    },
    SliceAccess {
        container: LoweredLocalId,
        start: LoweredLocalId,
        end: LoweredLocalId,
    },
    Cast {
        operand: LoweredLocalId,
        target_type: LoweredTypeId,
    },
    UnwrapShell {
        operand: LoweredLocalId,
    },
    BinaryOp {
        op: LoweredBinaryOp,
        left: LoweredLocalId,
        right: LoweredLocalId,
    },
    UnaryOp {
        op: LoweredUnaryOp,
        operand: LoweredLocalId,
    },
    RoutineRef {
        routine: LoweredRoutineId,
    },
    /// A first-class routine value with a captured environment: wraps
    /// `routine` (whose leading parameters are the captures) together with the
    /// materialized `env` values into a callable that re-supplies them on
    /// every invocation.
    ClosureRef {
        routine: LoweredRoutineId,
        env: Vec<LoweredLocalId>,
    },
    CallIndirect {
        callee: LoweredLocalId,
        args: Vec<LoweredLocalId>,
        error_type: Option<LoweredTypeId>,
    },
    /// A method call on a constrained generic parameter. The concrete callee
    /// is resolved during monomorphization (args[0] is the receiver); this
    /// variant must never survive into backend emission.
    ConstraintCall {
        method: String,
        args: Vec<LoweredLocalId>,
        error_type: Option<LoweredTypeId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredInstr {
    pub id: LoweredInstrId,
    pub result: Option<LoweredLocalId>,
    pub kind: LoweredInstrKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredTerminator {
    Jump {
        target: LoweredBlockId,
    },
    Branch {
        condition: LoweredLocalId,
        then_block: LoweredBlockId,
        else_block: LoweredBlockId,
    },
    Return {
        value: Option<LoweredLocalId>,
    },
    Report {
        value: Option<LoweredLocalId>,
    },
    Panic {
        value: Option<LoweredLocalId>,
    },
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredBlock {
    pub id: LoweredBlockId,
    pub instructions: Vec<LoweredInstrId>,
    pub terminator: Option<LoweredTerminator>,
}

impl LoweredBlock {
    pub fn is_terminated(&self) -> bool {
        self.terminator.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredRoutine {
    pub id: LoweredRoutineId,
    pub name: String,
    pub symbol_id: Option<SymbolId>,
    pub source_unit_id: Option<SourceUnitId>,
    pub signature: Option<LoweredTypeId>,
    pub receiver_type: Option<LoweredTypeId>,
    pub params: Vec<LoweredLocalId>,
    pub mutex_params: BTreeSet<LoweredLocalId>,
    pub local_symbols: BTreeMap<SymbolId, LoweredLocalId>,
    pub locals: IdTable<LoweredLocalId, LoweredLocal>,
    pub blocks: IdTable<LoweredBlockId, LoweredBlock>,
    pub instructions: IdTable<LoweredInstrId, LoweredInstr>,
    pub entry_block: LoweredBlockId,
    pub body_result: Option<LoweredLocalId>,
    /// Capability bounds declared on this routine's generic parameters, keyed by
    /// the parameter name the backend renders. Only capabilities with a Rust
    /// equivalent become emitted trait bounds; the rest stay FOL-side
    /// obligations checked at the call site.
    pub generic_bounds: BTreeMap<String, BTreeSet<String>>,
}

impl LoweredRoutine {
    pub fn new(id: LoweredRoutineId, name: impl Into<String>, entry_block: LoweredBlockId) -> Self {
        Self {
            id,
            name: name.into(),
            symbol_id: None,
            source_unit_id: None,
            signature: None,
            receiver_type: None,
            params: Vec::new(),
            mutex_params: BTreeSet::new(),
            local_symbols: BTreeMap::new(),
            locals: IdTable::new(),
            blocks: IdTable::new(),
            instructions: IdTable::new(),
            entry_block,
            body_result: None,
            generic_bounds: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LoweredBlock, LoweredInstr, LoweredInstrKind, LoweredLocal, LoweredOperand, LoweredRoutine,
        LoweredTerminator,
    };
    use crate::ids::{LoweredBlockId, LoweredInstrId, LoweredLocalId, LoweredRoutineId};

    #[test]
    fn lowered_routine_shell_keeps_entry_block_and_named_locals() {
        let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
        let local_id = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: None,
            name: Some("tmp".to_string()),
        });

        assert_eq!(routine.entry_block, LoweredBlockId(0));
        assert_eq!(local_id, LoweredLocalId(0));
        assert_eq!(
            routine
                .locals
                .get(local_id)
                .and_then(|local| local.name.as_deref()),
            Some("tmp")
        );
    }

    #[test]
    fn lowered_blocks_and_terminators_form_a_control_shell() {
        let block = LoweredBlock {
            id: LoweredBlockId(1),
            instructions: vec![LoweredInstrId(0)],
            terminator: Some(LoweredTerminator::Return {
                value: Some(LoweredLocalId(0)),
            }),
        };
        let instr = LoweredInstr {
            id: LoweredInstrId(0),
            result: Some(LoweredLocalId(0)),
            kind: LoweredInstrKind::Const(LoweredOperand::Int(42)),
        };

        assert_eq!(block.id, LoweredBlockId(1));
        assert_eq!(block.instructions, vec![LoweredInstrId(0)]);
        assert!(block.is_terminated());
        assert_eq!(instr.result, Some(LoweredLocalId(0)));
    }

    #[test]
    fn lowered_blocks_report_missing_terminators_explicitly() {
        let block = LoweredBlock {
            id: LoweredBlockId(2),
            instructions: Vec::new(),
            terminator: None,
        };

        assert!(!block.is_terminated());
    }
}

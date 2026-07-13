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
    UnwrapRecoverable {
        operand: LoweredLocalId,
    },
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
    /// End the lexical lifetime of a still-owned local after deferred bodies
    /// have run. Move analysis excludes locals whose ownership already left
    /// the scope, so this never double-drops a transferred value.
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
    Call {
        callee: LoweredRoutineId,
        args: Vec<LoweredLocalId>,
        error_type: Option<LoweredTypeId>,
    },
    SpawnCall {
        callee: LoweredRoutineId,
        args: Vec<LoweredLocalId>,
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
    ChannelSend {
        channel: LoweredLocalId,
        value: LoweredLocalId,
    },
    ChannelReceive {
        channel: LoweredLocalId,
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
    },
    LengthOf {
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
    ConstructPointer {
        type_id: LoweredTypeId,
        value: LoweredLocalId,
        shared: bool,
    },
    DerefPointer {
        pointer: LoweredLocalId,
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

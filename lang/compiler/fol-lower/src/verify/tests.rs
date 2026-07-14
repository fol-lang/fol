use super::verify_workspace;
use crate::{
    control::{
        LoweredBlock, LoweredInstr, LoweredInstrKind, LoweredLocal, LoweredRoutine,
        LoweredTerminator,
    },
    ids::{
        LoweredBlockId, LoweredInstrId, LoweredLocalId, LoweredPackageId, LoweredRoutineId,
        LoweredTypeId,
    },
    model::{
        LoweredPackage, LoweredRecoverableAbi, LoweredSourceMap, LoweredSymbolOwnership,
        LoweredWorkspace,
    },
    types::{LoweredBuiltinType, LoweredTypeTable},
};
use fol_resolver::{
    MountedSymbolProvenance, PackageIdentity, PackageSourceKind, SourceUnitId, SymbolId,
};
use std::collections::BTreeMap;

fn identity(name: &str) -> PackageIdentity {
    PackageIdentity {
        source_kind: PackageSourceKind::Entry,
        canonical_root: format!("/workspace/{name}"),
        display_name: name.to_string(),
    }
}

fn empty_workspace(identity: PackageIdentity, package: LoweredPackage) -> LoweredWorkspace {
    let mut type_table = LoweredTypeTable::new();
    let recoverable_abi =
        LoweredRecoverableAbi::v1(type_table.intern_builtin(LoweredBuiltinType::Bool));
    LoweredWorkspace::new(
        identity.clone(),
        BTreeMap::from([(identity, package)]),
        Vec::new(),
        type_table,
        LoweredSourceMap::new(),
        recoverable_abi,
    )
}

#[test]
fn verifier_rejects_missing_branch_targets() {
    let identity = identity("app");
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: Some(LoweredTerminator::Jump {
            target: LoweredBlockId(9),
        }),
    });
    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = empty_workspace(identity, package);

    let errors =
        verify_workspace(&workspace).expect_err("verifier should reject missing jump targets");

    assert!(errors
        .iter()
        .any(|error| error.message().contains("missing block 9")));
}

#[test]
fn verifier_rejects_unreachable_blocks() {
    let identity = identity("app");
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: Some(LoweredTerminator::Return { value: None }),
    });
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(1),
        instructions: Vec::new(),
        terminator: Some(LoweredTerminator::Return { value: None }),
    });
    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = empty_workspace(identity, package);

    let errors =
        verify_workspace(&workspace).expect_err("verifier should reject unreachable blocks");

    assert!(errors
        .iter()
        .any(|error| error.message().contains("unreachable block 1")));
}

#[test]
fn verifier_rejects_dangling_locals_and_missing_type_ids() {
    let identity = identity("app");
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(LoweredTypeId(9)),
        name: Some("bad".to_string()),
    });
    routine.instructions.push(LoweredInstr {
        id: LoweredInstrId(0),
        result: Some(LoweredLocalId(1)),
        kind: LoweredInstrKind::LoadLocal {
            local: LoweredLocalId(2),
        },
    });
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: vec![LoweredInstrId(0)],
        terminator: Some(LoweredTerminator::Return {
            value: Some(LoweredLocalId(1)),
        }),
    });

    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = empty_workspace(identity, package);

    let errors = verify_workspace(&workspace)
        .expect_err("verifier should reject missing locals and missing lowered type ids");

    assert!(errors
        .iter()
        .any(|error| error.message().contains("references missing type 9")));
    assert!(errors
        .iter()
        .any(|error| error.message().contains("writes to missing local 1")));
    assert!(errors
        .iter()
        .any(|error| error.message().contains("uses missing operand local 2")));
    assert!(errors
        .iter()
        .any(|error| error.message().contains("return uses missing local 1")));
}

#[test]
fn verifier_rejects_impossible_mounted_symbol_ownership() {
    let app_id = identity("app");
    let foreign = identity("shared");
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: Some(LoweredTerminator::Return { value: None }),
    });

    let mut package = LoweredPackage::new(LoweredPackageId(0), app_id.clone());
    package.symbol_ownership.insert(
        SymbolId(7),
        LoweredSymbolOwnership {
            symbol_id: SymbolId(7),
            source_unit_id: SourceUnitId(0),
            owning_package: foreign.clone(),
            mounted_from: Some(MountedSymbolProvenance {
                package_identity: app_id.clone(),
                foreign_symbol: SymbolId(3),
            }),
        },
    );
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = empty_workspace(app_id, package);

    let errors = verify_workspace(&workspace)
        .expect_err("verifier should reject conflicting mounted symbol ownership");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("recorded mounted symbol 7 with conflicting owning package")
    }));
}

#[test]
fn verifier_rejects_intrinsic_calls_using_non_pure_intrinsics() {
    let identity = identity("app");
    let mut type_table = LoweredTypeTable::new();
    let bool_type = type_table.intern_builtin(LoweredBuiltinType::Bool);
    let recoverable_abi = LoweredRecoverableAbi::v1(bool_type);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(bool_type),
        name: Some("flag".to_string()),
    });
    routine.instructions.push(LoweredInstr {
        id: LoweredInstrId(0),
        result: Some(LoweredLocalId(0)),
        kind: LoweredInstrKind::IntrinsicCall {
            intrinsic: fol_intrinsics::intrinsic_by_canonical_name("echo")
                .expect("echo should exist")
                .id,
            args: vec![LoweredLocalId(0)],
        },
    });
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: vec![LoweredInstrId(0)],
        terminator: Some(LoweredTerminator::Return {
            value: Some(LoweredLocalId(0)),
        }),
    });

    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = LoweredWorkspace::new(
        identity.clone(),
        BTreeMap::from([(identity, package)]),
        Vec::new(),
        type_table,
        LoweredSourceMap::new(),
        recoverable_abi,
    );

    let errors = verify_workspace(&workspace)
        .expect_err("verifier should reject runtime hooks lowered as intrinsic calls");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("uses intrinsic '.echo' as an IntrinsicCall")
    }));
}

#[test]
fn verifier_rejects_runtime_hooks_with_results_and_helper_without_results() {
    let identity = identity("app");
    let mut type_table = LoweredTypeTable::new();
    let bool_type = type_table.intern_builtin(LoweredBuiltinType::Bool);
    let int_type = type_table.intern_builtin(LoweredBuiltinType::Int);
    let seq_type = type_table.intern(crate::types::LoweredType::Sequence { element_type: bool_type });
    let recoverable_abi = LoweredRecoverableAbi::v1(bool_type);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(bool_type),
        name: Some("flag".to_string()),
    });
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(seq_type),
        name: Some("items".to_string()),
    });
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(int_type),
        name: Some("count".to_string()),
    });
    routine.instructions.push(LoweredInstr {
        id: LoweredInstrId(0),
        result: Some(LoweredLocalId(2)),
        kind: LoweredInstrKind::RuntimeHook {
            intrinsic: fol_intrinsics::intrinsic_by_canonical_name("echo")
                .expect("echo should exist")
                .id,
            args: vec![LoweredLocalId(0)],
        },
    });
    routine.instructions.push(LoweredInstr {
        id: LoweredInstrId(1),
        result: None,
        kind: LoweredInstrKind::LengthOf {
            operand: LoweredLocalId(1),
        },
    });
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: vec![LoweredInstrId(0), LoweredInstrId(1)],
        terminator: Some(LoweredTerminator::Return {
            value: Some(LoweredLocalId(0)),
        }),
    });

    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = LoweredWorkspace::new(
        identity.clone(),
        BTreeMap::from([(identity, package)]),
        Vec::new(),
        type_table,
        LoweredSourceMap::new(),
        recoverable_abi,
    );

    let errors = verify_workspace(&workspace)
        .expect_err("verifier should reject impossible runtime-hook and helper result shapes");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("runtime hook instruction 0 must not write result local 2")
    }));
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("length helper instruction 1 must write a result local")
    }));
}

#[test]
fn verifier_rejects_recoverable_helpers_on_non_call_results() {
    let identity = identity("app");
    let mut type_table = LoweredTypeTable::new();
    let bool_type = type_table.intern_builtin(LoweredBuiltinType::Bool);
    let recoverable_abi = LoweredRecoverableAbi::v1(bool_type);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(bool_type),
        name: Some("flag".to_string()),
    });
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(bool_type),
        name: Some("checked".to_string()),
    });
    routine.instructions.push(LoweredInstr {
        id: LoweredInstrId(0),
        result: Some(LoweredLocalId(0)),
        kind: LoweredInstrKind::Const(crate::LoweredOperand::Bool(true)),
    });
    routine.instructions.push(LoweredInstr {
        id: LoweredInstrId(1),
        result: Some(LoweredLocalId(1)),
        kind: LoweredInstrKind::CheckRecoverable {
            operand: LoweredLocalId(0),
        },
    });
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: vec![LoweredInstrId(0), LoweredInstrId(1)],
        terminator: Some(LoweredTerminator::Return {
            value: Some(LoweredLocalId(1)),
        }),
    });

    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = LoweredWorkspace::new(
        identity.clone(),
        BTreeMap::from([(identity, package)]),
        Vec::new(),
        type_table,
        LoweredSourceMap::new(),
        recoverable_abi,
    );

    let errors = verify_workspace(&workspace)
        .expect_err("verifier should reject recoverable helpers on plain locals");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("expects a recoverable call-result operand local 0")
    }));
}

#[test]
fn verifier_rejects_reading_move_only_aggregates_out_of_borrows() {
    let identity = identity("app");
    let mut type_table = LoweredTypeTable::new();
    let bool_type = type_table.intern_builtin(LoweredBuiltinType::Bool);
    let int_type = type_table.intern_builtin(LoweredBuiltinType::Int);
    let pointer_type = type_table.intern(crate::types::LoweredType::Pointer {
        target: int_type,
        shared: false,
    });
    let record_type = type_table.intern(crate::types::LoweredType::Record {
        fields: BTreeMap::from([("pointer".to_string(), pointer_type)]),
    });
    let borrow_type = type_table.intern(crate::types::LoweredType::Borrowed {
        inner: record_type,
        mutable: false,
    });
    let recoverable_abi = LoweredRecoverableAbi::v1(bool_type);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "read", LoweredBlockId(0));
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(borrow_type),
        name: Some("borrow".to_string()),
    });
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(record_type),
        name: Some("result".to_string()),
    });
    routine.instructions.push(LoweredInstr {
        id: LoweredInstrId(0),
        result: Some(LoweredLocalId(1)),
        kind: LoweredInstrKind::ReadBorrow {
            borrow: LoweredLocalId(0),
        },
    });
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: vec![LoweredInstrId(0)],
        terminator: Some(LoweredTerminator::Return {
            value: Some(LoweredLocalId(1)),
        }),
    });
    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = LoweredWorkspace::new(
        identity.clone(),
        BTreeMap::from([(identity, package)]),
        Vec::new(),
        type_table,
        LoweredSourceMap::new(),
        recoverable_abi,
    );

    let errors = verify_workspace(&workspace)
        .expect_err("verifier should reject move-only borrow reads");
    assert!(errors.iter().any(|error| error
        .message()
        .contains("reads a move-only or generic value out of a borrow")));
}

#[test]
fn verifier_rejects_generic_parameter_leaked_into_non_generic_routine() {
    // Hardening round 5 P1 safety net: monomorphization must erase generic
    // parameters before backend emission. A routine whose signature declares no
    // generics but whose local still references a `GenericParameter` (for
    // example a leaked `seq[t]` variadic pack) would emit as an unbound Rust
    // type parameter, so the verifier must reject it.
    let identity = identity("app");
    let mut type_table = LoweredTypeTable::new();
    let bool_type = type_table.intern_builtin(LoweredBuiltinType::Bool);
    let generic = type_table.intern(crate::types::LoweredType::GenericParameter {
        name: "t".to_string(),
    });
    let leaked_pack = type_table.intern(crate::types::LoweredType::Sequence {
        element_type: generic,
    });
    let recoverable_abi = LoweredRecoverableAbi::v1(bool_type);

    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    // No signature => the routine declares no generic parameters.
    routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(leaked_pack),
        name: Some("pack".to_string()),
    });
    routine.blocks.push(LoweredBlock {
        id: LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: Some(LoweredTerminator::Return { value: None }),
    });

    let mut package = LoweredPackage::new(LoweredPackageId(0), identity.clone());
    package.routine_decls.insert(LoweredRoutineId(0), routine);
    let workspace = LoweredWorkspace::new(
        identity.clone(),
        BTreeMap::from([(identity, package)]),
        Vec::new(),
        type_table,
        LoweredSourceMap::new(),
        recoverable_abi,
    );

    let errors = verify_workspace(&workspace)
        .expect_err("verifier should reject leaked generic parameters");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("leaks generic parameter 't'")),
        "expected a leaked-generic-parameter diagnostic, got: {errors:?}"
    );
}

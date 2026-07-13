use super::super::render_core_instruction;
use super::super::render_core_instruction_in_workspace;
use crate::testing::package_identity;
use crate::BackendErrorKind;
use fol_intrinsics::intrinsic_by_canonical_name;
use fol_lower::{
    LoweredBlockId, LoweredBuiltinType, LoweredInstr, LoweredInstrId, LoweredInstrKind,
    LoweredLocal, LoweredLocalId, LoweredOperand, LoweredPackage, LoweredRecoverableAbi,
    LoweredRoutine, LoweredRoutineId, LoweredSourceMap, LoweredSourceUnit, LoweredType,
    LoweredTypeTable, LoweredWorkspace,
};
use fol_resolver::{PackageSourceKind, SourceUnitId, SymbolId};
use std::collections::BTreeMap;

#[test]
fn core_instruction_rendering_covers_constants_and_local_global_storage_shapes() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));
    let result_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let other_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("other".to_string()),
    });

    let const_instr = LoweredInstr {
        id: LoweredInstrId(0),
        result: Some(result_local),
        kind: LoweredInstrKind::Const(LoweredOperand::Int(7)),
    };
    let load_local = LoweredInstr {
        id: LoweredInstrId(1),
        result: Some(other_local),
        kind: LoweredInstrKind::LoadLocal {
            local: result_local,
        },
    };
    let store_local = LoweredInstr {
        id: LoweredInstrId(2),
        result: None,
        kind: LoweredInstrKind::StoreLocal {
            local: result_local,
            value: other_local,
        },
    };

    let const_rendered =
        render_core_instruction(&package_identity, &table, &routine, &const_instr).expect("const");
    let load_local_rendered =
        render_core_instruction(&package_identity, &table, &routine, &load_local).expect("load");
    let store_local_rendered =
        render_core_instruction(&package_identity, &table, &routine, &store_local).expect("store");

    assert!(const_rendered.contains("l__pkg__entry__app__r0__l0__value = 7_i64;"));
    assert!(load_local_rendered.contains(
        "l__pkg__entry__app__r0__l1__other = l__pkg__entry__app__r0__l0__value.clone();"
    ));
    assert!(store_local_rendered.contains(
        "l__pkg__entry__app__r0__l0__value = l__pkg__entry__app__r0__l1__other.clone();"
    ));

    let _ = SourceUnitId(0);
    let _ = SymbolId(0);
}

#[test]
fn core_instruction_rendering_emits_plain_routine_calls_for_non_recoverable_sites() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(3), "main", LoweredBlockId(0));
    let arg_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let result_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("result".to_string()),
    });
    let call = LoweredInstr {
        id: LoweredInstrId(3),
        result: Some(result_local),
        kind: LoweredInstrKind::Call {
            callee: LoweredRoutineId(9),
            args: vec![arg_local],
            error_type: None,
        },
    };

    let mut callee_routine = LoweredRoutine::new(LoweredRoutineId(9), "callee", LoweredBlockId(0));
    callee_routine.source_unit_id = Some(SourceUnitId(0));
    let mut package = LoweredPackage::new(fol_lower::LoweredPackageId(0), package_identity.clone());
    package.source_units.push(LoweredSourceUnit {
        source_unit_id: SourceUnitId(0),
        path: "app/main.fol".to_string(),
        package: "app".to_string(),
        namespace: "app".to_string(),
    });
    package
        .routine_decls
        .insert(LoweredRoutineId(9), callee_routine);
    let workspace = LoweredWorkspace::new(
        package_identity.clone(),
        BTreeMap::from([(package_identity.clone(), package)]),
        Vec::new(),
        table.clone(),
        LoweredSourceMap::new(),
        LoweredRecoverableAbi::v1(bool_id),
    );

    let rendered = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &call,
    )
    .expect("call");

    assert!(rendered.contains(
        "l__pkg__entry__app__r3__l1__result = crate::packages::pkg__entry__app::root::r__pkg__entry__app__r9__callee("
    ));
    assert!(rendered.contains("l__pkg__entry__app__r3__l0__value"));
}

#[test]
fn core_instruction_rendering_emits_record_field_accesses_as_native_member_reads() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(4), "main", LoweredBlockId(0));
    let base_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("user".to_string()),
    });
    let result_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("age".to_string()),
    });
    let access = LoweredInstr {
        id: LoweredInstrId(4),
        result: Some(result_local),
        kind: LoweredInstrKind::FieldAccess {
            base: base_local,
            field: "age".to_string(),
        },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &access)
        .expect("field access");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r4__l1__age = l__pkg__entry__app__r4__l0__user.age.clone();"
    );
}

#[test]
fn core_instruction_rendering_moves_unique_record_fields() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let pointer_id = table.intern(LoweredType::Pointer {
        target: int_id,
        shared: false,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(41), "main", LoweredBlockId(0));
    let base = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: None,
        name: Some("holder".to_string()),
    });
    let pointer = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(pointer_id),
        name: Some("pointer".to_string()),
    });
    let access = LoweredInstr {
        id: LoweredInstrId(41),
        result: Some(pointer),
        kind: LoweredInstrKind::FieldAccess {
            base,
            field: "pointer".to_string(),
        },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &access)
        .expect("unique field access");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r41__l1__pointer = l__pkg__entry__app__r41__l0__holder.pointer;"
    );
}

#[test]
fn core_instruction_rendering_emits_scalar_intrinsics_as_native_rust_ops() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(5), "main", LoweredBlockId(0));
    let lhs = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("lhs".to_string()),
    });
    let rhs = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("rhs".to_string()),
    });
    let bool_value = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(bool_id),
        name: Some("flag".to_string()),
    });
    let eq_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(bool_id),
        name: Some("same".to_string()),
    });
    let not_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(bool_id),
        name: Some("flipped".to_string()),
    });
    let eq_instr = LoweredInstr {
        id: LoweredInstrId(5),
        result: Some(eq_result),
        kind: LoweredInstrKind::IntrinsicCall {
            intrinsic: intrinsic_by_canonical_name("eq").expect("eq").id,
            args: vec![lhs, rhs],
        },
    };
    let not_instr = LoweredInstr {
        id: LoweredInstrId(6),
        result: Some(not_result),
        kind: LoweredInstrKind::IntrinsicCall {
            intrinsic: intrinsic_by_canonical_name("not").expect("not").id,
            args: vec![bool_value],
        },
    };

    let eq_rendered =
        render_core_instruction(&package_identity, &table, &routine, &eq_instr).expect("eq");
    let not_rendered =
        render_core_instruction(&package_identity, &table, &routine, &not_instr).expect("not");

    assert_eq!(
        eq_rendered,
        "l__pkg__entry__app__r5__l3__same = l__pkg__entry__app__r5__l0__lhs == l__pkg__entry__app__r5__l1__rhs;"
    );
    assert_eq!(
        not_rendered,
        "l__pkg__entry__app__r5__l4__flipped = !l__pkg__entry__app__r5__l2__flag;"
    );
}

#[test]
fn workspace_global_rendering_uses_fol_default_initializers_for_mutable_globals() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(15), "main", LoweredBlockId(0));
    let value_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let result_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("loaded".to_string()),
    });

    let load = LoweredInstr {
        id: LoweredInstrId(20),
        result: Some(result_local),
        kind: LoweredInstrKind::LoadGlobal {
            global: fol_lower::LoweredGlobalId(0),
        },
    };
    let store = LoweredInstr {
        id: LoweredInstrId(21),
        result: None,
        kind: LoweredInstrKind::StoreGlobal {
            global: fol_lower::LoweredGlobalId(0),
            value: value_local,
        },
    };

    let mut package = LoweredPackage::new(fol_lower::LoweredPackageId(0), package_identity.clone());
    package.source_units.push(LoweredSourceUnit {
        source_unit_id: SourceUnitId(0),
        path: "app/main.fol".to_string(),
        package: "app".to_string(),
        namespace: "app".to_string(),
    });
    package.global_decls.insert(
        fol_lower::LoweredGlobalId(0),
        fol_lower::LoweredGlobal {
            id: fol_lower::LoweredGlobalId(0),
            symbol_id: SymbolId(20),
            source_unit_id: SourceUnitId(0),
            name: "counter".to_string(),
            type_id: int_id,
            mutable: true,
        },
    );
    let workspace = LoweredWorkspace::new(
        package_identity.clone(),
        BTreeMap::from([(package_identity.clone(), package)]),
        Vec::new(),
        table.clone(),
        LoweredSourceMap::new(),
        LoweredRecoverableAbi::v1(int_id),
    );

    let load_rendered = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &load,
    )
    .expect("load global");
    let store_rendered = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &store,
    )
    .expect("store global");

    assert!(load_rendered.contains("get_or_init(|| std::sync::Mutex::new(0_i64))"));
    assert!(load_rendered.contains(".lock().unwrap_or_else(|e| e.into_inner()).clone()"));
    assert!(store_rendered.contains(
        "*crate::packages::pkg__entry__app::root::g__pkg__entry__app__g0__counter.get_or_init(|| std::sync::Mutex::new(0_i64)).lock().unwrap_or_else(|e| e.into_inner()) = l__pkg__entry__app__r15__l0__value.clone();"
    ));
}

#[test]
fn field_stores_move_unique_values_and_global_storage_rejects_them() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let owned_id = table.intern(LoweredType::Owned { inner: int_id });
    let unique_pointer_id = table.intern(LoweredType::Pointer {
        target: int_id,
        shared: false,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(16), "main", LoweredBlockId(0));
    let base = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("record".to_string()),
    });
    let owned = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(owned_id),
        name: Some("child".to_string()),
    });
    let unique_pointer = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(unique_pointer_id),
        name: Some("pointer".to_string()),
    });

    let field_store = LoweredInstr {
        id: LoweredInstrId(22),
        result: None,
        kind: LoweredInstrKind::StoreField {
            base,
            field: "child".to_string(),
            value: owned,
        },
    };
    let global_store = LoweredInstr {
        id: LoweredInstrId(23),
        result: None,
        kind: LoweredInstrKind::StoreGlobal {
            global: fol_lower::LoweredGlobalId(0),
            value: unique_pointer,
        },
    };
    let global_load = LoweredInstr {
        id: LoweredInstrId(24),
        result: Some(unique_pointer),
        kind: LoweredInstrKind::LoadGlobal {
            global: fol_lower::LoweredGlobalId(0),
        },
    };

    let mut package = LoweredPackage::new(fol_lower::LoweredPackageId(0), package_identity.clone());
    package.source_units.push(LoweredSourceUnit {
        source_unit_id: SourceUnitId(0),
        path: "app/main.fol".to_string(),
        package: "app".to_string(),
        namespace: "app".to_string(),
    });
    package.global_decls.insert(
        fol_lower::LoweredGlobalId(0),
        fol_lower::LoweredGlobal {
            id: fol_lower::LoweredGlobalId(0),
            symbol_id: SymbolId(21),
            source_unit_id: SourceUnitId(0),
            name: "pointer".to_string(),
            type_id: unique_pointer_id,
            mutable: true,
        },
    );
    let workspace = LoweredWorkspace::new(
        package_identity.clone(),
        BTreeMap::from([(package_identity.clone(), package)]),
        Vec::new(),
        table.clone(),
        LoweredSourceMap::new(),
        LoweredRecoverableAbi::v1(int_id),
    );

    let field_rendered = render_core_instruction(&package_identity, &table, &routine, &field_store)
        .expect("field store");
    let global_error = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &global_store,
    )
    .expect_err("move-only globals must stop before store emission");
    let load_error = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &global_load,
    )
    .expect_err("move-only globals must stop before clone-based load emission");

    assert_eq!(
        field_rendered,
        "l__pkg__entry__app__r16__l0__record.child = l__pkg__entry__app__r16__l1__child;"
    );
    assert!(!field_rendered.contains(".clone()"));
    assert_eq!(global_error.kind(), BackendErrorKind::InvalidInput);
    assert!(global_error.message().contains("move-only values"));
    assert_eq!(load_error.kind(), BackendErrorKind::InvalidInput);
    assert!(load_error.message().contains("move-only values"));
}

#[test]
fn combined_core_instruction_snapshot_stays_stable() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(6), "main", LoweredBlockId(0));
    let lhs = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("lhs".to_string()),
    });
    let rhs = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("rhs".to_string()),
    });
    let flag = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(bool_id),
        name: Some("flag".to_string()),
    });
    let tmp = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(int_id),
        name: Some("tmp".to_string()),
    });
    let bool_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(bool_id),
        name: Some("same".to_string()),
    });

    let mut callee_routine = LoweredRoutine::new(LoweredRoutineId(8), "callee", LoweredBlockId(0));
    callee_routine.source_unit_id = Some(SourceUnitId(0));
    let mut package = LoweredPackage::new(fol_lower::LoweredPackageId(0), package_identity.clone());
    package.source_units.push(LoweredSourceUnit {
        source_unit_id: SourceUnitId(0),
        path: "app/main.fol".to_string(),
        package: "app".to_string(),
        namespace: "app".to_string(),
    });
    package
        .routine_decls
        .insert(LoweredRoutineId(8), callee_routine);
    let workspace = LoweredWorkspace::new(
        package_identity.clone(),
        BTreeMap::from([(package_identity.clone(), package)]),
        Vec::new(),
        table.clone(),
        LoweredSourceMap::new(),
        LoweredRecoverableAbi::v1(bool_id),
    );

    let rendered = [
        LoweredInstr {
            id: LoweredInstrId(10),
            result: Some(tmp),
            kind: LoweredInstrKind::Const(LoweredOperand::Int(7)),
        },
        LoweredInstr {
            id: LoweredInstrId(11),
            result: Some(lhs),
            kind: LoweredInstrKind::LoadLocal { local: tmp },
        },
        LoweredInstr {
            id: LoweredInstrId(12),
            result: None,
            kind: LoweredInstrKind::StoreLocal {
                local: rhs,
                value: lhs,
            },
        },
        LoweredInstr {
            id: LoweredInstrId(13),
            result: Some(tmp),
            kind: LoweredInstrKind::Call {
                callee: LoweredRoutineId(8),
                args: vec![lhs, rhs],
                error_type: None,
            },
        },
        LoweredInstr {
            id: LoweredInstrId(14),
            result: Some(bool_result),
            kind: LoweredInstrKind::IntrinsicCall {
                intrinsic: intrinsic_by_canonical_name("eq").expect("eq").id,
                args: vec![lhs, rhs],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(15),
            result: Some(bool_result),
            kind: LoweredInstrKind::IntrinsicCall {
                intrinsic: intrinsic_by_canonical_name("not").expect("not").id,
                args: vec![flag],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(16),
            result: Some(tmp),
            kind: LoweredInstrKind::FieldAccess {
                base: rhs,
                field: "count".to_string(),
            },
        },
    ]
    .iter()
    .map(|instruction| {
        render_core_instruction_in_workspace(
            Some(&workspace),
            &package_identity,
            &table,
            &routine,
            instruction,
        )
    })
    .collect::<Result<Vec<_>, _>>()
    .expect("snapshot should render")
    .join("\n");

    assert_eq!(
        rendered,
        concat!(
            "l__pkg__entry__app__r6__l3__tmp = 7_i64;\n",
            "l__pkg__entry__app__r6__l0__lhs = l__pkg__entry__app__r6__l3__tmp.clone();\n",
            "l__pkg__entry__app__r6__l1__rhs = l__pkg__entry__app__r6__l0__lhs.clone();\n",
            "l__pkg__entry__app__r6__l3__tmp = crate::packages::pkg__entry__app::root::r__pkg__entry__app__r8__callee(l__pkg__entry__app__r6__l0__lhs.clone(), l__pkg__entry__app__r6__l1__rhs.clone());\n",
            "l__pkg__entry__app__r6__l4__same = l__pkg__entry__app__r6__l0__lhs == l__pkg__entry__app__r6__l1__rhs;\n",
            "l__pkg__entry__app__r6__l4__same = !l__pkg__entry__app__r6__l2__flag;\n",
            "l__pkg__entry__app__r6__l3__tmp = l__pkg__entry__app__r6__l1__rhs.count.clone();"
        )
    );
}

#[test]
fn core_instruction_rendering_emits_routine_ref_as_fn_pointer_cast() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
    let fn_sig = table.intern(fol_lower::LoweredType::Routine(
        fol_lower::LoweredRoutineType {
            params: vec![int_id],
            return_type: Some(int_id),
            error_type: None,
        },
    ));
    let mut routine = LoweredRoutine::new(LoweredRoutineId(10), "caller", LoweredBlockId(0));
    let result_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(fn_sig),
        name: Some("fptr".to_string()),
    });

    let mut callee_routine = LoweredRoutine::new(LoweredRoutineId(11), "target", LoweredBlockId(0));
    callee_routine.source_unit_id = Some(SourceUnitId(0));
    callee_routine.signature = Some(fn_sig);
    let mut mutex_routine =
        LoweredRoutine::new(LoweredRoutineId(12), "mutex_target", LoweredBlockId(0));
    mutex_routine.source_unit_id = Some(SourceUnitId(0));
    mutex_routine.signature = Some(fn_sig);
    mutex_routine.mutex_params.insert(LoweredLocalId(0));
    let mut package = LoweredPackage::new(fol_lower::LoweredPackageId(0), package_identity.clone());
    package.source_units.push(LoweredSourceUnit {
        source_unit_id: SourceUnitId(0),
        path: "app/main.fol".to_string(),
        package: "app".to_string(),
        namespace: "app".to_string(),
    });
    package
        .routine_decls
        .insert(LoweredRoutineId(11), callee_routine);
    package
        .routine_decls
        .insert(LoweredRoutineId(12), mutex_routine);
    let workspace = LoweredWorkspace::new(
        package_identity.clone(),
        BTreeMap::from([(package_identity.clone(), package)]),
        Vec::new(),
        table.clone(),
        LoweredSourceMap::new(),
        LoweredRecoverableAbi::v1(bool_id),
    );

    let routine_ref = LoweredInstr {
        id: LoweredInstrId(20),
        result: Some(result_local),
        kind: LoweredInstrKind::RoutineRef {
            routine: LoweredRoutineId(11),
        },
    };
    let rendered = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &routine_ref,
    )
    .expect("routine ref");

    assert!(rendered.contains("l__pkg__entry__app__r10__l0__fptr"));
    assert!(rendered.contains("r__pkg__entry__app__r11__target"));
    assert!(rendered.contains(" as "));

    let mutex_ref = LoweredInstr {
        id: LoweredInstrId(21),
        result: Some(result_local),
        kind: LoweredInstrKind::RoutineRef {
            routine: LoweredRoutineId(12),
        },
    };
    let error = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &mutex_ref,
    )
    .expect_err("[mux] routine references must stop before fn-pointer casting");
    assert_eq!(error.kind(), BackendErrorKind::Unsupported);
    assert!(error.message().contains("[mux] parameters"));
}

#[test]
fn core_instruction_rendering_emits_call_indirect_with_callee_local() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let fn_sig = table.intern(fol_lower::LoweredType::Routine(
        fol_lower::LoweredRoutineType {
            params: vec![int_id],
            return_type: Some(int_id),
            error_type: None,
        },
    ));
    let mut routine = LoweredRoutine::new(LoweredRoutineId(12), "main", LoweredBlockId(0));
    let callee_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(fn_sig),
        name: Some("callback".to_string()),
    });
    let arg_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("arg".to_string()),
    });
    let result_local = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(int_id),
        name: Some("out".to_string()),
    });

    let call_indirect = LoweredInstr {
        id: LoweredInstrId(21),
        result: Some(result_local),
        kind: LoweredInstrKind::CallIndirect {
            callee: callee_local,
            args: vec![arg_local],
            error_type: None,
        },
    };
    let rendered = render_core_instruction(&package_identity, &table, &routine, &call_indirect)
        .expect("call indirect");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r12__l2__out = l__pkg__entry__app__r12__l0__callback(l__pkg__entry__app__r12__l1__arg);"
    );

    let void_indirect = LoweredInstr {
        id: LoweredInstrId(22),
        result: None,
        kind: LoweredInstrKind::CallIndirect {
            callee: callee_local,
            args: vec![arg_local],
            error_type: None,
        },
    };
    let void_rendered =
        render_core_instruction(&package_identity, &table, &routine, &void_indirect)
            .expect("void indirect");

    assert_eq!(
        void_rendered,
        "l__pkg__entry__app__r12__l0__callback(l__pkg__entry__app__r12__l1__arg);"
    );
}

use super::super::render_core_instruction;
use super::super::render_core_instruction_in_workspace;
use crate::testing::package_identity;
use crate::BackendErrorKind;
use fol_lower::{
    control::LoweredLinearKind, LoweredBlockId, LoweredBuiltinType, LoweredFieldLayout,
    LoweredInstr, LoweredInstrId, LoweredInstrKind, LoweredLocal, LoweredLocalId, LoweredPackage,
    LoweredRecoverableAbi, LoweredRoutine, LoweredRoutineId, LoweredSourceMap, LoweredType,
    LoweredTypeDecl, LoweredTypeDeclKind, LoweredTypeTable, LoweredVariantLayout, LoweredWorkspace,
};
use fol_resolver::{PackageSourceKind, SourceUnitId, SymbolId};
use std::collections::BTreeMap;

#[test]
fn aggregate_and_container_rendering_emits_native_array_literals() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let array_id = table.intern(fol_lower::LoweredType::Array {
        element_type: int_id,
        size: Some(2),
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(16), "main", LoweredBlockId(0));
    let a = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("a".to_string()),
    });
    let b = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("b".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(array_id),
        name: Some("arr".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(40),
        result: Some(result),
        kind: LoweredInstrKind::ConstructLinear {
            kind: LoweredLinearKind::Array,
            type_id: array_id,
            elements: vec![a, b],
        },
    };

    let rendered =
        render_core_instruction(&package_identity, &table, &routine, &instruction).expect("array");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r16__l2__arr = [l__pkg__entry__app__r16__l0__a.clone(), l__pkg__entry__app__r16__l1__b.clone()];"
    );
}

#[test]
fn aggregate_and_container_rendering_emits_vector_and_sequence_runtime_constructors() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let vec_id = table.intern(fol_lower::LoweredType::Vector {
        element_type: int_id,
    });
    let seq_id = table.intern(fol_lower::LoweredType::Sequence {
        element_type: int_id,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(17), "main", LoweredBlockId(0));
    let a = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("a".to_string()),
    });
    let b = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("b".to_string()),
    });
    let vec_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(vec_id),
        name: Some("vec".to_string()),
    });
    let seq_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(seq_id),
        name: Some("seq".to_string()),
    });
    let vec_instr = LoweredInstr {
        id: LoweredInstrId(41),
        result: Some(vec_result),
        kind: LoweredInstrKind::ConstructLinear {
            kind: LoweredLinearKind::Vector,
            type_id: vec_id,
            elements: vec![a, b],
        },
    };
    let seq_instr = LoweredInstr {
        id: LoweredInstrId(42),
        result: Some(seq_result),
        kind: LoweredInstrKind::ConstructLinear {
            kind: LoweredLinearKind::Sequence,
            type_id: seq_id,
            elements: vec![a, b],
        },
    };

    let vec_rendered =
        render_core_instruction(&package_identity, &table, &routine, &vec_instr).expect("vector");
    let seq_rendered =
        render_core_instruction(&package_identity, &table, &routine, &seq_instr).expect("sequence");

    assert_eq!(
        vec_rendered,
        "l__pkg__entry__app__r17__l2__vec = rt_model::FolVec::from_items(vec![l__pkg__entry__app__r17__l0__a.clone(), l__pkg__entry__app__r17__l1__b.clone()]);"
    );
    assert_eq!(
        seq_rendered,
        "l__pkg__entry__app__r17__l3__seq = rt_model::FolSeq::from_items(vec![l__pkg__entry__app__r17__l0__a.clone(), l__pkg__entry__app__r17__l1__b.clone()]);"
    );
}

#[test]
fn aggregate_and_container_rendering_emits_set_and_map_runtime_constructors() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let set_id = table.intern(fol_lower::LoweredType::Set {
        member_types: vec![int_id],
    });
    let map_id = table.intern(fol_lower::LoweredType::Map {
        key_type: int_id,
        value_type: int_id,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(18), "main", LoweredBlockId(0));
    let a = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("a".to_string()),
    });
    let b = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("b".to_string()),
    });
    let set_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(set_id),
        name: Some("set".to_string()),
    });
    let map_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(map_id),
        name: Some("map".to_string()),
    });
    let set_instr = LoweredInstr {
        id: LoweredInstrId(43),
        result: Some(set_result),
        kind: LoweredInstrKind::ConstructSet {
            type_id: set_id,
            members: vec![a, b],
        },
    };
    let map_instr = LoweredInstr {
        id: LoweredInstrId(44),
        result: Some(map_result),
        kind: LoweredInstrKind::ConstructMap {
            type_id: map_id,
            entries: vec![(a, b), (b, a)],
        },
    };

    let set_rendered =
        render_core_instruction(&package_identity, &table, &routine, &set_instr).expect("set");
    let map_rendered =
        render_core_instruction(&package_identity, &table, &routine, &map_instr).expect("map");

    assert_eq!(
        set_rendered,
        "l__pkg__entry__app__r18__l2__set = rt_model::FolSet::from_items(vec![l__pkg__entry__app__r18__l0__a.clone(), l__pkg__entry__app__r18__l1__b.clone()]);"
    );
    assert_eq!(
        map_rendered,
        "l__pkg__entry__app__r18__l3__map = rt_model::FolMap::from_pairs(vec![(l__pkg__entry__app__r18__l0__a.clone(), l__pkg__entry__app__r18__l1__b.clone()), (l__pkg__entry__app__r18__l1__b.clone(), l__pkg__entry__app__r18__l0__a.clone())]);"
    );
}

#[test]
fn aggregate_constructors_move_unique_elements_and_values() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let pointer_id = table.intern(LoweredType::Pointer {
        target: int_id,
        shared: false,
        weak: false,
        sync: false,
    });
    let array_id = table.intern(LoweredType::Array {
        element_type: pointer_id,
        size: Some(2),
    });
    let map_id = table.intern(LoweredType::Map {
        key_type: int_id,
        value_type: pointer_id,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(181), "main", LoweredBlockId(0));
    let first = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(pointer_id),
        name: Some("first".to_string()),
    });
    let second = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(pointer_id),
        name: Some("second".to_string()),
    });
    let key = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(int_id),
        name: Some("key".to_string()),
    });
    let array = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(array_id),
        name: Some("array".to_string()),
    });
    let map = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(map_id),
        name: Some("map".to_string()),
    });
    let array_instr = LoweredInstr {
        id: LoweredInstrId(441),
        result: Some(array),
        kind: LoweredInstrKind::ConstructLinear {
            kind: LoweredLinearKind::Array,
            type_id: array_id,
            elements: vec![first, second],
        },
    };
    let map_instr = LoweredInstr {
        id: LoweredInstrId(442),
        result: Some(map),
        kind: LoweredInstrKind::ConstructMap {
            type_id: map_id,
            entries: vec![(key, first)],
        },
    };

    let array_rendered = render_core_instruction(&package_identity, &table, &routine, &array_instr)
        .expect("unique array");
    let map_rendered = render_core_instruction(&package_identity, &table, &routine, &map_instr)
        .expect("unique map value");

    assert_eq!(
        array_rendered,
        "l__pkg__entry__app__r181__l3__array = [l__pkg__entry__app__r181__l0__first, l__pkg__entry__app__r181__l1__second];"
    );
    assert_eq!(
        map_rendered,
        "l__pkg__entry__app__r181__l4__map = rt_model::FolMap::from_pairs(vec![(l__pkg__entry__app__r181__l2__key.clone(), l__pkg__entry__app__r181__l0__first)]);"
    );
}

#[test]
fn aggregate_and_container_rendering_emits_runtime_index_helpers() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let array_id = table.intern(fol_lower::LoweredType::Array {
        element_type: int_id,
        size: Some(2),
    });
    let vec_id = table.intern(fol_lower::LoweredType::Vector {
        element_type: int_id,
    });
    let seq_id = table.intern(fol_lower::LoweredType::Sequence {
        element_type: int_id,
    });
    let map_id = table.intern(fol_lower::LoweredType::Map {
        key_type: int_id,
        value_type: int_id,
    });
    let unique_pointer = table.intern(fol_lower::LoweredType::Pointer {
        target: int_id,
        shared: false,
        weak: false,
        sync: false,
    });
    let move_only_record = table.intern(fol_lower::LoweredType::Record {
        fields: BTreeMap::from([("pointer".to_string(), unique_pointer)]),
        finalized: false,
    });
    let move_only_array = table.intern(fol_lower::LoweredType::Array {
        element_type: move_only_record,
        size: Some(1),
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(19), "main", LoweredBlockId(0));
    let array = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(array_id),
        name: Some("arr".to_string()),
    });
    let vector = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(vec_id),
        name: Some("vec".to_string()),
    });
    let sequence = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(seq_id),
        name: Some("seq".to_string()),
    });
    let map = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(map_id),
        name: Some("map".to_string()),
    });
    let index = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(int_id),
        name: Some("index".to_string()),
    });
    let arr_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(5),
        type_id: Some(int_id),
        name: Some("a".to_string()),
    });
    let vec_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(6),
        type_id: Some(int_id),
        name: Some("b".to_string()),
    });
    let seq_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(7),
        type_id: Some(int_id),
        name: Some("c".to_string()),
    });
    let map_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(8),
        type_id: Some(int_id),
        name: Some("d".to_string()),
    });
    let move_only_values = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(9),
        type_id: Some(move_only_array),
        name: Some("owners".to_string()),
    });
    let move_only_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(10),
        type_id: Some(move_only_record),
        name: Some("owner".to_string()),
    });

    let rendered = [
        LoweredInstr {
            id: LoweredInstrId(45),
            result: Some(arr_result),
            kind: LoweredInstrKind::IndexAccess {
                container: array,
                index,
            },
        },
        LoweredInstr {
            id: LoweredInstrId(46),
            result: Some(vec_result),
            kind: LoweredInstrKind::IndexAccess {
                container: vector,
                index,
            },
        },
        LoweredInstr {
            id: LoweredInstrId(47),
            result: Some(seq_result),
            kind: LoweredInstrKind::IndexAccess {
                container: sequence,
                index,
            },
        },
        LoweredInstr {
            id: LoweredInstrId(48),
            result: Some(map_result),
            kind: LoweredInstrKind::IndexAccess {
                container: map,
                index,
            },
        },
    ]
    .iter()
    .map(|instruction| render_core_instruction(&package_identity, &table, &routine, instruction))
    .collect::<Result<Vec<_>, _>>()
    .expect("index renders");

    assert_eq!(
        rendered[0],
        "l__pkg__entry__app__r19__l5__a = rt::index_array(&l__pkg__entry__app__r19__l0__arr, l__pkg__entry__app__r19__l4__index.clone()).unwrap().clone();"
    );
    assert_eq!(
        rendered[1],
        "l__pkg__entry__app__r19__l6__b = rt::index_vec(&l__pkg__entry__app__r19__l1__vec, l__pkg__entry__app__r19__l4__index.clone()).unwrap().clone();"
    );
    assert_eq!(
        rendered[2],
        "l__pkg__entry__app__r19__l7__c = rt::index_seq(&l__pkg__entry__app__r19__l2__seq, l__pkg__entry__app__r19__l4__index.clone()).unwrap().clone();"
    );
    assert_eq!(
        rendered[3],
        "l__pkg__entry__app__r19__l8__d = rt::lookup_map(&l__pkg__entry__app__r19__l3__map, &l__pkg__entry__app__r19__l4__index).unwrap().clone();"
    );

    let error = render_core_instruction(
        &package_identity,
        &table,
        &routine,
        &LoweredInstr {
            id: LoweredInstrId(49),
            result: Some(move_only_result),
            kind: LoweredInstrKind::IndexAccess {
                container: move_only_values,
                index,
            },
        },
    )
    .expect_err("move-only index reads must stop before clone emission");
    assert_eq!(error.kind(), BackendErrorKind::InvalidInput);
    assert!(error.message().contains("move-only index results"));
}

#[test]
fn slice_rendering_rejects_move_only_results_before_clone_emission() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let unique_pointer = table.intern(LoweredType::Pointer {
        target: int_id,
        shared: false,
        weak: false,
        sync: false,
    });
    let scalar_vec = table.intern(LoweredType::Vector {
        element_type: int_id,
    });
    let move_only_vec = table.intern(LoweredType::Vector {
        element_type: unique_pointer,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(20), "main", LoweredBlockId(0));
    let values = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(scalar_vec),
        name: Some("values".to_string()),
    });
    let start = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("start".to_string()),
    });
    let end = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(int_id),
        name: Some("end".to_string()),
    });
    let scalar_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(scalar_vec),
        name: Some("slice".to_string()),
    });
    let owners = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(move_only_vec),
        name: Some("owners".to_string()),
    });
    let move_only_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(5),
        type_id: Some(move_only_vec),
        name: Some("owner_slice".to_string()),
    });

    let rendered = render_core_instruction(
        &package_identity,
        &table,
        &routine,
        &LoweredInstr {
            id: LoweredInstrId(50),
            result: Some(scalar_result),
            kind: LoweredInstrKind::SliceAccess {
                container: values,
                start,
                end,
            },
        },
    )
    .expect("copy-safe slice renders");
    assert_eq!(
        rendered,
        "l__pkg__entry__app__r20__l3__slice = rt::slice_vec(&l__pkg__entry__app__r20__l0__values, l__pkg__entry__app__r20__l1__start.clone(), l__pkg__entry__app__r20__l2__end.clone()).unwrap();"
    );

    let error = render_core_instruction(
        &package_identity,
        &table,
        &routine,
        &LoweredInstr {
            id: LoweredInstrId(51),
            result: Some(move_only_result),
            kind: LoweredInstrKind::SliceAccess {
                container: owners,
                start,
                end,
            },
        },
    )
    .expect_err("move-only slices must stop before clone-based runtime emission");
    assert_eq!(error.kind(), BackendErrorKind::InvalidInput);
    assert_eq!(
        error.message(),
        "move-only slice results are not supported in V3; slice emission would clone unique ownership"
    );
}

#[test]
fn aggregate_and_container_rendering_emits_record_and_entry_constructors() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let record_type = table.intern(LoweredType::Record {
        fields: BTreeMap::from([("count".to_string(), int_id)]),
        finalized: false,
    });
    let entry_type = table.intern(LoweredType::Entry {
        variants: BTreeMap::from([("Ok".to_string(), Some(int_id))]),
    });

    let mut package = LoweredPackage::new(fol_lower::LoweredPackageId(0), package_identity.clone());
    package.source_units.push(fol_lower::LoweredSourceUnit {
        source_unit_id: SourceUnitId(0),
        path: "app/main.fol".to_string(),
        package: "app".to_string(),
        namespace: "app".to_string(),
    });
    package.type_decls.insert(
        SymbolId(0),
        LoweredTypeDecl {
            symbol_id: SymbolId(0),
            source_unit_id: SourceUnitId(0),
            name: "Counter".to_string(),
            runtime_type: record_type,
            kind: LoweredTypeDeclKind::Record {
                fields: vec![LoweredFieldLayout {
                    name: "count".to_string(),
                    type_id: int_id,
                }],
            },
        },
    );
    package.type_decls.insert(
        SymbolId(1),
        LoweredTypeDecl {
            symbol_id: SymbolId(1),
            source_unit_id: SourceUnitId(0),
            name: "Status".to_string(),
            runtime_type: entry_type,
            kind: LoweredTypeDeclKind::Entry {
                variants: vec![LoweredVariantLayout {
                    name: "Ok".to_string(),
                    payload_type: Some(int_id),
                }],
            },
        },
    );
    let workspace = LoweredWorkspace::new(
        package_identity.clone(),
        BTreeMap::from([(package_identity.clone(), package)]),
        Vec::new(),
        table.clone(),
        LoweredSourceMap::new(),
        LoweredRecoverableAbi::v1(bool_id),
    );

    let mut routine = LoweredRoutine::new(LoweredRoutineId(30), "main", LoweredBlockId(0));
    let value = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let record_out = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(record_type),
        name: Some("record".to_string()),
    });
    let entry_out = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(entry_type),
        name: Some("entry".to_string()),
    });

    let record_rendered = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &LoweredInstr {
            id: LoweredInstrId(70),
            result: Some(record_out),
            kind: LoweredInstrKind::ConstructRecord {
                type_id: record_type,
                fields: vec![("count".to_string(), value)],
            },
        },
    )
    .expect("record constructor");
    let entry_rendered = render_core_instruction_in_workspace(
        Some(&workspace),
        &package_identity,
        &table,
        &routine,
        &LoweredInstr {
            id: LoweredInstrId(71),
            result: Some(entry_out),
            kind: LoweredInstrKind::ConstructEntry {
                type_id: entry_type,
                variant: "Ok".to_string(),
                payload: Some(value),
            },
        },
    )
    .expect("entry constructor");

    assert_eq!(
        record_rendered,
        "l__pkg__entry__app__r30__l1__record = crate::packages::pkg__entry__app::root::ty__pkg__entry__app__t2__counter { count: l__pkg__entry__app__r30__l0__value.clone() };"
    );
    assert_eq!(
        entry_rendered,
        "l__pkg__entry__app__r30__l2__entry = crate::packages::pkg__entry__app::root::ty__pkg__entry__app__t3__status::Ok(l__pkg__entry__app__r30__l0__value.clone());"
    );
}

#[test]
fn aggregate_and_container_snapshot_stays_stable() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let array_id = table.intern(fol_lower::LoweredType::Array {
        element_type: int_id,
        size: Some(2),
    });
    let vec_id = table.intern(fol_lower::LoweredType::Vector {
        element_type: int_id,
    });
    let seq_id = table.intern(fol_lower::LoweredType::Sequence {
        element_type: int_id,
    });
    let set_id = table.intern(fol_lower::LoweredType::Set {
        member_types: vec![int_id],
    });
    let map_id = table.intern(fol_lower::LoweredType::Map {
        key_type: int_id,
        value_type: int_id,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(20), "main", LoweredBlockId(0));
    let a = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("a".to_string()),
    });
    let b = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("b".to_string()),
    });
    let arr = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(array_id),
        name: Some("arr".to_string()),
    });
    let vec = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(vec_id),
        name: Some("vec".to_string()),
    });
    let seq = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(seq_id),
        name: Some("seq".to_string()),
    });
    let set = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(5),
        type_id: Some(set_id),
        name: Some("set".to_string()),
    });
    let map = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(6),
        type_id: Some(map_id),
        name: Some("map".to_string()),
    });
    let out = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(7),
        type_id: Some(int_id),
        name: Some("out".to_string()),
    });
    let set_out = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(8),
        type_id: Some(int_id),
        name: Some("set_out".to_string()),
    });

    let rendered = [
        LoweredInstr {
            id: LoweredInstrId(49),
            result: Some(arr),
            kind: LoweredInstrKind::ConstructLinear {
                kind: LoweredLinearKind::Array,
                type_id: array_id,
                elements: vec![a, b],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(50),
            result: Some(vec),
            kind: LoweredInstrKind::ConstructLinear {
                kind: LoweredLinearKind::Vector,
                type_id: vec_id,
                elements: vec![a, b],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(51),
            result: Some(seq),
            kind: LoweredInstrKind::ConstructLinear {
                kind: LoweredLinearKind::Sequence,
                type_id: seq_id,
                elements: vec![a, b],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(52),
            result: Some(set),
            kind: LoweredInstrKind::ConstructSet {
                type_id: set_id,
                members: vec![a, b],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(53),
            result: Some(map),
            kind: LoweredInstrKind::ConstructMap {
                type_id: map_id,
                entries: vec![(a, b)],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(54),
            result: Some(out),
            kind: LoweredInstrKind::IndexAccess {
                container: vec,
                index: a,
            },
        },
        LoweredInstr {
            id: LoweredInstrId(55),
            result: Some(set_out),
            kind: LoweredInstrKind::IndexAccess {
                container: set,
                index: a,
            },
        },
    ]
    .iter()
    .map(|instruction| render_core_instruction(&package_identity, &table, &routine, instruction))
    .collect::<Result<Vec<_>, _>>()
    .expect("container snapshot renders")
    .join("\n");

    assert_eq!(
        rendered,
        concat!(
            "l__pkg__entry__app__r20__l2__arr = [l__pkg__entry__app__r20__l0__a.clone(), l__pkg__entry__app__r20__l1__b.clone()];\n",
            "l__pkg__entry__app__r20__l3__vec = rt_model::FolVec::from_items(vec![l__pkg__entry__app__r20__l0__a.clone(), l__pkg__entry__app__r20__l1__b.clone()]);\n",
            "l__pkg__entry__app__r20__l4__seq = rt_model::FolSeq::from_items(vec![l__pkg__entry__app__r20__l0__a.clone(), l__pkg__entry__app__r20__l1__b.clone()]);\n",
            "l__pkg__entry__app__r20__l5__set = rt_model::FolSet::from_items(vec![l__pkg__entry__app__r20__l0__a.clone(), l__pkg__entry__app__r20__l1__b.clone()]);\n",
            "l__pkg__entry__app__r20__l6__map = rt_model::FolMap::from_pairs(vec![(l__pkg__entry__app__r20__l0__a.clone(), l__pkg__entry__app__r20__l1__b.clone())]);\n",
            "l__pkg__entry__app__r20__l7__out = rt::index_vec(&l__pkg__entry__app__r20__l3__vec, l__pkg__entry__app__r20__l0__a.clone()).unwrap().clone();\n",
            "l__pkg__entry__app__r20__l8__set_out = rt::index_set(&l__pkg__entry__app__r20__l5__set, l__pkg__entry__app__r20__l0__a.clone()).unwrap().clone();"
        )
    );
}

#[test]
fn cast_instruction_rendering_emits_native_rust_cast_expression() {
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut routine = LoweredRoutine::new(LoweredRoutineId(21), "main", LoweredBlockId(0));
    let local_id = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });

    let cast_instr = LoweredInstr {
        id: LoweredInstrId(62),
        result: Some(local_id),
        kind: LoweredInstrKind::Cast {
            operand: local_id,
            target_type: int_id,
        },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &cast_instr)
        .expect("cast should render");
    assert!(rendered.contains("l__pkg__entry__app__r21__l0__value"));
    assert!(rendered.contains("as"));
}

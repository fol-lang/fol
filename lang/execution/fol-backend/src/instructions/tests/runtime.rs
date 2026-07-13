use super::super::render_core_instruction;
use crate::testing::package_identity;
use fol_intrinsics::intrinsic_by_canonical_name;
use fol_lower::{
    LoweredBlockId, LoweredBuiltinType, LoweredInstr, LoweredInstrId, LoweredInstrKind,
    LoweredLocal, LoweredLocalId, LoweredRoutine, LoweredRoutineId, LoweredType,
    LoweredTypeTable,
};
use fol_resolver::PackageSourceKind;

#[test]
fn runtime_shaped_instruction_rendering_emits_length_via_runtime_prelude() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(7), "main", LoweredBlockId(0));
    let source = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("items".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("count".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(20),
        result: Some(result),
        kind: LoweredInstrKind::LengthOf { operand: source },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("length");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r7__l1__count = rt::len(&l__pkg__entry__app__r7__l0__items);"
    );
}

#[test]
fn borrowed_value_reads_clone_through_the_reference() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let borrowed_id = table.intern(LoweredType::Borrowed {
        inner: int_id,
        mutable: false,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(8), "read", LoweredBlockId(0));
    let borrow = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(borrowed_id),
        name: Some("borrow".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(21),
        result: Some(result),
        kind: LoweredInstrKind::ReadBorrow { borrow },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("borrow read");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r8__l1__value = (*l__pkg__entry__app__r8__l0__borrow).clone();"
    );
}

#[test]
fn runtime_shaped_instruction_rendering_emits_echo_via_runtime_hook() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(8), "main", LoweredBlockId(0));
    let value = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("shown".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(21),
        result: Some(result),
        kind: LoweredInstrKind::RuntimeHook {
            intrinsic: intrinsic_by_canonical_name("echo").expect("echo").id,
            args: vec![value],
        },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("echo");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r8__l1__shown = rt::echo(l__pkg__entry__app__r8__l0__value.clone());"
    );
}

#[test]
fn runtime_shaped_echo_moves_a_unique_value() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let pointer_id = table.intern(fol_lower::LoweredType::Pointer {
        target: int_id,
        shared: false,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(81), "main", LoweredBlockId(0));
    let pointer = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(pointer_id),
        name: Some("pointer".to_string()),
    });
    let shown = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(pointer_id),
        name: Some("shown".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(211),
        result: Some(shown),
        kind: LoweredInstrKind::RuntimeHook {
            intrinsic: intrinsic_by_canonical_name("echo").expect("echo").id,
            args: vec![pointer],
        },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("unique echo");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r81__l1__shown = rt::echo(l__pkg__entry__app__r81__l0__pointer);"
    );
}

#[test]
fn runtime_shaped_instruction_rendering_emits_check_recoverable_via_runtime_helper() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(9), "main", LoweredBlockId(0));
    let source = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: None,
        name: Some("value".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(bool_id),
        name: Some("failed".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(22),
        result: Some(result),
        kind: LoweredInstrKind::CheckRecoverable { operand: source },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("check");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r9__l1__failed = rt::check_recoverable(&l__pkg__entry__app__r9__l0__value);"
    );
}

#[test]
fn runtime_shaped_instruction_rendering_emits_unwrap_recoverable_success_lane() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let mut routine = LoweredRoutine::new(LoweredRoutineId(10), "main", LoweredBlockId(0));
    let source = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: None,
        name: Some("value".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(int_id),
        name: Some("unwrapped".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(23),
        result: Some(result),
        kind: LoweredInstrKind::UnwrapRecoverable { operand: source },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("unwrap");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r10__l1__unwrapped = l__pkg__entry__app__r10__l0__value.into_value().expect(\"unwrap of recoverable value failed: result contains an error\");"
    );
}

#[test]
fn runtime_shaped_instruction_rendering_emits_recoverable_error_extraction() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let table = LoweredTypeTable::new();
    let mut routine = LoweredRoutine::new(LoweredRoutineId(11), "main", LoweredBlockId(0));
    let source = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: None,
        name: Some("value".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: None,
        name: Some("error".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(24),
        result: Some(result),
        kind: LoweredInstrKind::ExtractRecoverableError { operand: source },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("extract");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r11__l1__error = l__pkg__entry__app__r11__l0__value.into_error().expect(\"extract of recoverable error failed: result contains a value\");"
    );
}

#[test]
fn runtime_shaped_instruction_rendering_emits_optional_shell_construction() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let optional_id = table.intern(fol_lower::LoweredType::Optional { inner: int_id });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(12), "main", LoweredBlockId(0));
    let payload = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let some_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(optional_id),
        name: Some("maybe".to_string()),
    });
    let nil_result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(optional_id),
        name: Some("empty".to_string()),
    });
    let some_instr = LoweredInstr {
        id: LoweredInstrId(25),
        result: Some(some_result),
        kind: LoweredInstrKind::ConstructOptional {
            type_id: optional_id,
            value: Some(payload),
        },
    };
    let nil_instr = LoweredInstr {
        id: LoweredInstrId(26),
        result: Some(nil_result),
        kind: LoweredInstrKind::ConstructOptional {
            type_id: optional_id,
            value: None,
        },
    };

    let some_rendered =
        render_core_instruction(&package_identity, &table, &routine, &some_instr)
            .expect("some");
    let nil_rendered =
        render_core_instruction(&package_identity, &table, &routine, &nil_instr).expect("nil");

    assert_eq!(
        some_rendered,
        "l__pkg__entry__app__r12__l1__maybe = rt::FolOption::some(l__pkg__entry__app__r12__l0__value.clone());"
    );
    assert_eq!(
        nil_rendered,
        "l__pkg__entry__app__r12__l2__empty = rt::FolOption::nil();"
    );
}

#[test]
fn runtime_shaped_instruction_rendering_emits_error_shell_construction() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let error_id = table.intern(fol_lower::LoweredType::Error {
        inner: Some(int_id),
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(13), "main", LoweredBlockId(0));
    let payload = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(error_id),
        name: Some("error".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(27),
        result: Some(result),
        kind: LoweredInstrKind::ConstructError {
            type_id: error_id,
            value: Some(payload),
        },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("error shell");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r13__l1__error = rt::FolError::new(l__pkg__entry__app__r13__l0__value.clone());"
    );
}

#[test]
fn runtime_shaped_error_shell_moves_a_unique_payload() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let pointer_id = table.intern(fol_lower::LoweredType::Pointer {
        target: int_id,
        shared: false,
    });
    let error_id = table.intern(fol_lower::LoweredType::Error {
        inner: Some(pointer_id),
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(131), "main", LoweredBlockId(0));
    let payload = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(pointer_id),
        name: Some("pointer".to_string()),
    });
    let result = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(error_id),
        name: Some("error".to_string()),
    });
    let instruction = LoweredInstr {
        id: LoweredInstrId(271),
        result: Some(result),
        kind: LoweredInstrKind::ConstructError {
            type_id: error_id,
            value: Some(payload),
        },
    };

    let rendered = render_core_instruction(&package_identity, &table, &routine, &instruction)
        .expect("unique error shell");

    assert_eq!(
        rendered,
        "l__pkg__entry__app__r131__l1__error = rt::FolError::new(l__pkg__entry__app__r131__l0__pointer);"
    );
}

#[test]
fn runtime_shaped_instruction_rendering_emits_shell_unwraps_for_optional_and_error_values() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let optional_id = table.intern(fol_lower::LoweredType::Optional { inner: int_id });
    let error_id = table.intern(fol_lower::LoweredType::Error {
        inner: Some(int_id),
    });
    let pointer_id = table.intern(fol_lower::LoweredType::Pointer {
        target: int_id,
        shared: false,
    });
    let unique_optional_id = table.intern(fol_lower::LoweredType::Optional {
        inner: pointer_id,
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(14), "main", LoweredBlockId(0));
    let maybe = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(optional_id),
        name: Some("maybe".to_string()),
    });
    let err = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: Some(error_id),
        name: Some("err".to_string()),
    });
    let a = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(int_id),
        name: Some("a".to_string()),
    });
    let b = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(int_id),
        name: Some("b".to_string()),
    });
    let unique_maybe = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(unique_optional_id),
        name: Some("unique_maybe".to_string()),
    });
    let pointer = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(5),
        type_id: Some(pointer_id),
        name: Some("pointer".to_string()),
    });
    let optional_instr = LoweredInstr {
        id: LoweredInstrId(28),
        result: Some(a),
        kind: LoweredInstrKind::UnwrapShell { operand: maybe },
    };
    let error_instr = LoweredInstr {
        id: LoweredInstrId(29),
        result: Some(b),
        kind: LoweredInstrKind::UnwrapShell { operand: err },
    };
    let unique_optional_instr = LoweredInstr {
        id: LoweredInstrId(30),
        result: Some(pointer),
        kind: LoweredInstrKind::UnwrapShell {
            operand: unique_maybe,
        },
    };

    let optional_rendered =
        render_core_instruction(&package_identity, &table, &routine, &optional_instr)
            .expect("optional unwrap");
    let error_rendered =
        render_core_instruction(&package_identity, &table, &routine, &error_instr)
            .expect("error unwrap");
    let unique_optional_rendered = render_core_instruction(
        &package_identity,
        &table,
        &routine,
        &unique_optional_instr,
    )
    .expect("unique optional unwrap");

    assert_eq!(
        optional_rendered,
        "l__pkg__entry__app__r14__l2__a = rt::unwrap_optional_shell(l__pkg__entry__app__r14__l0__maybe.clone()).unwrap();"
    );
    assert_eq!(
        error_rendered,
        "l__pkg__entry__app__r14__l3__b = rt::unwrap_error_shell(l__pkg__entry__app__r14__l1__err.clone());"
    );
    assert_eq!(
        unique_optional_rendered,
        "l__pkg__entry__app__r14__l5__pointer = rt::unwrap_optional_shell(l__pkg__entry__app__r14__l4__unique_maybe).unwrap();"
    );
}

#[test]
fn runtime_shaped_instruction_snapshot_stays_stable() {
    let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
    let mut table = LoweredTypeTable::new();
    let int_id = table.intern_builtin(LoweredBuiltinType::Int);
    let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
    let optional_id = table.intern(fol_lower::LoweredType::Optional { inner: int_id });
    let error_id = table.intern(fol_lower::LoweredType::Error {
        inner: Some(int_id),
    });
    let mut routine = LoweredRoutine::new(LoweredRoutineId(15), "main", LoweredBlockId(0));
    let value = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(0),
        type_id: Some(int_id),
        name: Some("value".to_string()),
    });
    let rec = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(1),
        type_id: None,
        name: Some("recover".to_string()),
    });
    let maybe = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(2),
        type_id: Some(optional_id),
        name: Some("maybe".to_string()),
    });
    let err = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(3),
        type_id: Some(error_id),
        name: Some("err".to_string()),
    });
    let count = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(4),
        type_id: Some(int_id),
        name: Some("count".to_string()),
    });
    let shown = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(5),
        type_id: Some(int_id),
        name: Some("shown".to_string()),
    });
    let failed = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(6),
        type_id: Some(bool_id),
        name: Some("failed".to_string()),
    });
    let ok = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(7),
        type_id: Some(int_id),
        name: Some("ok".to_string()),
    });
    let bad = routine.locals.push(LoweredLocal {
        id: LoweredLocalId(8),
        type_id: Some(int_id),
        name: Some("bad".to_string()),
    });

    let rendered = [
        LoweredInstr {
            id: LoweredInstrId(30),
            result: Some(count),
            kind: LoweredInstrKind::LengthOf { operand: maybe },
        },
        LoweredInstr {
            id: LoweredInstrId(31),
            result: Some(shown),
            kind: LoweredInstrKind::RuntimeHook {
                intrinsic: intrinsic_by_canonical_name("echo").expect("echo").id,
                args: vec![value],
            },
        },
        LoweredInstr {
            id: LoweredInstrId(32),
            result: Some(failed),
            kind: LoweredInstrKind::CheckRecoverable { operand: rec },
        },
        LoweredInstr {
            id: LoweredInstrId(33),
            result: Some(ok),
            kind: LoweredInstrKind::UnwrapRecoverable { operand: rec },
        },
        LoweredInstr {
            id: LoweredInstrId(34),
            result: Some(bad),
            kind: LoweredInstrKind::ExtractRecoverableError { operand: rec },
        },
        LoweredInstr {
            id: LoweredInstrId(35),
            result: Some(maybe),
            kind: LoweredInstrKind::ConstructOptional {
                type_id: optional_id,
                value: Some(value),
            },
        },
        LoweredInstr {
            id: LoweredInstrId(36),
            result: Some(err),
            kind: LoweredInstrKind::ConstructError {
                type_id: error_id,
                value: Some(value),
            },
        },
        LoweredInstr {
            id: LoweredInstrId(37),
            result: Some(ok),
            kind: LoweredInstrKind::UnwrapShell { operand: maybe },
        },
    ]
    .iter()
    .map(|instruction| {
        render_core_instruction(&package_identity, &table, &routine, instruction)
    })
    .collect::<Result<Vec<_>, _>>()
    .expect("runtime snapshot should render")
    .join("\n");

    assert_eq!(
        rendered,
        concat!(
            "l__pkg__entry__app__r15__l4__count = rt::len(&l__pkg__entry__app__r15__l2__maybe);\n",
            "l__pkg__entry__app__r15__l5__shown = rt::echo(l__pkg__entry__app__r15__l0__value.clone());\n",
            "l__pkg__entry__app__r15__l6__failed = rt::check_recoverable(&l__pkg__entry__app__r15__l1__recover);\n",
            "l__pkg__entry__app__r15__l7__ok = l__pkg__entry__app__r15__l1__recover.into_value().expect(\"unwrap of recoverable value failed: result contains an error\");\n",
            "l__pkg__entry__app__r15__l8__bad = l__pkg__entry__app__r15__l1__recover.into_error().expect(\"extract of recoverable error failed: result contains a value\");\n",
            "l__pkg__entry__app__r15__l2__maybe = rt::FolOption::some(l__pkg__entry__app__r15__l0__value.clone());\n",
            "l__pkg__entry__app__r15__l3__err = rt::FolError::new(l__pkg__entry__app__r15__l0__value.clone());\n",
            "l__pkg__entry__app__r15__l7__ok = rt::unwrap_optional_shell(l__pkg__entry__app__r15__l2__maybe.clone()).unwrap();"
        )
    );
}

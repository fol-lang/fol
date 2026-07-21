use super::lower_fixture_workspace;
use crate::{control::LoweredBinaryOp, control::LoweredUnaryOp, LoweredInstrKind};

#[test]
fn arithmetic_binary_operators_lower_to_binary_op_instructions() {
    let workspace = lower_fixture_workspace("fun[] main(): int = {\n    return 1 + 2;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_add = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Add,
                ..
            }
        )
    });
    assert!(
        has_add,
        "lowered IR should contain a BinaryOp::Add instruction"
    );
}

#[test]
fn comparison_binary_operators_lower_to_binary_op_instructions() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: int, b: int): bol = {\n    return .eq(a, b);\n};\n");
    assert_eq!(workspace.package_count(), 1);

    let workspace2 =
        lower_fixture_workspace("fun[] main(a: int, b: int): bol = {\n    return a == b;\n};\n");
    let routine = workspace2
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_eq = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Eq,
                ..
            }
        )
    });
    assert!(
        has_eq,
        "lowered IR should contain a BinaryOp::Eq instruction"
    );
}

#[test]
fn logical_binary_operators_lower_to_binary_op_instructions() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: bol, b: bol): bol = {\n    return a and b;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_and = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::And,
                ..
            }
        )
    });
    assert!(
        has_and,
        "lowered IR should contain a BinaryOp::And instruction"
    );
}

#[test]
fn negation_unary_operator_lowers_to_unary_op_instruction() {
    let workspace = lower_fixture_workspace("fun[] main(): int = {\n    return -1;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_neg = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::UnaryOp {
                op: LoweredUnaryOp::Neg,
                ..
            }
        )
    });
    assert!(
        has_neg,
        "lowered IR should contain a UnaryOp::Neg instruction"
    );
}

#[test]
fn boolean_not_unary_operator_lowers_to_unary_op_instruction() {
    let workspace = lower_fixture_workspace("fun[] main(): bol = {\n    return .not(true);\n};\n");
    assert_eq!(workspace.package_count(), 1);
}

#[test]
fn ref_deref_unary_operators_reject_at_typecheck() {
    let workspace = lower_fixture_workspace("fun[] main(): int = {\n    return 42;\n};\n");
    assert_eq!(workspace.package_count(), 1);
}

#[test]
fn move_only_unique_pointer_deref_lowers_as_consuming() {
    let workspace = lower_fixture_workspace(
        "fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var inner: ptr[int] = [ref]seed;\n\
             var outer: ptr[ptr[int]] = [ref]inner;\n\
             var extracted: ptr[int] = [drf]outer;\n\
             return [drf]extracted;\n\
         };\n",
    );
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine");
    let dereferences = routine
        .instructions
        .iter()
        .filter_map(|instruction| match instruction.kind {
            LoweredInstrKind::DerefPointer { consuming, .. } => Some(consuming),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        dereferences,
        vec![true, false],
        "moving ptr[int] out of ptr[ptr[int]] must consume only the outer pointer"
    );
}

#[test]
fn borrowed_pointer_deref_lowers_as_observation() {
    let workspace = lower_fixture_workspace(
        "fun[] read(pointer[bor]: ptr[int]): int = {\n\
             return [drf]pointer;\n\
         };\n\
         fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var pointer: ptr[int] = [ref]seed;\n\
             return read([bor]pointer);\n\
         };\n",
    );
    let read = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "read")
        .expect("read routine");
    assert!(read.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            LoweredInstrKind::DerefPointer {
                consuming: false,
                ..
            }
        )
    }));
}

#[test]
fn float_arithmetic_operators_lower_correctly() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: flt, b: flt): flt = {\n    return a + b;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_add = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Add,
                ..
            }
        )
    });
    assert!(has_add, "float addition should lower to BinaryOp::Add");
}

#[test]
fn string_concatenation_lowers_to_binary_add() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: str, b: str): str = {\n    return a + b;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_add = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Add,
                ..
            }
        )
    });
    assert!(
        has_add,
        "string concatenation should lower to BinaryOp::Add"
    );
}

#[test]
fn division_modulo_power_operators_lower_correctly() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: int, b: int): int = {\n    return a / b;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_div = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Div,
                ..
            }
        )
    });
    assert!(has_div, "lowered IR should contain BinaryOp::Div");

    let workspace2 =
        lower_fixture_workspace("fun[] main(a: int, b: int): int = {\n    return a % b;\n};\n");
    let routine2 = workspace2
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_mod = routine2.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Mod,
                ..
            }
        )
    });
    assert!(has_mod, "lowered IR should contain BinaryOp::Mod");

    let workspace3 =
        lower_fixture_workspace("fun[] main(a: int, b: int): int = {\n    return a ^ b;\n};\n");
    let routine3 = workspace3
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_pow = routine3.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Pow,
                ..
            }
        )
    });
    assert!(has_pow, "lowered IR should contain BinaryOp::Pow");
}

#[test]
fn ordering_comparison_operators_lower_correctly() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: int, b: int): bol = {\n    return a < b;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_lt = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Lt,
                ..
            }
        )
    });
    assert!(has_lt, "lowered IR should contain BinaryOp::Lt");
}

#[test]
fn or_and_xor_logical_operators_lower_correctly() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: bol, b: bol): bol = {\n    return a or b;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_or = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Or,
                ..
            }
        )
    });
    assert!(has_or, "lowered IR should contain BinaryOp::Or");

    let workspace2 =
        lower_fixture_workspace("fun[] main(a: bol, b: bol): bol = {\n    return a xor b;\n};\n");
    let routine2 = workspace2
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_xor = routine2.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Xor,
                ..
            }
        )
    });
    assert!(has_xor, "lowered IR should contain BinaryOp::Xor");
}

#[test]
fn subtraction_and_multiplication_lower_correctly() {
    let workspace =
        lower_fixture_workspace("fun[] main(a: int, b: int): int = {\n    return a - b * a;\n};\n");
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|r| r.name == "main")
        .expect("should find main routine");
    let has_sub = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Sub,
                ..
            }
        )
    });
    let has_mul = routine.instructions.iter().any(|instr| {
        matches!(
            instr.kind,
            LoweredInstrKind::BinaryOp {
                op: LoweredBinaryOp::Mul,
                ..
            }
        )
    });
    assert!(has_sub, "lowered IR should contain BinaryOp::Sub");
    assert!(has_mul, "lowered IR should contain BinaryOp::Mul");
}

use super::*;

#[test]
fn binary_add_rejects_mismatched_types() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             return 1 + true;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.kind() == TypecheckErrorKind::InvalidInput }),
        "Adding int and bol should be rejected, got: {errors:?}"
    );
}

#[test]
fn binary_add_accepts_matching_int_operands() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             return 1 + 2;\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn binary_add_accepts_matching_float_operands() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): flt = {\n\
             return 1.0 + 2.0;\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn binary_add_accepts_matching_string_operands() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): str = {\n\
             return \"hello\" + \"world\";\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn binary_sub_rejects_string_operands() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): str = {\n\
             return \"a\" - \"b\";\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.kind() == TypecheckErrorKind::InvalidInput }),
        "Subtracting strings should be rejected, got: {errors:?}"
    );
}

#[test]
fn logical_and_rejects_int_operands() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): bol = {\n\
             return 1 and 2;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.kind() == TypecheckErrorKind::InvalidInput }),
        "Logical 'and' on integers should be rejected, got: {errors:?}"
    );
}

#[test]
fn comparison_lt_rejects_bool_operands() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): bol = {\n\
             return true < false;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.kind() == TypecheckErrorKind::InvalidInput }),
        "Ordering comparison on booleans should be rejected, got: {errors:?}"
    );
}

#[test]
fn equality_accepts_matching_int_operands() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): bol = {\n\
             return 1 == 2;\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn negation_rejects_bool_operand() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             return -true;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.kind() == TypecheckErrorKind::InvalidInput }),
        "Negating a boolean should be rejected, got: {errors:?}"
    );
}

#[test]
fn not_rejects_int_operand() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): bol = {\n\
             return .not(1);\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.kind() == TypecheckErrorKind::InvalidInput }),
        "Boolean not on integer should be rejected, got: {errors:?}"
    );
}

#[test]
fn return_type_mismatch_is_rejected() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): str = {\n\
             return 42;\n\
         };\n",
    )]);

    assert!(
        !errors.is_empty(),
        "Returning int from str function should be rejected"
    );
}

#[test]
fn argument_type_mismatch_is_rejected() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] add(a: int, b: int): int = {\n\
             return a + b;\n\
         };\n\
         fun[] main(): int = {\n\
             return add(1, true);\n\
         };\n",
    )]);

    assert!(
        !errors.is_empty(),
        "Passing bool where int is expected should be rejected"
    );
}

#[test]
fn assignment_type_mismatch_is_rejected() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var x: int = true;\n\
             return x;\n\
         };\n",
    )]);

    assert!(
        !errors.is_empty(),
        "Assigning bool to int variable should be rejected"
    );
}

#[test]
fn division_by_zero_type_still_valid() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             return 10 / 0;\n\
         };\n",
    )]);
    assert!(
        !typed.type_table().is_empty(),
        "Division by zero is a runtime concern, not a type error"
    );
}

#[test]
fn ne_operator_accepts_matching_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(a: int, b: int): bol = {\n\
             return a != b;\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn ge_le_operators_accept_ordered_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(a: int, b: int): bol = {\n\
             var ge: bol = a >= b;\n\
             var le: bol = a <= b;\n\
             return ge and le;\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn vec_literal_accepts_homogeneous_int_elements() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): vec[int] = {\n\
             return {1, 2, 3};\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn vec_literal_rejects_heterogeneous_elements() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): vec[int] = {\n\
             return {1, true, 3};\n\
         };\n",
    )]);

    assert!(
        !errors.is_empty(),
        "Mixing int and bool in vec[int] should be rejected"
    );
}

#[test]
fn map_literal_accepts_matching_key_value_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): map[str, int] = {\n\
             return {{\"hello\", 1}, {\"world\", 2}};\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn opt_wrap_and_nil_typecheck_correctly() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): opt[int] = {\n\
             return nil;\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn chained_boolean_logic_typechecks() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(a: bol, b: bol, c: bol): bol = {\n\
             return (a and b) or (b xor c);\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

#[test]
fn nested_arithmetic_typechecks() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(x: int, y: int): int = {\n\
             return (x + y) * (x - y);\n\
         };\n",
    )]);
    assert!(!typed.type_table().is_empty());
}

use super::*;

#[test]
fn check_typing_accepts_errorful_calls_and_returns_bool() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): bol = {\n\
             return check(load());\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Bool))
    );
}

#[test]
fn check_typing_rejects_plain_values() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): bol = {\n\
             return check(1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("check(...) requires a routine call result with '/ ErrorType' in V1")
        }),
        "Expected an invalid check diagnostic, got: {errors:?}"
    );
}

#[test]
fn check_typing_rejects_wrong_arity_through_keyword_intrinsic_diagnostics() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): bol = {\n\
             return check();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("check(...) expects exactly 1 argument(s) but got 0")
        }),
        "Expected registry-backed check arity diagnostic, got: {errors:?}"
    );
}

#[test]
fn check_typing_rejects_err_shell_values_explicitly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "ali Failure: err[str];\nfun[] main(value: Failure): bol = {\n    return check(value);\n};\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("check(...) inspects routine call results with '/ ErrorType', not err[...] shell values in V1")
        }),
        "Expected the err-shell check diagnostic, got: {errors:?}"
    );
}

#[test]
fn pipe_or_typing_accepts_default_value_fallbacks() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int = {\n\
             return load() || 5;\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn pipe_or_typing_rejects_err_shell_values_explicitly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "ali Failure: err[int];\nfun[] main(value: Failure): int = {\n    return value || 5;\n};\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("'||' handles routine call results with '/ ErrorType', not err[...] shell values in V1")
        }),
        "Expected the err-shell pipe-or diagnostic, got: {errors:?}"
    );
}

#[test]
fn pipe_or_typing_rejects_incompatible_fallback_values() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int = {\n\
             return load() || \"fallback\";\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("recoverable-error fallback")
        }),
        "Expected an incompatible fallback diagnostic, got: {errors:?}"
    );
}

#[test]
fn pipe_or_fallback_reinitialization_does_not_erase_the_success_path_move() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
         fun[] load(fail: bol): int / int = {\n\
             when(fail) {\n\
                 case(true) { report 1; }\n\
                 * { return 5; }\n\
             }\n\
         };\n\
         fun[] main(): int = {\n\
             var first: int = 1;\n\
             var second: int = 2;\n\
             var[mut] pointer: ptr[int] = &first;\n\
             consume(pointer);\n\
             var value: int = load(false) || when(true) {\n\
                 case(true) { pointer = &second; 7; }\n\
                 * { pointer = &second; 8; }\n\
             };\n\
             return *pointer + value;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("use of moved heap-owned binding 'pointer'")
        }),
        "the successful call path skips the fallback reinitialization: {errors:#?}"
    );
}

#[test]
fn err_shell_values_remain_storable_passable_and_returnable() {
    let _typed = typecheck_fixture_folder(&[(
        "main.fol",
        "ali Failure: err[str];\n\
         fun[] keep(value: Failure): Failure = {\n\
             return value;\n\
         };\n\
         fun[] main(): Failure = {\n\
             var raised: Failure = \"broken\";\n\
             return keep(raised);\n\
         };\n",
    )]);
}

#[test]
fn err_shell_values_keep_postfix_unwrap_behavior() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "ali Failure: err[str];\n\
         fun[] main(value: Failure): str = {\n\
             return value!;\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
}

#[test]
fn recoverable_calls_reject_inferred_plain_bindings() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int = {\n\
             var captured = load();\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("initializer for 'captured'")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict inferred-binding diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_typed_plain_bindings() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int / str = {\n\
             var captured: int = load();\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("initializer for 'captured'")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict typed-binding diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_direct_plain_returns() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int / str = {\n\
             return load();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("return expression")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict return diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_plain_arguments_even_in_error_aware_routines() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] consume(value: int): int = {\n\
             return value;\n\
         };\n\
         fun[] main(): int / str = {\n\
             return consume(load());\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("call to 'consume'")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict argument diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_field_access_receivers() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ User: rec = { name: str };\n\
         fun[] load(): User / str = {\n\
             report \"bad\";\n\
             return { name = \"ok\" };\n\
         };\n\
         fun[] main(): str = {\n\
             return load().name;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("field access '.name' receiver")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict field-access diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_index_access_receivers() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): seq[int] / str = {\n\
             report \"bad\";\n\
             return { 1, 2 };\n\
         };\n\
         fun[] main(): int = {\n\
             return load()[0];\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("index access receiver")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict index-access diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_method_receivers() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ User: rec = { name: str };\n\
         fun[] (User)label(): str = {\n\
             return \"ok\";\n\
         };\n\
         fun[] load(): User / str = {\n\
             report \"bad\";\n\
             return { name = \"ok\" };\n\
         };\n\
         fun[] main(): str = {\n\
             return load().label();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("method receiver for 'label'")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict method-receiver diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_when_selectors() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): bol / str = {\n\
             report \"bad\";\n\
             return true;\n\
         };\n\
         fun[] main(): int = {\n\
             when(load()) {\n\
                 case(true) { return 1; }\n\
                 * { return 0; }\n\
             }\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("when selector")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict when-selector diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_reject_loop_conditions() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): bol / str = {\n\
             report \"bad\";\n\
             return true;\n\
         };\n\
         fun[] main(): int = {\n\
             loop(load()) {\n\
                 break;\n\
             }\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("loop condition")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected the strict loop-condition diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_do_not_implicitly_convert_into_err_shell_bindings() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): err[int] / str = {\n\
             var captured: err[int] = load();\n\
             return captured;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error.message().contains("cannot turn a '/ ErrorType' routine result into err[...] in V1")
                && error.message().contains("initializer for 'captured'")
        }),
        "Expected the err-shell binding boundary diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_do_not_implicitly_convert_into_err_shell_returns() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): err[int] / str = {\n\
             return load();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error.message().contains("cannot turn a '/ ErrorType' routine result into err[...] in V1")
                && error.message().contains("return cannot turn")
        }),
        "Expected the err-shell return boundary diagnostic, got: {errors:?}"
    );
}

#[test]
fn recoverable_calls_do_not_implicitly_convert_into_err_shell_arguments() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] consume(value: err[int]): int = {\n\
             return 1;\n\
         };\n\
         fun[] main(): int / str = {\n\
             return consume(load());\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error.message().contains("cannot turn a '/ ErrorType' routine result into err[...] in V1")
                && error.message().contains("call to 'consume'")
        }),
        "Expected the err-shell call-argument boundary diagnostic, got: {errors:?}"
    );
}

#[test]
fn when_result_typing_accepts_matching_branch_values() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "var yes: int = 1;\n\
         var no: int = 2;\n\
         fun[] demo(flag: bol): int = {\n\
             when(flag) {\n\
                 case(true) { return yes }\n\
                 * { return no }\n\
             }\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "demo");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn when_result_typing_rejects_branch_type_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var yes: int = 1;\n\
         var no: bol = false;\n\
         fun[] demo(flag: bol): int = {\n\
             when(flag) {\n\
                 case(true) { yes }\n\
                 * { no }\n\
             }\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("when branch expects")
        }),
        "Expected a when-branch mismatch diagnostic, got: {errors:?}"
    );
}

#[test]
fn loop_typing_infers_iteration_binder_types_and_checks_bool_guards() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] demo(items: seq[int], ready: bol, limit: int): int = {\n\
             loop(item in items when ready) {\n\
                 break;\n\
             }\n\
             return limit;\n\
         };\n",
    )]);

    let (_item_id, item) = find_typed_symbol(&typed, "item", SymbolKind::LoopBinder);

    assert_eq!(
        typed
            .type_table()
            .get(item.declared_type.expect("loop binder should infer an element type")),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn loop_typing_rejects_non_boolean_conditions_and_reserved_yields() {
    let condition_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad(limit: int): int = {\n\
             loop(limit) {\n\
                 break;\n\
             }\n\
             return limit;\n\
         };\n",
    )]);

    assert!(
        condition_errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("loop condition expects")
        }),
        "Expected a non-boolean loop condition diagnostic, got: {condition_errors:?}"
    );

    let yield_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad(items: seq[int]): seq[int] = {\n\
             loop(item in items) {\n\
                 yield item;\n\
             }\n\
             return items;\n\
         };\n",
    )]);

    assert!(
        yield_errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("yield expressions are not yet supported")
        }),
        "Expected an explicit yield boundary diagnostic, got: {yield_errors:?}"
    );
}

#[test]
fn control_never_typing_allows_panic_and_skips_unreachable_tails() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] demo(): int = {\n\
             panic \"boom\";\n\
             return \"bad\";\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "demo");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn control_never_typing_treats_report_branches_as_early_exits() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] demo(flag: bol): int / str = {\n\
             when(flag) {\n\
                 case(true) { report \"bad\"; }\n\
                 * { return 1 }\n\
             }\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "demo");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn container_literal_typing_accepts_array_vector_and_sequence_contexts() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] make_arr(): arr[int, 3] = {\n\
             return {1, 2, 3};\n\
         };\n\
         fun[] make_vec(): vec[int] = {\n\
             return {1, 2, 3};\n\
         };\n\
         fun[] make_seq(): seq[int] = {\n\
             return {1, 2, 3};\n\
         };\n",
    )]);

    for (name, expected_label) in [
        ("make_arr", "Array"),
        ("make_vec", "Vector"),
        ("make_seq", "Sequence"),
    ] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        let inferred = typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id));
        assert!(
            matches!(
                inferred,
                Some(CheckedType::Array { element_type, .. })
                    if expected_label == "Array"
                        && typed.type_table().get(*element_type)
                            == Some(&CheckedType::Builtin(BuiltinType::Int))
            ) || matches!(
                inferred,
                Some(CheckedType::Vector { element_type })
                    if expected_label == "Vector"
                        && typed.type_table().get(*element_type)
                            == Some(&CheckedType::Builtin(BuiltinType::Int))
            ) || matches!(
                inferred,
                Some(CheckedType::Sequence { element_type })
                    if expected_label == "Sequence"
                        && typed.type_table().get(*element_type)
                            == Some(&CheckedType::Builtin(BuiltinType::Int))
            ),
            "Expected {name} to keep a {expected_label} container type, got {inferred:?}"
        );
    }
}

#[test]
fn container_literal_typing_rejects_mixed_element_families() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad(): vec[int] = {\n\
             return {1, false};\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("container element expects")
        }),
        "Expected a mixed-container-element diagnostic, got: {errors:?}"
    );
}

#[test]
fn container_literal_typing_accepts_set_and_map_contexts() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] tuple_second(): str = {\n\
             var parts: set[int, str] = {1, \"two\"};\n\
             return parts[1];\n\
         };\n\
         fun[] lookup(): int = {\n\
             var counts: map[str, int] = {{\"US\", 45}, {\"DE\", 82}};\n\
             return counts[\"US\"];\n\
         };\n",
    )]);

    let (_parts_id, parts) = find_typed_symbol(&typed, "parts", SymbolKind::ValueBinding);
    let (_counts_id, counts) = find_typed_symbol(&typed, "counts", SymbolKind::ValueBinding);
    let tuple_second = find_named_routine_syntax_id(&typed, "tuple_second");
    let lookup = find_named_routine_syntax_id(&typed, "lookup");

    assert!(matches!(
        typed.type_table().get(parts.declared_type.expect("set binding should lower")),
        Some(CheckedType::Set { member_types })
            if member_types == &vec![typed.builtin_types().int, typed.builtin_types().str_]
    ));
    assert!(matches!(
        typed.type_table().get(counts.declared_type.expect("map binding should lower")),
        Some(CheckedType::Map { key_type, value_type })
            if *key_type == typed.builtin_types().str_
                && *value_type == typed.builtin_types().int
    ));
    assert_eq!(
        typed
            .typed_node(tuple_second)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
    assert_eq!(
        typed
            .typed_node(lookup)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn container_literal_typing_rejects_bad_map_pairs_and_nonliteral_heterogeneous_set_indexes() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_map(): map[str, int] = {\n\
             return {{1, 45}};\n\
         };\n\
         fun[] bad_set(idx: int): int = {\n\
             var parts: set[int, str] = {1, \"two\"};\n\
             return parts[idx];\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("map key expects")
        }),
        "Expected a map-key compatibility diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("heterogeneous sets is not yet supported")
        }),
        "Expected a heterogeneous-set indexing diagnostic, got: {errors:?}"
    );
}

#[test]
fn record_initializer_typing_accepts_nested_record_construction() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Bonus: rec = {\n\
             hta: int;\n\
             ra: int\n\
         };\n\
         typ Salary: rec = {\n\
             basic: int;\n\
             bonus: Bonus\n\
         };\n\
         typ Employee: rec = {\n\
             name: str;\n\
             salary: Salary\n\
         };\n\
         fun[] build(): Employee = {\n\
             return {\n\
                 name = \"Mark\",\n\
                 salary = {\n\
                     basic = 15000,\n\
                     bonus = { hta = 2100, ra = 5000 },\n\
                 },\n\
             };\n\
         };\n",
    )]);

    let (employee_id, _employee) = find_typed_symbol(&typed, "Employee", SymbolKind::Type);
    let syntax_id = find_named_routine_syntax_id(&typed, "build");

    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: employee_id,
            name: "Employee".to_string(),
            kind: DeclaredTypeKind::Type,
            args: Vec::new(),
        })
    );
}

#[test]
fn record_initializer_typing_accepts_named_binding_and_call_argument_contexts() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ User: rec = {\n\
             name: str;\n\
             count: int\n\
         };\n\
         fun[] count(user: User): int = {\n\
             return user.count;\n\
         };\n\
         fun[] build(): int = {\n\
             var current: User = { name = \"ok\", count = 1 };\n\
             return count({ name = \"next\", count = 2 });\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "build");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn record_initializer_typing_rejects_missing_unknown_and_mismatched_fields() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ User: rec = {\n\
             name: str;\n\
             count: int\n\
         };\n\
         fun[] bad_type(): User = {\n\
             return { name = false, count = 1 };\n\
         };\n\
         fun[] bad_field(): User = {\n\
             return { name = \"ok\" };\n\
         };\n\
         fun[] unknown_field(): User = {\n\
             return { name = \"ok\", count = 1, extra = 3 };\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("record field 'name' expects")
        }),
        "Expected a mismatched record-field diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("missing required fields: count")
        }),
        "Expected a missing-record-field diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("does not define a field named 'extra'")
        }),
        "Expected an unknown-record-field diagnostic, got: {errors:?}"
    );
}

#[test]
fn nested_record_initializer_mismatches_keep_inner_value_locations() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Meta: rec = {\n\
             ok: bol\n\
         };\n\
         typ User: rec = {\n\
             meta: Meta\n\
         };\n\
         fun[] main(): User = {\n\
             return { meta = { ok = 1 } };\n\
         };\n",
    )]);

    let error = errors
        .iter()
        .find(|error| error.message().contains("record field 'ok' expects"))
        .expect("Expected nested record-field mismatch diagnostic");
    let location = error
        .diagnostic_location()
        .expect("Expected nested record-field mismatch location");

    assert_eq!(location.line, 8);
    assert_eq!(location.column, 17);
    assert_eq!(location.length, Some(1));
    assert!(
        location
            .file
            .as_deref()
            .is_some_and(|file| file.ends_with("/main.fol")),
        "Expected nested record mismatch to point at main.fol, got: {location:?}"
    );
}

#[test]
fn imported_nested_record_initializer_mismatches_keep_inner_value_locations() {
    let root = unique_temp_dir("workspace_imported_nested_record_locations");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Meta: rec = {\n",
                    "    ok: bol;\n",
                    "};\n",
                    "typ[exp] User: rec = {\n",
                    "    meta: Meta;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "var user: User = { meta = { ok = 1 } };\n",
                ),
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject imported nested record mismatches");
    let error = errors
        .iter()
        .find(|error| error.message().contains("record field 'ok' expects"))
        .expect("Expected imported nested record-field mismatch diagnostic");

    assert_eq!(
        error.diagnostic_location(),
        Some(DiagnosticLocation {
            file: Some(root.join("app/main.fol").display().to_string()),
            line: 2,
            column: 27,
            length: Some(1),
        })
    );
}

#[test]
fn workspace_record_initializer_typing_accepts_imported_named_record_contexts() {
    let root = unique_temp_dir("workspace_imported_record_initializers");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] User: rec = {\n",
                    "    name: str;\n",
                    "    count: int;\n",
                    "};\n",
                    "fun[exp] count(user: User): int = {\n",
                    "    return user.count;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "var imported_user: User = { name = \"ok\", count = 1 };\n",
                    "fun[] main(): int = {\n",
                    "    return count({ name = \"next\", count = 2 });\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported named record initializers in bindings and call arguments");
    let syntax_id = find_named_routine_syntax_id(&typed, "main");

    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn entry_value_typing_accepts_entry_variant_accesses() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Color: ent = {\n\
             var BLUE: str = \"#0037cd\";\n\
             var RED: str = \"#ff0000\";\n\
         };\n\
         fun[] blue(): str = {\n\
             return Color.BLUE;\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "blue");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
}

#[test]
fn entry_value_typing_accepts_named_entry_binding_return_and_call_contexts() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Status: ent = {\n\
             var OK: int = 1;\n\
             var FAIL: int = 2;\n\
         };\n\
         fun[] echo(status: Status): Status = {\n\
             return status;\n\
         };\n\
         fun[] main(): Status = {\n\
             var current: Status = Status.OK;\n\
             return echo(Status.FAIL);\n\
         };\n",
    )]);

    let (status_id, _status) = find_typed_symbol(&typed, "Status", SymbolKind::Type);
    let syntax_id = find_named_routine_syntax_id(&typed, "main");

    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: status_id,
            name: "Status".to_string(),
            kind: DeclaredTypeKind::Type,
            args: Vec::new(),
        })
    );
}

#[test]
fn entry_value_typing_rejects_unknown_variants() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Color: ent = {\n\
             var BLUE: str = \"#0037cd\";\n\
         };\n\
         fun[] bad(): str = {\n\
             return Color.BLACK;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("entry receiver does not expose a variant named 'BLACK'")
        }),
        "Expected an unknown-entry-variant diagnostic, got: {errors:?}"
    );
}

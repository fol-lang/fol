use super::*;

#[test]
fn generic_routine_signatures_keep_generic_parameter_facts() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n",
    )]);

    let (_generic_id, generic_symbol) = find_typed_symbol(&typed, "T", SymbolKind::GenericParameter);
    let (_pick_id, pick_symbol) = find_typed_symbol(&typed, "pick", SymbolKind::Routine);
    let signature = match typed
        .type_table()
        .get(pick_symbol.declared_type.expect("generic routine should keep a signature"))
    {
        Some(CheckedType::Routine(signature)) => signature,
        other => panic!("expected routine signature, got {other:?}"),
    };

    assert_eq!(signature.generic_params.len(), 1);
    assert_eq!(signature.generic_params[0], generic_symbol.symbol_id);
    assert_eq!(
        typed.type_table().get(signature.params[0]),
        Some(&CheckedType::Declared {
            symbol: generic_symbol.symbol_id,
            name: "T".to_string(),
            kind: DeclaredTypeKind::GenericParameter,
        })
    );
    assert_eq!(
        signature
            .return_type
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: generic_symbol.symbol_id,
            name: "T".to_string(),
            kind: DeclaredTypeKind::GenericParameter,
        })
    );
}

#[test]
fn generic_routine_calls_infer_identity_return_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             return pick(1);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routine_calls_infer_repeated_type_params_for_same_scalar_family() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun pair(T)(left: T, right: T): T = {\n\
             return right;\n\
         };\n\
         fun[] main(): int = {\n\
             return pair(1, 2);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routine_calls_infer_across_multiple_scalar_families() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun choose(T, U)(left: T, right: U): U = {\n\
             return right;\n\
         };\n\
         fun[] main(): int = {\n\
             var flag: bol = choose(1, true);\n\
             when(flag) {\n\
                 case(true) { return 1; }\n\
                 * { return 0; }\n\
             }\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routine_calls_typecheck_when_nested_inside_expressions() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun id(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun twice(value: int): int = {\n\
             return value + value;\n\
         };\n\
         fun[] main(): int = {\n\
             return twice(id(3)) + id(4);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_receiver_routine_calls_typecheck_with_direct_method_sugar() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Box: rec = {\n\
             value: int\n\
         };\n\
         var current: Box = { value = 1 };\n\
         fun (Box)pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             return current.pick(7);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn explicit_generic_call_turbofish_substitutes_type_argument() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             return pick::[int](7);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn explicit_generic_call_turbofish_reports_arity_mismatch() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             return pick::[int, str](7);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("explicit generic call to 'pick' expects 1 type argument(s) but got 2")),
        "turbofish arity mismatch should produce a clean error: {errors:?}"
    );
}

#[test]
fn explicit_generic_call_turbofish_rejects_non_conforming_constraint() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Plain(): rec = { var width: int; };\n\
         fun measure(T: geo)(value: T): int = {\n\
             return 0;\n\
         };\n\
         fun[] main(): int = {\n\
             var plain: Plain = { width = 1 };\n\
             return measure::[Plain](plain);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("requires type 'Plain' to satisfy standard 'geo'")),
        "turbofish constraint failure should surface the generic-constraint error: {errors:?}"
    );
}

#[test]
fn generic_receiver_types_lower_cleanly() {
    // H1: the "generic receiver types are not yet supported" rejection at
    // routine signature lowering is gone. The routine signature lowers
    // cleanly; method resolution unifies the generic receiver against the
    // call-site object type in H2.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Box(T): rec = {\n\
             value: T\n\
         };\n\
         fun (Box[T])unwrap(T)(fallback: T): T = {\n\
             return fallback;\n\
         };\n",
    )]);

    let (_unwrap_id, unwrap_symbol) = find_typed_symbol(&typed, "unwrap", SymbolKind::Routine);
    assert!(unwrap_symbol.declared_type.is_some(),
        "generic receiver routines must lower to a typed signature");
}

#[test]
fn generic_receiver_routine_call_binds_routine_generic_through_receiver() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Box(T): rec = {\n\
             value: T\n\
         };\n\
         fun (Box[T])unwrap(T)(fallback: T): T = {\n\
             return fallback;\n\
         };\n\
         fun[] main(): int = {\n\
             var box: Box[int] = { value = 7 };\n\
             return box.unwrap(3);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn instantiated_generic_receiver_routines_typecheck_with_direct_method_sugar() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Box(T): rec = {\n\
             value: T\n\
         };\n\
         fun (Box[int])area(): int = {\n\
             return 1;\n\
         };\n\
         fun[] main(): int = {\n\
             var box: Box[int] = { value = 7 };\n\
             return box.area();\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routine_calls_typecheck_with_matching_default_arguments() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun pick(T)(value: T, fallback: int = 1): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             return pick(7);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routine_calls_typecheck_with_concrete_recoverable_error_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun bounce(T)(value: T, fail: bol): T / str = {\n\
             when(fail) {\n\
                 case(true) { report(\"bad\"); }\n\
                 * { return pick(value); }\n\
             }\n\
         };\n\
         fun[] main(): int = {\n\
             when(check(bounce(7, false))) {\n\
                 case(true) { return 0; }\n\
                 * { return 1; }\n\
             }\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routine_calls_typecheck_across_optional_and_vec_signature_shapes() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "ali MaybeInt: opt[int];\n\
         fun keep_opt(T)(value: opt[T]): opt[T] = {\n\
             return value;\n\
         };\n\
         fun keep_vec(T)(items: vec[T]): vec[T] = {\n\
             return items;\n\
         };\n\
         fun[] main(value: MaybeInt): int = {\n\
             var values: vec[int] = {1, 2, 3};\n\
             var kept_opt: MaybeInt = keep_opt(value);\n\
             var kept_values: vec[int] = keep_vec(values);\n\
             return .len(kept_values);\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routine_calls_keep_seq_signature_shapes_on_the_current_boundary() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun keep_seq(T)(items: seq[T]): seq[T] = {\n\
             return items;\n\
         };\n\
         fun[] main(): int = {\n\
             var numbers: seq[int] = {4, 5};\n\
             var kept_numbers: seq[int] = keep_seq(numbers);\n\
             return kept_numbers.len();\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error.message().contains("initializer for 'kept_numbers' expects")
    }), "Expected seq[T] generic signature use to stay on the current explicit boundary, got: {errors:?}");
}

#[test]
fn generic_routine_calls_reject_mismatched_repeated_type_params() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pair(T)(left: T, right: T): T = {\n\
             return left;\n\
         };\n\
         fun[] main(): int = {\n\
             return pair(1, \"x\");\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("call to 'pair' expects")
        }),
        "Expected mismatched repeated generic parameter use to fail locally, got: {errors:?}"
    );
}

#[test]
fn generic_routine_calls_reject_array_vs_scalar_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pair(T)(left: T, right: T): T = {\n\
             return left;\n\
         };\n\
         fun[] main(): arr[int, 3] = {\n\
             var values: arr[int, 3] = {1, 2, 3};\n\
             return pair(values, 1);\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
    }), "Expected array-vs-scalar mismatch to fail with an incompatible-type error, got: {errors:?}");
}

#[test]
fn generic_routine_calls_reject_memo_container_vs_scalar_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pair(T)(left: T, right: T): T = {\n\
             return left;\n\
         };\n\
         fun[] main(): int = {\n\
             var values: vec[int] = {1, 2, 3};\n\
             return pair(values, 1);\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
    }), "Expected memo-container-vs-scalar mismatch to fail with an incompatible-type error, got: {errors:?}");
}

#[test]
fn generic_routine_calls_reject_alias_backed_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "ali Count: int;\n\
         ali Label: str;\n\
         fun pair(T)(left: T, right: T): T = {\n\
             return left;\n\
         };\n\
         fun[] main(): int = {\n\
             var count: Count = 1;\n\
             var label: Label = \"x\";\n\
             return pair(count, label);\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
    }), "Expected alias-backed mismatch to fail with an incompatible-type error, got: {errors:?}");
}

#[test]
fn generic_routine_calls_reject_default_arguments_that_conflict_with_inferred_types() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pair(T)(left: T, right: T = 1): T = {\n\
             return left;\n\
         };\n\
         fun[] main(): chr = {\n\
             return pair(\"x\");\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("default value for parameter 'right' expects")
    }), "Expected defaulted generic parameters to keep an explicit generic/default mismatch error, got: {errors:?}");
}

#[test]
fn generic_routine_calls_reject_variadic_arguments_that_break_inferred_types() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun gather(T)(head: T, tail: ... T): T = {\n\
             return head;\n\
         };\n\
         fun[] main(): int = {\n\
             return gather(1, 2, \"x\");\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error.message().contains("call to 'gather' expects")
    }), "Expected variadic generic mismatches to fail locally, got: {errors:?}");
}

#[test]
fn generic_routine_calls_reject_underconstrained_return_only_generics() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun make(T)(): T = {\n\
             panic(\"boom\");\n\
         };\n\
         fun[] main(): int = {\n\
             return make();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("leaves generic parameter 'T' underconstrained")
                && error
                    .message()
                    .contains("inference only uses call arguments")
        }),
        "Expected underconstrained generic returns to fail explicitly, got: {errors:?}"
    );
}

#[test]
fn generic_routine_calls_reject_partially_inferred_generics() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun carry(T, U)(value: T): U = {\n\
             panic(\"boom\");\n\
         };\n\
         fun[] main(): int = {\n\
             return carry(1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("leaves generic parameter 'U' underconstrained")
                && error
                    .message()
                    .contains("add an argument whose type mentions 'U'")
        }),
        "Expected partially inferred generic calls to fail explicitly, got: {errors:?}"
    );
}

#[test]
fn generic_routine_calls_reject_arguments_omitted_from_inference_even_with_nested_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun choose(T, U)(left: T): U = {\n\
             panic(\"boom\");\n\
         };\n\
         fun id(V)(value: V): V = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             return choose(id(1));\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("leaves generic parameter 'U' underconstrained")
                && error
                    .message()
                    .contains("inference only uses call arguments")
        }),
        "Expected omitted generic arguments to stay underconstrained in M1, got: {errors:?}"
    );
}

#[test]
fn generic_routine_calls_reject_context_only_inference() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun make(T)(): T = {\n\
             panic(\"boom\");\n\
         };\n\
         fun[] main(): int = {\n\
             var value: int = make();\n\
             return value;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("leaves generic parameter 'T' underconstrained")
                && error
                    .message()
                    .contains("make the routine stop depending on 'T' outside the argument list")
        }),
        "Expected contextual return typing to stay outside M1 inference, got: {errors:?}"
    );
}

#[test]
fn generic_routine_values_reject_returning_generic_routines() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
         "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun choose(): {fun (value: int): int} = {\n\
             return pick;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("cannot be used as a plain routine value in V2 Milestone 1")
        }),
        "Expected returning generic routines as values to stay unsupported in M1, got: {errors:?}"
    );
}

#[test]
fn generic_routine_values_reject_plain_callable_bindings() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
         "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             var chosen: {fun (value: int): int} = pick;\n\
             return chosen(1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("cannot be used as a plain routine value in V2 Milestone 1")
        }),
        "Expected generic routine value binding to stay unsupported in M1, got: {errors:?}"
    );
}

#[test]
fn generic_routine_values_reject_storing_generic_routines_in_records() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
         "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         typ Holder: rec = {\n\
             var action: {fun (value: int): int};\n\
         };\n\
         fun[] main(): int = {\n\
             var holder: Holder = { action = pick };\n\
             return holder.action(1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("cannot be used as a plain routine value in V2 Milestone 1")
        }),
        "Expected storing generic routines in aggregates to stay unsupported in M1, got: {errors:?}"
    );
}

#[test]
fn generic_routine_values_reject_plain_callable_arguments() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
         "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun call_once(action: {fun (value: int): int}): int = {\n\
             return action(1);\n\
         };\n\
         fun[] main(): int = {\n\
             return call_once(pick);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("cannot be used as a plain routine value in V2 Milestone 1")
        }),
        "Expected generic routine arguments to stay unsupported in M1, got: {errors:?}"
    );
}

#[test]
fn generic_routines_reject_constraints_exhaustively() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pick(T: int)(value: T): T = {\n\
             return value;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("must resolve to a standard declaration")
        }),
        "Expected non-standard generic constraints to be rejected, got: {errors:?}"
    );
}

#[test]
fn generic_routines_accept_standard_constraints_with_conforming_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 1;\n\
         };\n\
         fun pick(T: geo)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             var rect: Rect = { width = 1 };\n\
             pick(rect);\n\
             return 0;\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn generic_routines_reject_generic_error_shells_exhaustively() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pick(T)(value: T): T / T = {\n\
             return value;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("generic error types are not yet supported in V2 Milestone 1")
        }),
        "Expected generic error shells to stay unsupported in M1, got: {errors:?}"
    );
}

#[test]
fn template_style_generic_calls_remain_explicitly_rejected() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             return pick$(1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("template instantiation is not yet supported")
        }),
        "Expected template-call syntax to remain outside generic-call M1 support, got: {errors:?}"
    );
}

#[test]
fn imported_generic_routine_calls_keep_the_current_workspace_boundary_explicit() {
    let root = unique_temp_dir("generic_workspace_imports_ok");
    create_dir_all(root.join("shared")).expect("shared fixture directory should exist");
    create_dir_all(root.join("app")).expect("app fixture directory should exist");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                "fun[exp] pick(T)(value: T): T = {\n\
                     return value;\n\
                 };\n\
                 fun[exp] choose(T, U)(left: T, right: U): U = {\n\
                     return right;\n\
                 };\n",
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\n\
                 fun[] main(): int = {\n\
                     var ready: bol = choose(1, true);\n\
                     when(ready) {\n\
                         case(true) { return pick(1); }\n\
                         * { return 0; }\n\
                     }\n\
                 };\n",
            ),
        ],
    );

    let errors = typecheck_fixture_entry_with_config(&root, "app", ResolverConfig::default())
        .expect_err("imported generic calls should keep the current workspace-aware boundary");
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("requires workspace-aware typechecking in V1")
    }));
}

#[test]
fn imported_generic_routine_calls_keep_underconstrained_cases_behind_the_same_workspace_boundary() {
    let root = unique_temp_dir("generic_workspace_imports_underconstrained");
    create_dir_all(root.join("shared")).expect("shared fixture directory should exist");
    create_dir_all(root.join("app")).expect("app fixture directory should exist");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                "fun[exp] make(T)(): T = {\n\
                     panic(\"boom\");\n\
                 };\n",
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\n\
                 fun[] main(): int = {\n\
                     return make();\n\
                 };\n",
            ),
        ],
    );

    let errors = typecheck_fixture_entry_with_config(&root, "app", ResolverConfig::default())
        .expect_err("imported underconstrained generic calls should fail at the same workspace boundary");

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("requires workspace-aware typechecking in V1")
    }));
}

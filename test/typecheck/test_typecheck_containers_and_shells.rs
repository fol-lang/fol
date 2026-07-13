use super::*;
use fol_typecheck::TypecheckCapabilityModel;

#[test]
fn workspace_entry_value_typing_accepts_imported_named_entry_contexts() {
    let root = unique_temp_dir("workspace_imported_entry_values");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Status: ent = {\n",
                    "    var OK: int = 1;\n",
                    "    var FAIL: int = 2;\n",
                    "};\n",
                    "fun[exp] echo(status: Status): Status = {\n",
                    "    return status;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "var imported_status: Status = Status.OK;\n",
                    "fun[] main(): Status = {\n",
                    "    return echo(Status.FAIL);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported named entry values in bindings, returns, and call arguments");
    let (_status_id, status) = find_typed_symbol(&typed, "Status", SymbolKind::Type);
    let syntax_id = find_named_routine_syntax_id(&typed, "main");

    assert_eq!(
        typed.type_table().get(status.declared_type.expect("imported entry type should retain a declared shell")),
        Some(&CheckedType::Entry {
            variants: BTreeMap::from([
                ("FAIL".to_string(), Some(typed.builtin_types().int)),
                ("OK".to_string(), Some(typed.builtin_types().int)),
            ]),
        })
    );
    assert!(matches!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(CheckedType::Declared {
            name,
            kind: DeclaredTypeKind::Type,
            ..
        }) if name == "Status"
    ));
}

#[test]
fn workspace_aggregate_typing_keeps_qualified_imported_record_and_entry_surfaces() {
    let root = unique_temp_dir("workspace_qualified_imported_aggregates");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] User: rec = {\n",
                    "    count: int;\n",
                    "};\n",
                    "var[exp] current: User = { count = 1 };\n",
                    "typ[exp] Status: ent = {\n",
                    "    var OK: int = 1;\n",
                    "    var FAIL: int = 2;\n",
                    "};\n",
                    "fun[exp] echo(status: Status): Status = {\n",
                    "    return status;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): Status = {\n",
                    "    var total: int = shared::current.count;\n",
                    "    return shared::echo(shared::Status.OK);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should keep qualified imported record and entry aggregate surfaces");
    let syntax_id = find_named_routine_syntax_id(&typed, "main");

    assert!(matches!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(CheckedType::Declared {
            name,
            kind: DeclaredTypeKind::Type,
            ..
        }) if name == "Status"
    ));
}

#[test]
fn shell_typing_accepts_optional_and_error_payload_lifting() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "ali MaybeText: opt[str];\n\
         ali Failure: err[str];\n\
         var label: MaybeText = \"ok\";\n\
         var issue: Failure = \"bad\";\n\
         fun[] maybe(): MaybeText = {\n\
             return \"ready\";\n\
         };\n\
         fun[] fail(): int / Failure = {\n\
             report \"broken\";\n\
             return 0;\n\
         };\n",
    )]);

    let (maybe_id, _maybe_alias) = find_typed_symbol(&typed, "MaybeText", SymbolKind::Alias);
    let (failure_id, _failure_alias) = find_typed_symbol(&typed, "Failure", SymbolKind::Alias);
    let (_label_id, label) = find_typed_symbol(&typed, "label", SymbolKind::ValueBinding);
    let (_issue_id, issue) = find_typed_symbol(&typed, "issue", SymbolKind::ValueBinding);
    let maybe_syntax = find_named_routine_syntax_id(&typed, "maybe");
    let fail_syntax = find_named_routine_syntax_id(&typed, "fail");

    assert_eq!(
        typed.type_table().get(label.declared_type.expect("optional binding should lower")),
        Some(&CheckedType::Declared {
            symbol: maybe_id,
            name: "MaybeText".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    assert_eq!(
        typed.type_table().get(issue.declared_type.expect("error binding should lower")),
        Some(&CheckedType::Declared {
            symbol: failure_id,
            name: "Failure".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    assert_eq!(
        typed
            .typed_node(maybe_syntax)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: maybe_id,
            name: "MaybeText".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    assert_eq!(
        typed
            .typed_node(fail_syntax)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn shell_typing_rejects_mismatched_optional_payloads() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "ali MaybeText: opt[str];\n\
         fun[] bad(): int = {\n\
             var label: MaybeText = 1;\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error
                    .message()
                    .contains("initializer for 'label' expects")
        }),
        "Expected an optional-shell mismatch diagnostic, got: {errors:?}"
    );
}

#[test]
fn shell_typing_accepts_shipped_v3_pointer_types() {
    let _typed = typecheck_fixture_folder(&[("main.fol", "ali CounterPtr: ptr[int];\n")]);
}

#[test]
fn core_model_rejects_heap_backed_type_surfaces() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            concat!(
                "ali Label: str;\n",
                "ali Numbers: vec[int];\n",
                "ali Steps: seq[int];\n",
                "ali Names: set[str];\n",
                "ali Scores: map[str, int];\n",
            ),
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        },
    );

    let messages = errors.iter().map(|error| error.message()).collect::<Vec<_>>();
    assert!(messages
        .iter()
        .any(|message| message.contains("str requires heap support")));
    assert!(messages
        .iter()
        .any(|message| message.contains("vec[...] requires heap support")));
    assert!(messages
        .iter()
        .any(|message| message.contains("seq[...] requires heap support")));
    assert!(messages
        .iter()
        .any(|message| message.contains("set[...] requires heap support")));
    assert!(messages
        .iter()
        .any(|message| message.contains("map[...] requires heap support")));
}

#[test]
fn core_model_rejects_inferred_string_literals() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            concat!(
                "fun[] text(): int = {\n",
                "    var label = \"ok\";\n",
                "    return 0;\n",
                "};\n",
            ),
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        },
    );

    let messages = errors.iter().map(|error| error.message()).collect::<Vec<_>>();
    assert!(messages
        .iter()
        .any(|message| message.contains("string literals require heap support")));
}

#[test]
fn operator_typing_accepts_v1_scalar_operator_families() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] math(): int = {\n\
             return 1 + 2 * 3 - 4 % 2;\n\
         };\n\
         fun[] text(): str = {\n\
             return \"ab\" + \"cd\";\n\
         };\n\
         fun[] compare(): bol = {\n\
             return 1 < 2 and 3 != 4;\n\
         };\n\
         fun[] invert(flag: bol): bol = {\n\
             return not flag xor false;\n\
         };\n",
    )]);

    for (name, expected) in [
        ("math", CheckedType::Builtin(BuiltinType::Int)),
        ("text", CheckedType::Builtin(BuiltinType::Str)),
        ("compare", CheckedType::Builtin(BuiltinType::Bool)),
        ("invert", CheckedType::Builtin(BuiltinType::Bool)),
    ] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&expected),
            "Expected {name} to typecheck as {expected:?}"
        );
    }
}

#[test]
fn operator_typing_rejects_invalid_scalar_pairs_and_pointer_operators() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_math(): int = {\n\
             return true + 1;\n\
         };\n\
         fun[] bad_logic(): bol = {\n\
             return 1 and 2;\n\
         };\n\
         fun[] bad_ref(value: int): int = {\n\
             return &value;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("binary operator 'Add' is not valid")
        }),
        "Expected an invalid arithmetic-operator diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("binary operator 'And' is not valid")
        }),
        "Expected an invalid logical-operator diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("got 'ptr[int]'")
        }),
        "Expected the typed pointer result to mismatch the int return, got: {errors:?}"
    );
}

#[test]
fn intrinsic_comparison_typing_accepts_v1_equality_pairs() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] same_number(): bol = {\n\
             return .eq(1, 1);\n\
         };\n\
         fun[] different_flag(flag: bol): bol = {\n\
             return .nq(flag, false);\n\
         };\n\
         fun[] same_text(): bol = {\n\
             return .eq(\"Ada\", \"Ada\");\n\
         };\n",
    )]);

    for name in ["same_number", "different_flag", "same_text"] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&CheckedType::Builtin(BuiltinType::Bool)),
            "Expected {name} to lower to bool"
        );
    }
}

#[test]
fn intrinsic_comparison_typing_rejects_wrong_arity_and_mixed_scalar_pairs() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_arity(): bol = {\n\
             return .eq(1);\n\
         };\n\
         fun[] bad_pair(): bol = {\n\
             return .eq(1, \"Ada\");\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(".eq(...) expects exactly 2 argument(s) but got 1")
        }),
        "Expected wrong-arity intrinsic diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains(".eq(...) expects two comparable scalar operands")
        }),
        "Expected wrong-type-family intrinsic diagnostic, got: {errors:?}"
    );
}

#[test]
fn intrinsic_ordered_comparison_typing_accepts_v1_ordered_pairs() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] ints(): bol = {\n\
             return .lt(1, 2);\n\
         };\n\
         fun[] text(): bol = {\n\
             return .ge(\"Ada\", \"Ada\");\n\
         };\n\
         fun[] chars(): bol = {\n\
             return .le('a', 'z');\n\
         };\n\
         fun[] floats(): bol = {\n\
             return .gt(3.5, 1.0);\n\
         };\n",
    )]);

    for name in ["ints", "text", "chars", "floats"] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&CheckedType::Builtin(BuiltinType::Bool)),
            "Expected {name} to lower to bool"
        );
    }
}

#[test]
fn intrinsic_ordered_comparison_typing_rejects_non_ordered_pairs() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_bool(): bol = {\n\
             return .lt(true, false);\n\
         };\n\
         fun[] bad_mixed(): bol = {\n\
             return .gt(1, 1.0);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains(".lt(...) expects two ordered scalar operands")
        }),
        "Expected ordered-family intrinsic diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains(".gt(...) expects two ordered scalar operands")
        }),
        "Expected mixed ordered-family intrinsic diagnostic, got: {errors:?}"
    );
}

#[test]
fn intrinsic_boolean_typing_accepts_not_for_bool_values() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] flip(flag: bol): bol = {\n\
             return .not(flag);\n\
         };\n\
         fun[] literal(): bol = {\n\
             return .not(false);\n\
         };\n",
    )]);

    for name in ["flip", "literal"] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&CheckedType::Builtin(BuiltinType::Bool)),
            "Expected {name} to lower to bol through .not(...)",
        );
    }
}

#[test]
fn intrinsic_boolean_typing_rejects_wrong_arity_and_non_boolean_operands() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_arity(): bol = {\n\
             return .not();\n\
         };\n\
         fun[] bad_type(): bol = {\n\
             return .not(1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(".not(...) expects exactly 1 argument(s) but got 0")
        }),
        "Expected wrong-arity .not diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains(".not(...) expects one boolean operand but got 'int'")
        }),
        "Expected non-boolean .not diagnostic, got: {errors:?}"
    );
}

#[test]
fn intrinsic_query_typing_accepts_len_for_v1_length_queries() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] text_len(): int = {\n\
             return .len(\"Ada\");\n\
         };\n\
         fun[] seq_len(items: seq[int]): int = {\n\
             return .len(items);\n\
         };\n",
    )]);

    for name in ["text_len", "seq_len"] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&CheckedType::Builtin(BuiltinType::Int)),
            "Expected {name} to lower to int through .len(...)",
        );
    }
}

#[test]
fn intrinsic_query_typing_rejects_wrong_arity_and_non_length_operands() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_arity(items: seq[int]): int = {\n\
             return .len(items, items);\n\
         };\n\
         fun[] bad_type(): int = {\n\
             return .len(1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(".len(...) expects exactly 1 argument(s) but got 2")
        }),
        "Expected wrong-arity .len diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(
                    ".len(...) expects one string, array, vector, sequence, set, or map operand",
                )
                && error.message().contains("'int'")
        }),
        "Expected non-length .len diagnostic, got: {errors:?}"
    );
}

#[test]
fn intrinsic_query_typing_covers_full_v1_length_family_matrix() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Flagged: rec = {\n\
             name: str;\n\
         };\n\
         fun[] text_len(): int = {\n\
             return .len(\"Ada\");\n\
         };\n\
         fun[] arr_len(items: arr[int, 3]): int = {\n\
             return .len(items);\n\
         };\n\
         fun[] vec_len(items: vec[int]): int = {\n\
             return .len(items);\n\
         };\n\
         fun[] seq_len(items: seq[int]): int = {\n\
             return .len(items);\n\
         };\n\
         fun[] set_len(items: set[int, str]): int = {\n\
             return .len(items);\n\
         };\n\
         fun[] map_len(items: map[str, int]): int = {\n\
             return .len(items);\n\
         };\n",
    )]);

    for name in ["text_len", "arr_len", "vec_len", "seq_len", "set_len", "map_len"] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&CheckedType::Builtin(BuiltinType::Int)),
            "Expected {name} to lower to int through .len(...)",
        );
    }
}

#[test]
fn core_model_keeps_array_length_queries_available() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            concat!(
                "fun[] arr_len(items: arr[int, 3]): int = {\n",
                "    return .len(items);\n",
                "};\n",
            ),
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        },
    );

    let syntax_id = find_named_routine_syntax_id(&typed, "arr_len");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int)),
        "Expected array .len(...) to remain available in 'fol_model = core'",
    );
}

#[test]
fn mem_model_accepts_dynamic_length_queries() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            concat!(
                "fun[] text_len(): int = {\n",
                "    return .len(\"Ada\");\n",
                "};\n",
                "fun[] seq_len(items: seq[int]): int = {\n",
                "    return .len(items);\n",
                "};\n",
            ),
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Memo,
        },
    );

    for name in ["text_len", "seq_len"] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&CheckedType::Builtin(BuiltinType::Int)),
            "Expected {name} to retain dynamic .len(...) support in 'fol_model = memo'",
        );
    }
}

#[test]
fn intrinsic_query_typing_rejects_non_query_receiver_families() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Flagged: rec = {\n\
             name: str;\n\
         };\n\
         fun[] bad_record(value: Flagged): int = {\n\
             return .len(value);\n\
         };\n\
         fun[] bad_optional(value: opt[str]): int = {\n\
             return .len(value);\n\
         };\n",
    )]);

    assert_eq!(
        errors
            .iter()
            .filter(|error| {
                error.kind() == TypecheckErrorKind::InvalidInput
                    && error.message().contains(
                        ".len(...) expects one string, array, vector, sequence, set, or map operand",
                    )
            })
            .count(),
        2,
        "Expected rejected .len diagnostics for record and optional receivers, got: {errors:?}"
    );
}

#[test]
fn intrinsic_query_typing_distinguishes_implemented_and_deferred_families() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] count(items: seq[int]): int = {\n\
             return .len(items);\n\
         };\n",
    )]);
    let count_syntax_id = find_named_routine_syntax_id(&typed, "count");
    assert_eq!(
        typed
            .typed_node(count_syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int)),
        "Expected .len(...) to remain the implemented V1 query intrinsic",
    );

    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] capacity(items: seq[int]): int = {\n\
             return .cap(items);\n\
         };\n\
         fun[] low_bound(items: seq[int]): int = {\n\
             return .low(items);\n\
         };\n\
         fun[] minimum(left: int, right: int): int = {\n\
             return .min(left, right);\n\
         };\n",
    )]);

    for expected in [
        ".cap(...) is not yet supported",
        ".low(...) is not yet supported",
        ".min(...) is not yet supported",
    ] {
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error.message().contains(expected)
            }),
            "Expected deferred intrinsic diagnostic containing '{expected}', got: {errors:?}"
        );
    }
}

#[test]
fn intrinsic_diagnostic_typing_accepts_echo_as_a_value_preserving_tap() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] log_flag(flag: bol): bol = {\n\
             return .echo(flag);\n\
         };\n\
         fun[] log_count(items: seq[int]): int = {\n\
             return .echo(.len(items));\n\
         };\n",
    )]);

    for (name, expected) in [
        ("log_flag", CheckedType::Builtin(BuiltinType::Bool)),
        ("log_count", CheckedType::Builtin(BuiltinType::Int)),
    ] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&expected),
            "Expected {name} to preserve its operand type through .echo(...)",
        );
    }
}

#[test]
fn intrinsic_diagnostic_typing_rejects_wrong_arity_for_echo() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_arity(): int = {\n\
             return .echo();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(".echo(...) expects exactly 1 argument(s) but got 0")
        }),
        "Expected wrong-arity .echo diagnostic, got: {errors:?}"
    );
}

#[test]
fn retired_memory_intrinsics_are_unknown_and_deallocation_is_v4_only() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] free_value(value: int): int = {\n\
             return .de_alloc(value);\n\
         };\n\
         fun[] hand_back(value: int): int = {\n\
             return .give_back(value);\n\
         };\n\
         fun[] take_address(value: int): int = {\n\
             return .address_of(value);\n\
         };\n\
         fun[] read_pointer(value: int): int = {\n\
             return .pointer_value(value);\n\
         };\n\
         fun[] borrow_value(value: int): int = {\n\
             return .borrow_from(value);\n\
         };\n",
    )]);

    for intrinsic in [
        ".de_alloc(...) is a V4/FFI boundary and is not part of V3",
        "unknown dot-root intrinsic '.give_back(...)'",
        "unknown dot-root intrinsic '.address_of(...)'",
        "unknown dot-root intrinsic '.pointer_value(...)'",
        "unknown dot-root intrinsic '.borrow_from(...)'",
    ] {
        assert!(
            errors.iter().any(|error| {
                error.message().contains(intrinsic)
            }),
            "Expected retired/boundary intrinsic diagnostic containing '{intrinsic}', got: {errors:?}"
        );
    }
}

#[test]
fn intrinsic_comparison_typing_covers_full_v1_scalar_matrix() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] eq_ints(): bol = {\n\
             return .eq(1, 1);\n\
         };\n\
         fun[] eq_floats(): bol = {\n\
             return .eq(1.0, 1.0);\n\
         };\n\
         fun[] eq_bools(): bol = {\n\
             return .eq(true, false);\n\
         };\n\
         fun[] eq_chars(): bol = {\n\
             return .eq('a', 'z');\n\
         };\n\
         fun[] eq_text(): bol = {\n\
             return .eq(\"Ada\", \"Lin\");\n\
         };\n\
         fun[] lt_ints(): bol = {\n\
             return .lt(1, 2);\n\
         };\n\
         fun[] lt_floats(): bol = {\n\
             return .lt(1.0, 2.0);\n\
         };\n\
         fun[] lt_chars(): bol = {\n\
             return .lt('a', 'z');\n\
         };\n\
         fun[] lt_text(): bol = {\n\
             return .lt(\"Ada\", \"Lin\");\n\
         };\n",
    )]);

    for name in [
        "eq_ints",
        "eq_floats",
        "eq_bools",
        "eq_chars",
        "eq_text",
        "lt_ints",
        "lt_floats",
        "lt_chars",
        "lt_text",
    ] {
        let syntax_id = find_named_routine_syntax_id(&typed, name);
        assert_eq!(
            typed
                .typed_node(syntax_id)
                .and_then(|node| node.inferred_type)
                .and_then(|type_id| typed.type_table().get(type_id)),
            Some(&CheckedType::Builtin(BuiltinType::Bool)),
            "Expected {name} to resolve to bol across the supported V1 scalar matrix",
        );
    }
}

#[test]
fn intrinsic_comparison_typing_rejects_non_scalar_and_cross_family_pairs() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_container(): bol = {\n\
             return .eq({1, 2}, {1, 2});\n\
         };\n\
         fun[] bad_ordered_bool(): bol = {\n\
             return .lt(true, false);\n\
         };\n\
         fun[] bad_mixed_eq(): bol = {\n\
             return .eq(1, 1.0);\n\
         };\n\
         fun[] bad_mixed_lt(): bol = {\n\
             return .lt('a', 1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains(".eq(...) expects two comparable scalar operands")
        }),
        "Expected non-scalar equality-family intrinsic diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains(".lt(...) expects two ordered scalar operands")
        }),
        "Expected ordered-family intrinsic diagnostic, got: {errors:?}"
    );
}

#[test]
fn coercion_policy_rejects_implicit_int_float_cross_family_conversions() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] take_float(value: flt): flt = {\n\
             return value;\n\
         };\n\
         fun[] bad_binding(): int = {\n\
             var count: int = 1.5;\n\
             return count;\n\
         };\n\
         fun[] bad_call(): flt = {\n\
             return take_float(1);\n\
         };\n\
         fun[] bad_return(): int = {\n\
             return 1.5;\n\
         };\n\
         fun[] bad_report(): int / int = {\n\
             report 1.5;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error
                    .message()
                    .contains("initializer for 'count' expects")
        }),
        "Expected an initializer coercion diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("call to 'take_float' expects")
        }),
        "Expected a call coercion diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("return expects")
        }),
        "Expected a return coercion diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("report expects")
        }),
        "Expected a report coercion diagnostic, got: {errors:?}"
    );
}

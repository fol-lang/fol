use super::*;

#[test]
fn workspace_expression_typing_rejects_plain_imported_call_argument_mismatches() {
    let root = unique_temp_dir("workspace_imported_call_checks");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return emit(\"bad\");\n",
                    "};\n",
                ),
            ),
        ],
    );

    let errors = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect_err("Workspace entry typing should reject imported call argument mismatches");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error
                    .message()
                    .contains("call to 'emit' expects 'int'")
        }),
        "Expected imported-call argument mismatch diagnostic, got: {errors:?}"
    );
}

#[test]
fn workspace_expression_typing_keeps_qualified_imported_value_and_call_types() {
    let root = unique_temp_dir("workspace_qualified_import_types");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "var[exp] answer: int = 42;\n",
                    "fun[exp] emit(value: int): int = {\n",
                    "    return value;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    var current: int = shared::answer;\n",
                    "    return shared::emit(current);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept qualified imported value and call references");
    let value_reference =
        find_typed_reference(&typed, "shared::answer", ReferenceKind::QualifiedIdentifier);
    let call_reference =
        find_typed_reference(&typed, "shared::emit", ReferenceKind::QualifiedFunctionCall);

    assert_eq!(
        typed.type_table().get(
            value_reference
                .resolved_type
                .expect("qualified imported value references should keep a resolved type"),
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
    assert_eq!(
        typed.type_table().get(
            call_reference
                .resolved_type
                .expect("qualified imported call references should keep a resolved type"),
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn workspace_expression_typing_types_plain_imported_method_calls() {
    let root = unique_temp_dir("workspace_imported_method_calls");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "fun[exp] (Counter)read(): int = {\n",
                    "    return 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.read();\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported method calls through typed package facts");
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
fn workspace_expression_typing_accepts_named_imported_method_calls() {
    let root = unique_temp_dir("workspace_named_imported_method_calls");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "fun[exp] (Counter)shift(by: int, step: int): int = {\n",
                    "    return by;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.shift(step = 2, by = 1);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept named imported method calls through typed package facts");
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
fn workspace_expression_typing_accepts_imported_default_parameters_for_method_calls() {
    let root = unique_temp_dir("workspace_imported_default_method_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "fun[exp] (Counter)shift(by: int, step: int = 2): int = {\n",
                    "    return by;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.shift(1);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported method-call defaults through typed package facts");
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
fn workspace_expression_typing_accepts_imported_variadic_free_calls() {
    let root = unique_temp_dir("workspace_imported_variadic_free_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "fun[exp] sum(head: int, tail: ... int): int = {\n",
                    "    return head;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return sum(1, 2, 3, 4);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported variadic free calls through typed package facts");
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
fn workspace_expression_typing_accepts_imported_unpack_for_variadic_free_calls() {
    let root = unique_temp_dir("workspace_imported_unpack_variadic_free_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "var[exp] nums: seq[int];\n",
                    "fun[exp] sum(head: int, tail: ... int): int = {\n",
                    "    return head;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return sum(1, ...nums);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported variadic unpack free calls through typed package facts");
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
fn workspace_expression_typing_accepts_named_imported_unpack_for_variadic_free_calls() {
    let root = unique_temp_dir("workspace_named_imported_unpack_variadic_free_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "var[exp] nums: seq[int];\n",
                    "fun[exp] score(base: int, step: int = 2, tail: ... int): int = {\n",
                    "    return base;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return score(base = 1, ...nums);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept named imported variadic unpack free calls through typed package facts");
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
fn workspace_expression_typing_accepts_imported_variadic_method_calls() {
    let root = unique_temp_dir("workspace_imported_variadic_method_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "fun[exp] (Counter)shift(values: ... int): int = {\n",
                    "    return 0;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.shift(1, 2, 3);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported variadic method calls through typed package facts");
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
fn workspace_expression_typing_accepts_named_imported_unpack_for_variadic_method_calls() {
    let root = unique_temp_dir("workspace_named_imported_unpack_variadic_method_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "var[exp] nums: seq[int];\n",
                    "fun[exp] (Counter)shift(step: int = 2, values: ... int): int = {\n",
                    "    return step;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.shift(step = 3, ...nums);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept named imported variadic unpack method calls through typed package facts");
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
fn workspace_expression_typing_accepts_imported_unpack_for_variadic_method_calls() {
    let root = unique_temp_dir("workspace_imported_unpack_variadic_method_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "var[exp] nums: seq[int];\n",
                    "fun[exp] (Counter)shift(values: ... int): int = {\n",
                    "    return 0;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.shift(...nums);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported variadic unpack method calls through typed package facts");
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
fn workspace_expression_typing_accepts_imported_default_parameters_for_free_calls() {
    let root = unique_temp_dir("workspace_imported_default_free_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "fun[exp] pair(left: int, right: int = 2): int = {\n",
                    "    return left;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return pair(1);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept imported free-call defaults through typed package facts");
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
fn workspace_expression_typing_rejects_unknown_named_imported_free_calls() {
    let root = unique_temp_dir("workspace_unknown_named_imported_free_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "fun[exp] pair(left: int, right: int): int = {\n",
                    "    return left;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return pair(missing = 1, left = 2);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject unknown named imported free-call args");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("does not have a parameter named 'missing'")
        }),
        "Expected an unknown named imported free-call diagnostic, got: {errors:?}"
    );
}

#[test]
fn workspace_expression_typing_rejects_duplicate_named_imported_method_calls() {
    let root = unique_temp_dir("workspace_duplicate_named_imported_method_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "fun[exp] (Counter)shift(by: int, step: int): int = {\n",
                    "    return by;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.shift(by = 1, by = 2);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject duplicate named imported method args");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("supplies parameter 'by' more than once")
        }),
        "Expected a duplicate named imported method-call diagnostic, got: {errors:?}"
    );
}

#[test]
fn workspace_expression_typing_rejects_unpack_for_imported_non_variadic_free_calls() {
    let root = unique_temp_dir("workspace_unpack_non_variadic_imported_free_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "var[exp] nums: seq[int];\n",
                    "fun[exp] pair(left: int): int = {\n",
                    "    return left;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return pair(...nums);\n",
                    "};\n",
                ),
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject unpack on imported non-variadic free calls");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("call-site unpack is only supported for variadic calls in V1")
        }),
        "Expected an imported non-variadic unpack diagnostic, got: {errors:?}"
    );
}

#[test]
fn workspace_expression_typing_types_qualified_imported_method_calls() {
    let root = unique_temp_dir("workspace_qualified_imported_method_calls");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current: Counter;\n",
                    "fun[exp] (Counter)read(): int = {\n",
                    "    return 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return shared::current.read();\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept qualified imported method calls through typed package facts");
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
fn workspace_expression_typing_selects_imported_method_overloads_by_record_receiver_type() {
    let root = unique_temp_dir("workspace_imported_method_overloads");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Counter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "typ[exp] Meter: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "var[exp] current_counter: Counter;\n",
                    "var[exp] current_meter: Meter;\n",
                    "fun[exp] (Counter)read(): int = {\n",
                    "    return 1;\n",
                    "};\n",
                    "fun[exp] (Meter)read(): bol = {\n",
                    "    return true;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] read_counter(): int = {\n",
                    "    return current_counter.read();\n",
                    "};\n",
                    "fun[] read_meter(): bol = {\n",
                    "    return shared::current_meter.read();\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should select imported method overloads by receiver type");
    let counter_syntax_id = find_named_routine_syntax_id(&typed, "read_counter");
    let meter_syntax_id = find_named_routine_syntax_id(&typed, "read_meter");

    assert_eq!(
        typed
            .typed_node(counter_syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
    assert_eq!(
        typed
            .typed_node(meter_syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Bool))
    );
}

#[test]
fn workspace_expression_typing_expands_imported_alias_record_shells_for_field_access() {
    let root = unique_temp_dir("workspace_imported_alias_record_field_access");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ CounterShape: rec = {\n",
                    "    value: int;\n",
                    "};\n",
                    "ali Counter: CounterShape;\n",
                    "var[exp] current: Counter = { value = 1 };\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.value;\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should expand imported alias record shells for field access");
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
fn reopened_v1_imported_missing_method_diagnostics_keep_call_site_locations() {
    let root = unique_temp_dir("reopened_imported_missing_method_locations");
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
                    "var[exp] current: User;\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return current.bump();\n",
                    "};\n",
                ),
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject missing imported methods");
    let error = errors
        .iter()
        .find(|error| {
            error
                .message()
                .contains("method 'bump' is not available for the receiver type in V1")
        })
        .expect("Expected missing imported-method diagnostic");

    assert_eq!(
        error.diagnostic_location(),
        Some(DiagnosticLocation {
            file: Some(root.join("app/main.fol").display().to_string()),
            line: 3,
            column: 12,
            length: Some(7),
        })
    );
}

#[test]
fn legacy_single_package_typecheck_rejects_imported_loc_values_explicitly() {
    let root = unique_temp_dir("reopened_loc_import");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("shared/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
            ),
        ],
    );

    let errors = typecheck_fixture_entry_with_config(&root, "app", ResolverConfig::default())
        .expect_err("legacy single-package typechecking should still reject imported loc values");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("requires workspace-aware typechecking in V1")
                && error.diagnostic_location().is_some()
        }),
        "Expected the explicit legacy loc-import diagnostic, got: {errors:?}"
    );
}

#[test]
fn legacy_single_package_typecheck_rejects_imported_std_values_explicitly() {
    let root = unique_temp_dir("reopened_std_import");
    let store_root = root.join("store");
    create_dir_all(&store_root).expect("Package store root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "store/std/build.fol",
                "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"std\", version = \"0.1.0\" });\n};\n",
            ),
            ("store/std/src/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return std::src::answer;\n};\n",
            ),
        ],
    );

    let errors = typecheck_fixture_entry_with_config(
        &root,
        "app",
        ResolverConfig {
            std_root: None,
            package_store_root: Some(
                store_root
                    .to_str()
                    .expect("package store root should be utf8")
                    .to_string(),
            ),
        },
    )
    .expect_err("legacy single-package typechecking should still reject imported std values");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("requires workspace-aware typechecking in V1")
                && error.diagnostic_location().is_some()
        }),
        "Expected the explicit legacy std-import diagnostic, got: {errors:?}"
    );
}

#[test]
fn legacy_single_package_typecheck_rejects_imported_pkg_values_explicitly() {
    let root = unique_temp_dir("reopened_pkg_import");
    let store_root = root.join("store");
    create_dir_all(&store_root).expect("Package store root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("store/json/build.fol", "name: json\nversion: 1.0.0\n"),
            (
                "store/json/build.fol",
                "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"json\",\n        version = \"1.0.0\",\n    });\n};\n",
            ),
            ("store/json/src/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return json::src::answer;\n};\n",
            ),
        ],
    );

    let errors = typecheck_fixture_entry_with_config(
        &root,
        "app",
        ResolverConfig {
            std_root: None,
            package_store_root: Some(
                store_root
                    .to_str()
                    .expect("package store fixture path should be utf8")
                    .to_string(),
            ),
        },
    )
    .expect_err("legacy single-package typechecking should still reject imported pkg values");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("requires workspace-aware typechecking in V1")
                && error.diagnostic_location().is_some()
        }),
        "Expected the explicit legacy pkg-import diagnostic, got: {errors:?}"
    );
}

#[test]
fn legacy_single_package_typecheck_rejects_imported_routine_calls_explicitly() {
    let root = unique_temp_dir("reopened_imported_call");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                "fun[exp] answer(): int = {\n    return 42;\n};\n",
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer();\n};\n",
            ),
        ],
    );

    let errors = typecheck_fixture_entry_with_config(&root, "app", ResolverConfig::default())
        .expect_err("legacy single-package typechecking should still reject imported routine calls");

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("requires workspace-aware typechecking in V1")
                && error.diagnostic_location().is_some()
        }),
        "Expected the explicit legacy imported-call diagnostic, got: {errors:?}"
    );
}

#[test]
fn shell_typing_accepts_nil_in_optional_and_error_binding_contexts() {
    let typed = typecheck_fixture_folder(&[
        (
            "main.fol",
            "ali MaybeText: opt[str];\nali Failure: err[str];\nali BareFailure: err[];\nvar label: MaybeText = nil;\nvar raised: Failure = nil;\nvar empty: BareFailure = nil;\n",
        ),
    ]);

    let (_label_id, label) = find_typed_symbol(&typed, "label", SymbolKind::ValueBinding);
    let (_raised_id, raised) = find_typed_symbol(&typed, "raised", SymbolKind::ValueBinding);
    let (_empty_id, empty) = find_typed_symbol(&typed, "empty", SymbolKind::ValueBinding);

    assert!(matches!(
        typed
            .type_table()
            .get(label.declared_type.expect("label should keep its declared type")),
        Some(CheckedType::Declared { name, .. }) if name == "MaybeText"
    ));
    assert!(matches!(
        typed
            .type_table()
            .get(raised.declared_type.expect("raised should keep its declared type")),
        Some(CheckedType::Declared { name, .. }) if name == "Failure"
    ));
    assert!(matches!(
        typed
            .type_table()
            .get(empty.declared_type.expect("empty should keep its declared type")),
        Some(CheckedType::Declared { name, .. }) if name == "BareFailure"
    ));
}

#[test]
fn typecheck_reports_explicit_top_level_binding_type_requirements() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var mystery;\nfun[] main(): int = {\n    return mystery;\n};\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("binding 'mystery' needs a declared type or an inferable initializer in V1")
        }),
        "Expected the explicit top-level binding type diagnostic, got: {errors:?}"
    );
}

#[test]
fn typecheck_reports_explicit_local_binding_type_requirements() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n    var mystery;\n    return mystery;\n};\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("binding 'mystery' needs a declared type or an inferable initializer in V1")
        }),
        "Expected the explicit local binding type diagnostic, got: {errors:?}"
    );
}

#[test]
fn reopened_v1_import_failures_no_longer_use_raw_lowered_type_fallbacks() {
    let root = unique_temp_dir("reopened_loc_import_regression");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("shared/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
            ),
        ],
    );

    let errors = typecheck_fixture_entry_with_config(&root, "app", ResolverConfig::default())
        .expect_err("Legacy single-program typecheck should still reject imported value typing");

    assert!(
        errors.iter().all(|error| {
            error.kind() != TypecheckErrorKind::Internal
                && !error.message().contains("does not have a lowered type yet")
        }),
        "Imported fallback regressions should no longer surface raw lowered-type wording, got: {errors:?}"
    );
}

#[test]
fn reopened_v1_binding_failures_no_longer_use_raw_lowered_type_fallbacks() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n    var mystery;\n    return mystery;\n};\n",
    )]);

    assert!(
        errors.iter().all(|error| {
            error.kind() != TypecheckErrorKind::Internal
                && !error.message().contains("does not have a lowered type yet")
        }),
        "Binding fallback regressions should no longer surface raw lowered-type wording, got: {errors:?}"
    );
}

#[test]
fn reopened_v1_imported_value_diagnostics_keep_binding_site_locations() {
    let root = unique_temp_dir("reopened_imported_value_locations");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("shared/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nvar label: str = answer;\n",
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject imported binding mismatches");
    let error = errors
        .iter()
        .find(|error| error.message().contains("initializer for 'label' expects"))
        .expect("Expected imported binding mismatch diagnostic");

    assert_eq!(
        error.diagnostic_location(),
        Some(DiagnosticLocation {
            file: Some(root.join("app/main.fol").display().to_string()),
            line: 2,
            column: 18,
            length: Some(6),
        })
    );
}

#[test]
fn reopened_v1_imported_call_diagnostics_keep_call_site_locations() {
    let root = unique_temp_dir("reopened_imported_call_locations");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                "fun[exp] twice(value: int): int = {\n    return value;\n};\n",
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return twice(\"bad\");\n};\n",
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject imported call argument mismatches");
    let error = errors
        .iter()
        .find(|error| error.message().contains("call to 'twice' expects"))
        .expect("Expected imported call mismatch diagnostic");

    assert_eq!(
        error.diagnostic_location(),
        Some(DiagnosticLocation {
            file: Some(root.join("app/main.fol").display().to_string()),
            line: 3,
            column: 12,
            length: Some(5),
        })
    );
}

#[test]
fn reopened_v1_imported_method_diagnostics_keep_call_site_locations() {
    let root = unique_temp_dir("reopened_imported_method_locations");
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
                    "fun[exp] bump(user: User, step: int): int = {\n",
                    "    return step;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    var user: User = { count = 1 };\n",
                    "    return user.bump();\n",
                    "};\n",
                ),
            ),
        ],
    );

    let errors =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect_err("Workspace entry typing should reject imported method argument mismatches");
    assert!(
        errors.iter().any(|error| error.message().contains("bump")),
        "Expected imported method diagnostics to mention 'bump', got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.diagnostic_location()
                == Some(DiagnosticLocation {
                    file: Some(root.join("app/main.fol").display().to_string()),
                    line: 4,
                    column: 12,
                    length: Some(4),
                })
        }),
        "Expected imported method diagnostics to preserve the call-site location, got: {errors:?}"
    );
}

#[test]
fn reopened_v1_nil_diagnostics_keep_binding_site_locations() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "ali MaybeText: opt[str];\nvar label = nil;\n",
    )]);
    let error = errors
        .iter()
        .find(|error| {
            error
                .message()
                .contains("nil literals require an expected opt[...] or err[...] shell type in V1")
        })
        .expect("Expected nil expected-shell diagnostic");
    let location = error
        .diagnostic_location()
        .expect("Expected nil diagnostic location");

    assert_eq!(location.line, 2);
    assert_eq!(location.column, 1);
    assert_eq!(location.length, Some(3));
    assert!(
        location
            .file
            .as_deref()
            .is_some_and(|file| file.ends_with("/main.fol")),
        "Expected nil diagnostic file to point at main.fol, got: {location:?}"
    );
}

#[test]
fn reopened_v1_unwrap_diagnostics_keep_receiver_site_locations() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n    var count: int = 1;\n    return count!;\n};\n",
    )]);
    let error = errors
        .iter()
        .find(|error| {
            error
                .message()
                .contains("unwrap requires an opt[...] or err[...] shell with a value type in V1")
        })
        .expect("Expected unwrap shell diagnostic");
    let location = error
        .diagnostic_location()
        .expect("Expected unwrap diagnostic location");

    assert_eq!(location.line, 3);
    assert_eq!(location.column, 12);
    assert_eq!(location.length, Some(5));
    assert!(
        location
            .file
            .as_deref()
            .is_some_and(|file| file.ends_with("/main.fol")),
        "Expected unwrap diagnostic file to point at main.fol, got: {location:?}"
    );
}

#[test]
fn nil_typing_rejects_missing_expected_shell_contexts() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var label = nil;\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("nil literals require an expected opt[...] or err[...] shell type in V1")
        }),
        "Expected the nil expected-shell diagnostic, got: {errors:?}"
    );
}

#[test]
fn shell_typing_accepts_postfix_unwrap_for_optional_and_typed_error_values() {
    let _typed = typecheck_fixture_folder(&[(
        "main.fol",
        "ali MaybeText: opt[str];\nali Failure: err[str];\nfun[] take_text(value: MaybeText): str = {\n    return value!;\n};\nfun[] take_error(value: Failure): str = {\n    return value!;\n};\n",
    )]);
}

#[test]
fn shell_typing_rejects_postfix_unwrap_for_recoverable_routine_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int / str = {\n\
             return load()!;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("postfix '!' unwrap applies to opt[...] and err[...] shell values, not to routine call results with '/ ErrorType' in V1")
        }),
        "Expected the recoverable-call unwrap boundary diagnostic, got: {errors:?}"
    );
}

#[test]
fn shell_typing_rejects_postfix_unwrap_for_bare_error_shells() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "ali BareFailure: err[];\nfun[] main(value: BareFailure): str = {\n    return value!;\n};\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("unwrap requires an opt[...] or err[...] shell with a value type in V1")
        }),
        "Expected the unwrap shell diagnostic, got: {errors:?}"
    );
}

#[test]
fn shell_typing_accepts_nil_in_return_and_call_argument_contexts() {
    let _typed = typecheck_fixture_folder(&[(
        "main.fol",
        "ali MaybeText: opt[str];\nfun[] echo(value: MaybeText): MaybeText = {\n    return value;\n};\nfun[] make(): MaybeText = {\n    return nil;\n};\nfun[] main(): MaybeText = {\n    return echo(nil);\n};\n",
    )]);
}

#[test]
fn shell_typing_accepts_postfix_unwrap_in_binding_and_return_contexts() {
    let _typed = typecheck_fixture_folder(&[(
        "main.fol",
        "ali MaybeText: opt[str];\nfun[] main(value: MaybeText): str = {\n    var label: str = value!;\n    return value!;\n};\n",
    )]);
}

#[test]
fn shell_typing_rejects_postfix_unwrap_for_non_shell_targets() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(value: int): int = {\n    return value!;\n};\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("unwrap requires an opt[...] or err[...] shell with a value type in V1")
        }),
        "Expected the unwrap non-shell diagnostic, got: {errors:?}"
    );
}

#[test]
fn declaration_signature_lowering_resolves_qualified_named_types() {
    let typed = typecheck_fixture_folder(&[
        ("util/types.fol", "ali Count: int;\n"),
        ("main.fol", "var total: util::Count = 1;\n"),
    ]);

    let (count_id, _count) = find_typed_symbol(&typed, "Count", SymbolKind::Alias);
    let (_total_id, total) = find_typed_symbol(&typed, "total", SymbolKind::ValueBinding);

    assert_eq!(
        typed.type_table().get(total.declared_type.expect("binding should lower")),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
        })
    );
}

#[test]
fn declaration_signature_lowering_checks_local_bindings() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] demo(): int = {\n\
             var local: int = 1;\n\
             return local;\n\
         };\n",
    )]);

    let (_local_id, local) = find_typed_symbol(&typed, "local", SymbolKind::ValueBinding);

    assert_eq!(
        typed.type_table().get(local.declared_type.expect("local binding should lower")),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn declaration_signature_lowering_checks_nested_routine_signatures() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] demo(seed: int): int = {\n\
             fun[] helper(item: str): int = {\n\
                 return seed;\n\
             };\n\
             return seed;\n\
         };\n",
    )]);

    let (_helper_id, helper) = find_typed_symbol(&typed, "helper", SymbolKind::Routine);
    let helper_type = typed
        .type_table()
        .get(helper.declared_type.expect("nested routine should lower"))
        .expect("nested routine type should exist");
    let CheckedType::Routine(helper_type) = helper_type else {
        panic!("nested routine should lower to a routine type");
    };

    assert_eq!(helper_type.error_type, None);
    assert_eq!(
        typed.type_table().get(helper_type.params[0]),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
    assert_eq!(
        typed
            .type_table()
            .get(helper_type.return_type.expect("nested routine return should lower")),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn declaration_signature_lowering_keeps_alias_target_types_exact() {
    let typed = typecheck_fixture_folder(&[
        ("types.fol", "ali PathLabel: str;\n"),
        ("main.fol", "var current: PathLabel = \"main\";\n"),
    ]);

    let (alias_id, alias) = find_typed_symbol(&typed, "PathLabel", SymbolKind::Alias);
    let (_current_id, current) = find_typed_symbol(&typed, "current", SymbolKind::ValueBinding);

    assert_eq!(
        typed.type_table().get(alias.declared_type.expect("alias should lower")),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
    assert_eq!(
        typed.type_table().get(current.declared_type.expect("binding should lower")),
        Some(&CheckedType::Declared {
            symbol: alias_id,
            name: "PathLabel".to_string(),
            kind: DeclaredTypeKind::Alias,
        })
    );
}

#[test]
fn declaration_signature_lowering_records_entry_variant_payload_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Token: ent = {\n\
             var Word: str = \"word\";\n\
             con Number: int = 1;\n\
         };\n",
    )]);

    let (_token_id, token) = find_typed_symbol(&typed, "Token", SymbolKind::Type);
    let CheckedType::Entry { variants } = typed
        .type_table()
        .get(token.declared_type.expect("entry type should lower"))
        .expect("entry type facts should exist")
    else {
        panic!("entry declaration should lower to an entry semantic type");
    };

    assert_eq!(variants.get("Word"), Some(&Some(typed.builtin_types().str_)));
    assert_eq!(variants.get("Number"), Some(&Some(typed.builtin_types().int)));
}

#[test]
fn declaration_signature_lowering_allows_forward_cross_file_alias_references() {
    let typed = typecheck_fixture_folder(&[
        ("00_main.fol", "var total: Count = 1;\n"),
        ("10_types.fol", "ali Count: int;\n"),
    ]);

    let (count_id, _count) = find_typed_symbol(&typed, "Count", SymbolKind::Alias);
    let (_total_id, total) = find_typed_symbol(&typed, "total", SymbolKind::ValueBinding);

    assert_eq!(
        typed.type_table().get(total.declared_type.expect("binding should lower")),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
        })
    );
}

#[test]
fn declaration_signature_lowering_allows_cross_file_named_type_references_in_routine_signatures() {
    let typed = typecheck_fixture_folder(&[
        (
            "00_main.fol",
            "fun[] load(item: model::User): model::User = {\n\
                 return item;\n\
             };\n",
        ),
        ("model/user.fol", "typ User: rec = {\n    name: str\n};\n"),
    ]);

    let (user_id, _user) = find_typed_symbol(&typed, "User", SymbolKind::Type);
    let (_load_id, load) = find_typed_symbol(&typed, "load", SymbolKind::Routine);
    let CheckedType::Routine(load_type) = typed
        .type_table()
        .get(load.declared_type.expect("routine should lower"))
        .expect("routine type should exist")
    else {
        panic!("routine declaration should lower to a routine semantic type");
    };

    assert_eq!(
        typed.type_table().get(load_type.params[0]),
        Some(&CheckedType::Declared {
            symbol: user_id,
            name: "User".to_string(),
            kind: DeclaredTypeKind::Type,
        })
    );
    assert_eq!(
        typed
            .type_table()
            .get(load_type.return_type.expect("routine return type should lower")),
        Some(&CheckedType::Declared {
            symbol: user_id,
            name: "User".to_string(),
            kind: DeclaredTypeKind::Type,
        })
    );
}

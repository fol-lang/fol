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
        }),
        "Expected underconstrained generic returns to fail explicitly, got: {errors:?}"
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
        }),
        "Expected contextual return typing to stay outside M1 inference, got: {errors:?}"
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

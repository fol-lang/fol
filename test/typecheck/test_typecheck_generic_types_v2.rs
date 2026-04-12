use super::*;

#[test]
fn generic_record_instantiations_typecheck_with_field_access() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Box(T): rec = {\n\
             value: T\n\
         };\n\
         fun[] main(value: Box[int]): int = {\n\
             return value.value;\n\
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
fn generic_alias_instantiations_typecheck_through_nested_generic_uses() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Box(T): rec = {\n\
             value: T\n\
         };\n\
         typ MaybeBox(T): Box[T];\n\
         fun[] main(value: MaybeBox[int]): int = {\n\
             return value.value;\n\
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
fn imported_generic_type_instantiations_typecheck_with_field_access() {
    let root = unique_temp_dir("generic_type_workspace_ok");
    create_dir_all(root.join("shared")).expect("shared fixture directory should exist");
    create_dir_all(root.join("app")).expect("app fixture directory should exist");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                "typ[exp] Box(T): rec = {\n\
                     value: T\n\
                 };\n",
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\n\
                 fun[] main(value: shared::Box[int]): int = {\n\
                     return value.value;\n\
                 };\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("imported generic type fixture should typecheck");

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
fn generic_type_instantiations_reject_arity_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Box(T): rec = {\n\
             value: T\n\
         };\n\
         fun[] main(value: Box[int, str]): int = {\n\
             return 0;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::InvalidInput
            && error
                .message()
                .contains("generic type 'Box' expects 1 type argument(s) but got 2")
    }), "Expected generic type arity mismatch to fail locally, got: {errors:?}");
}

#[test]
fn generic_type_instantiations_reject_recursive_self_reference_boundary() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Node(T): rec = {\n\
             next: Node[T]\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("generic recursive type instantiation is not yet supported")
    }), "Expected recursive generic type instantiation boundary to stay explicit, got: {errors:?}");
}

#[test]
fn generic_type_instantiations_accept_protocol_constraints() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             value: int\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 1;\n\
         };\n\
         typ Box(T: geo): rec = {\n\
             value: T\n\
         };\n\
         fun[] main(value: Box[Rect]): int = {\n\
             return value.value.area();\n\
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
fn generic_type_instantiations_reject_nonconforming_protocol_constraints() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Plain(): rec = {\n\
             value: int\n\
         };\n\
         typ Box(T: geo): rec = {\n\
             value: T\n\
         };\n\
         fun[] main(value: Box[Plain]): int = {\n\
             return 0;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("requires type 'Plain' to satisfy standard 'geo'")
            && error
                .message()
                .contains("implement the required routines")
    }), "Expected constrained generic type instantiations to reject nonconforming types, got: {errors:?}");
}

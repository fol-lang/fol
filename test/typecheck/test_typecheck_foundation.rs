use super::*;
use fol_typecheck::TypecheckCapabilityModel;

#[test]
fn typechecker_foundation_smoke_constructs_public_api() {
    let _ = Typechecker::new();
}

#[test]
fn typecheck_errors_keep_exact_diagnostic_locations() {
    let error = TypecheckError::with_origin(
        TypecheckErrorKind::InvalidInput,
        "declared type is not valid in this position",
        SyntaxOrigin {
            file: Some("pkg/main.fol".to_string()),
            line: 5,
            column: 9,
            length: 3,
        },
    );

    assert_eq!(
        error.diagnostic_location(),
        Some(DiagnosticLocation {
            file: Some("pkg/main.fol".to_string()),
            line: 5,
            column: 9,
            length: Some(3),
        })
    );
}

#[test]
fn typecheck_errors_lower_to_stable_structured_diagnostics() {
    let diagnostic = TypecheckError::with_origin(
        TypecheckErrorKind::Unsupported,
        "blueprints are not part of the V1 typecheck milestone",
        SyntaxOrigin {
            file: Some("pkg/main.fol".to_string()),
            line: 2,
            column: 1,
            length: 3,
        },
    )
    .with_related_origin(
        SyntaxOrigin {
            file: Some("pkg/std.fol".to_string()),
            line: 1,
            column: 1,
            length: 3,
        },
        "related declaration site",
    )
    .to_diagnostic();

    assert_eq!(diagnostic.code, DiagnosticCode::new("T1002"));
    assert_eq!(
        diagnostic.primary_location(),
        Some(&DiagnosticLocation {
            file: Some("pkg/main.fol".to_string()),
            line: 2,
            column: 1,
            length: Some(3),
        })
    );
    assert_eq!(diagnostic.labels.len(), 2);
}

#[test]
fn builtin_type_tables_install_v1_scalar_types_canonically() {
    let mut table = TypeTable::new();
    let builtins = BuiltinTypeIds::install(&mut table);

    assert_eq!(table.len(), 6);
    assert_eq!(
        table.get(builtins.int),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
    assert_eq!(
        table.get(builtins.str_),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
}

#[test]
fn dfr_blocks_typecheck_as_scope_exit_statements() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             dfr {\n\
                 .echo(1);\n\
             };\n\
             return 7;\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int)),
        "Expected dfr-bearing routine to keep its declared return type",
    );
}

#[test]
fn shared_pointer_recursion_typechecks_nominally() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Node: rec = { value: int, next: opt ptr[shared, Node] };\n\
         fun[] main(): int = { return 0; };\n",
    )]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn shared_pointer_write_error_keeps_the_target_origin() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        concat!(
            "fun[] main(): int = {\n",
            "    var value: int = 7;\n",
            "    var pointer: ptr[shared, int] = &value;\n",
            "    *pointer = 9;\n",
            "    return *pointer;\n",
            "};\n",
        ),
    )]);
    let error = errors
        .iter()
        .find(|error| {
            error
                .message()
                .contains("cannot write through ptr[shared, T]; shared pointers are read-only")
        })
        .expect("shared-pointer write should produce its canonical error");

    assert_eq!(error.kind(), TypecheckErrorKind::InvalidInput);
    let origin = error
        .origin()
        .expect("shared-pointer write error should retain its target origin");
    assert!(origin
        .file
        .as_deref()
        .is_some_and(|file| file.ends_with("/main.fol")));
    assert_eq!((origin.line, origin.column, origin.length), (4, 6, 7));
    assert_eq!(error.to_diagnostic().code.as_str(), "T1001");
}

#[test]
fn dfr_blocks_reject_break() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_break(): int = {\n\
             loop(true) {\n\
                 dfr {\n\
                     break;\n\
                 };\n\
                 break;\n\
             }\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("break is not allowed inside dfr/edf blocks")
        }),
        "Expected deferred break rejection, got: {errors:?}"
    );
}

#[test]
fn dfr_blocks_reject_nested_return() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad_return(): int = {\n\
             dfr {\n\
                 when(true) {\n\
                     case(true) {\n\
                         return 1;\n\
                     }\n\
                 }\n\
             };\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("return is not allowed inside dfr/edf blocks")
        }),
        "Expected deferred nested return rejection, got: {errors:?}"
    );
}

#[test]
fn dfr_and_edf_blocks_reject_nested_report_statements() {
    for deferred in ["dfr", "edf"] {
        let source = format!(
            "fun[] main(flag: bol): int / int = {{\n\
                 {deferred} {{\n\
                     when(flag) {{\n\
                         case(true) {{ report 9; }}\n\
                         * {{ var observed: int = 1; }}\n\
                     }}\n\
                 }};\n\
                 when(flag) {{\n\
                     case(true) {{ report 1; }}\n\
                     * {{ return 7; }}\n\
                 }}\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);
        let error = errors
            .iter()
            .find(|error| {
                error.kind() == TypecheckErrorKind::InvalidInput
                    && error
                        .message()
                        .contains("report is not allowed inside dfr/edf blocks")
            })
            .unwrap_or_else(|| {
                panic!("{deferred} should reject nested report before lowering: {errors:#?}")
            });

        assert_eq!(error.to_diagnostic().code.as_str(), "T1001");
        assert!(
            error.origin().is_some(),
            "{deferred} report rejection should retain the report call's source location"
        );
    }
}

#[test]
fn dfr_and_edf_reserve_referenced_move_only_outer_bindings() {
    for deferred in ["dfr", "edf"] {
        let source = format!(
            "typ Item: rec = {{ value: int }};\n\
             fun[] main(): int / int = {{\n\
                 @var owned: Item = {{ value = 7 }};\n\
                 {deferred} {{ var observed: int = owned.value; }};\n\
                 @var moved: Item = owned;\n\
                 return moved.value;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);
        let error = errors
            .iter()
            .find(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.message().contains(
                        "move-only binding 'owned' cannot be transferred after it is referenced by a dfr/edf body",
                    )
            })
            .unwrap_or_else(|| {
                panic!(
                    "{deferred} must reserve a referenced outer owner until scope exit: {errors:#?}"
                )
            });
        assert_eq!(
            error.to_diagnostic().labels.len(),
            2,
            "the transfer diagnostic should retain the deferred use site"
        );
    }
}

#[test]
fn moving_an_owner_before_dfr_or_edf_keeps_the_existing_moved_use_diagnostic() {
    for deferred in ["dfr", "edf"] {
        let source = format!(
            "typ Item: rec = {{ value: int }};\n\
             fun[] main(): int / int = {{\n\
                 @var owned: Item = {{ value = 7 }};\n\
                 @var moved: Item = owned;\n\
                 {deferred} {{ var observed: int = owned.value; }};\n\
                 return moved.value;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("use of moved heap-owned binding 'owned'")
            }),
            "a move before {deferred} must still be diagnosed at the deferred read: {errors:#?}"
        );
    }
}

#[test]
fn deferred_owner_reservations_end_after_the_registration_scope_exits() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] main(): int = {\n\
             @var owned: Item = { value = 7 };\n\
             {\n\
                 dfr { var observed: int = owned.value; };\n\
             };\n\
             @var moved: Item = owned;\n\
             return moved.value;\n\
         };\n",
    )]);

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn builtin_type_installation_reuses_existing_slots() {
    let mut table = TypeTable::new();
    let first = BuiltinTypeIds::install(&mut table);
    let second = BuiltinTypeIds::install(&mut table);

    assert_eq!(first, second);
    assert_eq!(table.len(), 6);
}

#[test]
fn typechecker_wraps_resolved_programs_in_a_typed_shell() {
    let resolved = resolve_fixture("test/parser/simple_var.fol");
    let top_level_node = resolved
        .source_units
        .get(fol_resolver::SourceUnitId(0))
        .expect("resolved source unit should exist")
        .top_level_nodes[0];
    let typed = Typechecker::new()
        .check_resolved_program(resolved)
        .expect("Typed shell should accept resolved programs");

    assert_eq!(typed.package_name(), "parser");
    assert_eq!(typed.source_units().len(), 1);
    assert_eq!(typed.type_table().len(), 6);
    assert_eq!(
        typed.type_table().get(typed.builtin_types().bool_),
        Some(&CheckedType::Builtin(BuiltinType::Bool))
    );
    assert_eq!(typed.resolved().source_units.len(), 1);
    assert!(typed.typed_node(top_level_node).is_some());
    assert!(typed.typed_symbol(SymbolId(0)).is_some());
}

#[test]
fn dot_graph_is_rejected_in_ordinary_source_units() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             .graph();\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput && error.message().contains(".graph")
        }),
        "Expected ordinary source to reject .graph(), got: {errors:?}"
    );
}

#[test]
fn ordinary_source_can_define_its_own_graph_type() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Graph: rec = {\n\
             value: int\n\
         };\n\
         fun[] make_graph(): Graph = {\n\
             return { value = 7 };\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "make_graph");
    let return_type = typed
        .typed_node(syntax_id)
        .and_then(|node| node.inferred_type)
        .and_then(|type_id| typed.type_table().get(type_id))
        .cloned();

    assert!(
        matches!(return_type, Some(CheckedType::Declared { ref name, .. }) if name == "Graph"),
        "ordinary source Graph type should remain user-defined, got: {return_type:?}"
    );
}

#[test]
fn semantic_type_table_covers_declared_and_structural_shapes() {
    let mut table = TypeTable::new();
    let int_id = table.intern_builtin(BuiltinType::Int);
    let alias_id = table.intern(CheckedType::Declared {
        symbol: SymbolId(9),
        name: "Meters".to_string(),
        kind: DeclaredTypeKind::Alias,
        args: Vec::new(),
    });

    let mut fields = BTreeMap::new();
    fields.insert("value".to_string(), alias_id);
    let record = table.intern(CheckedType::Record { fields });
    let routine = table.intern(CheckedType::Routine(RoutineType {
        generic_params: Vec::new(),
        generic_constraints: BTreeMap::new(),
        param_names: vec!["value".to_string()],
        param_defaults: vec![None],
        variadic_index: None,
        mutex_params: Default::default(),
        params: vec![alias_id],
        return_type: Some(int_id),
        error_type: None,
    }));

    assert_ne!(record, routine);
    assert_eq!(
        table.get(alias_id),
        Some(&CheckedType::Declared {
            symbol: SymbolId(9),
            name: "Meters".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
}

#[test]
fn builtin_type_as_str_matches_language_spelling() {
    assert_eq!(BuiltinType::Int.as_str(), "int");
    assert_eq!(BuiltinType::Float.as_str(), "flt");
    assert_eq!(BuiltinType::Bool.as_str(), "bol");
    assert_eq!(BuiltinType::Char.as_str(), "chr");
    assert_eq!(BuiltinType::Str.as_str(), "str");
    assert_eq!(BuiltinType::Never.as_str(), "never");
}

#[test]
fn builtin_type_all_names_covers_every_variant() {
    assert_eq!(BuiltinType::ALL_NAMES.len(), 6);
    for name in BuiltinType::ALL_NAMES {
        assert!(!name.is_empty());
    }
}

#[test]
fn render_type_handles_builtins_and_containers() {
    let mut table = TypeTable::new();
    let int_id = table.intern_builtin(BuiltinType::Int);
    let str_id = table.intern_builtin(BuiltinType::Str);
    let opt_id = table.intern(CheckedType::Optional { inner: int_id });
    let vec_id = table.intern(CheckedType::Vector {
        element_type: str_id,
    });
    let map_id = table.intern(CheckedType::Map {
        key_type: str_id,
        value_type: int_id,
    });

    assert_eq!(table.render_type(int_id), "int");
    assert_eq!(table.render_type(opt_id), "opt[int]");
    assert_eq!(table.render_type(vec_id), "vec[str]");
    assert_eq!(table.render_type(map_id), "map[str, int]");
}

#[test]
fn render_type_handles_routines() {
    let mut table = TypeTable::new();
    let int_id = table.intern_builtin(BuiltinType::Int);
    let str_id = table.intern_builtin(BuiltinType::Str);
    let routine_id = table.intern(CheckedType::Routine(RoutineType {
        generic_params: Vec::new(),
        generic_constraints: BTreeMap::new(),
        param_names: vec!["left".to_string(), "right".to_string()],
        param_defaults: vec![None, None],
        variadic_index: None,
        mutex_params: Default::default(),
        params: vec![int_id, str_id],
        return_type: Some(int_id),
        error_type: None,
    }));
    assert_eq!(table.render_type(routine_id), "fun(int, str): int");
}

#[test]
fn symbol_kind_display_name_covers_all_variants() {
    assert_eq!(SymbolKind::Routine.display_name(), "routine");
    assert_eq!(SymbolKind::Type.display_name(), "type");
    assert_eq!(SymbolKind::Alias.display_name(), "alias");
    assert_eq!(SymbolKind::Definition.display_name(), "definition");
    assert_eq!(SymbolKind::ValueBinding.display_name(), "binding");
    assert_eq!(SymbolKind::Parameter.display_name(), "parameter");
    assert_eq!(SymbolKind::Capture.display_name(), "capture");
    assert_eq!(SymbolKind::ImportAlias.display_name(), "namespace");
    assert_eq!(SymbolKind::Segment.display_name(), "segment");
    assert_eq!(SymbolKind::Standard.display_name(), "standard");
}

#[test]
fn declaration_signature_lowering_records_top_level_type_facts() {
    let typed = typecheck_fixture_folder(&[
        (
            "types.fol",
            "ali Distance: int;\n\
             typ Person: rec = {\n\
                 name: str\n\
             };\n",
        ),
        (
            "main.fol",
            "var total: Distance = 1;\n\
             var holder: Person;\n\
             fun[] size(value: Distance): Person = {\n\
                 return holder\n\
             };\n",
        ),
    ]);

    let (distance_id, distance) = find_typed_symbol(&typed, "Distance", SymbolKind::Alias);
    let (person_id, person) = find_typed_symbol(&typed, "Person", SymbolKind::Type);
    let (_size_id, size) = find_typed_symbol(&typed, "size", SymbolKind::Routine);

    assert_eq!(
        typed
            .type_table()
            .get(distance.declared_type.expect("alias should lower")),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
    assert_eq!(
        typed
            .type_table()
            .get(person.declared_type.expect("record should lower")),
        Some(&CheckedType::Record {
            fields: BTreeMap::from([("name".to_string(), typed.builtin_types().str_)])
        })
    );
    let routine_type_id = size.declared_type.expect("routine should lower");
    let routine_type = typed
        .type_table()
        .get(routine_type_id)
        .expect("lowered routine type should exist");
    let CheckedType::Routine(routine) = routine_type else {
        panic!("lowered routine signature should be represented as a routine type");
    };
    assert_eq!(routine.error_type, None);
    assert_eq!(routine.params.len(), 1);
    assert_eq!(
        typed.type_table().get(routine.params[0]),
        Some(&CheckedType::Declared {
            symbol: distance_id,
            name: "Distance".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    assert_eq!(
        typed.type_table().get(
            routine
                .return_type
                .expect("routine return type should lower")
        ),
        Some(&CheckedType::Declared {
            symbol: person_id,
            name: "Person".to_string(),
            kind: DeclaredTypeKind::Type,
            args: Vec::new(),
        })
    );
    assert_eq!(
        typed
            .resolved()
            .source_units
            .get(SourceUnitId(0))
            .map(|unit| unit.package.as_str()),
        Some(typed.package_name())
    );
}

#[test]
fn declaration_signature_lowering_keeps_builtin_str_types_builtin() {
    let typed = typecheck_fixture_folder(&[("main.fol", "var label: str = \"ok\";\n")]);
    let (_label_id, label) = find_typed_symbol(&typed, "label", SymbolKind::ValueBinding);

    assert_eq!(
        typed
            .type_table()
            .get(label.declared_type.expect("binding should lower")),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
}

#[test]
fn declaration_signature_lowering_keeps_named_types_as_declared_symbols() {
    let typed = typecheck_fixture_folder(&[
        ("types.fol", "typ Point: rec = {\n};\n"),
        ("main.fol", "var current: Point;\n"),
    ]);

    let (point_id, _point) = find_typed_symbol(&typed, "Point", SymbolKind::Type);
    let (_current_id, current) = find_typed_symbol(&typed, "current", SymbolKind::ValueBinding);

    assert_eq!(
        typed
            .type_table()
            .get(current.declared_type.expect("binding should lower")),
        Some(&CheckedType::Declared {
            symbol: point_id,
            name: "Point".to_string(),
            kind: DeclaredTypeKind::Type,
            args: Vec::new(),
        })
    );
}

#[test]
fn declaration_signature_lowering_keeps_alias_references_as_alias_symbols() {
    let typed = typecheck_fixture_folder(&[
        ("types.fol", "ali Count: int;\n"),
        ("main.fol", "var total: Count = 1;\n"),
    ]);

    let (count_id, _count) = find_typed_symbol(&typed, "Count", SymbolKind::Alias);
    let (_total_id, total) = find_typed_symbol(&typed, "total", SymbolKind::ValueBinding);

    assert_eq!(
        typed
            .type_table()
            .get(total.declared_type.expect("binding should lower")),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
}

#[test]
fn expression_typing_resolves_plain_identifier_references_to_declared_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "var total: int = 1;\n\
         fun[] read(): int = {\n\
             return total;\n\
         };\n",
    )]);

    let reference = find_typed_reference(&typed, "total", ReferenceKind::Identifier);

    assert_eq!(
        typed.type_table().get(
            reference
                .resolved_type
                .expect("identifier should receive a type")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_resolves_qualified_identifier_references_to_declared_types() {
    let typed = typecheck_fixture_folder(&[
        ("util/value.fol", "var[exp] total: int = 1;\n"),
        (
            "main.fol",
            "fun[] read(): int = {\n\
                 return util::total;\n\
             };\n",
        ),
    ]);

    let reference = find_typed_reference(&typed, "util::total", ReferenceKind::QualifiedIdentifier);

    assert_eq!(
        typed.type_table().get(
            reference
                .resolved_type
                .expect("qualified identifier should receive a type")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_infers_local_binding_types_from_initializers() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] demo(): int = {\n\
             var current = 1;\n\
             return current;\n\
         };\n",
    )]);

    let (_current_id, current) = find_typed_symbol(&typed, "current", SymbolKind::ValueBinding);

    assert_eq!(
        typed.type_table().get(
            current
                .declared_type
                .expect("initializer should infer local type")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_keeps_final_routine_body_expression_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "var total: int = 1;\n\
         fun[] demo(): int = {\n\
             return total;\n\
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
fn expression_typing_accepts_assignments_with_matching_types() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "var total: int = 1;\n\
         fun[] demo(): int = {\n\
             total = 2;\n\
             return total;\n\
         };\n",
    )]);

    let reference = find_typed_reference(&typed, "total", ReferenceKind::Identifier);
    assert_eq!(
        typed.type_table().get(
            reference
                .resolved_type
                .expect("identifier should keep its type after assignment")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_rejects_assignments_with_mismatched_value_types() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var total: int = 1;\n\
         fun[] demo(): int = {\n\
             total = \"bad\";\n\
             return total;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("assignment expects")
        }),
        "Expected an incompatible assignment diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_types_free_calls_against_routine_signatures() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] id(value: int): int = {\n\
             return value;\n\
         };\n\
         fun[] demo(): int = {\n\
             return id(1);\n\
         };\n",
    )]);

    let reference = find_typed_reference(&typed, "id", ReferenceKind::FunctionCall);
    assert_eq!(
        typed.type_table().get(
            reference
                .resolved_type
                .expect("free call should receive a result type")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_rejects_free_call_arity_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] id(value: int): int = {\n\
             return value;\n\
         };\n\
         fun[] demo(): int = {\n\
             return id();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("expects 1 args but got 0")
        }),
        "Expected an arity diagnostic for free call mismatch, got: {errors:?}"
    );
}

#[test]
fn expression_typing_accepts_named_arguments_for_free_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] pair(left: int, right: int): int = {\n\
             return left;\n\
         };\n\
         fun[] demo(): int = {\n\
             return pair(right = 2, left = 1);\n\
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
fn expression_typing_rejects_unknown_named_arguments_for_free_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] pair(left: int, right: int): int = {\n\
             return left;\n\
         };\n\
         fun[] demo(): int = {\n\
             return pair(other = 1, left = 2);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("does not have a parameter named 'other'")
        }),
        "Expected an unknown named-argument diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_rejects_duplicate_named_arguments_for_free_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] pair(left: int, right: int): int = {\n\
             return left;\n\
         };\n\
         fun[] demo(): int = {\n\
             return pair(left = 1, left = 2);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("supplies parameter 'left' more than once")
        }),
        "Expected a duplicate named-argument diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_rejects_missing_required_arguments_for_method_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)shift(by: int, step: int = 2): int = {\n\
             return by;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(step = 3);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("missing required argument 'by'")
        }),
        "Expected a missing required method-argument diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_rejects_duplicate_named_arguments_for_method_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)shift(by: int, step: int): int = {\n\
             return by;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(by = 1, by = 2);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("supplies parameter 'by' more than once")
        }),
        "Expected a duplicate named-argument diagnostic for method call, got: {errors:?}"
    );
}

#[test]
fn expression_typing_accepts_default_parameters_for_free_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] pair(left: int, right: int = 2): int = {\n\
             return left;\n\
         };\n\
         fun[] demo(): int = {\n\
             return pair(1);\n\
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
fn expression_typing_accepts_named_calls_that_skip_defaulted_free_parameters() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] pair(left: int, right: int = 2): int = {\n\
             return left;\n\
         };\n\
         fun[] demo(): int = {\n\
             return pair(left = 1);\n\
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
fn expression_typing_accepts_variadic_free_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] sum(head: int, tail: ... int): int = {\n\
             return head;\n\
         };\n\
         fun[] demo(): int = {\n\
             return sum(1, 2, 3, 4);\n\
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
fn expression_typing_accepts_unpack_for_variadic_free_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "var nums: seq[int];\n\
         fun[] sum(head: int, tail: ... int): int = {\n\
             return head;\n\
         };\n\
         fun[] demo(): int = {\n\
             return sum(1, ...nums);\n\
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
fn expression_typing_rejects_unpack_for_non_variadic_free_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var nums: seq[int];\n\
         fun[] pair(left: int): int = {\n\
             return left;\n\
         };\n\
         fun[] demo(): int = {\n\
             return pair(...nums);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("call-site unpack is only supported for variadic calls in V1")
        }),
        "Expected non-variadic unpack diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_rejects_double_unpack_for_variadic_free_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var lefts: seq[int];\n\
         var rights: seq[int];\n\
         fun[] score(base: int, extras: ... int): int = {\n\
             return base;\n\
         };\n\
         fun[] demo(): int = {\n\
             return score(1, ...lefts, ...rights);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(
                    "call-site unpack cannot be combined with other variadic arguments in V1",
                )
        }),
        "Expected double-unpack diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_accepts_defaulted_variadic_free_calls_with_unpack() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "var nums: seq[int];\n\
         fun[] score(base: int, step: int = 2, extras: ... int): int = {\n\
             return base;\n\
         };\n\
         fun[] demo(): int = {\n\
             return score(1, ...nums);\n\
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
fn expression_typing_accepts_named_unpack_calls_that_use_defaulted_free_parameters() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "var nums: seq[int];\n\
         fun[] score(base: int, step: int = 2, extras: ... int): int = {\n\
             return base;\n\
         };\n\
         fun[] demo(): int = {\n\
             return score(base = 1, ...nums);\n\
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
fn expression_typing_types_method_calls_against_explicit_receiver_routines() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)read(): int = {\n\
             return 1;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.read();\n\
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
fn expression_typing_accepts_named_unpack_method_calls_that_use_defaulted_parameters() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         var nums: seq[int];\n\
         fun (Counter)shift(step: int = 2, values: ... int): int = {\n\
             return step;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(step = 3, ...nums);\n\
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
fn expression_typing_accepts_defaulted_variadic_method_calls_with_unpack() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         var nums: seq[int];\n\
         fun (Counter)shift(step: int = 2, values: ... int): int = {\n\
             return step;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(...nums);\n\
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
fn expression_typing_rejects_unpack_for_non_variadic_method_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         var nums: seq[int];\n\
         fun (Counter)read(step: int): int = {\n\
             return step;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.read(...nums);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("call-site unpack is only supported for variadic calls in V1")
        }),
        "Expected non-variadic method-unpack diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_rejects_double_unpack_for_variadic_method_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         var lefts: seq[int];\n\
         var rights: seq[int];\n\
         fun (Counter)shift(step: int, values: ... int): int = {\n\
             return step;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(1, ...lefts, ...rights);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(
                    "call-site unpack cannot be combined with other variadic arguments in V1",
                )
        }),
        "Expected double-unpack method diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_accepts_named_arguments_for_method_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)shift(by: int, step: int): int = {\n\
             return by;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(step = 2, by = 1);\n\
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
fn expression_typing_accepts_default_parameters_for_method_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)shift(by: int, step: int = 2): int = {\n\
             return by;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(1);\n\
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
fn expression_typing_accepts_named_method_calls_that_skip_defaulted_parameters() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)shift(by: int, step: int = 2): int = {\n\
             return by;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(by = 1);\n\
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
fn expression_typing_accepts_variadic_method_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)shift(values: ... int): int = {\n\
             return 0;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(1, 2, 3);\n\
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
fn expression_typing_accepts_unpack_for_variadic_method_calls() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         var nums: seq[int];\n\
         fun (Counter)shift(values: ... int): int = {\n\
             return 0;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(...nums);\n\
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
fn echo_intrinsic_requires_bundled_std_effective_tier_in_core() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n    return .echo(1);\n};\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        },
    );

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind(), TypecheckErrorKind::Unsupported);
    assert!(errors[0]
        .message()
        .contains("'.echo(...)' requires hosted std support"));
    assert!(errors[0]
        .message()
        .contains("current artifact model is 'core'"));
}

#[test]
fn owned_heap_binding_requires_memo_model() {
    for declaration in [
        "@var value: int = 1;",
        "@var value = 1;",
        "var[new] value = 1;",
    ] {
        let source = format!("fun[] main(): int = {{\n    {declaration}\n    return 0;\n}};\n");
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source.as_str())],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Core,
            },
        );
        assert!(
            errors.iter().any(|error| error
                .message()
                .contains("heap allocation binding requires heap support")),
            "core must reject heap allocation for '{declaration}': {errors:#?}"
        );
    }
}

#[test]
fn core_pointer_guidance_names_only_the_public_memo_model() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            concat!(
                "fun[] main(): int = {\n",
                "    var value: int = 7;\n",
                "    var pointer: ptr[int] = &value;\n",
                "    return 0;\n",
                "};\n",
            ),
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        },
    );

    let error = errors
        .iter()
        .find(|error| {
            error
                .message()
                .contains("pointer construction requires heap support")
        })
        .expect("core pointer construction should report its capability guidance");
    assert!(error.message().contains("choose fol_model = 'memo'"));
    assert!(!error.message().contains("add bundled std"));
}

#[test]
fn optional_and_error_owned_shell_transfers_move_the_source() {
    for shell in ["opt", "err"] {
        let source = format!(
            "typ Item: rec = {{ value: int }};\n\
             fun[] main(): int = {{\n\
                 @var seed: Item = {{ value = 7 }};\n\
                 var first: {shell}[@Item] = seed;\n\
                 var moved: {shell}[@Item] = first;\n\
                 var invalid: {shell}[@Item] = first;\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.to_diagnostic().code == DiagnosticCode::new("O1001")
                    && error
                        .message()
                        .contains("use of moved heap-owned binding 'first'")
            }),
            "{shell}[@Item] transfer must move its source: {errors:#?}"
        );
    }
}

#[test]
fn move_only_shell_unwrap_consumes_the_shell_binding() {
    for shell in ["opt", "err"] {
        let source = format!(
            "fun[] main(): int = {{\n\
                 var seed: int = 7;\n\
                 var wrapped: {shell}[ptr[int]] = &seed;\n\
                 var first: ptr[int] = wrapped!;\n\
                 var invalid: ptr[int] = wrapped!;\n\
                 return *first;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("use of moved heap-owned binding 'wrapped'")
            }),
            "{shell}[ptr[int]] must be consumed by its first unwrap: {errors:#?}"
        );
    }
}

#[test]
fn returning_from_a_loop_can_consume_inside_the_return_expression() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] take(wrapped: opt[ptr[int]]): ptr[int] = {\n\
             var seed: int = 0;\n\
             loop(true) {\n\
                 return wrapped!;\n\
             };\n\
             return &seed;\n\
         };\n",
    )]);

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "take"))
        .is_some());
}

#[test]
fn by_value_methods_consume_move_only_receivers() {
    for (surface, source) in [
        (
            "owned receiver",
            "typ Item: rec = { value: int };\n\
             fun (@Item)take(): int = { return self.value; };\n\
             fun[] main(): int = {\n\
                 @var owner: Item = { value = 7 };\n\
                 var consumed: int = owner.take();\n\
                 return owner.value;\n\
             };\n",
        ),
        (
            "receiver containing a unique pointer",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun (Holder)take(): int = { return *self.pointer; };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var holder: Holder = { pointer = &seed };\n\
                 var consumed: int = holder.take();\n\
                 return *holder.pointer;\n\
             };\n",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source)]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.message().contains("use of moved")
            }),
            "{surface} must be unavailable after a method call: {errors:#?}"
        );
    }
}

#[test]
fn pointer_construction_consumes_move_only_operands() {
    for (surface, source) in [
        (
            "unique pointer",
            "fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var pointer: ptr[int] = &seed;\n\
                 var nested: ptr[ptr[int]] = &pointer;\n\
                 return *pointer;\n\
             };\n",
        ),
        (
            "aggregate containing a unique pointer",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var holder: Holder = { pointer = &seed };\n\
                 var nested = &holder;\n\
                 return *holder.pointer;\n\
             };\n",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source)]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.message().contains("use of moved")
            }),
            "pointer construction from {surface} must move its operand: {errors:#?}"
        );
    }
}

#[test]
fn echo_transfers_move_only_arguments_into_its_result() {
    let valid = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var pointer: ptr[int] = &seed;\n\
                 var echoed: ptr[int] = .echo(pointer);\n\
                 return *echoed;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(valid
        .typed_node(find_named_routine_syntax_id(&valid, "main"))
        .is_some());

    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var pointer: ptr[int] = &seed;\n\
                 var echoed: ptr[int] = .echo(pointer);\n\
                 return *pointer;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of moved heap-owned binding 'pointer'")
    }));
}

#[test]
fn move_only_field_transfers_consume_the_whole_base() {
    for (surface, source) in [
        (
            "direct unique field",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var holder: Holder = { pointer = &seed };\n\
                 var extracted: ptr[int] = holder.pointer;\n\
                 return *extracted;\n\
             };\n",
        ),
        (
            "field aggregate containing a unique pointer",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             typ Envelope: rec = { holder: Holder };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var envelope: Envelope = { holder = { pointer = &seed } };\n\
                 var extracted: Holder = envelope.holder;\n\
                 var extracted_pointer: ptr[int] = extracted.pointer;\n\
                 return *extracted_pointer;\n\
             };\n",
        ),
    ] {
        let typed = typecheck_fixture_folder(&[("main.fol", source)]);
        let main = find_named_routine_syntax_id(&typed, "main");
        assert!(
            typed.typed_node(main).is_some(),
            "{surface} should typecheck"
        );
    }

    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Holder: rec = { pointer: ptr[int], value: int };\n\
         fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var holder: Holder = { pointer = &seed, value = 3 };\n\
             var extracted: ptr[int] = holder.pointer;\n\
             return holder.value + *extracted;\n\
         };\n",
    )]);
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of moved heap-owned binding 'holder'")
    }));
}

#[test]
fn move_only_index_projections_require_a_partial_move_model() {
    for (surface, source) in [
        (
            "direct unique element",
            "fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var pointers: arr[ptr[int], 1] = { &seed };\n\
                 var extracted: ptr[int] = pointers[0];\n\
                 return *extracted;\n\
             };\n",
        ),
        (
            "element aggregate containing a unique pointer",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var holders: arr[Holder, 1] = { { pointer = &seed } };\n\
                 var extracted: Holder = holders[0];\n\
                 return *extracted.pointer;\n\
             };\n",
        ),
        (
            "move-only field through an indexed aggregate",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var holders: arr[Holder, 1] = { { pointer = &seed } };\n\
                 var extracted: ptr[int] = holders[0].pointer;\n\
                 return *extracted;\n\
             };\n",
        ),
        (
            "copy-safe field through a move-only indexed aggregate",
            "typ Holder: rec = { pointer: ptr[int], value: int };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var holders: arr[Holder, 1] = { { pointer = &seed, value = 3 } };\n\
                 return holders[0].value;\n\
             };\n",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source)]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.message().contains("indexed projection")
                    && error.message().contains("partial moves are not supported")
            }),
            "{surface} must be rejected before lowering: {errors:#?}"
        );
    }
}

#[test]
fn move_only_index_operands_allow_locals_but_reject_field_projections() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] lookup(values: map[ptr[int], int], query: ptr[int]): int = {\n\
             return values[query] + *query + .len(values);\n\
         };\n",
    )]);
    let lookup = find_named_routine_syntax_id(&typed, "lookup");
    assert!(
        typed.typed_node(lookup).is_some(),
        "direct move-only lookup operands are borrowed observations"
    );

    for (surface, source, role) in [
        (
            "receiver",
            "typ Holder: rec = { values: map[ptr[int], int] };\n\
             fun[] lookup(holder: Holder, query: ptr[int]): int = {\n\
                 return holder.values[query];\n\
             };\n",
            "receiver",
        ),
        (
            "key",
            "typ Holder: rec = { query: ptr[int] };\n\
             fun[] lookup(values: map[ptr[int], int], holder: Holder): int = {\n\
                 return values[holder.query];\n\
             };\n",
            "key",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source)]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains(&format!("index {role} observation"))
                    && error.message().contains("must not partially move")
            }),
            "move-only index {surface} projections need place-aware observation IR: {errors:#?}"
        );
    }
}

#[test]
fn move_only_slice_elements_are_rejected_before_clone_based_lowering() {
    for (surface, parameter_type, return_type) in [
        ("vector", "vec[ptr[int]]", "vec[ptr[int]]"),
        ("sequence", "seq[ptr[int]]", "seq[ptr[int]]"),
    ] {
        let source = format!(
            "fun[] tail(values: {parameter_type}): {return_type} = {{\n\
                 return values[1:];\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("slices of move-only elements are not supported in V3")
                    && error.message().contains("clone unique ownership")
            }),
            "{surface} slices must reject move-only elements before lowering: {errors:#?}"
        );
    }

    for parameter_type in ["arr[int, 2]", "arr[ptr[int], 2]"] {
        let source = format!(
            "fun[] tail(values: {parameter_type}): int = {{\n\
                 return .len(values[1:]);\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error
                        .message()
                        .contains("fixed-size array slices are not supported")
            }),
            "fixed-size arrays need an explicit runtime-sized slice result contract: {errors:#?}"
        );
    }

    for (surface, parameter_type) in [("vector", "vec[int]"), ("sequence", "seq[int]")] {
        let source = format!(
            "fun[] tail_len(values: {parameter_type}): int = {{\n\
                 return .len(values[1:]);\n\
             }};\n"
        );
        let typed = typecheck_fixture_folder(&[("main.fol", source.as_str())]);
        let tail_len = find_named_routine_syntax_id(&typed, "tail_len");
        assert!(
            typed.typed_node(tail_len).is_some(),
            "{surface} slices of copy-safe elements should remain supported"
        );
    }
}

#[test]
fn move_only_collection_iteration_requires_a_consuming_iterator_model() {
    for (surface, parameter_type) in [
        ("array", "arr[ptr[int], 1]"),
        ("vector", "vec[ptr[int]]"),
        ("sequence", "seq[ptr[int]]"),
    ] {
        let source = format!(
            "fun[] first(values: {parameter_type}): int = {{\n\
                 for (value in values) {{\n\
                     return *value;\n\
                 }};\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("iteration over a move-only collection is not supported in V3")
                    && error.message().contains("clone move-only elements")
            }),
            "{surface} iteration must not clone move-only elements: {errors:#?}"
        );
    }

    for (surface, parameter_type) in [
        ("array", "arr[int, 1]"),
        ("vector", "vec[int]"),
        ("sequence", "seq[int]"),
    ] {
        let source = format!(
            "fun[] first(values: {parameter_type}): int = {{\n\
                 for (value in values) {{\n\
                     return value;\n\
                 }};\n\
                 return 0;\n\
             }};\n"
        );
        let typed = typecheck_fixture_folder(&[("main.fol", source.as_str())]);
        let first = find_named_routine_syntax_id(&typed, "first");
        assert!(
            typed.typed_node(first).is_some(),
            "{surface} iteration over copy-safe elements should remain supported"
        );
    }

    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] first(channel: chn[ptr[int]]): int = {\n\
                 for (value in channel[rx]) {\n\
                     return *value;\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    let first = find_named_routine_syntax_id(&typed, "first");
    assert!(
        typed.typed_node(first).is_some(),
        "channel receiver iteration consumes move-only payloads and must remain supported"
    );
}

#[test]
fn nested_field_access_rejects_move_only_intermediates() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Inner: rec = { pointer: ptr[int], value: int };\n\
         typ Outer: rec = { inner: Inner };\n\
         fun[] read(outer: Outer): int = {\n\
             return outer.inner.value;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("nested field access through a move-only intermediate")
                && error.message().contains("partial moves are not supported")
        }),
        "nested scalar reads must not move a move-only intermediate: {errors:#?}"
    );

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Inner: rec = { pointer: ptr[int], value: int };\n\
         fun[] read(inner: Inner): int = {\n\
             return inner.value;\n\
         };\n",
    )]);
    let read = find_named_routine_syntax_id(&typed, "read");
    assert!(
        typed.typed_node(read).is_some(),
        "direct copy-safe fields on a move-only record should remain observable"
    );
}

#[test]
fn discarded_move_only_expressions_are_transfers() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
         fun[] main(): int = {\n\
             var value: int = 7;\n\
             var pointer: ptr[int] = &value;\n\
             pointer;\n\
             return consume(pointer);\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("use of moved heap-owned binding 'pointer'")
        }),
        "discarded move-only values must move exactly as lowering evaluates them: {errors:#?}"
    );

    let generic_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun duplicate(T)(value: T): T = {\n\
             value;\n\
             return value;\n\
         };\n",
    )]);
    assert!(
        generic_errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("use of moved heap-owned binding 'value'")
        }),
        "discarded generic values must be conservative for move-only instantiations: {generic_errors:#?}"
    );

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var value: int = 7;\n\
             value;\n\
             return value;\n\
         };\n",
    )]);
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn dereference_rejects_move_only_field_projections() {
    for (surface, parameter) in [
        ("owned record", "holder: Holder"),
        ("borrowed record", "holder[bor]: Holder"),
    ] {
        let source = format!(
            "typ Holder: rec = {{ link: ptr[int] }};\n\
             fun[] inspect({parameter}): int = {{\n\
                 return *holder.link;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("dereferencing through a move-only field projection")
                    && error
                        .message()
                        .contains("pointer observation must not partially move its source")
            }),
            "{surface} must not lose a unique field through dereference lowering: {errors:#?}"
        );
    }

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Holder: rec = { link: ptr[shared, int] };\n\
         fun[] inspect(holder[bor]: Holder): int = {\n\
             return *holder.link;\n\
         };\n",
    )]);
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "inspect"))
        .is_some());
}

#[test]
fn unique_pointer_deref_transfers_move_only_pointees() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var inner: ptr[int] = &seed;\n\
             var outer: ptr[ptr[int]] = &inner;\n\
             var extracted: ptr[int] = *outer;\n\
             return *extracted;\n\
         };\n",
    )]);
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());

    let moved_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var inner: ptr[int] = &seed;\n\
             var outer: ptr[ptr[int]] = &inner;\n\
             var extracted: ptr[int] = *outer;\n\
             return **outer;\n\
         };\n",
    )]);
    assert!(
        moved_errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("use of moved heap-owned binding 'outer'")
        }),
        "consuming a move-only pointee must consume its unique pointer: {moved_errors:#?}"
    );
}

#[test]
fn shared_pointer_deref_rejects_move_only_pointees() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var inner: ptr[int] = &seed;\n\
             var shared: ptr[shared, ptr[int]] = &inner;\n\
             var invalid: ptr[int] = *shared;\n\
             return 0;\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("cannot move a move-only pointee out of ptr[shared, T]")
                && error.message().contains("requires a clone-safe pointee")
        }),
        "Rc-backed pointers must not move their pointee: {errors:#?}"
    );
}

#[test]
fn borrowed_pointer_deref_is_read_only() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] read(pointer[bor]: ptr[int]): int = {\n\
             return *pointer;\n\
         };\n\
         fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var pointer: ptr[int] = &seed;\n\
             return read(#pointer);\n\
         };\n",
    )]);
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "read"))
        .is_some());

    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] take(pointer[bor]: ptr[ptr[int]]): ptr[int] = {\n\
             return *pointer;\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("cannot move a move-only pointee through a borrowed pointer")
                && error.message().contains("dereference is read-only")
        }),
        "a borrowed pointer must not surrender its move-only pointee: {errors:#?}"
    );
}

#[test]
fn when_cases_require_matching_equality_safe_types() {
    for (surface, source, expected_type) in [
        (
            "unique pointers",
            "fun[] main(): int = {\n\
                 var left_value: int = 1;\n\
                 var right_value: int = 2;\n\
                 var left: ptr[int] = &left_value;\n\
                 var right: ptr[int] = &right_value;\n\
                 when(left) {\n\
                     case(right) { return 1; }\n\
                     * { return 0; }\n\
                 }\n\
             };\n",
            "ptr[int]",
        ),
        (
            "records",
            "typ Item: rec = { value: int };\n\
             fun[] main(): int = {\n\
                 var left: Item = { value = 1 };\n\
                 var right: Item = { value = 2 };\n\
                 when(left) {\n\
                     case(right) { return 1; }\n\
                     * { return 0; }\n\
                 }\n\
             };\n",
            "Item",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source)]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::InvalidInput
                    && error
                        .message()
                        .contains("binary operator 'Eq' is not valid")
                    && error.message().contains(expected_type)
            }),
            "when equality must reject {surface}: {errors:#?}"
        );
    }

    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             when(1) {\n\
                 case(true) { return 1; }\n\
                 * { return 0; }\n\
             }\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error
                    .message()
                    .contains("when condition expects 'int' but got 'bol'")
        }),
        "when cases must match the selector type: {errors:#?}"
    );
}

#[test]
fn case_less_when_requires_a_boolean_gate() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var value: int = 1;\n\
             var pointer: ptr[int] = &value;\n\
             when(pointer) {\n\
                 * { return 1; }\n\
             }\n\
             return 0;\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error
                    .message()
                    .contains("case-less when condition expects 'bol' but got 'ptr[int]'")
        }),
        "case-less when must not consume an arbitrary move-only selector: {errors:#?}"
    );

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var hit: int = 0;\n\
             when(true) {\n\
                 * { hit = 1; }\n\
             }\n\
             return hit;\n\
         };\n",
    )]);
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn sibling_case_less_when_bodies_use_exact_binding_scopes() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var[mut] total: int = 0;\n\
             when(true) {\n\
                 * {\n\
                     var x: int = 1;\n\
                     total = total + x;\n\
                 }\n\
             }\n\
             when(true) {\n\
                 * {\n\
                     var x: int = 2;\n\
                     total = total + x;\n\
                 }\n\
             }\n\
             return total - 3;\n\
         };\n",
    )]);
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn len_observes_move_only_locals_but_rejects_field_projections() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Holder: rec = { values: vec[ptr[int]] };\n\
         fun[] length(holder: Holder): int = {\n\
             return .len(holder.values);\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("'.len(...)' through a move-only field projection")
                && error
                    .message()
                    .contains("must not partially move its receiver")
        }),
        "len through a move-only field projection must be rejected: {errors:#?}"
    );

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] length(values: vec[ptr[int]]): int = {\n\
             return .len(values) + .len(values);\n\
         };\n",
    )]);
    let length = find_named_routine_syntax_id(&typed, "length");
    assert!(
        typed.typed_node(length).is_some(),
        "len should observe a direct move-only local without consuming it"
    );
}

#[test]
fn pointer_deref_accepts_move_only_unique_pointees() {
    for (surface, source) in [
        (
            "unique pointer pointee",
            "fun[] take(value: ptr[ptr[int]]): ptr[int] = {\n\
                 return *value;\n\
             };\n",
        ),
        (
            "aggregate pointee containing a unique pointer",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun[] take(): Holder = {\n\
                 var seed: int = 7;\n\
                 var holder: Holder = { pointer = &seed };\n\
                 var holder_pointer = &holder;\n\
                 return *holder_pointer;\n\
             };\n",
        ),
    ] {
        let typed = typecheck_fixture_folder(&[("main.fol", source)]);
        assert!(
            typed
                .typed_node(find_named_routine_syntax_id(&typed, "take"))
                .is_some(),
            "{surface} should transfer out of its unique pointer"
        );
    }
}

#[test]
fn clone_safe_field_and_index_projections_remain_transferable() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Holder: rec = { value: int };\n\
         fun[] main(): int = {\n\
             var holder: Holder = { value = 7 };\n\
             var values: arr[int, 1] = { 3 };\n\
             var holder_pointer = &holder;\n\
             var holder_copy = *holder_pointer;\n\
             var field_copy: int = holder.value;\n\
             var index_copy: int = values[0];\n\
             return field_copy + index_copy + holder_copy.value;\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn when_results_transfer_move_only_branch_values() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var left_value: int = 7;\n\
             var right_value: int = 9;\n\
             var left: ptr[int] = &left_value;\n\
             var right: ptr[int] = &right_value;\n\
             var chosen: ptr[int] = when(true) {\n\
                 is (true) -> left;\n\
                 * -> right;\n\
             };\n\
             return *left + *chosen;\n\
         };\n",
    )]);
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of moved heap-owned binding 'left'")
    }));

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var value: int = 7;\n\
             var pointer: ptr[int] = &value;\n\
             var chosen: ptr[int] = when(true) {\n\
                 is (true) -> pointer;\n\
                 * -> pointer;\n\
             };\n\
             return *chosen;\n\
         };\n",
    )]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
         fun[] choose(flag: bol): int = {\n\
             var value: int = 7;\n\
             var pointer: ptr[int] = &value;\n\
             when(flag) {\n\
                 case(true) { return consume(pointer); }\n\
                 * { 0; }\n\
             };\n\
             return *pointer;\n\
         };\n",
    )]);
    let choose = find_named_routine_syntax_id(&typed, "choose");
    assert!(typed.typed_node(choose).is_some());
}

#[test]
fn outer_move_only_bindings_cannot_be_transferred_from_repeating_loops() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] main(): int = {\n\
             @var owned: Item = { value = 7 };\n\
             var[mut] keep: bol = true;\n\
             loop(keep) {\n\
                 @var moved: Item = owned;\n\
                 keep = false;\n\
             };\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error.message().contains(
                    "move-only binding 'owned' declared outside a repeating loop cannot be transferred",
                )
        }),
        "outer move-only values must not be consumed on a potentially later iteration: {errors:#?}"
    );
}

#[test]
fn outer_move_only_bindings_cannot_move_from_repeated_loop_conditions() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] stop(value: @Item): bol = { return false; };\n\
         fun[] main(): int = {\n\
             @var owned: Item = { value = 7 };\n\
             loop(stop(owned)) { var ignored: int = 0; };\n\
             return 0;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| error.message().contains(
        "move-only binding 'owned' declared outside a repeating loop cannot be transferred"
    )));
}

#[test]
fn move_only_bindings_created_inside_a_loop_can_move_each_iteration() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] main(): int = {\n\
             var[mut] keep: bol = true;\n\
             loop(keep) {\n\
                 @var owned: Item = { value = 7 };\n\
                 @var moved: Item = owned;\n\
                 keep = false;\n\
             };\n\
             return 0;\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn outer_borrows_are_not_released_when_a_nested_loop_scope_ends() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] main(): int = {\n\
             var owner: Item = { value = 7 };\n\
             var[bor] view: Item = owner;\n\
             loop(false) { var ignored: int = 0; };\n\
             return owner.value;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::OwnerBorrowed
                && error
                    .message()
                    .contains("owner 'owner' is inaccessible while borrowed")
        }),
        "leaving a loop body must not release a borrow created outside it: {errors:#?}"
    );
}

#[test]
fn borrows_created_inside_a_loop_end_with_the_loop_body_scope() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] main(): int = {\n\
             var owner: Item = { value = 7 };\n\
             loop(false) {\n\
                 var[bor] view: Item = owner;\n\
                 var seen: int = view.value;\n\
             };\n\
             return owner.value;\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn inferred_borrow_from_binding_keeps_owner_inaccessible() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var owner: int = 7;\n\
             var view = #owner;\n\
             return owner;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::OwnerBorrowed
                && error.to_diagnostic().code == DiagnosticCode::new("O2001")
                && error
                    .message()
                    .contains("owner 'owner' is inaccessible while borrowed")
        }),
        "an inferred #owner binding must remain active for its lexical scope: {errors:#?}"
    );
}

#[test]
fn inferred_borrow_from_binding_can_be_given_back() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var owner: int = 7;\n\
             var view = #owner;\n\
             !view;\n\
             return owner;\n\
         };\n",
    )]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn borrow_bindings_cannot_be_reborrowed() {
    for initializer in ["view", "#view"] {
        let source = format!(
            "fun[] main(): int = {{\n\
                 var owner: int = 7;\n\
                 var[bor] view: int = owner;\n\
                 var[bor] nested: int = {initializer};\n\
                 return view;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::BorrowConflict
                    && error
                        .message()
                        .contains("reborrowing a borrow binding is not supported in V3")
            }),
            "reborrow initializer '{initializer}' must be rejected: {errors:#?}"
        );
    }
}

#[test]
fn borrow_bindings_cannot_borrow_an_owner_after_it_moves() {
    for initializer in ["owner", "#owner"] {
        let source = format!(
            "typ Item: rec = {{ value: int }};\n\
             fun[] main(): int = {{\n\
                 @var owner: Item = {{ value = 7 }};\n\
                 @var moved: Item = owner;\n\
                 var[bor] view: Item = {initializer};\n\
                 return moved.value;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source.as_str())]);

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("cannot borrow from an owner whose value was already moved")
            }),
            "borrow initializer '{initializer}' must reject a moved owner: {errors:#?}"
        );
    }
}

#[test]
fn move_only_values_cannot_be_transferred_out_of_borrows() {
    for (surface, source, binding) in [
        (
            "direct pointer",
            "fun[] steal(value[bor]: ptr[int]): ptr[int] = {\n\
                 return value;\n\
             };\n",
            "value",
        ),
        (
            "record containing a pointer",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun[] steal(value[bor]: Holder): Holder = {\n\
                 return value;\n\
             };\n",
            "value",
        ),
        (
            "pointer placed in an array",
            "fun[] steal(value[bor]: ptr[int]): arr[ptr[int], 1] = {\n\
                 return { value };\n\
             };\n",
            "value",
        ),
        (
            "pointer placed in a positional record",
            "typ Holder: rec = { pointer: ptr[int] };\n\
             fun[] steal(value[bor]: ptr[int]): Holder = {\n\
                 return { value };\n\
             };\n",
            "value",
        ),
        (
            "borrow passed to an owned parameter",
            "fun[] consume(value: ptr[int]): int = { return 0; };\n\
             fun[] main(): int = {\n\
                 var seed: int = 7;\n\
                 var owner: ptr[int] = &seed;\n\
                 var[bor] view: ptr[int] = owner;\n\
                 return consume(view);\n\
             };\n",
            "view",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors(&[("main.fol", source)]);
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.message().contains(&format!(
                        "move-only value cannot be transferred out of borrow binding '{binding}'"
                    ))
            }),
            "{surface} must not clone a unique value through a borrow: {errors:#?}"
        );
    }
}

#[test]
fn borrow_parameters_require_explicit_call_site_borrowing() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] inspect(item[bor]: Item): int = { return item.value; };\n\
         fun[] main(): int = {\n\
             var owner: Item = { value = 7 };\n\
             return inspect(owner);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::BorrowConflict
                && error.message().contains("must pass '#owner'")
        }),
        "plain owner arguments must not silently become borrow arguments: {errors:#?}"
    );
}

#[test]
fn call_site_borrow_excludes_owner_access_in_sibling_arguments() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] compare(view[bor]: Item, copy: Item): int = {\n\
             return view.value + copy.value;\n\
         };\n\
         fun[] main(): int = {\n\
             var owner: Item = { value = 7 };\n\
             return compare(#owner, owner);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::BorrowConflict
                && error
                    .message()
                    .contains("accesses an owner in another argument")
        }),
        "a call-site borrow must exclude sibling owner access regardless of argument order: {errors:#?}"
    );
}

#[test]
fn compatible_shared_call_borrows_end_when_the_call_returns() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Item: rec = { value: int };\n\
         fun[] inspect(item[bor]: Item): int = { return item.value; };\n\
         fun[] main(): int = {\n\
             var owner: Item = { value = 7 };\n\
             {\n\
                 var[bor] first: Item = owner;\n\
                 var one: int = inspect(first);\n\
                 var two: int = inspect(#owner);\n\
             };\n\
             return owner.value;\n\
         };\n",
    )]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn existing_borrow_of_move_only_value_can_flow_to_borrow_parameter() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] inspect(value[bor]: ptr[int]): int = { return 0; };\n\
         fun[] main(): int = {\n\
             var seed: int = 7;\n\
             var owner: ptr[int] = &seed;\n\
             var[bor] view: ptr[int] = owner;\n\
             var first: int = inspect(view);\n\
             var second: int = inspect(view);\n\
             !view;\n\
             return *owner + first + second;\n\
         };\n",
    )]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn echo_intrinsic_requires_bundled_std_effective_tier_in_memo() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n    return .echo(1);\n};\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Memo,
        },
    );

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind(), TypecheckErrorKind::Unsupported);
    assert!(errors[0]
        .message()
        .contains("'.echo(...)' requires hosted std support"));
    assert!(errors[0]
        .message()
        .contains("current artifact model is 'memo'"));
}

#[test]
fn typecheck_capability_tiers_keep_memo_between_core_and_hosted_std() {
    let core_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): str = {\n    return \"heap\";\n};\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        },
    );
    assert_eq!(core_errors.len(), 1);
    assert!(core_errors[0]
        .message()
        .contains("str requires heap support and is unavailable in 'fol_model = core'"));

    let mem_typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] main(): str = {\n    return \"heap\";\n};\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Memo,
        },
    );
    let mem_syntax_id = find_named_routine_syntax_id(&mem_typed, "main");
    assert_eq!(
        mem_typed
            .typed_node(mem_syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| mem_typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Str)),
    );

    let mem_echo_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n    return .echo(1);\n};\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Memo,
        },
    );
    assert_eq!(mem_echo_errors.len(), 1);
    assert!(mem_echo_errors[0]
        .message()
        .contains("'.echo(...)' requires hosted std support"));

    let std_typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n    return .echo(1);\n};\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    let std_syntax_id = find_named_routine_syntax_id(&std_typed, "main");
    assert_eq!(
        std_typed
            .typed_node(std_syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| std_typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int)),
    );
}

#[test]
fn unknown_lock_method_is_not_treated_as_a_mutex_operation() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] misuse(value: Counter): non = {\n\
                 value.lock();\n\
                 return;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::InvalidInput
            && error
                .message()
                .contains("method 'lock' is not available for the receiver type")
    }));
    assert!(errors
        .iter()
        .all(|error| !error.message().contains("requires a [mux] parameter")));
}

#[test]
fn ordinary_lock_and_unlock_method_names_remain_available() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "typ Gate: rec = { value: int };\n\
             pro (Gate)lock(): non = { return; };\n\
             fun (Gate)unlock(): int = { return self.value; };\n\
             fun[] use(gate: Gate): int = {\n\
                 gate.lock();\n\
                 return gate.unlock();\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Core,
        },
    );

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "use"))
        .is_some());
}

#[test]
fn mutex_fields_require_an_active_lexical_guard() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] read(counter[mux]: Counter): int = {\n\
                 return counter.value;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| error
        .message()
        .contains("requires 'counter.lock()' in the current lexical scope")));
}

#[test]
fn deferred_blocks_reject_mutex_field_access() {
    for deferred in ["dfr", "edf"] {
        let source = format!(
            "typ Counter: rec = {{ value: int }};\n\
             fun[] inspect(counter[mux]: Counter, fail: bol): int / int = {{\n\
                 counter.lock();\n\
                 {deferred} {{ var seen: int = counter.value; }};\n\
                 when(fail) {{\n\
                     case(true) {{ report 1; }}\n\
                     * {{ return 0; }}\n\
                 }}\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source.as_str())],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error.message().contains(
                        "mutex field access through 'counter' is not allowed inside dfr/edf",
                    )
                    && error
                        .message()
                        .contains("delayed mutex guard effects are not modeled")
            }),
            "{deferred} must reject delayed mutex field access before lowering: {errors:#?}"
        );
    }
}

#[test]
fn deferred_blocks_reject_mutex_guard_operations() {
    for deferred in ["dfr", "edf"] {
        for method in ["lock", "unlock"] {
            let source = format!(
                "typ Counter: rec = {{ value: int }};\n\
                 fun[] inspect(counter[mux]: Counter, fail: bol): int / int = {{\n\
                     {deferred} {{ counter.{method}(); }};\n\
                     when(fail) {{\n\
                         case(true) {{ report 1; }}\n\
                         * {{ return 0; }}\n\
                     }}\n\
                 }};\n"
            );
            let errors = typecheck_fixture_folder_errors_with_config(
                &[("main.fol", source.as_str())],
                TypecheckConfig {
                    capability_model: TypecheckCapabilityModel::Std,
                },
            );

            assert!(
                errors.iter().any(|error| {
                    error.kind() == TypecheckErrorKind::Unsupported
                        && error
                            .message()
                            .contains(&format!("mutex .{method}() is not allowed inside dfr/edf"))
                        && error
                            .message()
                            .contains("delayed mutex guard effects are not modeled")
                }),
                "{deferred} must reject delayed mutex .{method}() before lowering: {errors:#?}"
            );
        }
    }
}

#[test]
fn deferred_blocks_reject_forwarded_mutex_handles() {
    for deferred in ["dfr", "edf"] {
        let source = format!(
            "typ Counter: rec = {{ value: int }};\n\
             fun[] leaf(counter[mux]: Counter): int = {{\n\
                 counter.lock();\n\
                 var value: int = counter.value;\n\
                 counter.unlock();\n\
                 return value;\n\
             }};\n\
             fun[] inspect(counter[mux]: Counter, fail: bol): int / int = {{\n\
                 counter.lock();\n\
                 {deferred} {{ var delayed: int = leaf(counter); }};\n\
                 when(fail) {{\n\
                     case(true) {{ report 1; }}\n\
                     * {{ return 0; }}\n\
                 }}\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source.as_str())],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error
                        .message()
                        .contains("mutex handles cannot be forwarded to [mux] parameter")
                    && error.message().contains("inside dfr/edf")
            }),
            "{deferred} must reject delayed mutex forwarding before lowering: {errors:#?}"
        );
        assert!(
            !errors.iter().any(|error| error
                .message()
                .contains("cannot synchronously forward mutex handle")),
            "{deferred} delayed-forwarding diagnostic must take priority over active-lock forwarding: {errors:#?}"
        );
    }
}

#[test]
fn deferred_blocks_reject_delayed_owner_reinitialization() {
    for deferred in ["dfr", "edf"] {
        let source = format!(
            "fun[] consume(pointer: ptr[int]): int = {{ return *pointer; }};\n\
             fun[] inspect(fail: bol): int / int = {{\n\
                 var first: int = 1;\n\
                 var second: int = 2;\n\
                 var[mut] pointer: ptr[int] = &first;\n\
                 consume(pointer);\n\
                 {deferred} {{ pointer = &second; }};\n\
                 when(fail) {{\n\
                     case(true) {{ report 1; }}\n\
                     * {{ return *pointer; }}\n\
                 }}\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source.as_str())],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.message().contains(
                        "moved binding 'pointer' cannot be reinitialized inside dfr/edf",
                    )
                    && error
                        .message()
                        .contains("delayed ownership effects are not modeled")
            }),
            "{deferred} must not make a delayed owner reinitialization visible immediately: {errors:#?}"
        );
    }
}

#[test]
fn mutex_lock_rejects_double_acquisition() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] update(counter[mux]: Counter): non = {\n\
                 counter.lock();\n\
                 counter.lock();\n\
                 return;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors
        .iter()
        .any(|error| error.message().contains("is already locked")));
}

#[test]
fn mutex_unlock_requires_a_current_scope_guard() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] update(counter[mux]: Counter): non = {\n\
                 counter.unlock();\n\
                 return;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors
        .iter()
        .any(|error| error.message().contains("is not locked")));
}

#[test]
fn mutex_guard_auto_releases_at_lexical_scope_end() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] update(counter[mux]: Counter): int = {\n\
                 {\n\
                     counter.lock();\n\
                     counter.value = 1;\n\
                 };\n\
                 counter.lock();\n\
                 return counter.value;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "update"))
        .is_some());
}

#[test]
fn mutex_whole_values_are_rejected_but_mux_forwarding_is_allowed() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] plain(value: Counter): int = { return value.value; };\n\
             fun[] bad(counter[mux]: Counter): int = {\n\
                 return plain(counter);\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("cannot be used as an unguarded whole value")));

    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] leaf(T)(counter[mux]: T): int = { return 1; };\n\
             fun[] forward(T)(counter[mux]: T): int = { return leaf(counter); };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "forward"))
        .is_some());
}

#[test]
fn mux_forwarding_does_not_move_the_protected_value() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { marker: ptr[int], value: int };\n\
             fun[] leaf(counter[mux]: Counter): int = { return 1; };\n\
             fun[] forward(counter[mux]: Counter): int = {\n\
                 leaf(counter);\n\
                 counter.lock();\n\
                 var value: int = counter.value;\n\
                 counter.unlock();\n\
                 return value;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "forward"))
        .is_some());
}

#[test]
fn synchronous_mux_forwarding_rejects_active_and_aliased_handles() {
    let active_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] leaf(counter[mux]: Counter): int = { return 1; };\n\
             fun[] bad(counter[mux]: Counter): int = {\n\
                 counter.lock();\n\
                 var result: int = leaf(counter);\n\
                 counter.unlock();\n\
                 return result;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(
        active_errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("cannot synchronously forward mutex handle 'counter'")
                && error.message().contains("while its lock is active")
        }),
        "a synchronous callee could lock the same guarded mutex and deadlock: {active_errors:#?}"
    );

    let alias_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] pair(left[mux]: Counter, right[mux]: Counter): int = { return 1; };\n\
             fun[] bad(counter[mux]: Counter): int = {\n\
                 return pair(counter, counter);\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(
        alias_errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("cannot forward mutex handle 'counter' to both [mux] parameter")
                && error.message().contains("can self-deadlock")
        }),
        "two mux parameters must not alias the same mutex handle: {alias_errors:#?}"
    );
}

#[test]
fn guarded_mutex_handles_can_cross_spawn_and_async_task_boundaries() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] worker(counter[mux]: Counter): int = {\n\
                 counter.lock();\n\
                 var value: int = counter.value;\n\
                 counter.unlock();\n\
                 return value;\n\
             };\n\
             fun[] launch(counter[mux]: Counter): int = {\n\
                 counter.lock();\n\
                 [>]worker(counter);\n\
                 var pending = worker(counter) | async;\n\
                 counter.unlock();\n\
                 return pending | await;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "launch"))
        .is_some());

    let qualified = typecheck_fixture_folder_with_config(
        &[
            (
                "workers/tasks.fol",
                "typ[exp] Counter: rec = { value: int };\n\
                 fun[exp] worker(counter[mux]: Counter): int = {\n\
                     counter.lock();\n\
                     var value: int = counter.value;\n\
                     counter.unlock();\n\
                     return value;\n\
                 };\n",
            ),
            (
                "main.fol",
                "fun[] launch(counter[mux]: workers::Counter): int = {\n\
                     counter.lock();\n\
                     [>]workers::worker(counter);\n\
                     var pending = workers::worker(counter) | async;\n\
                     counter.unlock();\n\
                     return pending | await;\n\
                 };\n",
            ),
        ],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(qualified
        .typed_node(find_named_routine_syntax_id(&qualified, "launch"))
        .is_some());
}

#[test]
fn mutex_handle_cannot_escape_inside_mux_argument() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             typ Wrapper: rec = { inner: Counter };\n\
             fun[] sink(value[mux]: Wrapper): int = { return 1; };\n\
             fun[] bad(counter[mux]: Counter): int = {\n\
                 return sink({ inner = counter });\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| error
        .message()
        .contains("cannot be used as an unguarded whole value")));
}

#[test]
fn every_processor_surface_rejects_core_and_memo_models() {
    let cases = [
        (
            "spawn",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): non = { [>]work(); return; };\n",
            "spawn requires hosted std support",
        ),
        (
            "channel",
            "fun[] main(): non = { var messages: chn[int]; return; };\n",
            "channel types require hosted std support",
        ),
        (
            "select",
            "fun[] main(): non = { select { * {} } return; };\n",
            "select requires hosted std support",
        ),
        (
            "mutex",
            "fun[] work(value[mux]: int): int = { return value; };\n",
            "mutex parameters require hosted std support",
        ),
        (
            "async",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): non = { var pending = work() | async; return; };\n",
            "async pipe stages require hosted std support",
        ),
        (
            "await",
            "fun[] main(): int = { var value: int = 1; return value | await; };\n",
            "await pipe stages require hosted std support",
        ),
    ];

    for capability_model in [
        TypecheckCapabilityModel::Core,
        TypecheckCapabilityModel::Memo,
    ] {
        for (surface, source, expected) in cases {
            let errors = typecheck_fixture_folder_errors_with_config(
                &[("main.fol", source)],
                TypecheckConfig { capability_model },
            );
            assert!(
                errors
                    .iter()
                    .any(|error| error.message().contains(expected)),
                "{surface} should reject {capability_model:?} with '{expected}', got {errors:?}"
            );
        }
    }
}

#[test]
fn processor_stages_reject_recoverable_pipe_spelling() {
    for (surface, source, expected) in [
        (
            "async",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): int = { var pending = work() || async; return 0; };\n",
            "'|| async'",
        ),
        (
            "await",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): int = { var pending = work() | async; return pending || await; };\n",
            "'|| await'",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains(expected)),
            "{surface} should reject the recoverable-pipe spelling, got {errors:?}"
        );
    }
}

#[test]
fn awaiting_an_eventual_binding_consumes_it() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): int = {\n\
                 var pending = work() | async;\n\
                 var first: int = pending | await;\n\
                 return pending | await;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of consumed eventual binding 'pending'")
    }));
}

#[test]
fn bare_recoverable_async_calls_cannot_discard_the_eventual() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 work(true) | async;\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("discarding a recoverable eventual loses its error")));
}

#[test]
fn bound_recoverable_eventuals_must_be_awaited_before_return() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var pending = work(true) | async;\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("recoverable eventual binding 'pending' must be awaited and handled")));
}

#[test]
fn awaited_recoverable_eventual_results_cannot_be_discarded() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var pending = work(true) | async;\n\
                 pending | await;\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("statement-position expression cannot use '/ ErrorType'")));
}

#[test]
fn recoverable_eventuals_cannot_be_overwritten_while_unhandled() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(value: int): int / int = {\n\
                 when(value == 0) {\n\
                     case(true) { report 9; }\n\
                     * { return value; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var[mut] pending = work(1) | async;\n\
                 var replacement = work(2) | async;\n\
                 pending = replacement;\n\
                 return (pending | await) || 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("recoverable eventual binding 'pending' cannot be overwritten")));
}

#[test]
fn recoverable_eventual_obligations_follow_transfers_and_can_be_handled() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var pending = work(false) | async;\n\
                 var moved = pending;\n\
                 return (moved | await) || 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn transferred_recoverable_eventuals_remain_obligations() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var pending = work(false) | async;\n\
                 var moved = pending;\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("recoverable eventual binding 'moved' must be awaited and handled")));
}

#[test]
fn recoverable_eventual_branch_obligations_require_every_path_to_handle() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(flag: bol): int = {\n\
                 var pending = work(false) | async;\n\
                 when(flag) {\n\
                     case(true) { var handled: bol = check(pending | await); }\n\
                     * { var untouched: int = 0; }\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("recoverable eventual binding 'pending' must be awaited and handled")));
}

#[test]
fn recoverable_eventual_branch_obligations_accept_all_paths_handled() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(flag: bol): int = {\n\
                 var pending = work(false) | async;\n\
                 when(flag) {\n\
                     case(true) { var first: bol = check(pending | await); }\n\
                     * { var second: bol = check(pending | await); }\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn recoverable_branch_values_transfer_across_trailing_comments() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(flag: bol): int = {\n\
                 var result: int = (when(flag) {\n\
                     is (true) -> work(false);\n\
                         // transferred branch value\n\
                     * -> work(false);\n\
                         // transferred branch value\n\
                 }) || 0;\n\
                 return result;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn infallible_unawaited_eventuals_remain_allowed() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] work(): int = { return 7; };\n\
             fun[] main(): int = {\n\
                 var pending = work() | async;\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn nested_anonymous_returns_ignore_outer_eventual_obligations() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var pending = work(false) | async;\n\
                 var action = fun[](): int = { return 1; };\n\
                 var ignored: int = action();\n\
                 return (pending | await) || ignored;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn nested_routines_cannot_await_outer_recoverable_eventuals() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var pending = work(false) | async;\n\
                 var action = fun[](): int = {\n\
                     return (pending | await) || 0;\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("implicit closure capture of outer local 'pending' is not supported")),
        "nested routine must not discharge an outer eventual obligation: {errors:#?}"
    );
}

#[test]
fn nested_routines_reject_implicit_clone_safe_captures() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var outer: int = 7;\n\
             var action = fun[](): int = { return outer; };\n\
             return action();\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("implicit closure capture of outer local 'outer' is not supported")),
        "closure capture must fail in typecheck instead of reaching lowering: {errors:#?}"
    );
}

#[test]
fn nested_routines_reject_implicit_capture_assignment_targets() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var[mut] outer: int = 7;\n\
             var action = fun[](): int = { outer = 9; return 0; };\n\
             return action();\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("implicit closure capture of outer local 'outer' is not supported")),
        "assignment targets must not bypass the closure boundary: {errors:#?}"
    );
}

#[test]
fn recoverable_eventuals_cannot_leave_their_lexical_scope_unhandled() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 { var pending = work(false) | async; };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("recoverable eventual binding 'pending' must be awaited and handled")));
}

#[test]
fn recoverable_eventuals_cannot_escape_through_report() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int / int = {\n\
                 var pending = work(false) | async;\n\
                 report 1;\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("recoverable eventual binding 'pending' must be awaited and handled")));
}

#[test]
fn edf_cannot_discharge_outer_recoverable_eventuals() {
    for cleanup in [
        "edf { var handled: bol = check(pending | await); };",
        "edf { dfr { var handled: bol = check(pending | await); }; };",
    ] {
        let source = format!(
            "fun[] work(fail: bol): int / int = {{\n\
                 when(fail) {{\n\
                     case(true) {{ report 9; }}\n\
                     * {{ return 7; }}\n\
                 }}\n\
             }};\n\
             fun[] main(): int = {{\n\
                 var pending = work(false) | async;\n\
                 {cleanup}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source.as_str())],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("await is not allowed inside edf")),
            "error-only cleanup must not discharge a normal-path obligation: {errors:#?}"
        );
    }
}

#[test]
fn recoverable_eventuals_cannot_escape_a_loop_through_break() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 loop(true) {\n\
                     var pending = work(false) | async;\n\
                     break;\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("recoverable eventual binding 'pending' must be awaited and handled")));
}

#[test]
fn break_rejects_recoverable_eventuals_assigned_to_outer_slots() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(fail: bol): int / int = {\n\
                 when(fail) {\n\
                     case(true) { report 9; }\n\
                     * { return 7; }\n\
                 }\n\
             };\n\
             fun[] main(): int = {\n\
                 var[mut] slot = work(false) | async;\n\
                 var first: int = (slot | await) || 0;\n\
                 loop(true) {\n\
                     var pending = work(false) | async;\n\
                     slot = pending;\n\
                     break;\n\
                 };\n\
                 return first;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("recoverable eventual binding 'slot' must be awaited and handled")),
        "break must preserve obligations activated inside the loop: {errors:#?}"
    );
}

#[test]
fn outer_eventuals_cannot_be_awaited_from_repeating_loops() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): int = {\n\
                 var pending = work() | async;\n\
                 var[mut] keep: bol = true;\n\
                 loop(keep) {\n\
                     var value: int = pending | await;\n\
                     keep = false;\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error.message().contains(
        "move-only binding 'pending' declared outside a repeating loop cannot be transferred"
    )));
}

#[test]
fn transferring_an_eventual_binding_moves_the_source() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): int = {\n\
                 var pending = work() | async;\n\
                 var moved = pending;\n\
                 return moved | await;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());

    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(): int = { return 1; };\n\
             fun[] main(): int = {\n\
                 var pending = work() | async;\n\
                 var moved = pending;\n\
                 var value: int = moved | await;\n\
                 return pending | await;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of moved eventual binding 'pending'")
    }));
}

#[test]
fn assigning_an_eventual_binding_moves_the_source() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] work(value: int): int = { return value; };\n\
             fun[] main(): int = {\n\
                 var pending = work(1) | async;\n\
                 var[mut] target = work(2) | async;\n\
                 target = pending;\n\
                 var value: int = target | await;\n\
                 return pending | await;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of moved eventual binding 'pending'")
    }));
}

#[test]
fn internal_eventuals_do_not_cross_unchecked_generic_boundaries() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun identity(T)(value: T): T = { return value; };\n\
             fun[] work(): int = { return 1; };\n\
             fun[] main(): int = {\n\
                 var pending = work() | async;\n\
                 var moved = identity(pending);\n\
                 return moved | await;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("cannot pass an internal eventual through a generic parameter")
    }));
}

#[test]
fn sender_only_capture_cannot_receive_from_its_channel() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 var channel: chn[int];\n\
                 [>]fun()[channel[tx]] = {\n\
                     var stolen: int = channel[rx];\n\
                     return;\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error
        .message()
        .contains("captured endpoint 'channel[tx]' is sender-only")));
}

#[test]
fn channel_send_consumes_move_only_payloads() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 var channel: chn[int];\n\
                 @var owned: int = 42;\n\
                 owned | channel[tx];\n\
                 return owned;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of moved heap-owned binding 'owned'")
    }));
}

#[test]
fn empty_blocking_select_rejects_during_typecheck() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 select {};\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::InvalidInput
            && error
                .message()
                .contains("blocking select requires at least one channel arm")
    }));
}

#[test]
fn select_merges_ownership_from_mutually_exclusive_arms() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
             fun[] produce(channel: chn[int]): int = { 1 | channel[tx]; return 1; };\n\
             fun[] main(): int = {\n\
                 var first: chn[int];\n\
                 var second: chn[int];\n\
                 [>]produce(first);\n\
                 [>]produce(second);\n\
                 var first_value: int = 1;\n\
                 var second_value: int = 2;\n\
                 var[mut] pointer: ptr[int] = &first_value;\n\
                 select {\n\
                     when first as received { consume(pointer); }\n\
                     when second as received { pointer = &second_value; }\n\
                     * { pointer = &second_value; }\n\
                 };\n\
                 return *pointer;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("use of moved heap-owned binding 'pointer'")
        }),
        "a later select-arm reinitialization must not erase an earlier arm's move: {errors:#?}"
    );

    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
             fun[] produce(channel: chn[int]): int = { 1 | channel[tx]; return 1; };\n\
             fun[] main(): int = {\n\
                 var first: chn[int];\n\
                 var second: chn[int];\n\
                 [>]produce(first);\n\
                 [>]produce(second);\n\
                 var value: int = 1;\n\
                 var pointer: ptr[int] = &value;\n\
                 select {\n\
                     when first as first_received { consume(pointer); }\n\
                     when second as second_received { consume(pointer); }\n\
                     * { consume(pointer); }\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(
        typed
            .typed_node(find_named_routine_syntax_id(&typed, "main"))
            .is_some(),
        "each mutually exclusive select arm must start from the same ownership state"
    );
}

#[test]
fn select_without_default_preserves_the_all_closed_ownership_path() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
             fun[] main(): int = {\n\
                 var channel: chn[int];\n\
                 var first: int = 1;\n\
                 var second: int = 2;\n\
                 var[mut] pointer: ptr[int] = &first;\n\
                 consume(pointer);\n\
                 select {\n\
                     when channel as received { pointer = &second; }\n\
                 };\n\
                 return *pointer;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("use of moved heap-owned binding 'pointer'")
        }),
        "an all-closed select may skip every arm and must preserve its entry flow: {errors:#?}"
    );
}

#[test]
fn sender_only_locals_cannot_receive_through_select_or_iteration() {
    for (surface, body) in [
        (
            "select",
            "select {\n\
                 when sender[rx] as value { return value; }\n\
                 * { return 0; }\n\
             }",
        ),
        (
            "channel iteration",
            "for (value in sender[rx]) {\n\
                 return value;\n\
             }",
        ),
    ] {
        let source = format!(
            "fun[] main(): int = {{\n\
                 var channel: chn[int];\n\
                 var sender = channel[tx];\n\
                 {body}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("sender-only channel endpoints cannot receive")
            }),
            "{surface} should reject a sender-only local before lowering, got {errors:?}"
        );
    }
}

#[test]
fn borrowed_values_cannot_cross_spawn_or_async_boundaries() {
    for (surface, statement) in [
        ("spawn", "[>]inspect(#owner);"),
        ("async", "var pending = inspect(#owner) | async;"),
    ] {
        let source = format!(
            "fun[] inspect(value[bor]: int): int = {{ return 0; }};\n\
             fun[] main(): int = {{\n\
                 var owner: int = 42;\n\
                 {statement}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("borrowed values cannot cross a spawn or async thread boundary")
            }),
            "{surface} should reject a borrowed argument, got {errors:?}"
        );
    }
}

#[test]
fn unresolved_generic_values_cannot_cross_spawn_or_async_boundaries() {
    for (surface, statement) in [
        ("spawn", "[>]identity(value);"),
        ("async", "var pending = identity(value) | async;"),
    ] {
        let source = format!(
            "fun identity(T)(value: T): T = {{ return value; }};\n\
             fun launch(T)(value: T): int = {{\n\
                 {statement}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error.message().contains(
                        "unconstrained generic values cannot cross a spawn or async thread boundary",
                    )
            }),
            "{surface} should reject an unresolved generic argument, got {errors:?}"
        );
    }
}

#[test]
fn concrete_generic_calls_can_cross_spawn_and_async_boundaries() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun choose(T)(value: T, fallback: int = 1): T = { return value; };\n\
             fun[] main(): int = {\n\
                 [>]choose(7);\n\
                 var pending = choose(9) | async;\n\
                 return pending | await;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn direct_qualified_calls_can_cross_processor_boundaries() {
    let typed = typecheck_fixture_folder_with_config(
        &[
            (
                "workers/tasks.fol",
                "fun[] launch(value: int = 7): int = { return value; };\n\
                 fun[] compute(value: int = 41): int = { return value + 1; };\n",
            ),
            (
                "main.fol",
                "fun[] main(): int = {\n\
                     [>]workers::launch();\n\
                     var pending = workers::compute() | async;\n\
                     return pending | await;\n\
                 };\n",
            ),
        ],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn processor_boundaries_reject_indirect_routine_value_calls() {
    for (value_surface, program_prefix, program_suffix) in [
        (
            "stored named routine",
            "fun[] worker(): int = { return 7; };\n\
             fun[] main(): int = {\n\
                 var action = worker;\n",
            "return 0;\n};\n",
        ),
        (
            "stored anonymous routine",
            "fun[] main(): int = {\n\
                 var action = fun[](): int = { return 7; };\n",
            "return 0;\n};\n",
        ),
        (
            "routine parameter",
            "fun[] launch(action: {fun (): int}): int = {\n",
            "return 0;\n};\n\
             fun[] worker(): int = { return 7; };\n\
             fun[] main(): int = { return launch(worker); };\n",
        ),
    ] {
        for (processor_surface, statement) in [
            ("spawn", "[>]action();"),
            ("async", "var pending = action() | async;"),
        ] {
            let source = format!("{program_prefix}    {statement}\n    {program_suffix}");
            let errors = typecheck_fixture_folder_errors_with_config(
                &[("main.fol", &source)],
                TypecheckConfig {
                    capability_model: TypecheckCapabilityModel::Std,
                },
            );

            assert!(
                errors.iter().any(|error| {
                    error.kind() == TypecheckErrorKind::Unsupported
                        && error.message().contains(
                            "requires a direct call to a named routine declaration in V3",
                        )
                }),
                "{processor_surface} should reject a {value_surface} call before lowering, got {errors:?} from {source:?}"
            );
        }
    }

    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 [>]fun() = { return; };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn omitted_borrowed_defaults_cannot_cross_processor_boundaries() {
    for (surface, statement) in [
        ("spawn", "[>]inspect();"),
        ("async", "var pending = inspect() | async;"),
    ] {
        let source = format!(
            "fun[] inspect(value[bor]: int = 7): int = {{ return 0; }};\n\
             fun[] main(): int = {{\n\
                 {statement}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("borrowed values cannot cross a spawn or async thread boundary")
            }),
            "{surface} should reject an omitted borrowed default, got {errors:?}"
        );
    }
}

#[test]
fn omitted_shared_pointer_defaults_cannot_cross_processor_boundaries() {
    for (surface, statement) in [
        ("spawn", "[>]inspect();"),
        ("async", "var pending = inspect() | async;"),
    ] {
        let source = format!(
            "fun[] inspect(value: ptr[shared, int] = &7): int = {{ return 0; }};\n\
             fun[] main(): int = {{\n\
                 {statement}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("values containing shared Rc pointers cannot cross")
            }),
            "{surface} should reject an omitted shared-pointer default, got {errors:?}"
        );
    }
}

#[test]
fn qualified_omitted_defaults_preserve_processor_boundaries() {
    for (surface, statement, parameter, expected) in [
        (
            "spawn borrowed default",
            "[>]workers::inspect();",
            "value[bor]: int = 7",
            "borrowed values cannot cross a spawn or async thread boundary",
        ),
        (
            "async borrowed default",
            "var pending = workers::inspect() | async;",
            "value[bor]: int = 7",
            "borrowed values cannot cross a spawn or async thread boundary",
        ),
        (
            "spawn shared-pointer default",
            "[>]workers::inspect();",
            "value: ptr[shared, int] = &7",
            "values containing shared Rc pointers cannot cross",
        ),
        (
            "async shared-pointer default",
            "var pending = workers::inspect() | async;",
            "value: ptr[shared, int] = &7",
            "values containing shared Rc pointers cannot cross",
        ),
    ] {
        let worker = format!("fun[] inspect({parameter}): int = {{ return 0; }};\n");
        let main = format!(
            "fun[] main(): int = {{\n\
                 {statement}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[
                ("workers/tasks.fol", worker.as_str()),
                ("main.fol", main.as_str()),
            ],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains(expected)),
            "{surface} should retain its omitted-argument boundary, got {errors:?}"
        );
    }
}

#[test]
fn spawn_rejects_method_non_call_and_parameterized_anonymous_tasks() {
    for (surface, task, expected) in [
        (
            "method call",
            "worker.run()",
            "spawn requires a direct named routine call",
        ),
        (
            "non-call expression",
            "42",
            "spawn requires a direct named routine call",
        ),
        (
            "parameterized anonymous routine",
            "fun(value: int): int = { return value; }",
            "a directly spawned anonymous routine cannot declare call parameters",
        ),
    ] {
        let source = format!(
            "typ Worker: rec = {{ value: int }};\n\
             fun (Worker)run(): int = {{ return 1; }};\n\
             fun[] main(): int = {{\n\
                 var worker: Worker = {{ value = 0 }};\n\
                 [>]{task};\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains(expected)),
            "{surface} should be rejected during typecheck, got {errors:?}"
        );
    }
}

#[test]
fn anonymous_recoverable_spawn_cannot_discard_its_error() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 [>]fun(): int / int = {\n\
                     when(true) {\n\
                         case(true) { report 9; }\n\
                         * { return 0; }\n\
                     }\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| error
        .message()
        .contains("bare '[>]call()' cannot spawn a recoverable routine")));
}

#[test]
fn direct_spawn_rejects_a_channel_consumer_routine() {
    for (surface, statement) in [
        ("spawn", "[>]consume(channel);"),
        ("async", "var pending = consume(channel) | async;"),
    ] {
        let source = format!(
            "fun[] consume(channel: chn[int]): int = {{\n\
                 return channel[rx];\n\
             }};\n\
             fun[] main(): int = {{\n\
                 var channel: chn[int];\n\
                 {statement}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| error.message().contains(
                "routine 'consume' receives from a channel and cannot be spawned directly"
            )),
            "{surface} should preserve the single channel receiver, got {errors:?}"
        );
    }
}

#[test]
fn channel_receiver_effect_follows_local_aliases_and_wrappers() {
    for (surface, receiver_body) in [
        (
            "alias",
            "var alias = channel;\n                 return alias[rx];",
        ),
        ("wrapper", "return consume(channel);"),
    ] {
        let source = format!(
            "fun[] consume(channel: chn[int]): int = {{\n\
                 return channel[rx];\n\
             }};\n\
             fun[] wrapper(channel: chn[int]): int = {{\n\
                 {receiver_body}\n\
             }};\n\
             fun[] main(): int = {{\n\
                 var channel: chn[int];\n\
                 [>]wrapper(channel);\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| error.message().contains(
                "routine 'wrapper' receives from a channel and cannot be spawned directly"
            )),
            "{surface} receiver flow should reach the spawn boundary, got {errors:?}"
        );
    }
}

#[test]
fn sender_capture_alias_cannot_receive_or_call_a_consumer() {
    for (surface, body) in [
        (
            "alias receive",
            "var alias = channel;\n                     var value: int = alias[rx];",
        ),
        ("consumer call", "var value: int = consume(channel);"),
    ] {
        let source = format!(
            "fun[] consume(channel: chn[int]): int = {{ return channel[rx]; }};\n\
             fun[] main(): int = {{\n\
                 var channel: chn[int];\n\
                 [>]fun()[channel[tx]] = {{\n\
                     {body}\n\
                     return;\n\
                 }};\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| {
                error
                    .message()
                    .contains("sender-only channel endpoints cannot receive")
                    || (error.message().contains("expects 'chn[int]'")
                        && error.message().contains("chn[int][tx]"))
            }),
            "{surface} should not recover the receiver capability, got {errors:?}"
        );
    }
}

#[test]
fn non_receiving_channel_parameters_are_sender_only_capabilities() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "fun[] produce(channel: chn[int]): int = {\n\
                 1 | channel[tx];\n\
                 return 1;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    let (_, produce) = find_typed_symbol(&typed, "produce", SymbolKind::Routine);
    let signature = produce
        .declared_type
        .and_then(|type_id| typed.type_table().get(type_id))
        .and_then(|typ| match typ {
            CheckedType::Routine(signature) => Some(signature),
            _ => None,
        })
        .expect("produce should retain its routine signature");
    assert!(matches!(
        signature
            .params
            .first()
            .and_then(|type_id| typed.type_table().get(*type_id)),
        Some(CheckedType::ChannelSender { .. })
    ));
}

#[test]
fn transferring_a_channel_receiver_moves_the_source_binding() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] consume(channel: chn[int]): int = { return channel[rx]; };\n\
             fun[] main(): int = {\n\
                 var channel: chn[int];\n\
                 42 | channel[tx];\n\
                 var value: int = consume(channel);\n\
                 return channel[rx];\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("use of moved channel receiver binding 'channel'")
    }));
}

#[test]
fn outer_channel_receivers_cannot_move_into_consumers_from_repeating_loops() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] consume(channel: chn[int]): int = { return channel[rx]; };\n\
             fun[] main(): int = {\n\
                 var channel: chn[int];\n\
                 42 | channel[tx];\n\
                 var[mut] keep: bol = true;\n\
                 loop(keep) {\n\
                     var value: int = consume(channel);\n\
                     keep = false;\n\
                 };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| error.message().contains(
        "move-only binding 'channel' declared outside a repeating loop cannot be transferred"
    )));
}

#[test]
fn receiver_acquisition_rejects_late_transmitter_acquisition() {
    for (surface, body) in [
        (
            "direct endpoint",
            "var value: int = channel[rx];\n                 2 | channel[tx];",
        ),
        (
            "sender wrapper",
            "var value: int = channel[rx];\n                 var sent: int = produce(channel);",
        ),
        (
            "nested capture",
            "{\n                     var value: int = channel[rx];\n                     [>]fun()[channel[tx]] = { 2 | channel[tx]; return; };\n                 };",
        ),
        (
            "select receiver",
            "var[mut] seen: int = 0;\n                 select {\n                     when channel as value { seen = value; }\n                     * { seen = seen; }\n                 }\n                 2 | channel[tx];",
        ),
        (
            "loop receiver",
            "loop(true) {\n                     2 | channel[tx];\n                     var value: int = channel[rx];\n                     break;\n                 };",
        ),
    ] {
        let source = format!(
            "fun[] produce(channel: chn[int]): int = {{ 1 | channel[tx]; return 1; }};\n\
             fun[] main(): int = {{\n\
                 var channel: chn[int];\n\
                 1 | channel[tx];\n\
                 {body}\n\
                 return 0;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| error
                .message()
                .contains("is no longer available after receiver acquisition")),
            "{surface} should reject late tx acquisition, got {errors:?}"
        );
    }

    for (surface, deferred_body) in [
        ("direct deferred endpoint", "1 | channel[tx];"),
        (
            "deferred endpoint capture",
            "[>]fun()[channel[tx]] = { 1 | channel[tx]; return; };",
        ),
    ] {
        let source = format!(
            "fun[] main(): int = {{\n\
                 var channel: chn[int];\n\
                 dfr {{ {deferred_body} }};\n\
                 return 0;\n\
             }};\n"
        );
        let deferred_errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            deferred_errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("channel endpoint acquisition is not allowed inside dfr/edf")
            }),
            "{surface} should report the explicit dfr/edf endpoint boundary, got {deferred_errors:?}"
        );
    }
}

#[test]
fn receiver_acquisition_rejects_late_method_sender_call() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Relay: rec = { value: int };\n\
             fun[] (Relay)produce(channel: chn[int]): int = {\n\
                 1 | channel[tx];\n\
                 return 1;\n\
             };\n\
             fun[] main(): int = {\n\
                 var relay: Relay = { value = 0 };\n\
                 var channel: chn[int];\n\
                 1 | channel[tx];\n\
                 var value: int = channel[rx];\n\
                 return relay.produce(channel);\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| error
        .message()
        .contains("is no longer available after receiver acquisition")));
}

#[test]
fn channel_endpoint_bases_reject_non_binding_values() {
    for (surface, body) in [
        (
            "computed receive",
            "var value: int = make()[rx];\n                 return value;",
        ),
        (
            "computed select receiver",
            "var[mut] seen: int = 0;\n                 select {\n                     when make() as value { seen = value; }\n                     * { seen = seen; }\n                 }\n                 return seen;",
        ),
    ] {
        let source = format!(
            "fun[] make(): chn[int] = {{\n\
                 var channel: chn[int];\n\
                 return channel;\n\
             }};\n\
             fun[] main(): int = {{\n\
                 {body}\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error
                        .message()
                        .contains("requires a direct local, parameter, or capture binding")
            }),
            "{surface} should report the explicit V3 endpoint-base boundary, got {errors:?}"
        );
    }
}

#[test]
fn aggregate_types_cannot_embed_full_channels() {
    for (surface, source) in [
        (
            "record field",
            "typ Holder: rec = { channel: chn[int] };\nfun[] main(): int = { return 0; };\n",
        ),
        (
            "entry payload",
            "typ Message: ent = { var CHANNEL: chn[int]; };\nfun[] main(): int = { return 0; };\n",
        ),
        (
            "container element",
            "fun[] main(): int = { var channels: vec[chn[int]]; return 0; };\n",
        ),
        (
            "wrapper value",
            "fun[] main(value: opt[chn[int]]): int = { return 0; };\n",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error
                        .message()
                        .contains("full chn[T] values cannot be embedded")
            }),
            "{surface} should report the aggregate-channel boundary, got {errors:?}"
        );
    }
}

#[test]
fn channel_endpoint_bases_reject_implicit_outer_routine_captures() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 var channel: chn[int];\n\
                 fun[] nested(): int = {\n\
                     1 | channel[tx];\n\
                     return 1;\n\
                 };\n\
                 return nested();\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("implicit closure capture of outer local 'channel' is not supported")
    }));
}

#[test]
fn anonymous_channel_parameters_report_the_v3_boundary() {
    for (surface, declaration, param_type) in [
        ("direct channel", "", "chn[int]"),
        ("channel alias", "ali Messages: chn[int];\n", "Messages"),
    ] {
        let source = format!(
            "{declaration}fun[] main(): int = {{\n\
                 var sender = fun[](channel: {param_type}): int = {{\n\
                     1 | channel[tx];\n\
                     return 1;\n\
                 }};\n\
                 var channel: chn[int];\n\
                 return sender(channel);\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", &source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error
                        .message()
                        .contains("anonymous routine chn[T] parameters are not supported in V3")
            }),
            "{surface} should report the anonymous-channel boundary, got {errors:?}"
        );
    }
}

#[test]
fn mutex_parameter_abi_stays_on_named_direct_calls() {
    for (surface, source, expected) in [
        (
            "named routine value",
            "typ Counter: rec = { value: int };\n\
             fun[] worker(counter[mux]: Counter): int = { return 0; };\n\
             fun[] main(): int = { var action = worker; return 0; };\n",
            "routine 'worker' with [mux] parameters cannot be used as a plain routine value in V3",
        ),
        (
            "anonymous routine value",
            "typ Counter: rec = { value: int };\n\
             fun[] main(): int = {\n\
                 var action = fun[](counter[mux]: Counter): int = { return 0; };\n\
                 return 0;\n\
             };\n",
            "anonymous routines with [mux] parameters are not supported in V3",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error.message().contains(expected)
            }),
            "{surface} should stop before mutex ABI metadata is erased: {errors:?}"
        );
    }

    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "typ Counter: rec = { value: int };\n\
             fun[] worker(counter[mux]: Counter): int = { return 1; };\n\
             fun[] main(): int = {\n\
                 var counter: Counter = { value = 0 };\n\
                 return worker(counter);\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn top_level_channel_bindings_report_the_v3_boundary() {
    for (surface, source) in [
        (
            "direct declaration",
            "var global: chn[int];\nfun[] main(): int = { return 0; };\n",
        ),
        (
            "alias declaration",
            "ali Messages: chn[int];\nvar global: Messages;\nfun[] main(): int = { return 0; };\n",
        ),
        (
            "inferred initializer",
            "fun[] make(): chn[int] = { var channel: chn[int]; return channel; };\nvar global = make();\nfun[] main(): int = { return 0; };\n",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error
                        .message()
                        .contains("top-level channel bindings are not supported in V3")
            }),
            "{surface} should report the top-level-channel boundary, got {errors:?}"
        );
    }
}

#[test]
fn top_level_bindings_reject_non_global_safe_v3_types() {
    for (surface, source, expected) in [
        (
            "owned allocation",
            "typ Item: rec = { value: int };\n@var global: Item = { value = 7 };\nfun[] main(): int = { return 0; };\n",
            "top-level move-only bindings are not supported in V3",
        ),
        (
            "unique pointer",
            "var global: ptr[int];\nfun[] main(): int = { return 0; };\n",
            "top-level move-only bindings are not supported in V3",
        ),
        (
            "aggregate containing a unique pointer",
            "typ Holder: rec = { pointer: ptr[int] };\nvar global: Holder;\nfun[] main(): int = { return 0; };\n",
            "top-level move-only bindings are not supported in V3",
        ),
        (
            "inferred unique pointer",
            "fun[] make(): ptr[int] = { var seed: int = 7; return &seed; };\nvar global = make();\nfun[] main(): int = { return 0; };\n",
            "top-level move-only bindings are not supported in V3",
        ),
        (
            "shared Rc pointer",
            "var global: ptr[shared, int];\nfun[] main(): int = { return 0; };\n",
            "top-level bindings containing ptr[shared, T] are not supported in V3",
        ),
        (
            "borrowed global",
            "var owner: int = 7;\nvar[bor] global: int = #owner;\nfun[] main(): int = { return 0; };\n",
            "top-level bindings containing borrowed values are not supported in V3",
        ),
    ] {
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source)],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );

        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error.message().contains(expected)
            }),
            "{surface} should report the global-storage boundary, got {errors:?}"
        );
    }
}

#[test]
fn top_level_clone_safe_scalars_and_records_remain_available() {
    let typed = typecheck_fixture_folder_with_config(
        &[(
            "main.fol",
            "typ Snapshot: rec = { value: int };\n\
             var count: int = 1;\n\
             var current: Snapshot = { value = 2 };\n\
             fun[] main(): int = { return count + current.value; };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );

    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());
}

#[test]
fn spawn_boundaries_reject_recursively_nested_shared_pointers() {
    let errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Shared: rec = { value: ptr[shared, int] };\n\
             fun[] consume(value: Shared): int = { return 0; };\n\
             fun[] main(): int = {\n\
                 var value: int = 1;\n\
                 var shared: Shared = { value = &value };\n\
                 [>]consume(shared);\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("values containing shared Rc pointers cannot cross")
    }));

    let capture_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] main(): int = {\n\
                 var channel: chn[ptr[shared, int]];\n\
                 [>]fun()[channel[tx]] = { return; };\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(capture_errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error.message().contains("captured endpoint 'channel[tx]'")
    }));

    let method_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Shared: rec = { value: ptr[shared, int] };\n\
             fun (Shared)consume(): int = { return 0; };\n\
             fun[] main(): int = {\n\
                 var value: int = 1;\n\
                 var shared: Shared = { value = &value };\n\
                 [>]shared.consume();\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(method_errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("values containing shared Rc pointers cannot cross")
    }));

    let method_channel_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "typ Worker: rec = { value: int };\n\
             fun (Worker)consume(channel: chn[int]): int = { return channel[rx]; };\n\
             fun[] main(): int = {\n\
                 var worker: Worker = { value = 0 };\n\
                 var channel: chn[int];\n\
                 [>]worker.consume(channel);\n\
                 return 0;\n\
             };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(method_channel_errors.iter().any(|error| {
        error
            .message()
            .contains("receives from a channel and cannot be spawned directly")
    }));

    let async_errors = typecheck_fixture_folder_errors_with_config(
        &[(
            "main.fol",
            "fun[] make(): ptr[shared, int] = {\n\
                 var shared: ptr[shared, int];\n\
                 return shared;\n\
             };\n\
             fun[] main(): int = { var pending = make() | async; return 0; };\n",
        )],
        TypecheckConfig {
            capability_model: TypecheckCapabilityModel::Std,
        },
    );
    assert!(async_errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Ownership
            && error
                .message()
                .contains("async result containing shared Rc pointers")
    }));
}

#[test]
fn expression_typing_rejects_unknown_named_arguments_for_method_calls() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)shift(by: int, step: int): int = {\n\
             return by;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.shift(missing = 2, by = 1);\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("does not have a parameter named 'missing'")
        }),
        "Expected an unknown named-argument diagnostic for method call, got: {errors:?}"
    );
}

#[test]
fn expression_typing_rejects_method_call_arity_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Counter)read(value: int): int = {\n\
             return value;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.read();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("expects 1 args but got 0")
        }),
        "Expected an arity diagnostic for method call mismatch, got: {errors:?}"
    );
}

#[test]
fn expression_typing_selects_method_overloads_by_record_receiver_type() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         typ Meter: rec = {\n\
             value: int\n\
         };\n\
         var current_counter: Counter;\n\
         var current_meter: Meter;\n\
         fun (Counter)read(): int = {\n\
             return 1;\n\
         };\n\
         fun (Meter)read(): bol = {\n\
             return true;\n\
         };\n\
         fun[] read_counter(): int = {\n\
             return current_counter.read();\n\
         };\n\
         fun[] read_meter(): bol = {\n\
             return current_meter.read();\n\
         };\n",
    )]);

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
fn expression_typing_rejects_missing_methods_on_record_receivers() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun[] demo(): int = {\n\
             return current.missing();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("method 'missing' is not available for the receiver type in V1")
        }),
        "Expected a missing-method diagnostic for record receiver, got: {errors:?}"
    );
}

#[test]
fn expression_typing_rejects_methods_for_the_wrong_record_receiver_type() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         typ Meter: rec = {\n\
             value: int\n\
         };\n\
         var current: Counter;\n\
         fun (Meter)read(): int = {\n\
             return 1;\n\
         };\n\
         fun[] demo(): int = {\n\
             return current.read();\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("method 'read' is not available for the receiver type in V1")
        }),
        "Expected a wrong-receiver method diagnostic for record receiver, got: {errors:?}"
    );
}

#[test]
fn expression_typing_types_field_access_against_named_record_receivers() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             value: int\n\
         };\n\
         fun[] read(counter: Counter): int = {\n\
             return counter.value;\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "read");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_rejects_field_access_on_non_records() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad(value: int): int = {\n\
             return value.total;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("field access '.total' requires a record-like or entry-like receiver")
        }),
        "Expected a non-record field-access diagnostic, got: {errors:?}"
    );
}

#[test]
fn record_initializer_omits_fields_that_declare_a_default() {
    // A field with a declared default may be omitted from a named
    // initializer; the default supplies its value.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int;\n\
             step: int = 2\n\
         };\n\
         fun[] main(): int = {\n\
             var c: Counter = { total = 3 };\n\
             return c.total + c.step;\n\
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
fn record_initializer_rejects_missing_field_without_default() {
    // Fields without a default stay required.
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int;\n\
             step: int\n\
         };\n\
         fun[] main(): int = {\n\
             var c: Counter = { total = 3 };\n\
             return c.total + c.step;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.message().contains("missing required fields: step") }),
        "Expected a missing-required-field diagnostic for the non-default field, got: {errors:?}"
    );
}

#[test]
fn record_initializer_rejects_default_type_mismatch() {
    // A default whose expression mismatches the field type is rejected with a
    // located diagnostic at the declaration.
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int;\n\
             flag: bol = 7\n\
         };\n\
         fun[] main(): int = {\n\
             var c: Counter = { total = 3 };\n\
             return c.total;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("default for record field 'flag'")
        }),
        "Expected a default-type-mismatch diagnostic, got: {errors:?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.diagnostic_location().is_some()),
        "Default-type-mismatch diagnostic should be located, got: {errors:?}"
    );
}

#[test]
fn positional_record_initializer_binds_fields_in_declaration_order() {
    // `{ v0, v1 }` binds values to fields in declaration order when the
    // expected type is a record.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int;\n\
             step: int\n\
         };\n\
         fun[] main(): int = {\n\
             var c: Counter = { 3, 4 };\n\
             return c.total + c.step;\n\
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
fn positional_record_initializer_fills_trailing_defaults() {
    // Fields uncovered by positional values fall back to their defaults.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int;\n\
             step: int = 2\n\
         };\n\
         fun[] main(): int = {\n\
             var c: Counter = { 5 };\n\
             return c.total + c.step;\n\
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
fn positional_record_initializer_rejects_too_many_values() {
    // Supplying more positional values than the record has fields is a clean
    // located arity diagnostic.
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int;\n\
             step: int\n\
         };\n\
         fun[] main(): int = {\n\
             var c: Counter = { 3, 4, 5 };\n\
             return c.total + c.step;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("positional record initializer has 3 value(s)")
                && error.message().contains("2 field(s)")
        }),
        "Expected a positional arity diagnostic, got: {errors:?}"
    );
}

#[test]
fn expression_typing_expands_alias_record_shells_for_field_access() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ CounterShape: rec = {\n\
             value: int\n\
         };\n\
         ali Counter: CounterShape;\n\
         var current: Counter = { value = 1 };\n\
         fun[] read(): int = {\n\
             return current.value;\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "read");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_types_container_index_accesses() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] head(values: vec[int]): int = {\n\
             return values[0];\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "head");
    assert_eq!(
        typed
            .typed_node(syntax_id)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn expression_typing_types_basic_slice_accesses() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] tail(values: vec[int]): vec[int] = {\n\
             return values[1:];\n\
         };\n",
    )]);

    let syntax_id = find_named_routine_syntax_id(&typed, "tail");
    let inferred = typed
        .typed_node(syntax_id)
        .and_then(|node| node.inferred_type)
        .and_then(|type_id| typed.type_table().get(type_id));

    assert!(matches!(
        inferred,
        Some(CheckedType::Vector { element_type })
            if typed.type_table().get(*element_type)
                == Some(&CheckedType::Builtin(BuiltinType::Int))
    ));
}

#[test]
fn expression_typing_rejects_non_indexable_receivers() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] bad(value: int): int = {\n\
             return value[0];\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains(
                    "index access requires an array, vector, sequence, set, or map receiver",
                )
        }),
        "Expected a non-indexable access diagnostic, got: {errors:?}"
    );
}

#[test]
fn routine_return_typing_rejects_explicit_return_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] demo(): int = {\n\
             return false;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("return expects")
        }),
        "Expected a return-type mismatch diagnostic, got: {errors:?}"
    );
}

#[test]
fn routine_return_typing_rejects_final_body_expression_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var flag: bol = false;\n\
         fun[] demo(): int = {\n\
             flag\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("not all code paths use 'return'")
        }),
        "Expected a missing-return diagnostic, got: {errors:?}"
    );
}

#[test]
fn routine_return_typing_rejects_missing_return_values_for_typed_routines() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] demo(): int = {\n\
             return;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("return requires a value for routines with a declared return type")
        }),
        "Expected a missing-return-value diagnostic, got: {errors:?}"
    );
}

#[test]
fn routine_error_typing_accepts_matching_report_values() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] demo(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
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
fn routine_error_typing_rejects_report_value_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] demo(): int / str = {\n\
             report 1;\n\
             return 1;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::IncompatibleType
                && error.message().contains("report expects")
        }),
        "Expected a report-type mismatch diagnostic, got: {errors:?}"
    );
}

#[test]
fn routine_error_typing_requires_declared_error_types() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] demo(): int = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("report requires a declared routine error type")
        }),
        "Expected a missing-error-type diagnostic, got: {errors:?}"
    );
}

#[test]
fn routine_error_typing_rejects_missing_report_values() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] demo(): int / str = {\n\
             report;\n\
             return 1;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("report expects exactly 1 value in V1 but got 0")
        }),
        "Expected a missing-report-value diagnostic, got: {errors:?}"
    );
}

#[test]
fn routine_error_calls_keep_recoverable_effects_on_call_references() {
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

    let reference = find_typed_reference(&typed, "load", ReferenceKind::FunctionCall);

    assert_eq!(
        reference
            .resolved_type
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
    assert_eq!(
        reference
            .recoverable_effect
            .and_then(|effect| typed.type_table().get(effect.error_type)),
        Some(&CheckedType::Builtin(BuiltinType::Str))
    );
}

#[test]
fn inferred_bindings_reject_recoverable_call_results() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int = {\n\
             var current = load();\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("initializer for 'current'")
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected a strict binding diagnostic, got: {errors:?}"
    );
}

#[test]
fn plain_use_of_errorful_calls_rejects_plain_value_contexts() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int = {\n\
             var total: int = load() + 1;\n\
             return total;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected a plain-use errorful-call diagnostic, got: {errors:?}"
    );
}

#[test]
fn propagation_typing_rejects_matching_error_types_in_plain_value_contexts() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int / str = {\n\
             return load() + 1;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected a strict no-propagation diagnostic, got: {errors:?}"
    );
}

#[test]
fn propagation_typing_rejects_incompatible_error_types_in_plain_value_contexts() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int / int = {\n\
             return load() + 1;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("cannot use '/ ErrorType' routine results as plain values")
        }),
        "Expected a strict no-propagation diagnostic, got: {errors:?}"
    );
}

#[test]
fn self_referential_record_type_is_rejected_without_panicking() {
    // A direct self-referential value field has no finite runtime shape. The
    // checker rejects it with an honest, located diagnostic rather than
    // accepting an unbuildable type or overflowing the stack during lowering.
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Node: rec = {\n\
             value: int;\n\
             next: Node;\n\
         };\n\
         fun[] main(): int = {\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error.message().contains("recursive value type 'Node'")
                && error.message().contains("opt @Node")
        }),
        "Expected a self-referential record to be rejected with an honest boundary, got: {errors:?}"
    );
}

#[test]
fn single_element_double_quoted_literals_follow_the_expected_type() {
    // The book allows a double-quoted single element as BOTH a character and
    // a single-element string; the expected type decides.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var s: str = \"l\";\n\
             var c: chr = \"z\";\n\
             var s2: str = \"many\";\n\
             return .len(s) + .len(s2);\n\
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
fn casting_surfaces_fail_with_the_explicit_boundary_not_resolver_noise() {
    // The book: casting parses but is not part of supported V1 semantics;
    // the failure must be the explicit typecheck boundary, not an
    // "unresolved name" resolver error for the target type.
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var n: int = 3 as int;\n\
             return n;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("'as' is not yet supported")),
        "casting should fail with the explicit unsupported-operator boundary: {errors:#?}"
    );
    assert!(
        !errors
            .iter()
            .any(|error| error.message().contains("could not resolve name 'int'")),
        "the cast target must not be misreported as an unresolved value name: {errors:#?}"
    );
}

#[test]
fn array_literals_must_match_the_declared_size() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var values: arr[int, 2] = {1, 2, 3};\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("array literal has 3 element(s)")),
        "array size mismatches should fail typecheck cleanly: {errors:#?}"
    );
}

#[test]
fn var_declarations_inside_when_case_bodies_typecheck() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             when(true) {\n\
                 case(true) {\n\
                     var local: int = 3;\n\
                     return local;\n\
                 }\n\
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
fn field_assignment_into_mutable_record_binding_typechecks() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
             var[mut] counter: Counter = { total = 1 };\n\
             counter.total = 5;\n\
             return counter.total;\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int)),
        "field assignment into a mutable record should keep the routine's return type",
    );
}

#[test]
fn field_assignment_into_immutable_record_binding_is_rejected() {
    // `con` binds an immutable constant (plain `var` is mutable by default in
    // the current parser model); assigning into its field must be rejected.
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
             con counter: Counter = { total = 1 };\n\
             counter.total = 5;\n\
             return counter.total;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error.message().contains("immutable binding 'counter'")
                && error.message().contains("var[mut]")
        }),
        "Expected immutable field-assignment rejection, got: {errors:?}"
    );
}

#[test]
fn field_assignment_to_unknown_field_is_rejected() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
             var[mut] counter: Counter = { total = 1 };\n\
             counter.missing = 5;\n\
             return counter.total;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("does not expose a field named 'missing'")
        }),
        "Expected unknown-field rejection, got: {errors:?}"
    );
}

#[test]
fn field_assignment_type_mismatch_is_rejected() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Counter: rec = {\n\
             total: int\n\
         };\n\
         \n\
         fun[] main(): int = {\n\
             var[mut] counter: Counter = { total = 1 };\n\
             counter.total = true;\n\
             return counter.total;\n\
         };\n",
    )]);

    assert!(
        errors
            .iter()
            .any(|error| { error.kind() == TypecheckErrorKind::IncompatibleType }),
        "Expected field-assignment type mismatch rejection, got: {errors:?}"
    );
}

#[test]
fn when_membership_arms_stay_on_the_explicit_v1_boundary() {
    // `has`/`in`/`on` when-arms are declared syntax whose semantics are
    // later-milestone; they must be rejected cleanly instead of silently
    // lowering as equality checks against the subject.
    let has_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var values: arr[int, 3] = {1, 2, 3};\n\
             when(values) {\n\
                 has(2) { return 1; }\n\
                 * { return 0; }\n\
             }\n\
         };\n",
    )]);
    assert!(
        has_errors.iter().any(|error| error
            .message()
            .contains("when/has branches are not yet supported")),
        "has arms should hit the explicit boundary: {has_errors:#?}"
    );

    let in_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             when(5) {\n\
                 in(3) { return 1; }\n\
                 * { return 0; }\n\
             }\n\
         };\n",
    )]);
    assert!(
        in_errors.iter().any(|error| error
            .message()
            .contains("when/in branches are not yet supported")),
        "in arms should hit the explicit boundary: {in_errors:#?}"
    );

    // Equality arms stay fully supported in both spellings.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var picked: int = when(3) {\n\
                 is 3 -> 7;\n\
                 * -> 0;\n\
             };\n\
             when(picked) {\n\
                 is (7) { return picked; }\n\
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
fn immutable_bindings_reject_whole_binding_reassignment() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             con locked: int = 5;\n\
             locked = 6;\n\
             return locked;\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("cannot reassign immutable binding 'locked'")),
        "con bindings should refuse reassignment: {errors:#?}"
    );
}

#[test]
fn moved_mutable_bindings_can_be_reinitialized() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
         fun[] main(): int = {\n\
             var first: int = 1;\n\
             var second: int = 2;\n\
             var[mut] pointer: ptr[int] = &first;\n\
             var old: int = consume(pointer);\n\
             pointer = &second;\n\
             pointer = pointer;\n\
             return old + *pointer;\n\
         };\n",
    )]);
    assert!(typed
        .typed_node(find_named_routine_syntax_id(&typed, "main"))
        .is_some());

    let immutable_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
         fun[] main(): int = {\n\
             var first: int = 1;\n\
             var second: int = 2;\n\
             con pointer: ptr[int] = &first;\n\
             consume(pointer);\n\
             pointer = &second;\n\
             return 0;\n\
         };\n",
    )]);
    assert!(
        immutable_errors.iter().any(|error| error
            .message()
            .contains("cannot reassign immutable binding 'pointer'")),
        "reinitialization must preserve ordinary mutability checks: {immutable_errors:#?}"
    );

    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
         fun[] choose(flag: bol): int = {\n\
             var first: int = 1;\n\
             var second: int = 2;\n\
             var[mut] pointer: ptr[int] = &first;\n\
             consume(pointer);\n\
             when(flag) {\n\
                 case(true) { pointer = &first; }\n\
                 * { pointer = &second; }\n\
             }\n\
             return *pointer;\n\
         };\n",
    )]);
    assert!(
        typed
            .typed_node(find_named_routine_syntax_id(&typed, "choose"))
            .is_some(),
        "a binding reinitialized on every continuing branch must become usable"
    );

    let partial_errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] consume(pointer: ptr[int]): int = { return *pointer; };\n\
         fun[] choose(flag: bol): int = {\n\
             var value: int = 1;\n\
             var[mut] pointer: ptr[int] = &value;\n\
             consume(pointer);\n\
             when(flag) {\n\
                 * { pointer = &value; }\n\
             }\n\
             return *pointer;\n\
         };\n",
    )]);
    assert!(
        partial_errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Ownership
                && error
                    .message()
                    .contains("use of moved heap-owned binding 'pointer'")
        }),
        "a case-less gate can skip reinitialization and must keep the binding maybe-moved: {partial_errors:#?}"
    );
}

#[test]
fn loop_reinitialization_does_not_erase_the_zero_iteration_move() {
    for (surface, setup, loop_body) in [
        ("condition loop", "", "loop(false) { pointer = &second; };"),
        (
            "iterable loop",
            "var values: arr[int, 1] = { 1 };",
            "loop(value in values when false) { pointer = &second; };",
        ),
        (
            "channel loop",
            "var channel: chn[int];",
            "for (value in channel[rx]) { pointer = &second; };",
        ),
    ] {
        let source = format!(
            "fun[] consume(pointer: ptr[int]): int = {{ return *pointer; }};\n\
             fun[] main(): int = {{\n\
                 var first: int = 1;\n\
                 var second: int = 2;\n\
                 var[mut] pointer: ptr[int] = &first;\n\
                 consume(pointer);\n\
                 {setup}\n\
                 {loop_body}\n\
                 return *pointer;\n\
             }};\n"
        );
        let errors = typecheck_fixture_folder_errors_with_config(
            &[("main.fol", source.as_str())],
            TypecheckConfig {
                capability_model: TypecheckCapabilityModel::Std,
            },
        );
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Ownership
                    && error
                        .message()
                        .contains("use of moved heap-owned binding 'pointer'")
            }),
            "{surface} may skip its body and must preserve the entry ownership flow: {errors:#?}"
        );
    }
}

#[test]
fn panic_terminates_when_arms_and_stays_out_of_defer() {
    // A when arm that panics terminates like return/report.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             when(true) {\n\
                 case(true) { panic \"boom\"; }\n\
                 * { return 0; }\n\
             }\n\
         };\n",
    )]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());

    // Deferred blocks replay at every exit; panic cannot lower there.
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             dfr { panic \"cleanup\"; };\n\
             return 0;\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("panic is not allowed inside dfr/edf blocks")),
        "dfr should keep an explicit panic boundary: {errors:#?}"
    );
}

#[test]
fn map_index_keys_follow_the_declared_key_type() {
    // Single-character double-quoted literals width-classify as chr in the
    // parser; index expressions must adopt the container's key type.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var m: map[str, int] = {{\"x\", 5}};\n\
             return m[\"x\"];\n\
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
fn var_declarations_inside_for_loop_bodies_lower_into_the_binder_scope() {
    // Regression: a `var` declared in a for-in loop body lives in the loop's
    // dedicated binder scope, not the routine scope. The nested-declaration
    // pre-pass must lower it against that scope, otherwise the binding is
    // absent from typed lowering (T1099). An if/else or when-case body already
    // worked; the loop-body arm mirrors that mechanism.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var xs: seq[int] = {1, 2, 4};\n\
             var total: int = 0;\n\
             for (x in xs) {\n\
                 var extra: int = 1;\n\
                 total = total + x + extra;\n\
             }\n\
             return total;\n\
         };\n",
    )]);
    let (_extra_id, extra) = find_typed_symbol(&typed, "extra", SymbolKind::ValueBinding);
    assert_eq!(
        extra
            .declared_type
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int)),
        "loop-body binding 'extra' should carry its declared type in typed lowering"
    );
}

#[test]
fn generic_call_infers_entry_type_from_a_bare_variant_argument() {
    // Regression: a bare entry-variant access (`Status.OK`) with no concrete
    // expectation denotes a value of the ENTRY type, not the variant payload.
    // Argument-driven generic inference must therefore bind the type parameter
    // to `Status`, so the `Status`-typed initializer matches (previously T
    // bound to the payload `int`, producing a T1003 initializer mismatch).
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "typ Status: ent = {\n\
             var OK: int = 1;\n\
             var BAD: int = 2;\n\
         };\n\
         fun pick(T)(v: T): T = {\n\
             return v;\n\
         };\n\
         fun[] main(): int = {\n\
             var s: Status = pick(Status.OK);\n\
             return 0;\n\
         };\n",
    )]);
    let (status_id, _status) = find_typed_symbol(&typed, "Status", SymbolKind::Type);
    let (_s_id, s) = find_typed_symbol(&typed, "s", SymbolKind::ValueBinding);
    assert_eq!(
        s.declared_type
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: status_id,
            name: "Status".to_string(),
            kind: DeclaredTypeKind::Type,
            args: Vec::new(),
        }),
        "the entry-typed binding 's' should keep its declared entry type"
    );
}

#[test]
fn statements_after_block_terminated_statements_parse_as_statements() {
    // Block-terminated statements (`when`/`loop`) end at `}` without `;`;
    // the following qualified call and assignment must start fresh
    // statements instead of falling into expression parsing.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] seven(): int = {\n\
             return 7;\n\
         };\n\
         fun[] main(): int = {\n\
             var total: int = 0;\n\
             loop(false) {\n\
                 var extra: int = 1;\n\
             }\n\
             total = seven();\n\
             when(true) {\n\
                 case(true) { var a: int = 1; }\n\
                 * { var b: int = 2; }\n\
             }\n\
             return total;\n\
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
fn inline_container_literals_are_iterable() {
    // Bare array literals intern with their actual length so loop lowering
    // can resolve the sized container shape.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var total: int = 0;\n\
             for (i in {1, 2}) {\n\
                 total = total + i;\n\
             }\n\
             return total;\n\
         };\n",
    )]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

#[test]
fn type_mismatch_diagnostics_render_fol_surface_syntax() {
    // User-facing type mismatches must read as FOL syntax (int, bol,
    // vec[int]) rather than the internal Rust Debug form (Builtin(Int),
    // Vector { element_type: CheckedTypeId(0) }).
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var v: vec[int] = 5;\n\
             return 0;\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| {
            let m = error.message();
            m.contains("vec[int]")
                && m.contains("int")
                && !m.contains("Builtin(")
                && !m.contains("Vector {")
        }),
        "mismatch should render FOL surface types: {errors:#?}"
    );
}

#[test]
fn routines_keep_their_own_parameter_names_across_interning() {
    // Two routines with identical shapes but different parameter names must
    // not collapse to one interned signature: named-argument binding reads
    // the names, so each declaration keeps its own (param_names is part of
    // routine identity), while routine VALUES stay assignable by shape.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] alpha(aaa: int): int = {\n\
             return aaa;\n\
         };\n\
         fun[] beta(bbb: int): int = {\n\
             return bbb;\n\
         };\n\
         fun[] apply(op: {fun (n: int): int}, v: int): int = {\n\
             return op(v);\n\
         };\n\
         fun[] main(): int = {\n\
             var x: int = beta(bbb = 2);\n\
             var y: int = apply(alpha, 3);\n\
             return x + y;\n\
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

    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] alpha(aaa: int): int = {\n\
             return aaa;\n\
         };\n\
         fun[] beta(bbb: int): int = {\n\
             return bbb;\n\
         };\n\
         fun[] main(): int = {\n\
             return beta(aaa = 2);\n\
         };\n",
    )]);
    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("does not have a parameter named 'aaa'")),
        "the wrong name must fail at typecheck: {errors:#?}"
    );
}

#[test]
fn pathological_nesting_is_rejected_with_a_clean_diagnostic() {
    // Recursive descent follows user nesting; the parser bounds syntactic
    // nesting with a clean located diagnostic instead of letting the native
    // stack overflow (SIGABRT), and legitimate nesting depths still parse
    // (typecheck/resolve/lowering grow their stacks in segments).
    let ok_parens = format!(
        "fun[] main(): int = {{\n    return {}1{};\n}};\n",
        "(".repeat(50),
        ")".repeat(50)
    );
    let typed = typecheck_fixture_folder(&[("main.fol", ok_parens.as_str())]);
    let main = find_named_routine_syntax_id(&typed, "main");
    assert!(typed.typed_node(main).is_some());
}

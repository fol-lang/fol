use super::*;

#[test]
fn cast_policy_rejects_as_and_cast_surfaces_in_v1() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "var text: str = \"label\";\n\
         var target: int = 0;\n\
         fun[] bad_as(value: int): int = {\n\
             return value as text;\n\
         };\n\
         fun[] bad_cast(value: int): int = {\n\
             return value cast target;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("operator 'as' is not yet supported")
                && error.diagnostic_location().is_some()
        }),
        "Expected an unsupported 'as' cast diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("operator 'cast' is not yet supported")
                && error.diagnostic_location().is_some()
        }),
        "Expected an unsupported 'cast' diagnostic, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_additional_lower_only_operator_surfaces_early() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(flag: bol, value: int): bol = {\n\
             var seen: bol = value is value;\n\
             return seen;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("type testing operator 'is' is not yet supported")
        }),
        "Expected the unsupported 'is' diagnostic, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_template_calls_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             var value: int = 1;\n\
             return value$;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("template instantiation is not yet supported")
        }),
        "Expected template-call rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_anonymous_routines_with_captures_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): non = {\n\
             var keep = fun[](value: int)[outer;]: int = {\n\
                 return value;\n\
             };\n\
             return;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("V3 anonymous captures must name a channel endpoint")
        }),
        "Expected anonymous capture rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_complex_anonymous_routine_types_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): non = {\n\
             var keep = fun[](items: vec[int]): int = {\n\
                 return .len(items);\n\
             };\n\
             return;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("complex type annotation in anonymous routine is not yet supported")
        }),
        "Expected complex anonymous-type rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_type_matching_when_of_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Box: rec = {\n\
             value: int\n\
         };\n\
         fun[] main(value: Box): int = {\n\
             when(value) {\n\
                 of(Box) { return 1; }\n\
                 { return 0; }\n\
             }\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("type-matching when/of branches are not yet supported in V1")
        }),
        "Expected when/of type-matching rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_procedure_style_method_calls_as_values_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Box: rec = {\n\
             value: int\n\
         };\n\
         pro (Box)reset(): non = {\n\
             return;\n\
         };\n\
         fun[] main(box: Box): int = {\n\
             var kept: int = box.reset();\n\
             return kept;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("initializer for 'kept' does not have a type")
        }),
        "Expected procedure-style method value rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_procedure_style_free_calls_as_values_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "pro greet(): non = {\n\
             return;\n\
         };\n\
         fun[] main(): int = {\n\
             var kept: int = greet();\n\
             return kept;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("initializer for 'kept' does not have a type")
        }),
        "Expected procedure-style free-call value rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_pipe_loop_stages_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun decide(value: int): int = {\n\
             return value | while(true) {\n\
                 break;\n\
             };\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("binary operator right operand does not have a type")
        }),
        "Expected pipe-loop rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_pipe_block_stages_before_lowering() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] block_stage(value: int): int = {\n\
             return value | {\n\
                 value\n\
             };\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("pipe operator '|>' is not yet supported")
        }),
        "Expected pipe-block rejection before lowering, got: {errors:?}"
    );
}

#[test]
fn literal_family_policy_accepts_matching_integer_and_float_sites() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun[] take_int(value: int): int = {\n\
             return value;\n\
         };\n\
         fun[] take_float(value: flt): flt = {\n\
             return value;\n\
         };\n\
         fun[] good_int(): int = {\n\
             var count: int = 1;\n\
             return take_int(2);\n\
         };\n\
         fun[] good_float(): flt = {\n\
             var ratio: flt = 1.5;\n\
             return take_float(2.5);\n\
         };\n",
    )]);

    let (_count_id, count) = find_typed_symbol(&typed, "count", SymbolKind::ValueBinding);
    let (_ratio_id, ratio) = find_typed_symbol(&typed, "ratio", SymbolKind::ValueBinding);
    let good_int = find_named_routine_syntax_id(&typed, "good_int");
    let good_float = find_named_routine_syntax_id(&typed, "good_float");

    assert_eq!(
        typed.type_table().get(
            count
                .declared_type
                .expect("int literal binding should lower")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
    assert_eq!(
        typed.type_table().get(
            ratio
                .declared_type
                .expect("float literal binding should lower")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Float))
    );
    assert_eq!(
        typed
            .typed_node(good_int)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
    assert_eq!(
        typed
            .typed_node(good_float)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Float))
    );
}

#[test]
fn v1_boundary_rejects_generic_headers_and_meta_declarations() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Bound: rec = {\n\
         };\n\
         typ Box(T: Bound): rec = {\n\
             value: int\n\
         };\n\
         def helper: mod = {\n\
         };\n\
         seg core: mod = {\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("definition/meta declarations are planned for a future release")
        }),
        "Expected a def/meta boundary diagnostic, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("segment declarations are planned for a future release")
        }),
        "Expected a seg/meta boundary diagnostic, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_generic_routine_constraints_in_all_supported_header_kinds() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Bound: rec = {\n\
         };\n\
         fun pick(T: Bound)(value: T): T = {\n\
             return value;\n\
         };\n\
         pro apply(T: Bound)(value: T): T = {\n\
             return value;\n\
         };\n\
         log check(T: Bound)(value: T): bol = {\n\
             return true;\n\
         };\n",
    )]);

    let generic_routine_errors = errors
        .iter()
        .filter(|error| {
            error.kind() == TypecheckErrorKind::InvalidInput
                && error
                    .message()
                    .contains("must resolve to a standard declaration")
        })
        .count();

    assert!(
        generic_routine_errors >= 3,
        "Expected non-standard generic constraints to be rejected across headers, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_accepts_generic_error_types() {
    // Generic error types are now part of the shipped V2 contract.
    // Confirm the routine lowers cleanly and retains a typed signature.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "fun pick(T)(value: T): int / T = {\n\
             report(value);\n\
             return 1;\n\
         };\n",
    )]);
    let (_pick_id, pick_symbol) = find_typed_symbol(&typed, "pick", SymbolKind::Routine);
    assert!(pick_symbol.declared_type.is_some());
}

#[test]
fn v1_boundary_keeps_protocol_conformance_while_rejecting_later_surfaces() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Shape()(geo): rec = {\n\
             var value: int;\n\
         };\n\
         fun (Shape)area(): int = {\n\
             return 1;\n\
         };\n\
         typ[ext] StrExt: str;\n\
         typ Box: rec = {\n\
         };\n\
         std geometry: blu = {\n\
             var width: int;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("type extension declarations are planned for a future release")
        }),
        "Expected a type-extension boundary diagnostic, got: {errors:?}"
    );
    // Blueprints are now part of the shipped V2 contract; the standalone
    // `std geometry: blu` with no conformer should simply typecheck.
    assert!(
        !errors.iter().any(|error| {
            error
                .message()
                .contains("blueprint standards are planned for a future release")
        }),
        "Blueprint standards should no longer surface a 'planned' diagnostic, got: {errors:?}"
    );
}

#[test]
fn current_boundary_rejects_unshipped_system_declaration_surfaces() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "!var cached: int = 1;\n\
         ?var watching: int = 1;\n\
         ",
    )]);

    for expected in [
        "static binding semantics are not yet supported",
        "reactive binding semantics are not yet supported",
    ] {
        assert!(
            errors.iter().any(|error| {
                error.kind() == TypecheckErrorKind::Unsupported
                    && error.message().contains(expected)
            }),
            "Expected an unshipped declaration boundary diagnostic containing '{expected}', got: {errors:?}"
        );
    }
}

#[test]
fn v1_boundary_rejects_v3_expression_surfaces() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun rangeDemo(): int = {\n\
             var span: int = 0;\n\
             span = 1..2;\n\
             return 0;\n\
         };\n",
    )]);

    let expected = "range expressions are not yet supported";
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported && error.message().contains(expected)
        }),
        "Expected a V3 expression boundary diagnostic containing '{expected}', got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_rolling_expression() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun rolling(items: vec[int]): vec[int] = {\n\
             return { x for x in items };\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("rolling/comprehension expressions are not yet supported")
        }),
        "Rolling expressions should be rejected at typecheck, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_yield_expression() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(): int = {\n\
             yield 42;\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("yield expressions are not yet supported")
        }),
        "Yield expressions should be rejected at typecheck, got: {errors:?}"
    );
}

#[test]
fn v1_boundary_rejects_pattern_access() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "fun[] main(items: vec[int]): int = {\n\
             var subset: vec[int] = items[0, 1, 2];\n\
             return 0;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("pattern access is not yet supported")
        }),
        "Pattern access should be rejected at typecheck, got: {errors:?}"
    );
}

#[test]
fn ordinary_typechecking_keeps_build_fol_source_units_without_failing() {
    let typed = typecheck_fixture_folder(&[("build.fol", "`package build`\n")]);

    assert_eq!(typed.source_units().len(), 1);
    assert_eq!(
        typed.source_units()[0].kind,
        fol_parser::ast::ParsedSourceUnitKind::Build
    );
    assert_eq!(typed.build_source_units().count(), 1);
}

#[test]
fn workspace_typechecking_caches_loaded_packages_by_identity() {
    let root = unique_temp_dir("workspace_typecheck_cache");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("shared/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_with_config(&root, "app", ResolverConfig::default())
        .expect(
            "Workspace typechecking should accept packages that do not yet use imported values",
        );

    assert_eq!(typed.package_count(), 2);
    assert_eq!(typed.entry_program().package_name(), "app");
    assert!(
        typed
            .packages()
            .any(|package| package.identity.display_name == "shared"),
        "Typed workspace should retain typed facts for directly loaded packages"
    );
}

#[test]
fn workspace_typechecking_dedupes_repeated_loaded_packages() {
    let root = unique_temp_dir("workspace_typecheck_dedupe");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("shared/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                concat!(
                    "use left: loc = {\"../shared\"};\n",
                    "use right: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return 0;\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace typechecking should reuse one typed package per identity");

    assert_eq!(typed.package_count(), 2);
    assert_eq!(
        typed
            .packages()
            .filter(|package| package.identity.display_name == "shared")
            .count(),
        1,
        "Typed workspace should cache one typed package fact per package identity"
    );
}

#[test]
fn workspace_typechecking_imports_mounted_value_and_routine_types_from_foreign_packages() {
    let root = unique_temp_dir("workspace_typecheck_mounted_types");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "var[exp] answer: int = 42;\n",
                    "fun[exp] bump(value: int): int = {\n",
                    "    return value + 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_with_config(&root, "app", ResolverConfig::default())
        .expect(
            "Workspace typechecking should import mounted symbol facts from dependency packages",
        );
    let entry = typed.entry_program();

    let (_answer_id, answer) = find_typed_symbol(entry, "answer", SymbolKind::ValueBinding);
    assert_eq!(
        entry.type_table().get(
            answer
                .declared_type
                .expect("mounted imported values should keep translated types")
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );

    let (_bump_id, bump) = find_typed_symbol(entry, "bump", SymbolKind::Routine);
    assert_eq!(
        entry.type_table().get(
            bump.declared_type
                .expect("mounted imported routines should keep translated signatures")
        ),
        Some(&CheckedType::Routine(RoutineType {
            generic_params: Vec::new(),
            generic_constraints: BTreeMap::new(),
            param_names: vec!["value".to_string()],
            param_defaults: vec![None],
            variadic_index: None,
            mutex_params: Default::default(),
            params: vec![entry.builtin_types().int],
            return_type: Some(entry.builtin_types().int),
            error_type: None,
        }))
    );
}

#[test]
fn workspace_typechecking_preserves_local_only_success_shape() {
    let root = unique_temp_dir("workspace_typecheck_local_only");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[(
            "app/main.fol",
            concat!(
                "ali Count: int;\n",
                "fun[] helper(value: Count): Count = {\n",
                "    return value;\n",
                "};\n",
            ),
        )],
    );

    let direct = typecheck_fixture_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Direct typechecking should still accept local-only packages");
    let workspace =
        typecheck_fixture_workspace_with_config(&root, "app", ResolverConfig::default())
            .expect("Workspace typechecking should preserve local-only packages");
    let workspace_entry = workspace.entry_program();

    assert_eq!(workspace.package_count(), 1);

    let (direct_count_id, _direct_count) = find_typed_symbol(&direct, "Count", SymbolKind::Alias);
    let (workspace_count_id, _workspace_count) =
        find_typed_symbol(workspace_entry, "Count", SymbolKind::Alias);
    let (_direct_helper_id, direct_helper) =
        find_typed_symbol(&direct, "helper", SymbolKind::Routine);
    let (_workspace_helper_id, workspace_helper) =
        find_typed_symbol(workspace_entry, "helper", SymbolKind::Routine);

    let direct_signature = match direct.type_table().get(
        direct_helper
            .declared_type
            .expect("direct helper should have a signature"),
    ) {
        Some(CheckedType::Routine(signature)) => signature,
        other => panic!("expected direct helper routine signature, got {other:?}"),
    };
    assert_eq!(direct_signature.params.len(), 1);
    assert_eq!(direct_signature.error_type, None);
    assert_eq!(
        direct.type_table().get(direct_signature.params[0]),
        Some(&CheckedType::Declared {
            symbol: direct_count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    assert_eq!(
        direct_signature
            .return_type
            .and_then(|type_id| direct.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: direct_count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );

    let workspace_signature = match workspace_entry.type_table().get(
        workspace_helper
            .declared_type
            .expect("workspace helper should have a signature"),
    ) {
        Some(CheckedType::Routine(signature)) => signature,
        other => panic!("expected workspace helper routine signature, got {other:?}"),
    };
    assert_eq!(workspace_signature.params.len(), 1);
    assert_eq!(workspace_signature.error_type, None);
    assert_eq!(
        workspace_entry
            .type_table()
            .get(workspace_signature.params[0]),
        Some(&CheckedType::Declared {
            symbol: workspace_count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    assert_eq!(
        workspace_signature
            .return_type
            .and_then(|type_id| workspace_entry.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: workspace_count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
}

#[test]
fn workspace_typechecking_keeps_loaded_package_declaration_signatures() {
    let root = unique_temp_dir("workspace_typecheck_loaded_package_decls");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "ali Count: int;\n",
                    "var[exp] answer: Count = 42;\n",
                    "fun[exp] bump(value: Count): Count = {\n",
                    "    return value + 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_with_config(&root, "app", ResolverConfig::default())
        .expect(
            "Workspace typechecking should type loaded package declarations before entry typing",
        );
    let shared = &find_typed_package(&typed, "shared").program;

    let (count_id, _count) = find_typed_symbol(shared, "Count", SymbolKind::Alias);
    let (_answer_id, answer) = find_typed_symbol(shared, "answer", SymbolKind::ValueBinding);
    let (_bump_id, bump) = find_typed_symbol(shared, "bump", SymbolKind::Routine);

    assert_eq!(
        shared.type_table().get(
            answer
                .declared_type
                .expect("loaded package values should lower declared types")
        ),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    let signature = match shared.type_table().get(
        bump.declared_type
            .expect("loaded package routines should lower signatures"),
    ) {
        Some(CheckedType::Routine(signature)) => signature,
        other => panic!("expected loaded package routine signature, got {other:?}"),
    };
    assert_eq!(signature.params.len(), 1);
    assert_eq!(signature.error_type, None);
    assert_eq!(
        shared.type_table().get(signature.params[0]),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
    assert_eq!(
        signature
            .return_type
            .and_then(|type_id| shared.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: DeclaredTypeKind::Alias,
            args: Vec::new(),
        })
    );
}

#[test]
fn workspace_expression_typing_keeps_plain_imported_value_reference_types() {
    let root = unique_temp_dir("workspace_imported_value_reference_types");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("shared/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return answer;\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect("Workspace entry typing should accept imported value references");
    let reference = find_typed_reference(&typed, "answer", ReferenceKind::Identifier);

    assert_eq!(
        typed.type_table().get(
            reference
                .resolved_type
                .expect("imported value references should keep a resolved type"),
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn workspace_expression_typing_keeps_plain_imported_call_types() {
    let root = unique_temp_dir("workspace_imported_call_types");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!("fun[exp] answer(): int = {\n", "    return 42;\n", "};\n",),
            ),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] main(): int = {\n",
                    "    return answer();\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect("Workspace entry typing should accept imported routine calls");
    let reference = find_typed_reference(&typed, "answer", ReferenceKind::FunctionCall);

    assert_eq!(
        typed.type_table().get(
            reference
                .resolved_type
                .expect("imported call references should keep a resolved type"),
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );
}

#[test]
fn workspace_typechecking_imports_alias_record_and_entry_type_facts() {
    let root = unique_temp_dir("workspace_imported_type_facts");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Count: int;\n",
                    "typ[exp] Point: rec = {\n",
                    "    x: int;\n",
                    "    y: int;\n",
                    "};\n",
                    "typ[exp] Outcome: ent = {\n",
                    "    var Ok: int = 1;\n",
                    "    con Fail: str = \"bad\";\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect(
            "Workspace entry typing should import semantic type facts for exported type surfaces",
        );

    let (_count_id, count) = find_typed_symbol(&typed, "Count", SymbolKind::Type);
    let (_point_id, point) = find_typed_symbol(&typed, "Point", SymbolKind::Type);
    let (_outcome_id, outcome) = find_typed_symbol(&typed, "Outcome", SymbolKind::Type);

    assert_eq!(
        typed.type_table().get(
            count
                .declared_type
                .expect("imported aliases should keep lowered semantic types"),
        ),
        Some(&CheckedType::Builtin(BuiltinType::Int))
    );

    let point_fields = BTreeMap::from([
        ("x".to_string(), typed.builtin_types().int),
        ("y".to_string(), typed.builtin_types().int),
    ]);
    assert_eq!(
        typed.type_table().get(
            point
                .declared_type
                .expect("imported record types should keep lowered semantic types"),
        ),
        Some(&CheckedType::Record {
            fields: point_fields
        })
    );

    let outcome_variants = BTreeMap::from([
        ("Fail".to_string(), Some(typed.builtin_types().str_)),
        ("Ok".to_string(), Some(typed.builtin_types().int)),
    ]);
    assert_eq!(
        typed.type_table().get(
            outcome
                .declared_type
                .expect("imported entry types should keep lowered semantic types"),
        ),
        Some(&CheckedType::Entry {
            variants: outcome_variants,
        })
    );
}

#[test]
fn workspace_typechecking_keeps_direct_loc_import_declaration_facts() {
    let root = unique_temp_dir("workspace_direct_loc_decls");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                concat!(
                    "typ[exp] Count: int;\n",
                    "var[exp] answer: Count = 42;\n",
                    "fun[exp] bump(value: Count): Count = {\n",
                    "    return value + 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed =
        typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
            .expect("Workspace entry typing should keep direct loc import declaration facts");

    assert_imported_declared_count_binding_and_routine(&typed, SymbolKind::Type);
}

#[test]
fn workspace_typechecking_keeps_direct_std_import_declaration_facts() {
    let root = unique_temp_dir("workspace_direct_std_decls");
    let store_root = root.join("store");
    create_dir_all(&store_root).expect("Package store root should be creatable");
    write_fixture_files(
        &root,
        &[
            (
                "store/std/build.fol",
                "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"std\", version = \"0.1.0\" });\n};\n",
            ),
            (
                "store/std/src/lib.fol",
                concat!(
                    "typ[exp] Count: int;\n",
                    "var[exp] answer: Count = 42;\n",
                    "fun[exp] bump(value: Count): Count = {\n",
                    "    return value + 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                "use std: pkg = {\"std\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(
        &root,
        "app",
        ResolverConfig {
            std_root: None,
            package_store_root: Some(
                store_root
                    .to_str()
                    .expect("package store root should be valid UTF-8")
                    .to_string(),
            ),
        },
    )
    .expect("Workspace entry typing should keep direct std import declaration facts");

    assert_imported_declared_count_binding_and_routine(&typed, SymbolKind::Type);
}

#[test]
fn workspace_typechecking_keeps_direct_pkg_import_declaration_facts() {
    let root = unique_temp_dir("workspace_direct_pkg_decls");
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
            (
                "store/json/src/lib.fol",
                concat!(
                    "typ[exp] Count: int;\n",
                    "var[exp] answer: Count = 42;\n",
                    "fun[exp] bump(value: Count): Count = {\n",
                    "    return value + 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(
        &root,
        "app",
        ResolverConfig {
            std_root: None,
            package_store_root: Some(
                store_root
                    .to_str()
                    .expect("Package store root should be valid UTF-8")
                    .to_string(),
            ),
        },
    )
    .expect("Workspace entry typing should keep direct pkg import declaration facts");

    assert_imported_declared_count_binding_and_routine(&typed, SymbolKind::Type);
}

#[test]
fn workspace_typechecking_keeps_transitive_pkg_import_declaration_facts() {
    let root = unique_temp_dir("workspace_transitive_pkg_decls");
    let store_root = root.join("store");
    create_dir_all(&store_root).expect("Package store root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("store/core/build.fol", "name: core\nversion: 1.0.0\n"),
            (
                "store/core/build.fol",
                "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"core\",\n        version = \"1.0.0\",\n    });\n};\n",
            ),
            ("store/core/src/lib.fol", "typ[exp] Count: int;\n"),
            (
                "store/json/build.fol",
                "name: json\nversion: 1.0.0\ndep.core: pkg:core\n",
            ),
            (
                "store/json/build.fol",
                "pro[] build(): non = {\n    var build = .build();\n    build.meta({\n        name = \"json\",\n        version = \"1.0.0\",\n    });\n    build.add_dep({\n        alias = \"core\",\n        source = \"pkg\",\n        target = \"core\",\n    });\n};\n",
            ),
            (
                "store/json/src/lib.fol",
                concat!(
                    "use core: pkg = {\"core\"};\n",
                    "var[exp] answer: core::src::Count = 42;\n",
                    "fun[exp] bump(value: core::src::Count): core::src::Count = {\n",
                    "    return value + 1;\n",
                    "};\n",
                ),
            ),
            (
                "app/main.fol",
                "use json: pkg = {\"json\"};\nfun[] main(): int = {\n    return 0;\n};\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(
        &root,
        "app",
        ResolverConfig {
            std_root: None,
            package_store_root: Some(
                store_root
                    .to_str()
                    .expect("Package store root should be valid UTF-8")
                    .to_string(),
            ),
        },
    )
    .expect("Workspace entry typing should keep transitive pkg declaration facts");

    let (_answer_id, answer) = find_typed_symbol(&typed, "answer", SymbolKind::ValueBinding);
    let (_bump_id, bump) = find_typed_symbol(&typed, "bump", SymbolKind::Routine);

    assert!(matches!(
        typed
            .type_table()
            .get(answer.declared_type.expect("transitive imported values should keep declared types")),
        Some(CheckedType::Declared {
            name,
            kind: DeclaredTypeKind::Type,
            ..
        }) if name == "Count"
    ));

    let signature = match typed.type_table().get(
        bump.declared_type
            .expect("transitive imported routines should keep translated signatures"),
    ) {
        Some(CheckedType::Routine(signature)) => signature,
        other => panic!("expected transitive imported routine signature, got {other:?}"),
    };
    assert!(matches!(
        typed.type_table().get(signature.params[0]),
        Some(CheckedType::Declared {
            name,
            kind: DeclaredTypeKind::Type,
            ..
        }) if name == "Count"
    ));
    assert!(matches!(
        signature
            .return_type
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(CheckedType::Declared {
            name,
            kind: DeclaredTypeKind::Type,
            ..
        }) if name == "Count"
    ));
}

#[test]
fn build_typechecking_accepts_dependency_handle_method_calls() {
    let typed = typecheck_fixture_folder(&[(
        "build.fol",
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var dep = build.add_dep({\n",
            "        alias = \"core\",\n",
            "        source = \"pkg\",\n",
            "        target = \"core\",\n",
            "    });\n",
            "    var module = dep.module(\"root\");\n",
            "    var artifact = dep.artifact(\"corelib\");\n",
            "    var step = dep.step(\"check\");\n",
            "    var file = dep.file(\"config\");\n",
            "    var dir = dep.dir(\"assets\");\n",
            "    var path = dep.path(\"schema\");\n",
            "    var generated = dep.generated(\"bindings\");\n",
            "    return;\n",
            "};\n",
        ),
    )]);

    let _ = find_typed_symbol(&typed, "dep", SymbolKind::ValueBinding);
    let _ = find_typed_symbol(&typed, "module", SymbolKind::ValueBinding);
    let _ = find_typed_symbol(&typed, "artifact", SymbolKind::ValueBinding);
    let _ = find_typed_symbol(&typed, "step", SymbolKind::ValueBinding);
    let _ = find_typed_symbol(&typed, "file", SymbolKind::ValueBinding);
    let _ = find_typed_symbol(&typed, "dir", SymbolKind::ValueBinding);
    let _ = find_typed_symbol(&typed, "path", SymbolKind::ValueBinding);
    let _ = find_typed_symbol(&typed, "generated", SymbolKind::ValueBinding);
}

#[test]
fn workspace_expression_typing_keeps_plain_imported_value_types_in_bindings_returns_and_call_args()
{
    let root = unique_temp_dir("workspace_imported_value_contexts");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(
        &root,
        &[
            ("shared/lib.fol", "var[exp] answer: int = 42;\n"),
            (
                "app/main.fol",
                concat!(
                    "use shared: loc = {\"../shared\"};\n",
                    "fun[] echo(value: int): int = {\n",
                    "    return value;\n",
                    "};\n",
                    "fun[] main(): int = {\n",
                    "    var current: int = answer;\n",
                    "    var echoed: int = echo(answer);\n",
                    "    return answer;\n",
                    "};\n",
                ),
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("Workspace entry typing should accept plain imported values in all basic expression contexts");
    let references = find_typed_references(&typed, "answer", ReferenceKind::Identifier);

    assert_eq!(
        references.len(),
        3,
        "expected imported value references in binding, call argument, and return"
    );
    for reference in references {
        assert_eq!(
            typed.type_table().get(
                reference
                    .resolved_type
                    .expect("imported value references should keep a resolved type"),
            ),
            Some(&CheckedType::Builtin(BuiltinType::Int))
        );
    }
}

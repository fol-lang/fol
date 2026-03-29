use super::*;
use fol_parser::ast::StandardKind;
use fol_parser::parser::AstParser;
use fol_resolver::ResolverErrorKind;
use fol_resolver::SymbolKind;
use fol_stream::FileStream;

#[test]
fn standards_m2_protocols_lower_typed_standard_and_conformance_metadata() {
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
         };\n",
    )]);

    let standard_symbol = typed
        .resolved()
        .symbols
        .iter_with_ids()
        .find_map(|(symbol_id, symbol)| {
            (symbol.kind == SymbolKind::Standard && symbol.name == "geo").then_some(symbol_id)
        })
        .expect("typed fixture should keep the protocol standard symbol");
    let type_symbol = typed
        .resolved()
        .symbols
        .iter_with_ids()
        .find_map(|(symbol_id, symbol)| {
            (symbol.kind == SymbolKind::Type && symbol.name == "Rect").then_some(symbol_id)
        })
        .expect("typed fixture should keep the conforming type symbol");

    let standard = typed
        .typed_standard(standard_symbol)
        .expect("typed fixture should record protocol standard metadata");
    assert_eq!(standard.kind, StandardKind::Protocol);
    assert_eq!(standard.required_routines.len(), 1);
    assert_eq!(standard.required_routines[0].name, "area");

    let conformance = typed
        .typed_conformance(type_symbol)
        .expect("typed fixture should record type conformance metadata");
    assert_eq!(conformance.standard_symbol_ids, vec![standard_symbol]);
}

#[test]
fn standards_m2_reject_missing_required_routines_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("type 'Rect' does not satisfy standard 'geo': missing required routine 'area'")
    }));
}

#[test]
fn standards_m2_reject_incompatible_required_routine_signatures_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(scale: int): int = {\n\
             return scale;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error.message().contains("routine 'area' has incompatible signature")
            && error.message().contains("expected fun area(): int")
            && error.message().contains("found fun area(int): int")
    }));
}

#[test]
fn standards_m2_accept_exact_required_routines_with_extra_overloads() {
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
         fun (Rect)area(scale: int): int = {\n\
             return scale;\n\
         };\n",
    )]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_accept_multiple_required_routines_cleanly() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
             fun perimeter(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 1;\n\
         };\n\
         fun (Rect)perimeter(): int = {\n\
             return 4;\n\
         };\n",
    )]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_accept_multi_standard_conformance_on_one_type() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         std sized: pro = {\n\
             fun size(): int;\n\
         };\n\
         typ Rect()(geo, sized): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 1;\n\
         };\n\
         fun (Rect)size(): int = {\n\
             return 2;\n\
         };\n",
    )]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_reject_multi_standard_conformance_when_one_protocol_is_missing() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         std sized: pro = {\n\
             fun size(): int;\n\
         };\n\
         typ Rect()(geo, sized): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 1;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("type 'Rect' does not satisfy standard 'sized': missing required routine 'size'")
    }));
}

#[test]
fn standards_m2_reject_partial_multi_routine_conformance_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
             fun perimeter(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 1;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("type 'Rect' does not satisfy standard 'geo': missing required routine 'perimeter'")
    }));
}

#[test]
fn standards_m2_reject_multi_routine_conformance_with_one_mismatch_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
             fun perimeter(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 1;\n\
         };\n\
         fun (Rect)perimeter(scale: int): int = {\n\
             return scale;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error.message().contains("routine 'perimeter' has incompatible signature")
            && error.message().contains("expected fun perimeter(): int")
    }));
}

#[test]
fn standards_m2_accept_cross_file_protocol_conformance() {
    let typed = typecheck_fixture_folder(&[
        (
            "contracts.fol",
            "std geo: pro = {\n\
                 fun area(): int;\n\
             };\n",
        ),
        (
            "rect.fol",
            "typ Rect()(geo): rec = {\n\
                 var width: int;\n\
             };\n\
             fun (Rect)area(): int = {\n\
                 return 1;\n\
             };\n",
        ),
    ]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_reject_cross_file_protocol_signature_mismatches() {
    let errors = typecheck_fixture_folder_errors(&[
        (
            "contracts.fol",
            "std geo: pro = {\n\
                 fun area(): int;\n\
             };\n",
        ),
        (
            "rect.fol",
            "typ Rect()(geo): rec = {\n\
                 var width: int;\n\
             };\n\
             fun (Rect)area(scale: int): int = {\n\
                 return scale;\n\
             };\n",
        ),
    ]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error.message().contains("routine 'area' has incompatible signature")
    }));
}

#[test]
fn standards_m2_exact_match_overload_ambiguity_stops_at_resolver_duplicate_boundary() {
    let root = unique_temp_dir("standards_m2_resolver_duplicate_boundary");
    write_fixture_files(
        &root,
        &[(
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
             pro (Rect)area(): int = {\n\
                 return 2;\n\
             };\n",
        )],
    );

    let package_root = root;
    let mut file_stream = FileStream::from_folder(
        package_root
            .to_str()
            .expect("fixture source directory should be valid UTF-8"),
    )
    .expect("fixture source directory should be readable");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut file_stream);
    let syntax = AstParser::new()
        .parse_package(&mut lexer)
        .expect("fixture should parse before resolver duplicate checks");
    let errors = fol_resolver::resolve_package(syntax)
        .expect_err("duplicate exact-match receiver routines should stop at resolver boundary");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::DuplicateSymbol
            && error.to_string().contains("duplicate symbol 'area'")
    }));
}

#[test]
fn standards_m2_accept_defaulted_overloads_when_one_exact_match_exists() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(scale: int): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(scale: int): int = {\n\
             return scale;\n\
         };\n\
         fun (Rect)area(scale: int, extra: int = 1): int = {\n\
             return scale + extra;\n\
         };\n",
    )]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_accept_aliases_imported_records_and_memo_types_in_legal_conformance() {
    let root = unique_temp_dir("standards_m2_workspace_mix");
    std::fs::create_dir_all(root.join("shared")).expect("shared fixture directory should exist");
    std::fs::create_dir_all(root.join("app")).expect("app fixture directory should exist");
    write_fixture_files(
        &root,
        &[
            (
                "shared/lib.fol",
                "typ[exp] RemoteUser: rec = { var name: str; };\n",
            ),
            (
                "app/main.fol",
                "use shared: loc = {\"../shared\"};\n\
                 ali Count: int;\n\
                 std geo: pro = {\n\
                     fun size(): Count;\n\
                     fun owner(): RemoteUser;\n\
                     fun label(): str;\n\
                 };\n\
                 typ Rect()(geo): rec = { var width: int; };\n\
                 fun (Rect)size(): Count = { return 1; };\n\
                 fun (Rect)owner(): RemoteUser = { return { name = \"remote\" }; };\n\
                 fun (Rect)label(): str = { return \"rect\"; };\n",
            ),
        ],
    );

    let typed = typecheck_fixture_workspace_entry_with_config(&root, "app", ResolverConfig::default())
        .expect("workspace standards fixture should typecheck");
    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_accept_multi_file_conformance_with_extra_unrelated_routines_and_overloads() {
    let typed = typecheck_fixture_folder(&[
        (
            "contracts.fol",
            "std geo: pro = {\n\
                 fun area(): int;\n\
             };\n",
        ),
        (
            "rect.fol",
            "typ Rect()(geo): rec = {\n\
                 var width: int;\n\
             };\n\
             fun (Rect)area(): int = {\n\
                 return 1;\n\
             };\n\
             fun (Rect)area(scale: int): int = {\n\
                 return scale;\n\
             };\n\
             fun (Rect)perimeter(): int = {\n\
                 return 4;\n\
             };\n",
        ),
    ]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_reject_claims_against_unsupported_standard_kinds_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std shape: blu = {\n\
             var area: int;\n\
         };\n\
         typ Rect()(shape): rec = {\n\
             var width: int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error.message().contains(
                "type 'Rect' claims unsupported standard 'shape'; only protocol standards are supported in V2 Milestone 2",
            )
    }));
}

#[test]
fn standards_m2_reject_generic_required_routine_signatures_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(T)(value: T): int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("generic standard routine requirements are not yet supported in V2 Milestone 2")
    }));
}

#[test]
fn standards_m2_reject_receiver_qualified_required_routine_signatures_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "typ Rect: rec = { var width: int; };\n\
         std geo: pro = {\n\
             fun (Rect)area(): int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("receiver-qualified standard requirements are not yet supported in V2 Milestone 2")
    }));
}

#[test]
fn standards_m2_reject_capturing_required_routine_signatures_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area()[cache]: int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("capturing standard routine requirements are not yet supported in V2 Milestone 2")
    }));
}

#[test]
fn standards_m2_reject_standards_as_ordinary_types_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         fun use(value: geo): int = {\n\
             return 1;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("standard 'geo' cannot be used as an ordinary type in V2 Milestone 2")
    }));
}

#[test]
fn standards_m2_reject_standards_as_ordinary_types_across_more_contexts() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Wrapper: rec = {\n\
             var item: geo;\n\
         };\n\
         fun produce(value: geo): geo = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             var local: geo = produce(panic(\"boom\"));\n\
             return 1;\n\
         };\n",
    )]);

    let count = errors
        .iter()
        .filter(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("standard 'geo' cannot be used as an ordinary type in V2 Milestone 2")
        })
        .count();
    assert!(
        count >= 3,
        "standards-as-types rejection should cover more than one surface, got: {errors:?}"
    );
}

#[test]
fn standards_m2_reject_generic_constraints_that_use_standards() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         fun pick(T: geo)(value: T): T = {\n\
             return value;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("generic routine constraints are not yet supported in V2 Milestone 1")
    }));
}

#[test]
fn standards_m2_reject_implementation_dispatch_surfaces_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect: rec = {\n\
             var width: int;\n\
         };\n\
         imp Self: geo = {\n\
             fun area(): int = {\n\
                 return 1;\n\
             };\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("implementation declarations are planned for a future release")
    }));
}

#[test]
fn standards_m2_reject_unsupported_protocol_member_shapes_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             ali Area: int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error.message().contains(
                "protocol standards currently support only required routine signatures in V2 Milestone 2",
            )
    }));
}

#[test]
fn standards_m2_reject_default_protocol_implementations_cleanly() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int = {\n\
                 return 1;\n\
             };\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("default standard routine implementations are not yet supported in V2 Milestone 2")
    }));
}

#[test]
fn standards_m2_boundary_rejects_blueprint_and_extended_standards() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std shape: blu = {\n\
             var size: int;\n\
         };\n\
         std display: ext = {\n\
             fun draw(): int;\n\
             var color: int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("blueprint standards are planned for a future release")
    }));
    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::Unsupported
            && error
                .message()
                .contains("extended standards are planned for a future release")
    }));
}

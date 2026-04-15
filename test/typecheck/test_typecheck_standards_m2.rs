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

    let typed_standards = typed.all_typed_standards().collect::<Vec<_>>();
    assert_eq!(
        typed_standards.len(),
        1,
        "standard declarations should record typed standard metadata instead of disappearing"
    );
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
            && error.message().contains("expected fun area(): int")
    }));
}

#[test]
fn standards_m2_missing_required_routine_diagnostic_includes_multi_param_signature() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun scale(factor: int, offset: int): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("missing required routine 'scale'")
            && error
                .message()
                .contains("expected fun scale(int, int): int")
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
            && error.message().contains("expected fun area(): int")
    }));
}

#[test]
fn standards_m2_reject_cross_file_missing_required_routine_with_expected_signature() {
    let errors = typecheck_fixture_folder_errors(&[
        (
            "contracts.fol",
            "std geo: pro = {\n\
                 fun perimeter(): int;\n\
             };\n",
        ),
        (
            "rect.fol",
            "typ Rect()(geo): rec = {\n\
                 var width: int;\n\
             };\n",
        ),
    ]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("type 'Rect' does not satisfy standard 'geo': missing required routine 'perimeter'")
            && error.message().contains("expected fun perimeter(): int")
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
fn standards_m2_accept_blueprint_conformer_with_matching_field() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std shape: blu = {\n\
             var size: int;\n\
         };\n\
         typ Rect()(shape): rec = {\n\
             var size: int;\n\
         };\n",
    )]);

    let (_rect_id, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_reject_blueprint_conformer_with_wrong_field_type() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std shape: blu = {\n\
             var size: int;\n\
         };\n\
         typ Rect()(shape): rec = {\n\
             var size: bol;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("field 'size' has incompatible type; expected int, found bol")
    }));
}

#[test]
fn standards_m2_reject_blueprint_conformer_missing_required_field() {
    // Blueprint standards now ship in V2 — the conformer must declare
    // a field with the required name and type. A conformer with only
    // an unrelated field should fail conformance with a clean message.
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
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error.message().contains(
                "type 'Rect' does not satisfy blueprint standard 'shape': missing required field 'area: int'",
            )
    }));
}

fn parse_standards_fixture_errors(source: &str) -> Vec<fol_diagnostics::Diagnostic> {
    let root = unique_temp_dir("standards_m2_parse_error");
    write_fixture_files(&root, &[("main.fol", source)]);
    let mut file_stream = FileStream::from_folder(
        root.to_str()
            .expect("fixture source directory should be valid UTF-8"),
    )
    .expect("fixture source directory should be readable");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut file_stream);
    AstParser::new()
        .parse_package(&mut lexer)
        .expect_err("standards fixture should fail to parse")
}

#[test]
fn standards_m2_generic_required_routine_signatures_removed_from_grammar() {
    let errors = parse_standards_fixture_errors(
        "std geo: pro = {\n\
             fun area(T)(value: T): int;\n\
         };\n",
    );
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("Standard routine requirements cannot declare their own generic parameters")),
        "generic required routines should fail at parse time, got: {errors:?}"
    );
}

#[test]
fn standards_m2_receiver_qualified_required_routine_signatures_removed_from_grammar() {
    let errors = parse_standards_fixture_errors(
        "typ Rect: rec = { var width: int; };\n\
         std geo: pro = {\n\
             fun (Rect)area(): int;\n\
         };\n",
    );
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("Standard routine requirements cannot declare a receiver")),
        "receiver-qualified required routines should fail at parse time, got: {errors:?}"
    );
}

#[test]
fn standards_m2_capturing_required_routine_signatures_removed_from_grammar() {
    let errors = parse_standards_fixture_errors(
        "std geo: pro = {\n\
             fun area()[cache]: int;\n\
         };\n",
    );
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("Standard routine requirements cannot declare captures")),
        "capturing required routines should fail at parse time, got: {errors:?}"
    );
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
                .contains("standard 'geo' is a static contract, not a value type; use it as a generic constraint instead")
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
                    .contains("standard 'geo' is a static contract, not a value type; use it as a generic constraint instead")
        })
        .count();
    assert!(
        count >= 3,
        "standards-as-types rejection should cover more than one surface, got: {errors:?}"
    );
}

#[test]
fn standards_m2_reject_nonconforming_generic_constraints() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Plain(): rec = {\n\
             var width: int;\n\
         };\n\
         fun pick(T: geo)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[] main(): int = {\n\
             var plain: Plain = { width = 1 };\n\
             pick(plain);\n\
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
                .contains("add an explicit conformance header for 'geo' on 'Plain'")
    }));
}

#[test]
fn standards_m2_reject_nonconforming_generic_constraint_from_imported_standard() {
    let errors = typecheck_fixture_folder_errors(&[
        (
            "contracts.fol",
            "std geo: pro = {\n\
                 fun area(): int;\n\
             };\n",
        ),
        (
            "main.fol",
            "typ Plain(): rec = {\n\
                 var width: int;\n\
             };\n\
             fun pick(T: geo)(value: T): T = {\n\
                 return value;\n\
             };\n\
             fun[] main(): int = {\n\
                 var plain: Plain = { width = 1 };\n\
                 pick(plain);\n\
                 return 0;\n\
             };\n",
        ),
    ]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("requires type 'Plain' to satisfy standard 'geo'")
            && error
                .message()
                .contains("add an explicit conformance header for 'geo' on 'Plain'")
    }));
}

#[test]
fn standards_m2_reject_nonconforming_generic_constraint_when_two_standards_are_in_scope() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         std sized: pro = {\n\
             fun size(): int;\n\
         };\n\
         typ Plain(): rec = {\n\
             var width: int;\n\
         };\n\
         fun measure(T: sized)(value: T): int = {\n\
             return 0;\n\
         };\n\
         fun[] main(): int = {\n\
             var plain: Plain = { width = 1 };\n\
             measure(plain);\n\
             return 0;\n\
         };\n",
    )]);

    assert!(errors.iter().any(|error| {
        error.kind() == TypecheckErrorKind::IncompatibleType
            && error
                .message()
                .contains("requires type 'Plain' to satisfy standard 'sized'")
            && !error.message().contains("standard 'geo'")
    }));
}

#[test]
fn standards_m2_imp_implementation_blocks_are_removed_from_grammar() {
    let root = unique_temp_dir("standards_m2_imp_removed_from_grammar");
    write_fixture_files(
        &root,
        &[(
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
        )],
    );

    let mut file_stream = FileStream::from_folder(
        root.to_str()
            .expect("fixture source directory should be valid UTF-8"),
    )
    .expect("fixture source directory should be readable");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut file_stream);
    AstParser::new()
        .parse_package(&mut lexer)
        .expect_err("'imp' implementation blocks should no longer parse after V2 cleanup");
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
fn standards_m2_default_protocol_implementation_records_has_default_body() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int = {\n\
                 return 1;\n\
             };\n\
         };\n",
    )]);

    let standard = typed
        .all_typed_standards()
        .next()
        .expect("default-body fixture should record a typed standard");
    assert_eq!(standard.required_routines.len(), 1);
    assert!(
        standard.required_routines[0].has_default_body,
        "default body should flip has_default_body on the required routine"
    );
}

#[test]
fn standards_m2_default_protocol_implementation_inherited_when_conformer_skips_routine() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int = {\n\
                 return 1;\n\
             };\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n",
    )]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(
        rect.declared_type.is_some(),
        "conformer without its own area should inherit the standard default body"
    );
}

#[test]
fn standards_m2_default_protocol_implementation_overridden_by_exact_conformer_routine() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int = {\n\
                 return 1;\n\
             };\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 4;\n\
         };\n",
    )]);

    let (_type_symbol, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_default_protocol_implementation_dispatches_at_call_site() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int = {\n\
                 return 1;\n\
             };\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n\
         fun[] main(): int = {\n\
             var r: Rect = { width = 2 };\n\
             return r.area();\n\
         };\n",
    )]);

    let main = find_named_routine_syntax_id(&typed, "main");
    assert_eq!(
        typed
            .typed_node(main)
            .and_then(|node| node.inferred_type)
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Builtin(BuiltinType::Int)),
        "calling a default-inherited routine should type through method resolution"
    );
}

#[test]
fn standards_m2_default_protocol_implementation_still_requires_signature_match_when_conformer_overrides() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int = {\n\
                 return 1;\n\
             };\n\
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
            && error
                .message()
                .contains("routine 'area' has incompatible signature")
    }), "override with wrong signature should still fail: {errors:?}");
}

#[test]
fn standards_o_generic_standard_with_int_type_arg_typechecks_cleanly() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std Iterator(T): pro = {\n\
             fun next(): T;\n\
         };\n\
         typ IntIter()(Iterator[int]): rec = {\n\
             var value: int;\n\
         };\n\
         fun (IntIter)next(): int = {\n\
             return 1;\n\
         };\n",
    )]);

    let (_iter_id, iter) = find_typed_symbol(&typed, "IntIter", SymbolKind::Type);
    assert!(iter.declared_type.is_some());
}

#[test]
fn standards_o_generic_standard_rejects_wrong_return_type_at_conformance() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std Iterator(T): pro = {\n\
             fun next(): T;\n\
         };\n\
         typ IntIter()(Iterator[int]): rec = {\n\
             var value: int;\n\
         };\n\
         fun (IntIter)next(): str = {\n\
             return \"oops\";\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("routine 'next' has incompatible signature")
            && error.message().contains("expected fun next(): int")),
        "conformer with wrong substituted return type should fail: {errors:?}"
    );
}

#[test]
fn standards_o_generic_standard_rejects_arity_mismatch_in_conformance_header() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std Iterator(T): pro = {\n\
             fun next(): T;\n\
         };\n\
         typ IntIter()(Iterator[int, str]): rec = {\n\
             var value: int;\n\
         };\n\
         fun (IntIter)next(): int = {\n\
             return 1;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("claims generic standard 'Iterator' with 2 type argument(s) but the standard expects 1")),
        "arity mismatch in conformance header should fail: {errors:?}"
    );
}

#[test]
fn standards_m2_accept_extended_conformance_with_both_routine_and_field() {
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std drawable: ext = {\n\
             fun draw(): int;\n\
             var color: int;\n\
         };\n\
         typ Rect()(drawable): rec = {\n\
             var color: int;\n\
         };\n\
         fun (Rect)draw(): int = {\n\
             return 1;\n\
         };\n",
    )]);

    let (_rect_id, rect) = find_typed_symbol(&typed, "Rect", SymbolKind::Type);
    assert!(rect.declared_type.is_some());
}

#[test]
fn standards_m2_reject_extended_conformance_missing_routine_side() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std drawable: ext = {\n\
             fun draw(): int;\n\
             var color: int;\n\
         };\n\
         typ Rect()(drawable): rec = {\n\
             var color: int;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("missing required routine 'draw'")),
        "extended conformance missing routine should still fire routine diagnostic: {errors:?}"
    );
}

#[test]
fn standards_m2_reject_extended_conformance_missing_field_side() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std drawable: ext = {\n\
             fun draw(): int;\n\
             var color: int;\n\
         };\n\
         typ Rect()(drawable): rec = {\n\
             var width: int;\n\
         };\n\
         fun (Rect)draw(): int = {\n\
             return 1;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| error
            .message()
            .contains("missing required field 'color: int'")),
        "extended conformance missing field should still fire blueprint diagnostic: {errors:?}"
    );
}

#[test]
fn standards_m2_accept_blueprint_and_extended_standalone_declarations() {
    // Blueprint and extended standards are now both part of the shipped
    // V2 contract. Standalone declarations without conformers should
    // typecheck cleanly.
    let typed = typecheck_fixture_folder(&[(
        "main.fol",
        "std shape: blu = {\n\
             var size: int;\n\
         };\n\
         std display: ext = {\n\
             fun draw(): int;\n\
             var color: int;\n\
         };\n",
    )]);
    assert_eq!(typed.all_typed_standards().count(), 2);
}

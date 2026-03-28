use super::*;
use fol_parser::ast::StandardKind;
use fol_resolver::SymbolKind;

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

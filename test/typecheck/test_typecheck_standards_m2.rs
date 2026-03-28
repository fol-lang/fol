use super::*;

#[test]
fn standards_m2_boundary_rejects_protocol_standards_and_type_conformance() {
    let errors = typecheck_fixture_folder_errors(&[(
        "main.fol",
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n",
    )]);

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("protocol standards are planned for a future release")
        }),
        "Expected protocol standards to stay outside current semantics, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("type contract conformance is planned for a future release")
        }),
        "Expected type contract conformance to stay outside current semantics, got: {errors:?}"
    );
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

    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("blueprint standards are planned for a future release")
        }),
        "Expected blueprint standards to stay outside current semantics, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|error| {
            error.kind() == TypecheckErrorKind::Unsupported
                && error
                    .message()
                    .contains("extended standards are planned for a future release")
        }),
        "Expected extended standards to stay outside current semantics, got: {errors:?}"
    );
}

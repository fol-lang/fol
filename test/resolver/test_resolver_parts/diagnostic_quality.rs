use super::{try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::ResolverErrorKind;
use std::fs;

#[test]
fn test_resolver_unresolved_qualified_type_diagnostics_keep_exact_role_and_location() {
    let temp_root = unique_temp_root("resolver_diag_qualified_type");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("main.fol"), "ali Broken: tools::Missing;\n")
        .expect("Should write the unresolved qualified type fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unresolved qualified type references");
    let error = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::UnresolvedName)
        .expect("Resolver should report an unresolved-name error");
    let origin = error
        .origin()
        .expect("Qualified unresolved type diagnostics should keep exact syntax origins");

    assert!(
        error
            .to_string()
            .contains("could not resolve qualified type 'tools::Missing'"),
        "Resolver should report the exact unresolved role and name"
    );
    assert_eq!(
        origin.file.as_deref(),
        Some(
            temp_root
                .join("main.fol")
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        ),
        "Qualified unresolved diagnostics should keep the exact source file"
    );
    assert_eq!(
        origin.line, 1,
        "Qualified unresolved diagnostics should keep the exact line"
    );
    assert_eq!(
        origin.column, 13,
        "Qualified unresolved diagnostics should point at the qualified type root token"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_unresolved_named_inquiry_target_diagnostics_use_target_role() {
    let temp_root = unique_temp_root("resolver_diag_inquiry_target");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    return 0;\n    where(cache) {\n        0;\n    };\n};\n",
    )
    .expect("Should write the unresolved inquiry target fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unresolved inquiry targets");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error
                    .to_string()
                    .contains("could not resolve inquiry target 'cache'")
        }),
        "Resolver should report the exact unresolved inquiry-target role"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_private_imported_symbols_with_the_export_hint() {
    // A qualified path through an import alias may only land on exported
    // declarations; resolving a private one here would greenlight code the
    // workspace build (which mounts exports only) later rejects.
    let temp_root = unique_temp_root("resolver_diag_private_import");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("shared/lib.fol"),
        "fun hidden(): int = {\n    return 1;\n};\n",
    )
    .expect("Should write the private imported routine fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use shared: loc = {\"shared\"};\nfun[] main(): int = {\n    return shared::hidden();\n};\n",
    )
    .expect("Should write the private import call fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject private imported symbols");
    let error = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::UnresolvedName)
        .expect("Resolver should report an unresolved-name error");
    assert!(
        error.message().contains("not exported") && error.message().contains("'[exp]'"),
        "private import diagnostics should carry the export hint: {}",
        error.message()
    );
}

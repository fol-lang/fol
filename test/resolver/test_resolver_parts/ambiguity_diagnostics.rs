use super::{try_resolve_package_from_folder, unique_temp_root};
use fol_diagnostics::ToDiagnostic;
use fol_resolver::ResolverErrorKind;
use std::fs;

#[test]
fn test_resolver_duplicate_top_level_diagnostics_include_first_declaration_site() {
    let temp_root = unique_temp_root("duplicate_symbol_sites");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    let first_file = temp_root.join("00_first.fol");
    let second_file = temp_root.join("10_second.fol");
    fs::write(&first_file, "var value: int = 1;\n").expect("Should write first duplicate fixture");
    fs::write(&second_file, "var value: int = 2;\n")
        .expect("Should write second duplicate fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Duplicate top-level symbols should fail resolver collection");
    let error = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::DuplicateSymbol)
        .expect("Resolver should report a duplicate symbol error");

    assert!(
        error
            .to_string()
            .contains("existing value binding declaration first declared at"),
        "Duplicate diagnostics should mention the conflicting declaration site"
    );
    assert!(
        error.to_string().contains(
            first_file
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        ),
        "Duplicate diagnostics should include the first declaration file path"
    );
    assert!(
        error.to_string().contains(":1:5"),
        "Duplicate diagnostics should include the first declaration line and column"
    );
    assert_eq!(
        error.origin().and_then(|origin| origin.file.as_deref()),
        Some(
            second_file
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        ),
        "Duplicate diagnostics should still point at the conflicting declaration as the primary site"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_duplicate_top_level_diagnostics_lower_first_site_as_secondary_label() {
    let temp_root = unique_temp_root("duplicate_symbol_secondary_labels");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    let first_file = temp_root.join("00_first.fol");
    let second_file = temp_root.join("10_second.fol");
    fs::write(&first_file, "var value: int = 1;\n").expect("Should write first duplicate fixture");
    fs::write(&second_file, "var value: int = 2;\n")
        .expect("Should write second duplicate fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Duplicate top-level symbols should fail resolver collection");
    let diagnostic = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::DuplicateSymbol)
        .expect("Resolver should report a duplicate symbol error")
        .to_diagnostic();

    assert_eq!(diagnostic.labels.len(), 2);
    assert_eq!(
        diagnostic.labels[1].location.file.as_deref(),
        Some(
            first_file
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        )
    );
    assert_eq!(
        diagnostic.labels[1].message.as_deref(),
        Some("first value binding declaration")
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_ambiguous_plain_calls_include_candidate_sites() {
    let temp_root = unique_temp_root("ambiguous_call_sites");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    let first_file = temp_root.join("00_first.fol");
    let second_file = temp_root.join("10_second.fol");
    fs::write(
        &first_file,
        "fun[] helper(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the first overload fixture");
    fs::write(
        &second_file,
        "fun[] helper(value: seq[int]): int = {\n    return 0;\n};\n\nfun[] main(values: seq[int]): int = {\n    return helper(values);\n};\n",
    )
    .expect("Should write the second overload fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Overloaded plain calls should be ambiguous without type-directed resolution");
    let error = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::AmbiguousReference)
        .expect("Resolver should report an ambiguous reference error");

    assert!(
        error
            .to_string()
            .contains("callable routine 'helper' is ambiguous"),
        "Ambiguous callable diagnostics should keep the callable role wording"
    );
    assert!(
        error.to_string().contains(
            first_file
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        ) && error.to_string().contains(
            second_file
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        ),
        "Ambiguous callable diagnostics should include both candidate declaration sites"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_ambiguous_plain_calls_lower_candidate_sites_as_secondary_labels() {
    let temp_root = unique_temp_root("ambiguous_call_secondary_labels");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    let first_file = temp_root.join("00_first.fol");
    let second_file = temp_root.join("10_second.fol");
    fs::write(
        &first_file,
        "fun[] helper(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the first overload fixture");
    fs::write(
        &second_file,
        "fun[] helper(value: seq[int]): int = {\n    return 0;\n};\n\nfun[] main(values: seq[int]): int = {\n    return helper(values);\n};\n",
    )
    .expect("Should write the second overload fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Overloaded plain calls should be ambiguous without type-directed resolution");
    let diagnostic = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::AmbiguousReference)
        .expect("Resolver should report an ambiguous reference error")
        .to_diagnostic();

    assert_eq!(diagnostic.labels.len(), 3);
    let secondary_files = diagnostic.labels[1..]
        .iter()
        .map(|label| {
            label
                .location
                .file
                .as_deref()
                .unwrap_or_default()
                .to_string()
        })
        .collect::<Vec<_>>();
    assert!(secondary_files
        .iter()
        .any(|file| file == first_file.to_string_lossy().as_ref()));
    assert!(secondary_files
        .iter()
        .any(|file| file == second_file.to_string_lossy().as_ref()));
    assert!(diagnostic.labels[1..]
        .iter()
        .all(|label| label.message.as_deref() == Some("candidate routine declaration")));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_ambiguity_diagnostics_keep_primary_use_site_before_candidate_labels() {
    let temp_root = unique_temp_root("ambiguous_call_label_order");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    let first_file = temp_root.join("00_first.fol");
    let second_file = temp_root.join("10_second.fol");
    fs::write(
        &first_file,
        "fun[] helper(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the first overload fixture");
    fs::write(
        &second_file,
        "fun[] helper(value: seq[int]): int = {\n    return 0;\n};\n\nfun[] main(values: seq[int]): int = {\n    return helper(values);\n};\n",
    )
    .expect("Should write the second overload fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Overloaded plain calls should be ambiguous without type-directed resolution");
    let diagnostic = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::AmbiguousReference)
        .expect("Resolver should report an ambiguous reference error")
        .to_diagnostic();

    assert_eq!(
        diagnostic.labels[0].location.file.as_deref(),
        second_file.to_str()
    );
    assert_eq!(diagnostic.labels[0].location.line, 6);
    assert_eq!(diagnostic.labels[0].message, None);
    assert_eq!(
        diagnostic.labels[1].location.file.as_deref(),
        first_file.to_str()
    );
    assert_eq!(
        diagnostic.labels[1].message.as_deref(),
        Some("candidate routine declaration")
    );
    assert_eq!(
        diagnostic.labels[2].location.file.as_deref(),
        second_file.to_str()
    );
    assert_eq!(
        diagnostic.labels[2].message.as_deref(),
        Some("candidate routine declaration")
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

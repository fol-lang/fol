use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_resolves_qualified_identifiers_through_namespace_roots() {
    let temp_root = unique_temp_root("qualified_identifier_namespace");
    fs::create_dir_all(temp_root.join("tools"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("tools/values.fol"),
        "var answer: int = 42;\n",
    )
    .expect("Should write the qualified namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    return tools::answer;\n};\n",
    )
    .expect("Should write the qualified namespace lookup fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let package = temp_root
        .file_name()
        .expect("Temp resolver root should have a folder name")
        .to_string_lossy()
        .to_string();
    let namespace_scope = resolved
        .namespace_scope(&format!("{package}::tools"))
        .expect("Resolver should create the imported namespace scope");
    let answer_symbol = resolved
        .symbols_in_scope(namespace_scope)
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Namespace scope should keep the qualified value binding");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "tools::answer"
        })
        .expect("Routine scope should record the qualified identifier reference");

    assert_eq!(
        answer_reference.resolved,
        Some(answer_symbol.id),
        "Qualified identifiers should resolve through same-package namespace roots"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_identifiers_through_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_identifier_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/values.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(): int = {\n    return math::answer;\n};\n",
    )
    .expect("Should write the qualified import-alias lookup fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "math")
        .expect("Program scope should keep the math import alias");
    let answer_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Imported namespace scope should keep the qualified value binding");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier && reference.name == "math::answer"
        })
        .expect("Routine scope should record the qualified import-alias reference");

    assert_eq!(
        answer_reference.resolved,
        Some(answer_symbol.id),
        "Qualified identifiers should resolve through imported namespace aliases"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_identifiers_through_non_matching_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_identifier_nonmatching_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/values.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use tools: loc = {\"math\"};\nfun[] main(): int = {\n    return tools::answer;\n};\n",
    )
    .expect("Should write the non-matching qualified import-alias lookup fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "tools")
        .expect("Program scope should keep the tools import alias");
    let answer_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Imported namespace scope should keep the qualified value binding");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "tools::answer"
        })
        .expect("Routine scope should record the qualified non-matching import-alias reference");

    assert_eq!(
        answer_reference.resolved,
        Some(answer_symbol.id),
        "Qualified identifiers should resolve through import aliases even when alias spelling differs from the namespace root"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_identifiers_through_non_matching_local_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_identifier_nonmatching_local_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/values.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    use tools: loc = {\"math\"};\n    return tools::answer;\n};\n",
    )
    .expect("Should write the non-matching local qualified import-alias lookup fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let import = resolved
        .imports_in_scope(routine_scope_id)
        .into_iter()
        .find(|import| import.alias_name == "tools")
        .expect("Routine scope should keep the local tools import alias");
    let answer_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Imported namespace scope should keep the qualified value binding");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedIdentifier
                && reference.name == "tools::answer"
        })
        .expect("Routine scope should record the qualified non-matching local import-alias reference");

    assert_eq!(
        answer_reference.resolved,
        Some(answer_symbol.id),
        "Qualified identifiers should resolve through local import aliases with non-matching spellings"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_unresolved_qualified_identifier_paths() {
    let temp_root = unique_temp_root("qualified_identifier_missing");
    fs::create_dir_all(temp_root.join("tools"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("tools/values.fol"),
        "var answer: int = 42;\n",
    )
    .expect("Should write the existing namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(): int = {\n    return tools::missing;\n};\n",
    )
    .expect("Should write the unresolved qualified identifier fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unresolved qualified identifier paths");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error
                    .to_string()
                    .contains("could not resolve qualified identifier 'tools::missing'")
        }),
        "Resolver should report unresolved qualified identifier diagnostics explicitly"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

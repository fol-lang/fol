use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_resolves_qualified_types_through_namespace_roots() {
    let temp_root = unique_temp_root("qualified_type_namespace");
    fs::create_dir_all(temp_root.join("tools"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("tools/types.fol"), "typ Answer: int;\n")
        .expect("Should write the qualified namespace type fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(value: tools::Answer): tools::Answer = {\n    var local: tools::Answer = value;\n    return local;\n};\n",
    )
    .expect("Should write the qualified namespace type lookup fixture");

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
        .expect("Resolver should create the tools namespace scope");
    let answer_symbol = resolved
        .symbols_in_scope(namespace_scope)
        .into_iter()
        .find(|symbol| symbol.name == "Answer" && symbol.kind == SymbolKind::Type)
        .expect("Namespace scope should keep the qualified type symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");

    assert_eq!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .filter(|reference| {
                reference.kind == ReferenceKind::QualifiedTypeName
                    && reference.name == "tools::Answer"
                    && reference.resolved == Some(answer_symbol.id)
            })
            .count(),
        3,
        "Routine signatures and local type hints should resolve qualified namespace types"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_types_through_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_type_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/types.fol"), "typ[exp] Number: int;\n")
        .expect("Should write the imported namespace type fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(value: math::Number): math::Number = {\n    var local: math::Number = value;\n    return local;\n};\n",
    )
    .expect("Should write the qualified import-alias type fixture");

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
    let number_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "Number" && symbol.kind == SymbolKind::Type)
        .expect("Imported namespace scope should keep the qualified type symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");

    assert_eq!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .filter(|reference| {
                reference.kind == ReferenceKind::QualifiedTypeName
                    && reference.name == "math::Number"
                    && reference.resolved == Some(number_symbol.id)
            })
            .count(),
        3,
        "Routine signatures and local type hints should resolve qualified imported types"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_types_through_non_matching_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_type_nonmatching_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/types.fol"), "typ[exp] Number: int;\n")
        .expect("Should write the imported namespace type fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use tools: loc = {\"math\"};\nfun[] main(value: tools::Number): tools::Number = {\n    var local: tools::Number = value;\n    return local;\n};\n",
    )
    .expect("Should write the qualified non-matching import-alias type fixture");

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
    let number_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "Number" && symbol.kind == SymbolKind::Type)
        .expect("Imported namespace scope should keep the qualified type symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");

    assert_eq!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .filter(|reference| {
                reference.kind == ReferenceKind::QualifiedTypeName
                    && reference.name == "tools::Number"
                    && reference.resolved == Some(number_symbol.id)
            })
            .count(),
        3,
        "Qualified types should resolve through import aliases even when alias spelling differs from the namespace root"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_types_through_non_matching_local_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_type_nonmatching_local_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/types.fol"), "typ[exp] Number: int;\n")
        .expect("Should write the imported namespace type fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(value: int): int = {\n    use tools: loc = {\"math\"};\n    var local: tools::Number = value;\n    return local;\n};\n",
    )
    .expect("Should write the qualified non-matching local import-alias type fixture");

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
    let number_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "Number" && symbol.kind == SymbolKind::Type)
        .expect("Imported namespace scope should keep the qualified type symbol");

    assert!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::QualifiedTypeName
                    && reference.name == "tools::Number"
                    && reference.resolved == Some(number_symbol.id)
            }),
        "Qualified types should resolve through local import aliases with non-matching spellings"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_unresolved_qualified_type_paths() {
    let temp_root = unique_temp_root("qualified_type_missing");
    fs::create_dir_all(temp_root.join("tools"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("tools/types.fol"), "typ Answer: int;\n")
        .expect("Should write the existing namespace type fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(value: tools::Missing): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the unresolved qualified type fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unresolved qualified type paths");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error
                    .to_string()
                    .contains("could not resolve qualified type 'tools::Missing'")
        }),
        "Resolver should report unresolved qualified type diagnostics explicitly"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

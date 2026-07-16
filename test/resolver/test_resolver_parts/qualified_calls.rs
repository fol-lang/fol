use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_resolves_qualified_calls_through_namespace_roots() {
    let temp_root = unique_temp_root("qualified_call_namespace");
    fs::create_dir_all(temp_root.join("tools"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("tools/helpers.fol"),
        "fun[] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the qualified namespace routine fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(input: int): int = {\n    return tools::emit(input);\n};\n",
    )
    .expect("Should write the qualified namespace call fixture");

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
    let emit_symbol = resolved
        .symbols_in_scope(namespace_scope)
        .into_iter()
        .find(|symbol| symbol.name == "emit" && symbol.kind == SymbolKind::Routine)
        .expect("Namespace scope should keep the qualified routine symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let emit_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedFunctionCall
                && reference.name == "tools::emit"
        })
        .expect("Routine scope should record the qualified call reference");

    assert_eq!(
        emit_reference.resolved,
        Some(emit_symbol.id),
        "Qualified calls should resolve through same-package namespace roots"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_calls_through_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_call_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("math/helpers.fol"),
        "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the imported namespace routine fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(input: int): int = {\n    return math::emit(input);\n};\n",
    )
    .expect("Should write the qualified import-alias call fixture");

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
    let emit_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "emit" && symbol.kind == SymbolKind::Routine)
        .expect("Imported namespace scope should keep the qualified routine symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let emit_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedFunctionCall && reference.name == "math::emit"
        })
        .expect("Routine scope should record the qualified import-alias call");

    assert_eq!(
        emit_reference.resolved,
        Some(emit_symbol.id),
        "Qualified calls should resolve through imported namespace aliases"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_calls_through_non_matching_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_call_nonmatching_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("math/helpers.fol"),
        "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the imported namespace routine fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use tools: loc = {\"math\"};\nfun[] main(input: int): int = {\n    return tools::emit(input);\n};\n",
    )
    .expect("Should write the qualified non-matching import-alias call fixture");

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
    let emit_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "emit" && symbol.kind == SymbolKind::Routine)
        .expect("Imported namespace scope should keep the qualified routine symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let emit_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedFunctionCall
                && reference.name == "tools::emit"
        })
        .expect("Routine scope should record the qualified non-matching import-alias call");

    assert_eq!(
        emit_reference.resolved,
        Some(emit_symbol.id),
        "Qualified calls should resolve through import aliases even when alias spelling differs from the namespace root"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_calls_through_non_matching_local_import_alias_roots() {
    let temp_root = unique_temp_root("qualified_call_nonmatching_local_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("math/helpers.fol"),
        "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the imported namespace routine fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(input: int): int = {\n    {\n        use tools: loc = {\"math\"};\n        return tools::emit(input);\n    };\n};\n",
    )
    .expect("Should write the qualified non-matching local import-alias call fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let block_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Block).then_some(scope_id))
        .expect("Resolver should create a block scope for the nested local import");
    let import = resolved
        .imports_in_scope(block_scope_id)
        .into_iter()
        .find(|import| import.alias_name == "tools")
        .expect("Block scope should keep the local tools import alias");
    let emit_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "emit" && symbol.kind == SymbolKind::Routine)
        .expect("Imported namespace scope should keep the qualified routine symbol");
    let emit_reference = resolved
        .references_in_scope(block_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::QualifiedFunctionCall
                && reference.name == "tools::emit"
        })
        .expect("Block scope should record the qualified non-matching local import-alias call");

    assert_eq!(
        emit_reference.resolved,
        Some(emit_symbol.id),
        "Qualified calls should resolve through local import aliases with non-matching spellings"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_unresolved_qualified_call_paths() {
    let temp_root = unique_temp_root("qualified_call_missing");
    fs::create_dir_all(temp_root.join("tools"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("tools/helpers.fol"),
        "fun[] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the existing namespace routine fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(input: int): int = {\n    return tools::missing(input);\n};\n",
    )
    .expect("Should write the unresolved qualified call fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unresolved qualified call paths");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error
                    .to_string()
                    .contains("could not resolve qualified callable routine 'tools::missing'")
        }),
        "Resolver should report unresolved qualified call diagnostics explicitly"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

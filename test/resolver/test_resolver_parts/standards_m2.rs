use super::{resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_keeps_top_level_standard_symbols_for_m2_inventory() {
    let temp_root = unique_temp_root("resolver_standards_m2_symbols");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "std geometry: pro = { fun area(): int; };\n\
         std shape: blu = { var size: int; };\n\
         std display: ext = { fun draw(): int; var color: int; };\n",
    )
    .expect("Should write standard-symbol resolver fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let symbols = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .filter(|symbol| symbol.kind == SymbolKind::Standard)
        .map(|symbol| symbol.name.clone())
        .collect::<Vec<_>>();

    assert!(symbols.contains(&"geometry".to_string()));
    assert!(symbols.contains(&"shape".to_string()));
    assert!(symbols.contains(&"display".to_string()));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_type_contract_headers_to_standard_symbols() {
    let temp_root = unique_temp_root("resolver_standards_m2_contract_refs");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "std geo: pro = { fun area(): int; };\n\
         typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n",
    )
    .expect("Should write standard contract resolver fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let standard_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "geo" && symbol.kind == SymbolKind::Standard)
        .expect("Program scope should keep the standard symbol");
    assert!(
        resolved
            .scopes
            .iter_with_ids()
            .filter_map(|(scope_id, scope)| {
                matches!(scope.kind, ScopeKind::TypeDeclaration).then_some(scope_id)
            })
            .flat_map(|scope_id| resolved.references_in_scope(scope_id).into_iter())
            .any(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "geo"
                    && reference.resolved == Some(standard_symbol.id)
            }),
        "Type-side contract headers should resolve to the declared standard symbol"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_binds_required_standard_routines_into_standard_scope() {
    let temp_root = unique_temp_root("resolver_standards_m2_scope_members");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "std geo: pro = {\n\
             fun area(): int;\n\
             pro ready(): bol;\n\
         };\n",
    )
    .expect("Should write standard-scope resolver fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let standard_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::StandardDeclaration).then_some(scope_id))
        .expect("Resolver should create a standard-declaration scope");
    let member_names = resolved
        .symbols_in_scope(standard_scope_id)
        .into_iter()
        .filter(|symbol| symbol.kind == SymbolKind::Routine)
        .map(|symbol| symbol.name.clone())
        .collect::<Vec<_>>();

    assert!(member_names.contains(&"area".to_string()));
    assert!(member_names.contains(&"ready".to_string()));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_duplicate_top_level_standard_names_cleanly() {
    let temp_root = unique_temp_root("resolver_standards_m2_duplicate_names");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "std geo: pro = { fun area(): int; };\n\
         std geo: pro = { fun area(): int; };\n",
    )
    .expect("Should write duplicate-standard resolver fixture");

    let errors = super::try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject duplicate top-level standard names");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::DuplicateSymbol
            && error.to_string().contains("duplicate symbol 'geo'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_unknown_standard_contract_headers_cleanly() {
    let temp_root = unique_temp_root("resolver_standards_m2_missing_contract");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "typ Rect()(geo): rec = {\n\
             var width: int;\n\
         };\n",
    )
    .expect("Should write missing-contract resolver fixture");

    let errors = super::try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unknown standard contract headers");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::UnresolvedName
            && error.to_string().contains("could not resolve standard 'geo'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_non_standard_contract_headers_cleanly() {
    let temp_root = unique_temp_root("resolver_standards_m2_non_standard_contract");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "typ Geo: rec = {\n\
             var width: int;\n\
         };\n\
         typ Rect()(Geo): rec = {\n\
             var width: int;\n\
         };\n",
    )
    .expect("Should write non-standard contract resolver fixture");

    let errors = super::try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject non-standard contract headers");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::UnresolvedName
            && error.to_string().contains("could not resolve standard 'Geo'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_multiple_standard_declarations_and_conformance_claims_in_one_source_unit() {
    let temp_root = unique_temp_root("resolver_standards_m2_multi_one_file");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "std geo: pro = { fun area(): int; };\n\
         std sized: pro = { fun size(): int; };\n\
         typ Rect()(geo, sized): rec = { var width: int; };\n\
         fun (Rect)area(): int = { return 1; };\n\
         fun (Rect)size(): int = { return 2; };\n",
    )
    .expect("Should write multi-standard resolver fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let type_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::TypeDeclaration).then_some(scope_id))
        .expect("Resolver should create a type scope");
    let contract_refs = resolved
        .references_in_scope(type_scope_id)
        .into_iter()
        .filter(|reference| reference.kind == ReferenceKind::TypeName)
        .map(|reference| reference.name.clone())
        .collect::<Vec<_>>();

    assert!(contract_refs.contains(&"geo".to_string()));
    assert!(contract_refs.contains(&"sized".to_string()));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_standard_declarations_and_conformance_claims_across_files() {
    let temp_root = unique_temp_root("resolver_standards_m2_multi_files");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("00_standards.fol"), "std geo: pro = { fun area(): int; };\n")
        .expect("Should write standard fixture");
    fs::write(
        temp_root.join("10_types.fol"),
        "typ Rect()(geo): rec = { var width: int; };\n\
         fun (Rect)area(): int = { return 1; };\n",
    )
    .expect("Should write type fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let standard_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.kind == SymbolKind::Standard && symbol.name == "geo")
        .expect("Program scope should keep the standard symbol");
    assert!(resolved.scopes.iter_with_ids().any(|(scope_id, scope)| {
        matches!(scope.kind, ScopeKind::TypeDeclaration)
            && resolved.references_in_scope(scope_id).into_iter().any(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "geo"
                    && reference.resolved == Some(standard_symbol.id)
            })
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_conformance_claims_against_imported_standards() {
    let temp_root = unique_temp_root("resolver_standards_m2_imported");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create a temporary shared package fixture directory");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create a temporary app package fixture directory");
    fs::write(
        temp_root.join("shared/lib.fol"),
        "std[export] geo: pro = { fun area(): int; };\n",
    )
    .expect("Should write shared standard fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use shared: loc = {\"../shared\"};\n\
         typ Rect()(geo): rec = { var width: int; };\n\
         fun (Rect)area(): int = { return 1; };\n",
    )
    .expect("Should write importing app fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "shared")
        .expect("Program scope should keep the shared import");
    let imported_standard = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.kind == SymbolKind::Standard && symbol.name == "geo")
        .expect("Imported scope should keep the exported standard");

    assert!(resolved.scopes.iter_with_ids().any(|(scope_id, scope)| {
        matches!(scope.kind, ScopeKind::TypeDeclaration)
            && resolved.references_in_scope(scope_id).into_iter().any(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "geo"
                    && reference.resolved == Some(imported_standard.id)
            })
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_case_insensitive_standard_contract_headers() {
    let temp_root = unique_temp_root("resolver_standards_m2_case_mismatch");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "std geo: pro = { fun area(): int; };\n\
         typ Rect()(Geo): rec = { var width: int; };\n",
    )
    .expect("Should write standard case-mismatch fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let standard_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.kind == SymbolKind::Standard && symbol.name == "geo")
        .expect("Program scope should keep the standard symbol");

    assert!(resolved.scopes.iter_with_ids().any(|(scope_id, scope)| {
        matches!(scope.kind, ScopeKind::TypeDeclaration)
            && resolved.references_in_scope(scope_id).into_iter().any(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "Geo"
                    && reference.resolved == Some(standard_symbol.id)
            })
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_ambiguous_imported_standard_contract_headers() {
    let temp_root = unique_temp_root("resolver_standards_m2_import_ambiguity");
    fs::create_dir_all(temp_root.join("alpha"))
        .expect("Should create the first imported namespace fixture directory");
    fs::create_dir_all(temp_root.join("beta"))
        .expect("Should create the second imported namespace fixture directory");
    fs::write(
        temp_root.join("alpha/lib.fol"),
        "std[export] geo: pro = { fun area(): int; };\n",
    )
    .expect("Should write the first imported standard fixture");
    fs::write(
        temp_root.join("beta/lib.fol"),
        "std[export] geo: pro = { fun area(): int; };\n",
    )
    .expect("Should write the second imported standard fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use alpha: loc = {\"alpha\"};\n\
         use beta: loc = {\"beta\"};\n\
         typ Rect()(geo): rec = { var width: int; };\n",
    )
    .expect("Should write ambiguous imported standard fixture");

    let errors = super::try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject ambiguous imported standards");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::AmbiguousReference
            && error
                .to_string()
                .contains("standard 'geo' is ambiguous in lexical scope")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

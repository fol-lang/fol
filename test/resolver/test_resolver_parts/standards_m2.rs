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

use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_rejects_generic_parameter_references_outside_routine_scope() {
    let temp_root = unique_temp_root("generic_out_of_scope");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n    return value;\n};\nali Outside: T;\n",
    )
    .expect("Should write the out-of-scope generic fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject generic type names outside the routine scope");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::UnresolvedName
            && error.message().contains("could not resolve type 'T'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_binds_generic_parameters_into_nested_routine_scope() {
    let temp_root = unique_temp_root("nested_generic_scope");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n    fun copy(): T = {\n        return value;\n    };\n    return copy();\n};\n",
    )
    .expect("Should write the nested generic scope fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let routine_scopes = resolved
        .scopes
        .iter_with_ids()
        .filter_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .collect::<Vec<_>>();

    assert!(
        routine_scopes.iter().any(|scope_id| {
            resolved
                .symbols_in_scope(*scope_id)
                .into_iter()
                .any(|symbol| symbol.name == "T" && symbol.kind == SymbolKind::GenericParameter)
        }),
        "Expected at least one routine scope to retain the generic parameter symbol"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_ambiguous_generic_calls_explicitly() {
    let temp_root = unique_temp_root("ambiguous_generic_calls");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n    return value;\n};\n\
         fun pick(U)(value: U): U = {\n    return value;\n};\n\
         fun[] main(): int = {\n    return pick(1);\n};\n",
    )
    .expect("Should write the ambiguous generic call fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject ambiguous generic calls");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::AmbiguousReference
            && error
                .message()
                .contains("callable routine 'pick' is ambiguous")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_generic_parameter_references_in_sibling_routines() {
    let temp_root = unique_temp_root("generic_sibling_non_visibility");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n    return value;\n};\n\
         fun[] leak(value: int): T = {\n    return value;\n};\n",
    )
    .expect("Should write the sibling generic visibility fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject sibling routines using generic parameters");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::UnresolvedName
            && error.message().contains("could not resolve type 'T'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_local_bindings_that_shadow_generic_names() {
    let temp_root = unique_temp_root("generic_local_shadow_value_positions");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n\
             var T: int = 7;\n\
             fun[] use_local(): int = {\n\
                 return T;\n\
             };\n\
             return value;\n\
         };\n",
    )
    .expect("Should write the generic/local shadow fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject local bindings that shadow generic parameters");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::DuplicateSymbol
            && error
                .message()
                .contains("duplicate local symbol 'T' conflicts with existing generic parameter declaration")
    }));
 
    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_routine_parameters_that_shadow_generic_names() {
    let temp_root = unique_temp_root("generic_parameter_shadowing_param");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(T: int): T = {\n    return T;\n};\n",
    )
    .expect("Should write the generic/parameter shadow fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject routine parameters that shadow generic parameters");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::DuplicateSymbol
            && error
                .message()
                .contains("duplicate local symbol 'T' conflicts with existing generic parameter declaration")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_generic_parameter_use_in_top_level_value_annotations() {
    let temp_root = unique_temp_root("generic_top_level_value_annotation");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pick(T)(value: T): T = {\n    return value;\n};\n\
         var leaked: T = 1;\n",
    )
    .expect("Should write the top-level annotation misuse fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject top-level value annotations using generic parameters");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::UnresolvedName
            && error.message().contains("could not resolve type 'T'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_accepts_standard_names_in_generic_constraint_positions_without_unresolved_errors() {
    let temp_root = unique_temp_root("generic_standard_constraint_refs");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         fun pick(T: geo)(value: T): T = {\n\
             return value;\n\
         };\n",
    )
    .expect("Should write the generic standard-constraint fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let standard_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "geo" && symbol.kind == SymbolKind::Standard)
        .expect("Resolver should keep the standard symbol for the generic-constraint fixture");
    let routine_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "pick" && symbol.kind == SymbolKind::Routine)
        .expect("Resolver should keep the generic routine symbol for the standard-constraint fixture");

    assert_eq!(standard_symbol.name, "geo");
    assert_eq!(routine_symbol.name, "pick");

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_generic_parameter_references_in_nested_signature_positions() {
    let temp_root = unique_temp_root("generic_nested_signature_positions");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "typ Holder: rec = {\n\
             var item: int;\n\
         };\n\
         ali Choice: opt[int];\n\
         fun wrap(T)(value: T, items: seq[T], bucket: vec[T]): opt[T] = {\n\
             return nil;\n\
         };\n\
         fun alias_wrap(U)(value: U): Choice = {\n\
             panic(\"boom\");\n\
         };\n",
    )
    .expect("Should write the nested signature generic fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let routine_scope_ids = resolved
        .scopes
        .iter_with_ids()
        .filter_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .collect::<Vec<_>>();

    let generic_refs = routine_scope_ids
        .iter()
        .flat_map(|scope_id| resolved.references_in_scope(*scope_id).into_iter())
        .filter(|reference| reference.kind == ReferenceKind::TypeName)
        .filter(|reference| reference.name == "T" || reference.name == "U")
        .collect::<Vec<_>>();

    assert!(
        generic_refs.len() >= 4,
        "nested optional/container/alias signatures should keep generic type references visible to the resolver"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_generic_parameter_leakage_into_nested_record_annotations() {
    let temp_root = unique_temp_root("generic_nested_record_annotation_leak");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun wrap(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         typ Holder: rec = {\n\
             var item: T;\n\
         };\n",
    )
    .expect("Should write the nested record annotation leak fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject generic leakage into nested record annotations");

    assert!(errors.iter().any(|error| {
        error.kind() == ResolverErrorKind::UnresolvedName
            && error.message().contains("could not resolve type 'T'")
    }));

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_exposes_imported_generic_routines_across_loc_package_boundaries() {
    let temp_root = unique_temp_root("generic_imported_loc_calls");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package root fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported package root fixture directory");
    fs::write(
        temp_root.join("shared/lib.fol"),
        "fun[exp] pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun[exp] choose(T, U)(left: T, right: U): U = {\n\
             return right;\n\
         };\n",
    )
    .expect("Should write the imported generic routine fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use shared: loc = {\"../shared\"};\n\
         fun[] main(): int = {\n\
             var ready: bol = choose(1, true);\n\
             when(ready) {\n\
                 case(true) { return pick(1); }\n\
                 * { return 0; }\n\
             }\n\
         };\n",
    )
    .expect("Should write the importing generic call fixture");

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
        .expect("Resolver should keep the imported generic package alias");
    let target_scope = import
        .target_scope
        .expect("Imported loc package should resolve to a mounted root scope");
    let member_names = resolved
        .symbols_in_scope(target_scope)
        .into_iter()
        .filter(|symbol| symbol.kind == SymbolKind::Routine)
        .map(|symbol| symbol.name.clone())
        .collect::<Vec<_>>();

    assert!(
        member_names.contains(&"pick".to_string()),
        "Imported loc package should expose exported generic routine symbols at the mounted root"
    );
    assert!(
        member_names.contains(&"choose".to_string()),
        "Imported loc package should expose multi-parameter generic routine symbols at the mounted root"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_resolves_owned_recursive_type_edges_nominally() {
    let temp_root = unique_temp_root("type_resolution_owned_recursive");
    fs::create_dir_all(&temp_root).unwrap();
    fs::write(
        temp_root.join("main.fol"),
        "typ Node: rec = {\n    value: int,\n    next: opt @Node,\n};\n",
    )
    .unwrap();

    let resolved = resolve_package_from_folder(temp_root.to_str().unwrap());
    let node = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.kind == SymbolKind::Type && symbol.name == "Node")
        .expect("Node should resolve as a nominal type declaration");
    assert!(resolved.references.iter().any(|reference| {
        reference.kind == ReferenceKind::TypeName
            && reference.name == "Node"
            && reference.resolved == Some(node.id)
    }));

    fs::remove_dir_all(temp_root).ok();
}

#[test]
fn test_resolver_resolves_named_types_against_top_level_type_symbols() {
    let temp_root = unique_temp_root("type_resolution_named");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "typ Item: int;\nali NamedItem: Item;\nfun[] main(value: Item): NamedItem = {\n    var local: Item = value;\n    return local;\n};\n",
    )
    .expect("Should write the named-type resolver fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let source_unit_scope = resolved
        .source_units
        .iter()
        .next()
        .expect("Resolver should keep the source unit")
        .scope_id;
    let item_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "Item" && symbol.kind == SymbolKind::Type)
        .expect("Program scope should keep the Item type symbol");
    let named_item_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "NamedItem" && symbol.kind == SymbolKind::Alias)
        .expect("Program scope should keep the NamedItem alias symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");

    assert!(
        resolved
            .references_in_scope(source_unit_scope)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "Item"
                    && reference.resolved == Some(item_symbol.id)
            }),
        "Alias declarations should record named type references in the enclosing scope"
    );
    assert!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "NamedItem"
                    && reference.resolved == Some(named_item_symbol.id)
            }),
        "Routine signatures should resolve alias return types as named type references"
    );
    assert!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .filter(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "Item"
                    && reference.resolved == Some(item_symbol.id)
            })
            .count()
            >= 2,
        "Routine signatures and local type hints should both resolve named Item references"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_prefers_generic_parameters_over_outer_type_symbols() {
    let temp_root = unique_temp_root("type_resolution_generics");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "typ T: int;\nfun identity(T)(value: T): T = {\n    var copy: T = value;\n    return copy;\n};\n",
    )
    .expect("Should write the generic type resolver fixture");

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
    let generic_symbol = resolved
        .symbols_in_scope(routine_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "T" && symbol.kind == SymbolKind::GenericParameter)
        .expect("Routine scope should keep the generic parameter symbol");

    assert!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .filter(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "T"
                    && reference.resolved == Some(generic_symbol.id)
            })
            .count()
            >= 3,
        "Routine type references should prefer the generic parameter over outer type symbols"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_keeps_generic_parameter_symbols_for_param_and_return_type_references() {
    let temp_root = unique_temp_root("type_resolution_generic_signature_surface");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun pair(T, U)(left: T, right: U): T = {\n    return left;\n};\n",
    )
    .expect("Should write the generic signature resolver fixture");

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

    let generic_t = resolved
        .symbols_in_scope(routine_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "T" && symbol.kind == SymbolKind::GenericParameter)
        .expect("Routine scope should keep generic parameter T");
    let generic_u = resolved
        .symbols_in_scope(routine_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "U" && symbol.kind == SymbolKind::GenericParameter)
        .expect("Routine scope should keep generic parameter U");

    let type_refs = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .filter(|reference| reference.kind == ReferenceKind::TypeName)
        .collect::<Vec<_>>();

    assert!(type_refs.iter().any(|reference| {
        reference.name == "T" && reference.resolved == Some(generic_t.id)
    }));
    assert!(type_refs.iter().any(|reference| {
        reference.name == "U" && reference.resolved == Some(generic_u.id)
    }));
    assert!(
        type_refs
            .iter()
            .filter(|reference| reference.name == "T" && reference.resolved == Some(generic_t.id))
            .count()
            >= 2,
        "T should resolve in both a parameter and the return type"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_reports_unresolved_named_types() {
    let temp_root = unique_temp_root("type_resolution_missing");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(value: Missing): int = {\n    return 0;\n};\n",
    )
    .expect("Should write the unresolved type fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject unresolved named types");
    let error = errors
        .iter()
        .find(|error| error.kind() == ResolverErrorKind::UnresolvedName)
        .expect("Resolver should report unresolved-name errors for missing type names");
    let origin = error
        .origin()
        .expect("Plain unresolved named types should keep exact syntax origins");

    assert!(
        error.to_string().contains("could not resolve type 'Missing'"),
        "Resolver should report unresolved named type references explicitly"
    );
    assert_eq!(origin.line, 1);
    assert_eq!(origin.column, 19);
    assert_eq!(
        origin.file.as_deref(),
        Some(
            temp_root
                .join("main.fol")
                .to_str()
                .expect("Temporary resolver fixture path should be valid UTF-8")
        ),
        "Plain unresolved named types should retain their exact source file"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_treats_str_as_a_builtin_type() {
    let temp_root = unique_temp_root("type_resolution_builtin_str");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(path: str): str = {\n    var local: str = path;\n    return local;\n};\n",
    )
    .expect("Should write the builtin str resolver fixture");

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

    assert!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .all(|reference| {
                !(reference.kind == ReferenceKind::TypeName && reference.name == "str")
            }),
        "Builtin str should not enter named-type resolution as a user-defined symbol"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_treats_str_as_builtin_across_alias_and_type_definition_surfaces() {
    let temp_root = unique_temp_root("type_resolution_builtin_str_surfaces");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "ali Text: str;\ntyp Label: str;\ntyp User: rec = {\n    var name: str;\n};\nfun[] main(value: Text): Label = {\n    var local: str = value;\n    return local;\n};\n",
    )
    .expect("Should write the builtin str surface coverage fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let source_unit_scope = resolved
        .source_units
        .iter()
        .next()
        .expect("Resolver should keep the source unit")
        .scope_id;
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");

    assert!(
        resolved
            .references_in_scope(source_unit_scope)
            .into_iter()
            .all(|reference| {
                !(reference.kind == ReferenceKind::TypeName && reference.name == "str")
            }),
        "Builtin str should stay out of top-level alias and type-definition reference lowering"
    );
    assert!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .all(|reference| {
                !(reference.kind == ReferenceKind::TypeName && reference.name == "str")
            }),
        "Builtin str should stay out of routine signature and local type-hint reference lowering"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_instantiated_generic_type_references_to_the_declared_base_type() {
    let temp_root = unique_temp_root("type_resolution_generic_type_instantiation");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "typ Box(T): rec = {\n    var value: T;\n};\nali IntBox: Box[int];\nfun[] main(value: Box[int]): IntBox = {\n    var copy: Box[int] = value;\n    return copy;\n};\n",
    )
    .expect("Should write generic type instantiation resolver fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let box_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "Box" && symbol.kind == SymbolKind::Type)
        .expect("Program scope should keep the generic type declaration symbol");
    let source_unit_scope = resolved
        .source_units
        .iter()
        .next()
        .expect("Resolver should keep the source unit")
        .scope_id;
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");

    assert!(
        resolved
            .references_in_scope(source_unit_scope)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "Box[int]"
                    && reference.resolved == Some(box_symbol.id)
            }),
        "Alias targets should resolve instantiated generic type references against the declared base type"
    );
    assert!(
        resolved
            .references_in_scope(routine_scope_id)
            .into_iter()
            .filter(|reference| {
                reference.kind == ReferenceKind::TypeName
                    && reference.name == "Box[int]"
                    && reference.resolved == Some(box_symbol.id)
            })
            .count()
            >= 2,
        "Routine signatures and local hints should resolve instantiated generic type references against the declared base type"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_imported_instantiated_generic_type_references() {
    let temp_root = unique_temp_root("type_resolution_imported_generic_type_instantiation");
    let shared_root = temp_root.join("shared");
    let app_root = temp_root.join("app");
    fs::create_dir_all(&shared_root).expect("Should create the shared fixture root");
    fs::create_dir_all(&app_root).expect("Should create the app fixture root");
    fs::write(
        shared_root.join("types.fol"),
        "typ[exp] Box(T): rec = {\n    var value: T;\n};\n",
    )
    .expect("Should write shared generic type declaration fixture");
    fs::write(
        app_root.join("main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] main(value: shared::Box[int]): shared::Box[int] = {\n    var copy: shared::Box[int] = value;\n    return copy;\n};\n",
    )
    .expect("Should write imported generic type instantiation fixture");

    let resolved = resolve_package_from_folder(
        app_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "shared")
        .expect("Program scope should keep the shared import alias");
    let box_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "Box" && symbol.kind == SymbolKind::Type)
        .expect("Imported scope should keep the generic type declaration symbol");
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
                    && reference.name == "shared::Box[int]"
                    && reference.resolved == Some(box_symbol.id)
            })
            .count(),
        3,
        "Routine signatures and local hints should resolve imported instantiated generic types through the base declaration symbol"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

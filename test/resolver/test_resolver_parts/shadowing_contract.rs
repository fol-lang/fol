use super::{resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_allows_nested_blocks_to_shadow_routine_parameters() {
    let temp_root = unique_temp_root("shadowing_parameter");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(value: int): int = {\n    {\n        var value = 2;\n        return value;\n    };\n};\n",
    )
    .expect("Should write the parameter shadowing fixture");

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
    let block_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            (matches!(scope.kind, ScopeKind::Block) && scope.parent == Some(routine_scope_id))
                .then_some(scope_id)
        })
        .expect("Resolver should create a nested block scope");
    let outer_parameter = resolved
        .symbols_in_scope(routine_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "value" && symbol.kind == SymbolKind::Parameter)
        .expect("Routine scope should keep the outer parameter");
    let inner_binding = resolved
        .symbols_in_scope(block_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "value" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Block scope should keep the shadowing local binding");
    let inner_reference = resolved
        .references_in_scope(block_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::Identifier && reference.name == "value"
        })
        .expect("Nested block should record the shadowed identifier reference");

    assert_ne!(outer_parameter.id, inner_binding.id);
    assert_eq!(
        inner_reference.resolved,
        Some(inner_binding.id),
        "Nested block references should resolve against the shadowing local binding"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_allows_nested_blocks_to_shadow_import_aliases() {
    let temp_root = unique_temp_root("shadowing_import_alias");
    fs::create_dir_all(temp_root.join("net/http"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("net/http/route.fol"), "var handler: int = 1;\n")
        .expect("Should write the imported namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use http: loc = {\"net::http\"};\nfun[] main(): int = {\n    {\n        var http = 1;\n        return http;\n    };\n};\n",
    )
    .expect("Should write the import shadowing fixture");

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
    let block_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            (matches!(scope.kind, ScopeKind::Block) && scope.parent == Some(routine_scope_id))
                .then_some(scope_id)
        })
        .expect("Resolver should create a nested block scope");
    let import_alias = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "http" && symbol.kind == SymbolKind::ImportAlias)
        .expect("Program scope should keep the import alias");
    let inner_binding = resolved
        .symbols_in_scope(block_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "http" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Block scope should keep the shadowing local binding");
    let inner_reference = resolved
        .references_in_scope(block_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::Identifier && reference.name == "http"
        })
        .expect("Nested block should record the shadowed identifier reference");

    assert_ne!(import_alias.id, inner_binding.id);
    assert_eq!(
        inner_reference.resolved,
        Some(inner_binding.id),
        "Nested block references should resolve against the local shadow instead of the import alias"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_resolves_named_inquiry_targets_against_visible_symbols() {
    let temp_root = unique_temp_root("inquiry_named_target");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] helper(value: int): int = {\n    return value;\n};\n\nfun[] main(input: int): int = {\n    return helper(input);\n    where(helper) {\n        helper(input);\n    };\n};\n",
    )
    .expect("Should write the named inquiry target fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let helper_symbol = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .find(|symbol| symbol.name == "helper" && symbol.kind == SymbolKind::Routine)
        .expect("Program scope should keep the helper routine symbol");
    let main_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            (matches!(scope.kind, ScopeKind::Routine)
                && resolved
                    .symbols_in_scope(scope_id)
                    .into_iter()
                    .any(|symbol| symbol.name == "input" && symbol.kind == SymbolKind::Parameter))
            .then_some(scope_id)
        })
        .expect("Resolver should create a routine scope for main");

    assert!(
        resolved
            .references_in_scope(main_scope_id)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::InquiryTarget
                    && reference.name == "helper"
                    && reference.resolved == Some(helper_symbol.id)
            }),
        "Named inquiry targets should resolve against visible routine symbols"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_inquiry_targets_against_namespace_symbols() {
    let temp_root = unique_temp_root("inquiry_qualified_target");
    fs::create_dir_all(temp_root.join("tools"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("tools/helpers.fol"),
        "fun[] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the qualified inquiry namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] main(input: int): int = {\n    return tools::emit(input);\n    where(tools::emit) {\n        tools::emit(input);\n    };\n};\n",
    )
    .expect("Should write the qualified inquiry target fixture");

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
        .expect("Namespace scope should keep the emit routine symbol");
    let main_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            (matches!(scope.kind, ScopeKind::Routine)
                && resolved
                    .symbols_in_scope(scope_id)
                    .into_iter()
                    .any(|symbol| symbol.name == "input" && symbol.kind == SymbolKind::Parameter))
            .then_some(scope_id)
        })
        .expect("Resolver should create a routine scope for main");

    assert!(
        resolved
            .references_in_scope(main_scope_id)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::InquiryTarget
                    && reference.name == "tools::emit"
                    && reference.resolved == Some(emit_symbol.id)
            }),
        "Qualified inquiry targets should resolve through namespace roots"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_qualified_inquiry_targets_against_non_matching_import_alias_roots() {
    let temp_root = unique_temp_root("inquiry_nonmatching_import_alias");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("math/helpers.fol"),
        "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the imported inquiry namespace fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use tools: loc = {\"math\"};\nfun[] main(input: int): int = {\n    return tools::emit(input);\n    where(tools::emit) {\n        tools::emit(input);\n    };\n};\n",
    )
    .expect("Should write the qualified non-matching import-alias inquiry fixture");

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
        .expect("Imported namespace scope should keep the emit routine symbol");
    let main_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| {
            (matches!(scope.kind, ScopeKind::Routine)
                && resolved
                    .symbols_in_scope(scope_id)
                    .into_iter()
                    .any(|symbol| symbol.name == "input" && symbol.kind == SymbolKind::Parameter))
            .then_some(scope_id)
        })
        .expect("Resolver should create a routine scope for main");

    assert!(
        resolved
            .references_in_scope(main_scope_id)
            .into_iter()
            .any(|reference| {
                reference.kind == ReferenceKind::InquiryTarget
                    && reference.name == "tools::emit"
                    && reference.resolved == Some(emit_symbol.id)
            }),
        "Qualified inquiry targets should resolve through import aliases even when alias spelling differs from the namespace root"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_rejects_this_inquiry_targets_without_declared_return_types() {
    let temp_root = unique_temp_root("inquiry_this_without_return");
    fs::create_dir_all(&temp_root).expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("main.fol"),
        "fun[] show(value: int) = {\n    value;\n    where(this) {\n        value;\n    };\n};\n",
    )
    .expect("Should write the invalid this-target inquiry fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject this-target inquiries without declared return types");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::InvalidInput
                && error.to_string().contains(
                    "inquiry target 'this' requires an enclosing routine with a declared return type",
                )
        }),
        "Resolver should report invalid-context diagnostics for this-target inquiries without declared return types"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

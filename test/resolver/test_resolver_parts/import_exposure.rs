use super::{resolve_package_from_folder, try_resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ReferenceKind, ResolverErrorKind, ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_resolves_plain_identifiers_against_imported_exported_values() {
    let temp_root = unique_temp_root("import_exposure_value");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/values.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported exported value fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the plain imported-value lookup fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "math")
        .expect("Program scope should keep the imported namespace alias");
    let answer_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Imported namespace scope should keep the exported value binding");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::Identifier && reference.name == "answer"
        })
        .expect("Routine scope should record the imported plain identifier reference");

    assert_eq!(
        answer_reference.resolved,
        Some(answer_symbol.id),
        "Plain identifiers should resolve against imported exported value members"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_plain_free_calls_against_imported_exported_routines() {
    let temp_root = unique_temp_root("import_exposure_routine");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(
        temp_root.join("math/helpers.fol"),
        "fun[exp] emit(value: int): int = {\n    return value;\n};\n",
    )
    .expect("Should write the imported exported routine fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(value: int): int = {\n    return emit(value);\n};\n",
    )
    .expect("Should write the plain imported-routine call fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "math")
        .expect("Program scope should keep the imported namespace alias");
    let emit_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "emit" && symbol.kind == SymbolKind::Routine)
        .expect("Imported namespace scope should keep the exported routine symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let emit_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::FunctionCall && reference.name == "emit"
        })
        .expect("Routine scope should record the imported plain free-call reference");

    assert_eq!(
        emit_reference.resolved,
        Some(emit_symbol.id),
        "Plain free calls should resolve against imported exported routines"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_resolves_plain_named_types_against_imported_exported_types() {
    let temp_root = unique_temp_root("import_exposure_type");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/types.fol"), "typ[exp] Number: int;\n")
        .expect("Should write the imported exported type fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(value: Number): Number = {\n    return value;\n};\n",
    )
    .expect("Should write the plain imported-type lookup fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let import = resolved
        .imports_in_scope(resolved.program_scope)
        .into_iter()
        .find(|import| import.alias_name == "math")
        .expect("Program scope should keep the imported namespace alias");
    let number_symbol = resolved
        .symbols_in_scope(import.target_scope.expect("Import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "Number" && symbol.kind == SymbolKind::Type)
        .expect("Imported namespace scope should keep the exported type symbol");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let type_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::TypeName && reference.name == "Number"
        })
        .expect("Routine scope should record the imported plain named-type reference");

    assert_eq!(
        type_reference.resolved,
        Some(number_symbol.id),
        "Plain named types should resolve against imported exported type symbols"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_plain_import_exposure_rejects_non_exported_members() {
    let temp_root = unique_temp_root("import_exposure_non_exported");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/values.fol"), "var answer: int = 42;\n")
        .expect("Should write the non-exported imported value fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the non-exported import lookup fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject plain lookup against non-exported imported members");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error.to_string().contains("could not resolve name 'answer'")
        }),
        "Plain import exposure should not surface package-visible members without exp visibility"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_plain_import_exposure_rejects_hidden_members() {
    let temp_root = unique_temp_root("import_exposure_hidden");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/values.fol"), "var[hid] answer: int = 42;\n")
        .expect("Should write the hidden imported value fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the hidden import lookup fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject plain lookup against hidden imported members");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::UnresolvedName
                && error.to_string().contains("could not resolve name 'answer'")
        }),
        "Plain import exposure should never surface file-private imported members"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_plain_import_exposure_reports_ambiguity_for_multiple_visible_imports() {
    let temp_root = unique_temp_root("import_exposure_ambiguity");
    fs::create_dir_all(temp_root.join("alpha"))
        .expect("Should create the first imported namespace fixture directory");
    fs::create_dir_all(temp_root.join("beta"))
        .expect("Should create the second imported namespace fixture directory");
    fs::write(temp_root.join("alpha/values.fol"), "var[exp] answer: int = 1;\n")
        .expect("Should write the first imported exported value fixture");
    fs::write(temp_root.join("beta/values.fol"), "var[exp] answer: int = 2;\n")
        .expect("Should write the second imported exported value fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use alpha: loc = {\"alpha\"};\nuse beta: loc = {\"beta\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the ambiguous imported plain-name fixture");

    let errors = try_resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    )
    .expect_err("Resolver should reject ambiguous imported plain names");

    assert!(
        errors.iter().any(|error| {
            error.kind() == ResolverErrorKind::AmbiguousReference
                && error
                    .to_string()
                    .contains("name 'answer' is ambiguous in lexical scope")
        }),
        "Multiple visible imports that expose the same plain name should be ambiguous"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_plain_import_exposure_still_yields_to_local_bindings() {
    let temp_root = unique_temp_root("import_exposure_shadowing");
    fs::create_dir_all(temp_root.join("math"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("math/values.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the imported exported value fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use math: loc = {\"math\"};\nfun[] main(): int = {\n    var answer = 7;\n    return answer;\n};\n",
    )
    .expect("Should write the local-shadowing import exposure fixture");

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
    let local_answer = resolved
        .symbols_in_scope(routine_scope_id)
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Routine scope should keep the local shadowing binding");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .filter(|reference| reference.kind == ReferenceKind::Identifier && reference.name == "answer")
        .last()
        .expect("Routine scope should record the shadowed plain identifier reference");

    assert_eq!(
        answer_reference.resolved,
        Some(local_answer.id),
        "Direct lexical bindings should win before imported plain-name exposure"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

#[test]
fn test_resolver_plain_import_exposure_dedupes_repeated_imports_of_the_same_directory() {
    let temp_root = unique_temp_root("import_exposure_duplicate_root");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create a temporary resolver fixture directory");
    fs::write(temp_root.join("shared/values.fol"), "var[exp] answer: int = 42;\n")
        .expect("Should write the shared imported exported value fixture");
    fs::write(
        temp_root.join("main.fol"),
        "use alpha: loc = {\"shared\"};\nuse beta: loc = {\"shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the repeated-import plain exposure fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary resolver fixture path should be valid UTF-8"),
    );
    let imports = resolved.imports_in_scope(resolved.program_scope);
    let alpha = imports
        .iter()
        .find(|import| import.alias_name == "alpha")
        .expect("Program scope should keep the first import alias");
    let beta = imports
        .iter()
        .find(|import| import.alias_name == "beta")
        .expect("Program scope should keep the second import alias");
    let answer_symbol = resolved
        .symbols_in_scope(alpha.target_scope.expect("First import target should resolve"))
        .into_iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted imported root should keep the exported value binding");
    let routine_scope_id = resolved
        .scopes
        .iter_with_ids()
        .find_map(|(scope_id, scope)| matches!(scope.kind, ScopeKind::Routine).then_some(scope_id))
        .expect("Resolver should create a routine scope");
    let answer_reference = resolved
        .references_in_scope(routine_scope_id)
        .into_iter()
        .find(|reference| {
            reference.kind == ReferenceKind::Identifier && reference.name == "answer"
        })
        .expect("Routine scope should record the repeated-import plain identifier reference");

    assert_eq!(
        alpha.target_scope,
        beta.target_scope,
        "Repeated loc imports of the same canonical directory should share one mounted target scope"
    );
    assert_eq!(
        answer_reference.resolved,
        Some(answer_symbol.id),
        "Repeated loc imports of the same directory should not make plain imported names ambiguous"
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

use super::{resolve_package_from_folder, unique_temp_root};
use fol_resolver::{ScopeKind, SymbolKind};
use std::fs;

#[test]
fn test_resolver_collects_top_level_named_declarations_across_multiple_files() {
    let temp_root = unique_temp_root("top_level_symbols");
    fs::create_dir_all(temp_root.join("core"))
        .expect("Should create resolver top-level fixture dir");
    fs::write(
        temp_root.join("00_values.fol"),
        "var value: int = 1;\nlab label: int = 2;\nvar left, *right = { 1, 2, 3 };\n",
    )
    .expect("Should write value fixture");
    fs::write(
        temp_root.join("01_routines.fol"),
        "fun run(a: int): int = { return a; };\npro apply(a: int): int = { return a; };\nlog ready(a: int): bol = { return true; };\n",
    )
    .expect("Should write routine fixture");
    fs::write(
        temp_root.join("02_types.fol"),
        "typ Text: int;\nali Count: int;\nuse core: loc = {\"core\"};\n",
    )
    .expect("Should write type fixture");
    fs::write(
        temp_root.join("core/module.fol"),
        "var imported: int = 1;\n",
    )
    .expect("Should write imported namespace fixture");
    fs::write(
        temp_root.join("03_meta.fol"),
        "def[] mark: blk[];\nseg coreSeg: mod = { def helper: blk[] = { } };\nstd geometry: pro = { fun area(): int; };\n",
    )
    .expect("Should write meta fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Resolver folder fixture path should be utf-8"),
    );
    let symbols = resolved
        .symbols_in_scope(resolved.program_scope)
        .into_iter()
        .map(|symbol| (symbol.name.clone(), symbol.kind))
        .collect::<Vec<_>>();

    for expected in [
        ("value".to_string(), SymbolKind::ValueBinding),
        ("label".to_string(), SymbolKind::LabelBinding),
        ("left".to_string(), SymbolKind::DestructureBinding),
        ("right".to_string(), SymbolKind::DestructureBinding),
        ("run".to_string(), SymbolKind::Routine),
        ("apply".to_string(), SymbolKind::Routine),
        ("ready".to_string(), SymbolKind::Routine),
        ("Text".to_string(), SymbolKind::Type),
        ("Count".to_string(), SymbolKind::Alias),
        ("core".to_string(), SymbolKind::ImportAlias),
        ("mark".to_string(), SymbolKind::Definition),
        ("coreSeg".to_string(), SymbolKind::Segment),
        ("geometry".to_string(), SymbolKind::Standard),
    ] {
        assert!(
            symbols.contains(&expected),
            "Expected top-level symbol {:?} to be collected into the program scope",
            expected
        );
    }

    fs::remove_dir_all(&temp_root).ok();
}

#[test]
fn test_resolver_collects_nested_namespace_declarations_into_namespace_scopes() {
    let temp_root = unique_temp_root("namespace_symbols");
    fs::create_dir_all(temp_root.join("net/http")).expect("Should create nested namespace dirs");
    fs::write(temp_root.join("main.fol"), "var rootValue: int = 1;\n")
        .expect("Should write root file");
    fs::write(
        temp_root.join("net/http/route.fol"),
        "fun handle(code: int): int = { return code; };\n",
    )
    .expect("Should write namespace file");

    let resolved = resolve_package_from_folder(
        temp_root
            .to_str()
            .expect("Resolver folder fixture path should be utf-8"),
    );
    let package = temp_root
        .file_name()
        .expect("Temp resolver root should have a folder name")
        .to_string_lossy()
        .to_string();
    let namespace_scope = resolved
        .namespace_scope(&format!("{}::net::http", package))
        .expect("Nested namespace scope should exist");
    let namespace_symbols = resolved
        .symbols_in_scope(namespace_scope)
        .into_iter()
        .map(|symbol| (symbol.name.clone(), symbol.kind))
        .collect::<Vec<_>>();
    let root_scope = resolved
        .scope(resolved.program_scope)
        .expect("Program scope should exist");

    assert!(
        namespace_symbols.contains(&("handle".to_string(), SymbolKind::Routine)),
        "Namespace-scoped declarations should be collected into their namespace scope"
    );
    assert!(
        matches!(root_scope.kind, ScopeKind::ProgramRoot { .. }),
        "The package root should remain a distinct scope from nested namespace scopes"
    );
    assert!(
        resolved
            .symbols_in_scope(resolved.program_scope)
            .into_iter()
            .any(|symbol| symbol.name == "rootValue" && symbol.kind == SymbolKind::ValueBinding),
        "Root-namespace declarations should stay in the program scope"
    );

    fs::remove_dir_all(&temp_root).ok();
}

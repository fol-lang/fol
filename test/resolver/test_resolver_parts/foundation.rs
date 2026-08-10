use super::{resolve_package_from_file, resolve_package_from_folder, unique_temp_root};
use fol_resolver::SymbolKind;
use std::fs;

#[test]
fn test_resolver_smoke_lowers_source_units() {
    let resolved = resolve_package_from_file("test/parser/simple_var.fol");

    assert_eq!(resolved.package_name(), "parser");
    assert_eq!(resolved.source_units.len(), 1);

    let source_unit = resolved
        .source_units
        .iter()
        .next()
        .expect("Resolved program should expose a source unit");

    assert!(source_unit.path.ends_with("simple_var.fol"));
    assert_eq!(source_unit.top_level_nodes.len(), 1);
}

#[test]
fn test_resolver_keeps_parser_syntax_origins_available() {
    let resolved = resolve_package_from_file("test/parser/simple_var.fol");
    let source_unit = resolved
        .source_units
        .iter()
        .next()
        .expect("Resolved program should expose a source unit");
    let first_node = *source_unit
        .top_level_nodes
        .first()
        .expect("Resolved source unit should keep top-level syntax ids");
    let origin = resolved
        .syntax_index()
        .origin(first_node)
        .expect("Resolver-visible syntax ids should resolve back to syntax origins");

    assert!(origin
        .file
        .as_deref()
        .expect("Syntax origin should retain file path")
        .ends_with("simple_var.fol"));
    assert_eq!(origin.line, 1);
    // Binding items anchor at the declared NAME (`x`), not the `var` keyword.
    assert_eq!(origin.column, 5);
    assert!(origin.length >= 1);
}

#[test]
fn test_resolver_keeps_mounted_symbol_provenance_for_imported_exports() {
    let temp_root = unique_temp_root("mounted_symbol_provenance");
    fs::create_dir_all(temp_root.join("app"))
        .expect("Should create the importing package fixture directory");
    fs::create_dir_all(temp_root.join("shared"))
        .expect("Should create the imported package fixture directory");
    fs::write(
        temp_root.join("shared/lib.fol"),
        "var[exp] answer: int = 42;\n",
    )
    .expect("Should write the imported package fixture");
    fs::write(
        temp_root.join("app/main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] main(): int = {\n    return answer;\n};\n",
    )
    .expect("Should write the importing package fixture");

    let resolved = resolve_package_from_folder(
        temp_root
            .join("app")
            .to_str()
            .expect("Temporary fixture path should be valid UTF-8"),
    );
    let answer = resolved
        .symbols
        .iter()
        .find(|symbol| symbol.name == "answer" && symbol.kind == SymbolKind::ValueBinding)
        .expect("Mounted imported value symbol should exist");
    let provenance = answer
        .mounted_from
        .as_ref()
        .expect("Mounted imported symbols should keep foreign provenance");

    assert_eq!(provenance.package_identity.display_name, "shared");
    assert!(
        provenance
            .package_identity
            .canonical_root
            .ends_with("shared"),
        "Expected mounted provenance to keep the imported package root, got {:?}",
        provenance.package_identity.canonical_root
    );
    assert_eq!(provenance.foreign_symbol.0, 0);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary resolver fixture directory should be removable after the test");
}

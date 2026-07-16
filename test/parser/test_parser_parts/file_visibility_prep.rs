use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_parser_visibility_prep_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

fn write_folder_fixture(root: &std::path::Path, files: &[(&str, &str)]) {
    fs::create_dir_all(root).expect("Should create temporary parser visibility fixture");
    for (name, source) in files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Should create parser visibility fixture folder");
        }
        fs::write(path, source).expect("Should write parser visibility fixture file");
    }
}

fn top_level_name(node: &AstNode) -> Option<&str> {
    match node {
        AstNode::VarDecl { name, .. }
        | AstNode::LabDecl { name, .. }
        | AstNode::FunDecl { name, .. }
        | AstNode::ProDecl { name, .. }
        | AstNode::LogDecl { name, .. }
        | AstNode::TypeDecl { name, .. }
        | AstNode::UseDecl { name, .. }
        | AstNode::AliasDecl { name, .. }
        | AstNode::DefDecl { name, .. }
        | AstNode::SegDecl { name, .. }
        | AstNode::StdDecl { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

#[test]
fn test_visibility_marked_declarations_stay_in_their_own_source_units() {
    let temp_root = unique_temp_root("placement");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_public.fol", "var[exp] shared: int = 1;\n"),
            ("10_hidden.fol", "var[imu,hid] local: int = 2;\n"),
            (
                "nested/20_branch.fol",
                "use[hidden] cache: loc = {\"core::cache\"};\nvar branch: int = 3;\n",
            ),
        ],
    );

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary parser visibility fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    assert_eq!(
        parsed.source_units.len(),
        3,
        "Visibility-marked declarations should remain grouped by their physical source units"
    );

    assert_eq!(
        top_level_name(&parsed.source_units[0].items[0].node),
        Some("shared"),
        "The exported root declaration should remain in the first file's source unit"
    );
    assert_eq!(
        parsed.source_units[0].items[0].declaration_visibility(),
        Some(ParsedDeclVisibility::Exported)
    );

    assert_eq!(
        top_level_name(&parsed.source_units[1].items[0].node),
        Some("local"),
        "The hidden root declaration should remain in the second file's source unit"
    );
    assert_eq!(
        parsed.source_units[1].items[0].declaration_visibility(),
        Some(ParsedDeclVisibility::Hidden)
    );

    assert_eq!(
        top_level_name(&parsed.source_units[2].items[0].node),
        Some("cache"),
        "The nested hidden use declaration should remain in the nested file's source unit"
    );
    assert_eq!(
        parsed.source_units[2].items[0].declaration_visibility(),
        Some(ParsedDeclVisibility::Hidden)
    );
    assert_eq!(
        top_level_name(&parsed.source_units[2].items[1].node),
        Some("branch"),
        "The nested package-visible declaration should remain beside its sibling in the same nested source unit"
    );
}

#[test]
fn test_parsed_top_levels_distinguish_package_namespace_and_file_scope() {
    let temp_root = unique_temp_root("scope_kinds");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_public.fol", "var[exp] shared: int = 1;\n"),
            ("10_hidden.fol", "var[imu,hid] local: int = 2;\n"),
            (
                "nested/20_branch.fol",
                "use[hidden] cache: loc = {\"core::cache\"};\nvar branch: int = 3;\n",
            ),
        ],
    );

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary parser scope-kind fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    assert_eq!(
        parsed.source_units[0].items[0].declaration_scope(),
        Some(ParsedDeclScope::Package),
        "Root-namespace exported declarations should be marked as package-wide"
    );
    assert_eq!(
        parsed.source_units[1].items[0].declaration_scope(),
        Some(ParsedDeclScope::File),
        "Hidden declarations should be marked as file-private regardless of package membership"
    );
    assert_eq!(
        parsed.source_units[2].items[0].declaration_scope(),
        Some(ParsedDeclScope::File),
        "Hidden nested declarations should still be marked as file-private"
    );
    assert_eq!(
        parsed.source_units[2].items[1].declaration_scope(),
        Some(ParsedDeclScope::Namespace),
        "Non-hidden declarations in nested folders should be marked as namespace-contained"
    );
    assert_eq!(
        parsed.source_units[2].items[1].declaration_visibility(),
        Some(ParsedDeclVisibility::Normal),
        "Nested declarations without explicit visibility should still carry an explicit normal visibility classification"
    );
}

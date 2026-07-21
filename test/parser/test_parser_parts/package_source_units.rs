use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_parser_package_units_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

fn write_folder_fixture(root: &std::path::Path, files: &[(&str, &str)]) {
    fs::create_dir_all(root).expect("Should create temporary parser package fixture");
    for (name, source) in files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("Should create parser package fixture folder");
        }
        fs::write(path, source).expect("Should write parser package fixture file");
    }
}

#[derive(Clone, Copy, Debug)]
enum SourceUnitDeclFamily {
    Use,
    Def,
    Seg,
    Var,
    Lab,
    Fun,
    Pro,
    Log,
    Type,
    Alias,
    Standard,
}

fn source_unit_has_decl_family(
    source_unit: &ParsedSourceUnit,
    family: SourceUnitDeclFamily,
) -> bool {
    source_unit.items.iter().any(|item| {
        matches!(
            (&item.node, family),
            (AstNode::UseDecl { .. }, SourceUnitDeclFamily::Use)
                | (AstNode::DefDecl { .. }, SourceUnitDeclFamily::Def)
                | (AstNode::SegDecl { .. }, SourceUnitDeclFamily::Seg)
                | (AstNode::VarDecl { .. }, SourceUnitDeclFamily::Var)
                | (AstNode::LabDecl { .. }, SourceUnitDeclFamily::Lab)
                | (AstNode::FunDecl { .. }, SourceUnitDeclFamily::Fun)
                | (AstNode::ProDecl { .. }, SourceUnitDeclFamily::Pro)
                | (AstNode::LogDecl { .. }, SourceUnitDeclFamily::Log)
                | (AstNode::TypeDecl { .. }, SourceUnitDeclFamily::Type)
                | (AstNode::AliasDecl { .. }, SourceUnitDeclFamily::Alias)
                | (AstNode::StdDecl { .. }, SourceUnitDeclFamily::Standard)
        )
    })
}

#[test]
fn test_parse_package_groups_top_level_items_by_source_unit() {
    let temp_root = unique_temp_root("grouping");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_alpha.fol", "var alpha = 1;\n"),
            ("10_beta.fol", "var beta = 2;\n"),
        ],
    );

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary parser package fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    let expected_package = temp_root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("Temp parser package fixture should have a UTF-8 folder name")
        .to_string();

    assert_eq!(parsed.package, expected_package);
    assert_eq!(
        parsed.source_units.len(),
        2,
        "Folder parsing should retain one parsed source unit per physical file"
    );
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[0]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "alpha"
    ));
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[1]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "beta"
    ));
}

#[test]
fn test_parse_package_retains_namespace_per_source_unit() {
    let temp_root = unique_temp_root("namespaces");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_root.fol", "var root_value = 1;\n"),
            ("printing/logg/entry.fol", "var warn_value = 2;\n"),
        ],
    );

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary parser namespace fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    let expected_package = temp_root
        .file_name()
        .and_then(|name| name.to_str())
        .expect("Temp parser namespace fixture should have a UTF-8 folder name")
        .to_string();

    assert_eq!(parsed.source_units[0].namespace, expected_package);
    assert_eq!(
        parsed.source_units[1].namespace,
        format!("{expected_package}::printing::logg"),
        "Nested folder files should retain the namespace path computed by fol-stream"
    );
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[1]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "warn_value"
    ));
}

#[test]
fn test_parse_package_source_unit_order_matches_stream_traversal_order() {
    let temp_root = unique_temp_root("ordering");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_root.fol", "var root_first = 1;\n"),
            ("a_nested/00_alpha.fol", "var nested_alpha = 2;\n"),
            ("a_nested/10_beta.fol", "var nested_beta = 3;\n"),
            ("b_more/00_gamma.fol", "var nested_gamma = 4;\n"),
            ("m_root.fol", "var root_last = 5;\n"),
        ],
    );

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary parser ordering fixture path should be UTF-8"),
    );

    let canonical_root =
        std::fs::canonicalize(&temp_root).expect("Ordering fixture root should canonicalize");

    fs::remove_dir_all(&temp_root).ok();

    let relative_paths: Vec<String> = parsed
        .source_units
        .iter()
        .map(|unit| {
            std::path::Path::new(&unit.path)
                .strip_prefix(&canonical_root)
                .expect("Parsed source unit should stay under the fixture root")
                .to_string_lossy()
                .to_string()
        })
        .collect();

    assert_eq!(
        relative_paths,
        vec![
            "00_root.fol",
            "a_nested/00_alpha.fol",
            "a_nested/10_beta.fol",
            "b_more/00_gamma.fol",
            "m_root.fol",
        ],
        "Parsed source units should preserve the exact deterministic traversal order from fol-stream"
    );
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[0]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "root_first"
    ));
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[4]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "root_last"
    ));
}

#[test]
fn test_decl_package_keeps_root_comments_as_source_unit_items() {
    let temp_root = unique_temp_root("decl_root_comments");
    write_folder_fixture(
        &temp_root,
        &[
            (
                "00_alpha.fol",
                "`[doc] alpha docs`\n`alpha note`\nvar alpha = 1;\n",
            ),
            ("10_beta.fol", "`[doc] beta docs`\nvar beta = 2;\n"),
        ],
    );

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary parser declaration-package fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    assert_eq!(
        parsed.source_units.len(),
        2,
        "Declaration-only package parsing should still retain one source unit per file"
    );
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[0]).as_slice(),
        [
            AstNode::Comment {
                kind: CommentKind::Doc,
                raw,
            },
            AstNode::Comment {
                kind: CommentKind::Backtick,
                raw: note,
            },
            AstNode::VarDecl { name, .. },
        ] if raw == "`[doc] alpha docs`" && note == "`alpha note`" && name == "alpha"
    ));
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[1]).as_slice(),
        [
            AstNode::Comment {
                kind: CommentKind::Doc,
                raw,
            },
            AstNode::VarDecl { name, .. },
        ] if raw == "`[doc] beta docs`" && name == "beta"
    ));
}

#[test]
fn test_decl_package_preserves_comment_only_source_units() {
    let temp_root = unique_temp_root("decl_comment_only");
    write_folder_fixture(
        &temp_root,
        &[
            ("00_notes.fol", "`[doc] package note`\n`side note`\n"),
            ("10_values.fol", "var ready = 1;\n"),
        ],
    );

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary parser comment-only fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    assert!(matches!(
        source_unit_nodes(&parsed.source_units[0]).as_slice(),
        [
            AstNode::Comment {
                kind: CommentKind::Doc,
                raw,
            },
            AstNode::Comment {
                kind: CommentKind::Backtick,
                raw: note,
            },
        ] if raw == "`[doc] package note`" && note == "`side note`"
    ));
    assert!(matches!(
        source_unit_nodes(&parsed.source_units[1]).as_slice(),
        [AstNode::VarDecl { name, .. }] if name == "ready"
    ));
}

#[test]
fn test_parse_package_assigns_all_supported_root_decl_families_to_matching_source_units() {
    let temp_root = unique_temp_root("all_decl_families");
    let fixture_files = [
        (
            "00_use.fol",
            "test/parser/simple_use_bare_mod_type.fol",
            SourceUnitDeclFamily::Use,
        ),
        (
            "01_def.fol",
            "test/parser/simple_def_module.fol",
            SourceUnitDeclFamily::Def,
        ),
        (
            "02_seg.fol",
            "test/parser/simple_seg_module.fol",
            SourceUnitDeclFamily::Seg,
        ),
        (
            "04_var.fol",
            "test/parser/simple_var.fol",
            SourceUnitDeclFamily::Var,
        ),
        (
            "05_lab.fol",
            "test/parser/simple_lab_decl.fol",
            SourceUnitDeclFamily::Lab,
        ),
        (
            "06_fun.fol",
            "test/parser/simple_fun.fol",
            SourceUnitDeclFamily::Fun,
        ),
        (
            "07_pro.fol",
            "test/parser/simple_pro.fol",
            SourceUnitDeclFamily::Pro,
        ),
        (
            "08_log.fol",
            "test/parser/simple_log.fol",
            SourceUnitDeclFamily::Log,
        ),
        (
            "09_typ.fol",
            "test/parser/simple_typ_object_marker.fol",
            SourceUnitDeclFamily::Type,
        ),
        (
            "10_ali.fol",
            "test/parser/simple_ali.fol",
            SourceUnitDeclFamily::Alias,
        ),
        (
            "11_std.fol",
            "test/parser/simple_std_protocol.fol",
            SourceUnitDeclFamily::Standard,
        ),
    ];

    fs::create_dir_all(&temp_root).expect("Should create declaration-family fixture root");
    for (target, source_fixture, _) in fixture_files {
        let contents =
            fs::read_to_string(source_fixture).expect("Should read declaration-family fixture");
        fs::write(temp_root.join(target), contents)
            .expect("Should write declaration-family package fixture");
    }

    let parsed = parse_package_from_folder(
        temp_root
            .to_str()
            .expect("Temporary declaration-family fixture path should be UTF-8"),
    );

    fs::remove_dir_all(&temp_root).ok();

    assert_eq!(
        parsed.source_units.len(),
        fixture_files.len(),
        "Each declaration-family fixture file should remain its own parsed source unit"
    );

    for (index, (_, _, family)) in fixture_files.iter().enumerate() {
        assert!(
            source_unit_has_decl_family(&parsed.source_units[index], *family),
            "Source unit {} should retain its declaration family {:?} instead of mixing files",
            index,
            family
        );
    }
}

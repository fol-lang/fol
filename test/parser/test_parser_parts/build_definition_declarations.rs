use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_build_definition_decl_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[test]
fn test_package_parser_accepts_canonical_build_procedures() {
    let temp_root = unique_temp_root("canonical_build_procedure");
    fs::create_dir_all(&temp_root).expect("Should create temporary parser fixture root");
    let file_path = temp_root.join("build.fol");
    fs::write(
        &file_path,
        "pro[] build(): non = {\n    return;\n};\n",
    )
    .expect("Should write the canonical build fixture");

    let parsed = parse_package_from_file(
        file_path
            .to_str()
            .expect("Temporary parser fixture path should be valid UTF-8"),
    );
    let source_unit = parsed
        .source_units
        .first()
        .expect("Build definition fixture should produce one source unit");

    assert!(
        source_unit.items.iter().any(|item| {
            matches!(
                &item.node,
                AstNode::ProDecl {
                    name,
                    params,
                    return_type: Some(FolType::None),
                    ..
                } if name == "build" && params.is_empty()
            )
        }),
        "Package parser should accept canonical build procedures for build surfaces",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary parser fixture root should be removable after the test");
}

#[test]
fn test_package_parser_accepts_helper_declarations_alongside_the_build_entry() {
    let temp_root = unique_temp_root("build_helper_declarations");
    fs::create_dir_all(&temp_root).expect("Should create temporary parser fixture root");
    let file_path = temp_root.join("build.fol");
    fs::write(
        &file_path,
        concat!(
            "fun[] helper(): int = {\n    return 1;\n};\n",
            "pro[] build(): non = {\n    return;\n};\n",
        ),
    )
    .expect("Should write the helper build fixture");

    let parsed = parse_package_from_file(
        file_path
            .to_str()
            .expect("Temporary parser fixture path should be valid UTF-8"),
    );
    let source_unit = parsed
        .source_units
        .first()
        .expect("Build definition fixture should produce one source unit");

    assert!(
        source_unit.items.iter().any(|item| {
            matches!(
                &item.node,
                AstNode::FunDecl {
                    name,
                    ..
                } if name == "helper"
            )
        }),
        "Package parser should accept helper declarations in build surfaces",
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary parser fixture root should be removable after the test");
}

#[test]
fn test_package_parser_accepts_build_local_meta_calls_in_build_files() {
    let temp_root = unique_temp_root("build_local_meta_calls");
    fs::create_dir_all(&temp_root).expect("Should create temporary parser fixture root");
    let file_path = temp_root.join("build.fol");
    fs::write(
        &file_path,
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({\n",
            "        name = \"json\",\n",
            "        version = \"1.0.0\",\n",
            "    });\n",
            "};\n",
        ),
    )
    .expect("Should write the local build metadata fixture");

    let parsed = parse_package_from_file(
        file_path
            .to_str()
            .expect("Temporary parser fixture path should be valid UTF-8"),
    );

    assert_eq!(parsed.source_units.len(), 1);

    fs::remove_dir_all(&temp_root)
        .expect("Temporary parser fixture root should be removable after the test");
}

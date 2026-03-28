use super::*;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_v2_generics_m1_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

fn parse_script_package_from_inline(label: &str, source: &str) -> fol_parser::ast::ParsedPackage {
    let temp_root = unique_temp_root(label);
    fs::create_dir_all(&temp_root).expect("Should create a temporary parser fixture directory");
    let source_path = temp_root.join("main.fol");
    fs::write(&source_path, source).expect("Should write the temporary parser fixture");

    let parsed = parse_script_package_from_file(
        source_path
            .to_str()
            .expect("Temporary parser fixture path should be valid UTF-8"),
    );

    fs::remove_dir_all(&temp_root)
        .expect("Temporary parser fixture directory should be removable after the test");
    parsed
}

#[test]
fn test_v2_m1_parser_accepts_generic_routines_with_type_params_in_param_and_return_positions() {
    let ast = parse_script_package_from_inline(
        "signature_surface",
        "fun pair(T, U)(left: T, right: U): T = {\n\
             return left;\n\
         };\n",
    );

    let generic_fun = ast
        .source_units
        .iter()
        .flat_map(|unit| unit.items.iter())
        .find_map(|item| match &item.node {
            AstNode::FunDecl {
                name,
                generics,
                params,
                return_type,
                ..
            } if name == "pair" => Some((generics, params, return_type)),
            _ => None,
        })
        .expect("parser should keep the generic routine fixture");

    let (generics, params, return_type) = generic_fun;
    assert_eq!(generics.len(), 2);
    assert_eq!(generics[0].name, "T");
    assert_eq!(generics[1].name, "U");
    assert_eq!(params.len(), 2);
    assert!(matches!(
        &params[0].param_type,
        FolType::Named { name, .. } if name == "T"
    ));
    assert!(matches!(
        &params[1].param_type,
        FolType::Named { name, .. } if name == "U"
    ));
    assert!(matches!(
        return_type,
        Some(FolType::Named { name, .. }) if name == "T"
    ));
}

#[test]
fn test_v2_m1_parser_accepts_generic_routine_headers_for_fun_pro_and_log() {
    let ast = parse_script_package_from_inline(
        "routine_kinds",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         pro apply(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         log check(T)(value: T): bol = {\n\
             return true;\n\
         };\n",
    );
    let declarations = ast
        .source_units
        .iter()
        .flat_map(|unit| unit.items.iter().map(|item| &item.node))
        .collect::<Vec<_>>();

    assert!(declarations.iter().any(|node| {
        matches!(node, AstNode::FunDecl { generics, .. } if generics.len() == 1)
    }));
    assert!(declarations.iter().any(|node| {
        matches!(node, AstNode::ProDecl { generics, .. } if generics.len() == 1)
    }));
    assert!(declarations.iter().any(|node| {
        matches!(node, AstNode::LogDecl { generics, .. } if generics.len() == 1)
    }));
}

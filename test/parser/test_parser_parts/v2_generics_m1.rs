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

fn parse_script_package_errors_from_inline(
    label: &str,
    source: &str,
) -> Vec<fol_diagnostics::Diagnostic> {
    let temp_root = unique_temp_root(label);
    fs::create_dir_all(&temp_root).expect("Should create a temporary parser fixture directory");
    let source_path = temp_root.join("main.fol");
    fs::write(&source_path, source).expect("Should write the temporary parser fixture");

    let mut file_stream =
        FileStream::from_file(source_path.to_str().expect("Temporary parser path should be utf-8"))
            .expect("Should open temporary parser fixture");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse_script_package(&mut lexer)
        .expect_err("fixture should fail parsing");

    fs::remove_dir_all(&temp_root)
        .expect("Temporary parser fixture directory should be removable after the test");
    errors
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

#[test]
fn test_v2_m1_parser_accepts_generic_routines_with_defaults_variadics_and_captures() {
    let ast = parse_script_package_from_inline(
        "mixed_header_surface",
        "fun pick(T)(head: T, label: str = \"x\", tail: ... int)[seed]: T = {\n\
             return head;\n\
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
                captures,
                ..
            } if name == "pick" => Some((generics, params, captures)),
            _ => None,
        })
        .expect("parser should keep the mixed generic routine fixture");

    let (generics, params, captures) = generic_fun;
    assert_eq!(generics.len(), 1);
    assert_eq!(generics[0].name, "T");
    assert_eq!(captures, &vec!["seed".to_string()]);
    assert_eq!(params.len(), 3);
    assert_eq!(params[0].name, "head");
    assert!(matches!(
        &params[0].param_type,
        FolType::Named { name, .. } if name == "T"
    ));
    assert_eq!(params[1].name, "label");
    assert!(params[1].default.is_some());
    assert_eq!(params[2].name, "tail");
    assert!(matches!(
        params[2].param_type,
        FolType::Sequence { .. }
    ));
}

#[test]
fn test_v2_m1_parser_accepts_receiver_qualified_generic_routines() {
    let ast = parse_script_package_from_inline(
        "receiver_generic_surface",
        "typ Box: rec = {\n\
             var value: int;\n\
         };\n\
         fun (Box)pick(T)(value: T): T = {\n\
             return value;\n\
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
                receiver_type,
                params,
                return_type,
                ..
            } if name == "pick" => Some((generics, receiver_type, params, return_type)),
            _ => None,
        })
        .expect("parser should keep the receiver-qualified generic routine fixture");

    let (generics, receiver_type, params, return_type) = generic_fun;
    assert_eq!(generics.len(), 1);
    assert!(matches!(
        receiver_type,
        Some(FolType::Named { name, .. }) if name == "Box"
    ));
    assert_eq!(params.len(), 1);
    assert!(matches!(
        &params[0].param_type,
        FolType::Named { name, .. } if name == "T"
    ));
    assert!(matches!(
        return_type,
        Some(FolType::Named { name, .. }) if name == "T"
    ));
}

#[test]
fn test_v2_m1_parser_accepts_receiver_qualified_generic_routines_with_named_and_default_params() {
    let ast = parse_script_package_from_inline(
        "receiver_generic_default_named_surface",
        "typ Box: rec = {\n\
             var value: int;\n\
         };\n\
         fun (Box)choose(T, U)(left: T, right: U, prefer_left: bol = true): T = {\n\
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
                receiver_type,
                params,
                ..
            } if name == "choose" => Some((generics, receiver_type, params)),
            _ => None,
        })
        .expect("parser should keep the receiver-qualified generic routine with defaults");

    let (generics, receiver_type, params) = generic_fun;
    assert_eq!(generics.len(), 2);
    assert!(matches!(
        receiver_type,
        Some(FolType::Named { name, .. }) if name == "Box"
    ));
    assert_eq!(params.len(), 3);
    assert_eq!(params[2].name, "prefer_left");
    assert!(params[2].default.is_some());
}

#[test]
fn test_v2_m1_parser_rejects_missing_generic_parameter_names() {
    let errors = parse_script_package_errors_from_inline(
        "missing_generic_name",
        "fun pick(, U)(value: U): U = {\n    return value;\n};\n",
    );

    let first = errors.first().expect("parser should report a first error");
    assert!(
        first.message.contains("Expected generic parameter name"),
        "missing generic names should keep the explicit parser wording, got: {}",
        first.message
    );
}

#[test]
fn test_v2_m1_parser_rejects_repeated_generic_separators() {
    let errors = parse_script_package_errors_from_inline(
        "repeated_generic_separator",
        "fun pick(T,, U)(value: T): T = {\n    return value;\n};\n",
    );

    let first = errors.first().expect("parser should report a first error");
    assert!(
        first.message.contains("Expected generic parameter name")
            || first.message.contains("Expected parameter name after ','"),
        "repeated separators should fail in the generic header, got: {}",
        first.message
    );
}

#[test]
fn test_v2_m1_parser_rejects_broken_generic_header_punctuation() {
    let errors = parse_script_package_errors_from_inline(
        "broken_generic_punctuation",
        "fun pick(T; U](value: T): T = {\n    return value;\n};\n",
    );

    let first = errors.first().expect("parser should report a first error");
    assert!(
        first.message.contains("Expected ',', ';', or ')' after generic parameter")
            || first.message.contains("Expected ')'"),
        "broken generic punctuation should fail explicitly, got: {}",
        first.message
    );
}

#[test]
fn test_v2_m1_parser_keeps_template_call_surface_separate_from_generics() {
    let ast = parse_script_package_from_inline(
        "template_call_boundary",
        "fun pick(T)(value: T): T = {\n\
             return value;\n\
         };\n\
         fun stringify(file: file): str = {\n\
             return file$;\n\
         };\n",
    );

    assert!(ast
        .source_units
        .iter()
        .flat_map(|unit| unit.items.iter())
        .any(|item| matches!(
            &item.node,
            AstNode::FunDecl { name, body, .. }
                if name == "stringify"
                    && body.iter().any(|stmt| matches!(
                        stmt,
                        AstNode::Return { value: Some(value) }
                            if matches!(value.as_ref(), AstNode::TemplateCall { template, .. } if template == "$")
                    ))
        )));
}

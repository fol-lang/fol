use super::*;
use fol_parser::ast::StandardKind;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_v2_standards_m2_{}_{}_{}",
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
fn test_v2_standards_m2_parser_inventory_keeps_standard_declaration_kinds() {
    let protocol = parse_package_from_file("test/parser/simple_std_protocol.fol");
    let blueprint = parse_package_from_file("test/parser/simple_std_blueprint.fol");
    let extended = parse_package_from_file("test/parser/simple_std_extended.fol");

    assert!(protocol.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::StdDecl { name, kind: StandardKind::Protocol, body, .. }
            if name == "geometry" && !body.is_empty()
        )
    }));
    assert!(blueprint.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::StdDecl { name, kind: StandardKind::Blueprint, body, .. }
            if name == "geometry" && !body.is_empty()
        )
    }));
    assert!(extended.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::StdDecl { name, kind: StandardKind::Extended, body, .. }
            if name == "geometry" && !body.is_empty()
        )
    }));
}

#[test]
fn test_v2_standards_m2_parser_inventory_keeps_type_contract_headers() {
    let package = parse_package_from_file("test/parser/simple_typ_record_explicit_contracts.fol");

    assert!(package.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::TypeDecl { name, generics, contracts, .. }
            if name == "Shape" && !generics.is_empty() && !contracts.is_empty()
        )
    }));
}

#[test]
fn test_v2_standards_m2_parser_truth_is_broader_than_the_first_semantic_subset() {
    let package = parse_package_from_file("test/parser/simple_std_blueprint.fol");

    assert!(package.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::StdDecl { kind: StandardKind::Blueprint, body, .. }
            if body.iter().any(|member| matches!(member, AstNode::VarDecl { .. }))
        )
    }));
}

#[test]
fn test_v2_standards_m2_parser_accepts_multiple_required_routines() {
    let package = parse_script_package_from_inline(
        "multi_required_routines",
        "std geo: pro = {\n\
             fun area(): int;\n\
             fun perimeter(): int;\n\
         };\n",
    );

    assert!(package.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::StdDecl { kind: StandardKind::Protocol, body, .. }
                if body.iter().filter(|member| matches!(member, AstNode::FunDecl { .. })).count() == 2
        )
    }));
}

#[test]
fn test_v2_standards_m2_parser_accepts_multi_parameter_protocol_signatures() {
    let package = parse_script_package_from_inline(
        "multi_parameter_protocol",
        "std geo: pro = {\n\
             fun area(width: int, height: int): int;\n\
         };\n",
    );

    assert!(package.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::StdDecl { body, .. }
                if body.iter().any(|member| matches!(
                    member,
                    AstNode::FunDecl { name, params, return_type, .. }
                        if name == "area" && params.len() == 2 && return_type.is_some()
                ))
        )
    }));
}

#[test]
fn test_v2_standards_m2_parser_accepts_protocol_signatures_with_error_shells() {
    let package = parse_script_package_from_inline(
        "protocol_error_shell",
        "std geo: pro = {\n\
             fun area(): int / str;\n\
         };\n",
    );

    assert!(package.source_units.iter().flat_map(source_unit_nodes).any(|node| {
        matches!(
            node,
            AstNode::StdDecl { body, .. }
                if body.iter().any(|member| matches!(
                    member,
                    AstNode::FunDecl { name, return_type, error_type, .. }
                        if name == "area" && return_type.is_some() && error_type.is_some()
                ))
        )
    }));
}

#[test]
fn test_v2_standards_m2_parser_rejects_broken_standard_member_separators() {
    let errors = parse_script_package_errors_from_inline(
        "broken_standard_separators",
        "std geo: pro = {\n\
             fun area(): int,,\n\
         };\n",
    );

    let first = errors.first().expect("parser should report a first error");
    assert!(
        first.message.contains("Expected standard member")
            || first.message.contains("Unexpected token")
            || first.message.contains("Expected ';'"),
        "broken standard separators should fail explicitly, got: {}",
        first.message
    );
}

#[test]
fn test_v2_standards_m2_parser_rejects_malformed_standard_headers() {
    let errors = parse_script_package_errors_from_inline(
        "malformed_standard_header",
        "std geo pro = {\n\
             fun area(): int;\n\
         };\n",
    );

    let first = errors.first().expect("parser should report a first error");
    assert!(
        first.message.contains("Expected ':'")
            || first.message.contains("Expected standard kind")
            || first.message.contains("Expected kind")
            || first.message.contains("Expected procedure name after 'pro'"),
        "malformed standard headers should fail explicitly, got: {}",
        first.message
    );
}

#[test]
fn test_v2_standards_m2_parser_rejects_malformed_mixed_member_bodies() {
    let errors = parse_script_package_errors_from_inline(
        "malformed_mixed_member_body",
        "std geo: pro = {\n\
             var size: = 1;\n\
             fun area(): int;\n\
         };\n",
    );

    let first = errors.first().expect("parser should report a first error");
    assert!(
        first.message.contains("Expected type reference")
            || first.message.contains("Expected")
            || first.message.contains("Unexpected token")
            || first.message.contains(
                "Protocol standards currently support only routine, alias, type, and constant declarations",
            ),
        "malformed mixed standard members should fail explicitly, got: {}",
        first.message
    );
}

#[test]
fn test_v2_standards_m2_parser_rejects_malformed_standard_routine_punctuation() {
    let errors = parse_script_package_errors_from_inline(
        "malformed_standard_routine_punctuation",
        "std geo: pro = {\n\
             fun area(width int): int;\n\
         };\n",
    );

    let first = errors.first().expect("parser should report a first error");
    assert!(
        first.message.contains("Expected ':'")
            || first.message.contains("Expected parameter name")
            || first.message.contains("Expected ')' after parameter list")
            || first.message.contains("Expected type reference")
            || first
                .message
                .contains("Expected ',', ';', or ')' after generic parameter"),
        "malformed standard routine punctuation should fail explicitly, got: {}",
        first.message
    );
}

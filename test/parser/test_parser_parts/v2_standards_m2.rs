use super::*;
use fol_parser::ast::StandardKind;

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

use super::*;

#[test]
fn test_former_std_type_references_now_lower_as_pkg() {
    let mut file_stream = FileStream::from_file("test/parser/simple_source_pkg_types.fol")
        .expect("Should read pkg source-kind type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower pkg source-kind types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::UseDecl {
                    name,
                    path_type: FolType::Package { name: kind_name },
                    import_target,
                    ..
                } if name == "json"
                    && kind_name.is_empty()
                    && import_target == "json"
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Package { name: kind_name }
                    },
                    ..
                } if name == "JsonPkg" && kind_name == "registry"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_url_source_kind_reports_pkg_migration_diagnostic() {
    let mut file_stream = FileStream::from_file("test/parser/simple_source_url_types.fol")
        .expect("Should read url source-kind type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let errors = parser
        .parse(&mut lexer)
        .expect_err("Parser should reject legacy url source-kind syntax");

    let message = errors
        .first()
        .expect("First parser error should exist")
        .message.clone();
    assert!(
        message.contains("Legacy source kind 'url' was removed; use 'pkg' instead"),
        "Parser should direct legacy url users to pkg, got: {message}",
    );
}

#[test]
fn test_loc_type_references_lower_structurally() {
    let mut file_stream = FileStream::from_file("test/parser/simple_source_loc_types.fol")
        .expect("Should read loc source-kind type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower loc source-kind types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::UseDecl {
                    name,
                    path_type: FolType::Location { name: kind_name },
                    ..
                } if name == "local" && kind_name.is_empty()
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Location { name: kind_name }
                    },
                    ..
                } if name == "LocalPath" && kind_name == "disk"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

#[test]
fn test_pkg_type_references_lower_structurally() {
    let mut file_stream = FileStream::from_file("test/parser/simple_source_std_types.fol")
        .expect("Should read pkg source-kind type fixture");

    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let ast = parser
        .parse(&mut lexer)
        .expect("Parser should lower pkg source-kind types");

    match ast {
        AstNode::Program { declarations } => {
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::UseDecl {
                    name,
                    path_type: FolType::Package { name: kind_name },
                    ..
                } if name == "fmt" && kind_name.is_empty()
            )));
            assert!(declarations.iter().any(|node| matches!(
                node,
                AstNode::TypeDecl {
                    name,
                    type_def: TypeDefinition::Alias {
                        target: FolType::Package { name: kind_name }
                    },
                    ..
                } if name == "StdPkg" && kind_name == "pkg"
            )));
        }
        _ => panic!("Expected program node"),
    }
}

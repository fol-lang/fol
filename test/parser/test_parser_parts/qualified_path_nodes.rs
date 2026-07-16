use super::*;

#[test]
fn test_qualified_path_helpers_preserve_segment_structure() {
    let path = QualifiedPath::new(vec![
        "io".to_string(),
        "console".to_string(),
        "writer".to_string(),
    ]);

    assert!(path.is_qualified());
    assert_eq!(
        path.segments,
        vec!["io".to_string(), "console".to_string(), "writer".to_string()]
    );
    assert_eq!(path.joined(), "io::console::writer");
}

#[test]
fn test_qualified_path_from_joined_keeps_single_and_multi_segment_shapes() {
    let single = QualifiedPath::from_joined("value");
    assert!(!single.is_qualified());
    assert_eq!(single.segments, vec!["value".to_string()]);
    assert_eq!(single.joined(), "value");

    let multi = QualifiedPath::from_joined("pkg::cache::entry");
    assert!(multi.is_qualified());
    assert_eq!(
        multi.segments,
        vec!["pkg".to_string(), "cache".to_string(), "entry".to_string()]
    );
    assert_eq!(multi.joined(), "pkg::cache::entry");
}

#[test]
fn test_named_type_helpers_preserve_flat_and_structured_forms() {
    let flat = FolType::Named {
        syntax_id: None,
        name: "value".to_string(),
    };
    assert_eq!(flat.named_text().as_deref(), Some("value"));

    let structured = FolType::QualifiedNamed {
        path: QualifiedPath::new(vec![
            "pkg".to_string(),
            "cache".to_string(),
            "entry".to_string(),
        ]),
    };
    assert_eq!(structured.named_text().as_deref(), Some("pkg::cache::entry"));
}

#[test]
fn test_method_call_children_include_structured_receiver_before_args() {
    let receiver = AstNode::QualifiedIdentifier {
        path: QualifiedPath::new(vec![
            "pkg".to_string(),
            "cache".to_string(),
            "entry".to_string(),
        ]),
    };
    let call = AstNode::MethodCall {
        syntax_id: None,
        object: Box::new(receiver),
        method: "read".to_string(),
        args: vec![AstNode::Literal(Literal::Integer(1))],
    };

    let children = call.children();
    assert_eq!(children.len(), 2);
    assert!(matches!(
        children[0],
        AstNode::QualifiedIdentifier { path }
            if path.segments
                == vec![
                    "pkg".to_string(),
                    "cache".to_string(),
                    "entry".to_string(),
                ]
    ));
    assert!(matches!(
        children[1],
        AstNode::Literal(Literal::Integer(1))
    ));
}

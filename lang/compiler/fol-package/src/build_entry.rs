#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEntrySignatureExpectation {
    pub return_type_names: Vec<String>,
}

impl BuildEntrySignatureExpectation {
    pub fn canonical() -> Self {
        Self {
            return_type_names: vec!["non".to_string(), "none".to_string()],
        }
    }

    pub fn accepts_return_type(&self, name: &str) -> bool {
        self.return_type_names
            .iter()
            .any(|candidate| candidate == name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEntryCandidate {
    pub source_unit_path: String,
    pub syntax_id: fol_parser::ast::SyntaxNodeId,
    pub name: String,
    pub parameter_names: Vec<String>,
    pub parameter_type_names: Vec<Option<String>>,
    pub return_type_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBuildEntry {
    pub candidate: BuildEntryCandidate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEntryValidationErrorKind {
    MissingEntry,
    MultipleEntries,
    WrongParameterCount,
    WrongParameterType,
    WrongReturnType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildEntryValidationError {
    pub kind: BuildEntryValidationErrorKind,
    pub message: String,
    pub origin: Option<fol_parser::ast::SyntaxOrigin>,
}

impl BuildEntryValidationError {
    pub fn new(kind: BuildEntryValidationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: None,
        }
    }

    pub fn with_origin(
        kind: BuildEntryValidationErrorKind,
        message: impl Into<String>,
        origin: fol_parser::ast::SyntaxOrigin,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            origin: Some(origin),
        }
    }
}

pub fn collect_build_entry_candidates(
    syntax: &fol_parser::ast::ParsedPackage,
) -> Vec<BuildEntryCandidate> {
    let mut candidates = Vec::new();

    for source_unit in &syntax.source_units {
        if source_unit.kind != fol_parser::ast::ParsedSourceUnitKind::Build {
            continue;
        }

        for item in &source_unit.items {
            let fol_parser::ast::AstNode::ProDecl {
                name,
                params,
                return_type,
                ..
            } = &item.node
            else {
                continue;
            };

            if name != "build" {
                continue;
            }

            candidates.push(BuildEntryCandidate {
                source_unit_path: source_unit.path.clone(),
                syntax_id: item.node_id,
                name: name.clone(),
                parameter_names: params.iter().map(|param| param.name.clone()).collect(),
                parameter_type_names: params
                    .iter()
                    .map(|param| param.param_type.named_text())
                    .collect(),
                return_type_name: build_entry_type_label(return_type.as_ref()),
            });
        }
    }

    candidates
}

pub fn validate_build_entry_cardinality(
    syntax: &fol_parser::ast::ParsedPackage,
    candidates: &[BuildEntryCandidate],
) -> Result<ValidatedBuildEntry, Vec<BuildEntryValidationError>> {
    match candidates {
        [] => Err(vec![BuildEntryValidationError::new(
            BuildEntryValidationErrorKind::MissingEntry,
            "build.fol must declare exactly one canonical `pro[] build(): non` entry",
        )]),
        [candidate] => Ok(ValidatedBuildEntry {
            candidate: candidate.clone(),
        }),
        many => Err(many
            .iter()
            .map(|candidate| {
                BuildEntryValidationError::with_origin(
                    BuildEntryValidationErrorKind::MultipleEntries,
                    "multiple semantic `build` entries were found in build source units",
                    syntax
                        .syntax_index
                        .origin(candidate.syntax_id)
                        .cloned()
                        .unwrap_or(fol_parser::ast::SyntaxOrigin {
                            file: Some(candidate.source_unit_path.clone()),
                            line: 1,
                            column: 1,
                            length: 5,
                        }),
                )
            })
            .collect()),
    }
}

pub fn validate_build_entry_parameter_shape(
    entry: ValidatedBuildEntry,
) -> Result<ValidatedBuildEntry, Vec<BuildEntryValidationError>> {
    if !entry.candidate.parameter_names.is_empty() {
        return Err(vec![BuildEntryValidationError::new(
            BuildEntryValidationErrorKind::WrongParameterCount,
            "canonical build entry must not declare parameters; use `.graph()` inside `build.fol` instead",
        )]);
    }

    Ok(entry)
}

pub fn validate_build_entry_parameter_type(
    entry: ValidatedBuildEntry,
    _expectation: &BuildEntrySignatureExpectation,
) -> Result<ValidatedBuildEntry, Vec<BuildEntryValidationError>> {
    if !entry.candidate.parameter_type_names.is_empty() {
        return Err(vec![BuildEntryValidationError::new(
            BuildEntryValidationErrorKind::WrongParameterType,
            "canonical build entry does not accept a public `Graph` parameter type; use `.graph()` inside `build.fol` instead",
        )]);
    }

    Ok(entry)
}

pub fn validate_build_entry_return_type(
    entry: ValidatedBuildEntry,
    expectation: &BuildEntrySignatureExpectation,
) -> Result<ValidatedBuildEntry, Vec<BuildEntryValidationError>> {
    let Some(return_type_name) = entry.candidate.return_type_name.as_deref() else {
        return Err(vec![BuildEntryValidationError::new(
            BuildEntryValidationErrorKind::WrongReturnType,
            "canonical build entry must declare return type 'non'",
        )]);
    };

    if !expectation.accepts_return_type(return_type_name) {
        return Err(vec![BuildEntryValidationError::new(
            BuildEntryValidationErrorKind::WrongReturnType,
            format!(
                "canonical build entry return type '{}' is not one of the accepted non-returning procedure types",
                return_type_name
            ),
        )]);
    }

    Ok(entry)
}

pub fn validate_parsed_build_entry(
    syntax: &fol_parser::ast::ParsedPackage,
    expectation: &BuildEntrySignatureExpectation,
) -> Result<ValidatedBuildEntry, Vec<BuildEntryValidationError>> {
    let candidates = collect_build_entry_candidates(syntax);
    let entry = validate_build_entry_cardinality(syntax, &candidates)?;
    let entry = validate_build_entry_parameter_shape(entry)?;
    let entry = validate_build_entry_parameter_type(entry, expectation)?;
    validate_build_entry_return_type(entry, expectation)
}

fn build_entry_type_label(fol_type: Option<&fol_parser::ast::FolType>) -> Option<String> {
    match fol_type {
        Some(fol_parser::ast::FolType::None) => Some("non".to_string()),
        Some(fol_type) => fol_type.named_text(),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        collect_build_entry_candidates, validate_build_entry_cardinality,
        validate_build_entry_parameter_shape, validate_build_entry_parameter_type,
        validate_build_entry_return_type, validate_parsed_build_entry, BuildEntryCandidate,
        BuildEntrySignatureExpectation, BuildEntryValidationError, BuildEntryValidationErrorKind,
        ValidatedBuildEntry,
    };

    #[test]
    fn canonical_build_entry_signature_expectation_requires_non_return_only() {
        let expectation = BuildEntrySignatureExpectation::canonical();

        assert!(expectation.accepts_return_type("non"));
        assert!(expectation.accepts_return_type("none"));
    }

    #[test]
    fn build_entry_candidate_and_validation_error_models_capture_core_metadata() {
        let candidate = BuildEntryCandidate {
            source_unit_path: "build.fol".to_string(),
            syntax_id: fol_parser::ast::SyntaxNodeId(7),
            name: "build".to_string(),
            parameter_names: Vec::new(),
            parameter_type_names: Vec::new(),
            return_type_name: Some("non".to_string()),
        };
        let validated = ValidatedBuildEntry {
            candidate: candidate.clone(),
        };
        let error = BuildEntryValidationError::new(
            BuildEntryValidationErrorKind::WrongReturnType,
            "wrong return type",
        );

        assert_eq!(validated.candidate, candidate);
        assert_eq!(error.kind, BuildEntryValidationErrorKind::WrongReturnType);
        assert!(error.origin.is_none());
    }

    #[test]
    fn candidate_collection_scans_only_build_source_units() {
        let syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![
                fol_parser::ast::ParsedSourceUnit {
                    path: "build.fol".to_string(),
                    package: "demo".to_string(),
                    namespace: "demo".to_string(),
                    kind: fol_parser::ast::ParsedSourceUnitKind::Build,
                    items: vec![fol_parser::ast::ParsedTopLevel {
                        node_id: fol_parser::ast::SyntaxNodeId(1),
                        node: fol_parser::ast::AstNode::ProDecl {
                            syntax_id: None,
                            options: Vec::new(),
                            generics: Vec::new(),
                            name: "build".to_string(),
                            receiver_type: None,
                            captures: Vec::new(),
                            params: Vec::new(),
                            return_type: Some(fol_parser::ast::FolType::None),
                            error_type: None,
                            body: Vec::new(),
                            inquiries: Vec::new(),
                        },
                        meta: fol_parser::ast::ParsedTopLevelMeta::default(),
                    }],
                },
                fol_parser::ast::ParsedSourceUnit {
                    path: "src/main.fol".to_string(),
                    package: "demo".to_string(),
                    namespace: "demo::src".to_string(),
                    kind: fol_parser::ast::ParsedSourceUnitKind::Ordinary,
                    items: Vec::new(),
                },
            ],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };

        let candidates = collect_build_entry_candidates(&syntax);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source_unit_path, "build.fol");
        assert!(candidates[0].parameter_names.is_empty());
        assert_eq!(candidates[0].return_type_name.as_deref(), Some("non"));
    }

    #[test]
    fn candidate_collection_ignores_old_def_build_declarations() {
        let syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "build.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: fol_parser::ast::ParsedSourceUnitKind::Build,
                items: vec![fol_parser::ast::ParsedTopLevel {
                    node_id: fol_parser::ast::SyntaxNodeId(1),
                    node: fol_parser::ast::AstNode::DefDecl {
                        options: Vec::new(),
                        name: "build".to_string(),
                        params: Vec::new(),
                        def_type: fol_parser::ast::FolType::Named {
                            syntax_id: None,
                            name: "Graph".to_string(),
                        },
                        body: Vec::new(),
                    },
                    meta: fol_parser::ast::ParsedTopLevelMeta::default(),
                }],
            }],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };

        let candidates = collect_build_entry_candidates(&syntax);

        assert!(candidates.is_empty());
    }

    #[test]
    fn candidate_collection_ignores_fun_build_declarations() {
        let syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "build.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: fol_parser::ast::ParsedSourceUnitKind::Build,
                items: vec![fol_parser::ast::ParsedTopLevel {
                    node_id: fol_parser::ast::SyntaxNodeId(1),
                    node: fol_parser::ast::AstNode::FunDecl {
                        syntax_id: None,
                        options: Vec::new(),
                        generics: Vec::new(),
                        name: "build".to_string(),
                        receiver_type: None,
                        captures: Vec::new(),
                        params: Vec::new(),
                        return_type: Some(fol_parser::ast::FolType::None),
                        error_type: None,
                        body: Vec::new(),
                        inquiries: Vec::new(),
                    },
                    meta: fol_parser::ast::ParsedTopLevelMeta::default(),
                }],
            }],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };

        let candidates = collect_build_entry_candidates(&syntax);

        assert!(candidates.is_empty());
    }

    #[test]
    fn parsed_build_entry_validation_treats_old_def_build_as_missing() {
        let syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "build.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: fol_parser::ast::ParsedSourceUnitKind::Build,
                items: vec![fol_parser::ast::ParsedTopLevel {
                    node_id: fol_parser::ast::SyntaxNodeId(1),
                    node: fol_parser::ast::AstNode::DefDecl {
                        options: Vec::new(),
                        name: "build".to_string(),
                        params: Vec::new(),
                        def_type: fol_parser::ast::FolType::Named {
                            syntax_id: None,
                            name: "Graph".to_string(),
                        },
                        body: Vec::new(),
                    },
                    meta: fol_parser::ast::ParsedTopLevelMeta::default(),
                }],
            }],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };

        let errors = validate_parsed_build_entry(&syntax, &BuildEntrySignatureExpectation::canonical())
            .expect_err("old def build declarations should not satisfy the semantic build entry contract");

        assert_eq!(errors[0].kind, BuildEntryValidationErrorKind::MissingEntry);
    }

    #[test]
    fn cardinality_validation_requires_exactly_one_build_entry() {
        let syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };
        let missing = validate_build_entry_cardinality(&syntax, &[])
            .expect_err("missing semantic build entries should fail");
        assert_eq!(missing[0].kind, BuildEntryValidationErrorKind::MissingEntry);

        let candidate = BuildEntryCandidate {
            source_unit_path: "build.fol".to_string(),
            syntax_id: fol_parser::ast::SyntaxNodeId(1),
            name: "build".to_string(),
            parameter_names: Vec::new(),
            parameter_type_names: Vec::new(),
            return_type_name: Some("non".to_string()),
        };
        let validated = validate_build_entry_cardinality(&syntax, &[candidate.clone()])
            .expect("one semantic build entry should pass cardinality validation");
        assert_eq!(validated.candidate, candidate);
    }

    #[test]
    fn parameter_shape_validation_requires_no_parameters() {
        let valid = ValidatedBuildEntry {
            candidate: BuildEntryCandidate {
                source_unit_path: "build.fol".to_string(),
                syntax_id: fol_parser::ast::SyntaxNodeId(1),
                name: "build".to_string(),
                parameter_names: Vec::new(),
                parameter_type_names: Vec::new(),
                return_type_name: Some("non".to_string()),
            },
        };
        assert!(validate_build_entry_parameter_shape(valid).is_ok());

        let invalid = ValidatedBuildEntry {
            candidate: BuildEntryCandidate {
                source_unit_path: "build.fol".to_string(),
                syntax_id: fol_parser::ast::SyntaxNodeId(2),
                name: "build".to_string(),
                parameter_names: vec!["graph".to_string()],
                parameter_type_names: vec![Some("Graph".to_string())],
                return_type_name: Some("non".to_string()),
            },
        };
        let errors = validate_build_entry_parameter_shape(invalid)
            .expect_err("multiple parameters should fail semantic build entry validation");
        assert_eq!(
            errors[0].kind,
            BuildEntryValidationErrorKind::WrongParameterCount
        );
    }

    #[test]
    fn parameter_type_validation_rejects_public_graph_type_usage() {
        let expectation = BuildEntrySignatureExpectation::canonical();
        let valid = ValidatedBuildEntry {
            candidate: BuildEntryCandidate {
                source_unit_path: "build.fol".to_string(),
                syntax_id: fol_parser::ast::SyntaxNodeId(1),
                name: "build".to_string(),
                parameter_names: Vec::new(),
                parameter_type_names: Vec::new(),
                return_type_name: Some("non".to_string()),
            },
        };
        assert!(validate_build_entry_parameter_type(valid, &expectation).is_ok());

        let invalid = ValidatedBuildEntry {
            candidate: BuildEntryCandidate {
                source_unit_path: "build.fol".to_string(),
                syntax_id: fol_parser::ast::SyntaxNodeId(2),
                name: "build".to_string(),
                parameter_names: vec!["graph".to_string()],
                parameter_type_names: vec![Some("Graph".to_string())],
                return_type_name: Some("non".to_string()),
            },
        };
        let errors = validate_build_entry_parameter_type(invalid, &expectation)
            .expect_err("public Graph parameter types should fail semantic build entry validation");
        assert_eq!(
            errors[0].kind,
            BuildEntryValidationErrorKind::WrongParameterType
        );
    }

    #[test]
    fn return_type_validation_requires_non_returning_procedure_type() {
        let expectation = BuildEntrySignatureExpectation::canonical();
        let valid = ValidatedBuildEntry {
            candidate: BuildEntryCandidate {
                source_unit_path: "build.fol".to_string(),
                syntax_id: fol_parser::ast::SyntaxNodeId(1),
                name: "build".to_string(),
                parameter_names: Vec::new(),
                parameter_type_names: Vec::new(),
                return_type_name: Some("non".to_string()),
            },
        };
        assert!(validate_build_entry_return_type(valid, &expectation).is_ok());

        let invalid = ValidatedBuildEntry {
            candidate: BuildEntryCandidate {
                source_unit_path: "build.fol".to_string(),
                syntax_id: fol_parser::ast::SyntaxNodeId(2),
                name: "build".to_string(),
                parameter_names: Vec::new(),
                parameter_type_names: Vec::new(),
                return_type_name: Some("int".to_string()),
            },
        };
        let errors = validate_build_entry_return_type(invalid, &expectation)
            .expect_err("non-non return types should fail semantic build entry validation");
        assert_eq!(
            errors[0].kind,
            BuildEntryValidationErrorKind::WrongReturnType
        );
    }

    #[test]
    fn parsed_build_entry_validation_reports_multiple_and_wrong_type_shapes() {
        let expectation = BuildEntrySignatureExpectation::canonical();
        let mut syntax = fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "build.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: fol_parser::ast::ParsedSourceUnitKind::Build,
                items: vec![],
            }],
            syntax_index: fol_parser::ast::SyntaxIndex::default(),
        };

        let missing = validate_parsed_build_entry(&syntax, &expectation)
            .expect_err("missing entries should fail semantic validation");
        assert_eq!(missing[0].kind, BuildEntryValidationErrorKind::MissingEntry);

        syntax.source_units[0].items = vec![
            fol_parser::ast::ParsedTopLevel {
                node_id: fol_parser::ast::SyntaxNodeId(1),
                node: fol_parser::ast::AstNode::ProDecl {
                    syntax_id: None,
                    options: Vec::new(),
                    generics: Vec::new(),
                    name: "build".to_string(),
                    receiver_type: None,
                    captures: Vec::new(),
                    params: Vec::new(),
                    return_type: Some(fol_parser::ast::FolType::Named {
                        syntax_id: None,
                        name: "int".to_string(),
                    }),
                    error_type: None,
                    body: Vec::new(),
                    inquiries: Vec::new(),
                },
                meta: fol_parser::ast::ParsedTopLevelMeta::default(),
            },
            fol_parser::ast::ParsedTopLevel {
                node_id: fol_parser::ast::SyntaxNodeId(2),
                node: fol_parser::ast::AstNode::ProDecl {
                    syntax_id: None,
                    options: Vec::new(),
                    generics: Vec::new(),
                    name: "build".to_string(),
                    receiver_type: None,
                    captures: Vec::new(),
                    params: Vec::new(),
                    return_type: Some(fol_parser::ast::FolType::None),
                    error_type: None,
                    body: Vec::new(),
                    inquiries: Vec::new(),
                },
                meta: fol_parser::ast::ParsedTopLevelMeta::default(),
            },
        ];

        let multiple = validate_parsed_build_entry(&syntax, &expectation)
            .expect_err("multiple entries should fail semantic validation");
        assert_eq!(
            multiple[0].kind,
            BuildEntryValidationErrorKind::MultipleEntries
        );
    }
}

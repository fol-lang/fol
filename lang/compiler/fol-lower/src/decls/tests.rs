#[cfg(test)]
mod tests {
    use super::super::{
        lower_alias_declarations, lower_entry_declarations, lower_global_declarations,
        lower_record_declarations, lower_routine_decl, lower_routine_declarations,
        lower_routine_signatures,
    };
    use crate::{
        types::LoweredType, LoweredBuiltinType, LoweredFieldLayout, LoweredPackage,
        LoweredTypeDeclKind, LoweredVariantLayout,
    };
    use fol_parser::ast::{AstNode, AstParser, FolType, Parameter};
    use fol_resolver::resolve_package_workspace;
    use fol_stream::FileStream;
    use fol_typecheck::Typechecker;

    fn safe_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("fol_test");
        std::fs::create_dir_all(&dir).expect("should create test temp root");
        dir
    }

    #[test]
    fn declaration_lowering_maps_top_level_routine_signatures_to_lowered_types() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_routine_sig_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(&fixture, "fun[] greet(count: int): str = { return \"ok\" }")
            .expect("should write lowering signature fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");
        let lowered_workspace = crate::LoweringSession::new(typed.clone())
            .lower_workspace()
            .expect("workspace lowering should succeed");
        let typed_package = typed.entry_package();
        let mut lowered_package =
            LoweredPackage::new(crate::LoweredPackageId(0), typed_package.identity.clone());
        lowered_package.checked_type_map =
            lowered_workspace.entry_package().checked_type_map.clone();

        lower_routine_signatures(typed_package, &mut lowered_package)
            .expect("top-level routine signatures should lower cleanly");

        let lowered_signature = lowered_package
            .routine_signatures
            .values()
            .next()
            .expect("routine signature should be recorded");
        let signature = lowered_workspace
            .type_table()
            .get(*lowered_signature)
            .expect("lowered signature type should exist");

        match signature {
            LoweredType::Routine(signature) => {
                assert_eq!(
                    lowered_workspace.type_table().get(signature.params[0]),
                    Some(&LoweredType::Builtin(LoweredBuiltinType::Int))
                );
                assert_eq!(
                    signature
                        .return_type
                        .and_then(|type_id| lowered_workspace.type_table().get(type_id)),
                    Some(&LoweredType::Builtin(LoweredBuiltinType::Str))
                );
            }
            other => panic!("expected lowered routine type, got {other:?}"),
        }
    }

    #[test]
    fn declaration_lowering_supports_generic_routines_in_the_lowered_workspace() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_generic_routine_m1_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "fun pick(T)(value: T): T = { return value }\nfun[] main(): int = { return pick(7) }",
        )
        .expect("should write generic routine lowering fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        let lowered = crate::LoweringSession::new(typed)
            .lower_workspace()
            .expect("generic routine lowering should succeed");
        assert!(
            (0..lowered.type_table().len()).any(|index| matches!(
                lowered.type_table().get(crate::LoweredTypeId(index)),
                Some(LoweredType::GenericParameter { name }) if name == "T"
            ))
        );
    }

    #[test]
    fn declaration_lowering_supports_multi_param_generic_routines_in_the_lowered_workspace() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_generic_routine_pair_m1_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "fun pair(T)(left: T, right: T): T = { return right }\nfun[] main(): int = { return pair(1, 2) }",
        )
        .expect("should write generic pair lowering fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        let lowered = crate::LoweringSession::new(typed)
            .lower_workspace()
            .expect("generic pair lowering should succeed");
        assert!(
            (0..lowered.type_table().len())
                .filter(|index| matches!(
                    lowered.type_table().get(crate::LoweredTypeId(*index)),
                    Some(LoweredType::GenericParameter { .. })
                ))
                .count()
                >= 1
        );
    }

    #[test]
    fn declaration_lowering_supports_return_only_generic_routines_in_the_lowered_workspace() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_generic_return_only_m1_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "fun make(T)(): T = { panic(\"boom\") }\nfun[] main(): int = { return 0 }",
        )
        .expect("should write generic return-only lowering fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        crate::LoweringSession::new(typed)
            .lower_workspace()
            .expect("generic return-only lowering should succeed");
    }

    #[test]
    fn declaration_lowering_supports_generic_routines_with_default_params_in_the_lowered_workspace() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_generic_defaults_m1_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "fun pick(T)(value: T, fallback: int = 1): T = { return value }\nfun[] main(): int = { return pick(7) }",
        )
        .expect("should write generic default-params lowering fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        crate::LoweringSession::new(typed)
            .lower_workspace()
            .expect("generic default-params lowering should succeed");
    }

    #[test]
    fn declaration_lowering_rejects_protocol_standards_with_explicit_m2_boundary() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_standard_protocol_m2_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            concat!(
                "std geo: pro = {\n",
                "    fun area(): int;\n",
                "};\n",
                "typ Rect()(geo): rec = {\n",
                "    var width: int;\n",
                "};\n",
                "fun (Rect)area(): int = {\n",
                "    return 1;\n",
                "};\n",
                "fun[] main(): int = {\n",
                "    return 0;\n",
                "}\n",
            ),
        )
        .expect("should write standard lowering fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        let errors = crate::LoweringSession::new(typed)
            .lower_workspace()
            .expect_err("protocol standard lowering should stay explicitly unsupported in M2");

        assert!(errors.iter().any(|error| {
            error.kind() == crate::LoweringErrorKind::Unsupported
                && error
                    .message()
                    .contains("protocol standard lowering is not yet supported in V2 Milestone 2")
        }));
    }

    #[test]
    fn declaration_lowering_records_aliases_as_erased_runtime_shapes() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_alias_decl_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "ali Counter: int\nfun[] main(value: Counter): Counter = { return value }",
        )
        .expect("should write lowering alias fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");
        let lowered_workspace = crate::LoweringSession::new(typed.clone())
            .lower_workspace()
            .expect("workspace lowering should succeed");
        let typed_package = typed.entry_package();
        let mut lowered_package =
            LoweredPackage::new(crate::LoweredPackageId(0), typed_package.identity.clone());
        lowered_package.checked_type_map =
            lowered_workspace.entry_package().checked_type_map.clone();

        lower_alias_declarations(typed_package, &mut lowered_package)
            .expect("aliases should lower as erased runtime declarations");

        let alias_decl = lowered_package
            .type_decls
            .values()
            .next()
            .expect("alias declaration should be recorded");
        assert_eq!(alias_decl.name, "Counter");
        assert_eq!(
            lowered_workspace.type_table().get(alias_decl.runtime_type),
            Some(&LoweredType::Builtin(LoweredBuiltinType::Int))
        );
        assert_eq!(
            alias_decl.kind,
            LoweredTypeDeclKind::Alias {
                target_type: alias_decl.runtime_type
            }
        );
    }

    #[test]
    fn declaration_lowering_records_explicit_record_field_layouts() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_record_decl_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "typ Point: { x: int, y: str }\nfun[] main(): int = { return 0 }",
        )
        .expect("should write lowering record fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");
        let lowered_workspace = crate::LoweringSession::new(typed.clone())
            .lower_workspace()
            .expect("workspace lowering should succeed");
        let typed_package = typed.entry_package();
        let mut lowered_package =
            LoweredPackage::new(crate::LoweredPackageId(0), typed_package.identity.clone());
        lowered_package.checked_type_map =
            lowered_workspace.entry_package().checked_type_map.clone();

        lower_record_declarations(typed_package, &mut lowered_package)
            .expect("record declarations should lower cleanly");

        let record_decl = lowered_package
            .type_decls
            .values()
            .next()
            .expect("record declaration should be recorded");

        assert_eq!(record_decl.name, "Point");
        assert_eq!(
            record_decl.kind,
            LoweredTypeDeclKind::Record {
                fields: vec![
                    LoweredFieldLayout {
                        name: "x".to_string(),
                        type_id: lowered_workspace.entry_package().checked_type_map
                            [&fol_typecheck::CheckedTypeId(0)],
                    },
                    LoweredFieldLayout {
                        name: "y".to_string(),
                        type_id: lowered_workspace.entry_package().checked_type_map
                            [&fol_typecheck::CheckedTypeId(4)],
                    },
                ],
            }
        );
        assert_eq!(
            lowered_workspace.type_table().get(record_decl.runtime_type),
            Some(&LoweredType::Record {
                fields: std::collections::BTreeMap::from([
                    (
                        "x".to_string(),
                        lowered_workspace.entry_package().checked_type_map
                            [&fol_typecheck::CheckedTypeId(0)],
                    ),
                    (
                        "y".to_string(),
                        lowered_workspace.entry_package().checked_type_map
                            [&fol_typecheck::CheckedTypeId(4)],
                    ),
                ]),
            })
        );
    }

    #[test]
    fn declaration_lowering_records_explicit_entry_variant_layouts() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_entry_decl_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "typ Outcome: ent = { var Ok: int; var Err: str }\nfun[] main(): int = { return 0 }",
        )
        .expect("should write lowering entry fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");
        let lowered_workspace = crate::LoweringSession::new(typed.clone())
            .lower_workspace()
            .expect("workspace lowering should succeed");
        let typed_package = typed.entry_package();
        let mut lowered_package =
            LoweredPackage::new(crate::LoweredPackageId(0), typed_package.identity.clone());
        lowered_package.checked_type_map =
            lowered_workspace.entry_package().checked_type_map.clone();

        lower_entry_declarations(typed_package, &mut lowered_package)
            .expect("entry declarations should lower cleanly");

        let entry_decl = lowered_package
            .type_decls
            .values()
            .next()
            .expect("entry declaration should be recorded");

        assert_eq!(
            entry_decl.kind,
            LoweredTypeDeclKind::Entry {
                variants: vec![
                    LoweredVariantLayout {
                        name: "Err".to_string(),
                        payload_type: Some(
                            lowered_workspace.entry_package().checked_type_map
                                [&fol_typecheck::CheckedTypeId(4)],
                        ),
                    },
                    LoweredVariantLayout {
                        name: "Ok".to_string(),
                        payload_type: Some(
                            lowered_workspace.entry_package().checked_type_map
                                [&fol_typecheck::CheckedTypeId(0)],
                        ),
                    },
                ],
            }
        );
        assert_eq!(
            lowered_workspace.type_table().get(entry_decl.runtime_type),
            Some(&LoweredType::Entry {
                variants: std::collections::BTreeMap::from([
                    (
                        "Err".to_string(),
                        Some(
                            lowered_workspace.entry_package().checked_type_map
                                [&fol_typecheck::CheckedTypeId(4)],
                        ),
                    ),
                    (
                        "Ok".to_string(),
                        Some(
                            lowered_workspace.entry_package().checked_type_map
                                [&fol_typecheck::CheckedTypeId(0)],
                        ),
                    ),
                ]),
            })
        );
    }

    #[test]
    fn declaration_lowering_records_top_level_globals_with_storage_types() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_globals_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(&fixture, "var count: int = 1\nlab label: str = \"ok\"")
            .expect("should write lowering global fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");
        let lowered_workspace = crate::LoweringSession::new(typed.clone())
            .lower_workspace()
            .expect("workspace lowering should succeed");
        let typed_package = typed.entry_package();
        let mut lowered_package =
            LoweredPackage::new(crate::LoweredPackageId(0), typed_package.identity.clone());
        lowered_package.checked_type_map =
            lowered_workspace.entry_package().checked_type_map.clone();
        let mut next_global_index = 0;

        lower_global_declarations(typed_package, &mut lowered_package, &mut next_global_index)
            .expect("top-level globals should lower cleanly");

        assert_eq!(lowered_package.globals.len(), 2);
        let globals = lowered_package
            .globals
            .iter()
            .map(|id| {
                lowered_package
                    .global_decls
                    .get(id)
                    .expect("global id should resolve")
            })
            .collect::<Vec<_>>();
        assert_eq!(globals[0].name, "count");
        assert!(globals[0].mutable);
        assert_eq!(
            lowered_workspace.type_table().get(globals[0].type_id),
            Some(&LoweredType::Builtin(LoweredBuiltinType::Int))
        );
        assert_eq!(globals[1].name, "label");
        assert!(!globals[1].mutable);
        assert_eq!(
            lowered_workspace.type_table().get(globals[1].type_id),
            Some(&LoweredType::Builtin(LoweredBuiltinType::Str))
        );
    }

    #[test]
    fn declaration_lowering_records_routine_shells_with_parameter_locals() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_routines_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            "fun[] add(left: int, right: int): int = { return left }",
        )
        .expect("should write lowering routine fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");
        let lowered_workspace = crate::LoweringSession::new(typed.clone())
            .lower_workspace()
            .expect("workspace lowering should succeed");
        let typed_package = typed.entry_package();
        let mut lowered_package =
            LoweredPackage::new(crate::LoweredPackageId(0), typed_package.identity.clone());
        lowered_package.checked_type_map =
            lowered_workspace.entry_package().checked_type_map.clone();
        lower_routine_signatures(typed_package, &mut lowered_package)
            .expect("routine signatures should lower cleanly");
        let mut next_routine_index = 0;

        lower_routine_declarations(typed_package, &mut lowered_package, &mut next_routine_index)
            .expect("top-level routines should lower cleanly");

        assert_eq!(lowered_package.routines.len(), 1);
        let routine = lowered_package
            .routine_decls
            .get(&lowered_package.routines[0])
            .expect("routine id should resolve");
        assert_eq!(routine.name, "add");
        assert_eq!(routine.params.len(), 2);
        assert_eq!(routine.entry_block, crate::LoweredBlockId(0));
        assert_eq!(
            routine
                .params
                .iter()
                .map(|local_id| routine
                    .locals
                    .get(*local_id)
                    .and_then(|local| local.name.clone()))
                .collect::<Vec<_>>(),
            vec![Some("left".to_string()), Some("right".to_string())]
        );
    }

    #[test]
    fn declaration_lowering_reports_missing_parameter_symbol_matches_explicitly() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_missing_param_{}.fol",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be monotonic enough for tmp names")
                .as_nanos()
        ));
        std::fs::write(&fixture, "fun[] add(left: int): int = { return left }")
            .expect("should write lowering routine fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");
        let lowered_workspace = crate::LoweringSession::new(typed.clone())
            .lower_workspace()
            .expect("workspace lowering should succeed");
        let typed_package = typed.entry_package();
        let mut lowered_package =
            LoweredPackage::new(crate::LoweredPackageId(0), typed_package.identity.clone());
        lowered_package.checked_type_map =
            lowered_workspace.entry_package().checked_type_map.clone();
        lower_routine_signatures(typed_package, &mut lowered_package)
            .expect("routine signatures should lower cleanly");

        let source_unit = typed_package
            .program
            .resolved()
            .syntax()
            .source_units
            .first()
            .expect("fixture source unit should exist");
        let AstNode::FunDecl {
            name, syntax_id, ..
        } = &source_unit.items[0].node
        else {
            panic!("fixture should retain one function declaration");
        };
        let symbol_id = super::super::find_local_symbol_id(
            &typed_package.program,
            fol_resolver::SourceUnitId(0),
            fol_resolver::SymbolKind::Routine,
            name,
        )
        .expect("routine symbol should exist");
        let mut next_routine_index = 0;
        let error = lower_routine_decl(
            typed_package,
            &lowered_package,
            symbol_id,
            fol_resolver::SourceUnitId(0),
            name,
            *syntax_id,
            &[Parameter {
                name: "missing".to_string(),
                param_type: FolType::Int { size: None, signed: true },
                is_borrowable: false,
                is_mutex: false,
                default: None,
            }],
            &mut next_routine_index,
        )
        .expect_err("mismatched parameter names should stay explicit");

        assert_eq!(error.kind(), crate::LoweringErrorKind::InvalidInput);
        assert!(
            error
                .message()
                .contains("routine parameter 'missing' does not map to a resolved symbol in its lowered scope"),
            "missing parameter matches should report the lowered-scope guard explicitly, got: {error:?}",
        );
    }

    #[test]
    fn declaration_lowering_keeps_local_and_imported_packages_separate() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tmp path")
            .as_nanos();
        let root = safe_temp_dir().join(format!("fol_lower_decl_parity_{stamp}"));
        let app_dir = root.join("app");
        let shared_dir = root.join("shared");
        fs::create_dir_all(&app_dir).expect("should create app dir");
        fs::create_dir_all(&shared_dir).expect("should create shared dir");
        fs::write(
            app_dir.join("main.fol"),
            "use shared: loc = {\"../shared\"}\nfun[] main(): int = { return answer() }",
        )
        .expect("should write app entry");
        fs::write(
            shared_dir.join("lib.fol"),
            "ali Counter: int\nvar[exp] base: Counter = 7\nfun[exp] answer(): Counter = { return base }",
        )
        .expect("should write shared library");

        let mut stream = FileStream::from_folder(app_dir.to_str().expect("utf8 temp path"))
            .expect("should open folder fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering folder fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering folder fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering folder fixture should typecheck");

        let lowered = crate::LoweringSession::new(typed)
            .lower_workspace()
            .expect("declaration lowering should succeed across imported packages");

        let app_package = lowered
            .packages()
            .find(|package| package.identity.display_name == "app")
            .expect("entry package should exist");
        let shared_package = lowered
            .packages()
            .find(|package| package.identity.display_name == "shared")
            .expect("imported package should exist");

        assert_eq!(app_package.global_decls.len(), 0);
        assert_eq!(app_package.type_decls.len(), 0);
        assert_eq!(app_package.routine_decls.len(), 1);
        assert_eq!(shared_package.type_decls.len(), 1);
        assert_eq!(shared_package.global_decls.len(), 1);
        assert_eq!(shared_package.routine_decls.len(), 1);
        assert!(
            app_package
                .symbol_ownership
                .values()
                .any(|ownership| ownership.mounted_from.is_some()),
            "entry package should retain mounted imported symbol ownership"
        );
        assert!(
            shared_package
                .symbol_ownership
                .values()
                .all(|ownership| ownership.mounted_from.is_none()),
            "owning package should not lower imported shells as local declarations"
        );
    }
}

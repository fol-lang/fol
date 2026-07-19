use super::super::cursor::{RoutineCursor, WorkspaceDeclIndex};
use super::{lower_fixture_workspace, lower_folder_fixture_workspace};
use crate::{
    types::{LoweredBuiltinType, LoweredTypeTable},
    LoweredBlock, LoweredGlobal, LoweredInstrKind, LoweredLocal, LoweredOperand, LoweredPackage,
    LoweredRoutine, LoweredTerminator, LoweredWorkspace, LoweringErrorKind,
};
use fol_parser::ast::AstParser;
use fol_parser::ast::Literal;
use fol_resolver::{
    resolve_package_workspace, PackageIdentity, PackageSourceKind, SourceUnitId, SymbolKind,
};
use fol_stream::FileStream;
use fol_typecheck::Typechecker;
use std::collections::BTreeMap;

#[test]
fn literal_lowering_emits_constant_instructions_into_the_current_block() {
    let mut types = LoweredTypeTable::new();
    let int_type = types.intern_builtin(LoweredBuiltinType::Int);
    let float_type = types.intern_builtin(LoweredBuiltinType::Float);
    let str_type = types.intern_builtin(LoweredBuiltinType::Str);

    let mut routine =
        LoweredRoutine::new(crate::LoweredRoutineId(0), "main", crate::LoweredBlockId(0));
    let entry = routine.blocks.push(LoweredBlock {
        id: crate::LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: None,
    });
    routine.entry_block = entry;
    let mut cursor = RoutineCursor::new(&mut routine, entry);

    let int_value = cursor
        .lower_literal(&Literal::Integer(7), int_type)
        .expect("integer literals should lower");
    let float_value = cursor
        .lower_literal(&Literal::Float(3.5), float_type)
        .expect("float literals should lower");
    let str_value = cursor
        .lower_literal(&Literal::String("ok".to_string()), str_type)
        .expect("string literals should lower");

    assert_eq!(
        routine
            .blocks
            .get(entry)
            .expect("entry block should exist")
            .instructions
            .len(),
        3
    );
    assert_eq!(routine.locals.len(), 3);
    assert_eq!(int_value.local_id.0, 0);
    assert_eq!(float_value.local_id.0, 1);
    assert_eq!(str_value.local_id.0, 2);
    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(0))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::Const(LoweredOperand::Int(7)))
    );
    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(1))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::Const(LoweredOperand::Float(
            3.5f64.to_bits()
        )))
    );
    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(2))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::Const(LoweredOperand::Str(
            "ok".to_string()
        )))
    );
}

#[test]
fn lowering_repro_keeps_same_name_parameters_distinct_per_routine_scope() {
    let lowered = lower_folder_fixture_workspace(&[(
        "app/main.fol",
        concat!(
            "fun[] build_user(flag: bol): int = {\n",
            "    return 1;\n",
            "};\n",
            "fun[] choose_count(flag: bol): int = {\n",
            "    return 2;\n",
            "};\n",
            "fun[] main(flag: bol): int = {\n",
            "    when(flag) {\n",
            "        case(true) { return build_user(flag); }\n",
            "        * { return choose_count(flag); }\n",
            "    }\n",
            "};\n",
        ),
    )]);
    let entry_package = lowered.entry_package();
    for routine_name in ["build_user", "choose_count", "main"] {
        let routine = entry_package
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .expect("routine shell should exist");
        let param_names = routine
            .params
            .iter()
            .filter_map(|local_id| {
                routine
                    .locals
                    .get(*local_id)
                    .and_then(|local| local.name.clone())
            })
            .collect::<Vec<_>>();
        assert!(
            param_names.iter().any(|name| name == "flag"),
            "routine '{routine_name}' should keep its own lowered flag parameter",
        );
    }
}

#[test]
fn lowering_repro_lowers_non_empty_seq_literals_in_typed_v1_contexts() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] take(values: ... str): seq[str] = {\n",
        "    return values;\n",
        "};\n",
        "fun[] from_binding(): seq[str] = {\n",
        "    var names: seq[str] = {\"Ada\", \"Lin\"};\n",
        "    return names;\n",
        "};\n",
        "fun[] from_return(): seq[str] = {\n",
        "    return {\"Ada\", \"Lin\"};\n",
        "};\n",
        "fun[] from_arg(): seq[str] = {\n",
        "    return take(\"Ada\", \"Lin\");\n",
        "};\n",
    ));

    for routine_name in ["from_binding", "from_return", "from_arg"] {
        let routine = lowered
            .entry_package()
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .expect("sequence lowering routine should exist");
        let construct = routine
            .instructions
            .iter()
            .find_map(|instr| match &instr.kind {
                LoweredInstrKind::ConstructLinear { kind, elements, .. } => {
                    Some((*kind, elements.len()))
                }
                _ => None,
            });

        assert_eq!(
            construct,
            Some((crate::control::LoweredLinearKind::Sequence, 2)),
            "typed sequence literals should lower as sequence instructions in {routine_name}",
        );
    }
}

#[test]
fn lowering_repro_lowers_non_empty_set_and_map_literals_in_typed_v1_contexts() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] set_return(): set[int, str] = {\n",
        "    return {1, \"two\"}\n",
        "};\n",
        "fun[] map_return(): map[str, int] = {\n",
        "    return {{\"US\", 45}, {\"DE\", 82}}\n",
        "};\n",
        "fun[] from_set_index(): str = {\n",
        "    var parts: set[int, str] = {1, \"two\"};\n",
        "    return parts[1];\n",
        "};\n",
        "fun[] from_map_index(): int = {\n",
        "    var counts: map[str, int] = {{\"US\", 45}, {\"DE\", 82}};\n",
        "    return counts[\"DE\"];\n",
        "};\n",
    ));

    let expected = [
        ("set_return", "set", 2usize),
        ("map_return", "map", 2usize),
        ("from_set_index", "set", 2usize),
        ("from_map_index", "map", 2usize),
    ];
    for (routine_name, aggregate_kind, expected_len) in expected {
        let routine = lowered
            .entry_package()
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .expect("aggregate lowering routine should exist");
        let lowered_members =
            routine
                .instructions
                .iter()
                .find_map(|instr| match (&instr.kind, aggregate_kind) {
                    (LoweredInstrKind::ConstructSet { members, .. }, "set") => Some(members.len()),
                    (LoweredInstrKind::ConstructMap { entries, .. }, "map") => Some(entries.len()),
                    _ => None,
                });

        assert_eq!(
            lowered_members,
            Some(expected_len),
            "typed {aggregate_kind} literals should lower in {routine_name}",
        );
    }
}

#[test]
fn lowering_repro_keeps_exact_typed_container_instruction_shapes() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] seq_return(): seq[str] = {\n",
        "    return {\"Ada\", \"Lin\"}\n",
        "};\n",
        "fun[] map_return(): map[str, int] = {\n",
        "    return {{\"US\", 45}, {\"DE\", 82}}\n",
        "};\n",
    ));

    let seq_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "seq_return")
        .expect("sequence return routine should exist");
    assert_eq!(seq_routine.instructions.len(), 3);
    assert!(matches!(
        seq_routine.instructions.get(crate::LoweredInstrId(0)).map(|instr| &instr.kind),
        Some(LoweredInstrKind::Const(LoweredOperand::Str(value))) if value == "Ada"
    ));
    assert!(matches!(
        seq_routine.instructions.get(crate::LoweredInstrId(1)).map(|instr| &instr.kind),
        Some(LoweredInstrKind::Const(LoweredOperand::Str(value))) if value == "Lin"
    ));
    assert!(matches!(
        seq_routine.instructions.get(crate::LoweredInstrId(2)).map(|instr| &instr.kind),
        Some(LoweredInstrKind::ConstructLinear {
            kind: crate::control::LoweredLinearKind::Sequence,
            elements,
            ..
        }) if elements.len() == 2
    ));

    let map_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "map_return")
        .expect("map return routine should exist");
    assert_eq!(map_routine.instructions.len(), 5);
    assert!(matches!(
        map_routine.instructions.get(crate::LoweredInstrId(0)).map(|instr| &instr.kind),
        Some(LoweredInstrKind::Const(LoweredOperand::Str(value))) if value == "US"
    ));
    assert!(matches!(
        map_routine
            .instructions
            .get(crate::LoweredInstrId(1))
            .map(|instr| &instr.kind),
        Some(LoweredInstrKind::Const(LoweredOperand::Int(45)))
    ));
    assert!(matches!(
        map_routine.instructions.get(crate::LoweredInstrId(2)).map(|instr| &instr.kind),
        Some(LoweredInstrKind::Const(LoweredOperand::Str(value))) if value == "DE"
    ));
    assert!(matches!(
        map_routine
            .instructions
            .get(crate::LoweredInstrId(3))
            .map(|instr| &instr.kind),
        Some(LoweredInstrKind::Const(LoweredOperand::Int(82)))
    ));
    assert!(matches!(
        map_routine.instructions.get(crate::LoweredInstrId(4)).map(|instr| &instr.kind),
        Some(LoweredInstrKind::ConstructMap { entries, .. }) if entries.len() == 2
    ));
}

#[test]
fn lowering_repro_lowers_early_return_when_branches_as_statement_control_flow() {
    let lowered = lower_fixture_workspace(concat!(
        "var enabled: bol = true;\n",
        "var default_name: str = \"Ada\";\n",
        "var low_count: int = 1;\n",
        "var high_count: int = 7;\n",
        "typ NameTag: rec = {\n",
        "    label: str;\n",
        "    code: int\n",
        "};\n",
        "typ Audit: rec = {\n",
        "    active: bol;\n",
        "    marker: NameTag\n",
        "};\n",
        "typ User: rec = {\n",
        "    name: str;\n",
        "    count: int;\n",
        "    audit: Audit\n",
        "};\n",
        "fun[] build_tag(): NameTag = {\n",
        "    return { label = \"stable\", code = high_count }\n",
        "};\n",
        "fun[] build_user(): User = {\n",
        "    return {\n",
        "        name = default_name,\n",
        "        count = high_count,\n",
        "        audit = {\n",
        "            active = enabled,\n",
        "            marker = build_tag(),\n",
        "        },\n",
        "    }\n",
        "};\n",
        "fun[] choose_count(): int = {\n",
        "    when(enabled) {\n",
        "        case(true) { return high_count; }\n",
        "        * { return low_count; }\n",
        "    }\n",
        "};\n",
        "fun[] main(current: User): int = {\n",
        "    loop(enabled) {\n",
        "        break;\n",
        "    }\n",
        "    when(enabled) {\n",
        "        case(true) { return current.audit.marker.code }\n",
        "        * { return choose_count() }\n",
        "    }\n",
        "};\n",
    ));
    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main lowering routine should exist");
    let return_blocks = routine
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.terminator,
                Some(crate::LoweredTerminator::Return { .. })
            )
        })
        .count();

    assert_eq!(routine.body_result, None);
    assert_eq!(
        return_blocks, 2,
        "early-return when lowering should preserve both branch returns without synthesizing a join value",
    );
}

#[test]
fn lowering_repro_keeps_exact_cfg_shape_for_early_return_when_branches() {
    let lowered = lower_fixture_workspace(
        "fun[] main(flag: bol): int = {\n    when(flag) {\n        case(true) { return 1 }\n        * { return 2 }\n    }\n};\n",
    );
    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main lowering routine should exist");

    assert_eq!(routine.blocks.len(), 3);
    assert_eq!(routine.body_result, None);
    assert!(matches!(
        routine
            .blocks
            .get(crate::LoweredBlockId(0))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Branch {
            then_block: crate::LoweredBlockId(1),
            else_block: crate::LoweredBlockId(2),
            ..
        })
    ));
    assert!(matches!(
        routine
            .blocks
            .get(crate::LoweredBlockId(1))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Return { value: Some(_) })
    ));
    assert!(matches!(
        routine
            .blocks
            .get(crate::LoweredBlockId(2))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Return { value: Some(_) })
    ));
}

#[test]
fn comparison_intrinsic_lowering_emits_intrinsic_calls_with_canonical_ids() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] eq_main(): bol = {\n",
        "    return .eq(1, 1);\n",
        "};\n",
        "fun[] lt_main(): bol = {\n",
        "    return .lt(1, 2);\n",
        "};\n",
    ));

    let entry = lowered.entry_package();
    for (routine_name, intrinsic_name) in [("eq_main", "eq"), ("lt_main", "lt")] {
        let routine = entry
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .expect("comparison intrinsic lowering routine should exist");
        let intrinsic_id = fol_intrinsics::intrinsic_by_canonical_name(intrinsic_name)
            .expect("intrinsic should exist")
            .id;
        let lowered_call = routine
            .instructions
            .iter()
            .find_map(|instr| match &instr.kind {
                LoweredInstrKind::IntrinsicCall { intrinsic, args } => {
                    Some((*intrinsic, args.len()))
                }
                _ => None,
            });

        assert_eq!(
            lowered_call,
            Some((intrinsic_id, 2)),
            "routine '{routine_name}' should lower through the canonical intrinsic id",
        );
    }
}

#[test]
fn boolean_intrinsic_lowering_emits_intrinsic_calls_with_canonical_ids() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(flag: bol): bol = {\n",
        "    return .not(flag);\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("boolean intrinsic lowering routine should exist");
    let intrinsic_id = fol_intrinsics::intrinsic_by_canonical_name("not")
        .expect("not intrinsic should exist")
        .id;
    let lowered_call = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::IntrinsicCall { intrinsic, args } => Some((*intrinsic, args.len())),
            _ => None,
        });

    assert_eq!(
        lowered_call,
        Some((intrinsic_id, 1)),
        "boolean intrinsic lowering should use the canonical '.not' intrinsic id",
    );
}

#[test]
fn length_intrinsic_lowering_emits_dedicated_length_instructions() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(items: seq[ptr[int]]): int = {\n",
        "    return .len(items);\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("length intrinsic lowering routine should exist");
    let lowered_len = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::LengthOf { operand } => Some(*operand),
            _ => None,
        })
        .expect("length intrinsic lowering should use the dedicated LengthOf instruction");
    let transfer_load = routine.instructions.iter().any(|instr| {
        matches!(
            &instr.kind,
            LoweredInstrKind::LoadLocal { local } if *local == routine.params[0]
        )
    });

    assert_eq!(
        lowered_len, routine.params[0],
        "LengthOf should observe the original move-only container local",
    );
    assert!(
        !transfer_load,
        "length observation must not route a move-only container through LoadLocal",
    );
}

#[test]
fn diagnostic_intrinsic_lowering_emits_runtime_hooks_and_forwards_values() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(flag: bol): bol = {\n",
        "    return .echo(flag);\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("diagnostic intrinsic lowering routine should exist");
    let intrinsic_id = fol_intrinsics::intrinsic_by_canonical_name("echo")
        .expect("echo intrinsic should exist")
        .id;
    let (lowered_intrinsic, hook_args) = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::RuntimeHook { intrinsic, args } => Some((*intrinsic, args.clone())),
            _ => None,
        })
        .expect("diagnostic intrinsic lowering should emit a runtime hook");

    assert_eq!(
        lowered_intrinsic, intrinsic_id,
        "diagnostic intrinsic lowering should emit a runtime hook using the canonical '.echo' intrinsic id",
    );
    assert_eq!(
        hook_args.len(),
        1,
        "the '.echo' runtime hook should forward exactly one argument",
    );
    let loaded_param =
        routine
            .instructions
            .iter()
            .find_map(|instr| match (&instr.result, &instr.kind) {
                (Some(result), LoweredInstrKind::LoadLocal { local })
                    if *result == hook_args[0] =>
                {
                    Some(*local)
                }
                _ => None,
            });
    assert_eq!(
        loaded_param,
        Some(routine.params[0]),
        "the '.echo' hook argument should come from loading the flag parameter",
    );
    assert!(
        matches!(
            routine.blocks.get(routine.entry_block).and_then(|block| block.terminator.clone()),
            Some(LoweredTerminator::Return { value: Some(value) }) if value == hook_args[0]
        ),
        "'.echo' should forward its argument value into the return terminator",
    );
}

#[test]
fn parser_typecheck_and_lower_keep_same_canonical_intrinsic_identity() {
    use fol_parser::ast::CallSurface;

    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_intrinsic_identity_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        concat!("fun[] main(): bol = {\n", "    return .eq(1, 1);\n", "};\n",),
    )
    .expect("should write lowering intrinsic identity fixture");

    let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
        .expect("Should open lowering fixture");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("identity fixture should parse");
    let resolved =
        resolve_package_workspace(syntax.clone()).expect("identity fixture should resolve");
    let typed = Typechecker::new()
        .check_resolved_workspace(resolved)
        .expect("identity fixture should typecheck");
    let lowered = crate::LoweringSession::new(typed.clone())
        .lower_workspace()
        .expect("identity fixture should lower");

    let call_syntax_id = syntax
        .source_units
        .first()
        .and_then(|unit| unit.items.first())
        .and_then(|item| match &item.node {
            fol_parser::ast::AstNode::FunDecl { body, .. } => body.first(),
            _ => None,
        })
        .and_then(|node| match node {
            fol_parser::ast::AstNode::Return { value: Some(value) } => match value.as_ref() {
                fol_parser::ast::AstNode::FunctionCall {
                    syntax_id,
                    surface,
                    name,
                    ..
                } if *surface == CallSurface::DotIntrinsic && name == "eq" => *syntax_id,
                _ => None,
            },
            _ => None,
        })
        .expect("parsed intrinsic call should be retained with a syntax id");
    let canonical_intrinsic = fol_intrinsics::intrinsic_by_canonical_name("eq")
        .expect("eq intrinsic should exist")
        .id;

    assert_eq!(
        typed
            .entry_program()
            .typed_node(call_syntax_id)
            .and_then(|node| node.intrinsic_id),
        Some(canonical_intrinsic),
        "typecheck should record the canonical intrinsic id selected from parser surface calls",
    );

    let main_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("lowered main routine should exist");
    let lowered_intrinsic = main_routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::IntrinsicCall { intrinsic, .. } => Some(*intrinsic),
            _ => None,
        });

    assert_eq!(
        lowered_intrinsic,
        Some(canonical_intrinsic),
        "lowering should preserve the same canonical intrinsic id recorded by typecheck",
    );
}

#[test]
fn nil_literal_lowering_stays_deferred_to_shell_lowering() {
    let mut types = LoweredTypeTable::new();
    let int_type = types.intern_builtin(LoweredBuiltinType::Int);
    let mut routine =
        LoweredRoutine::new(crate::LoweredRoutineId(0), "main", crate::LoweredBlockId(0));
    let entry = routine.blocks.push(LoweredBlock {
        id: crate::LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: None,
    });
    routine.entry_block = entry;

    let error = RoutineCursor::new(&mut routine, entry)
        .lower_literal(&Literal::Nil, int_type)
        .expect_err("nil should stay out of the core literal slice");

    assert_eq!(error.kind(), LoweringErrorKind::Unsupported);
}

#[test]
fn identifier_lowering_loads_parameter_locals_and_top_level_globals() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_identifier_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "var count: int = 1;\nfun[] main(value: int): non = { };",
    )
    .expect("should write lowering identifier fixture");

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

    let package = lowered_workspace.entry_package();
    let mut routine = package
        .routine_decls
        .values()
        .next()
        .expect("routine shell should exist")
        .clone();
    let decl_index = WorkspaceDeclIndex::build(&lowered_workspace);
    let int_type = package
        .checked_type_map
        .get(&fol_typecheck::CheckedTypeId(0))
        .copied()
        .expect("int builtin should map into lowering types");

    let param_symbol = typed
        .entry_program()
        .resolved()
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| symbol.kind == SymbolKind::Parameter && symbol.name == "value")
        .map(|(symbol_id, _)| symbol_id)
        .expect("parameter symbol should exist");
    let global_symbol = package
        .global_decls
        .values()
        .find(|global| global.name == "count")
        .map(|global| global.symbol_id)
        .expect("global symbol should exist");

    let entry_block = routine.entry_block;
    let mut cursor = RoutineCursor::new(&mut routine, entry_block);
    let param_value = cursor
        .lower_identifier_reference(
            lowered_workspace.entry_identity(),
            &decl_index,
            typed
                .entry_program()
                .resolved()
                .symbol(param_symbol)
                .expect("parameter symbol should resolve"),
            int_type,
        )
        .expect("parameter references should lower to local loads");
    let global_value = cursor
        .lower_identifier_reference(
            lowered_workspace.entry_identity(),
            &decl_index,
            typed
                .entry_program()
                .resolved()
                .symbol(global_symbol)
                .expect("global symbol should resolve"),
            int_type,
        )
        .expect("global references should lower to global loads");

    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(0))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::LoadLocal {
            local: routine.local_symbols[&param_symbol],
        })
    );
    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(1))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::LoadGlobal {
            global: package.globals[0],
        })
    );
    assert_eq!(param_value.local_id.0, routine.locals.len() - 2);
    assert_eq!(global_value.local_id.0, routine.locals.len() - 1);
}

#[test]
fn declaration_index_tracks_globals_and_routines_by_owning_package() {
    let identity = PackageIdentity {
        source_kind: PackageSourceKind::Entry,
        canonical_root: "/workspace/app".to_string(),
        display_name: "app".to_string(),
    };
    let mut package = LoweredPackage::new(crate::LoweredPackageId(0), identity.clone());
    package.global_decls.insert(
        crate::LoweredGlobalId(0),
        LoweredGlobal {
            id: crate::LoweredGlobalId(0),
            symbol_id: fol_resolver::SymbolId(1),
            source_unit_id: SourceUnitId(0),
            name: "answer".to_string(),
            type_id: crate::LoweredTypeId(0),
            mutable: false,
        },
    );
    package.routine_decls.insert(
        crate::LoweredRoutineId(0),
        LoweredRoutine::new(crate::LoweredRoutineId(0), "main", crate::LoweredBlockId(0)),
    );
    package
        .routine_decls
        .get_mut(&crate::LoweredRoutineId(0))
        .expect("routine should exist")
        .symbol_id = Some(fol_resolver::SymbolId(2));
    let mut packages = BTreeMap::new();
    packages.insert(identity.clone(), package);
    let mut type_table = crate::LoweredTypeTable::new();
    let recoverable_abi = crate::LoweredRecoverableAbi::v1(
        type_table.intern_builtin(crate::LoweredBuiltinType::Bool),
    );
    let workspace = LoweredWorkspace::new(
        identity.clone(),
        packages,
        vec![crate::LoweredEntryCandidate {
            package_identity: identity.clone(),
            routine_id: crate::LoweredRoutineId(0),
            name: "main".to_string(),
        }],
        type_table,
        crate::LoweredSourceMap::new(),
        recoverable_abi,
    );

    let index = WorkspaceDeclIndex::build(&workspace);

    assert_eq!(
        index.global_id_for_symbol(&identity, fol_resolver::SymbolId(1)),
        Some(crate::LoweredGlobalId(0))
    );
    assert_eq!(
        index.routine_id_for_symbol(&identity, fol_resolver::SymbolId(2)),
        Some(crate::LoweredRoutineId(0))
    );
}

#[test]
fn declaration_index_keeps_colliding_routine_metadata_package_local() {
    let app_identity = PackageIdentity {
        source_kind: PackageSourceKind::Entry,
        canonical_root: "/workspace/app".to_string(),
        display_name: "app".to_string(),
    };
    let dep_identity = PackageIdentity {
        source_kind: PackageSourceKind::Local,
        canonical_root: "/workspace/dep".to_string(),
        display_name: "dep".to_string(),
    };
    let routine_id = crate::LoweredRoutineId(0);

    let mut app_routine = LoweredRoutine::new(routine_id, "app_task", crate::LoweredBlockId(0));
    let app_param = app_routine.locals.push(LoweredLocal {
        id: crate::LoweredLocalId(0),
        type_id: Some(crate::LoweredTypeId(10)),
        name: Some("app_value".to_string()),
    });
    app_routine.params.push(app_param);

    let mut dep_routine = LoweredRoutine::new(routine_id, "dep_task", crate::LoweredBlockId(0));
    dep_routine.receiver_type = Some(crate::LoweredTypeId(20));
    let dep_param = dep_routine.locals.push(LoweredLocal {
        id: crate::LoweredLocalId(0),
        type_id: Some(crate::LoweredTypeId(20)),
        name: Some("dep_receiver".to_string()),
    });
    dep_routine.params.push(dep_param);

    let mut app_package = LoweredPackage::new(crate::LoweredPackageId(0), app_identity.clone());
    app_package.routine_decls.insert(routine_id, app_routine);
    let mut dep_package = LoweredPackage::new(crate::LoweredPackageId(1), dep_identity.clone());
    dep_package.routine_decls.insert(routine_id, dep_routine);

    let mut packages = BTreeMap::new();
    packages.insert(app_identity.clone(), app_package);
    packages.insert(dep_identity.clone(), dep_package);
    let mut type_table = crate::LoweredTypeTable::new();
    let recoverable_abi = crate::LoweredRecoverableAbi::v1(
        type_table.intern_builtin(crate::LoweredBuiltinType::Bool),
    );
    let workspace = LoweredWorkspace::new(
        app_identity.clone(),
        packages,
        Vec::new(),
        type_table,
        crate::LoweredSourceMap::new(),
        recoverable_abi,
    );

    let index = WorkspaceDeclIndex::build(&workspace);

    assert_eq!(
        index.routine_param_types(&app_identity, routine_id),
        Some([crate::LoweredTypeId(10)].as_slice())
    );
    assert_eq!(
        index.routine_param_names(&app_identity, routine_id),
        Some(["app_value".to_string()].as_slice())
    );
    assert!(!index.routine_has_receiver(&app_identity, routine_id));
    assert_eq!(
        index.routine_param_types(&dep_identity, routine_id),
        Some([crate::LoweredTypeId(20)].as_slice())
    );
    assert_eq!(
        index.routine_param_names(&dep_identity, routine_id),
        Some(["dep_receiver".to_string()].as_slice())
    );
    assert!(index.routine_has_receiver(&dep_identity, routine_id));
}

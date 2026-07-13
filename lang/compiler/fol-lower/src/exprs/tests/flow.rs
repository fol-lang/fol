use super::lower_fixture_workspace;
use crate::{LoweredInstrKind, LoweredOperand, LoweredTerminator};
use fol_parser::ast::AstParser;
use fol_resolver::resolve_package_workspace;
use fol_stream::FileStream;
use fol_typecheck::Typechecker;

fn collect_echoed_ints(routine: &crate::LoweredRoutine) -> Vec<i64> {
    let echo_id = fol_intrinsics::intrinsic_by_canonical_name("echo")
        .expect("echo intrinsic should exist")
        .id;
    let mut local_ints = std::collections::BTreeMap::new();
    let mut echoed = Vec::new();

    for instr in &routine.instructions {
        match &instr.kind {
            LoweredInstrKind::Const(LoweredOperand::Int(value)) => {
                if let Some(result) = instr.result {
                    local_ints.insert(result, *value);
                }
            }
            LoweredInstrKind::StoreLocal { local, value } => {
                if let Some(known) = local_ints.get(value).copied() {
                    local_ints.insert(*local, known);
                }
            }
            LoweredInstrKind::LoadLocal { local } => {
                if let (Some(result), Some(known)) =
                    (instr.result, local_ints.get(local).copied())
                {
                    local_ints.insert(result, known);
                }
            }
            LoweredInstrKind::RuntimeHook { intrinsic, args } if *intrinsic == echo_id => {
                let value = args
                    .first()
                    .and_then(|local| local_ints.get(local))
                    .copied()
                    .expect("echo argument should come from an integer constant in this fixture");
                echoed.push(value);
            }
            _ => {}
        }
    }

    echoed
}

#[test]
fn expression_lowering_keeps_local_and_imported_value_call_parity() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tmp path")
        .as_nanos();
    let root = super::safe_temp_dir().join(format!("fol_lower_expr_parity_{stamp}"));
    let app_dir = root.join("app");
    let shared_dir = root.join("shared");
    fs::create_dir_all(&app_dir).expect("should create app dir");
    fs::create_dir_all(&shared_dir).expect("should create shared dir");
    fs::write(
        app_dir.join("main.fol"),
        "use shared: loc = {\"../shared\"};\nfun[] local_helper(): int = { return 1; };\nfun[] main(): int = {\n    local_helper();\n    return shared::twice(answer);\n};",
    )
    .expect("should write app entry");
    fs::write(
        shared_dir.join("lib.fol"),
        "var[exp] answer: int = 7;\nfun[exp] twice(value: int): int = { return value; };",
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
        .expect("expression lowering parity should succeed");

    let main_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let shared_package = lowered
        .packages()
        .find(|package| package.identity.display_name == "shared")
        .expect("shared package should exist");

    assert!(
        main_routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::LoadGlobal { global } if shared_package.global_decls.contains_key(&global))),
        "entry routine should lower imported value references into foreign global loads"
    );
    assert!(
        main_routine
            .instructions
            .iter()
            .filter(|instr| matches!(instr.kind, LoweredInstrKind::Call { .. }))
            .count()
            >= 2,
        "entry routine should keep both local and imported call sites as direct Call instructions"
    );
}

#[test]
fn return_lowering_emits_explicit_return_terminators_and_skips_trailing_body_nodes() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_return_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(&fixture, "fun[] main(): int = {\n    return 1;\n    2;\n};")
        .expect("should write lowering return fixture");

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
        .expect("return lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let entry_block = routine
        .blocks
        .get(routine.entry_block)
        .expect("entry block should exist");

    assert_eq!(entry_block.instructions.len(), 1);
    assert_eq!(
        entry_block.terminator,
        Some(LoweredTerminator::Return {
            value: Some(crate::LoweredLocalId(0)),
        })
    );
    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(0))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::Const(LoweredOperand::Int(1)))
    );
    assert!(
        routine.body_result.is_none(),
        "early returns should not leave a trailing body_result behind"
    );
}

#[test]
fn dfr_lowering_runs_registered_bodies_before_return_in_reverse_order() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(): int = {\n",
        "    dfr { var first: int = .echo(1); };\n",
        "    dfr { var second: int = .echo(2); };\n",
        "    return 7;\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let echoed = collect_echoed_ints(routine);

    assert_eq!(echoed, vec![2, 1], "dfr should lower in reverse order before return");
    assert!(
        matches!(
            routine
                .blocks
                .get(routine.entry_block)
                .and_then(|block| block.terminator.as_ref()),
            Some(LoweredTerminator::Return { value: Some(_) })
        ),
        "dfr-bearing routine should still end in an explicit return terminator"
    );
}

#[test]
fn dfr_lowering_keeps_local_bindings_inside_deferred_blocks() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(): int = {\n",
        "    dfr {\n",
        "        var done: int = 1;\n",
        "        var shown: int = .echo(done);\n",
        "    };\n",
        "    return 7;\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(
        collect_echoed_ints(routine),
        vec![1],
        "deferred blocks should lower using their own typed scope so local bindings remain available"
    );
}

#[test]
fn dfr_lowering_runs_inner_scope_cleanup_before_outer_scope_cleanup() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(): int = {\n",
        "    dfr { var outer: int = .echo(1); };\n",
        "    {\n",
        "        dfr { var inner: int = .echo(2); };\n",
        "        var body: int = .echo(3);\n",
        "    }\n",
        "    return .echo(7);\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(
        collect_echoed_ints(routine),
        vec![3, 2, 7, 1],
        "nested block cleanup should run when the inner scope exits, then outer cleanup should wait for the return path"
    );
}

#[test]
fn dfr_lowering_runs_loop_cleanup_before_break_and_outer_cleanup_before_return() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(): int = {\n",
        "    dfr { var outer: int = .echo(1); };\n",
        "    loop(true) {\n",
        "        dfr { var inner: int = .echo(2); };\n",
        "        var body: int = .echo(3);\n",
        "        break;\n",
        "    }\n",
        "    return .echo(7);\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(
        collect_echoed_ints(routine),
        vec![3, 2, 7, 1],
        "break should drain only loop-local deferred bodies, then outer deferred bodies should remain active until the return path"
    );
}

#[test]
fn dfr_lowering_runs_cleanup_before_report_terminators() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(): int / str = {\n",
        "    dfr { var cleanup: int = .echo(1); };\n",
        "    report \"bad\";\n",
        "    return 7;\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let entry_block = routine
        .blocks
        .get(routine.entry_block)
        .expect("entry block should exist");

    assert_eq!(
        collect_echoed_ints(routine),
        vec![1],
        "report paths should drain deferred bodies before terminating"
    );
    assert!(
        matches!(entry_block.terminator, Some(LoweredTerminator::Report { .. })),
        "dfr-bearing report routine should still terminate with an explicit Report"
    );
}

#[test]
fn dfr_lowering_runs_cleanup_before_panic_terminators() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(): int = {\n",
        "    dfr { var cleanup: int = .echo(1); };\n",
        "    panic \"boom\";\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let entry_block = routine
        .blocks
        .get(routine.entry_block)
        .expect("entry block should exist");

    assert_eq!(
        collect_echoed_ints(routine),
        vec![1],
        "panic paths should drain deferred bodies before terminating"
    );
    assert!(
        matches!(entry_block.terminator, Some(LoweredTerminator::Panic { .. })),
        "dfr-bearing panic routine should still terminate with an explicit Panic"
    );
}

#[test]
fn report_lowering_emits_explicit_report_terminators_and_skips_trailing_body_nodes() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_report_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] main(flag: bol): int / bol = {\n    report flag;\n    return 1;\n};",
    )
    .expect("should write lowering report fixture");

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
        .expect("report lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let entry_block = routine
        .blocks
        .get(routine.entry_block)
        .expect("entry block should exist");

    assert_eq!(entry_block.instructions.len(), 1);
    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(0))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::LoadLocal {
            local: routine.params[0],
        })
    );
    assert_eq!(
        entry_block.terminator,
        Some(LoweredTerminator::Report {
            value: Some(crate::LoweredLocalId(1)),
        })
    );
    assert!(
        routine.body_result.is_none(),
        "early reports should not leave a trailing body_result behind"
    );
}

#[test]
fn when_statement_lowering_emits_branch_blocks_and_falls_through_afterward() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_when_stmt_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] main(flag: bol): int = {\n    when(flag) {\n        case(true) { 1 }\n        * { 2 }\n    }\n    return 2;\n};",
    )
    .expect("should write lowering when fixture");

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
        .expect("statement-style when lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert!(
        routine
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Some(LoweredTerminator::Branch { .. }))),
        "statement-style when should lower into an explicit branch terminator"
    );
    assert!(
        routine
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Some(LoweredTerminator::Jump { .. }))),
        "when bodies should jump into a shared continuation block"
    );
    assert!(
        routine
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Some(LoweredTerminator::Return { .. }))),
        "control should fall through after the when into the trailing return"
    );
}

#[test]
fn when_expression_lowering_stores_branch_values_into_one_join_local() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_when_expr_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "var yes: int = 1;\nvar no: int = 2;\nfun[] main(flag: bol): non = {\n    when(flag) {\n        case(true) { yes }\n        * { no }\n    }\n};",
    )
    .expect("should write lowering when-expression fixture");

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
        .expect("value-producing when lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    let mut stored_join_locals = routine
        .instructions
        .iter()
        .filter_map(|instr| match instr.kind {
            LoweredInstrKind::StoreLocal { local, .. } => Some(local),
            _ => None,
        })
        .collect::<Vec<_>>();
    stored_join_locals.sort_by_key(|local| local.0);
    stored_join_locals.dedup_by_key(|local| local.0);

    assert_eq!(stored_join_locals.len(), 1);
    assert_eq!(routine.body_result, Some(stored_join_locals[0]));
    assert!(
        routine
            .blocks
            .iter()
            .any(|block| matches!(block.terminator, Some(LoweredTerminator::Branch { .. }))),
        "value-producing when should branch explicitly"
    );
    assert!(
        routine
            .blocks
            .iter()
            .filter(|block| matches!(block.terminator, Some(LoweredTerminator::Jump { .. })))
            .count()
            >= 2,
        "value-producing when branches should jump into a shared join block"
    );
}

#[test]
fn when_statement_lowering_keeps_a_four_block_shape_for_case_default_fallthrough() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_when_stmt_shape_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] main(flag: bol): int = {\n    when(flag) {\n        case(true) { 1 }\n        * { 2 }\n    }\n    return 2;\n};",
    )
    .expect("should write lowering when shape fixture");

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
        .expect("statement-style when lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(routine.blocks.len(), 4);
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(0))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Branch {
            condition: crate::LoweredLocalId(3),
            then_block: crate::LoweredBlockId(2),
            else_block: crate::LoweredBlockId(3),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(2))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(1),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(3))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(1),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(1))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Return {
            value: Some(crate::LoweredLocalId(7)),
        })
    );
}

#[test]
fn when_expression_lowering_keeps_branch_default_and_join_block_shape() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_when_expr_shape_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "var yes: int = 1;\nvar no: int = 2;\nfun[] main(flag: bol): non = {\n    when(flag) {\n        case(true) { yes }\n        * { no }\n    }\n};",
    )
    .expect("should write lowering when-expression shape fixture");

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
        .expect("value-producing when lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(routine.blocks.len(), 4);
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(0))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Branch {
            condition: crate::LoweredLocalId(3),
            then_block: crate::LoweredBlockId(2),
            else_block: crate::LoweredBlockId(3),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(2))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(1),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(3))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(1),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(1))
            .and_then(|block| block.terminator.clone()),
        None
    );
    assert_eq!(routine.body_result, Some(crate::LoweredLocalId(5)));
}

#[test]
fn loop_condition_lowering_keeps_header_body_and_exit_blocks() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_loop_shape_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] main(flag: bol, limit: int): int = {\n    loop(flag) {\n        var current: int = limit;\n    }\n    return limit;\n};",
    )
    .expect("should write lowering loop shape fixture");

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
        .expect("condition loop lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(routine.blocks.len(), 4);
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(0))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(1),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(1))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Branch {
            condition: crate::LoweredLocalId(2),
            then_block: crate::LoweredBlockId(2),
            else_block: crate::LoweredBlockId(3),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(2))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(1),
        })
    );
    assert!(matches!(
        routine
            .blocks
            .get(crate::LoweredBlockId(3))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Return { .. })
    ));
}

#[test]
fn break_lowering_jumps_directly_to_the_loop_exit_block() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_break_shape_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] main(flag: bol, limit: int): int = {\n    loop(flag) {\n        break;\n    }\n    return limit;\n};",
    )
    .expect("should write lowering break shape fixture");

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
        .expect("break lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(routine.blocks.len(), 4);
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(0))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(1),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(1))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Branch {
            condition: crate::LoweredLocalId(2),
            then_block: crate::LoweredBlockId(2),
            else_block: crate::LoweredBlockId(3),
        })
    );
    assert_eq!(
        routine
            .blocks
            .get(crate::LoweredBlockId(2))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Jump {
            target: crate::LoweredBlockId(3),
        })
    );
    assert!(matches!(
        routine
            .blocks
            .get(crate::LoweredBlockId(3))
            .and_then(|block| block.terminator.clone()),
        Some(LoweredTerminator::Return { .. })
    ));
}

#[test]
fn iteration_loop_lowering_produces_index_driven_control_flow() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_iter_loop_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] sum(items: seq[int]): int = {\n    var total: int = 0;\n    loop(item in items) {\n        total = total + item;\n    }\n    return total;\n};",
    )
    .expect("should write iteration loop fixture");

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
        .expect("iteration loop lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "sum")
        .expect("sum routine should exist");

    assert!(
        routine.blocks.len() >= 4,
        "iteration loop should produce at least 4 blocks, got {}",
        routine.blocks.len()
    );

    assert!(
        routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::IndexAccess { .. })),
        "iteration loop should use IndexAccess to extract elements"
    );

    assert!(
        routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::LengthOf { .. })),
        "iteration loop should use LengthOf to get container length"
    );
}

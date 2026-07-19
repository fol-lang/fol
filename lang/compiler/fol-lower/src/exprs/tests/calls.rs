use super::lower_fixture_workspace;
use crate::{LoweredInstrKind, LoweredOperand, LoweredTerminator};
use fol_parser::ast::AstParser;
use fol_resolver::resolve_package_workspace;
use fol_stream::FileStream;
use fol_typecheck::Typechecker;

fn int_constants_for_args(
    routine: &crate::LoweredRoutine,
    args: &[crate::LoweredLocalId],
) -> Vec<i64> {
    args.iter()
        .map(|local_id| {
            routine
                .instructions
                .iter()
                .find_map(|instr| match (&instr.result, &instr.kind) {
                    (Some(result), LoweredInstrKind::Const(LoweredOperand::Int(value)))
                        if result == local_id =>
                    {
                        Some(*value)
                    }
                    _ => None,
                })
                .expect("call args should lower from integer constants in this fixture")
        })
        .collect()
}

#[test]
fn routine_body_lowering_keeps_local_initializers_and_final_expression_results() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_body_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] main(): non = {\n    var value: int = 1;\n    value;\n};",
    )
    .expect("should write lowering body fixture");

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
        .expect("body lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .next()
        .expect("lowered routine should exist");
    let entry_block = routine
        .blocks
        .get(routine.entry_block)
        .expect("entry block should exist");

    assert_eq!(entry_block.instructions.len(), 3);
    assert_eq!(
        routine
            .instructions
            .get(crate::LoweredInstrId(0))
            .map(|instr| &instr.kind),
        Some(&LoweredInstrKind::Const(LoweredOperand::Int(1)))
    );
    assert!(
        matches!(
            routine
                .instructions
                .get(crate::LoweredInstrId(1))
                .map(|instr| &instr.kind),
            Some(LoweredInstrKind::StoreLocal { .. })
        ),
        "local binding initializer should lower into a store"
    );
    assert!(
        matches!(
            routine
                .instructions
                .get(crate::LoweredInstrId(2))
                .map(|instr| &instr.kind),
            Some(LoweredInstrKind::LoadLocal { .. })
        ),
        "final body expression should lower into a local load"
    );
    assert!(routine.body_result.is_some());
}

#[test]
fn assignment_lowering_emits_local_and_global_store_instructions() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_assignment_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "var count: int = 0;\nfun[] main(): int = {\n    var value: int = 1;\n    value = 2;\n    count = value;\n    return [mov]value;\n};",
    )
    .expect("should write lowering assignment fixture");

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
        .expect("assignment lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .next()
        .expect("lowered routine should exist");

    assert!(
        routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::StoreLocal { .. })),
        "assignment to local bindings should lower into StoreLocal"
    );
    assert!(
        routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::StoreGlobal { .. })),
        "assignment to globals should lower into StoreGlobal"
    );
}

#[test]
fn owned_bindings_drop_at_lexical_exit_after_defers_and_moves() {
    let lowered = lower_fixture_workspace(
        "typ Item: rec = { value: int };\n\
         fun[] main(): int = {\n\
             {\n\
                 @var retained: Item = { value = 1 };\n\
                 dfr[retained[bor]] { var seen: int = retained.value; };\n\
             };\n\
             {\n\
                 @var moved: Item = { value = 2 };\n\
                 @var receiver: Item = [mov]moved;\n\
             };\n\
             return 0;\n\
         };",
    );
    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should lower");

    let dropped_names = routine
        .instructions
        .iter()
        .filter_map(|instruction| match instruction.kind {
            LoweredInstrKind::DropLocal { local } => routine
                .locals
                .get(local)
                .and_then(|local| local.name.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(dropped_names, vec!["retained", "receiver", "moved"]);

    let retained = routine
        .locals
        .iter_with_ids()
        .find_map(|(local_id, local)| {
            (local.name.as_deref() == Some("retained")).then_some(local_id)
        })
        .expect("retained local should exist");
    let deferred_read_index = routine
        .instructions
        .iter_with_ids()
        .find_map(|(instruction_id, instruction)| match instruction.kind {
            LoweredInstrKind::FieldAccess { base, .. } if base == retained => {
                Some(instruction_id.0)
            }
            _ => None,
        })
        .expect("deferred body should read retained before its drop");
    let retained_drop_index = routine
        .instructions
        .iter_with_ids()
        .find_map(|(instruction_id, instruction)| match instruction.kind {
            LoweredInstrKind::DropLocal { local } if local == retained => Some(instruction_id.0),
            _ => None,
        })
        .expect("retained should have a lexical drop");
    assert!(deferred_read_index < retained_drop_index);
}

#[test]
fn maybe_moved_bindings_still_drop_reinitialized_branch_values() {
    let lowered = lower_fixture_workspace(
        "fun[] consume(pointer: ptr[int]): int = { return [drf]pointer; };\n\
         fun[] main(): int = {\n\
             var choose: bol = true;\n\
             {\n\
                 var first: int = 1;\n\
                 var second: int = 2;\n\
                 var[mut] pointer: ptr[int] = [ref]first;\n\
                 when(choose) {\n\
                     case(true) { var consumed: int = consume([mov]pointer); }\n\
                     * { pointer = [ref]second; }\n\
                 }\n\
             };\n\
             return 0;\n\
         };",
    );
    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should lower");

    let pointer = routine
        .locals
        .iter_with_ids()
        .find_map(|(local_id, local)| {
            (local.name.as_deref() == Some("pointer")).then_some(local_id)
        })
        .expect("pointer local should exist");
    assert!(routine.instructions.iter().any(|instruction| {
        matches!(
            instruction.kind,
            LoweredInstrKind::DropLocal { local } if local == pointer
        )
    }));
}

#[test]
fn aggregates_and_moved_sources_drop_at_lexical_exit() {
    let lowered = lower_fixture_workspace(
        "typ Holder: rec = { pointer: ptr[int] };\n\
         fun[] main(): int = {\n\
             {\n\
                 var seed: int = 1;\n\
                 var pointer: ptr[int] = [ref]seed;\n\
                 var holder: Holder = { pointer = [mov]pointer };\n\
             };\n\
             return 0;\n\
         };",
    );
    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should lower");

    let dropped_names = routine
        .instructions
        .iter()
        .filter_map(|instruction| match instruction.kind {
            LoweredInstrKind::DropLocal { local } => routine
                .locals
                .get(local)
                .and_then(|local| local.name.as_deref()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(dropped_names, vec!["holder", "pointer"]);
}

#[test]
fn returned_owned_and_shelled_locals_drop_only_source_slots() {
    let lowered = lower_fixture_workspace(
        "typ Item: rec = { value: int };\n\
         fun[] take_owned(): @Item = {\n\
             @var value: Item = { value = 1 };\n\
             return [mov]value;\n\
         };\n\
         fun[] take_optional(): opt @Item = {\n\
             @var value: Item = { value = 2 };\n\
             var wrapped: opt @Item = [mov]value;\n\
             return [mov]wrapped;\n\
         };\n\
         fun[] take_error(): err[@Item] = {\n\
             @var value: Item = { value = 3 };\n\
             var wrapped: err[@Item] = [mov]value;\n\
             return [mov]wrapped;\n\
         };\n\
         fun[] main(): int = { return 0; };",
    );

    for (routine_name, expected_drops) in [
        ("take_owned", vec!["value"]),
        ("take_optional", vec!["wrapped", "value"]),
        ("take_error", vec!["wrapped", "value"]),
    ] {
        let routine = lowered
            .entry_package()
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .unwrap_or_else(|| panic!("{routine_name} routine should lower"));
        let dropped = routine
            .instructions
            .iter()
            .filter_map(|instruction| match instruction.kind {
                LoweredInstrKind::DropLocal { local } => Some(local),
                _ => None,
            })
            .collect::<Vec<_>>();
        let dropped_names = dropped
            .iter()
            .filter_map(|local| {
                routine
                    .locals
                    .get(*local)
                    .and_then(|local| local.name.as_deref())
            })
            .collect::<Vec<_>>();
        assert_eq!(dropped_names, expected_drops);

        let returned = routine
            .blocks
            .iter()
            .find_map(|block| match block.terminator {
                Some(crate::LoweredTerminator::Return { value: Some(value) }) => Some(value),
                _ => None,
            })
            .expect("routine should return a lowered value");
        assert!(
            !dropped.contains(&returned),
            "{routine_name} must return the transfer temporary, not a dropped source slot"
        );
    }
}

#[test]
fn call_lowering_emits_direct_callee_calls_for_plain_and_qualified_forms() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tmp path")
        .as_nanos();
    let root = super::safe_temp_dir().join(format!("fol_lower_call_exprs_{stamp}"));
    let app_dir = root.join("app");
    let math_dir = app_dir.join("math");
    fs::create_dir_all(&math_dir).expect("should create nested namespace dir");
    fs::write(
        app_dir.join("main.fol"),
        "fun[] helper(): int = { return 1; };\nfun[] main(): int = {\n    helper();\n    return math::triple();\n};",
    )
    .expect("should write entry file");
    fs::write(
        math_dir.join("lib.fol"),
        "fun[exp] triple(): int = { return 3; };\n",
    )
    .expect("should write nested namespace file");

    let mut stream = FileStream::from_folder(app_dir.to_str().expect("utf8 temp path"))
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
        .expect("call lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_instrs = routine
        .instructions
        .iter()
        .filter(|instr| matches!(instr.kind, LoweredInstrKind::Call { .. }))
        .collect::<Vec<_>>();

    assert_eq!(call_instrs.len(), 2);
}

#[test]
fn method_call_lowering_rewrites_receivers_into_direct_call_arguments() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_method_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun (int)double(): int = { return 2; };\nfun[] main(): int = {\n    var value: int = 1;\n    return value.double();\n};",
    )
    .expect("should write lowering method fixture");

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
        .expect("method call lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { callee, args, .. } => Some((*callee, args.clone())),
            _ => None,
        })
        .expect("method body should contain a lowered call");

    assert_eq!(call.1.len(), 1);
}

#[test]
fn method_call_lowering_reorders_named_arguments_after_the_receiver() {
    let workspace = lower_fixture_workspace(
        "typ Counter: rec = { value: int };\n\
         fun (Counter)shift(by: int, step: int): int = {\n\
             return by;\n\
         };\n\
         fun[] main(current: Counter): int = {\n\
             return current.shift(step = 2, by = 1);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let (call_result, call_args) = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some((instr.result, args.clone())),
            _ => None,
        })
        .expect("method body should contain a lowered call");

    assert!(
        call_result.is_some(),
        "expression-style method call should keep a result local"
    );
    assert_eq!(
        call_args.len(),
        3,
        "method call should lower receiver plus two explicit args"
    );
    let lowered_arg_constants = int_constants_for_args(routine, &call_args[1..]);

    assert_eq!(
        lowered_arg_constants,
        vec![1, 2],
        "named method arguments should lower in declared parameter order after the receiver"
    );
}

#[test]
fn free_call_lowering_reorders_named_arguments_in_declared_parameter_order() {
    let workspace = lower_fixture_workspace(
        "fun[] pair(left: int, right: int): int = {\n\
             return left;\n\
         };\n\
         fun[] main(): int = {\n\
             return pair(right = 2, left = 1);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let (call_result, call_args) = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some((instr.result, args.clone())),
            _ => None,
        })
        .expect("method body should contain a lowered call");

    assert!(
        call_result.is_some(),
        "expression-style free call should keep a result local"
    );
    assert_eq!(
        call_args.len(),
        2,
        "free named call should lower both declared params"
    );
    assert_eq!(
        int_constants_for_args(routine, &call_args),
        vec![1, 2],
        "named free-call arguments should lower in declared parameter order"
    );
}

#[test]
fn free_call_lowering_synthesizes_default_arguments() {
    let workspace = lower_fixture_workspace(
        "fun[] pair(left: int, right: int = 2): int = {\n\
             return left;\n\
         };\n\
         fun[] main(): int = {\n\
             return pair(1);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered free call");

    assert_eq!(
        call_args.len(),
        2,
        "defaulted free call should lower a full argument list"
    );

    let lowered_arg_constants = int_constants_for_args(routine, &call_args);

    assert_eq!(
        lowered_arg_constants,
        vec![1, 2],
        "omitted default parameters should lower in declared order"
    );
}

#[test]
fn method_call_lowering_synthesizes_default_arguments_after_the_receiver() {
    let workspace = lower_fixture_workspace(
        "typ Counter: rec = { value: int };\n\
         fun (Counter)shift(by: int, step: int = 2): int = {\n\
             return by;\n\
         };\n\
         fun[] main(current: Counter): int = {\n\
             return current.shift(1);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered method call");

    assert_eq!(
        call_args.len(),
        3,
        "method default call should lower receiver plus two explicit args"
    );

    let lowered_arg_constants = int_constants_for_args(routine, &call_args[1..]);

    assert_eq!(
        lowered_arg_constants,
        vec![1, 2],
        "omitted method defaults should lower after the receiver in declared parameter order"
    );
}

#[test]
fn free_call_lowering_packs_variadic_arguments_into_a_sequence() {
    let workspace = lower_fixture_workspace(
        "fun[] sum(head: int, tail: ... int): int = {\n\
             return head;\n\
         };\n\
         fun[] main(): int = {\n\
             return sum(1, 2, 3, 4);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered variadic free call");

    assert_eq!(
        call_args.len(),
        2,
        "variadic free call should lower fixed args plus one packed sequence"
    );

    let packed_sequence = routine
        .instructions
        .iter()
        .find_map(|instr| match (&instr.result, &instr.kind) {
            (Some(result), LoweredInstrKind::ConstructLinear { kind, elements, .. })
                if *result == call_args[1] =>
            {
                Some((*kind, elements.len()))
            }
            _ => None,
        })
        .expect("variadic trailing args should lower into a sequence construction");

    assert_eq!(packed_sequence, (crate::LoweredLinearKind::Sequence, 3));
}

#[test]
fn free_call_lowering_passes_unpack_sequences_without_repacking() {
    let workspace = lower_fixture_workspace(
        "fun[] sum(head: int, tail: ... int): int = {\n\
             return head;\n\
         };\n\
         fun[] main(values: seq[int]): int = {\n\
             return sum(1, ...values);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered variadic unpack free call");

    assert_eq!(
        call_args.len(),
        2,
        "unpacked free call should lower fixed args plus one sequence arg"
    );
    assert!(
        routine
            .instructions
            .iter()
            .all(|instr| match (&instr.result, &instr.kind) {
                (Some(result), LoweredInstrKind::ConstructLinear { .. }) => *result != call_args[1],
                _ => true,
            }),
        "free-call unpack should pass the existing sequence through without repacking it"
    );
}

#[test]
fn free_call_lowering_passes_named_unpack_sequences_without_repacking() {
    let workspace = lower_fixture_workspace(
        "fun[] score(base: int, step: int = 2, tail: ... int): int = {\n\
             return base;\n\
         };\n\
         fun[] main(values: seq[int]): int = {\n\
             return score(base = 1, ...values);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered named variadic unpack free call");

    assert_eq!(
        call_args.len(),
        3,
        "named unpack free call should lower explicit args plus one sequence arg"
    );
    assert!(
        routine
            .instructions
            .iter()
            .all(|instr| match (&instr.result, &instr.kind) {
                (Some(result), LoweredInstrKind::ConstructLinear { .. }) => *result != call_args[2],
                _ => true,
            }),
        "named free-call unpack should pass the existing sequence through without repacking it"
    );
}

#[test]
fn method_call_lowering_packs_variadic_arguments_after_the_receiver() {
    let workspace = lower_fixture_workspace(
        "typ Counter: rec = { value: int };\n\
         fun (Counter)shift(values: ... int): int = {\n\
             return 0;\n\
         };\n\
         fun[] main(current: Counter): int = {\n\
             return current.shift(1, 2, 3);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered variadic method call");

    assert_eq!(
        call_args.len(),
        2,
        "variadic method call should lower receiver plus one packed sequence"
    );

    let packed_sequence = routine
        .instructions
        .iter()
        .find_map(|instr| match (&instr.result, &instr.kind) {
            (Some(result), LoweredInstrKind::ConstructLinear { kind, elements, .. })
                if *result == call_args[1] =>
            {
                Some((*kind, elements.len()))
            }
            _ => None,
        })
        .expect("variadic method args should lower into a sequence construction");

    assert_eq!(packed_sequence, (crate::LoweredLinearKind::Sequence, 3));
}

#[test]
fn method_call_lowering_passes_named_unpack_sequences_after_the_receiver() {
    let workspace = lower_fixture_workspace(
        "typ Counter: rec = { value: int };\n\
         fun (Counter)shift(step: int = 2, values: ... int): int = {\n\
             return 0;\n\
         };\n\
         fun[] main(current: Counter, values: seq[int]): int = {\n\
             return current.shift(step = 3, ...values);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered named variadic unpack method call");

    assert_eq!(
        call_args.len(),
        3,
        "named unpack method call should lower receiver, explicit args, and one sequence arg"
    );
    assert!(
        routine
            .instructions
            .iter()
            .all(|instr| match (&instr.result, &instr.kind) {
                (Some(result), LoweredInstrKind::ConstructLinear { .. }) => *result != call_args[2],
                _ => true,
            }),
        "named method unpack should pass the existing sequence through without repacking it"
    );
}

#[test]
fn method_call_lowering_passes_unpack_sequences_after_the_receiver() {
    let workspace = lower_fixture_workspace(
        "typ Counter: rec = { value: int };\n\
         fun (Counter)shift(values: ... int): int = {\n\
             return 0;\n\
         };\n\
         fun[] main(current: Counter, values: seq[int]): int = {\n\
             return current.shift(...values);\n\
         };",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_args = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { args, .. } => Some(args.clone()),
            _ => None,
        })
        .expect("main routine should contain a lowered variadic unpack method call");

    assert_eq!(
        call_args.len(),
        2,
        "unpacked method call should lower receiver plus one sequence arg"
    );
    assert!(
        routine
            .instructions
            .iter()
            .all(|instr| match (&instr.result, &instr.kind) {
                (Some(result), LoweredInstrKind::ConstructLinear { .. }) => *result != call_args[1],
                _ => true,
            }),
        "method unpack should pass the existing sequence through without repacking it"
    );
}

#[test]
fn errorful_call_lowering_retains_explicit_error_type_metadata() {
    let lowered = lower_fixture_workspace(
        "fun[] load(): int / str = {\n\
             report \"bad\";\n\
             return 1;\n\
         };\n\
         fun[] main(): int / str = {\n\
             return load() || report \"forwarded\";\n\
         };\n",
    );

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let call_error_type = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::Call { error_type, .. } => *error_type,
            _ => None,
        })
        .expect("errorful call should retain an explicit lowered error type");

    assert_eq!(
        lowered.type_table().get(call_error_type),
        Some(&crate::LoweredType::Builtin(crate::LoweredBuiltinType::Str))
    );
    let signature = routine
        .signature
        .and_then(|signature| lowered.type_table().get(signature))
        .expect("main routine should retain a lowered signature");
    match signature {
        crate::LoweredType::Routine(signature) => {
            assert_eq!(
                signature
                    .error_type
                    .and_then(|error_type| lowered.type_table().get(error_type)),
                Some(&crate::LoweredType::Builtin(crate::LoweredBuiltinType::Str))
            );
        }
        other => panic!("expected lowered routine signature, got {other:?}"),
    }
}

#[test]
fn explicit_report_fallback_lowering_branches_and_reports_recoverable_calls() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] load(flag: bol): int / str = {\n",
        "    when(flag) {\n",
        "        case(true) { report \"bad\" }\n",
        "        * { return 7 }\n",
        "    }\n",
        "};\n",
        "fun[] main(flag: bol): int / str = {\n",
        "    return load(flag) || report \"forwarded\";\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert!(routine.instructions.iter().any(|instr| matches!(
        instr.kind,
        LoweredInstrKind::Call {
            error_type: Some(_),
            ..
        }
    )));
    assert!(routine
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, LoweredInstrKind::CheckRecoverable { .. })));
    assert!(!routine
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, LoweredInstrKind::ExtractRecoverableError { .. })));
    assert!(routine
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(LoweredTerminator::Branch { .. }))));
    assert!(routine
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(LoweredTerminator::Report { .. }))));
}

#[test]
fn check_lowering_observes_recoverable_bindings_without_propagation() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] load(flag: bol): int / str = {\n",
        "    when(flag) {\n",
        "        case(true) { report \"bad\" }\n",
        "        * { return 7 }\n",
        "    }\n",
        "};\n",
        "fun[] main(flag: bol): bol = {\n",
        "    return check(load(flag));\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    assert!(routine
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, LoweredInstrKind::CheckRecoverable { .. })));
    assert!(!routine
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(LoweredTerminator::Report { .. }))));
}

#[test]
fn pipe_or_default_lowering_branches_to_a_plain_fallback_value() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] load(flag: bol): int / str = {\n",
        "    when(flag) {\n",
        "        case(true) { report \"bad\" }\n",
        "        * { return 7 }\n",
        "    }\n",
        "};\n",
        "fun[] main(flag: bol): int = {\n",
        "    return load(flag) || 5;\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert!(routine
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, LoweredInstrKind::CheckRecoverable { .. })));
    assert!(routine
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, LoweredInstrKind::UnwrapRecoverable { .. })));
    assert!(!routine
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, LoweredInstrKind::ExtractRecoverableError { .. })));
    assert!(routine
        .instructions
        .iter()
        .any(|instr| matches!(instr.kind, LoweredInstrKind::Const(LoweredOperand::Int(5)))));
    assert!(!routine.blocks.iter().any(|block| matches!(
        block.terminator,
        Some(LoweredTerminator::Report { .. } | LoweredTerminator::Panic { .. })
    )));
}

#[test]
fn pipe_or_report_lowering_uses_error_branch_reports() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] load(flag: bol): int / str = {\n",
        "    when(flag) {\n",
        "        case(true) { report \"bad\" }\n",
        "        * { return 7 }\n",
        "    }\n",
        "};\n",
        "fun[] main(flag: bol): int / str = {\n",
        "    return load(flag) || report \"fallback\";\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert!(routine
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(LoweredTerminator::Report { .. }))));
}

#[test]
fn pipe_or_panic_lowering_uses_error_branch_panics() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] load(flag: bol): int / str = {\n",
        "    when(flag) {\n",
        "        case(true) { report \"bad\" }\n",
        "        * { return 7 }\n",
        "    }\n",
        "};\n",
        "fun[] main(flag: bol): int = {\n",
        "    return load(flag) || panic \"fallback\";\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert!(routine
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(LoweredTerminator::Panic { .. }))));
}

#[test]
fn standalone_panic_lowering_uses_keyword_intrinsic_terminators() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] main(): int = {\n",
        "    panic \"boom\";\n",
        "};\n",
    ));

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert!(routine
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(LoweredTerminator::Panic { .. }))));
}

#[test]
fn field_access_lowering_emits_explicit_extraction_instructions() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_field_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "typ Point: rec = { x: int, y: int };\nfun[] main(point: Point): int = {\n    return point.x;\n};",
    )
    .expect("should write lowering field fixture");

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
        .expect("field access lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert!(
        routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::FieldAccess { .. })),
        "record field access should lower into an explicit FieldAccess instruction"
    );
}

#[test]
fn index_access_lowering_emits_explicit_container_access_instructions() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_index_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] head(values: vec[int]): int = {\n    return values[0];\n};",
    )
    .expect("should write lowering index fixture");

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
        .expect("index access lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "head")
        .expect("head routine should exist");

    assert!(
        routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::IndexAccess { .. })),
        "container index access should lower into an explicit IndexAccess instruction"
    );
}

#[test]
fn map_index_observes_move_only_receiver_and_key_without_transfer_loads() {
    let lowered = lower_fixture_workspace(concat!(
        "fun[] lookup(values: map[ptr[int], int], query: ptr[int]): int = {\n",
        "    return values[query];\n",
        "};\n",
    ));
    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "lookup")
        .expect("lookup routine should exist");
    let (container, index) = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::IndexAccess { container, index } => Some((*container, *index)),
            _ => None,
        })
        .expect("map lookup should lower into IndexAccess");

    assert_eq!(container, routine.params[0]);
    assert_eq!(index, routine.params[1]);
    assert!(
        !routine.instructions.iter().any(|instr| matches!(
            &instr.kind,
            LoweredInstrKind::LoadLocal { local }
                if *local == routine.params[0] || *local == routine.params[1]
        )),
        "borrowed lookup operands must not be materialized through transfer loads",
    );
}

#[test]
fn slice_access_lowering_emits_explicit_slice_instructions() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_slice_exprs_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] mid(values: vec[int]): vec[int] = {\n    return values[1:3];\n};",
    )
    .expect("should write lowering slice fixture");

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
        .expect("slice access lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "mid")
        .expect("mid routine should exist");

    assert!(
        routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::SliceAccess { .. })),
        "container slice access should lower into an explicit SliceAccess instruction"
    );
}

#[test]
fn procedure_style_free_call_lowering_emits_void_call_instruction() {
    let workspace = lower_fixture_workspace(
        "pro greet(): non = {\n    return;\n};\nfun[] main(): int = {\n    greet();\n    return 0;\n};",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    let call_instrs: Vec<_> = routine
        .instructions
        .iter()
        .filter(|instr| matches!(instr.kind, LoweredInstrKind::Call { .. }))
        .collect();

    assert_eq!(
        call_instrs.len(),
        1,
        "procedure-style free call should produce exactly one Call instruction"
    );
    assert_eq!(
        call_instrs[0].result, None,
        "procedure-style call should have no result local"
    );
}

#[test]
fn procedure_style_method_call_lowering_emits_void_call_instruction() {
    let workspace = lower_fixture_workspace(
        "typ Box: rec = { value: int };\npro (Box)reset(): non = {\n    return;\n};\nfun[] main(b: Box): int = {\n    b.reset();\n    return 0;\n};",
    );

    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    let call_instrs: Vec<_> = routine
        .instructions
        .iter()
        .filter(|instr| matches!(instr.kind, LoweredInstrKind::Call { .. }))
        .collect();

    assert_eq!(
        call_instrs.len(),
        1,
        "procedure-style method call should produce exactly one Call instruction"
    );
    assert_eq!(
        call_instrs[0].result, None,
        "procedure-style method call should have no result local"
    );
}

#[test]
fn ordinary_lock_and_unlock_methods_lower_as_calls() {
    let workspace = lower_fixture_workspace(
        "typ Gate: rec = { value: int };\n\
         pro (Gate)lock(): non = { return; };\n\
         fun (Gate)unlock(): int = { return self.value; };\n\
         fun[] main(gate: Gate): int = {\n\
             gate.lock();\n\
             return gate.unlock();\n\
         };",
    );
    let routine = workspace
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");

    assert_eq!(
        routine
            .instructions
            .iter()
            .filter(|instr| matches!(instr.kind, LoweredInstrKind::Call { .. }))
            .count(),
        2,
    );
    assert!(routine.instructions.iter().all(|instr| !matches!(
        instr.kind,
        LoweredInstrKind::MutexLock { .. } | LoweredInstrKind::MutexUnlock { .. }
    )));
}

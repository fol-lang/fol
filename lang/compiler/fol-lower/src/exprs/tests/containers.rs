use crate::{LoweredInstrKind, LoweredOperand, LoweringErrorKind};
use fol_parser::ast::AstParser;
use fol_resolver::resolve_package_workspace;
use fol_stream::FileStream;
use fol_typecheck::Typechecker;

#[test]
fn record_initializer_lowering_constructs_records_in_binding_and_call_contexts() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_record_init_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "typ User: rec = { name: str, count: int };\nfun[] echo(user: User): User = { return user; };\nfun[] main(): User = {\n    var current: User = { name = \"ok\", count = 1 };\n    return echo({ name = \"next\", count = 2 });\n};",
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
    let lowered = crate::LoweringSession::new(typed)
        .lower_workspace()
        .expect("record initializer lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let construct_types = routine
        .instructions
        .iter()
        .filter_map(|instr| match &instr.kind {
            LoweredInstrKind::ConstructRecord { type_id, fields } => {
                Some((*type_id, fields.len()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(construct_types.len(), 2);
    assert_eq!(construct_types[0], construct_types[1]);
    assert_eq!(construct_types[0].1, 2);
}

#[test]
fn linear_container_lowering_constructs_array_vector_and_sequence_values() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_linear_container_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] make_arr(): arr[int, 3] = { return {1, 2, 3}; };\nfun[] make_vec(): vec[int] = { return {1, 2, 3}; };\nfun[] make_seq(): seq[int] = { return {1, 2, 3} };\n",
    )
    .expect("should write lowering linear-container fixture");

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
        .expect("linear container lowering should succeed");

    for (routine_name, expected_kind, expected_len) in [
        ("make_arr", crate::control::LoweredLinearKind::Array, 3usize),
        (
            "make_vec",
            crate::control::LoweredLinearKind::Vector,
            3usize,
        ),
        (
            "make_seq",
            crate::control::LoweredLinearKind::Sequence,
            3usize,
        ),
    ] {
        let routine = lowered
            .entry_package()
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .expect("lowered routine should exist");
        let construct = routine
            .instructions
            .iter()
            .find_map(|instr| match &instr.kind {
                LoweredInstrKind::ConstructLinear {
                    kind,
                    type_id: _,
                    elements,
                } => Some((*kind, elements.len())),
                _ => None,
            });

        assert_eq!(construct, Some((expected_kind, expected_len)));
    }
}

#[test]
fn set_and_map_lowering_construct_explicit_aggregate_instructions() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_set_map_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "fun[] take_set(items: set[int, str]): str = { return items[1]; };\nfun[] take_map(items: map[str, int]): int = { return items[\"US\"]; };\nfun[] main(): int = {\n    var parts: set[int, str] = {1, \"two\"};\n    var counts: map[str, int] = {{\"US\", 45}, {\"DE\", 82}};\n    var current: str = take_set(parts);\n    return take_map(counts);\n};\n",
    )
    .expect("should write lowering set/map fixture");

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
        .expect("set/map lowering should succeed");

    let routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let set_instr = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::ConstructSet { members, .. } => Some(members.len()),
            _ => None,
        });
    let map_instr = routine
        .instructions
        .iter()
        .find_map(|instr| match &instr.kind {
            LoweredInstrKind::ConstructMap { entries, .. } => Some(entries.len()),
            _ => None,
        });

    assert_eq!(set_instr, Some(2));
    assert_eq!(map_instr, Some(2));
}

#[test]
fn entry_variant_lowering_supports_payload_access_and_entry_construction() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_entry_variant_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "typ Color: ent = {\n    var BLUE: str = \"#0037cd\";\n    var RED: str = \"#ff0000\";\n};\nfun[] payload(): str = {\n    return Color.BLUE;\n};\nfun[] typed(): Color = {\n    return Color.RED;\n};\n",
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
    let lowered = crate::LoweringSession::new(typed)
        .lower_workspace()
        .expect("entry variant lowering should succeed");

    let payload_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "payload")
        .expect("payload routine should exist");
    let typed_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "typed")
        .expect("typed routine should exist");

    assert!(
        payload_routine
            .instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::Const(LoweredOperand::Str(_)))),
        "entry payload access should lower the default payload expression"
    );
    assert!(
        typed_routine.instructions.iter().any(|instr| matches!(
            &instr.kind,
            LoweredInstrKind::ConstructEntry {
                variant,
                payload: Some(_),
                ..
            } if variant == "RED"
        )),
        "typed entry construction should lower to an explicit ConstructEntry instruction"
    );
}

#[test]
fn nil_lowering_constructs_optional_and_error_shell_values() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_nil_shells_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "ali MaybeText: opt[str];\nali Failure: err[str];\nfun[] make(): MaybeText = { return nil; };\nfun[] fail(): int / Failure = {\n    report nil;\n    return 1;\n};\n",
    )
    .expect("should write lowering nil fixture");

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
        .expect("nil lowering should succeed");

    let make_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "make")
        .expect("make routine should exist");
    let fail_routine = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "fail")
        .expect("fail routine should exist");

    assert!(
        make_routine.instructions.iter().any(|instr| matches!(
            instr.kind,
            LoweredInstrKind::ConstructOptional { value: None, .. }
        )),
        "nil in an optional context should lower to an explicit empty optional constructor"
    );
    assert!(
        fail_routine.instructions.iter().any(|instr| matches!(
            instr.kind,
            LoweredInstrKind::ConstructError { value: None, .. }
        )),
        "nil in a typed error context should lower to an explicit empty error constructor"
    );
}

#[test]
fn unwrap_lowering_uses_explicit_shell_unwrap_instructions() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_unwrap_shells_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "ali MaybeText: opt[str];\nali Failure: err[str];\nfun[] from_optional(value: MaybeText): str = { return value!; };\nfun[] from_error(value: Failure): str = { return value! };\n",
    )
    .expect("should write lowering unwrap fixture");

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
        .expect("unwrap lowering should succeed");

    for routine_name in ["from_optional", "from_error"] {
        let routine = lowered
            .entry_package()
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .unwrap_or_else(|| panic!("{routine_name} routine should exist"));

        assert!(
            routine.instructions.iter().any(|instr| matches!(
                instr.kind,
                LoweredInstrKind::UnwrapShell { .. }
            )),
            "{routine_name} should lower postfix unwrap into an explicit shell-unwrapping instruction"
        );
    }
}

#[test]
fn alias_shell_contexts_lower_to_concrete_runtime_shell_operations() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tmp path")
        .as_nanos();
    let root = super::safe_temp_dir().join(format!("fol_lower_shell_alias_{stamp}"));
    let app_dir = root.join("app");
    let shared_dir = root.join("shared");
    fs::create_dir_all(&app_dir).expect("should create app dir");
    fs::create_dir_all(&shared_dir).expect("should create shared dir");
    fs::write(
        shared_dir.join("lib.fol"),
        "ali RemoteText: opt[str];\nali RemoteFailure: err[str];\nfun imported_wrap(): RemoteText = { return \"shared\"; };\nfun imported_fail(): int / RemoteFailure = {\n    report \"shared\";\n    return 1;\n};\nfun[exp] remote_ping(): int = { return 1; };\n",
    )
    .expect("should write shared package");
    // Aliases are never exported in current FOL (`ali` has no `[exp]` form),
    // so imported alias-backed shells are exercised inside the imported
    // package itself rather than through qualified alias type references.
    fs::write(
        app_dir.join("main.fol"),
        "use shared: loc = {\"../shared\"};\nali LocalText: opt[str];\nali LocalFailure: err[str];\nfun[] local_wrap(): LocalText = { return \"local\"; };\nfun[] local_fail(): int / LocalFailure = {\n    report \"local\";\n    return 1;\n};\nfun[] main(): int = {\n    return shared::remote_ping();\n};\n",
    )
    .expect("should write app package");

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
        .expect("shell alias lowering should succeed");

    let app_package = lowered.entry_package();
    let shared_package = lowered
        .packages()
        .find(|package| package.identity.display_name == "shared")
        .expect("shared package should exist");
    for (package, routine_name) in [
        (app_package, "local_wrap"),
        (app_package, "local_fail"),
        (shared_package, "imported_wrap"),
        (shared_package, "imported_fail"),
    ] {
        let routine = package
            .routine_decls
            .values()
            .find(|routine| routine.name == routine_name)
            .unwrap_or_else(|| panic!("{routine_name} routine should exist"));

        let has_shell_ctor = routine.instructions.iter().any(|instr| {
            matches!(
                instr.kind,
                LoweredInstrKind::ConstructOptional { value: Some(_), .. }
                    | LoweredInstrKind::ConstructError { value: Some(_), .. }
            )
        });
        assert!(
            has_shell_ctor,
            "{routine_name} should lower alias-backed shell contexts into concrete runtime shell construction"
        );
    }
}

#[test]
fn shell_payload_lifting_lowers_to_explicit_runtime_wrappers() {
    let fixture = super::safe_temp_dir().join(format!(
        "fol_lower_shell_lifts_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
    std::fs::write(
        &fixture,
        "ali MaybeText: opt[str];\nali Failure: err[str];\nfun[] echo(value: MaybeText): MaybeText = { return value; };\nfun[] direct(): MaybeText = { return \"return\"; };\nfun[] main(): MaybeText = {\n    var local: MaybeText = \"bind\";\n    return echo(\"call\");\n};\nfun[] fail(): int / Failure = {\n    report \"broken\";\n    return 1;\n};\n",
    )
    .expect("should write lowering shell lift fixture");

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
        .expect("shell lifting lowering should succeed");

    let direct = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "direct")
        .expect("direct routine should exist");
    let main = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let fail = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "fail")
        .expect("fail routine should exist");

    assert!(
        direct.instructions.iter().any(|instr| matches!(
            instr.kind,
            LoweredInstrKind::ConstructOptional { value: Some(_), .. }
        )),
        "return payload lifting should lower to an explicit optional constructor"
    );
    assert!(
        main.instructions
            .iter()
            .filter(|instr| matches!(
                instr.kind,
                LoweredInstrKind::ConstructOptional { value: Some(_), .. }
            ))
            .count()
            >= 2,
        "binding and call payload lifting should each lower to explicit optional constructors"
    );
    assert!(
        fail.instructions.iter().any(|instr| matches!(
            instr.kind,
            LoweredInstrKind::ConstructError { value: Some(_), .. }
        )),
        "report payload lifting should lower to an explicit error constructor"
    );
}

#[test]
fn aggregate_container_and_shell_lowering_stays_aligned_across_local_and_imported_surfaces() {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic enough for tmp path")
        .as_nanos();
    let root = super::safe_temp_dir().join(format!("fol_lower_parity_mix_{stamp}"));
    let app_dir = root.join("app");
    let shared_dir = root.join("shared");
    fs::create_dir_all(&app_dir).expect("should create app dir");
    fs::create_dir_all(&shared_dir).expect("should create shared dir");
    // Aliases are never exported in current FOL, so the imported package
    // exercises its shell alias internally while records cross the package
    // boundary through `typ[exp]`.
    fs::write(
        shared_dir.join("lib.fol"),
        "ali RemoteText: opt[str];\ntyp[exp] RemoteUser: rec = { name: str, count: int };\nfun[exp] keep_remote(user: RemoteUser): RemoteUser = { return user; };\nfun remote_label(): RemoteText = { return \"shared\"; };\n",
    )
    .expect("should write shared package");
    fs::write(
        app_dir.join("main.fol"),
        "use shared: loc = {\"../shared\"};\nali LocalText: opt[str];\ntyp LocalUser: rec = { name: str, count: int };\nfun[] main(): shared::RemoteUser = {\n    var local: LocalText = \"ok\";\n    var local_user: LocalUser = { name = \"local\", count = 1 };\n    var remote_user: shared::RemoteUser = { name = \"remote\", count = 2 };\n    var ids: seq[int] = {1, 2, 3};\n    return shared::keep_remote(remote_user);\n};\n",
    )
    .expect("should write app package");

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
        .expect("mixed parity lowering should succeed");

    let main = lowered
        .entry_package()
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main routine should exist");
    let remote_label = lowered
        .packages()
        .find(|package| package.identity.display_name == "shared")
        .expect("shared package should exist")
        .routine_decls
        .values()
        .find(|routine| routine.name == "remote_label")
        .expect("remote_label routine should exist");

    assert!(
        main.instructions
            .iter()
            .any(|instr| matches!(
                instr.kind,
                LoweredInstrKind::ConstructOptional { value: Some(_), .. }
            )),
        "local shell aliases should lower to explicit shell constructors"
    );
    assert!(
        remote_label
            .instructions
            .iter()
            .any(|instr| matches!(
                instr.kind,
                LoweredInstrKind::ConstructOptional { value: Some(_), .. }
            )),
        "imported-package shell aliases should lower to explicit shell constructors"
    );
    assert!(
        main.instructions
            .iter()
            .filter(|instr| matches!(instr.kind, LoweredInstrKind::ConstructRecord { .. }))
            .count()
            >= 2,
        "local and imported record contexts should both lower to explicit record constructors"
    );
    assert!(
        main.instructions
            .iter()
            .any(|instr| matches!(instr.kind, LoweredInstrKind::ConstructLinear { .. })),
        "container literals should keep lowering alongside aggregate and shell surfaces"
    );
}

#[test]
fn unsupported_lowering_surfaces_report_explicit_boundary_messages() {
    // Typecheck now front-runs source-level `nil` outside opt/err shell
    // contexts, so the lowering boundary is exercised directly against a
    // non-shell expected runtime type.
    use crate::types::{LoweredBuiltinType, LoweredTypeTable};

    let mut types = LoweredTypeTable::new();
    let int_type = types.intern_builtin(LoweredBuiltinType::Int);
    let mut routine = crate::LoweredRoutine::new(
        crate::LoweredRoutineId(0),
        "main",
        crate::LoweredBlockId(0),
    );
    let entry = routine.blocks.push(crate::LoweredBlock {
        id: crate::LoweredBlockId(0),
        instructions: Vec::new(),
        terminator: None,
    });
    routine.entry_block = entry;
    let mut cursor = super::super::cursor::RoutineCursor::new(&mut routine, entry);

    let nil_error =
        super::super::containers::lower_nil_literal(&types, &mut cursor, Some(int_type))
            .expect_err("nil lowering outside shell contexts should stay an explicit boundary");
    assert_eq!(nil_error.kind(), LoweringErrorKind::Unsupported);
    assert!(nil_error.message().contains(
        "nil lowering requires an expected opt[...] or err[...] runtime type"
    ));
}

#[test]
fn audited_v1_lowering_boundaries_fail_with_explicit_messages() {
    // Typecheck now rejects when/of branches before lowering runs, so the
    // remaining lowering boundary is exercised directly against the when-case
    // lowering entry point.
    let cases = [(
        crate::UnsupportedLoweringSurface::TypeMatchingWhenOf,
        fol_parser::ast::WhenCase::Of {
            type_match: fol_parser::ast::FolType::Int {
                size: None,
                signed: true,
            },
            body: Vec::new(),
        },
        "type-matching when/of branches are not lowered in this slice yet",
    )];

    assert_eq!(crate::v1_lowering_boundaries().len(), cases.len());

    for (surface, case, expected_message) in cases {
        let error = super::super::flow::when_case_condition_and_body(&case)
            .expect_err("when/of branches should stay an explicit lowering boundary");
        assert_eq!(
            error.kind(),
            LoweringErrorKind::Unsupported,
            "expected unsupported lowering for boundary '{}'",
            surface.label()
        );
        assert!(
            error.message().contains(expected_message),
            "expected lowering boundary '{}' to mention '{expected_message}', got: {:?}",
            surface.label(),
            error
        );
    }
}

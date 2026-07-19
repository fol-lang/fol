use fol_lower::{
    LoweredBuiltinType, LoweredEntryCandidate, LoweredExportMount, LoweredFieldLayout,
    LoweredGlobal, LoweredPackage, LoweredRecoverableAbi, LoweredRoutine, LoweredRoutineType,
    LoweredSourceMap, LoweredSourceMapEntry, LoweredSourceSymbol, LoweredSourceUnit, LoweredType,
    LoweredTypeDecl, LoweredTypeDeclKind, LoweredTypeTable, LoweredWorkspace,
};
use fol_package::{PackageConfig, PackageSession};
use fol_parser::ast::SyntaxOrigin;
use fol_resolver::{PackageIdentity, PackageSourceKind, SourceUnitId, SymbolId};
use fol_typecheck::Typechecker;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(crate) fn sample_lowered_workspace() -> LoweredWorkspace {
    sample_lowered_workspace_named("app")
}

pub(crate) fn sample_lowered_workspace_named(entry_name: &str) -> LoweredWorkspace {
    let entry_identity = package_identity(
        entry_name,
        PackageSourceKind::Entry,
        &format!("/workspace/{entry_name}"),
    );
    let shared_identity = package_identity("shared", PackageSourceKind::Local, "/workspace/shared");

    let mut type_table = LoweredTypeTable::new();
    let int_type = type_table.intern_builtin(LoweredBuiltinType::Int);
    let bool_type = type_table.intern_builtin(LoweredBuiltinType::Bool);
    let str_type = type_table.intern_builtin(LoweredBuiltinType::Str);

    let user_record_type = type_table.intern(LoweredType::Record {
        fields: BTreeMap::from([
            ("name".to_string(), str_type),
            ("active".to_string(), bool_type),
        ]),
        finalized: false,
    });
    let main_signature = type_table.intern(LoweredType::Routine(LoweredRoutineType {
        params: vec![bool_type],
        return_type: Some(int_type),
        error_type: Some(str_type),
    }));
    let helper_signature = type_table.intern(LoweredType::Routine(LoweredRoutineType {
        params: vec![int_type],
        return_type: Some(int_type),
        error_type: None,
    }));

    let mut entry_package =
        LoweredPackage::new(fol_lower::LoweredPackageId(0), entry_identity.clone());
    entry_package.source_units = vec![
        LoweredSourceUnit {
            source_unit_id: SourceUnitId(0),
            path: format!("{entry_name}/main.fol"),
            package: entry_name.to_string(),
            namespace: entry_name.to_string(),
        },
        LoweredSourceUnit {
            source_unit_id: SourceUnitId(1),
            path: format!("{entry_name}/math/add.fol"),
            package: entry_name.to_string(),
            namespace: format!("{entry_name}::math"),
        },
    ];
    entry_package.exports = vec![LoweredExportMount {
        source_namespace: entry_name.to_string(),
        mounted_namespace_suffix: None,
    }];
    entry_package.types = vec![user_record_type];
    entry_package.routines = vec![
        fol_lower::LoweredRoutineId(0),
        fol_lower::LoweredRoutineId(1),
    ];
    entry_package.globals = vec![fol_lower::LoweredGlobalId(0)];
    entry_package.type_decls.insert(
        SymbolId(10),
        LoweredTypeDecl {
            symbol_id: SymbolId(10),
            source_unit_id: SourceUnitId(0),
            name: "User".to_string(),
            runtime_type: user_record_type,
            kind: LoweredTypeDeclKind::Record {
                fields: vec![
                    LoweredFieldLayout {
                        name: "name".to_string(),
                        type_id: str_type,
                    },
                    LoweredFieldLayout {
                        name: "active".to_string(),
                        type_id: bool_type,
                    },
                ],
            },
        },
    );
    entry_package.global_decls.insert(
        fol_lower::LoweredGlobalId(0),
        LoweredGlobal {
            id: fol_lower::LoweredGlobalId(0),
            symbol_id: SymbolId(20),
            source_unit_id: SourceUnitId(0),
            name: "default_name".to_string(),
            type_id: str_type,
            mutable: false,
            initializer: None,
        },
    );
    entry_package
        .routine_signatures
        .insert(SymbolId(30), main_signature);
    entry_package
        .routine_signatures
        .insert(SymbolId(31), helper_signature);
    let mut main_routine = routine(
        fol_lower::LoweredRoutineId(0),
        "main",
        Some(SymbolId(30)),
        Some(SourceUnitId(0)),
        Some(main_signature),
    );
    // The main signature declares one bool parameter, so the routine must retain
    // the matching param local slot; the entry wrapper feeds it from CLI args.
    let main_param = main_routine.locals.push(fol_lower::LoweredLocal {
        id: fol_lower::LoweredLocalId(0),
        type_id: Some(bool_type),
        name: Some("flag".to_string()),
    });
    main_routine.params.push(main_param);
    entry_package
        .routine_decls
        .insert(fol_lower::LoweredRoutineId(0), main_routine);
    entry_package.routine_decls.insert(
        fol_lower::LoweredRoutineId(1),
        routine(
            fol_lower::LoweredRoutineId(1),
            "add_one",
            Some(SymbolId(31)),
            Some(SourceUnitId(1)),
            Some(helper_signature),
        ),
    );

    let mut shared_package =
        LoweredPackage::new(fol_lower::LoweredPackageId(1), shared_identity.clone());
    shared_package.source_units = vec![
        LoweredSourceUnit {
            source_unit_id: SourceUnitId(2),
            path: "shared/lib.fol".to_string(),
            package: "shared".to_string(),
            namespace: "shared".to_string(),
        },
        LoweredSourceUnit {
            source_unit_id: SourceUnitId(3),
            path: "shared/util/log.fol".to_string(),
            package: "shared".to_string(),
            namespace: "shared::util".to_string(),
        },
    ];
    shared_package.exports = vec![LoweredExportMount {
        source_namespace: "shared::util".to_string(),
        mounted_namespace_suffix: Some("util".to_string()),
    }];
    shared_package.globals = vec![fol_lower::LoweredGlobalId(1)];
    shared_package.routines = vec![fol_lower::LoweredRoutineId(2)];
    shared_package.global_decls.insert(
        fol_lower::LoweredGlobalId(1),
        LoweredGlobal {
            id: fol_lower::LoweredGlobalId(1),
            symbol_id: SymbolId(40),
            source_unit_id: SourceUnitId(2),
            name: "answer".to_string(),
            type_id: int_type,
            mutable: false,
            initializer: None,
        },
    );
    shared_package
        .routine_signatures
        .insert(SymbolId(41), helper_signature);
    shared_package.routine_decls.insert(
        fol_lower::LoweredRoutineId(2),
        routine(
            fol_lower::LoweredRoutineId(2),
            "emit",
            Some(SymbolId(41)),
            Some(SourceUnitId(3)),
            Some(helper_signature),
        ),
    );

    let mut packages = BTreeMap::new();
    packages.insert(entry_identity.clone(), entry_package);
    packages.insert(shared_identity.clone(), shared_package);

    let mut source_map = LoweredSourceMap::new();
    source_map.push(source_map_entry(
        &format!("{entry_name}/main.fol"),
        1,
        1,
        3,
        LoweredSourceSymbol::Routine(fol_lower::LoweredRoutineId(0)),
    ));
    source_map.push(source_map_entry(
        "shared/lib.fol",
        2,
        1,
        6,
        LoweredSourceSymbol::Global(fol_lower::LoweredGlobalId(1)),
    ));

    LoweredWorkspace::new(
        entry_identity.clone(),
        packages,
        vec![LoweredEntryCandidate {
            package_identity: entry_identity,
            routine_id: fol_lower::LoweredRoutineId(0),
            name: "main".to_string(),
        }],
        type_table,
        source_map,
        LoweredRecoverableAbi::v1(bool_type),
    )
}

pub(crate) fn package_identity(name: &str, kind: PackageSourceKind, root: &str) -> PackageIdentity {
    PackageIdentity {
        source_kind: kind,
        canonical_root: root.to_string(),
        display_name: name.to_string(),
    }
}

pub(crate) fn distinct_namespaces(workspace: &LoweredWorkspace) -> BTreeSet<String> {
    workspace
        .packages()
        .flat_map(|package| {
            package
                .source_units
                .iter()
                .map(|unit| unit.namespace.clone())
        })
        .collect()
}

pub(crate) fn lowered_workspace_from_entry_path(path: &Path) -> LoweredWorkspace {
    lowered_workspace_from_entry_path_with_config(
        path,
        PackageConfig::default(),
        fol_resolver::ResolverConfig::default(),
    )
}

pub(crate) fn lowered_workspace_from_entry_path_with_config(
    path: &Path,
    package_config: PackageConfig,
    resolver_config: fol_resolver::ResolverConfig,
) -> LoweredWorkspace {
    let mut stream = if path.is_dir() {
        fol_stream::FileStream::from_folder(
            path.to_str()
                .expect("backend test folder fixtures should be valid utf-8"),
        )
        .expect("backend test folder fixture should open")
    } else {
        fol_stream::FileStream::from_file(
            path.to_str()
                .expect("backend test file fixtures should be valid utf-8"),
        )
        .expect("backend test file fixture should open")
    };
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = fol_parser::ast::AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("backend test fixture should parse");
    let prepared = PackageSession::with_config(package_config)
        .prepare_entry_package(syntax)
        .expect("backend test fixture should prepare");
    let resolved = fol_resolver::resolve_prepared_workspace_with_config(prepared, resolver_config)
        .expect("backend test fixture should resolve");
    let typed = Typechecker::new()
        .check_resolved_workspace(resolved)
        .expect("backend test fixture should typecheck");
    fol_lower::Lowerer::new()
        .lower_typed_workspace(typed)
        .expect("backend test fixture should lower")
}

fn routine(
    id: fol_lower::LoweredRoutineId,
    name: &str,
    symbol_id: Option<SymbolId>,
    source_unit_id: Option<SourceUnitId>,
    signature: Option<fol_lower::LoweredTypeId>,
) -> LoweredRoutine {
    let mut routine = LoweredRoutine::new(id, name, fol_lower::LoweredBlockId(0));
    routine.symbol_id = symbol_id;
    routine.source_unit_id = source_unit_id;
    routine.signature = signature;
    routine
}

fn source_map_entry(
    file: &str,
    line: usize,
    column: usize,
    length: usize,
    symbol: LoweredSourceSymbol,
) -> LoweredSourceMapEntry {
    LoweredSourceMapEntry {
        symbol,
        origin: SyntaxOrigin {
            file: Some(file.to_string()),
            line,
            column,
            length,
        },
    }
}

use crate::{
    decls, exprs,
    ids::{LoweredPackageId, LoweredTypeId},
    types::{LoweredBuiltinType, LoweredRoutineType, LoweredType, LoweredTypeTable},
    verify, LoweredEntryCandidate, LoweredExportMount, LoweredPackage, LoweredRecoverableAbi,
    LoweredSourceMap, LoweredSourceMapEntry, LoweredSourceSymbol, LoweredSourceUnit,
    LoweredSymbolOwnership, LoweredWorkspace, LoweringError, LoweringErrorKind, LoweringResult,
};
use fol_resolver::PackageIdentity;
use fol_typecheck::{BuiltinType, CheckedType, CheckedTypeId, DeclaredTypeKind};
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct LoweringSession {
    typed: fol_typecheck::TypedWorkspace,
}

impl LoweringSession {
    pub fn new(typed: fol_typecheck::TypedWorkspace) -> Self {
        Self { typed }
    }

    pub fn typed_workspace(&self) -> &fol_typecheck::TypedWorkspace {
        &self.typed
    }

    pub fn lower_workspace(self) -> LoweringResult<LoweredWorkspace> {
        let entry_identity = self.typed.entry_identity().clone();
        let mut packages = BTreeMap::new();
        let mut type_table = LoweredTypeTable::new();
        let mut lowered_type_cache = BTreeMap::new();
        let mut next_global_index = 0;
        let mut next_routine_index = 0;

        for (index, package) in self.typed.packages().enumerate() {
            let mut lowered =
                LoweredPackage::new(LoweredPackageId(index), package.identity.clone());
            lowered.exports = package
                .export_mounts
                .iter()
                .map(|mount| LoweredExportMount {
                    source_namespace: mount.source_namespace.clone(),
                    mounted_namespace_suffix: mount.mounted_namespace_suffix.clone(),
                })
                .collect();
            lowered.source_units = package
                .program
                .ordinary_source_units()
                .map(|unit| LoweredSourceUnit {
                    source_unit_id: unit.source_unit_id,
                    path: unit.path.clone(),
                    package: unit.package.clone(),
                    namespace: unit.namespace.clone(),
                })
                .collect();
            lowered.symbol_ownership = package
                .program
                .resolved()
                .symbols
                .iter_with_ids()
                .map(|(symbol_id, symbol)| {
                    let mounted_from = symbol.mounted_from.clone();
                    (
                        symbol_id,
                        LoweredSymbolOwnership {
                            symbol_id,
                            source_unit_id: symbol.source_unit,
                            owning_package: mounted_from
                                .as_ref()
                                .map(|provenance| provenance.package_identity.clone())
                                .unwrap_or_else(|| package.identity.clone()),
                            mounted_from,
                        },
                    )
                })
                .collect();
            lowered.checked_type_map = (0..package.program.type_table().len())
                .map(|raw_type_id| {
                    let checked_type_id = CheckedTypeId(raw_type_id);
                    translate_checked_type(
                        &mut type_table,
                        &mut lowered_type_cache,
                        &package.identity,
                        &package.program,
                        checked_type_id,
                    )
                    .map(|lowered_type_id| (checked_type_id, lowered_type_id))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            decls::lower_routine_signatures(package, &mut lowered)?;
            decls::lower_alias_declarations(package, &mut lowered)?;
            decls::lower_record_declarations(package, &mut lowered)?;
            decls::lower_entry_declarations(package, &mut lowered)?;
            decls::lower_standard_declarations(package, &mut lowered)?;
            decls::synthesize_structural_runtime_type_declarations(package, &mut lowered)?;
            decls::lower_global_declarations(package, &mut lowered, &mut next_global_index)?;
            decls::lower_routine_declarations(package, &mut lowered, &mut next_routine_index)?;
            packages.insert(package.identity.clone(), lowered);
        }

        let decl_index = exprs::WorkspaceDeclIndex::from_workspace(&self.typed, &packages);

        for package in self.typed.packages() {
            let Some(lowered) = packages.get_mut(&package.identity) else {
                continue;
            };
            exprs::lower_routine_bodies(
                package,
                &type_table,
                &decl_index,
                lowered,
                &mut next_routine_index,
            )?;
        }

        crate::mono::monomorphize_generic_receiver_routines(
            &mut packages,
            &mut type_table,
            &mut next_routine_index,
        )?;

        let source_map = build_workspace_source_map(&self.typed, &packages);

        let entry_candidates = packages
            .get(&entry_identity)
            .into_iter()
            .flat_map(|package| {
                package
                    .routine_decls
                    .iter()
                    .filter(|(_, routine)| routine.name == "main")
                    .map(|(routine_id, routine)| LoweredEntryCandidate {
                        package_identity: entry_identity.clone(),
                        routine_id: *routine_id,
                        name: routine.name.clone(),
                    })
            })
            .collect::<Vec<_>>();

        let recoverable_abi =
            LoweredRecoverableAbi::v1(type_table.intern_builtin(LoweredBuiltinType::Bool));
        let workspace = LoweredWorkspace::new(
            entry_identity,
            packages,
            entry_candidates,
            type_table,
            source_map,
            recoverable_abi,
        );
        verify::verify_workspace(&workspace)?;
        Ok(workspace)
    }
}

fn build_workspace_source_map(
    typed: &fol_typecheck::TypedWorkspace,
    packages: &BTreeMap<PackageIdentity, LoweredPackage>,
) -> LoweredSourceMap {
    let mut source_map = LoweredSourceMap::new();

    for typed_package in typed.packages() {
        let Some(lowered_package) = packages.get(&typed_package.identity) else {
            continue;
        };

        for source_unit in typed_package.program.source_units() {
            if source_unit.kind == fol_parser::ast::ParsedSourceUnitKind::Build {
                continue;
            }
            for syntax_id in &source_unit.top_level_nodes {
                if let Some(origin) = typed_package
                    .program
                    .resolved()
                    .syntax_index()
                    .origin(*syntax_id)
                {
                    source_map.push(LoweredSourceMapEntry {
                        symbol: LoweredSourceSymbol::Package(lowered_package.id),
                        origin: origin.clone(),
                    });
                }
            }
        }
    }

    source_map
}

fn translate_checked_type(
    lowered_types: &mut LoweredTypeTable,
    cache: &mut BTreeMap<(PackageIdentity, CheckedTypeId), LoweredTypeId>,
    package_identity: &PackageIdentity,
    program: &fol_typecheck::TypedProgram,
    checked_type_id: CheckedTypeId,
) -> Result<LoweredTypeId, Vec<LoweringError>> {
    if let Some(existing) = cache.get(&(package_identity.clone(), checked_type_id)) {
        return Ok(*existing);
    }

    let checked_type = program
        .type_table()
        .get(checked_type_id)
        .cloned()
        .ok_or_else(|| {
            vec![LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!("typed program lost checked type {}", checked_type_id.0),
            )]
        })?;

    let lowered_type_id = match checked_type {
        CheckedType::Builtin(builtin) => lowered_types.intern_builtin(lower_builtin(builtin)),
        CheckedType::Declared {
            symbol, name, kind, ..
        } => {
            if kind == DeclaredTypeKind::GenericParameter {
                let lowered = lowered_types.intern(LoweredType::GenericParameter { name });
                cache.insert((package_identity.clone(), checked_type_id), lowered);
                return Ok(lowered);
            }
            // Apparent overrides are authoritative structural shapes. They
            // cover generic instantiations and declared types imported through
            // another package without mounting the declaration itself. The
            // latter intentionally carries only a nominal shell in the target
            // type table; its foreign SymbolId is not valid in the importing
            // program and may collide with an unrelated local symbol.
            if let Some(apparent) = program.apparent_type_override(checked_type_id) {
                let lowered = translate_checked_type(
                    lowered_types,
                    cache,
                    package_identity,
                    program,
                    apparent,
                )?;
                cache.insert((package_identity.clone(), checked_type_id), lowered);
                return Ok(lowered);
            }
            let typed_symbol = program.typed_symbol(symbol);
            let runtime_type = typed_symbol
                .and_then(|typed_symbol| typed_symbol.declared_type)
                .ok_or_else(|| {
                    let detail = program
                        .resolved()
                        .symbol(symbol)
                        .map(|resolved_symbol| {
                            format!(
                                "{} '{}' in scope {}",
                                match resolved_symbol.kind {
                                    fol_resolver::SymbolKind::Type => "type",
                                    fol_resolver::SymbolKind::Alias => "alias",
                                    fol_resolver::SymbolKind::Routine => "routine",
                                    fol_resolver::SymbolKind::ValueBinding => "value",
                                    fol_resolver::SymbolKind::LabelBinding => "label",
                                    fol_resolver::SymbolKind::DestructureBinding => "destructure",
                                    fol_resolver::SymbolKind::Definition => "definition",
                                    fol_resolver::SymbolKind::Segment => "segment",
                                    fol_resolver::SymbolKind::ImportAlias => "import-alias",
                                    fol_resolver::SymbolKind::Parameter => "parameter",
                                    fol_resolver::SymbolKind::GenericParameter => "generic",
                                    fol_resolver::SymbolKind::Standard => "standard",
                                    fol_resolver::SymbolKind::Capture => "capture",
                                    fol_resolver::SymbolKind::LoopBinder => "loop-binder",
                                    fol_resolver::SymbolKind::RollingBinder => "rolling-binder",
                                },
                                resolved_symbol.name,
                                resolved_symbol.scope.0
                            )
                        })
                        .unwrap_or_else(|| format!("symbol {}", symbol.0));
                    vec![LoweringError::with_kind(
                        LoweringErrorKind::InvalidInput,
                        format!(
                            "typed declared {detail} does not retain a lowered runtime shape yet"
                        ),
                    )]
                })?;
            translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                runtime_type,
            )?
        }
        CheckedType::Array { element_type, size } => {
            let element_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                element_type,
            )?;
            lowered_types.intern(LoweredType::Array { element_type, size })
        }
        CheckedType::Vector { element_type } => {
            let element_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                element_type,
            )?;
            lowered_types.intern(LoweredType::Vector { element_type })
        }
        CheckedType::Sequence { element_type } => {
            let element_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                element_type,
            )?;
            lowered_types.intern(LoweredType::Sequence { element_type })
        }
        CheckedType::Channel { element_type } => {
            let element_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                element_type,
            )?;
            lowered_types.intern(LoweredType::Channel { element_type })
        }
        CheckedType::ChannelSender { element_type } => {
            let element_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                element_type,
            )?;
            lowered_types.intern(LoweredType::ChannelSender { element_type })
        }
        CheckedType::ChannelReceiver { element_type } => {
            let element_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                element_type,
            )?;
            lowered_types.intern(LoweredType::ChannelReceiver { element_type })
        }
        CheckedType::Eventual {
            value_type,
            error_type,
        } => {
            let value_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                value_type,
            )?;
            let error_type = error_type
                .map(|error_type| {
                    translate_checked_type(
                        lowered_types,
                        cache,
                        package_identity,
                        program,
                        error_type,
                    )
                })
                .transpose()?;
            lowered_types.intern(LoweredType::Eventual {
                value_type,
                error_type,
            })
        }
        CheckedType::Owned { inner } => {
            let inner = match program.type_table().get(inner).cloned() {
                Some(CheckedType::Declared { symbol, name, .. }) => {
                    lowered_types.intern(LoweredType::Named {
                        package: package_identity.clone(),
                        symbol,
                        name,
                    })
                }
                _ => {
                    translate_checked_type(lowered_types, cache, package_identity, program, inner)?
                }
            };
            lowered_types.intern(LoweredType::Owned { inner })
        }
        CheckedType::Borrowed { inner, mutable } => {
            let inner =
                translate_checked_type(lowered_types, cache, package_identity, program, inner)?;
            lowered_types.intern(LoweredType::Borrowed { inner, mutable })
        }
        CheckedType::Pointer {
            target,
            shared,
            weak,
            sync,
        } => {
            let target = match program.type_table().get(target).cloned() {
                Some(CheckedType::Declared { symbol, name, .. }) => {
                    lowered_types.intern(LoweredType::Named {
                        package: package_identity.clone(),
                        symbol,
                        name,
                    })
                }
                _ => {
                    translate_checked_type(lowered_types, cache, package_identity, program, target)?
                }
            };
            lowered_types.intern(LoweredType::Pointer {
                target,
                shared,
                weak,
                sync,
            })
        }
        CheckedType::Set { member_types } => {
            let member_types = member_types
                .into_iter()
                .map(|member_type| {
                    translate_checked_type(
                        lowered_types,
                        cache,
                        package_identity,
                        program,
                        member_type,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            lowered_types.intern(LoweredType::Set { member_types })
        }
        CheckedType::Map {
            key_type,
            value_type,
        } => {
            let key_type =
                translate_checked_type(lowered_types, cache, package_identity, program, key_type)?;
            let value_type = translate_checked_type(
                lowered_types,
                cache,
                package_identity,
                program,
                value_type,
            )?;
            lowered_types.intern(LoweredType::Map {
                key_type,
                value_type,
            })
        }
        CheckedType::Optional { inner } => {
            let inner =
                translate_checked_type(lowered_types, cache, package_identity, program, inner)?;
            lowered_types.intern(LoweredType::Optional { inner })
        }
        CheckedType::Error { inner } => {
            let inner = inner
                .map(|inner| {
                    translate_checked_type(lowered_types, cache, package_identity, program, inner)
                })
                .transpose()?;
            lowered_types.intern(LoweredType::Error { inner })
        }
        CheckedType::Record { fields } => {
            let fields = fields
                .into_iter()
                .map(|(field_name, field_type)| {
                    translate_checked_type(
                        lowered_types,
                        cache,
                        package_identity,
                        program,
                        field_type,
                    )
                    .map(|lowered_field_type| (field_name, lowered_field_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let finalized = program.type_resolves_to_fin(checked_type_id);
            lowered_types.intern(LoweredType::Record { fields, finalized })
        }
        CheckedType::Entry { variants } => {
            let variants = variants
                .into_iter()
                .map(|(variant_name, variant_type)| {
                    variant_type
                        .map(|variant_type| {
                            translate_checked_type(
                                lowered_types,
                                cache,
                                package_identity,
                                program,
                                variant_type,
                            )
                        })
                        .transpose()
                        .map(|lowered_variant_type| (variant_name, lowered_variant_type))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            lowered_types.intern(LoweredType::Entry { variants })
        }
        CheckedType::Routine(signature) => {
            let params = signature
                .params
                .into_iter()
                .map(|param_type| {
                    translate_checked_type(
                        lowered_types,
                        cache,
                        package_identity,
                        program,
                        param_type,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let return_type = signature
                .return_type
                .map(|return_type| {
                    translate_checked_type(
                        lowered_types,
                        cache,
                        package_identity,
                        program,
                        return_type,
                    )
                })
                .transpose()?;
            let error_type = signature
                .error_type
                .map(|error_type| {
                    translate_checked_type(
                        lowered_types,
                        cache,
                        package_identity,
                        program,
                        error_type,
                    )
                })
                .transpose()?;
            lowered_types.intern(LoweredType::Routine(LoweredRoutineType {
                params,
                return_type,
                error_type,
            }))
        }
    };

    cache.insert((package_identity.clone(), checked_type_id), lowered_type_id);
    Ok(lowered_type_id)
}

fn lower_builtin(builtin: BuiltinType) -> LoweredBuiltinType {
    match builtin {
        BuiltinType::Int => LoweredBuiltinType::Int,
        BuiltinType::Float => LoweredBuiltinType::Float,
        BuiltinType::Bool => LoweredBuiltinType::Bool,
        BuiltinType::Char => LoweredBuiltinType::Char,
        BuiltinType::Str => LoweredBuiltinType::Str,
        BuiltinType::Never => LoweredBuiltinType::Never,
    }
}

#[cfg(test)]
mod tests {
    use super::LoweringSession;
    use crate::types::{LoweredBuiltinType, LoweredType};
    use fol_parser::ast::AstParser;
    use fol_resolver::{
        resolve_package_workspace, resolve_package_workspace_with_config, ResolverConfig,
    };
    use fol_stream::FileStream;
    use fol_typecheck::Typechecker;

    fn safe_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("fol_test");
        std::fs::create_dir_all(&dir).expect("should create test temp root");
        dir
    }

    #[test]
    fn lowering_session_keeps_typed_workspace_identity_and_size() {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream = FileStream::from_file(fixture_path).expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        let session = LoweringSession::new(typed);

        assert_eq!(
            session.typed_workspace().entry_identity().display_name,
            "parser"
        );
        assert_eq!(session.typed_workspace().package_count(), 1);
    }

    #[test]
    fn lowering_session_translates_single_package_source_units_and_symbols() {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream = FileStream::from_file(fixture_path).expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering fixture should typecheck");

        let lowered = LoweringSession::new(typed)
            .lower_workspace()
            .expect("Lowering shell should now translate workspace shells");

        assert_eq!(lowered.package_count(), 1);
        let entry = lowered.entry_package();
        assert_eq!(entry.source_units.len(), 1);
        assert!(!entry.symbol_ownership.is_empty());
        assert!(!entry.checked_type_map.is_empty());
        assert!(!lowered.source_map().is_empty());
        assert_eq!(
            lowered
                .type_table()
                .get(
                    *entry
                        .checked_type_map
                        .get(&fol_typecheck::CheckedTypeId(0))
                        .expect("int builtin should translate")
                )
                .expect("lowered builtin type should exist"),
            &LoweredType::Builtin(LoweredBuiltinType::Int)
        );
    }

    #[test]
    fn lowering_session_translates_loaded_package_identity_boundaries() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tmp path")
            .as_nanos();
        let root = safe_temp_dir().join(format!("fol_lower_workspace_{stamp}"));
        let app_dir = root.join("app");
        let shared_dir = root.join("shared");
        fs::create_dir_all(&app_dir).expect("should create app dir");
        fs::create_dir_all(&shared_dir).expect("should create shared dir");
        fs::write(
            app_dir.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nfun[] main(): int = { return answer; };",
        )
        .expect("should write app entry");
        fs::write(shared_dir.join("lib.fol"), "var[exp] answer: int = 7;")
            .expect("should write shared library");

        let mut stream = FileStream::from_folder(app_dir.to_str().expect("utf8 temp path"))
            .expect("should open folder fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering folder fixture should parse");
        let resolved =
            resolve_package_workspace(syntax).expect("Lowering folder fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering folder fixture should typecheck");

        let lowered = LoweringSession::new(typed)
            .lower_workspace()
            .expect("Lowering shell should translate loaded package shells");

        assert_eq!(lowered.package_count(), 2);
        assert!(lowered
            .packages()
            .any(|package| package.identity.display_name == "shared"));
        let lowered_files = lowered
            .source_map()
            .entries()
            .iter()
            .filter_map(|entry| entry.origin.file.clone())
            .collect::<Vec<_>>();
        assert!(lowered_files
            .iter()
            .any(|path| path.ends_with("app/main.fol")));
        assert!(lowered_files
            .iter()
            .any(|path| path.ends_with("shared/lib.fol")));
        let app_package = lowered.entry_package();
        let imported_symbol = app_package
            .symbol_ownership
            .values()
            .find(|ownership| ownership.mounted_from.is_some())
            .expect("entry package should retain at least one mounted imported symbol");
        assert_eq!(imported_symbol.owning_package.display_name, "shared");
        assert_eq!(
            imported_symbol
                .mounted_from
                .as_ref()
                .expect("mounted symbol should keep foreign provenance")
                .package_identity
                .display_name,
            "shared"
        );
    }

    #[test]
    fn lowering_session_retains_prepared_package_export_mounts() {
        use fol_package::{
            PackageBuildDefinition, PackageBuildMode, PackageMetadata, PreparedExportMount,
            PreparedPackage,
        };
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tmp path")
            .as_nanos();
        let root = safe_temp_dir().join(format!("fol_lower_pkg_exports_{stamp}"));
        let json_root = root.join("json");
        fs::create_dir_all(json_root.join("src/fmt")).expect("should create package source dirs");
        fs::write(
            json_root.join("main.fol"),
            "fun[] main(): int = { return src::answer; };\n",
        )
        .expect("should write package entry");
        fs::write(
            json_root.join("src/lib.fol"),
            "var[exp] answer: int = 42;\n",
        )
        .expect("should write exported root source");
        fs::write(
            json_root.join("src/fmt/render.fol"),
            "var[exp] label: str = \"fmt\";\n",
        )
        .expect("should write exported fmt source");

        let mut stream = FileStream::from_folder(json_root.to_str().expect("utf8 temp path"))
            .expect("should open folder fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering folder fixture should parse");
        let prepared = PreparedPackage::with_controls(
            fol_package::PackageIdentity {
                source_kind: fol_package::PackageSourceKind::Entry,
                canonical_root: json_root.to_string_lossy().into_owned(),
                display_name: "json".to_string(),
            },
            PackageMetadata {
                name: "json".to_string(),
                version: "1.0.0".to_string(),
                kind: None,
                description: None,
                license: None,
                dependencies: Vec::new(),
            },
            PackageBuildDefinition {
                mode: PackageBuildMode::ModernOnly,
            },
            vec![
                PreparedExportMount {
                    source_namespace: "json::src".to_string(),
                    mounted_namespace_suffix: None,
                },
                PreparedExportMount {
                    source_namespace: "json::src::fmt".to_string(),
                    mounted_namespace_suffix: Some("fmt".to_string()),
                },
            ],
            None,
            None,
            syntax,
        );
        let resolved = fol_resolver::resolve_prepared_workspace(prepared)
            .expect("Lowering folder fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering folder fixture should typecheck");

        let lowered = LoweringSession::new(typed)
            .lower_workspace()
            .expect("Lowering should retain pkg export mounts");

        let json_package = lowered
            .packages()
            .find(|package| package.identity.display_name == "json")
            .expect("lowered workspace should retain the prepared package");
        assert_eq!(json_package.exports.len(), 2);
        assert!(json_package.exports.iter().any(|mount| {
            mount.source_namespace == "json::src" && mount.mounted_namespace_suffix.is_none()
        }));
        assert!(json_package.exports.iter().any(|mount| {
            mount.source_namespace == "json::src::fmt"
                && mount.mounted_namespace_suffix.as_deref() == Some("fmt")
        }));
    }

    #[test]
    fn lowering_session_marks_entry_package_main_routines_as_entry_candidates() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tmp path")
            .as_nanos();
        let root = safe_temp_dir().join(format!("fol_lower_entry_candidates_{stamp}"));
        let app_dir = root.join("app");
        let shared_dir = root.join("shared");
        fs::create_dir_all(&app_dir).expect("should create app dir");
        fs::create_dir_all(&shared_dir).expect("should create shared dir");
        fs::write(
            app_dir.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nfun[] main(): int = { return helper(); };\nfun[] helper(): int = { return 1; };\n",
        )
        .expect("should write app entry");
        fs::write(
            shared_dir.join("lib.fol"),
            "fun[exp] main(): int = { return 7; };\nfun[exp] helper(): int = { return 0; };\n",
        )
        .expect("should write shared library");

        let mut stream = FileStream::from_folder(app_dir.to_str().expect("utf8 temp path"))
            .expect("should open folder fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering folder fixture should parse");
        let resolved =
            resolve_package_workspace(syntax).expect("Lowering folder fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering folder fixture should typecheck");

        let lowered = LoweringSession::new(typed)
            .lower_workspace()
            .expect("Lowering should record entry candidates");

        assert_eq!(lowered.entry_candidates().len(), 1);
        assert_eq!(lowered.entry_candidates()[0].name, "main");
        assert_eq!(
            lowered.entry_candidates()[0].package_identity,
            *lowered.entry_identity()
        );
    }

    #[test]
    fn lowering_session_dedupes_packages_mounted_multiple_times() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tmp path")
            .as_nanos();
        let root = safe_temp_dir().join(format!("fol_lower_duplicate_mounts_{stamp}"));
        let app_dir = root.join("app");
        let shared_dir = root.join("shared");
        fs::create_dir_all(&app_dir).expect("should create app dir");
        fs::create_dir_all(&shared_dir).expect("should create shared dir");
        fs::write(
            app_dir.join("main.fol"),
            "use alpha: loc = {\"../shared\"};\nuse beta: loc = {\"../shared\"};\nfun[] main(): int = { return answer; };\n",
        )
        .expect("should write app entry");
        fs::write(shared_dir.join("lib.fol"), "var[exp] answer: int = 9;\n")
            .expect("should write shared library");

        let mut stream = FileStream::from_folder(app_dir.to_str().expect("utf8 temp path"))
            .expect("should open folder fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering folder fixture should parse");
        let resolved =
            resolve_package_workspace(syntax).expect("Lowering folder fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering folder fixture should typecheck");

        let lowered = LoweringSession::new(typed)
            .lower_workspace()
            .expect("Lowering should dedupe repeated imported packages");

        assert_eq!(lowered.package_count(), 2);
        assert_eq!(
            lowered
                .packages()
                .filter(|package| package.identity.display_name == "shared")
                .count(),
            1
        );
    }

    #[test]
    fn lowering_session_keeps_loc_std_and_pkg_packages_in_one_workspace() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tmp path")
            .as_nanos();
        let root = safe_temp_dir().join(format!("fol_lower_all_package_kinds_{stamp}"));
        let app_dir = root.join("app");
        let shared_dir = root.join("shared");
        let store_root = root.join("store");
        // The bundled standard library reaches source imports as a
        // store-materialized `std` package (the frontend fetch step projects
        // the internal `standard` dependency into the package store).
        let std_store_root = store_root.join("std");
        let json_root = store_root.join("json");

        fs::create_dir_all(&app_dir).expect("should create app dir");
        fs::create_dir_all(&shared_dir).expect("should create shared dir");
        fs::create_dir_all(std_store_root.join("src")).expect("should create std store dirs");
        fs::create_dir_all(json_root.join("src")).expect("should create pkg json dirs");

        fs::write(
            app_dir.join("main.fol"),
            "use shared: loc = {\"../shared\"};\nuse std: pkg = {\"std\"};\nuse json: pkg = {\"json\"};\nfun[] main(): int = { return shared::answer; };\n",
        )
        .expect("should write app entry");
        fs::write(shared_dir.join("lib.fol"), "var[exp] answer: int = 1;\n")
            .expect("should write local import");
        fs::write(
            std_store_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"std\", version = \"1.0.0\" });\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("should write std package build definition");
        fs::write(
            std_store_root.join("src/lib.fol"),
            "var[exp] answer: int = 2;\n",
        )
        .expect("should write std import");
        fs::write(
            json_root.join("build.fol"),
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"json\", version = \"1.0.0\" });\n",
                "    return;\n",
                "};\n",
            ),
        )
        .expect("should write package build definition");
        fs::write(json_root.join("src/lib.fol"), "var[exp] answer: int = 3;\n")
            .expect("should write package source");

        let mut stream = FileStream::from_folder(app_dir.to_str().expect("utf8 temp path"))
            .expect("should open folder fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("Lowering folder fixture should parse");
        let resolved = resolve_package_workspace_with_config(
            syntax,
            ResolverConfig {
                std_root: None,
                package_store_root: Some(store_root.to_string_lossy().into_owned()),
            },
        )
        .expect("Lowering folder fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("Lowering folder fixture should typecheck");

        let lowered = LoweringSession::new(typed)
            .lower_workspace()
            .expect("Lowering should keep all package kinds coherent");

        assert_eq!(lowered.package_count(), 4);
        for expected in ["app", "shared", "std", "json"] {
            let package = lowered
                .packages()
                .find(|package| package.identity.display_name == expected)
                .unwrap_or_else(|| panic!("{expected} package should be present"));
            assert!(
                !package.source_units.is_empty(),
                "{expected} package should retain at least one lowered source unit"
            );
        }
    }

    #[test]
    fn lowering_session_excludes_build_source_units_from_runtime_outputs() {
        use std::fs;
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be monotonic enough for tmp path")
            .as_nanos();
        let root = safe_temp_dir().join(format!("fol_lower_build_units_{stamp}"));
        fs::create_dir_all(root.join("src")).expect("should create temp source dir");
        fs::write(root.join("build.fol"), "`build`\n").expect("should write build file");
        fs::write(
            root.join("src/main.fol"),
            "fun[] main(): int = { return 1 };\n",
        )
        .expect("should write runtime source");

        let mut stream = FileStream::from_folder(root.to_str().expect("utf8 temp path"))
            .expect("should open folder fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("lowering fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("lowering fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("lowering fixture should typecheck");

        let lowered = LoweringSession::new(typed)
            .lower_workspace()
            .expect("lowering fixture should lower");

        assert_eq!(lowered.entry_package().source_units.len(), 1);
        assert!(lowered.entry_package().source_units[0]
            .path
            .ends_with("src/main.fol"));
        assert!(lowered.source_map().entries().iter().all(|entry| entry
            .origin
            .file
            .as_deref()
            .is_none_or(|file| !file.ends_with("build.fol"))));

        fs::remove_dir_all(root).ok();
    }
}

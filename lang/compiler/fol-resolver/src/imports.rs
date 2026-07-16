use crate::model::ScopeKind;
use crate::{
    ImportId, PackageSourceKind, ResolvedProgram, ResolverError, ResolverErrorKind,
    ResolverSession, ScopeId,
};
use fol_parser::ast::FolType;
use std::path::Path;

pub fn resolve_import_targets(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
) -> Result<(), Vec<ResolverError>> {
    let mut errors = Vec::new();
    let import_ids = program
        .imports
        .iter_with_ids()
        .map(|(import_id, _)| import_id)
        .collect::<Vec<_>>();

    for import_id in import_ids {
        if let Err(error) = resolve_import_target_with_session(session, program, import_id) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn resolve_import_target(
    program: &mut ResolvedProgram,
    import_id: ImportId,
) -> Result<(), ResolverError> {
    let Some(import) = program.import(import_id).cloned() else {
        return Ok(());
    };

    match &import.path_type {
        FolType::Location { .. } => {
            let target_scope = resolve_location_target_in_loaded_set(program, &import)?;
            if let Some(import_slot) = program.imports.get_mut(import_id) {
                import_slot.target_scope = Some(target_scope);
            }
            Ok(())
        }
        _ => {
            let origin = program
                .symbol(import.alias_symbol)
                .and_then(|symbol| symbol.origin.clone());
            let message = format!(
                "resolver does not support '{}' imports yet",
                import_kind_label(&import.path_type)
            );
            match origin {
                Some(origin) => Err(ResolverError::with_origin(
                    ResolverErrorKind::Unsupported,
                    message,
                    origin,
                )),
                None => Err(ResolverError::new(ResolverErrorKind::Unsupported, message)),
            }
        }
    }
}

pub(crate) fn resolve_import_target_with_session(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    import_id: ImportId,
) -> Result<(), ResolverError> {
    let Some(import) = program.import(import_id).cloned() else {
        return Ok(());
    };

    match &import.path_type {
        FolType::Location { .. } => {
            let target_scope = resolve_location_target_from_disk(session, program, &import)
                .map_err(|error| import_error_from(program, import.alias_symbol, error))?;
            if let Some(import_slot) = program.imports.get_mut(import_id) {
                import_slot.target_scope = Some(target_scope);
            }
            Ok(())
        }
        FolType::Package { .. } => {
            let target_scope = resolve_package_target_from_store(session, program, &import)
                .map_err(|error| import_error_from(program, import.alias_symbol, error))?;
            if let Some(import_slot) = program.imports.get_mut(import_id) {
                import_slot.target_scope = Some(target_scope);
            }
            Ok(())
        }
        _ => resolve_import_target(program, import_id),
    }
}

fn resolve_location_target_from_disk(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    import: &crate::ResolvedImport,
) -> Result<ScopeId, ResolverError> {
    let source_unit = program
        .source_unit(import.source_unit)
        .expect("import source unit should exist while resolving imports");
    let source_path = Path::new(&source_unit.path);
    let source_dir = source_path.parent().unwrap_or_else(|| Path::new("."));
    let target_path = fol_package::resolve_directory_target(source_dir, &import.import_target);
    if let Some(scope_id) = intra_package_namespace_scope(program, import, &target_path) {
        return Ok(scope_id);
    }
    let loaded =
        session.load_package_from_directory(target_path.as_path(), PackageSourceKind::Local)?;
    program.mount_loaded_package(&loaded)
}

/// A loc target that points back inside the importing package is already
/// loaded as one of that package's namespaces; loading it from disk again
/// would duplicate its source units. Resolve to the existing namespace scope.
fn intra_package_namespace_scope(
    program: &ResolvedProgram,
    import: &crate::ResolvedImport,
    target_path: &Path,
) -> Option<ScopeId> {
    let canonical_target = target_path.canonicalize().ok()?;
    let importing_package = program.source_unit(import.source_unit)?.package.clone();

    let namespace = program
        .ordinary_source_units()
        .filter(|unit| unit.id != import.source_unit && unit.package == importing_package)
        .find_map(|unit| {
            let unit_dir = Path::new(&unit.path).parent()?.canonicalize().ok()?;
            (unit_dir == canonical_target).then(|| unit.namespace.clone())
        })?;

    if namespace == program.package_name() {
        Some(program.program_scope)
    } else {
        program.namespace_scope(&namespace)
    }
}

fn resolve_package_target_from_store(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
    import: &crate::ResolvedImport,
) -> Result<ScopeId, ResolverError> {
    let store_root = session.config().package_store_root.clone().ok_or_else(|| {
        ResolverError::new(
            ResolverErrorKind::InvalidInput,
            format!(
                "resolver pkg import '{}' requires an explicit package store root",
                import.import_target
            ),
        )
    })?;
    let loaded =
        session.load_package_from_store_target(Path::new(&store_root), &import.import_target)?;
    program.mount_loaded_package(&loaded)
}

fn resolve_location_target_in_loaded_set(
    program: &ResolvedProgram,
    import: &crate::ResolvedImport,
) -> Result<ScopeId, ResolverError> {
    let source_unit = program
        .source_unit(import.source_unit)
        .expect("import source unit should exist while resolving imports");
    let package = &source_unit.package;
    let joined = import.import_target.clone();
    let mut candidate_names = std::collections::BTreeSet::new();

    if !joined.is_empty() {
        candidate_names.insert(joined.clone());
        candidate_names.insert(format!("{package}::{joined}"));
    }

    let mut candidate_scopes = candidate_names
        .into_iter()
        .filter_map(|namespace| namespace_scope_if_present(program, &namespace))
        .collect::<Vec<_>>();
    candidate_scopes.sort_unstable();
    candidate_scopes.dedup();

    match candidate_scopes.as_slice() {
        [scope_id] => Ok(*scope_id),
        [] => Err(import_error(
            program,
            import.alias_symbol,
            ResolverErrorKind::UnresolvedName,
            format!("could not resolve local import target '{}'", joined),
        )),
        _ => Err(import_error(
            program,
            import.alias_symbol,
            ResolverErrorKind::AmbiguousReference,
            format!(
                "local import target '{}' is ambiguous; candidates: {}",
                joined,
                candidate_scopes
                    .iter()
                    .map(|scope_id| describe_import_target(program, *scope_id))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
        )),
    }
}

fn namespace_scope_if_present(program: &ResolvedProgram, namespace: &str) -> Option<ScopeId> {
    if namespace == program.package_name() {
        Some(program.program_scope)
    } else {
        program.namespace_scope(namespace)
    }
}

fn import_error(
    program: &ResolvedProgram,
    alias_symbol: crate::SymbolId,
    kind: ResolverErrorKind,
    message: String,
) -> ResolverError {
    match program
        .symbol(alias_symbol)
        .and_then(|symbol| symbol.origin.clone())
    {
        Some(origin) => ResolverError::with_origin(kind, message, origin),
        None => ResolverError::new(kind, message),
    }
}

fn import_error_from(
    program: &ResolvedProgram,
    alias_symbol: crate::SymbolId,
    error: ResolverError,
) -> ResolverError {
    if error.origin().is_some() {
        error
    } else {
        import_error(
            program,
            alias_symbol,
            error.kind(),
            error.message().to_string(),
        )
    }
}

fn import_kind_label(path_type: &FolType) -> &'static str {
    match path_type {
        FolType::Package { .. } => "pkg",
        FolType::Location { .. } => "loc",
        FolType::Module { .. } => "mod",
        _ => "unknown",
    }
}

fn describe_import_target(program: &ResolvedProgram, scope_id: ScopeId) -> String {
    match program.scope(scope_id).map(|scope| &scope.kind) {
        Some(ScopeKind::ProgramRoot { package }) => format!("package '{package}'"),
        Some(ScopeKind::NamespaceRoot { namespace }) => format!("namespace '{namespace}'"),
        Some(ScopeKind::SourceUnitRoot { path }) => format!("source unit '{path}'"),
        Some(other) => format!("scope {other:?}"),
        None => "unknown scope".to_string(),
    }
}

use crate::{
    plan_namespace_layouts, BackendConfig, BackendError, BackendErrorKind, BackendResult,
    BackendSession,
};
use fol_lower::LoweredSourceSymbol;
use fol_parser::ast::SyntaxOrigin;
use fol_resolver::PackageIdentity;
use fol_resolver::SourceUnitId;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendEmittedSourceMapEntry {
    pub emitted_path: String,
    pub line: usize,
    pub column: usize,
    pub symbol: LoweredSourceSymbol,
    pub origin: SyntaxOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendEmittedSourceMap {
    entries: Vec<BackendEmittedSourceMapEntry>,
}

impl BackendEmittedSourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &[BackendEmittedSourceMapEntry] {
        &self.entries
    }

    pub fn push(&mut self, entry: BackendEmittedSourceMapEntry) {
        self.entries.push(entry);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendTraceKind {
    Session,
    Layout,
    Emission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendTraceRecord {
    pub kind: BackendTraceKind,
    pub emitted_path: Option<String>,
    pub package_identity: Option<PackageIdentity>,
    pub symbol: Option<LoweredSourceSymbol>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackendTrace {
    records: Vec<BackendTraceRecord>,
}

impl BackendTrace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn records(&self) -> &[BackendTraceRecord] {
        &self.records
    }

    pub fn push(&mut self, record: BackendTraceRecord) {
        self.records.push(record);
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

pub fn build_emitted_source_map(
    session: &BackendSession,
) -> BackendResult<BackendEmittedSourceMap> {
    let source_unit_paths = source_unit_output_paths(session)?;
    let mut map = BackendEmittedSourceMap::new();
    for entry in session.workspace().source_map().entries() {
        let source_unit_id = source_unit_for_symbol(session, entry.symbol)?;
        let emitted_path = source_unit_paths.get(&source_unit_id).ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!("missing emitted path for source unit {:?}", source_unit_id),
            )
        })?;
        map.push(BackendEmittedSourceMapEntry {
            emitted_path: emitted_path.clone(),
            line: 1,
            column: 1,
            symbol: entry.symbol,
            origin: entry.origin.clone(),
        });
    }
    Ok(map)
}

pub fn build_backend_trace(
    session: &BackendSession,
    config: &BackendConfig,
) -> BackendResult<BackendTrace> {
    let source_unit_paths = source_unit_output_paths(session)?;
    let mut trace = BackendTrace::new();
    trace.push(BackendTraceRecord {
        kind: BackendTraceKind::Session,
        emitted_path: None,
        package_identity: Some(session.entry_identity().clone()),
        symbol: None,
        detail: format!(
            "target={} profile={} fol_model={} runtime_tier={} runtime_module={}",
            config.machine_target.display_name(),
            config.build_profile.as_str(),
            config.fol_model.as_str(),
            config.runtime_tier().as_str(),
            config.runtime_tier().runtime_module_path()
        ),
    });
    for package in session.workspace().packages() {
        for source_unit in &package.source_units {
            let emitted_path = source_unit_paths.get(&source_unit.source_unit_id).cloned();
            trace.push(BackendTraceRecord {
                kind: BackendTraceKind::Emission,
                emitted_path,
                package_identity: Some(package.identity.clone()),
                symbol: None,
                detail: format!("namespace {}", source_unit.namespace),
            });
        }
    }
    Ok(trace)
}

fn source_unit_output_paths(
    session: &BackendSession,
) -> BackendResult<BTreeMap<SourceUnitId, String>> {
    let mut output_paths = BTreeMap::new();
    for plan in plan_namespace_layouts(session) {
        let emitted_path = format!(
            "src/packages/{}/{}",
            crate::mangle_package_module_name(&plan.package_identity),
            plan.relative_file
        );
        for source_unit_id in &plan.source_unit_ids {
            output_paths.insert(*source_unit_id, emitted_path.clone());
        }
    }
    Ok(output_paths)
}

fn source_unit_for_symbol(
    session: &BackendSession,
    symbol: LoweredSourceSymbol,
) -> BackendResult<SourceUnitId> {
    for package in session.workspace().packages() {
        match symbol {
            LoweredSourceSymbol::Global(global_id) => {
                if let Some(global) = package.global_decls.get(&global_id) {
                    return Ok(global.source_unit_id);
                }
            }
            LoweredSourceSymbol::Routine(routine_id) => {
                if let Some(routine) = package.routine_decls.get(&routine_id) {
                    return routine.source_unit_id.ok_or_else(|| {
                        BackendError::new(
                            BackendErrorKind::InvalidInput,
                            format!("routine {:?} is missing a source unit", routine_id),
                        )
                    });
                }
            }
            LoweredSourceSymbol::Type(type_id) => {
                if let Some(type_decl) = package
                    .type_decls
                    .values()
                    .find(|type_decl| type_decl.runtime_type == type_id)
                {
                    return Ok(type_decl.source_unit_id);
                }
            }
            LoweredSourceSymbol::Package(_) => {}
        }
    }

    Err(BackendError::new(
        BackendErrorKind::InvalidInput,
        format!("could not map lowered symbol {:?} to a source unit", symbol),
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        build_backend_trace, build_emitted_source_map, BackendEmittedSourceMap,
        BackendEmittedSourceMapEntry, BackendTrace, BackendTraceKind, BackendTraceRecord,
    };
    use crate::{BackendConfig, BackendFolModel};
    use crate::testing::{package_identity, sample_lowered_workspace};
    use fol_lower::{LoweredGlobalId, LoweredSourceSymbol};
    use fol_parser::ast::SyntaxOrigin;
    use fol_resolver::PackageSourceKind;

    #[test]
    fn backend_trace_and_source_map_capture_backend_owned_metadata() {
        let mut source_map = BackendEmittedSourceMap::new();
        let mut trace = BackendTrace::new();

        source_map.push(BackendEmittedSourceMapEntry {
            emitted_path: "src/app.rs".to_string(),
            line: 4,
            column: 8,
            symbol: LoweredSourceSymbol::Global(LoweredGlobalId(0)),
            origin: SyntaxOrigin {
                file: Some("app/main.fol".to_string()),
                line: 2,
                column: 3,
                length: 5,
            },
        });
        trace.push(BackendTraceRecord {
            kind: BackendTraceKind::Layout,
            emitted_path: Some("src/app.rs".to_string()),
            package_identity: Some(package_identity(
                "app",
                PackageSourceKind::Entry,
                "/workspace/app",
            )),
            symbol: Some(LoweredSourceSymbol::Global(LoweredGlobalId(0))),
            detail: "planned app root module".to_string(),
        });

        assert_eq!(source_map.entries().len(), 1);
        assert_eq!(trace.records().len(), 1);
        assert_eq!(trace.records()[0].kind, BackendTraceKind::Layout);
        assert_eq!(source_map.entries()[0].origin.line, 2);
    }

    #[test]
    fn backend_traceability_maps_lowered_origins_into_emitted_module_paths() {
        let session = crate::BackendSession::new(sample_lowered_workspace());

        let source_map = build_emitted_source_map(&session).expect("source map");
        let trace = build_backend_trace(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Core,
                ..BackendConfig::default()
            },
        )
        .expect("trace");

        assert!(!source_map.entries().is_empty());
        assert!(source_map
            .entries()
            .iter()
            .all(|entry| entry.emitted_path.starts_with("src/packages/")));
        assert!(source_map
            .entries()
            .iter()
            .any(|entry| entry.emitted_path.ends_with("root.rs")));
        assert!(!trace.records().is_empty());
        assert_eq!(trace.records()[0].kind, BackendTraceKind::Session);
        assert!(trace.records()[0].detail.contains("fol_model=core"));
        assert!(trace.records()[0].detail.contains("runtime_tier=core"));
        assert!(trace.records()[0]
            .detail
            .contains("runtime_module=fol_runtime::core"));
        assert!(trace
            .records()
            .iter()
            .skip(1)
            .all(|record| matches!(record.kind, BackendTraceKind::Emission)));
    }

    #[test]
    fn backend_trace_reports_public_memo_tier_with_internal_memo_runtime_module() {
        let session = crate::BackendSession::new(sample_lowered_workspace());
        let trace = build_backend_trace(
            &session,
            &BackendConfig {
                fol_model: BackendFolModel::Memo,
                ..BackendConfig::default()
            },
        )
        .expect("trace");

        assert!(trace.records()[0].detail.contains("fol_model=memo"));
        assert!(trace.records()[0].detail.contains("runtime_tier=memo"));
        assert!(trace.records()[0]
            .detail
            .contains("runtime_module=fol_runtime::memo"));
    }
}

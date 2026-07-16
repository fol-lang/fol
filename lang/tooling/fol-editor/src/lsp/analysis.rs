use crate::{
    diagnostic_to_lsp, materialize_analysis_overlay, EditorDocument, EditorError, EditorErrorKind,
    EditorResult, EditorWorkspaceMapping,
};
use fol_diagnostics::Diagnostic;
use fol_diagnostics::ToDiagnostic;
use fol_package::{
    canonical_directory_root, parse_directory_package_syntax, PackageIdentity, PackageSourceKind,
    PreparedPackage,
};
use fol_parser::ast::AstParser;
use fol_resolver::{Resolver, ResolverConfig};
use fol_stream::{FileStream, Source, SourceType};
use fol_typecheck::{TypecheckCapabilityModel, TypecheckConfig, Typechecker};
use std::path::Path;
use std::sync::Arc;

use super::semantic::SemanticSnapshot;

#[derive(Debug, Clone)]
pub(crate) struct CachedSemanticSnapshot {
    pub(crate) document_version: i32,
    pub(crate) snapshot: Arc<SemanticSnapshot>,
}

#[derive(Debug, Clone)]
pub(crate) struct CachedDiagnosticSnapshot {
    pub(crate) document_version: i32,
    pub(crate) diagnostics: Vec<crate::LspDiagnostic>,
}

#[cfg(test)]
thread_local! {
    // Analysis is synchronous on the calling thread and every test drives its
    // own server, so thread-local counters keep parallel tests isolated.
    static ANALYZE_DOCUMENT_SEMANTICS_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ANALYZE_DOCUMENT_DIAGNOSTICS_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static MATERIALIZE_ANALYSIS_OVERLAY_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PARSE_DIRECTORY_DIAGNOSTICS_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static LOAD_DIRECTORY_PACKAGE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static RESOLVE_WORKSPACE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static TYPECHECK_WORKSPACE_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_analyze_document_semantics_call_count() {
    ANALYZE_DOCUMENT_SEMANTICS_CALLS.with(|cell| cell.set(0));
}

#[cfg(test)]
pub(crate) fn analyze_document_semantics_call_count() -> usize {
    ANALYZE_DOCUMENT_SEMANTICS_CALLS.with(|cell| cell.get())
}

#[cfg(test)]
pub(crate) fn reset_analyze_document_diagnostics_call_count() {
    ANALYZE_DOCUMENT_DIAGNOSTICS_CALLS.with(|cell| cell.set(0));
}

#[cfg(test)]
pub(crate) fn analyze_document_diagnostics_call_count() -> usize {
    ANALYZE_DOCUMENT_DIAGNOSTICS_CALLS.with(|cell| cell.get())
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AnalysisStageCounts {
    pub materialize_overlay: usize,
    pub parse_directory_diagnostics: usize,
    pub load_directory_package: usize,
    pub resolve_workspace: usize,
    pub typecheck_workspace: usize,
}

#[cfg(test)]
pub(crate) fn reset_analysis_stage_counts() {
    MATERIALIZE_ANALYSIS_OVERLAY_CALLS.with(|cell| cell.set(0));
    PARSE_DIRECTORY_DIAGNOSTICS_CALLS.with(|cell| cell.set(0));
    LOAD_DIRECTORY_PACKAGE_CALLS.with(|cell| cell.set(0));
    RESOLVE_WORKSPACE_CALLS.with(|cell| cell.set(0));
    TYPECHECK_WORKSPACE_CALLS.with(|cell| cell.set(0));
}

#[cfg(test)]
pub(crate) fn analysis_stage_counts() -> AnalysisStageCounts {
    AnalysisStageCounts {
        materialize_overlay: MATERIALIZE_ANALYSIS_OVERLAY_CALLS.with(|cell| cell.get()),
        parse_directory_diagnostics: PARSE_DIRECTORY_DIAGNOSTICS_CALLS.with(|cell| cell.get()),
        load_directory_package: LOAD_DIRECTORY_PACKAGE_CALLS.with(|cell| cell.get()),
        resolve_workspace: RESOLVE_WORKSPACE_CALLS.with(|cell| cell.get()),
        typecheck_workspace: TYPECHECK_WORKSPACE_CALLS.with(|cell| cell.get()),
    }
}

pub(super) fn analyze_document_semantics(
    document: &EditorDocument,
    mapping: &EditorWorkspaceMapping,
) -> EditorResult<SemanticSnapshot> {
    #[cfg(test)]
    ANALYZE_DOCUMENT_SEMANTICS_CALLS.with(|cell| cell.set(cell.get() + 1));

    let is_build_document = mapping
        .document_path
        .file_name()
        .and_then(|name| name.to_str())
        == Some("build.fol");
    if mapping.fol_model_scope_unresolved && !is_build_document {
        // A source file outside every artifact in a mixed-model package (or
        // inside invalid/overlapping scopes) has no truthful typecheck model.
        // Analyze the open buffer alone instead of silently checking unrelated
        // sibling artifacts under the standalone Std default.
        let diagnostics = parse_single_file_diagnostics(&mapping.document_path, &document.text)?;
        return Ok(SemanticSnapshot {
            source_analysis_root: mapping.analysis_root.clone(),
            analyzed_analysis_root: mapping.analysis_root.clone(),
            analyzed_path: Some(mapping.document_path.clone()),
            analyzed_package_root: mapping.package_root.clone(),
            source_document_path: mapping.document_path.clone(),
            source_package_root: mapping.package_root.clone(),
            active_fol_model: None,
            active_internal_standard_aliases: mapping.active_internal_standard_aliases.clone(),
            fol_model_scope_unresolved: true,
            compiler_diagnostics: diagnostics.clone(),
            diagnostics: diagnostics.iter().map(diagnostic_to_lsp).collect(),
            resolved_workspace: None,
            typed_workspace: None,
        });
    }

    #[cfg(test)]
    MATERIALIZE_ANALYSIS_OVERLAY_CALLS.with(|cell| cell.set(cell.get() + 1));
    let overlay = materialize_analysis_overlay(mapping, document)?;
    if let Some(package_root) = overlay.package_root() {
        let source_scope = overlay.artifact_source_scope().unwrap_or(package_root);
        let parser_diags = parse_directory_diagnostics(source_scope)?
            .into_iter()
            .collect::<Vec<_>>();
        let parser_lsp_diags = parser_diags
            .iter()
            .filter(|diagnostic| diagnostic_targets_path(diagnostic, overlay.document_path()))
            .map(|diagnostic| diagnostic_to_lsp(&diagnostic))
            .collect::<Vec<_>>();
        if !parser_lsp_diags.is_empty() {
            return Ok(SemanticSnapshot {
                source_analysis_root: mapping.analysis_root.clone(),
                analyzed_analysis_root: overlay.analysis_root().to_path_buf(),
                analyzed_path: Some(overlay.document_path().to_path_buf()),
                analyzed_package_root: overlay.package_root().map(Path::to_path_buf),
                source_document_path: mapping.document_path.clone(),
                source_package_root: mapping.package_root.clone(),
                active_fol_model: mapping.active_fol_model,
                active_internal_standard_aliases: mapping.active_internal_standard_aliases.clone(),
                fol_model_scope_unresolved: mapping.fol_model_scope_unresolved,
                compiler_diagnostics: parser_diags,
                diagnostics: parser_lsp_diags,
                resolved_workspace: None,
                typed_workspace: None,
            });
        }
        if !parser_diags.is_empty() {
            // Some parser errors anchored at synthetic EOF do not carry a file
            // path. Confirm those against the open buffer alone before
            // publishing them; otherwise a neighbor/build-file error could be
            // misattributed to this document's URI.
            let current_parser_diags =
                parse_single_file_diagnostics(overlay.document_path(), &document.text)?;
            if !current_parser_diags.is_empty() {
                return Ok(SemanticSnapshot {
                    source_analysis_root: mapping.analysis_root.clone(),
                    analyzed_analysis_root: overlay.analysis_root().to_path_buf(),
                    analyzed_path: Some(overlay.document_path().to_path_buf()),
                    analyzed_package_root: overlay.package_root().map(Path::to_path_buf),
                    source_document_path: mapping.document_path.clone(),
                    source_package_root: mapping.package_root.clone(),
                    active_fol_model: mapping.active_fol_model,
                    active_internal_standard_aliases: mapping
                        .active_internal_standard_aliases
                        .clone(),
                    fol_model_scope_unresolved: mapping.fol_model_scope_unresolved,
                    diagnostics: current_parser_diags.iter().map(diagnostic_to_lsp).collect(),
                    compiler_diagnostics: current_parser_diags,
                    resolved_workspace: None,
                    typed_workspace: None,
                });
            }
        }

        #[cfg(test)]
        LOAD_DIRECTORY_PACKAGE_CALLS.with(|cell| cell.set(cell.get() + 1));
        let prepared = match prepare_analysis_entry_package(
            source_scope,
            mapping.package_root.as_deref().unwrap_or(package_root),
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                let diagnostic = error.to_diagnostic();
                let lsp_diags = parser_diags
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic_targets_path(diagnostic, overlay.document_path())
                    })
                    .map(diagnostic_to_lsp)
                    .chain(
                        diagnostic_targets_path(&diagnostic, overlay.document_path())
                            .then(|| diagnostic_to_lsp(&diagnostic)),
                    )
                    .collect();
                return Ok(SemanticSnapshot {
                    source_analysis_root: mapping.analysis_root.clone(),
                    analyzed_analysis_root: overlay.analysis_root().to_path_buf(),
                    analyzed_path: Some(overlay.document_path().to_path_buf()),
                    analyzed_package_root: overlay.package_root().map(Path::to_path_buf),
                    source_document_path: mapping.document_path.clone(),
                    source_package_root: mapping.package_root.clone(),
                    active_fol_model: mapping.active_fol_model,
                    active_internal_standard_aliases: mapping
                        .active_internal_standard_aliases
                        .clone(),
                    fol_model_scope_unresolved: mapping.fol_model_scope_unresolved,
                    compiler_diagnostics: vec![diagnostic.clone()],
                    diagnostics: lsp_diags,
                    resolved_workspace: None,
                    typed_workspace: None,
                });
            }
        };

        let package_store_root = package_root.join(".fol/pkg");
        let mut resolver = Resolver::new();
        #[cfg(test)]
        RESOLVE_WORKSPACE_CALLS.with(|cell| cell.set(cell.get() + 1));
        let resolved = match resolver.resolve_prepared_workspace_with_config(
            prepared,
            ResolverConfig {
                std_root: None,
                package_store_root: package_store_root
                    .is_dir()
                    .then(|| package_store_root.to_string_lossy().to_string()),
            },
        ) {
            Ok(resolved) => resolved,
            Err(errors) => {
                let diagnostics = errors
                    .iter()
                    .map(|error| error.to_diagnostic())
                    .collect::<Vec<_>>();
                return Ok(SemanticSnapshot {
                    source_analysis_root: mapping.analysis_root.clone(),
                    analyzed_analysis_root: overlay.analysis_root().to_path_buf(),
                    analyzed_path: Some(overlay.document_path().to_path_buf()),
                    analyzed_package_root: overlay.package_root().map(Path::to_path_buf),
                    source_document_path: mapping.document_path.clone(),
                    source_package_root: mapping.package_root.clone(),
                    active_fol_model: mapping.active_fol_model,
                    active_internal_standard_aliases: mapping
                        .active_internal_standard_aliases
                        .clone(),
                    fol_model_scope_unresolved: mapping.fol_model_scope_unresolved,
                    compiler_diagnostics: diagnostics.clone(),
                    diagnostics: diagnostics
                        .iter()
                        .filter(|diagnostic| {
                            diagnostic_targets_path(diagnostic, overlay.document_path())
                        })
                        .map(diagnostic_to_lsp)
                        .collect(),
                    resolved_workspace: None,
                    typed_workspace: None,
                });
            }
        };

        let mut typechecker = Typechecker::with_config(TypecheckConfig {
            capability_model: if mapping.fol_model_scope_unresolved {
                TypecheckCapabilityModel::Core
            } else {
                mapping.active_fol_model.unwrap_or_default()
            },
        });
        #[cfg(test)]
        TYPECHECK_WORKSPACE_CALLS.with(|cell| cell.set(cell.get() + 1));
        match typechecker.check_resolved_workspace(resolved.clone()) {
            Ok(typed_workspace) => Ok(SemanticSnapshot {
                source_analysis_root: mapping.analysis_root.clone(),
                analyzed_analysis_root: overlay.analysis_root().to_path_buf(),
                analyzed_path: Some(overlay.document_path().to_path_buf()),
                analyzed_package_root: overlay.package_root().map(Path::to_path_buf),
                source_document_path: mapping.document_path.clone(),
                source_package_root: mapping.package_root.clone(),
                active_fol_model: mapping.active_fol_model,
                active_internal_standard_aliases: mapping.active_internal_standard_aliases.clone(),
                fol_model_scope_unresolved: mapping.fol_model_scope_unresolved,
                compiler_diagnostics: Vec::new(),
                diagnostics: Vec::new(),
                resolved_workspace: Some(resolved),
                typed_workspace: Some(typed_workspace),
            }),
            Err(errors) => {
                let diagnostics = errors
                    .iter()
                    .map(|error| error.to_diagnostic())
                    .collect::<Vec<_>>();
                Ok(SemanticSnapshot {
                    source_analysis_root: mapping.analysis_root.clone(),
                    analyzed_analysis_root: overlay.analysis_root().to_path_buf(),
                    analyzed_path: Some(overlay.document_path().to_path_buf()),
                    analyzed_package_root: overlay.package_root().map(Path::to_path_buf),
                    source_document_path: mapping.document_path.clone(),
                    source_package_root: mapping.package_root.clone(),
                    active_fol_model: mapping.active_fol_model,
                    active_internal_standard_aliases: mapping
                        .active_internal_standard_aliases
                        .clone(),
                    fol_model_scope_unresolved: mapping.fol_model_scope_unresolved,
                    compiler_diagnostics: diagnostics.clone(),
                    diagnostics: diagnostics
                        .iter()
                        .filter(|diagnostic| {
                            diagnostic_targets_path(diagnostic, overlay.document_path())
                        })
                        .map(diagnostic_to_lsp)
                        .collect(),
                    resolved_workspace: Some(resolved),
                    typed_workspace: None,
                })
            }
        }
    } else {
        let diagnostics = parse_single_file_diagnostics(&mapping.document_path, &document.text)?;
        Ok(SemanticSnapshot {
            source_analysis_root: mapping.analysis_root.clone(),
            analyzed_analysis_root: mapping.analysis_root.clone(),
            analyzed_path: Some(mapping.document_path.clone()),
            analyzed_package_root: mapping.package_root.clone(),
            source_document_path: mapping.document_path.clone(),
            source_package_root: mapping.package_root.clone(),
            active_fol_model: mapping.active_fol_model,
            active_internal_standard_aliases: mapping.active_internal_standard_aliases.clone(),
            fol_model_scope_unresolved: mapping.fol_model_scope_unresolved,
            compiler_diagnostics: diagnostics.clone(),
            diagnostics: diagnostics
                .into_iter()
                .filter(|diagnostic| diagnostic_targets_path(diagnostic, &mapping.document_path))
                .map(|diagnostic| diagnostic_to_lsp(&diagnostic))
                .collect(),
            resolved_workspace: None,
            typed_workspace: None,
        })
    }
}

fn prepare_analysis_entry_package(
    source_scope: &Path,
    formal_package_root: &Path,
) -> Result<PreparedPackage, fol_package::PackageError> {
    let canonical_root = canonical_directory_root(formal_package_root, PackageSourceKind::Entry)?;
    let display_name = canonical_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("root")
        .to_string();
    let syntax =
        parse_directory_package_syntax(source_scope, &display_name, PackageSourceKind::Entry)?;
    Ok(PreparedPackage::new(
        PackageIdentity {
            source_kind: PackageSourceKind::Entry,
            canonical_root: canonical_root.to_string_lossy().to_string(),
            display_name,
        },
        syntax,
    ))
}

pub(super) fn note_diagnostic_snapshot_build() {
    #[cfg(test)]
    ANALYZE_DOCUMENT_DIAGNOSTICS_CALLS.with(|cell| cell.set(cell.get() + 1));
}

pub(super) fn diagnostic_targets_path(diagnostic: &Diagnostic, path: &Path) -> bool {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_text = canonical.to_string_lossy();
    diagnostic
        .primary_location()
        .and_then(|location| location.file.as_ref())
        .map(|file| file == &path_text)
        .or_else(|| {
            diagnostic
                .labels
                .first()
                .and_then(|label| label.location.file.as_ref())
                .map(|file| file == &path_text)
        })
        .unwrap_or(false)
}

/// Parse a single file for diagnostics when no package root is available.
///
/// Uses an in-memory source/stream so no temp files are written to disk.
pub(super) fn parse_single_file_diagnostics(
    path: &Path,
    text: &str,
) -> EditorResult<Vec<Diagnostic>> {
    let path_str = path.to_string_lossy().to_string();
    let package_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("main");
    let source = Source {
        call: path_str.clone(),
        path: path_str,
        data: text.to_string(),
        namespace: String::new(),
        package: package_name.to_string(),
    };
    let mut stream = FileStream::from_preloaded(vec![source]).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to create in-memory stream for '{}': {error}",
                path.display()
            ),
        )
    })?;
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    match parser.parse_package(&mut lexer) {
        Ok(_) => Ok(Vec::new()),
        Err(diagnostics) => Ok(diagnostics),
    }
}

pub(super) fn parse_directory_diagnostics(root: &Path) -> EditorResult<Vec<Diagnostic>> {
    #[cfg(test)]
    PARSE_DIRECTORY_DIAGNOSTICS_CALLS.with(|cell| cell.set(cell.get() + 1));
    let root_str = root.to_str().ok_or_else(|| {
        EditorError::new(
            EditorErrorKind::InvalidDocumentPath,
            format!("analysis root '{}' is not valid UTF-8", root.display()),
        )
    })?;
    let display_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("root");
    let sources =
        Source::init_with_package(root_str, SourceType::Folder, display_name).map_err(|error| {
            EditorError::new(
                EditorErrorKind::Internal,
                format!(
                    "failed to initialize analysis sources from '{}': {error}",
                    root.display()
                ),
            )
        })?;
    let mut stream = FileStream::from_sources(sources).map_err(|error| {
        EditorError::new(
            EditorErrorKind::Internal,
            format!(
                "failed to read analysis sources from '{}': {error}",
                root.display()
            ),
        )
    })?;
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();

    match parser.parse_package(&mut lexer) {
        Ok(_) => Ok(Vec::new()),
        Err(diagnostics) => Ok(diagnostics),
    }
}

pub(super) fn syntax_at_position(
    program: &fol_resolver::ResolvedProgram,
    path: &Path,
    position: crate::LspPosition,
) -> Option<fol_parser::ast::SyntaxNodeId> {
    let path_text = path.to_string_lossy();
    let mut best: Option<(fol_parser::ast::SyntaxNodeId, usize)> = None;
    for index in 0..program.syntax_index().len() {
        let syntax_id = fol_parser::ast::SyntaxNodeId(index);
        let Some(origin) = program.syntax_index().origin(syntax_id) else {
            continue;
        };
        let Some(file) = &origin.file else {
            continue;
        };
        if file != &path_text {
            continue;
        }
        let start_line = origin.line.saturating_sub(1) as u32;
        let start_character = origin.column.saturating_sub(1) as u32;
        let end_character = start_character + origin.length.max(1) as u32;
        let contains = position.line == start_line
            && position.character >= start_character
            && position.character <= end_character;
        if contains {
            match best {
                Some((_, current_len)) if current_len <= origin.length => {}
                _ => best = Some((syntax_id, origin.length)),
            }
        }
    }
    best.map(|(syntax_id, _)| syntax_id)
}

pub(super) fn nearest_scope_before_position(
    program: &fol_resolver::ResolvedProgram,
    path: &Path,
    position: crate::LspPosition,
) -> Option<fol_resolver::ScopeId> {
    let path_text = path.to_string_lossy();
    let mut best: Option<((u32, u32), fol_resolver::ScopeId)> = None;
    for index in 0..program.syntax_index().len() {
        let syntax_id = fol_parser::ast::SyntaxNodeId(index);
        let Some(scope_id) = program.scope_for_syntax(syntax_id) else {
            continue;
        };
        let Some(origin) = program.syntax_index().origin(syntax_id) else {
            continue;
        };
        let Some(file) = &origin.file else {
            continue;
        };
        if file != &path_text {
            continue;
        }
        let start = (
            origin.line.saturating_sub(1) as u32,
            origin.column.saturating_sub(1) as u32,
        );
        let cursor = (position.line, position.character);
        if start > cursor {
            continue;
        }
        match best {
            Some((best_start, _)) if best_start >= start => {}
            _ => best = Some((start, scope_id)),
        }
    }
    best.map(|(_, scope_id)| scope_id)
}

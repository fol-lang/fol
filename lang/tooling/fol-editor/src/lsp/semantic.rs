use crate::{
    diagnostic_to_lsp, location_to_range, EditorDocument, EditorDocumentUri, EditorError,
    EditorErrorKind, EditorResult, LspDiagnostic, LspLocation, LspPosition, LspRange, LspTextEdit,
    LspWorkspaceEdit,
};
use fol_intrinsics::IntrinsicSurface;
use fol_parser::ast::{AstNode, ParsedSourceUnitKind, SyntaxNodeId};
use fol_typecheck::{
    editor_builtin_type_names, editor_container_type_names, editor_implemented_intrinsics,
    editor_intrinsic_available_in_model, editor_model_capability,
    editor_processor_keyword_available_in_model, editor_processor_keyword_infos,
    editor_shell_type_names, editor_structured_type_infos, editor_type_family_available_in_model,
    EditorProcessorKeywordContext, EditorTypeFamily, TypecheckCapabilityModel,
};
use std::path::Path;
use std::path::PathBuf;

use super::completion_helpers::{
    completion_builtin_type_item, completion_intrinsic_item, completion_item_from_symbol,
    completion_namespace_item, completion_symbol_is_plain_top_level_candidate,
    completion_symbol_is_root_visible, current_routine_name, dedupe_completion_items,
    fallback_decl_name, fallback_items_from_package_dir, mark_fallback_completion_items,
    position_to_offset, render_checked_type, render_symbol_kind, symbol_kind_code,
    symbol_visibility_matches_namespace_root, CompletionContext, FALLBACK_ALIAS_PREFIXES,
    FALLBACK_ROUTINE_PREFIXES, FALLBACK_TYPE_PREFIXES,
};
use super::types::{
    EditorCompletionItem, LspDocumentHighlight, LspDocumentSymbol, LspFoldingRange, LspHover,
    LspInlayHint, LspParameterInformation, LspPrepareRenameResult, LspSelectionRange,
    LspSignatureHelp, LspSignatureInformation, LspWorkspaceSymbol,
};

const SEMANTIC_TOKEN_TYPES: &[&str] = &["namespace", "type", "function", "parameter", "variable"];

pub(super) fn semantic_token_types() -> &'static [&'static str] {
    SEMANTIC_TOKEN_TYPES
}

#[derive(Debug)]
pub(crate) struct SemanticSnapshot {
    pub(super) source_analysis_root: PathBuf,
    pub(super) analyzed_analysis_root: PathBuf,
    pub(super) analyzed_path: Option<PathBuf>,
    pub(super) analyzed_package_root: Option<PathBuf>,
    pub(super) source_document_path: PathBuf,
    pub(super) source_package_root: Option<PathBuf>,
    pub(super) active_fol_model: Option<TypecheckCapabilityModel>,
    pub(super) active_internal_standard_aliases: Vec<String>,
    pub(super) fol_model_scope_unresolved: bool,
    pub(super) compiler_diagnostics: Vec<fol_diagnostics::Diagnostic>,
    pub(super) diagnostics: Vec<LspDiagnostic>,
    pub(super) resolved_workspace: Option<fol_resolver::ResolvedWorkspace>,
    pub(super) typed_workspace: Option<fol_typecheck::TypedWorkspace>,
}

impl SemanticSnapshot {
    fn map_analyzed_file_to_source(&self, file: &str) -> String {
        let analyzed = Path::new(file);
        analyzed
            .strip_prefix(&self.analyzed_analysis_root)
            .ok()
            .map(|relative| self.source_analysis_root.join(relative))
            .unwrap_or_else(|| analyzed.to_path_buf())
            .to_string_lossy()
            .to_string()
    }

    fn map_analyzed_uri_to_source(&self, uri: &str) -> String {
        EditorDocumentUri::parse(uri)
            .and_then(|uri| uri.to_file_path())
            .map(|path| self.map_analyzed_file_to_source(&path.to_string_lossy()))
            .map(PathBuf::from)
            .and_then(EditorDocumentUri::from_file_path)
            .map(|uri| uri.as_str().to_string())
            .unwrap_or_else(|_| uri.to_string())
    }

    /// Group every compiler diagnostic by its real source URI.
    ///
    /// Compiler analysis runs in a disposable overlay, while LSP clients must
    /// receive diagnostics for the original workspace files. Keep the current
    /// document and imported dependency diagnostics in one compiler-backed
    /// result, remap the retained overlay paths back to source paths, and let
    /// the server publish one notification per URI.
    pub(super) fn diagnostics_by_source_uri(
        &self,
    ) -> std::collections::BTreeMap<String, Vec<LspDiagnostic>> {
        let mut grouped = std::collections::BTreeMap::<String, Vec<LspDiagnostic>>::new();

        for diagnostic in &self.compiler_diagnostics {
            let Some(file) = diagnostic
                .primary_location()
                .and_then(|location| location.file.as_deref())
                .or_else(|| {
                    diagnostic
                        .labels
                        .first()
                        .and_then(|label| label.location.file.as_deref())
                })
            else {
                continue;
            };
            let source_path = PathBuf::from(self.map_analyzed_file_to_source(file));
            let Ok(source_uri) = EditorDocumentUri::from_file_path(source_path) else {
                continue;
            };
            let mut converted = diagnostic_to_lsp(diagnostic);
            for related in &mut converted.related_information {
                related.location.uri = self.map_analyzed_uri_to_source(&related.location.uri);
            }
            grouped
                .entry(source_uri.as_str().to_string())
                .or_default()
                .push(converted);
        }

        // Diagnostics without a file are deliberately attributed to the open
        // document by the existing analysis path. Merge that current-document
        // view as a fallback, then exact-wire deduplication removes any normal
        // file-backed duplicates already inserted above.
        if let Ok(current_uri) =
            EditorDocumentUri::from_file_path(self.source_document_path.clone())
        {
            let current = grouped.entry(current_uri.as_str().to_string()).or_default();
            current.extend(self.diagnostics.iter().cloned().map(|mut diagnostic| {
                for related in &mut diagnostic.related_information {
                    related.location.uri = self.map_analyzed_uri_to_source(&related.location.uri);
                }
                diagnostic
            }));
        }

        for diagnostics in grouped.values_mut() {
            *diagnostics = crate::dedup_lsp_diagnostics(std::mem::take(diagnostics));
        }
        grouped
    }

    pub(super) fn signature_help(
        &self,
        document: &EditorDocument,
        position: LspPosition,
    ) -> Option<LspSignatureHelp> {
        let typed = self.typed_workspace.as_ref()?;
        let (package, program) = self.current_resolved_package()?;
        let typed_package = typed.package(&package.identity)?;
        let cursor_offset = offset_for_position(&document.text, position)?;
        let call_site = self.call_site_at_position(program, document, cursor_offset)?;
        let reference = reference_for_syntax_id(program, call_site.callee_syntax_id)?;
        let symbol_id = reference.resolved?;
        let declared_type = typed_package
            .program
            .typed_symbol(symbol_id)?
            .declared_type?;
        let signature = match typed_package.program.type_table().get(declared_type) {
            Some(fol_typecheck::CheckedType::Routine(signature)) => signature,
            _ => return None,
        };
        let parameters = signature
            .params
            .iter()
            .map(|type_id| render_checked_type(typed_package.program.type_table(), *type_id))
            .collect::<Vec<_>>();
        let label = render_signature_label(
            program
                .symbol(symbol_id)
                .map(|symbol| symbol.name.as_str())
                .unwrap_or(&call_site.display_name),
            &parameters,
            signature
                .return_type
                .map(|type_id| render_checked_type(typed_package.program.type_table(), type_id)),
            signature
                .error_type
                .map(|type_id| render_checked_type(typed_package.program.type_table(), type_id)),
        );
        let active_parameter = if parameters.is_empty() {
            None
        } else {
            Some(
                call_site
                    .active_parameter
                    .min(parameters.len().saturating_sub(1)) as u32,
            )
        };
        Some(LspSignatureHelp {
            signatures: vec![LspSignatureInformation {
                label,
                parameters: parameters
                    .into_iter()
                    .map(|label| LspParameterInformation { label })
                    .collect(),
            }],
            active_signature: Some(0),
            active_parameter,
        })
    }

    pub(super) fn code_actions(
        &self,
        uri: &str,
        range: LspRange,
        requested_diagnostics: &[LspDiagnostic],
    ) -> Vec<super::types::LspCodeAction> {
        let Some(analyzed_path) = self.analyzed_path.as_ref() else {
            return Vec::new();
        };
        let path_text = analyzed_path.to_string_lossy();
        let mut actions = self
            .compiler_diagnostics
            .iter()
            .filter_map(|diagnostic| {
                let location = diagnostic.primary_location()?;
                if location.file.as_deref()? != path_text {
                    return None;
                }
                let diagnostic_range = location_to_range(location);
                if !ranges_overlap(diagnostic_range, range) {
                    return None;
                }
                let lsp_diagnostic = diagnostic_to_lsp(diagnostic);
                if !requested_diagnostics.is_empty()
                    && !requested_diagnostics.iter().any(|requested| {
                        requested.code == lsp_diagnostic.code
                            && requested.range == lsp_diagnostic.range
                    })
                {
                    return None;
                }
                let suggestion = diagnostic.suggestions.iter().find(|suggestion| {
                    suggestion.replacement.is_some() && suggestion.location.is_some()
                })?;
                let suggestion_location = suggestion.location.as_ref()?;
                if suggestion_location.file.as_deref()? != path_text {
                    return None;
                }
                let mut changes = std::collections::BTreeMap::new();
                changes.insert(
                    uri.to_string(),
                    vec![LspTextEdit {
                        range: location_to_range(suggestion_location),
                        new_text: suggestion
                            .replacement
                            .clone()
                            .expect("replacement checked above"),
                    }],
                );
                Some(super::types::LspCodeAction {
                    title: suggestion.message.clone(),
                    kind: "quickfix".to_string(),
                    diagnostics: vec![lsp_diagnostic],
                    edit: LspWorkspaceEdit { changes },
                })
            })
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| left.title.cmp(&right.title));
        actions
    }

    pub(super) fn semantic_tokens_for_current_path(&self) -> Vec<u32> {
        let Some(program) = self.current_program() else {
            return Vec::new();
        };
        let Some(analyzed_path) = self.analyzed_path.as_ref() else {
            return Vec::new();
        };
        let path_text = analyzed_path.to_string_lossy();
        let mut entries = std::collections::BTreeSet::new();

        for symbol in program.all_symbols() {
            let Some(origin) = symbol.origin.as_ref() else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else {
                continue;
            };
            if file != &path_text {
                continue;
            }
            // Explicit parameters carry their own source-name origin and must
            // be tokenized at the declaration site. Only a receiver routine's
            // synthesized `self` still borrows the routine header origin; it
            // has no source token and would otherwise repaint the routine
            // name as a parameter.
            if matches!(symbol.kind, fol_resolver::SymbolKind::Parameter) && symbol.name == "self" {
                continue;
            }
            let Some(token_type) = semantic_token_type_for_symbol_kind(symbol.kind) else {
                continue;
            };
            let line = origin.line.saturating_sub(1) as u32;
            let start = origin.column.saturating_sub(1) as u32;
            let length = origin.length as u32;
            if length == 0 {
                continue;
            }
            entries.insert((line, start, length, token_type, 0_u32));
        }

        for reference in program.all_references() {
            let Some(symbol_id) = reference.resolved else {
                continue;
            };
            let Some(symbol) = program.symbol(symbol_id) else {
                continue;
            };
            let Some(token_type) = semantic_token_type_for_symbol_kind(symbol.kind) else {
                continue;
            };
            let Some(syntax_id) = reference.anchor() else {
                continue;
            };
            let Some(origin) = program.syntax_index().origin(syntax_id) else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else {
                continue;
            };
            if file != &path_text {
                continue;
            }
            let line = origin.line.saturating_sub(1) as u32;
            let start = origin.column.saturating_sub(1) as u32;
            let length = origin.length as u32;
            if length == 0 {
                continue;
            }
            entries.insert((line, start, length, token_type, 0_u32));
        }

        let mut data = Vec::with_capacity(entries.len() * 5);
        let mut previous_line = 0_u32;
        let mut previous_start = 0_u32;
        for (index, (line, start, length, token_type, modifiers)) in entries.into_iter().enumerate()
        {
            let delta_line = if index == 0 {
                line
            } else {
                line - previous_line
            };
            let delta_start = if index == 0 || delta_line != 0 {
                start
            } else {
                start - previous_start
            };
            data.extend([delta_line, delta_start, length, token_type, modifiers]);
            previous_line = line;
            previous_start = start;
        }
        data
    }

    pub(super) fn workspace_symbols(&self, query: &str) -> Vec<LspWorkspaceSymbol> {
        let Some(resolved) = self.resolved_workspace.as_ref() else {
            return Vec::new();
        };
        let query = query.trim().to_ascii_lowercase();
        let mut symbols = Vec::new();
        let bundled_standard_roots = self.bundled_standard_analysis_roots();

        for package in resolved.packages() {
            let package_root = Path::new(&package.prepared.identity.canonical_root);
            let is_workspace_package = matches!(
                package.identity.source_kind,
                fol_resolver::PackageSourceKind::Entry | fol_resolver::PackageSourceKind::Local
            );
            let is_declared_bundled_standard = bundled_standard_roots
                .iter()
                .any(|root| package_root == root);
            if !is_workspace_package && !is_declared_bundled_standard {
                continue;
            }

            for symbol in package.program.all_symbols() {
                if !completion_symbol_is_root_visible(&package.program, symbol) {
                    continue;
                }
                if !matches!(
                    symbol.kind,
                    fol_resolver::SymbolKind::Routine
                        | fol_resolver::SymbolKind::Type
                        | fol_resolver::SymbolKind::Alias
                        | fol_resolver::SymbolKind::Definition
                        | fol_resolver::SymbolKind::ValueBinding
                ) {
                    continue;
                }
                let Some(origin) = symbol.origin.as_ref() else {
                    continue;
                };
                let Some(file) = origin.file.as_ref() else {
                    continue;
                };
                let Some(source_unit) = package.program.source_units.get(symbol.source_unit) else {
                    continue;
                };
                if source_unit.kind != ParsedSourceUnitKind::Ordinary {
                    continue;
                }

                let package_root_namespace = source_unit.namespace == package.identity.display_name
                    || source_unit.namespace == format!("{}::src", package.identity.display_name);
                let qualified_name = if package_root_namespace {
                    symbol.name.clone()
                } else {
                    format!("{}::{}", source_unit.namespace, symbol.name)
                };
                let container_namespace = if package_root_namespace {
                    package.identity.display_name.as_str()
                } else {
                    source_unit.namespace.as_str()
                };
                let container_name = Some(format!(
                    "{container_namespace} ({})",
                    package.identity.display_name
                ));
                if !query.is_empty() {
                    let package_name = package.identity.display_name.to_ascii_lowercase();
                    let namespace = source_unit.namespace.to_ascii_lowercase();
                    let qualified_name_lower = qualified_name.to_ascii_lowercase();
                    let symbol_name = symbol.name.to_ascii_lowercase();
                    if !symbol_name.contains(&query)
                        && !qualified_name_lower.contains(&query)
                        && !package_name.contains(&query)
                        && !namespace.contains(&query)
                    {
                        continue;
                    }
                }

                symbols.push(LspWorkspaceSymbol {
                    name: qualified_name,
                    kind: symbol_kind_code(symbol.kind),
                    location: LspLocation {
                        uri: format!("file://{}", self.map_analyzed_file_to_source(file)),
                        range: location_to_range(&fol_diagnostics::DiagnosticLocation {
                            file: Some(self.map_analyzed_file_to_source(file)),
                            line: origin.line,
                            column: origin.column,
                            length: Some(origin.length),
                        }),
                    },
                    container_name,
                });
            }
        }

        symbols.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then(left.container_name.cmp(&right.container_name))
                .then(left.location.uri.cmp(&right.location.uri))
                .then(
                    left.location
                        .range
                        .start
                        .line
                        .cmp(&right.location.range.start.line),
                )
                .then(
                    left.location
                        .range
                        .start
                        .character
                        .cmp(&right.location.range.start.character),
                )
        });
        symbols.dedup_by(|left, right| left == right);
        symbols
    }

    fn bundled_standard_analysis_roots(&self) -> Vec<PathBuf> {
        let Some(analyzed_package_root) = self.analyzed_package_root.as_ref() else {
            return Vec::new();
        };

        self.active_internal_standard_aliases
            .iter()
            .map(|alias| analyzed_package_root.join(".fol/pkg").join(alias))
            .map(|root| root.canonicalize().unwrap_or(root))
            .collect()
    }

    pub(super) fn completion_items(
        &self,
        document: &EditorDocument,
        position: LspPosition,
        context: CompletionContext,
    ) -> Vec<EditorCompletionItem> {
        if let Some(items) = self.build_document_completion_items(document, position) {
            return items;
        }
        if self.current_program().is_none() {
            return self.fallback_completion_items(document, position, context);
        }
        let pipe_stage = context == CompletionContext::PipeStage;
        match context {
            CompletionContext::Plain | CompletionContext::PipeStage => {}
            CompletionContext::TypePosition => {
                let mut items = self.type_surface_completion_items();
                items.extend(self.visible_named_type_completion_items());
                return dedupe_completion_items(items);
            }
            CompletionContext::ParameterOption {
                allow_borrow,
                allow_mutex,
            } => {
                return self.parameter_option_completion_items(allow_borrow, allow_mutex);
            }
            CompletionContext::PointerTypePosition { allow_shared } => {
                let mut items = self.type_surface_completion_items_for_target(true);
                items.extend(self.visible_named_type_completion_items());
                if allow_shared {
                    items.push(completion_keyword_item("shared"));
                }
                return dedupe_completion_items(items);
            }
            CompletionContext::ChannelElementTypePosition => {
                if !self.active_model().supports_processor() {
                    return Vec::new();
                }
                let mut items = self.type_surface_completion_items_for_target(true);
                items.extend(self.visible_named_type_completion_items());
                return dedupe_completion_items(items);
            }
            CompletionContext::NestedTypePosition { forbid_channel } => {
                let mut items = self.type_surface_completion_items_for_target(forbid_channel);
                items.extend(self.visible_named_type_completion_items());
                return dedupe_completion_items(items);
            }
            CompletionContext::OwnedTypePosition { forbid_channel } => {
                if !editor_model_capability(self.active_model()).heap {
                    return Vec::new();
                }
                let mut items = self.type_surface_completion_items_for_target(forbid_channel);
                items.extend(self.visible_named_type_completion_items());
                return dedupe_completion_items(items);
            }
            CompletionContext::BracketAccess { receiver } => {
                if position_is_inside_deferred_block(&document.text, position)
                    && fallback_visible_binding_kind(&document.text, position, &receiver)
                        == Some(FallbackBindingKind::Channel)
                {
                    return Vec::new();
                }
                if let Some(items) =
                    self.channel_endpoint_completion_items(document, &receiver, position)
                {
                    return items;
                }
            }
            CompletionContext::HeapBinding => return self.heap_binding_completion_items(),
            CompletionContext::QualifiedPath { qualifier } => {
                let items = self.qualified_completion_items(&qualifier);
                if items.is_empty() {
                    return self.fallback_qualified_completion_items(&qualifier);
                }
                return items;
            }
            CompletionContext::DotTrigger { receiver } => {
                if let Some(receiver) = receiver.as_deref() {
                    if position_is_inside_deferred_block(&document.text, position)
                        && fallback_visible_binding_kind(&document.text, position, receiver)
                            == Some(FallbackBindingKind::Mutex)
                    {
                        return Vec::new();
                    }
                    if let Some(items) = self.mutex_completion_items(document, receiver, position) {
                        return items;
                    }
                }
                return self.dot_intrinsic_fallback_completion_items();
            }
        }
        let mut items = self.local_completion_items(position);
        items.extend(self.current_package_top_level_completion_items());
        items.extend(self.import_alias_completion_items(position));
        if pipe_stage {
            items.extend(self.processor_pipe_stage_completion_items());
        } else {
            items.extend(self.keyword_completion_items());
        }
        dedupe_completion_items(items)
    }

    /// Build-file (`build.fol`) completions come straight from the
    /// compiler-owned build semantic registry instead of an editor-owned
    /// copy of the surface.
    fn build_document_completion_items(
        &self,
        document: &EditorDocument,
        position: LspPosition,
    ) -> Option<Vec<EditorCompletionItem>> {
        use fol_package::BuildSemanticTypeFamily;

        if self
            .source_document_path
            .file_name()
            .and_then(|name| name.to_str())
            != Some("build.fol")
        {
            return None;
        }
        let line = document.text.lines().nth(position.line as usize)?;
        let cursor = (position.character as usize).min(line.len());
        let prefix = &line[..cursor];

        // Field completions inside a git dependency declaration.
        if prefix.contains("add_dep(") && prefix.contains("\"git") {
            let items = fol_package::canonical_build_context_config_shapes()
                .into_iter()
                .find(|shape| shape.name == "BuildDependencyConfig")
                .map(|shape| {
                    shape
                        .fields
                        .into_iter()
                        .map(|field| EditorCompletionItem {
                            label: field.name,
                            kind: 5,
                            detail: Some(if field.required {
                                "required dependency field".to_string()
                            } else {
                                "optional dependency field".to_string()
                            }),
                            insert_text: None,
                        })
                        .collect::<Vec<_>>()
                })?;
            return Some(items);
        }

        // Method completions after `<receiver>.`.
        let before_dot = prefix.trim_end().strip_suffix('.')?;
        let receiver = before_dot
            .rsplit(|c: char| !(c.is_alphanumeric() || c == '_'))
            .next()
            .filter(|name| !name.is_empty())?;
        let family = classify_build_receiver(&document.text, receiver)?;

        let mut items = Vec::new();
        let push_family = |target: BuildSemanticTypeFamily,
                           items: &mut Vec<EditorCompletionItem>| {
            for signature in fol_package::canonical_build_context_method_signatures()
                .into_iter()
                .chain(fol_package::canonical_graph_method_signatures())
                .chain(fol_package::canonical_handle_method_signatures())
            {
                if signature.receiver == target {
                    items.push(EditorCompletionItem {
                        label: signature.name,
                        kind: 2,
                        detail: Some("build surface".to_string()),
                        insert_text: None,
                    });
                }
            }
        };
        push_family(family, &mut items);
        Some(dedupe_completion_items(items))
    }

    fn fallback_completion_items(
        &self,
        document: &EditorDocument,
        position: LspPosition,
        context: CompletionContext,
    ) -> Vec<EditorCompletionItem> {
        let pipe_stage = context == CompletionContext::PipeStage;
        let bracket_receiver = match &context {
            CompletionContext::BracketAccess { receiver } => Some(receiver.clone()),
            _ => None,
        };
        match context {
            CompletionContext::DotTrigger { receiver } => receiver
                .as_deref()
                .and_then(|receiver| {
                    self.fallback_mutex_completion_items(document, position, receiver)
                })
                .unwrap_or_else(|| self.dot_intrinsic_fallback_completion_items()),
            CompletionContext::QualifiedPath { qualifier } => {
                self.fallback_qualified_completion_items(&qualifier)
            }
            CompletionContext::TypePosition => {
                let mut items = self.type_surface_completion_items();
                items.extend(self.fallback_local_named_type_items(document));
                items.extend(self.fallback_imported_named_type_items(document));
                dedupe_completion_items(items)
            }
            CompletionContext::ParameterOption {
                allow_borrow,
                allow_mutex,
            } => self.parameter_option_completion_items(allow_borrow, allow_mutex),
            CompletionContext::PointerTypePosition { allow_shared } => {
                let mut items = self.type_surface_completion_items_for_target(true);
                items.extend(self.fallback_local_named_type_items(document));
                items.extend(self.fallback_imported_named_type_items(document));
                if allow_shared {
                    items.push(completion_keyword_item("shared"));
                }
                dedupe_completion_items(items)
            }
            CompletionContext::ChannelElementTypePosition => {
                if !self.active_model().supports_processor() {
                    return Vec::new();
                }
                let mut items = self.type_surface_completion_items_for_target(true);
                items.extend(self.fallback_local_named_type_items(document));
                items.extend(self.fallback_imported_named_type_items(document));
                dedupe_completion_items(items)
            }
            CompletionContext::NestedTypePosition { forbid_channel } => {
                let mut items = self.type_surface_completion_items_for_target(forbid_channel);
                items.extend(self.fallback_local_named_type_items(document));
                items.extend(self.fallback_imported_named_type_items(document));
                dedupe_completion_items(items)
            }
            CompletionContext::OwnedTypePosition { forbid_channel } => {
                if !editor_model_capability(self.active_model()).heap {
                    return Vec::new();
                }
                let mut items = self.type_surface_completion_items_for_target(forbid_channel);
                items.extend(self.fallback_local_named_type_items(document));
                items.extend(self.fallback_imported_named_type_items(document));
                dedupe_completion_items(items)
            }
            CompletionContext::HeapBinding => self.heap_binding_completion_items(),
            CompletionContext::Plain
            | CompletionContext::PipeStage
            | CompletionContext::BracketAccess { .. } => {
                if let Some(receiver) = bracket_receiver.as_deref() {
                    if let Some(items) = self
                        .fallback_channel_endpoint_completion_items(document, position, receiver)
                    {
                        return items;
                    }
                }
                if position_to_offset(&document.text, position).is_none() {
                    if let Some(line) = document.text.lines().nth(position.line as usize) {
                        if line.contains("::") {
                            let aliases = self.fallback_import_alias_items(document);
                            if aliases.len() == 1 {
                                let items = self.fallback_imported_package_items(&aliases[0].label);
                                if !items.is_empty() {
                                    return dedupe_completion_items(items);
                                }
                            }
                        }
                    }
                }
                let mut items = self.fallback_local_scope_items(document, position);
                items.extend(self.fallback_current_package_top_level_items(document, position));
                items.extend(self.fallback_import_alias_items(document));
                if pipe_stage {
                    items.extend(self.processor_pipe_stage_completion_items());
                } else {
                    items.extend(self.keyword_completion_items());
                }
                dedupe_completion_items(items)
            }
        }
    }

    fn active_model(&self) -> TypecheckCapabilityModel {
        if self.fol_model_scope_unresolved {
            // Mixed packages without one honest artifact scope must not gain
            // the standalone editor's default hosted capabilities.
            TypecheckCapabilityModel::Core
        } else {
            self.active_fol_model.unwrap_or_default()
        }
    }

    fn parameter_option_completion_items(
        &self,
        allow_borrow: bool,
        allow_mutex: bool,
    ) -> Vec<EditorCompletionItem> {
        let mut items = Vec::new();
        if allow_borrow {
            items.push(completion_keyword_item("bor"));
        }
        if allow_mutex && self.active_model().supports_processor() {
            items.push(completion_keyword_item("mux"));
        }
        items
    }

    fn heap_binding_completion_items(&self) -> Vec<EditorCompletionItem> {
        editor_model_capability(self.active_model())
            .heap
            .then(|| completion_keyword_item("var"))
            .into_iter()
            .collect()
    }

    fn channel_endpoint_completion_items(
        &self,
        document: &EditorDocument,
        receiver: &str,
        position: LspPosition,
    ) -> Option<Vec<EditorCompletionItem>> {
        if !self.active_model().supports_processor() {
            return None;
        }
        let (package, _) = self.current_resolved_package()?;
        let (program, mut scope_id) = self.scope_at_position(position)?;
        let receiver_key = fol_types::canonical_identifier_key(receiver);
        let symbol_id = loop {
            let named = program.symbols_named_in_scope(scope_id, &receiver_key);
            if !named.is_empty() {
                break named
                    .into_iter()
                    .find(|symbol| {
                        matches!(
                            symbol.kind,
                            fol_resolver::SymbolKind::ValueBinding
                                | fol_resolver::SymbolKind::Parameter
                                | fol_resolver::SymbolKind::Capture
                        )
                    })?
                    .id;
            }
            scope_id = program.scope(scope_id)?.parent?;
        };
        let typed_package = self.typed_workspace.as_ref()?.package(&package.identity)?;
        let type_id = typed_package
            .program
            .typed_symbol(symbol_id)?
            .declared_type?;
        let endpoints: &[&str] = match typed_package.program.type_table().get(type_id)? {
            fol_typecheck::CheckedType::Channel { .. } => {
                if position_is_inside_deferred_block(&document.text, position) {
                    return Some(Vec::new());
                }
                if self.analyzed_path.as_deref().is_some_and(|path| {
                    resolved_channel_receive_precedes(
                        program,
                        symbol_id,
                        path,
                        &document.text,
                        position,
                    )
                }) {
                    &["rx"]
                } else {
                    &["tx", "rx"]
                }
            }
            fol_typecheck::CheckedType::ChannelSender { .. } => &["tx"],
            _ => return None,
        };
        Some(
            endpoints
                .iter()
                .map(|endpoint| completion_keyword_item(endpoint))
                .collect(),
        )
    }

    fn fallback_channel_endpoint_completion_items(
        &self,
        document: &EditorDocument,
        position: LspPosition,
        receiver: &str,
    ) -> Option<Vec<EditorCompletionItem>> {
        if !self.active_model().supports_processor() {
            return None;
        }
        if fallback_visible_binding_kind(&document.text, position, receiver)
            != Some(FallbackBindingKind::Channel)
        {
            return None;
        }
        if position_is_inside_deferred_block(&document.text, position) {
            return Some(Vec::new());
        }
        let endpoints: &[&str] =
            if direct_channel_receiver_precedes(&document.text, position, receiver) {
                &["rx"]
            } else {
                &["tx", "rx"]
            };
        Some(
            endpoints
                .iter()
                .copied()
                .map(completion_keyword_item)
                .collect(),
        )
    }

    fn mutex_completion_items(
        &self,
        document: &EditorDocument,
        receiver: &str,
        position: LspPosition,
    ) -> Option<Vec<EditorCompletionItem>> {
        if !self.active_model().supports_processor() {
            return None;
        }
        let (package, _) = self.current_resolved_package()?;
        let (program, mut scope_id) = self.scope_at_position(position)?;
        let receiver_key = fol_types::canonical_identifier_key(receiver);
        let symbol_id = loop {
            let named = program.symbols_named_in_scope(scope_id, &receiver_key);
            if !named.is_empty() {
                break named
                    .into_iter()
                    .find(|symbol| {
                        matches!(
                            symbol.kind,
                            fol_resolver::SymbolKind::ValueBinding
                                | fol_resolver::SymbolKind::Parameter
                                | fol_resolver::SymbolKind::Capture
                        )
                    })?
                    .id;
            }
            scope_id = program.scope(scope_id)?.parent?;
        };
        let typed_package = self.typed_workspace.as_ref()?.package(&package.identity)?;
        let symbol = typed_package.program.typed_symbol(symbol_id)?;
        if !symbol.is_mutex {
            return None;
        }
        if position_is_inside_deferred_block(&document.text, position) {
            return Some(Vec::new());
        }
        Some(mutex_method_completion_items())
    }

    fn fallback_mutex_completion_items(
        &self,
        document: &EditorDocument,
        position: LspPosition,
        receiver: &str,
    ) -> Option<Vec<EditorCompletionItem>> {
        if !self.active_model().supports_processor() {
            return None;
        }
        if fallback_visible_binding_kind(&document.text, position, receiver)
            != Some(FallbackBindingKind::Mutex)
        {
            return None;
        }
        if position_is_inside_deferred_block(&document.text, position) {
            return Some(Vec::new());
        }
        Some(mutex_method_completion_items())
    }

    fn type_surface_completion_items(&self) -> Vec<EditorCompletionItem> {
        let model = self.active_model();
        let mut items = editor_builtin_type_names()
            .iter()
            .filter(|name| editor_type_family_available_in_model(model, builtin_type_family(name)))
            .map(|name| completion_builtin_type_item(name))
            .collect::<Vec<_>>();
        items.extend(
            editor_container_type_names()
                .iter()
                .filter(|name| {
                    editor_type_family_available_in_model(model, container_type_family(name))
                })
                .map(|name| completion_builtin_type_item(name)),
        );
        items.extend(
            editor_shell_type_names()
                .iter()
                .filter(|name| {
                    editor_type_family_available_in_model(model, shell_type_family(name))
                })
                .map(|name| completion_builtin_type_item(name)),
        );
        items.extend(
            editor_structured_type_infos()
                .iter()
                .filter(|info| editor_type_family_available_in_model(model, info.family))
                .map(|info| completion_builtin_type_item(info.name)),
        );
        items
    }

    fn type_surface_completion_items_for_target(
        &self,
        forbid_channel: bool,
    ) -> Vec<EditorCompletionItem> {
        let mut items = self.type_surface_completion_items();
        if forbid_channel {
            items.retain(|item| item.label != "chn");
        }
        items
    }

    // COMPILER-BACKED: reads from resolved all_symbols
    fn visible_named_type_completion_items(&self) -> Vec<EditorCompletionItem> {
        let Some(program) = self.current_program() else {
            return Vec::new();
        };
        program
            .all_symbols()
            .filter(|symbol| {
                matches!(
                    symbol.kind,
                    fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias
                )
            })
            .filter(|symbol| completion_symbol_is_root_visible(program, symbol))
            .map(completion_item_from_symbol)
            .collect()
    }

    // COMPILER-BACKED: reads from resolver namespace/scope + child namespaces
    fn qualified_completion_items(&self, qualifier: &str) -> Vec<EditorCompletionItem> {
        let Some(program) = self.current_program() else {
            return Vec::new();
        };
        let qualifier_root = qualifier.split("::").next().unwrap_or(qualifier);
        let imported_root = program.all_symbols().any(|symbol| {
            symbol.kind == fol_resolver::SymbolKind::ImportAlias && symbol.name == qualifier_root
        });
        let mut items = Vec::new();

        if let Some(scope_id) = program.namespace_scope(qualifier) {
            items.extend(
                program
                    .symbols_in_scope(scope_id)
                    .into_iter()
                    .filter(|symbol| {
                        symbol_visibility_matches_namespace_root(symbol, imported_root)
                    })
                    .map(completion_item_from_symbol),
            );
        }

        for source_unit in program.source_units.iter() {
            if source_unit.namespace != qualifier {
                continue;
            }
            items.extend(
                program
                    .symbols_in_scope(source_unit.scope_id)
                    .into_iter()
                    .filter(|symbol| {
                        symbol_visibility_matches_namespace_root(symbol, imported_root)
                    })
                    .map(completion_item_from_symbol),
            );
        }

        let prefix = format!("{qualifier}::");
        let mut child_namespaces = std::collections::BTreeSet::new();
        for (_, scope) in program.scopes.iter_with_ids() {
            let fol_resolver::ScopeKind::NamespaceRoot { namespace } = &scope.kind else {
                continue;
            };
            let Some(remainder) = namespace.strip_prefix(&prefix) else {
                continue;
            };
            let child = remainder.split("::").next().unwrap_or("");
            if !child.is_empty() {
                child_namespaces.insert(child.to_string());
            }
        }
        items.extend(child_namespaces.into_iter().map(completion_namespace_item));

        dedupe_completion_items(items)
    }

    // COMPILER-BACKED: intrinsic registry is the canonical source
    fn dot_intrinsic_fallback_completion_items(&self) -> Vec<EditorCompletionItem> {
        let model = self.active_model();
        editor_implemented_intrinsics()
            .iter()
            .filter(|entry| entry.surface == IntrinsicSurface::DotRootCall)
            .filter(|entry| editor_intrinsic_available_in_model(model, **entry))
            .map(|entry| completion_intrinsic_item(entry.name))
            .collect()
    }

    // COMPILER-BACKED: the keyword set comes from the lexer's own keyword
    // tables, so completion stays in sync with what the language actually lexes.
    // Offered in plain (statement/expression) context, independent of whether
    // the buffer currently resolves, so keywords are available while typing.
    // Operator keywords (infix) and receiver-contextual keywords are omitted.
    fn keyword_completion_items(&self) -> Vec<EditorCompletionItem> {
        use fol_lexer::token::buildin::{
            CONTROL_KEYWORDS, DECLARATION_KEYWORDS, DIAGNOSTIC_KEYWORDS, LITERAL_KEYWORDS,
        };
        // LSP CompletionItemKind::Keyword == 14.
        let model = self.active_model();
        DECLARATION_KEYWORDS
            .iter()
            .chain(CONTROL_KEYWORDS.iter())
            .chain(LITERAL_KEYWORDS.iter())
            .chain(DIAGNOSTIC_KEYWORDS.iter())
            .filter(|keyword| {
                editor_processor_keyword_infos()
                    .iter()
                    .find(|info| info.name == **keyword)
                    .is_none_or(|info| {
                        info.context == EditorProcessorKeywordContext::Plain
                            && editor_processor_keyword_available_in_model(model, *info)
                    })
            })
            .map(|keyword| completion_keyword_item(keyword))
            .collect()
    }

    fn processor_pipe_stage_completion_items(&self) -> Vec<EditorCompletionItem> {
        let model = self.active_model();
        editor_processor_keyword_infos()
            .iter()
            .filter(|info| info.context == EditorProcessorKeywordContext::PipeStage)
            .filter(|info| editor_processor_keyword_available_in_model(model, **info))
            .map(|info| completion_keyword_item(info.name))
            .collect()
    }

    // COMPILER-BACKED: reads from resolver scope chain
    fn local_completion_items(&self, position: LspPosition) -> Vec<EditorCompletionItem> {
        let Some((program, scope_id)) = self.scope_at_position(position) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let mut cursor = Some(scope_id);
        while let Some(current_scope_id) = cursor {
            for symbol in program.symbols_in_scope(current_scope_id) {
                if !matches!(
                    symbol.kind,
                    fol_resolver::SymbolKind::ValueBinding
                        | fol_resolver::SymbolKind::LabelBinding
                        | fol_resolver::SymbolKind::DestructureBinding
                        | fol_resolver::SymbolKind::Parameter
                        | fol_resolver::SymbolKind::GenericParameter
                        | fol_resolver::SymbolKind::LoopBinder
                        | fol_resolver::SymbolKind::RollingBinder
                        | fol_resolver::SymbolKind::Capture
                ) {
                    continue;
                }
                items.push(completion_item_from_symbol(symbol));
            }
            cursor = program
                .scope(current_scope_id)
                .and_then(|scope| scope.parent);
        }
        items
    }

    // COMPILER-BACKED: reads from resolved program namespace/source-unit scopes
    fn current_package_top_level_completion_items(&self) -> Vec<EditorCompletionItem> {
        let Some(program) = self.current_program() else {
            return Vec::new();
        };
        let Some(namespace) = self.current_namespace() else {
            return Vec::new();
        };
        let mut items = Vec::new();
        if let Some(scope_id) = program.namespace_scope(namespace.as_str()) {
            items.extend(
                program
                    .symbols_in_scope(scope_id)
                    .into_iter()
                    .filter(|symbol| symbol.mounted_from.is_none())
                    .filter(|symbol| {
                        completion_symbol_is_plain_top_level_candidate(program, symbol)
                    })
                    .map(completion_item_from_symbol),
            );
        }
        for source_unit in program
            .source_units
            .iter()
            .filter(|unit| unit.namespace == namespace)
        {
            items.extend(
                program
                    .symbols_in_scope(source_unit.scope_id)
                    .into_iter()
                    .filter(|symbol| symbol.mounted_from.is_none())
                    .filter(|symbol| {
                        completion_symbol_is_plain_top_level_candidate(program, symbol)
                    })
                    .map(completion_item_from_symbol),
            );
        }
        items
    }

    // COMPILER-BACKED: reads from resolver scope chain
    fn import_alias_completion_items(&self, position: LspPosition) -> Vec<EditorCompletionItem> {
        let Some((program, scope_id)) = self.scope_at_position(position) else {
            return Vec::new();
        };
        let mut items = Vec::new();
        let mut cursor = Some(scope_id);
        while let Some(current_scope_id) = cursor {
            for symbol in program.symbols_in_scope(current_scope_id) {
                if symbol.kind != fol_resolver::SymbolKind::ImportAlias {
                    continue;
                }
                items.push(completion_item_from_symbol(symbol));
            }
            cursor = program
                .scope(current_scope_id)
                .and_then(|scope| scope.parent);
        }
        items
    }

    // FALLBACK: text-scans for `var ` bindings and `fun` parameters when
    // resolver data is absent or incomplete. Required for broken documents.
    fn fallback_local_scope_items(
        &self,
        document: &EditorDocument,
        position: LspPosition,
    ) -> Vec<EditorCompletionItem> {
        let offset = position_to_offset(&document.text, position).unwrap_or(document.text.len());
        let masked_before_cursor = crate::source_scan::mask_non_code(&document.text[..offset]);
        let before_cursor = masked_before_cursor.as_str();
        let mut items = self.fallback_import_alias_items(document);
        items.extend(self.fallback_current_package_top_level_items(document, position));

        if let Some(header) = before_cursor
            .rmatch_indices("fun")
            .next()
            .map(|(index, _)| &before_cursor[index..])
        {
            if let Some(open) = header.find('(') {
                if let Some(close) = header[open + 1..].find(')') {
                    for param in header[open + 1..open + 1 + close].split(',') {
                        let name = param.split(':').next().unwrap_or("").trim();
                        if !name.is_empty() {
                            items.push(EditorCompletionItem {
                                label: name.to_string(),
                                kind: 6,
                                detail: Some("parameter".to_string()),
                                insert_text: None,
                            });
                        }
                    }
                }
            }
        }

        for line in before_cursor.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("var ") {
                let name = rest
                    .split(|ch: char| ch == ':' || ch == '=' || ch.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    items.push(EditorCompletionItem {
                        label: name.to_string(),
                        kind: 6,
                        detail: Some("binding".to_string()),
                        insert_text: None,
                    });
                }
            }
        }

        items
    }

    // FALLBACK: text-matches current V1 declaration heads when resolver data is absent.
    // when resolver data is absent. Required for broken documents.
    fn fallback_current_package_top_level_items(
        &self,
        document: &EditorDocument,
        position: LspPosition,
    ) -> Vec<EditorCompletionItem> {
        let mut items = Vec::new();
        let current_routine = current_routine_name(&document.text, position);
        let masked = crate::source_scan::mask_non_code(&document.text);
        for line in masked.lines() {
            let trimmed = line.trim();
            if let Some(name) = fallback_decl_name(trimmed, FALLBACK_ROUTINE_PREFIXES) {
                if current_routine.as_deref() == Some(name.as_str()) {
                    continue;
                }
                items.push(EditorCompletionItem {
                    label: name,
                    kind: 3,
                    detail: Some("routine".to_string()),
                    insert_text: None,
                });
            } else if let Some(name) = fallback_decl_name(trimmed, FALLBACK_TYPE_PREFIXES) {
                items.push(EditorCompletionItem {
                    label: name,
                    kind: 22,
                    detail: Some("type".to_string()),
                    insert_text: None,
                });
            } else if let Some(name) = fallback_decl_name(trimmed, FALLBACK_ALIAS_PREFIXES) {
                items.push(EditorCompletionItem {
                    label: name,
                    kind: 22,
                    detail: Some("type alias".to_string()),
                    insert_text: None,
                });
            }
        }
        items
    }

    // FALLBACK: filters top-level text fallback for type/alias items only
    fn fallback_local_named_type_items(
        &self,
        document: &EditorDocument,
    ) -> Vec<EditorCompletionItem> {
        mark_fallback_completion_items(
            self.fallback_current_package_top_level_items(
                document,
                LspPosition {
                    line: u32::MAX,
                    character: u32::MAX,
                },
            )
            .into_iter()
            .filter(|item| {
                item.detail.as_deref() == Some("type")
                    || item.detail.as_deref() == Some("type alias")
            })
            .collect(),
        )
    }

    // FALLBACK: text-matches `use ` prefix to find import aliases
    fn fallback_import_alias_items(&self, document: &EditorDocument) -> Vec<EditorCompletionItem> {
        crate::source_scan::mask_non_code(&document.text)
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                let rest = trimmed.strip_prefix("use ")?;
                let alias = rest.split(':').next()?.trim();
                (!alias.is_empty()).then(|| EditorCompletionItem {
                    label: alias.to_string(),
                    kind: 9,
                    detail: Some("namespace".to_string()),
                    insert_text: None,
                })
            })
            .collect()
    }

    // FALLBACK: reads imported package files from disk + text-scans
    fn fallback_imported_named_type_items(
        &self,
        document: &EditorDocument,
    ) -> Vec<EditorCompletionItem> {
        let aliases = self.fallback_import_alias_items(document);
        let mut items = Vec::new();
        for alias in aliases {
            items.extend(
                self.fallback_imported_package_items(&alias.label)
                    .into_iter()
                    .filter(|item| {
                        item.detail.as_deref() == Some("type")
                            || item.detail.as_deref() == Some("type alias")
                    }),
            );
        }
        mark_fallback_completion_items(items)
    }

    // FALLBACK: combines local namespace + imported package fallbacks
    fn fallback_qualified_completion_items(&self, qualifier: &str) -> Vec<EditorCompletionItem> {
        let mut items = self.fallback_local_namespace_items(qualifier);
        items.extend(self.fallback_imported_package_items(qualifier));
        mark_fallback_completion_items(dedupe_completion_items(items))
    }

    // FALLBACK: reads imported package files from disk + text-scans declarations
    fn fallback_imported_package_items(&self, qualifier: &str) -> Vec<EditorCompletionItem> {
        let Some(package_root) = &self.source_package_root else {
            return Vec::new();
        };
        let text = std::fs::read_to_string(&self.source_document_path).unwrap_or_default();
        let mut parts = qualifier.split("::");
        let root_alias = parts.next().unwrap_or(qualifier);
        let namespace_suffix = parts.collect::<Vec<_>>().join("/");
        let masked = crate::source_scan::mask_non_code(&text);
        let rel_path = text
            .lines()
            .zip(masked.lines())
            .find_map(|(source_line, code_line)| {
                let code_rest = code_line.trim().strip_prefix("use ")?;
                let (code_alias, _) = code_rest.split_once(':')?;
                if code_alias.trim() != root_alias {
                    return None;
                }
                let source_rest = source_line.trim().strip_prefix("use ")?;
                let (_, source_rhs) = source_rest.split_once(':')?;
                Some(source_rhs.trim().to_string())
            });
        let Some(rhs) = rel_path else {
            return Vec::new();
        };
        let Some(start) = rhs.find('"') else {
            return Vec::new();
        };
        let tail = &rhs[start + 1..];
        let Some(end) = tail.find('"') else {
            return Vec::new();
        };
        let import_target = &tail[..end];
        let mut target = if rhs.starts_with("pkg") {
            let materialized = package_root.join(".fol/pkg").join(import_target);
            if materialized.is_dir() {
                materialized
            } else {
                declared_bundled_standard_root(package_root, import_target).unwrap_or(materialized)
            }
        } else {
            // `loc` targets resolve relative to the importing source file,
            // mirroring the compiler's package loader.
            self.source_document_path
                .parent()
                .unwrap_or(package_root)
                .join(import_target)
        };
        if !namespace_suffix.is_empty() {
            target = target.join(namespace_suffix);
        }
        let mut items = fallback_items_from_package_dir(&target);
        if let Ok(entries) = std::fs::read_dir(&target) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                        items.push(EditorCompletionItem {
                            label: name.to_string(),
                            kind: 9,
                            detail: Some("namespace".to_string()),
                            insert_text: None,
                        });
                    }
                }
            }
        }
        items
    }

    // FALLBACK: reads filesystem directories for namespace items
    fn fallback_local_namespace_items(&self, qualifier: &str) -> Vec<EditorCompletionItem> {
        let Some(package_root) = &self.source_package_root else {
            return Vec::new();
        };
        let namespace_dir = package_root.join("src").join(qualifier.replace("::", "/"));
        let mut items = fallback_items_from_package_dir(&namespace_dir);
        if let Ok(entries) = std::fs::read_dir(&namespace_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                        items.push(EditorCompletionItem {
                            label: name.to_string(),
                            kind: 9,
                            detail: Some("namespace".to_string()),
                            insert_text: None,
                        });
                    }
                }
            }
        }
        items
    }

    pub(super) fn current_program(&self) -> Option<&fol_resolver::ResolvedProgram> {
        let resolved = self.resolved_workspace.as_ref()?;
        let analyzed_path = self.analyzed_path.as_ref()?;
        let path_text = analyzed_path.to_string_lossy();
        resolved.packages().find_map(|package| {
            let program = &package.program;
            program
                .source_units
                .iter()
                .any(|unit| unit.path == path_text)
                .then_some(program)
        })
    }

    fn scope_at_position(
        &self,
        position: LspPosition,
    ) -> Option<(&fol_resolver::ResolvedProgram, fol_resolver::ScopeId)> {
        use super::analysis::{nearest_scope_before_position, syntax_at_position};
        let program = self.current_program()?;
        let analyzed_path = self.analyzed_path.as_ref()?;
        if let Some(syntax_id) = syntax_at_position(program, analyzed_path.as_path(), position) {
            if let Some(scope_id) = program.scope_for_syntax(syntax_id) {
                return Some((program, scope_id));
            }
        }
        if let Some(scope_id) =
            nearest_scope_before_position(program, analyzed_path.as_path(), position)
        {
            return Some((program, scope_id));
        }
        self.current_source_unit(program)
            .map(|unit| (program, unit.scope_id))
    }

    fn current_namespace(&self) -> Option<String> {
        let program = self.current_program()?;
        self.current_source_unit(program)
            .map(|unit| unit.namespace.clone())
    }

    fn current_resolved_package(
        &self,
    ) -> Option<(
        &fol_resolver::ResolvedPackage,
        &fol_resolver::ResolvedProgram,
    )> {
        let resolved = self.resolved_workspace.as_ref()?;
        let analyzed_path = self.analyzed_path.as_ref()?;
        let path_text = analyzed_path.to_string_lossy();
        resolved.packages().find_map(|package| {
            package
                .program
                .source_units
                .iter()
                .any(|unit| unit.path == path_text)
                .then_some((package, &package.program))
        })
    }

    fn call_site_at_position(
        &self,
        program: &fol_resolver::ResolvedProgram,
        document: &EditorDocument,
        cursor_offset: usize,
    ) -> Option<SignatureCallSite> {
        let source_unit = self.current_source_unit(program)?;
        let path = self.analyzed_path.as_ref()?;
        let text = document.text.as_str();
        let mut best: Option<SignatureCallSite> = None;
        for item in &program.syntax().source_units {
            if item.path != source_unit.path {
                continue;
            }
            for top_level in &item.items {
                visit_call_sites(
                    &top_level.node,
                    program,
                    path.as_path(),
                    text,
                    cursor_offset,
                    &mut best,
                );
            }
            break;
        }
        best
    }

    // COMPILER-BACKED: resolver reference lookup (no text fallback)
    pub(super) fn reference_at(
        &self,
        position: LspPosition,
    ) -> Option<fol_resolver::ResolvedReference> {
        let program = self.current_program()?;
        let analyzed_path = self.analyzed_path.as_ref()?;
        reference_at_position_in_program(program, analyzed_path.as_path(), position)
    }

    // COMPILER-BACKED: typecheck-recorded method-call target (no text fallback)
    pub(super) fn method_target_symbol_at(
        &self,
        position: LspPosition,
    ) -> Option<fol_resolver::SymbolId> {
        let (package, program) = self.current_resolved_package()?;
        let analyzed_path = self.analyzed_path.as_ref()?;
        let typed_package = self
            .typed_workspace
            .as_ref()
            .and_then(|typed| typed.package(&package.identity))?;
        let path_text = analyzed_path.to_string_lossy();
        let line = position.line as usize + 1;
        let column = position.character as usize + 1;

        let mut best: Option<(fol_resolver::SymbolId, usize)> = None;
        for (syntax_id, symbol_id) in typed_package.program.method_call_targets() {
            let Some(origin) = program.syntax_index().origin(syntax_id) else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else {
                continue;
            };
            if file != &path_text {
                continue;
            }
            if !origin_contains(origin, line, column) {
                continue;
            }
            match best {
                Some((_, best_len)) if best_len <= origin.length => {}
                _ => best = Some((symbol_id, origin.length.max(1))),
            }
        }
        best.map(|(symbol_id, _)| symbol_id)
    }

    fn borrow_is_active_at_position(
        &self,
        program: &fol_resolver::ResolvedProgram,
        typed: &fol_typecheck::TypedProgram,
        borrow: &fol_typecheck::model::ActiveBorrow,
        position: LspPosition,
        position_scope: Option<fol_resolver::ScopeId>,
    ) -> bool {
        let Some(analyzed_path) = self.analyzed_path.as_ref() else {
            return false;
        };
        let origin_is_at_or_before = |origin: &fol_parser::ast::SyntaxOrigin| {
            origin.file.as_deref() == analyzed_path.to_str()
                && (
                    origin.line.saturating_sub(1) as u32,
                    origin.column.saturating_sub(1) as u32,
                ) <= (position.line, position.character)
        };
        if !origin_is_at_or_before(&borrow.origin) {
            return false;
        }
        if typed
            .returned_borrow_origin(borrow.binding)
            .is_some_and(origin_is_at_or_before)
        {
            return false;
        }

        let Some(mut scope) = position_scope else {
            return false;
        };
        loop {
            if scope == borrow.scope {
                return true;
            }
            let Some(parent) = program.scope(scope).and_then(|scope| scope.parent) else {
                return false;
            };
            scope = parent;
        }
    }

    // COMPILER-BACKED: resolved symbol + typed type (no text fallback)
    pub(super) fn hover_for_symbol(
        &self,
        symbol_id: fol_resolver::SymbolId,
        position: LspPosition,
        position_scope: Option<fol_resolver::ScopeId>,
    ) -> Option<LspHover> {
        let (package, program) = self.current_resolved_package()?;
        let symbol = program.symbol(symbol_id)?;
        // Local bindings may not carry a declaration origin yet; hover still
        // has useful kind/name/type content without a highlight range.
        let origin = symbol.origin.as_ref();
        let typed_package = self
            .typed_workspace
            .as_ref()
            .and_then(|typed| typed.package(&package.identity));
        let type_summary = typed_package
            .and_then(|typed_package| typed_package.program.typed_symbol(symbol_id))
            .and_then(|typed_symbol| typed_symbol.declared_type)
            .map(|type_id| {
                let typed_package = typed_package
                    .expect("typed package should exist when declared type is available");
                render_checked_type(typed_package.program.type_table(), type_id)
            })
            .unwrap_or_else(|| "unknown".to_string());
        let ownership_summary = typed_package
            .and_then(|typed_package| {
                let file = self.analyzed_path.as_ref()?.to_str()?;
                typed_package.program.moved_binding_origin_at(
                    symbol_id,
                    file,
                    position.line as usize + 1,
                    position.character as usize + 1,
                )
            })
            .map(|origin| {
                format!(
                    " (moved; ownership transferred at {}:{})",
                    origin.line, origin.column
                )
            })
            .unwrap_or_default();
        let borrow_summary = typed_package
            .and_then(|typed_package| {
                typed_package
                    .program
                    .borrow_for_binding(symbol_id)
                    .map(|borrow| {
                        let owner = program
                            .symbol(borrow.owner)
                            .map(|symbol| symbol.name.as_str())
                            .unwrap_or("owner");
                        format!(
                            " ({} borrow of {owner})",
                            if borrow.mutable { "mutable" } else { "shared" }
                        )
                    })
                    .or_else(|| {
                        typed_package
                            .program
                            .borrows_for_owner(symbol_id)
                            .find(|borrow| {
                                self.borrow_is_active_at_position(
                                    program,
                                    &typed_package.program,
                                    borrow,
                                    position,
                                    position_scope,
                                )
                            })
                            .map(|borrow| {
                                let binding = program
                                    .symbol(borrow.binding)
                                    .map(|symbol| symbol.name.as_str())
                                    .unwrap_or("borrow");
                                format!(
                                    " (inaccessible while borrow {binding} is active in its lexical scope)"
                                )
                            })
                    })
            })
            .unwrap_or_default();
        let pointer_summary = typed_package
            .and_then(|typed_package| {
                let type_id = typed_package.program.typed_symbol(symbol_id)?.declared_type?;
                let (target, shared, borrowed) =
                    pointer_type_info(&typed_package.program, type_id)?;
                let target_name =
                    render_checked_type(typed_package.program.type_table(), target);
                let target_moves = fol_typecheck::exprs::bindings::ownership_moves_on_transfer(
                    &typed_package.program,
                    target,
                );
                Some(if borrowed {
                    if target_moves {
                        format!(
                            " (borrowed pointer; read-only dereference cannot transfer move-only {target_name})"
                        )
                    } else {
                        format!(
                            " (borrowed pointer; read-only dereference clones {target_name})"
                        )
                    }
                } else if shared {
                    if target_moves {
                        format!(
                            " (shared refcount pointer; read-only dereference cannot transfer move-only {target_name}; reference cycles leak until weak references exist)"
                        )
                    } else {
                        format!(
                            " (shared refcount pointer; read-only dereference clones {target_name}; reference cycles leak until weak references exist)"
                        )
                    }
                } else if target_moves {
                    format!(
                        " (unique pointer; dereference transfers move-only {target_name} and consumes the pointer; construction requires memo+)"
                    )
                } else {
                    format!(
                        " (unique pointer; dereference reads clone-safe {target_name} without consuming the pointer; construction requires memo+)"
                    )
                })
            })
            .unwrap_or_default();
        Some(LspHover {
            contents: format!(
                "{} {}: {}{}{}{}",
                render_symbol_kind(symbol.kind),
                symbol.name,
                type_summary,
                ownership_summary,
                borrow_summary,
                pointer_summary
            ),
            range: origin.map(|origin| {
                location_to_range(&fol_diagnostics::DiagnosticLocation {
                    file: origin
                        .file
                        .as_deref()
                        .map(|file| self.map_analyzed_file_to_source(file)),
                    line: origin.line,
                    column: origin.column,
                    length: Some(origin.length),
                })
            }),
        })
    }

    pub(super) fn hover_for_reference(
        &self,
        reference: &fol_resolver::ResolvedReference,
        position: LspPosition,
    ) -> Option<LspHover> {
        self.hover_for_symbol(reference.resolved?, position, Some(reference.scope))
    }

    // COMPILER-BACKED: the textual layer only locates the operand adjacent to
    // the pipe keyword; value and error types come from the typed symbol.
    pub(super) fn hover_for_processor_pipe_stage(
        &self,
        operand_position: LspPosition,
        stage: &str,
    ) -> Option<LspHover> {
        let reference = self.reference_at(operand_position)?;
        let symbol_id = reference.resolved?;
        let (package, _) = self.current_resolved_package()?;
        let typed_package = self.typed_workspace.as_ref()?.package(&package.identity)?;
        let type_id = typed_package
            .program
            .typed_symbol(symbol_id)?
            .declared_type?;
        let table = typed_package.program.type_table();

        let contents = match (stage, table.get(type_id)?) {
            (
                "await",
                fol_typecheck::CheckedType::Eventual {
                    value_type,
                    error_type,
                },
            ) => {
                let value = render_checked_type(table, *value_type);
                match error_type {
                    Some(error) => format!(
                        "| await: blocks for `{value}` and preserves recoverable error `{}`; handle it immediately with `||` or `check(...)`",
                        render_checked_type(table, *error)
                    ),
                    None => format!("| await: blocks for `{value}`"),
                }
            }
            ("async", fol_typecheck::CheckedType::Routine(signature)) => {
                let value = signature
                    .return_type
                    .map(|type_id| render_checked_type(table, type_id))
                    .unwrap_or_else(|| "non".to_string());
                match signature.error_type {
                    Some(error) => format!(
                        "| async: spawns an OS thread; yields an internal eventual of `{value}` with recoverable error `{}`; it must be awaited and handled before lexical fallthrough, break, return, or report, and every continuing branch must preserve or discharge the obligation",
                        render_checked_type(table, error)
                    ),
                    None => format!(
                        "| async: spawns an OS thread; yields an internal eventual of `{value}`"
                    ),
                }
            }
            _ => return None,
        };
        Some(LspHover {
            contents,
            range: None,
        })
    }

    // COMPILER-BACKED: endpoint element types come from the resolved channel
    // binding rather than reparsing its declaration text.
    pub(super) fn hover_for_channel_endpoint(
        &self,
        channel_position: LspPosition,
        endpoint: &str,
    ) -> Option<LspHover> {
        let reference = self.reference_at(channel_position)?;
        let symbol_id = reference.resolved?;
        let (package, _) = self.current_resolved_package()?;
        let typed_package = self.typed_workspace.as_ref()?.package(&package.identity)?;
        let type_id = typed_package
            .program
            .typed_symbol(symbol_id)?
            .declared_type?;
        let table = typed_package.program.type_table();
        let element = match table.get(type_id)? {
            fol_typecheck::CheckedType::Channel { element_type }
            | fol_typecheck::CheckedType::ChannelSender { element_type } => {
                render_checked_type(table, *element_type)
            }
            _ => return None,
        };
        Some(LspHover {
            contents: if endpoint == "tx" {
                format!("c[tx]: non-blocking send of `{element}`")
            } else {
                format!("c[rx]: blocking receive of `{element}`; iteration continues until closed")
            },
            range: None,
        })
    }

    // COMPILER-BACKED: parameter declaration sites are not references, so use
    // the resolver scope chain to find the named symbol and its typed mutex bit.
    pub(super) fn hover_for_mutex_binding(
        &self,
        position: LspPosition,
        name: &str,
    ) -> Option<LspHover> {
        let (program, mut scope_id) = self.scope_at_position(position)?;
        let key = fol_types::canonical_identifier_key(name);
        let symbol_id = loop {
            if let Some(symbol) = program
                .symbols_named_in_scope(scope_id, &key)
                .into_iter()
                .find(|symbol| symbol.kind == fol_resolver::SymbolKind::Parameter)
            {
                break symbol.id;
            }
            scope_id = program.scope(scope_id)?.parent?;
        };
        let (package, _) = self.current_resolved_package()?;
        let typed_package = self.typed_workspace.as_ref()?.package(&package.identity)?;
        let symbol = typed_package.program.typed_symbol(symbol_id)?;
        if !symbol.is_mutex {
            return None;
        }
        let guarded =
            render_checked_type(typed_package.program.type_table(), symbol.declared_type?);
        Some(LspHover {
            contents: format!(
                "mux[T]: mutex-guarded shared `{guarded}` (auto-unlock at scope end)"
            ),
            range: None,
        })
    }

    // COMPILER-BACKED: dereference hover is derived from the operand's checked
    // pointer type; the source scan only identifies the adjacent operand.
    pub(super) fn hover_for_dereference(&self, operand_position: LspPosition) -> Option<LspHover> {
        let reference = self.reference_at(operand_position)?;
        let symbol_id = reference.resolved?;
        let (package, _) = self.current_resolved_package()?;
        let typed_package = self.typed_workspace.as_ref()?.package(&package.identity)?;
        let type_id = typed_package
            .program
            .typed_symbol(symbol_id)?
            .declared_type?;
        let table = typed_package.program.type_table();
        let (target, shared, borrowed) = pointer_type_info(&typed_package.program, type_id)?;
        let target_name = render_checked_type(table, target);
        let target_moves = fol_typecheck::exprs::bindings::ownership_moves_on_transfer(
            &typed_package.program,
            target,
        );
        let contents = if borrowed {
            if target_moves {
                format!(
                    "[drf]: read-only borrowed pointer cannot transfer move-only `{target_name}`"
                )
            } else {
                format!(
                    "[drf]: read-only borrowed pointer clones `{target_name}` without consuming the pointer"
                )
            }
        } else if shared {
            if target_moves {
                format!("[drf]: shared pointer cannot transfer move-only `{target_name}`")
            } else {
                format!(
                    "[drf]: read-only shared pointer clones `{target_name}` without consuming the pointer"
                )
            }
        } else if target_moves {
            format!("[drf]: transfers move-only `{target_name}` and consumes the unique pointer")
        } else {
            format!("[drf]: reads clone-safe `{target_name}` without consuming the unique pointer")
        };
        Some(LspHover {
            contents,
            range: None,
        })
    }

    // COMPILER-BACKED: resolved symbol origin (no text fallback)
    pub(super) fn definition_for_symbol(
        &self,
        symbol_id: fol_resolver::SymbolId,
    ) -> Option<LspLocation> {
        let (_, program) = self.current_resolved_package()?;
        let symbol = program.symbol(symbol_id)?;
        let origin = symbol.origin.as_ref()?;
        let file = self.map_analyzed_file_to_source(origin.file.as_ref()?);
        Some(LspLocation {
            uri: format!("file://{file}"),
            range: location_to_range(&fol_diagnostics::DiagnosticLocation {
                file: Some(file),
                line: origin.line,
                column: origin.column,
                length: Some(origin.length),
            }),
        })
    }

    pub(super) fn definition_for_reference(
        &self,
        reference: &fol_resolver::ResolvedReference,
    ) -> Option<LspLocation> {
        self.definition_for_symbol(reference.resolved?)
    }

    pub(super) fn references_for_reference(
        &self,
        reference: &fol_resolver::ResolvedReference,
        include_declaration: bool,
    ) -> Vec<LspLocation> {
        let Some((_, program)) = self.current_resolved_package() else {
            return Vec::new();
        };
        let Some(symbol_id) = reference.resolved else {
            return Vec::new();
        };
        let mut locations = Vec::new();
        let Some(symbol) = program.symbol(symbol_id) else {
            return Vec::new();
        };

        if include_declaration {
            if let Some(origin) = symbol.origin.as_ref() {
                if let Some(file) = origin.file.as_ref() {
                    let source_file = self.map_analyzed_file_to_source(file);
                    locations.push(LspLocation {
                        uri: format!("file://{source_file}"),
                        range: location_to_range(&fol_diagnostics::DiagnosticLocation {
                            file: Some(source_file),
                            line: origin.line,
                            column: origin.column,
                            length: Some(origin.length),
                        }),
                    });
                }
            }
        }

        for hit in program
            .all_references()
            .filter(|hit| hit.resolved == Some(symbol_id))
        {
            let Some(syntax_id) = hit.anchor() else {
                continue;
            };
            let Some(origin) = program.syntax_index().origin(syntax_id) else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else {
                continue;
            };
            let source_file = self.map_analyzed_file_to_source(file);
            locations.push(LspLocation {
                uri: format!("file://{source_file}"),
                range: location_to_range(&fol_diagnostics::DiagnosticLocation {
                    file: Some(source_file),
                    line: origin.line,
                    column: origin.column,
                    length: Some(origin.length),
                }),
            });
        }

        locations.sort_by(|left, right| {
            left.uri
                .cmp(&right.uri)
                .then(left.range.start.line.cmp(&right.range.start.line))
                .then(left.range.start.character.cmp(&right.range.start.character))
        });
        locations.dedup_by(|left, right| left == right);
        locations
    }

    pub(super) fn rename_for_reference(
        &self,
        reference: &fol_resolver::ResolvedReference,
        new_name: &str,
    ) -> EditorResult<LspWorkspaceEdit> {
        ensure_renameable_identifier(new_name)?;
        let (program, symbol) = self.validated_rename_target(reference)?;
        let symbol_id = symbol.id;

        let mut edits = Vec::new();
        let declaration = symbol.origin.as_ref().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename target is missing a declaration location",
            )
        })?;
        let declaration_file = declaration.file.as_ref().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename target is missing a declaration file",
            )
        })?;
        edits.push((
            self.map_analyzed_file_to_source(declaration_file),
            LspTextEdit {
                range: location_to_range(&fol_diagnostics::DiagnosticLocation {
                    file: Some(self.map_analyzed_file_to_source(declaration_file)),
                    line: declaration.line,
                    column: declaration.column,
                    length: Some(declaration.length),
                }),
                new_text: new_name.to_string(),
            },
        ));

        for hit in program
            .all_references()
            .filter(|hit| hit.resolved == Some(symbol_id))
        {
            let Some(syntax_id) = hit.anchor() else {
                continue;
            };
            let Some(origin) = program.syntax_index().origin(syntax_id) else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else {
                continue;
            };
            edits.push((
                self.map_analyzed_file_to_source(file),
                LspTextEdit {
                    range: location_to_range(&fol_diagnostics::DiagnosticLocation {
                        file: Some(self.map_analyzed_file_to_source(file)),
                        line: origin.line,
                        column: origin.column,
                        length: Some(origin.length),
                    }),
                    new_text: new_name.to_string(),
                },
            ));
        }

        edits.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(left.1.range.start.line.cmp(&right.1.range.start.line))
                .then(
                    left.1
                        .range
                        .start
                        .character
                        .cmp(&right.1.range.start.character),
                )
        });
        edits.dedup_by(|left, right| left == right);
        let mut changes = std::collections::BTreeMap::new();
        for (file, edit) in edits {
            changes
                .entry(format!("file://{file}"))
                .or_insert_with(Vec::new)
                .push(edit);
        }
        for edits in changes.values_mut() {
            edits.sort_by(|left, right| {
                left.range
                    .start
                    .line
                    .cmp(&right.range.start.line)
                    .then(left.range.start.character.cmp(&right.range.start.character))
            });
        }
        Ok(LspWorkspaceEdit { changes })
    }

    fn validated_rename_target<'a>(
        &'a self,
        reference: &fol_resolver::ResolvedReference,
    ) -> EditorResult<(
        &'a fol_resolver::ResolvedProgram,
        &'a fol_resolver::ResolvedSymbol,
    )> {
        self.resolved_workspace.as_ref().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename requires a resolved workspace",
            )
        })?;
        let symbol_id = reference.resolved.ok_or_else(|| {
            EditorError::new(EditorErrorKind::InvalidInput, "rename target is unresolved")
        })?;
        let analyzed_path = self.analyzed_path.as_ref().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename target has no analyzed document path",
            )
        })?;
        let analyzed_path_text = analyzed_path.to_string_lossy().to_string();

        let (_, program) = self.current_resolved_package().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename target symbol was not found in the resolved workspace",
            )
        })?;
        let Some(symbol) = program.symbol(symbol_id) else {
            return Err(EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename target symbol was not found in the resolved workspace",
            ));
        };

        // Parameters now carry their own declaration origin (the parameter
        // NAME span), so renaming one rewrites the parameter declaration and
        // its uses rather than the routine header. That makes `Parameter`
        // safe to rename within the same file.
        if !matches!(
            symbol.kind,
            fol_resolver::SymbolKind::ValueBinding
                | fol_resolver::SymbolKind::LabelBinding
                | fol_resolver::SymbolKind::DestructureBinding
                | fol_resolver::SymbolKind::Parameter
                | fol_resolver::SymbolKind::Routine
                | fol_resolver::SymbolKind::Type
                | fol_resolver::SymbolKind::Alias
                | fol_resolver::SymbolKind::Definition
                | fol_resolver::SymbolKind::Capture
                | fol_resolver::SymbolKind::LoopBinder
                | fol_resolver::SymbolKind::RollingBinder
        ) {
            return Err(EditorError::new(
                    EditorErrorKind::InvalidInput,
                    format!(
                        "rename currently supports same-file local and current-package top-level symbols only, not {}",
                        render_symbol_kind(symbol.kind)
                    ),
                ));
        }

        let declaration = symbol.origin.as_ref().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename target is missing a declaration location",
            )
        })?;
        let declaration_file = declaration.file.as_ref().ok_or_else(|| {
            EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename target is missing a declaration file",
            )
        })?;
        // Build manifests (`build.fol`) declare the build entry graph, not
        // ordinary renameable source. The build entry routine name is a
        // fixed contract with the build system, so rename stays outside the
        // safe boundary for symbols declared in a build file.
        if Path::new(declaration_file)
            .file_name()
            .and_then(|name| name.to_str())
            == Some("build.fol")
        {
            return Err(EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename does not support build entry symbols",
            ));
        }
        if declaration_file != &analyzed_path_text {
            return Err(EditorError::new(
                EditorErrorKind::InvalidInput,
                "rename currently supports same-file symbols only",
            ));
        }

        for hit in program
            .all_references()
            .filter(|hit| hit.resolved == Some(symbol_id))
        {
            let Some(syntax_id) = hit.anchor() else {
                continue;
            };
            let Some(origin) = program.syntax_index().origin(syntax_id) else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else {
                continue;
            };
            if file != &analyzed_path_text {
                return Err(EditorError::new(
                    EditorErrorKind::InvalidInput,
                    "rename currently supports same-file symbols only",
                ));
            }
        }
        Ok((program, symbol))
    }

    // COMPILER-BACKED: resolved symbols by path (no text fallback)
    pub(super) fn document_symbols_for_current_path(&self) -> Vec<LspDocumentSymbol> {
        let resolved = match &self.resolved_workspace {
            Some(resolved) => resolved,
            None => return Vec::new(),
        };
        let Some(analyzed_path) = &self.analyzed_path else {
            return Vec::new();
        };
        let path_text = analyzed_path.to_string_lossy();
        let mut symbols = Vec::new();
        for package in resolved.packages() {
            let program = &package.program;
            for symbol in program.all_symbols() {
                let Some(origin) = &symbol.origin else {
                    continue;
                };
                let Some(file) = &origin.file else { continue };
                if file != &path_text {
                    continue;
                }
                let range = location_to_range(&fol_diagnostics::DiagnosticLocation {
                    file: Some(file.clone()),
                    line: origin.line,
                    column: origin.column,
                    length: Some(origin.length),
                });
                symbols.push(LspDocumentSymbol {
                    name: symbol.name.clone(),
                    kind: symbol_kind_code(symbol.kind),
                    range,
                    selection_range: range,
                    children: Vec::new(),
                });
            }
        }
        if let Some((program, source_unit)) = self.current_syntax_source_unit() {
            let syntax_symbols = syntax_document_symbols(program, source_unit);
            let expanded_ranges = syntax_symbols
                .iter()
                .map(|symbol| (document_symbol_key(symbol), symbol.range))
                .collect::<std::collections::BTreeMap<_, _>>();
            for symbol in &mut symbols {
                if let Some(expanded) = expanded_ranges.get(&document_symbol_key(symbol)) {
                    symbol.range = *expanded;
                }
            }
            let mut seen = symbols
                .iter()
                .map(document_symbol_key)
                .collect::<std::collections::BTreeSet<_>>();
            for symbol in syntax_symbols {
                if seen.insert(document_symbol_key(&symbol)) {
                    symbols.push(symbol);
                }
            }
        }
        symbols.sort_by(|left, right| {
            left.range
                .start
                .line
                .cmp(&right.range.start.line)
                .then(left.range.start.character.cmp(&right.range.start.character))
                .then(left.name.cmp(&right.name))
        });
        nest_document_symbols(symbols)
    }

    fn symbol_at_position(&self, position: LspPosition) -> Option<fol_resolver::SymbolId> {
        self.reference_at(position)
            .and_then(|reference| reference.resolved)
            .or_else(|| self.method_target_symbol_at(position))
    }

    fn current_document_uri(&self) -> Option<String> {
        let path = self.analyzed_path.as_ref()?;
        Some(format!(
            "file://{}",
            self.map_analyzed_file_to_source(&path.to_string_lossy())
        ))
    }

    // COMPILER-BACKED: symbol under the cursor -> its declared type -> that type's decl
    pub(super) fn type_definition_at(&self, position: LspPosition) -> Option<LspLocation> {
        let symbol_id = self.symbol_at_position(position)?;
        let (package, _) = self.current_resolved_package()?;
        let typed_package = self.typed_workspace.as_ref()?.package(&package.identity)?;
        let type_id = typed_package
            .program
            .typed_symbol(symbol_id)?
            .declared_type?;
        let type_symbol = declared_type_symbol(typed_package.program.type_table(), type_id)?;
        self.definition_for_symbol(type_symbol)
    }

    // COMPILER-BACKED: a protocol standard under the cursor -> its conforming types
    pub(super) fn implementations_at(&self, position: LspPosition) -> Vec<LspLocation> {
        let Some(symbol_id) = self.symbol_at_position(position) else {
            return Vec::new();
        };
        let Some((package, program)) = self.current_resolved_package() else {
            return Vec::new();
        };
        let Some(symbol) = program.symbol(symbol_id) else {
            return Vec::new();
        };
        if symbol.kind != fol_resolver::SymbolKind::Standard {
            return Vec::new();
        }
        let Some(typed_package) = self
            .typed_workspace
            .as_ref()
            .and_then(|typed| typed.package(&package.identity))
        else {
            return Vec::new();
        };
        let mut locations = Vec::new();
        for conformance in typed_package.program.all_typed_conformances() {
            if conformance.standard_symbol_ids.contains(&symbol_id) {
                if let Some(location) = self.definition_for_symbol(conformance.type_symbol_id) {
                    locations.push(location);
                }
            }
        }
        locations.sort_by(|left, right| {
            left.uri
                .cmp(&right.uri)
                .then(left.range.start.line.cmp(&right.range.start.line))
        });
        locations.dedup();
        locations
    }

    // COMPILER-BACKED: occurrences of the symbol under the cursor, current file only
    pub(super) fn document_highlights_at(
        &self,
        position: LspPosition,
    ) -> Vec<LspDocumentHighlight> {
        let Some(reference) = self.reference_at(position) else {
            return Vec::new();
        };
        let Some(current_uri) = self.current_document_uri() else {
            return Vec::new();
        };
        let mut highlights = self
            .references_for_reference(&reference, true)
            .into_iter()
            .filter(|location| location.uri == current_uri)
            .map(|location| LspDocumentHighlight {
                range: location.range,
                kind: Some(1),
            })
            .collect::<Vec<_>>();
        highlights.sort_by(|left, right| {
            left.range
                .start
                .line
                .cmp(&right.range.start.line)
                .then(left.range.start.character.cmp(&right.range.start.character))
        });
        highlights.dedup();
        highlights
    }

    // COMPILER-BACKED: the renameable identifier range under the cursor
    pub(super) fn prepare_rename_at(
        &self,
        position: LspPosition,
    ) -> Option<LspPrepareRenameResult> {
        let reference = self.reference_at(position)?;
        let (program, symbol) = self.validated_rename_target(&reference).ok()?;
        let syntax_id = reference.anchor()?;
        let origin = program.syntax_index().origin(syntax_id)?;
        let file = origin.file.as_ref()?;
        let range = location_to_range(&fol_diagnostics::DiagnosticLocation {
            file: Some(self.map_analyzed_file_to_source(file)),
            line: origin.line,
            column: origin.column,
            length: Some(origin.length),
        });
        Some(LspPrepareRenameResult {
            range,
            placeholder: symbol.name.clone(),
        })
    }

    // Structural folding of every multi-line brace block (routine/record bodies,
    // nested blocks). Text-driven so it works even while the buffer is mid-edit.
    pub(super) fn folding_ranges(&self, document: &EditorDocument) -> Vec<LspFoldingRange> {
        scan_brace_blocks(&document.text)
            .into_iter()
            .filter(|(start, end)| end.line > start.line)
            .map(|(start, end)| LspFoldingRange {
                start_line: start.line,
                start_character: None,
                end_line: end.line,
                end_character: None,
                kind: None,
            })
            .collect()
    }

    // Syntax-aware expand/shrink selection: word -> enclosing brace blocks -> file.
    pub(super) fn selection_ranges(
        &self,
        document: &EditorDocument,
        positions: &[LspPosition],
    ) -> Vec<LspSelectionRange> {
        let blocks = scan_brace_blocks(&document.text);
        let doc_range = document_full_range(&document.text);
        positions
            .iter()
            .map(|position| {
                let mut ranges: Vec<LspRange> = Vec::new();
                if let Some(word) = word_range_at(&document.text, *position) {
                    ranges.push(word);
                }
                let mut enclosing = blocks
                    .iter()
                    .map(|(start, end)| LspRange {
                        start: *start,
                        end: *end,
                    })
                    .filter(|range| range_contains_position(range, *position))
                    .collect::<Vec<_>>();
                enclosing.sort_by_key(|range| range.end.line.saturating_sub(range.start.line));
                ranges.extend(enclosing);
                ranges.push(doc_range);
                ranges.dedup();
                build_selection_chain(&ranges)
            })
            .collect()
    }

    // COMPILER-BACKED: inferred-type hints for `var`/`lab`/`con` bindings that
    // lack an explicit `: type` annotation (rust-analyzer/gopls style). The
    // resolver now records the binding-name declaration origin, so both the
    // type (the binding symbol's recorded/inferred type) and the position (the
    // symbol origin) come straight from compiler data — precise even for
    // multi-name and shadowed bindings.
    pub(super) fn inlay_hints(
        &self,
        document: &EditorDocument,
        range: LspRange,
    ) -> Vec<LspInlayHint> {
        let Some((package, program)) = self.current_resolved_package() else {
            return Vec::new();
        };
        let Some(analyzed_path) = self.analyzed_path.as_ref() else {
            return Vec::new();
        };
        let path_text = analyzed_path.to_string_lossy();
        let Some(typed_package) = self
            .typed_workspace
            .as_ref()
            .and_then(|typed| typed.package(&package.identity))
        else {
            return Vec::new();
        };
        let mut hints = Vec::new();
        for symbol in program.all_symbols() {
            if !matches!(
                symbol.kind,
                fol_resolver::SymbolKind::ValueBinding | fol_resolver::SymbolKind::LabelBinding
            ) {
                continue;
            }
            let Some(origin) = &symbol.origin else {
                continue;
            };
            let Some(file) = &origin.file else { continue };
            if file != &path_text {
                continue;
            }
            let line = origin.line.saturating_sub(1) as u32;
            if line < range.start.line || line > range.end.line {
                continue;
            }
            // Skip bindings that already carry an explicit `: type` annotation.
            if binding_has_explicit_annotation(&document.text, origin) {
                continue;
            }
            let Some(type_id) = typed_package
                .program
                .typed_symbol(symbol.id)
                .and_then(|typed_symbol| typed_symbol.declared_type)
            else {
                continue;
            };
            let type_text = render_checked_type(typed_package.program.type_table(), type_id);
            if type_text == "unknown" {
                continue;
            }
            let character = (origin.column.saturating_sub(1) + origin.length) as u32;
            hints.push(LspInlayHint {
                position: LspPosition { line, character },
                label: format!(": {type_text}"),
                kind: Some(1),
                padding_left: false,
                padding_right: false,
            });
        }
        hints.sort_by(|left, right| {
            left.position
                .line
                .cmp(&right.position.line)
                .then(left.position.character.cmp(&right.position.character))
        });
        hints
    }

    fn current_source_unit<'a>(
        &self,
        program: &'a fol_resolver::ResolvedProgram,
    ) -> Option<&'a fol_resolver::ResolvedSourceUnit> {
        let analyzed_path = self.analyzed_path.as_ref()?;
        let path_text = analyzed_path.to_string_lossy();
        program
            .source_units
            .iter()
            .find(move |unit| unit.path == path_text)
    }

    fn current_syntax_source_unit(
        &self,
    ) -> Option<(
        &fol_resolver::ResolvedProgram,
        &fol_parser::ast::ParsedSourceUnit,
    )> {
        let (_, program) = self.current_resolved_package()?;
        let resolved_unit = self.current_source_unit(program)?;
        let syntax_unit = program.syntax().source_units.get(resolved_unit.id.0)?;
        Some((program, syntax_unit))
    }
}

fn declared_bundled_standard_root(package_root: &Path, alias: &str) -> Option<PathBuf> {
    let metadata =
        fol_package::parse_package_metadata_from_build(&package_root.join("build.fol")).ok()?;
    metadata
        .dependencies
        .iter()
        .any(|dependency| {
            dependency.alias == alias
                && dependency.source_kind == fol_package::PackageDependencySourceKind::Internal
                && dependency.target == "standard"
        })
        .then(fol_package::available_bundled_std_root)
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::{
        direct_channel_receiver_precedes, direct_endpoint_occurs, fallback_visible_binding_kind,
        position_is_inside_deferred_block, FallbackBindingKind, SemanticSnapshot,
    };
    use crate::{EditorDocument, EditorDocumentUri, LspPosition};
    use std::fs;
    use std::path::PathBuf;

    fn fallback_snapshot(root: PathBuf, source_path: PathBuf) -> SemanticSnapshot {
        SemanticSnapshot {
            source_analysis_root: root.clone(),
            analyzed_analysis_root: root.clone(),
            analyzed_path: Some(source_path.clone()),
            analyzed_package_root: Some(root.clone()),
            source_document_path: source_path,
            source_package_root: Some(root),
            active_fol_model: None,
            active_internal_standard_aliases: Vec::new(),
            fol_model_scope_unresolved: false,
            compiler_diagnostics: Vec::new(),
            diagnostics: Vec::new(),
            resolved_workspace: None,
            typed_workspace: None,
        }
    }

    #[test]
    fn fallback_local_named_type_items_are_marked_as_uncertain() {
        let root = std::env::temp_dir().join(format!(
            "fol_semantic_types_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let source_path = root.join("src/main.fol");
        let text = concat!(
            "typ[exp] LocalRec: rec = {\n",
            "    var value: int;\n",
            "};\n",
            "ali[exp] LocalAlias: int;\n",
            "fun[] main(): int = {\n",
            "    var value: ;\n",
            "    return 0;\n",
            "};\n",
        );
        fs::write(&source_path, text).unwrap();
        let document = EditorDocument::new(
            EditorDocumentUri::from_file_path(source_path.clone()).unwrap(),
            1,
            text.to_string(),
        )
        .unwrap();
        let snapshot = fallback_snapshot(root.clone(), source_path);

        let items = snapshot.fallback_local_named_type_items(&document);
        assert_eq!(
            items
                .iter()
                .find(|item| item.label == "LocalRec")
                .and_then(|item| item.detail.as_deref()),
            Some("type (fallback)")
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.label == "LocalAlias")
                .and_then(|item| item.detail.as_deref()),
            Some("type alias (fallback)")
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fallback_qualified_completion_items_are_marked_as_uncertain() {
        let root = std::env::temp_dir().join(format!(
            "fol_semantic_qualified_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("shared")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("shared/lib.fol"),
            "fun[exp] helper(): int = {\n    return 9;\n};\n",
        )
        .unwrap();
        let source_path = root.join("src/main.fol");
        let text = concat!(
            "use shared: loc = {\"../shared\"};\n\n",
            "fun[] main(): int = {\n",
            "    return shared::;\n",
            "};\n",
        );
        fs::write(&source_path, text).unwrap();
        let snapshot = fallback_snapshot(root.clone(), source_path);

        let items = snapshot.fallback_qualified_completion_items("shared");
        assert_eq!(
            items
                .iter()
                .find(|item| item.label == "helper")
                .and_then(|item| item.detail.as_deref()),
            Some("routine (fallback)")
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fallback_local_scope_items_remain_unmarked() {
        let root = std::env::temp_dir().join(format!(
            "fol_semantic_plain_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let source_path = root.join("src/main.fol");
        let text = concat!(
            "fun[] helper(): int = {\n",
            "    return 7;\n",
            "};\n\n",
            "fun[] main(): int = {\n",
            "    var value: int = helper();\n",
            "    return value;\n",
            "};\n",
        );
        fs::write(&source_path, text).unwrap();
        let document = EditorDocument::new(
            EditorDocumentUri::from_file_path(source_path.clone()).unwrap(),
            1,
            text.to_string(),
        )
        .unwrap();
        let snapshot = fallback_snapshot(root.clone(), source_path);

        let items = snapshot.fallback_local_scope_items(
            &document,
            LspPosition {
                line: 6,
                character: 12,
            },
        );
        assert_eq!(
            items
                .iter()
                .find(|item| item.label == "value")
                .and_then(|item| item.detail.as_deref()),
            Some("binding")
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn channel_lifecycle_scanning_ignores_quoted_endpoint_text() {
        assert!(direct_endpoint_occurs(
            "var value: int = channel[rx];",
            "channel",
            "rx"
        ));
        assert!(!direct_endpoint_occurs(
            "var text: str = \"channel[rx]\";",
            "channel",
            "rx"
        ));
        assert!(!direct_endpoint_occurs(
            "var text: str = 'channel[rx]';",
            "channel",
            "rx"
        ));
        assert!(!direct_endpoint_occurs(
            "var value: int = other_channel[rx];",
            "channel",
            "rx"
        ));
        assert!(!direct_endpoint_occurs(
            "` channel[rx] in a comment `",
            "channel",
            "rx"
        ));
        assert!(!direct_endpoint_occurs(
            "/* channel[rx] in a comment */",
            "channel",
            "rx"
        ));
    }

    #[test]
    fn channel_lifecycle_scanning_ignores_multiline_comment_endpoints() {
        let text = concat!(
            "fun[] main(): int = {\n",
            "    ` channel[rx]\n",
            "       still a comment `\n",
            "    channel[\n",
        );

        assert!(!direct_channel_receiver_precedes(
            text,
            LspPosition {
                line: 3,
                character: 12,
            },
            "channel"
        ));
    }

    #[test]
    fn fallback_v3_binding_scan_ignores_comment_declarations() {
        let fake = concat!(
            "fun[] main(): int = {\n",
            "    /* var channel: chn[int];\n",
            "       still a comment */\n",
            "    channel[\n",
        );
        assert_eq!(
            fallback_visible_binding_kind(
                fake,
                LspPosition {
                    line: 3,
                    character: 12,
                },
                "channel",
            ),
            None
        );

        let real = concat!(
            "fun[] main(): int = {\n",
            "    var channel: chn[int];\n",
            "    channel[\n",
        );
        assert_eq!(
            fallback_visible_binding_kind(
                real,
                LspPosition {
                    line: 2,
                    character: 12,
                },
                "channel",
            ),
            Some(FallbackBindingKind::Channel)
        );
    }

    #[test]
    fn deferred_completion_scan_ignores_comment_and_quote_braces() {
        let fake = concat!(
            "` dfr {\n",
            "  still comment } `\n",
            "fun[] main(): int = {\n",
            "    var value: str = \"edf { }\";\n",
            "    value.\n",
        );
        assert!(!position_is_inside_deferred_block(
            fake,
            LspPosition {
                line: 4,
                character: 10,
            }
        ));

        let real = concat!("fun[] main(): int = {\n", "    dfr {\n", "        value.\n",);
        assert!(position_is_inside_deferred_block(
            real,
            LspPosition {
                line: 2,
                character: 14,
            }
        ));
    }
}

fn builtin_type_family(name: &str) -> EditorTypeFamily {
    match name {
        "str" => EditorTypeFamily::String,
        _ => EditorTypeFamily::Scalar,
    }
}

fn container_type_family(name: &str) -> EditorTypeFamily {
    match name {
        "arr" => EditorTypeFamily::Array,
        "vec" => EditorTypeFamily::Vector,
        "seq" => EditorTypeFamily::Sequence,
        "set" => EditorTypeFamily::Set,
        "map" => EditorTypeFamily::Map,
        _ => EditorTypeFamily::RecordLike,
    }
}

fn shell_type_family(name: &str) -> EditorTypeFamily {
    match name {
        "opt" => EditorTypeFamily::OptionalShell,
        "err" => EditorTypeFamily::ErrorShell,
        _ => EditorTypeFamily::RecordLike,
    }
}

fn completion_keyword_item(keyword: &str) -> EditorCompletionItem {
    // LSP CompletionItemKind::Keyword == 14.
    EditorCompletionItem {
        label: keyword.to_string(),
        kind: 14,
        detail: Some("keyword".to_string()),
        insert_text: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FallbackBindingKind {
    Channel,
    Mutex,
    Other,
}

fn fallback_visible_binding_kind(
    text: &str,
    position: LspPosition,
    receiver: &str,
) -> Option<FallbackBindingKind> {
    let offset = position_to_offset(text, position)?;
    let masked = crate::source_scan::mask_non_code(&text[..offset]);
    let prefix = current_routine_prefix(&masked);
    let is_identifier_char = |ch: char| ch.is_alphanumeric() || ch == '_';

    for line in prefix.lines().rev() {
        for (start, _) in line.rmatch_indices(receiver) {
            if line[..start]
                .chars()
                .next_back()
                .is_some_and(is_identifier_char)
            {
                continue;
            }
            let tail = &line[start + receiver.len()..];
            if tail.chars().next().is_some_and(is_identifier_char) {
                continue;
            }
            let before = &line[..start];
            let after = tail.trim_start();
            if let Some(type_text) = after.strip_prefix(':').map(str::trim_start) {
                // `name: mux[T]` marks a mutex-guarded parameter (V3_MEM §8.3),
                // replacing the removed `name[mux]:` option.
                if type_text
                    .strip_prefix("mux")
                    .is_some_and(|tail| tail.trim_start().starts_with('['))
                {
                    return Some(FallbackBindingKind::Mutex);
                }
                if type_text
                    .strip_prefix("chn")
                    .is_some_and(|tail| tail.trim_start().starts_with('['))
                {
                    return Some(FallbackBindingKind::Channel);
                }
                return Some(FallbackBindingKind::Other);
            }
            let binding_head = before
                .rsplit([';', '{', '}'])
                .next()
                .unwrap_or(before)
                .trim();
            if matches!(binding_head, "var" | "lab" | "@var" | "@lab")
                || binding_head.starts_with("var[") && binding_head.ends_with(']')
                || binding_head.starts_with("lab[") && binding_head.ends_with(']')
            {
                return Some(FallbackBindingKind::Other);
            }
        }
    }
    None
}

fn current_routine_prefix(prefix: &str) -> &str {
    let start = ["fun[", "pro[", "log[", "fun [", "pro [", "log ["]
        .into_iter()
        .filter_map(|needle| prefix.rfind(needle))
        .max()
        .unwrap_or(0);
    &prefix[start..]
}

fn direct_channel_receiver_precedes(text: &str, position: LspPosition, receiver: &str) -> bool {
    let Some(offset) = position_to_offset(text, position) else {
        return false;
    };
    let masked = crate::source_scan::mask_non_code(&text[..offset]);
    current_routine_prefix(&masked)
        .lines()
        .any(|line| direct_endpoint_occurs_in_code(line, receiver, "rx"))
}

/// Return whether the exact resolved channel binding has already been used as
/// a direct receiver before the completion cursor. Matching by `SymbolId`
/// keeps a receive on an outer same-name binding from consuming the lifecycle
/// of a shadowing inner channel.
fn resolved_channel_receive_precedes(
    program: &fol_resolver::ResolvedProgram,
    symbol_id: fol_resolver::SymbolId,
    path: &Path,
    text: &str,
    position: LspPosition,
) -> bool {
    let path_text = path.to_string_lossy();
    program
        .syntax()
        .source_units
        .iter()
        .filter(|unit| unit.path == path_text)
        .flat_map(|unit| unit.items.iter().map(|item| &item.node))
        .any(|node| {
            resolved_channel_receive_in_node_precedes(
                program, symbol_id, &path_text, text, position, node,
            )
        })
}

fn resolved_channel_receive_in_node_precedes(
    program: &fol_resolver::ResolvedProgram,
    symbol_id: fol_resolver::SymbolId,
    path: &str,
    text: &str,
    position: LspPosition,
    node: &AstNode,
) -> bool {
    if let AstNode::ChannelAccess {
        channel,
        endpoint: fol_parser::ast::ChannelEndpoint::Rx,
    } = node
    {
        if let Some(syntax_id) = channel.syntax_id() {
            let resolves_exact_binding = program.all_references().any(|reference| {
                reference.anchor() == Some(syntax_id) && reference.resolved == Some(symbol_id)
            });
            let occurs_before_cursor =
                program
                    .syntax_index()
                    .origin(syntax_id)
                    .is_some_and(|origin| {
                        if origin.file.as_deref() != Some(path) {
                            return false;
                        }
                        let line_index = origin.line.saturating_sub(1);
                        if line_index as u32 > position.line {
                            return false;
                        }
                        let Some(line) = text.lines().nth(line_index) else {
                            return false;
                        };
                        let visible_characters = if line_index as u32 == position.line {
                            position.character as usize
                        } else {
                            line.chars().count()
                        };
                        let visible = line.chars().take(visible_characters).collect::<String>();
                        let masked = crate::source_scan::mask_non_code(&visible);
                        let start = origin.column.saturating_sub(1);
                        let tail = masked
                            .chars()
                            .skip(start + origin.length)
                            .collect::<String>();
                        tail.trim_start()
                            .strip_prefix('[')
                            .map(str::trim_start)
                            .and_then(|tail| tail.strip_prefix("rx"))
                            .map(str::trim_start)
                            .is_some_and(|tail| tail.starts_with(']'))
                    });
            if resolves_exact_binding && occurs_before_cursor {
                return true;
            }
        }
    }

    node.children().into_iter().any(|child| {
        resolved_channel_receive_in_node_precedes(program, symbol_id, path, text, position, child)
    })
}

#[cfg(test)]
fn direct_endpoint_occurs(line: &str, receiver: &str, endpoint: &str) -> bool {
    let line = crate::source_scan::mask_non_code(line);
    direct_endpoint_occurs_in_code(&line, receiver, endpoint)
}

fn direct_endpoint_occurs_in_code(line: &str, receiver: &str, endpoint: &str) -> bool {
    let is_identifier_char = |ch: char| ch.is_alphanumeric() || ch == '_';
    line.match_indices(receiver).any(|(start, _)| {
        if line[..start]
            .chars()
            .next_back()
            .is_some_and(is_identifier_char)
        {
            return false;
        }
        let tail = &line[start + receiver.len()..];
        if tail.chars().next().is_some_and(is_identifier_char) {
            return false;
        }
        tail.trim_start()
            .strip_prefix('[')
            .map(str::trim_start)
            .and_then(|tail| tail.strip_prefix(endpoint))
            .map(str::trim_start)
            .is_some_and(|tail| tail.starts_with(']'))
    })
}

fn position_is_inside_deferred_block(text: &str, position: LspPosition) -> bool {
    let Some(offset) = position_to_offset(text, position) else {
        return false;
    };
    let mut deferred_stack = Vec::new();
    let mut last_identifier = String::new();
    let mut identifier = String::new();
    let masked = crate::source_scan::mask_non_code(&text[..offset]);

    for ch in masked.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            identifier.push(ch);
            continue;
        }
        if !identifier.is_empty() {
            last_identifier = std::mem::take(&mut identifier);
        }
        match ch {
            '{' => {
                let inherited = deferred_stack.last().copied().unwrap_or(false);
                deferred_stack.push(inherited || matches!(last_identifier.as_str(), "dfr" | "edf"));
                last_identifier.clear();
            }
            '}' => {
                let _ = deferred_stack.pop();
                last_identifier.clear();
            }
            ';' => last_identifier.clear(),
            _ => {}
        }
    }

    deferred_stack.last().copied().unwrap_or(false)
}

fn mutex_method_completion_items() -> Vec<EditorCompletionItem> {
    ["lock", "unlock"]
        .into_iter()
        .map(|method| EditorCompletionItem {
            label: method.to_string(),
            kind: 2,
            detail: Some("mutex method".to_string()),
            insert_text: Some(method.to_string()),
        })
        .collect()
}

#[derive(Debug, Clone)]
struct SignatureCallSite {
    callee_syntax_id: SyntaxNodeId,
    display_name: String,
    active_parameter: usize,
    span_len: usize,
}

fn visit_call_sites(
    node: &AstNode,
    program: &fol_resolver::ResolvedProgram,
    path: &std::path::Path,
    text: &str,
    cursor_offset: usize,
    best: &mut Option<SignatureCallSite>,
) {
    match node {
        AstNode::FunctionCall {
            syntax_id: Some(syntax_id),
            name,
            ..
        } => {
            if let Some(candidate) =
                signature_call_site(program, path, text, cursor_offset, *syntax_id, name)
            {
                choose_better_call_site(best, candidate);
            }
        }
        AstNode::QualifiedFunctionCall {
            path: qualified, ..
        } => {
            if let Some(syntax_id) = qualified.syntax_id() {
                if let Some(candidate) = signature_call_site(
                    program,
                    path,
                    text,
                    cursor_offset,
                    syntax_id,
                    &qualified.joined(),
                ) {
                    choose_better_call_site(best, candidate);
                }
            }
        }
        _ => {}
    }
    for child in node.children() {
        visit_call_sites(child, program, path, text, cursor_offset, best);
    }
}

fn choose_better_call_site(best: &mut Option<SignatureCallSite>, candidate: SignatureCallSite) {
    match best {
        Some(current) if current.span_len <= candidate.span_len => {}
        _ => *best = Some(candidate),
    }
}

fn signature_call_site(
    program: &fol_resolver::ResolvedProgram,
    path: &std::path::Path,
    text: &str,
    cursor_offset: usize,
    callee_syntax_id: SyntaxNodeId,
    display_name: &str,
) -> Option<SignatureCallSite> {
    let origin = program.syntax_index().origin(callee_syntax_id)?;
    if origin.file.as_deref()? != path.to_str()? {
        return None;
    }
    let callee_start = offset_for_origin(text, origin)?;
    let callee_end = callee_start + origin.length;
    let open_paren = find_call_open_paren(text, callee_end)?;
    let close_paren = find_matching_paren(text, open_paren)?;
    if cursor_offset < callee_start || cursor_offset > close_paren + 1 {
        return None;
    }
    Some(SignatureCallSite {
        callee_syntax_id,
        display_name: display_name.to_string(),
        active_parameter: active_parameter_index(text, open_paren, cursor_offset),
        span_len: close_paren.saturating_sub(callee_start),
    })
}

fn reference_for_syntax_id(
    program: &fol_resolver::ResolvedProgram,
    syntax_id: SyntaxNodeId,
) -> Option<fol_resolver::ResolvedReference> {
    if let Some(reference) = program.all_references().find(|reference| {
        reference.syntax_id == Some(syntax_id) || reference.anchor_syntax_id == Some(syntax_id)
    }) {
        return Some(reference.clone());
    }
    let origin = program.syntax_index().origin(syntax_id)?;
    let file = origin.file.as_deref()?;
    let line = origin.line;
    let column = origin.column;
    let mut best_reference: Option<(&fol_resolver::ResolvedReference, usize)> = None;
    for reference in program.all_references() {
        let Some(reference_syntax_id) = reference.anchor() else {
            continue;
        };
        let Some(reference_origin) = program.syntax_index().origin(reference_syntax_id) else {
            continue;
        };
        if reference_origin.file.as_deref()? != file {
            continue;
        }
        if !origin_contains(reference_origin, line, column) {
            continue;
        }
        match best_reference {
            Some((_, best_len)) if best_len <= reference_origin.length => {}
            _ => best_reference = Some((reference, reference_origin.length.max(1))),
        }
    }
    if let Some((reference, _)) = best_reference {
        return Some(reference.clone());
    }

    let mut best_symbol: Option<(&fol_resolver::ResolvedSymbol, usize)> = None;
    for symbol in program.all_symbols() {
        let Some(symbol_origin) = symbol.origin.as_ref() else {
            continue;
        };
        if symbol_origin.file.as_deref()? != file {
            continue;
        }
        if !origin_contains(symbol_origin, line, column) {
            continue;
        }
        match best_symbol {
            Some((_, best_len)) if best_len <= symbol_origin.length => {}
            _ => best_symbol = Some((symbol, symbol_origin.length.max(1))),
        }
    }
    best_symbol.map(|(symbol, _)| fol_resolver::ResolvedReference {
        id: fol_resolver::ReferenceId(usize::MAX),
        kind: fol_resolver::ReferenceKind::Identifier,
        syntax_id: Some(syntax_id),
        anchor_syntax_id: None,
        name: symbol.name.clone(),
        scope: symbol.scope,
        source_unit: symbol.source_unit,
        resolved: Some(symbol.id),
    })
}

fn reference_at_position_in_program(
    program: &fol_resolver::ResolvedProgram,
    path: &Path,
    position: LspPosition,
) -> Option<fol_resolver::ResolvedReference> {
    use super::analysis::syntax_at_position;

    if let Some(syntax_id) = syntax_at_position(program, path, position) {
        if let Some(reference) = reference_for_syntax_id(program, syntax_id) {
            return Some(reference);
        }
    }

    let path_text = path.to_string_lossy();
    let line = position.line as usize + 1;
    let column = position.character as usize + 1;
    let mut best_reference: Option<(&fol_resolver::ResolvedReference, usize)> = None;
    for reference in program.all_references() {
        let Some(syntax_id) = reference.anchor() else {
            continue;
        };
        let Some(origin) = program.syntax_index().origin(syntax_id) else {
            continue;
        };
        let Some(file) = origin.file.as_ref() else {
            continue;
        };
        if file != &path_text {
            continue;
        }
        if !origin_contains(origin, line, column) {
            continue;
        }
        match best_reference {
            Some((_, best_len)) if best_len <= origin.length => {}
            _ => best_reference = Some((reference, origin.length.max(1))),
        }
    }
    if let Some((reference, _)) = best_reference {
        return Some(reference.clone());
    }

    let mut best_symbol: Option<(&fol_resolver::ResolvedSymbol, usize)> = None;
    for symbol in program.all_symbols() {
        let Some(origin) = symbol.origin.as_ref() else {
            continue;
        };
        let Some(file) = origin.file.as_ref() else {
            continue;
        };
        if file != &path_text {
            continue;
        }
        if !origin_contains(origin, line, column) {
            continue;
        }
        match best_symbol {
            Some((_, best_len)) if best_len <= origin.length => {}
            _ => best_symbol = Some((symbol, origin.length.max(1))),
        }
    }
    best_symbol.map(|(symbol, _)| fol_resolver::ResolvedReference {
        id: fol_resolver::ReferenceId(usize::MAX),
        kind: fol_resolver::ReferenceKind::Identifier,
        syntax_id: None,
        anchor_syntax_id: None,
        name: symbol.name.clone(),
        scope: symbol.scope,
        source_unit: symbol.source_unit,
        resolved: Some(symbol.id),
    })
}

/// Applying a rename with an illegal identifier would write syntactically
/// broken source; validate the new name before producing any edit.
fn ensure_renameable_identifier(new_name: &str) -> EditorResult<()> {
    let valid_shape = !new_name.is_empty()
        && new_name
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && new_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
    if !valid_shape {
        return Err(EditorError::new(
            EditorErrorKind::InvalidInput,
            format!(
                "rename target '{}' is not a legal FOL identifier",
                new_name.escape_debug()
            ),
        ));
    }
    let is_keyword = fol_lexer::token::buildin::DECLARATION_KEYWORDS
        .iter()
        .chain(fol_lexer::token::buildin::CONTROL_KEYWORDS)
        .chain(fol_lexer::token::buildin::OPERATOR_KEYWORDS)
        .chain(fol_lexer::token::buildin::LITERAL_KEYWORDS)
        .chain(fol_lexer::token::buildin::DIAGNOSTIC_KEYWORDS)
        .chain(fol_lexer::token::buildin::OTHER_KEYWORDS)
        .any(|keyword| *keyword == new_name);
    if is_keyword {
        return Err(EditorError::new(
            EditorErrorKind::InvalidInput,
            format!("rename target '{new_name}' is a reserved FOL keyword"),
        ));
    }
    Ok(())
}

/// Classify a build.fol receiver variable by scanning its binding site.
fn classify_build_receiver(
    text: &str,
    receiver: &str,
) -> Option<fol_package::BuildSemanticTypeFamily> {
    use fol_package::BuildSemanticTypeFamily as Family;

    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("var ")
            .or_else(|| trimmed.strip_prefix("con "))
        else {
            continue;
        };
        let Some((name, value)) = rest.split_once('=') else {
            continue;
        };
        let name = name.split(':').next().unwrap_or("").trim();
        if name != receiver {
            continue;
        }
        let value = value.trim();
        if value.starts_with(".build()") {
            return Some(if value.contains(".graph()") {
                Family::Graph
            } else {
                Family::BuildContext
            });
        }
        if value.contains(".graph()") {
            return Some(Family::Graph);
        }
        if value.contains(".add_dep(") {
            return Some(Family::DependencyHandle);
        }
        if value.contains(".add_exe(")
            || value.contains(".add_static_lib(")
            || value.contains(".add_shared_lib(")
            || value.contains(".add_test(")
        {
            return Some(Family::ArtifactHandle);
        }
        if value.contains(".step(") {
            return Some(Family::StepHandle);
        }
        if value.contains(".add_run(") {
            return Some(Family::RunHandle);
        }
        if value.contains(".install(")
            || value.contains(".install_file(")
            || value.contains(".install_dir(")
        {
            return Some(Family::InstallHandle);
        }
    }
    None
}

fn origin_contains(origin: &fol_parser::ast::SyntaxOrigin, line: usize, column: usize) -> bool {
    let start_line = origin.line;
    let start_column = origin.column;
    let end_column = start_column + origin.length.max(1);
    line == start_line && column >= start_column && column <= end_column
}

/// Heuristic: does the binding whose name ends at `origin` carry an explicit
/// `: type` annotation? Used to suppress redundant inlay type hints.
fn binding_has_explicit_annotation(text: &str, origin: &fol_parser::ast::SyntaxOrigin) -> bool {
    let Some(line) = text.lines().nth(origin.line.saturating_sub(1)) else {
        return false;
    };
    let after = origin.column.saturating_sub(1) + origin.length;
    let chars: Vec<char> = line.chars().collect();
    if after > chars.len() {
        return false;
    }
    chars[after..]
        .iter()
        .find(|c| !c.is_whitespace())
        .map(|c| *c == ':')
        .unwrap_or(false)
}

/// Completion-item resolution: enrich a keyword item's detail with its lexer
/// category. Symbol items already carry an authoritative detail and pass
/// through unchanged.
pub(super) fn resolve_completion_item(mut item: EditorCompletionItem) -> EditorCompletionItem {
    if item.kind == 14 {
        if let Some(category) = keyword_category(&item.label) {
            item.detail = Some(format!("{category} keyword"));
        }
    }
    item
}

fn keyword_category(keyword: &str) -> Option<&'static str> {
    use fol_lexer::token::buildin::{
        CONTROL_KEYWORDS, DECLARATION_KEYWORDS, DIAGNOSTIC_KEYWORDS, LITERAL_KEYWORDS,
    };
    if DECLARATION_KEYWORDS.contains(&keyword) {
        Some("declaration")
    } else if CONTROL_KEYWORDS.contains(&keyword) {
        Some("control-flow")
    } else if LITERAL_KEYWORDS.contains(&keyword) {
        Some("literal")
    } else if DIAGNOSTIC_KEYWORDS.contains(&keyword) {
        Some("diagnostic")
    } else {
        None
    }
}

fn pointer_type_info(
    program: &fol_typecheck::TypedProgram,
    mut type_id: fol_typecheck::CheckedTypeId,
) -> Option<(fol_typecheck::CheckedTypeId, bool, bool)> {
    let mut borrowed = false;
    loop {
        match program.type_table().get(type_id)? {
            fol_typecheck::CheckedType::Owned { inner } => type_id = *inner,
            fol_typecheck::CheckedType::Borrowed { inner, .. } => {
                borrowed = true;
                type_id = *inner;
            }
            fol_typecheck::CheckedType::Pointer { target, shared, .. } => {
                return Some((*target, *shared, borrowed));
            }
            _ => return None,
        }
    }
}

/// Resolve a checked type id to the declaring symbol of the named type it
/// denotes, unwrapping optional/error/container shells (`opt Foo`, `vec[Foo]`).
/// Returns `None` for anonymous/structural or builtin types.
fn declared_type_symbol(
    table: &fol_typecheck::TypeTable,
    type_id: fol_typecheck::CheckedTypeId,
) -> Option<fol_resolver::SymbolId> {
    use fol_typecheck::{CheckedType, DeclaredTypeKind};
    match table.get(type_id)? {
        CheckedType::Declared {
            symbol,
            kind: DeclaredTypeKind::Type | DeclaredTypeKind::Alias,
            ..
        } => Some(*symbol),
        CheckedType::Optional { inner } => declared_type_symbol(table, *inner),
        CheckedType::Owned { inner } | CheckedType::Borrowed { inner, .. } => {
            declared_type_symbol(table, *inner)
        }
        CheckedType::Pointer { target, .. } => declared_type_symbol(table, *target),
        CheckedType::Error { inner } => inner.and_then(|inner| declared_type_symbol(table, inner)),
        CheckedType::Vector { element_type }
        | CheckedType::Sequence { element_type }
        | CheckedType::Array { element_type, .. }
        | CheckedType::Channel { element_type }
        | CheckedType::ChannelSender { element_type } => declared_type_symbol(table, *element_type),
        CheckedType::Eventual { value_type, .. } => declared_type_symbol(table, *value_type),
        _ => None,
    }
}

/// Scan matched `{ .. }` brace pairs, returning each pair's open and close
/// positions. Compiler-recognized comments and quoted forms are excluded by
/// the shared source scanner. Used for folding and syntax-aware selection.
fn scan_brace_blocks(text: &str) -> Vec<(LspPosition, LspPosition)> {
    let mut blocks = Vec::new();
    let mut stack: Vec<LspPosition> = Vec::new();
    for event in crate::source_scan::brace_events(text) {
        match event.kind {
            crate::source_scan::BraceKind::Open => stack.push(event.position),
            crate::source_scan::BraceKind::Close => {
                if let Some(open) = stack.pop() {
                    blocks.push((open, event.position));
                }
            }
        }
    }
    blocks
}

fn document_full_range(text: &str) -> LspRange {
    let last_line = text.lines().count().saturating_sub(1) as u32;
    let last_len = text
        .lines()
        .last()
        .map(|line| line.chars().count())
        .unwrap_or(0) as u32;
    LspRange {
        start: LspPosition {
            line: 0,
            character: 0,
        },
        end: LspPosition {
            line: last_line,
            character: last_len,
        },
    }
}

fn range_contains_position(range: &LspRange, position: LspPosition) -> bool {
    let after_start =
        (position.line, position.character) >= (range.start.line, range.start.character);
    let before_end = (position.line, position.character) <= (range.end.line, range.end.character);
    after_start && before_end
}

/// The identifier word range surrounding `position`, if the cursor is on one.
fn word_range_at(text: &str, position: LspPosition) -> Option<LspRange> {
    let line = text.lines().nth(position.line as usize)?;
    let chars: Vec<char> = line.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let cursor = (position.character as usize).min(chars.len());
    let mut start = cursor;
    while start > 0 && is_word(chars[start - 1]) {
        start -= 1;
    }
    let mut end = cursor;
    while end < chars.len() && is_word(chars[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    Some(LspRange {
        start: LspPosition {
            line: position.line,
            character: start as u32,
        },
        end: LspPosition {
            line: position.line,
            character: end as u32,
        },
    })
}

/// Build a selection-range chain from ranges ordered innermost -> outermost.
fn build_selection_chain(ranges: &[LspRange]) -> LspSelectionRange {
    let mut chain: Option<LspSelectionRange> = None;
    for range in ranges.iter().rev() {
        if chain.as_ref().map(|current| &current.range) == Some(range) {
            continue;
        }
        chain = Some(LspSelectionRange {
            range: *range,
            parent: chain.map(Box::new),
        });
    }
    chain.unwrap_or(LspSelectionRange {
        range: ranges.first().copied().unwrap_or(LspRange {
            start: LspPosition {
                line: 0,
                character: 0,
            },
            end: LspPosition {
                line: 0,
                character: 0,
            },
        }),
        parent: None,
    })
}

fn offset_for_origin(text: &str, origin: &fol_parser::ast::SyntaxOrigin) -> Option<usize> {
    offset_for_position(
        text,
        LspPosition {
            line: origin.line.saturating_sub(1) as u32,
            character: origin.column.saturating_sub(1) as u32,
        },
    )
}

fn offset_for_position(text: &str, position: LspPosition) -> Option<usize> {
    crate::positions::scalar_position_to_offset(text, position)
}

fn find_call_open_paren(text: &str, callee_end: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut cursor = callee_end;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'(' => return Some(cursor),
            b' ' | b'\t' | b'\r' | b'\n' => cursor += 1,
            _ => return None,
        }
    }
    None
}

fn find_matching_paren(text: &str, open_paren: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = open_paren;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            b'"' => {
                index += 1;
                while index < bytes.len() {
                    match bytes[index] {
                        b'\\' => index += 2,
                        b'"' => break,
                        _ => index += 1,
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn active_parameter_index(text: &str, open_paren: usize, cursor_offset: usize) -> usize {
    if cursor_offset <= open_paren + 1 {
        return 0;
    }
    let bytes = text.as_bytes();
    let limit = cursor_offset.min(bytes.len());
    let mut depth = 0usize;
    let mut index = open_paren + 1;
    let mut parameter = 0usize;
    while index < limit {
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
            }
            b',' if depth == 0 => parameter += 1,
            b'"' => {
                index += 1;
                while index < limit {
                    match bytes[index] {
                        b'\\' => index += 2,
                        b'"' => break,
                        _ => index += 1,
                    }
                }
            }
            _ => {}
        }
        index += 1;
    }
    parameter
}

fn render_signature_label(
    name: &str,
    parameters: &[String],
    return_type: Option<String>,
    error_type: Option<String>,
) -> String {
    let params = parameters.join(", ");
    match (return_type, error_type) {
        (Some(returns), Some(errors)) => format!("{name}({params}): {returns} / {errors}"),
        (Some(returns), None) => format!("{name}({params}): {returns}"),
        (None, Some(errors)) => format!("{name}({params}) / {errors}"),
        (None, None) => format!("{name}({params})"),
    }
}

fn ranges_overlap(left: LspRange, right: LspRange) -> bool {
    range_start(left) <= range_end(right) && range_start(right) <= range_end(left)
}

fn range_start(range: LspRange) -> (u32, u32) {
    (range.start.line, range.start.character)
}

fn range_end(range: LspRange) -> (u32, u32) {
    (range.end.line, range.end.character)
}

fn semantic_token_type_for_symbol_kind(kind: fol_resolver::SymbolKind) -> Option<u32> {
    match kind {
        fol_resolver::SymbolKind::ImportAlias => Some(0),
        fol_resolver::SymbolKind::Type | fol_resolver::SymbolKind::Alias => Some(1),
        fol_resolver::SymbolKind::Routine => Some(2),
        fol_resolver::SymbolKind::Parameter | fol_resolver::SymbolKind::GenericParameter => Some(3),
        fol_resolver::SymbolKind::ValueBinding
        | fol_resolver::SymbolKind::LabelBinding
        | fol_resolver::SymbolKind::DestructureBinding
        | fol_resolver::SymbolKind::Capture
        | fol_resolver::SymbolKind::LoopBinder
        | fol_resolver::SymbolKind::RollingBinder
        | fol_resolver::SymbolKind::Definition => Some(4),
        fol_resolver::SymbolKind::Segment | fol_resolver::SymbolKind::Standard => None,
    }
}

fn document_symbol_key(symbol: &LspDocumentSymbol) -> (String, u8, u32, u32) {
    (
        symbol.name.clone(),
        symbol.kind,
        symbol.selection_range.start.line,
        symbol.selection_range.start.character,
    )
}

fn syntax_document_symbols(
    program: &fol_resolver::ResolvedProgram,
    source_unit: &fol_parser::ast::ParsedSourceUnit,
) -> Vec<LspDocumentSymbol> {
    let mut symbols = Vec::new();
    for item in &source_unit.items {
        collect_syntax_document_symbols(program, &item.node, &mut symbols);
    }
    symbols
}

fn collect_syntax_document_symbols(
    program: &fol_resolver::ResolvedProgram,
    node: &AstNode,
    symbols: &mut Vec<LspDocumentSymbol>,
) {
    if let Some(symbol) = syntax_document_symbol_for_node(program, node) {
        symbols.push(symbol);
    }

    for child in node.children() {
        collect_syntax_document_symbols(program, child, symbols);
    }
}

fn syntax_document_symbol_for_node(
    program: &fol_resolver::ResolvedProgram,
    node: &AstNode,
) -> Option<LspDocumentSymbol> {
    let (name, kind, syntax_id) = match node {
        AstNode::FunDecl {
            name, syntax_id, ..
        }
        | AstNode::ProDecl {
            name, syntax_id, ..
        }
        | AstNode::LogDecl {
            name, syntax_id, ..
        } => (name.clone(), 12, (*syntax_id)?),
        _ => return None,
    };
    let selection_origin = program.syntax_index().origin(syntax_id)?.clone();
    let selection_range = location_to_range(&fol_diagnostics::DiagnosticLocation {
        file: selection_origin.file.clone(),
        line: selection_origin.line,
        column: selection_origin.column,
        length: Some(selection_origin.length),
    });
    Some(LspDocumentSymbol {
        name,
        kind,
        range: expanded_node_range(program, node, &selection_origin),
        selection_range,
        children: Vec::new(),
    })
}

fn expanded_node_range(
    program: &fol_resolver::ResolvedProgram,
    node: &AstNode,
    origin: &fol_parser::ast::SyntaxOrigin,
) -> LspRange {
    let mut range = location_to_range(&fol_diagnostics::DiagnosticLocation {
        file: origin.file.clone(),
        line: origin.line,
        column: origin.column,
        length: Some(origin.length),
    });
    for child in node.children() {
        let child_range = max_range_for_node(program, child);
        if range_end(child_range) > range_end(range) {
            range.end = child_range.end;
        }
    }
    range
}

fn max_range_for_node(program: &fol_resolver::ResolvedProgram, node: &AstNode) -> LspRange {
    let mut best = node
        .syntax_id()
        .and_then(|syntax_id| program.syntax_index().origin(syntax_id))
        .map(|origin| {
            location_to_range(&fol_diagnostics::DiagnosticLocation {
                file: origin.file.clone(),
                line: origin.line,
                column: origin.column,
                length: Some(origin.length),
            })
        })
        .unwrap_or(LspRange {
            start: LspPosition {
                line: 0,
                character: 0,
            },
            end: LspPosition {
                line: 0,
                character: 0,
            },
        });
    for child in node.children() {
        let child_range = max_range_for_node(program, child);
        if range_end(child_range) > range_end(best) {
            best.end = child_range.end;
        }
    }
    best
}

fn nest_document_symbols(symbols: Vec<LspDocumentSymbol>) -> Vec<LspDocumentSymbol> {
    fn insert(into: &mut Vec<LspDocumentSymbol>, symbol: LspDocumentSymbol) {
        if let Some(parent) = into
            .iter_mut()
            .rev()
            .find(|candidate| range_contains(&candidate.range, &symbol.range))
        {
            insert(&mut parent.children, symbol);
        } else {
            into.push(symbol);
        }
    }

    let mut nested = Vec::new();
    for symbol in symbols {
        insert(&mut nested, symbol);
    }
    nested
}

fn range_contains(parent: &LspRange, child: &LspRange) -> bool {
    let parent_start = (parent.start.line, parent.start.character);
    let parent_end = (parent.end.line, parent.end.character);
    let child_start = (child.start.line, child.start.character);
    let child_end = (child.end.line, child.end.character);

    parent_start <= child_start
        && child_end <= parent_end
        && (parent_start != child_start || parent_end != child_end)
}

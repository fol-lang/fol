use crate::{
    diagnostic_to_lsp, location_to_range, EditorDocument, EditorError, EditorErrorKind,
    EditorResult, LspDiagnostic, LspLocation, LspPosition, LspRange, LspTextEdit,
    LspWorkspaceEdit,
};
use fol_intrinsics::IntrinsicSurface;
use fol_parser::ast::{AstNode, ParsedSourceUnitKind, SyntaxNodeId};
use fol_typecheck::{
    editor_builtin_type_names, editor_container_type_names, editor_implemented_intrinsics,
    editor_intrinsic_available_in_model, editor_shell_type_names,
    editor_type_family_available_in_model, EditorTypeFamily, TypecheckCapabilityModel,
};
use std::path::Path;
use std::path::PathBuf;

use super::completion_helpers::{
    completion_builtin_type_item,
    completion_intrinsic_item, completion_item_from_symbol, completion_namespace_item,
    completion_symbol_is_plain_top_level_candidate, completion_symbol_is_root_visible,
    current_routine_name, dedupe_completion_items, fallback_decl_name,
    fallback_items_from_package_dir, position_to_offset, render_checked_type, render_symbol_kind,
    symbol_kind_code, symbol_visibility_matches_namespace_root, mark_fallback_completion_items,
    CompletionContext,
    FALLBACK_ALIAS_PREFIXES, FALLBACK_ROUTINE_PREFIXES, FALLBACK_TYPE_PREFIXES,
};
use super::types::{
    EditorCompletionItem, LspDocumentSymbol, LspHover, LspParameterInformation, LspSignatureHelp,
    LspSignatureInformation, LspWorkspaceSymbol,
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
        let declared_type = typed_package.program.typed_symbol(symbol_id)?.declared_type?;
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
            program.symbol(symbol_id).map(|symbol| symbol.name.as_str()).unwrap_or(&call_site.display_name),
            &parameters,
            signature.return_type.map(|type_id| {
                render_checked_type(typed_package.program.type_table(), type_id)
            }),
            signature.error_type.map(|type_id| {
                render_checked_type(typed_package.program.type_table(), type_id)
            }),
        );
        let active_parameter = if parameters.is_empty() {
            None
        } else {
            Some(call_site.active_parameter.min(parameters.len().saturating_sub(1)) as u32)
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
                let suggestion = diagnostic
                    .suggestions
                    .iter()
                    .find(|suggestion| suggestion.replacement.is_some() && suggestion.location.is_some())?;
                let suggestion_location = suggestion.location.as_ref()?;
                if suggestion_location.file.as_deref()? != path_text {
                    return None;
                }
                let mut changes = std::collections::BTreeMap::new();
                changes.insert(
                    uri.to_string(),
                    vec![LspTextEdit {
                        range: location_to_range(suggestion_location),
                        new_text: suggestion.replacement.clone().expect("replacement checked above"),
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
            let Some(origin) = symbol.origin.as_ref() else { continue };
            let Some(file) = origin.file.as_ref() else { continue };
            if file != &path_text {
                continue;
            }
            // Parameters borrow the routine header origin, so a symbol-site
            // token would repaint the routine name; their uses still get
            // tokens through resolved references below.
            if matches!(symbol.kind, fol_resolver::SymbolKind::Parameter) {
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
            let Some(syntax_id) = reference.syntax_id else { continue };
            let Some(origin) = program.syntax_index().origin(syntax_id) else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else { continue };
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
            let delta_line = if index == 0 { line } else { line - previous_line };
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

        for package in resolved.packages() {
            if !matches!(
                package.identity.source_kind,
                fol_resolver::PackageSourceKind::Entry | fol_resolver::PackageSourceKind::Local
            ) {
                continue;
            }
            let package_root = Path::new(&package.prepared.identity.canonical_root);

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
                if !Path::new(file).starts_with(package_root) {
                    continue;
                }
                let Some(source_unit) = package.program.source_units.get(symbol.source_unit) else {
                    continue;
                };
                if source_unit.kind != ParsedSourceUnitKind::Ordinary {
                    continue;
                }

                let package_root_namespace = source_unit.namespace == package.identity.display_name
                    || source_unit.namespace
                        == format!("{}::src", package.identity.display_name);
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
                let container_name =
                    Some(format!("{container_namespace} ({})", package.identity.display_name));
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
                .then(left.location.range.start.line.cmp(&right.location.range.start.line))
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
        match context {
            CompletionContext::Plain => {}
            CompletionContext::TypePosition => {
                let mut items = self.type_surface_completion_items();
                items.extend(self.visible_named_type_completion_items());
                return dedupe_completion_items(items);
            }
            CompletionContext::QualifiedPath { qualifier } => {
                let items = self.qualified_completion_items(&qualifier);
                if items.is_empty() {
                    return self.fallback_qualified_completion_items(&qualifier);
                }
                return items;
            }
            CompletionContext::DotTrigger => return self.dot_intrinsic_fallback_completion_items(),
        }
        let mut items = self.local_completion_items(position);
        items.extend(self.current_package_top_level_completion_items());
        items.extend(self.import_alias_completion_items(position));
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
        let push_family = |target: BuildSemanticTypeFamily, items: &mut Vec<EditorCompletionItem>| {
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
        match context {
            CompletionContext::DotTrigger => self.dot_intrinsic_fallback_completion_items(),
            CompletionContext::QualifiedPath { qualifier } => {
                self.fallback_qualified_completion_items(&qualifier)
            }
            CompletionContext::TypePosition => {
                let mut items = self.type_surface_completion_items();
                items.extend(self.fallback_local_named_type_items(document));
                items.extend(self.fallback_imported_named_type_items(document));
                dedupe_completion_items(items)
            }
            CompletionContext::Plain => {
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
                dedupe_completion_items(items)
            }
        }
    }

    fn active_model(&self) -> TypecheckCapabilityModel {
        self.active_fol_model.unwrap_or_default()
    }

    fn type_surface_completion_items(&self) -> Vec<EditorCompletionItem> {
        let model = self.active_model();
        let mut items = editor_builtin_type_names()
            .iter()
            .filter(|name| {
                editor_type_family_available_in_model(model, builtin_type_family(name))
            })
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
                .filter(|name| editor_type_family_available_in_model(model, shell_type_family(name)))
                .map(|name| completion_builtin_type_item(name)),
        );
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
    fn current_package_top_level_completion_items(
        &self,
    ) -> Vec<EditorCompletionItem> {
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
        let before_cursor = &document.text[..offset];
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

        for line in document.text.lines() {
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
        for line in document.text.lines() {
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
            item.detail.as_deref() == Some("type") || item.detail.as_deref() == Some("type alias")
        })
        .collect())
    }

    // FALLBACK: text-matches `use ` prefix to find import aliases
    fn fallback_import_alias_items(&self, document: &EditorDocument) -> Vec<EditorCompletionItem> {
        document
            .text
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
        let rel_path = text.lines().find_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix("use ")?;
            let (alias, rhs) = rest.split_once(':')?;
            (alias.trim() == root_alias).then_some(rhs.trim().to_string())
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
            package_root.join(".fol/pkg").join(import_target)
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
    ) -> Option<(&fol_resolver::ResolvedPackage, &fol_resolver::ResolvedProgram)> {
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

    // COMPILER-BACKED: resolved symbol + typed type (no text fallback)
    pub(super) fn hover_for_symbol(
        &self,
        symbol_id: fol_resolver::SymbolId,
    ) -> Option<LspHover> {
        let (package, program) = self.current_resolved_package()?;
        let symbol = program.symbol(symbol_id)?;
        // Local bindings may not carry a declaration origin yet; hover still
        // has useful kind/name/type content without a highlight range.
        let origin = symbol.origin.as_ref();
        let type_summary = self
            .typed_workspace
            .as_ref()
            .and_then(|typed| typed.package(&package.identity))
            .and_then(|typed_package| typed_package.program.typed_symbol(symbol_id))
            .and_then(|typed_symbol| typed_symbol.declared_type)
            .map(|type_id| {
                let typed_package = self
                    .typed_workspace
                    .as_ref()
                    .and_then(|typed| typed.package(&package.identity))
                    .expect("typed package should exist when declared type is available");
                render_checked_type(typed_package.program.type_table(), type_id)
            })
            .unwrap_or_else(|| "unknown".to_string());
        Some(LspHover {
            contents: format!(
                "{} {}: {}",
                render_symbol_kind(symbol.kind),
                symbol.name,
                type_summary
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
    ) -> Option<LspHover> {
        self.hover_for_symbol(reference.resolved?)
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

        for hit in program.all_references().filter(|hit| hit.resolved == Some(symbol_id)) {
            let Some(syntax_id) = hit.syntax_id else { continue };
            let Some(origin) = program.syntax_index().origin(syntax_id) else {
                continue;
            };
            let Some(file) = origin.file.as_ref() else { continue };
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
        let analyzed_package_root = self.analyzed_package_root.as_ref();

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

            if !matches!(
                symbol.kind,
                fol_resolver::SymbolKind::ValueBinding
                    | fol_resolver::SymbolKind::LabelBinding
                    | fol_resolver::SymbolKind::DestructureBinding
                    | fol_resolver::SymbolKind::Routine
                    | fol_resolver::SymbolKind::Type
                    | fol_resolver::SymbolKind::Alias
                    | fol_resolver::SymbolKind::Definition
                    | fol_resolver::SymbolKind::Parameter
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

            let supports_current_package_top_level = matches!(
                symbol.kind,
                fol_resolver::SymbolKind::Routine
                    | fol_resolver::SymbolKind::Type
                    | fol_resolver::SymbolKind::Alias
                    | fol_resolver::SymbolKind::Definition
            );
            let path_is_in_current_package = |file: &str| {
                analyzed_package_root
                    .map(|root| Path::new(file).starts_with(root))
                    .unwrap_or(false)
            };
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
            if declaration_file != &analyzed_path_text
                && !(supports_current_package_top_level
                    && path_is_in_current_package(declaration_file))
            {
                return Err(EditorError::new(
                    EditorErrorKind::InvalidInput,
                    "rename currently refuses multi-package symbols",
                ));
            }
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

            for hit in program.all_references().filter(|hit| hit.resolved == Some(symbol_id)) {
                let Some(syntax_id) = hit.syntax_id else { continue };
                let Some(origin) = program.syntax_index().origin(syntax_id) else {
                    continue;
                };
                let Some(file) = origin.file.as_ref() else { continue };
                if file != &analyzed_path_text
                    && !(supports_current_package_top_level && path_is_in_current_package(file))
                {
                    return Err(EditorError::new(
                        EditorErrorKind::InvalidInput,
                        "rename currently refuses multi-package symbols",
                    ));
                }
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
    ) -> Option<(&fol_resolver::ResolvedProgram, &fol_parser::ast::ParsedSourceUnit)> {
        let (_, program) = self.current_resolved_package()?;
        let resolved_unit = self.current_source_unit(program)?;
        let syntax_unit = program.syntax().source_units.get(resolved_unit.id.0)?;
        Some((program, syntax_unit))
    }
}

#[cfg(test)]
mod tests {
    use super::SemanticSnapshot;
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
            items.iter()
                .find(|item| item.label == "LocalRec")
                .and_then(|item| item.detail.as_deref()),
            Some("type (fallback)")
        );
        assert_eq!(
            items.iter()
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
            items.iter()
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
                line: 5,
                character: 12,
            },
        );
        assert_eq!(
            items.iter()
                .find(|item| item.label == "value")
                .and_then(|item| item.detail.as_deref()),
            Some("binding")
        );

        fs::remove_dir_all(root).ok();
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
        AstNode::QualifiedFunctionCall { path: qualified, .. } => {
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
    if let Some(reference) = program
        .all_references()
        .find(|reference| reference.syntax_id == Some(syntax_id))
    {
        return Some(reference.clone());
    }
    let origin = program.syntax_index().origin(syntax_id)?;
    let file = origin.file.as_deref()?;
    let line = origin.line;
    let column = origin.column;
    let mut best_reference: Option<(&fol_resolver::ResolvedReference, usize)> = None;
    for reference in program.all_references() {
        let Some(reference_syntax_id) = reference.syntax_id else {
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
        let Some(syntax_id) = reference.syntax_id else {
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
        name: symbol.name.clone(),
        scope: symbol.scope,
        source_unit: symbol.source_unit,
        resolved: Some(symbol.id),
    })
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
            .or_else(|| trimmed.strip_prefix("let "))
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
        if value.contains(".install(") || value.contains(".install_file(") || value.contains(".install_dir(") {
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
    let mut offset = 0usize;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        if line_index as u32 == position.line {
            let line_text = line.strip_suffix('\n').unwrap_or(line);
            return if position.character as usize <= line_text.len() {
                Some(offset + position.character as usize)
            } else if position.character as usize == line.len() {
                Some(offset + line.len())
            } else {
                None
            };
        }
        offset += line.len();
    }
    if position.line == text.lines().count() as u32 {
        Some(text.len())
    } else {
        None
    }
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
        fol_resolver::SymbolKind::Segment
        | fol_resolver::SymbolKind::Standard => None,
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
        AstNode::FunDecl { name, syntax_id, .. }
        | AstNode::ProDecl { name, syntax_id, .. }
        | AstNode::LogDecl { name, syntax_id, .. } => {
            (name.clone(), 12, (*syntax_id)?)
        }
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

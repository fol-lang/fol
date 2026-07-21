use crate::ids::{IdTable, ImportId, ReferenceId, ScopeId, SourceUnitId, SymbolId};
use crate::session::{LoadedPackage, PackageIdentity};
use crate::{ResolverError, ResolverErrorKind};
use fol_package::PreparedPackage;
use fol_parser::ast::{
    FolType, ParsedDeclScope, ParsedDeclVisibility, ParsedPackage, ParsedSourceUnitKind,
    SyntaxIndex, SyntaxNodeId, SyntaxOrigin, UsePathSegment,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeKind {
    ProgramRoot {
        package: String,
    },
    NamespaceRoot {
        namespace: String,
    },
    SourceUnitRoot {
        path: String,
    },
    /// Virtual scope injected as the parent of every `build.fol` source-unit scope.
    /// It holds synthetic handle symbols for the build stdlib (ArtifactHandle, …)
    /// and has no parent, providing file-bound isolation for `build.fol`.
    BuildStdlib,
    StandardDeclaration,
    Routine,
    TypeDeclaration,
    Block,
    LoopBinder,
    RollingBinder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSourceUnit {
    pub id: SourceUnitId,
    pub path: String,
    pub package: String,
    pub namespace: String,
    pub kind: ParsedSourceUnitKind,
    pub scope_id: ScopeId,
    pub top_level_nodes: Vec<SyntaxNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedScope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub source_unit: Option<SourceUnitId>,
    pub symbols: Vec<SymbolId>,
    pub symbol_keys: BTreeMap<String, Vec<SymbolId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    ValueBinding,
    LabelBinding,
    DestructureBinding,
    Routine,
    Type,
    Alias,
    Definition,
    Segment,
    Standard,
    ImportAlias,
    GenericParameter,
    Parameter,
    Capture,
    LoopBinder,
    RollingBinder,
}

impl SymbolKind {
    /// Human-readable display name for editor tooling.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Alias => "alias",
            Self::Routine => "routine",
            Self::Definition => "definition",
            Self::ValueBinding | Self::LabelBinding | Self::DestructureBinding => "binding",
            Self::Parameter => "parameter",
            Self::Capture => "capture",
            Self::ImportAlias => "namespace",
            Self::Segment => "segment",
            Self::Standard => "standard",
            Self::GenericParameter => "parameter",
            Self::LoopBinder | Self::RollingBinder => "binding",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceKind {
    Identifier,
    FunctionCall,
    QualifiedIdentifier,
    QualifiedFunctionCall,
    TypeName,
    QualifiedTypeName,
    InquiryTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedSymbolProvenance {
    pub package_identity: PackageIdentity,
    pub foreign_symbol: SymbolId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSymbol {
    pub id: SymbolId,
    pub name: String,
    pub canonical_name: String,
    pub duplicate_key: String,
    pub kind: SymbolKind,
    pub scope: ScopeId,
    pub source_unit: SourceUnitId,
    pub origin: Option<SyntaxOrigin>,
    pub visibility: Option<ParsedDeclVisibility>,
    pub declaration_scope: Option<ParsedDeclScope>,
    pub mounted_from: Option<MountedSymbolProvenance>,
    /// True for `var[mut]`/`lab[mut]` bindings. Bindings are immutable by
    /// default (see the variables chapter), and this drives assignment-target
    /// legality (e.g. field assignment requires a mutable record binding).
    pub is_mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReference {
    pub id: ReferenceId,
    pub kind: ReferenceKind,
    pub syntax_id: Option<SyntaxNodeId>,
    /// Precise use-site anchor when it differs from `syntax_id` (for
    /// qualified references, the final path segment). `syntax_id` stays the
    /// AST linkage key consumed by typecheck/lowering.
    pub anchor_syntax_id: Option<SyntaxNodeId>,
    pub name: String,
    pub scope: ScopeId,
    pub source_unit: SourceUnitId,
    pub resolved: Option<SymbolId>,
}

impl ResolvedReference {
    /// The syntax node an editor should treat as this reference's use site.
    pub fn anchor(&self) -> Option<SyntaxNodeId> {
        self.anchor_syntax_id.or(self.syntax_id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedImport {
    pub id: ImportId,
    pub alias_symbol: SymbolId,
    pub alias_name: String,
    pub path_type: FolType,
    pub import_target: String,
    pub path_segments: Vec<UsePathSegment>,
    pub scope: ScopeId,
    pub source_unit: SourceUnitId,
    pub target_scope: Option<ScopeId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPackage {
    pub identity: PackageIdentity,
    pub prepared: PreparedPackage,
    pub program: ResolvedProgram,
}

impl ResolvedPackage {
    pub(crate) fn from_loaded(loaded: LoadedPackage) -> Self {
        Self {
            identity: loaded.identity,
            prepared: loaded.prepared,
            program: loaded.program,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedWorkspace {
    entry_identity: PackageIdentity,
    packages: BTreeMap<PackageIdentity, ResolvedPackage>,
}

impl ResolvedWorkspace {
    pub(crate) fn new(
        entry_identity: PackageIdentity,
        entry_prepared: PreparedPackage,
        entry_program: ResolvedProgram,
        loaded_packages: impl IntoIterator<Item = LoadedPackage>,
    ) -> Self {
        let mut packages = BTreeMap::new();
        packages.insert(
            entry_identity.clone(),
            ResolvedPackage {
                identity: entry_identity.clone(),
                prepared: entry_prepared,
                program: entry_program,
            },
        );

        for loaded in loaded_packages {
            let package = ResolvedPackage::from_loaded(loaded);
            packages.insert(package.identity.clone(), package);
        }

        Self {
            entry_identity,
            packages,
        }
    }

    pub fn entry_identity(&self) -> &PackageIdentity {
        &self.entry_identity
    }

    pub fn entry_package(&self) -> &ResolvedPackage {
        self.packages
            .get(&self.entry_identity)
            .expect("resolved workspace should always contain the entry package")
    }

    pub fn entry_program(&self) -> &ResolvedProgram {
        &self.entry_package().program
    }

    pub fn package(&self, identity: &PackageIdentity) -> Option<&ResolvedPackage> {
        self.packages.get(identity)
    }

    pub fn packages(&self) -> impl Iterator<Item = &ResolvedPackage> {
        self.packages.values()
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedProgram {
    syntax: ParsedPackage,
    pub program_scope: ScopeId,
    /// The virtual scope that is the parent of every `build.fol` source-unit scope,
    /// providing file-bound isolation. `None` for packages that have no Build source units.
    pub build_stdlib_scope: Option<ScopeId>,
    namespace_scopes: BTreeMap<String, ScopeId>,
    syntax_scopes: BTreeMap<SyntaxNodeId, ScopeId>,
    mounted_package_roots: BTreeMap<String, ScopeId>,
    pub source_units: IdTable<SourceUnitId, ResolvedSourceUnit>,
    pub scopes: IdTable<ScopeId, ResolvedScope>,
    pub symbols: IdTable<SymbolId, ResolvedSymbol>,
    pub references: IdTable<ReferenceId, ResolvedReference>,
    pub imports: IdTable<ImportId, ResolvedImport>,
}

impl ResolvedProgram {
    pub fn new(syntax: ParsedPackage) -> Self {
        let mut scopes = IdTable::new();
        let program_scope = scopes.push(ResolvedScope {
            id: ScopeId(0),
            kind: ScopeKind::ProgramRoot {
                package: syntax.package.clone(),
            },
            parent: None,
            source_unit: None,
            symbols: Vec::new(),
            symbol_keys: BTreeMap::new(),
        });
        if let Some(scope) = scopes.get_mut(program_scope) {
            scope.id = program_scope;
        }

        let mut namespace_scopes = BTreeMap::new();
        namespace_scopes.insert(syntax.package.clone(), program_scope);
        let mut source_units = IdTable::new();

        for source_unit in &syntax.source_units {
            let top_level_nodes = source_unit.items.iter().map(|item| item.node_id).collect();
            let id = source_units.push(ResolvedSourceUnit {
                id: SourceUnitId(0),
                path: source_unit.path.clone(),
                package: source_unit.package.clone(),
                namespace: source_unit.namespace.clone(),
                kind: source_unit.kind,
                scope_id: ScopeId(0),
                top_level_nodes,
            });
            if let Some(slot) = source_units.get_mut(id) {
                slot.id = id;
            }

            ensure_namespace_scope(
                &mut scopes,
                &mut namespace_scopes,
                program_scope,
                &source_unit.package,
                &source_unit.namespace,
            );
        }

        let source_unit_ids = source_units
            .iter_with_ids()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();

        for source_unit_id in source_unit_ids {
            let source_unit = source_units
                .get(source_unit_id)
                .expect("source unit should exist while building source-unit scopes");
            let source_scope = scopes.push(ResolvedScope {
                id: ScopeId(0),
                kind: ScopeKind::SourceUnitRoot {
                    path: source_unit.path.clone(),
                },
                parent: Some(
                    *namespace_scopes
                        .get(&source_unit.namespace)
                        .expect("source unit namespace scope should exist"),
                ),
                source_unit: Some(source_unit_id),
                symbols: Vec::new(),
                symbol_keys: BTreeMap::new(),
            });
            if let Some(scope) = scopes.get_mut(source_scope) {
                scope.id = source_scope;
            }
            if let Some(unit) = source_units.get_mut(source_unit_id) {
                unit.scope_id = source_scope;
            }
        }

        Self {
            syntax,
            program_scope,
            build_stdlib_scope: None,
            namespace_scopes,
            syntax_scopes: BTreeMap::new(),
            mounted_package_roots: BTreeMap::new(),
            source_units,
            scopes,
            symbols: IdTable::new(),
            references: IdTable::new(),
            imports: IdTable::new(),
        }
    }

    /// Creates a `BuildStdlib` scope and wires every `Build` source unit's
    /// `SourceUnitRoot` scope to use it as parent, providing file-bound
    /// isolation for `build.fol`. Returns the scope id if any build units exist.
    pub fn init_build_stdlib_scope(&mut self) -> Option<ScopeId> {
        let build_scopes: Vec<(SourceUnitId, ScopeId)> = self
            .source_units
            .iter_with_ids()
            .filter(|(_, unit)| unit.kind == ParsedSourceUnitKind::Build)
            .map(|(id, unit)| (id, unit.scope_id))
            .collect();

        if build_scopes.is_empty() {
            return None;
        }

        let anchor_unit = build_scopes[0].0;
        let stdlib_scope = self.scopes.push(ResolvedScope {
            id: ScopeId(0),
            kind: ScopeKind::BuildStdlib,
            parent: None,
            source_unit: Some(anchor_unit),
            symbols: Vec::new(),
            symbol_keys: BTreeMap::new(),
        });
        if let Some(scope) = self.scopes.get_mut(stdlib_scope) {
            scope.id = stdlib_scope;
        }

        for (_, source_scope_id) in &build_scopes {
            if let Some(scope) = self.scopes.get_mut(*source_scope_id) {
                scope.parent = Some(stdlib_scope);
            }
        }

        self.build_stdlib_scope = Some(stdlib_scope);
        Some(stdlib_scope)
    }

    pub fn syntax(&self) -> &ParsedPackage {
        &self.syntax
    }

    pub fn syntax_index(&self) -> &SyntaxIndex {
        &self.syntax.syntax_index
    }

    pub fn package_name(&self) -> &str {
        &self.syntax.package
    }

    pub fn scope(&self, id: ScopeId) -> Option<&ResolvedScope> {
        self.scopes.get(id)
    }

    pub fn source_unit(&self, id: SourceUnitId) -> Option<&ResolvedSourceUnit> {
        self.source_units.get(id)
    }

    pub fn ordinary_source_units(&self) -> impl Iterator<Item = &ResolvedSourceUnit> {
        self.source_units
            .iter()
            .filter(|unit| unit.kind == ParsedSourceUnitKind::Ordinary)
    }

    pub fn build_source_units(&self) -> impl Iterator<Item = &ResolvedSourceUnit> {
        self.source_units
            .iter()
            .filter(|unit| unit.kind == ParsedSourceUnitKind::Build)
    }

    pub fn namespace_scope(&self, namespace: &str) -> Option<ScopeId> {
        self.namespace_scopes.get(namespace).copied()
    }

    pub fn scope_for_syntax(&self, syntax_id: SyntaxNodeId) -> Option<ScopeId> {
        self.syntax_scopes.get(&syntax_id).copied()
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&ResolvedSymbol> {
        self.symbols.get(id)
    }

    pub fn reference(&self, id: ReferenceId) -> Option<&ResolvedReference> {
        self.references.get(id)
    }

    pub fn import(&self, id: ImportId) -> Option<&ResolvedImport> {
        self.imports.get(id)
    }

    pub fn all_symbols(&self) -> impl Iterator<Item = &ResolvedSymbol> {
        self.symbols.iter()
    }

    pub fn all_references(&self) -> impl Iterator<Item = &ResolvedReference> {
        self.references.iter()
    }

    pub fn symbols_in_scope(&self, scope_id: ScopeId) -> Vec<&ResolvedSymbol> {
        self.scope(scope_id)
            .map(|scope| {
                scope
                    .symbols
                    .iter()
                    .filter_map(|symbol_id| self.symbol(*symbol_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn symbols_named_in_scope(&self, scope_id: ScopeId, key: &str) -> Vec<&ResolvedSymbol> {
        self.scope(scope_id)
            .and_then(|scope| scope.symbol_keys.get(key))
            .map(|ids| ids.iter().filter_map(|id| self.symbol(*id)).collect())
            .unwrap_or_default()
    }

    pub fn references_in_scope(&self, scope_id: ScopeId) -> Vec<&ResolvedReference> {
        self.references
            .iter()
            .filter(|reference| reference.scope == scope_id)
            .collect()
    }

    pub fn imports_in_scope(&self, scope_id: ScopeId) -> Vec<&ResolvedImport> {
        self.imports
            .iter()
            .filter(|import| import.scope == scope_id)
            .collect()
    }

    pub(crate) fn add_scope(
        &mut self,
        kind: ScopeKind,
        parent: ScopeId,
        source_unit: SourceUnitId,
    ) -> ScopeId {
        let scope_id = self.scopes.push(ResolvedScope {
            id: ScopeId(0),
            kind,
            parent: Some(parent),
            source_unit: Some(source_unit),
            symbols: Vec::new(),
            symbol_keys: BTreeMap::new(),
        });
        if let Some(scope) = self.scopes.get_mut(scope_id) {
            scope.id = scope_id;
        }
        scope_id
    }

    pub(crate) fn record_scope_for_syntax(
        &mut self,
        syntax_id: Option<SyntaxNodeId>,
        scope_id: ScopeId,
    ) {
        if let Some(syntax_id) = syntax_id {
            self.syntax_scopes.insert(syntax_id, scope_id);
        }
    }

    pub(crate) fn mount_loaded_package(
        &mut self,
        loaded: &LoadedPackage,
    ) -> Result<ScopeId, ResolverError> {
        if let Some(scope_id) = self
            .mounted_package_roots
            .get(&loaded.identity.canonical_root)
            .copied()
        {
            return Ok(scope_id);
        }

        let root_name = loaded.identity.display_name.clone();
        if let Some(existing_scope) = self.namespace_scopes.get(&root_name).copied() {
            self.mounted_package_roots
                .insert(loaded.identity.canonical_root.clone(), existing_scope);
            return Ok(existing_scope);
        }

        let source_unit_id = self.source_units.push(ResolvedSourceUnit {
            id: SourceUnitId(0),
            path: loaded.identity.canonical_root.clone(),
            package: root_name.clone(),
            namespace: root_name.clone(),
            kind: ParsedSourceUnitKind::Ordinary,
            scope_id: ScopeId(0),
            top_level_nodes: Vec::new(),
        });
        if let Some(unit) = self.source_units.get_mut(source_unit_id) {
            unit.id = source_unit_id;
        }

        let root_scope = self.scopes.push(ResolvedScope {
            id: ScopeId(0),
            kind: ScopeKind::ProgramRoot {
                package: root_name.clone(),
            },
            parent: None,
            source_unit: Some(source_unit_id),
            symbols: Vec::new(),
            symbol_keys: BTreeMap::new(),
        });
        if let Some(scope) = self.scopes.get_mut(root_scope) {
            scope.id = root_scope;
        }
        if let Some(unit) = self.source_units.get_mut(source_unit_id) {
            unit.scope_id = root_scope;
        }
        self.namespace_scopes.insert(root_name.clone(), root_scope);

        if loaded.identity.source_kind == crate::PackageSourceKind::Package
            && !loaded.prepared.exports.is_empty()
        {
            self.mount_declared_package_exports(loaded, source_unit_id, root_scope, &root_name)?;
            self.mounted_package_roots
                .insert(loaded.identity.canonical_root.clone(), root_scope);
            return Ok(root_scope);
        }

        self.mount_all_exported_package_scopes(loaded, source_unit_id, root_scope, &root_name)?;

        self.mounted_package_roots
            .insert(loaded.identity.canonical_root.clone(), root_scope);
        Ok(root_scope)
    }

    fn mount_declared_package_exports(
        &mut self,
        loaded: &LoadedPackage,
        source_unit_id: SourceUnitId,
        root_scope: ScopeId,
        root_name: &str,
    ) -> Result<(), ResolverError> {
        for (foreign_scope_id, foreign_namespace, foreign_symbols) in
            exported_symbol_scopes(&loaded.program)
        {
            for export in &loaded.prepared.exports {
                if foreign_namespace != export.source_namespace {
                    continue;
                }
                let mounted_namespace = match export.mounted_namespace_suffix.as_deref() {
                    Some(suffix) => format!("{root_name}::{suffix}"),
                    None => root_name.to_string(),
                };
                let mounted_scope = if mounted_namespace == root_name {
                    root_scope
                } else {
                    ensure_namespace_scope(
                        &mut self.scopes,
                        &mut self.namespace_scopes,
                        root_scope,
                        root_name,
                        &mounted_namespace,
                    )
                };
                self.mount_visible_symbols_from_scope(
                    loaded,
                    source_unit_id,
                    foreign_scope_id,
                    foreign_symbols.as_slice(),
                    mounted_scope,
                )?;
            }
        }

        Ok(())
    }

    fn mount_all_exported_package_scopes(
        &mut self,
        loaded: &LoadedPackage,
        source_unit_id: SourceUnitId,
        root_scope: ScopeId,
        root_name: &str,
    ) -> Result<(), ResolverError> {
        let mut mounted_scopes = BTreeMap::new();
        mounted_scopes.insert(loaded.program.program_scope, root_scope);
        let foreign_package_name = loaded.program.package_name().to_string();
        let mut foreign_namespaces = loaded
            .program
            .scopes
            .iter_with_ids()
            .filter_map(|(scope_id, scope)| match &scope.kind {
                ScopeKind::NamespaceRoot { namespace } => Some((scope_id, namespace.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        foreign_namespaces.sort_by_key(|(_, namespace)| namespace.matches("::").count());

        for (foreign_scope_id, foreign_namespace) in foreign_namespaces {
            let mounted_namespace =
                remap_loaded_namespace(&foreign_namespace, &foreign_package_name, root_name);
            let mounted_scope = ensure_namespace_scope(
                &mut self.scopes,
                &mut self.namespace_scopes,
                root_scope,
                root_name,
                &mounted_namespace,
            );
            mounted_scopes.insert(foreign_scope_id, mounted_scope);
        }

        for (foreign_scope_id, _, foreign_symbols) in exported_symbol_scopes(&loaded.program) {
            let Some(mounted_scope) = mounted_scopes.get(&foreign_scope_id).copied() else {
                continue;
            };
            self.mount_visible_symbols_from_scope(
                loaded,
                source_unit_id,
                foreign_scope_id,
                foreign_symbols.as_slice(),
                mounted_scope,
            )?;
        }

        Ok(())
    }

    fn mount_visible_symbols_from_scope(
        &mut self,
        loaded: &LoadedPackage,
        source_unit_id: SourceUnitId,
        _foreign_scope_id: ScopeId,
        foreign_symbols: &[SymbolId],
        mounted_scope: ScopeId,
    ) -> Result<(), ResolverError> {
        for foreign_symbol_id in foreign_symbols {
            let Some(symbol) = loaded.program.symbol(*foreign_symbol_id) else {
                continue;
            };
            // Mount first-party exports only: a symbol that itself arrived
            // through the importee's own imports (`mounted_from` set) must
            // not be re-exported, or any package that imports both the
            // library and the library's dependency collides on the
            // transitive copies.
            if symbol.visibility != Some(ParsedDeclVisibility::Exported)
                || symbol.kind == SymbolKind::ImportAlias
                || symbol.mounted_from.is_some()
            {
                continue;
            }
            self.insert_mounted_symbol(
                mounted_scope,
                source_unit_id,
                symbol.clone(),
                &loaded.identity,
            )?;
        }

        Ok(())
    }

    fn insert_mounted_symbol(
        &mut self,
        scope_id: ScopeId,
        source_unit_id: SourceUnitId,
        symbol: ResolvedSymbol,
        package_identity: &PackageIdentity,
    ) -> Result<SymbolId, ResolverError> {
        let foreign_symbol_id = symbol.id;
        let canonical_name = symbol.canonical_name.clone();
        if let Some(existing) = self
            .scope(scope_id)
            .and_then(|scope| scope.symbol_keys.get(&canonical_name))
            .into_iter()
            .flat_map(|ids| ids.iter())
            .filter_map(|id| self.symbol(*id))
            .find(|existing| existing.duplicate_key == symbol.duplicate_key)
        {
            return Err(ResolverError::new(
                ResolverErrorKind::DuplicateSymbol,
                format!(
                    "mounted imported symbol '{}' conflicts with existing symbol '{}'",
                    symbol.name, existing.name
                ),
            ));
        }

        let symbol_id = self.symbols.push(ResolvedSymbol {
            id: SymbolId(0),
            scope: scope_id,
            source_unit: source_unit_id,
            mounted_from: Some(MountedSymbolProvenance {
                package_identity: package_identity.clone(),
                foreign_symbol: foreign_symbol_id,
            }),
            ..symbol
        });
        if let Some(inserted) = self.symbols.get_mut(symbol_id) {
            inserted.id = symbol_id;
        }

        let Some(scope) = self.scopes.get_mut(scope_id) else {
            return Err(ResolverError::new(
                ResolverErrorKind::Internal,
                format!("mounted symbol target scope {:?} does not exist", scope_id),
            ));
        };
        scope.symbols.push(symbol_id);
        scope
            .symbol_keys
            .entry(canonical_name)
            .or_default()
            .push(symbol_id);

        Ok(symbol_id)
    }
}

#[cfg(test)]
mod tests {
    use super::ResolvedProgram;
    use fol_parser::ast::{ParsedPackage, ParsedSourceUnit, ParsedSourceUnitKind, SyntaxIndex};

    #[test]
    fn resolved_program_keeps_build_source_unit_kinds() {
        let syntax = ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![
                ParsedSourceUnit {
                    path: "build.fol".to_string(),
                    package: "demo".to_string(),
                    namespace: "demo".to_string(),
                    kind: ParsedSourceUnitKind::Build,
                    items: Vec::new(),
                },
                ParsedSourceUnit {
                    path: "src/main.fol".to_string(),
                    package: "demo".to_string(),
                    namespace: "demo::src".to_string(),
                    kind: ParsedSourceUnitKind::Ordinary,
                    items: Vec::new(),
                },
            ],
            syntax_index: SyntaxIndex::default(),
        };

        let resolved = ResolvedProgram::new(syntax);

        assert_eq!(resolved.build_source_units().count(), 1);
        assert_eq!(resolved.ordinary_source_units().count(), 1);
        assert_eq!(
            resolved
                .source_unit(crate::SourceUnitId(0))
                .map(|unit| unit.kind),
            Some(ParsedSourceUnitKind::Build)
        );
        assert_eq!(
            resolved
                .source_unit(crate::SourceUnitId(1))
                .map(|unit| unit.kind),
            Some(ParsedSourceUnitKind::Ordinary)
        );
    }
}

fn remap_loaded_namespace(
    namespace: &str,
    foreign_package_name: &str,
    mounted_root: &str,
) -> String {
    if namespace == foreign_package_name {
        mounted_root.to_string()
    } else if let Some(suffix) = namespace.strip_prefix(&format!("{foreign_package_name}::")) {
        format!("{mounted_root}::{suffix}")
    } else {
        namespace.to_string()
    }
}

fn exported_symbol_scopes(program: &ResolvedProgram) -> Vec<(ScopeId, String, Vec<SymbolId>)> {
    let mut scopes = program
        .scopes
        .iter_with_ids()
        .filter_map(|(scope_id, scope)| {
            namespace_for_export_scope(&scope.kind)
                .map(|namespace| (scope_id, namespace, scope.symbols.clone()))
        })
        .collect::<Vec<_>>();
    scopes.sort_by_key(|(_, namespace, _)| namespace.matches("::").count());
    scopes
}

fn namespace_for_export_scope(kind: &ScopeKind) -> Option<String> {
    match kind {
        ScopeKind::ProgramRoot { package } => Some(package.clone()),
        ScopeKind::NamespaceRoot { namespace } => Some(namespace.clone()),
        _ => None,
    }
}

fn ensure_namespace_scope(
    scopes: &mut IdTable<ScopeId, ResolvedScope>,
    namespace_scopes: &mut BTreeMap<String, ScopeId>,
    program_scope: ScopeId,
    package: &str,
    namespace: &str,
) -> ScopeId {
    if let Some(scope_id) = namespace_scopes.get(namespace) {
        return *scope_id;
    }

    let mut parent_scope = program_scope;
    let mut current_namespace = package.to_string();

    for segment in namespace.split("::").skip(1) {
        current_namespace.push_str("::");
        current_namespace.push_str(segment);

        if let Some(scope_id) = namespace_scopes.get(&current_namespace) {
            parent_scope = *scope_id;
            continue;
        }

        let scope_id = scopes.push(ResolvedScope {
            id: ScopeId(0),
            kind: ScopeKind::NamespaceRoot {
                namespace: current_namespace.clone(),
            },
            parent: Some(parent_scope),
            source_unit: None,
            symbols: Vec::new(),
            symbol_keys: BTreeMap::new(),
        });
        if let Some(scope) = scopes.get_mut(scope_id) {
            scope.id = scope_id;
        }
        namespace_scopes.insert(current_namespace.clone(), scope_id);
        parent_scope = scope_id;
    }

    parent_scope
}

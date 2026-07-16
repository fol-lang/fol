use crate::{
    collect, imports,
    model::{ResolvedProgram, ResolvedWorkspace},
    traverse, ResolverError, ResolverErrorKind, ResolverResult,
};
use fol_package::{effective_std_root, PackageSession, PreparedPackage};
use fol_parser::ast::ParsedPackage;
use std::collections::BTreeMap;
use std::path::Path;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolverConfig {
    pub std_root: Option<String>,
    pub package_store_root: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackageSourceKind {
    Entry,
    Local,
    Standard,
    Package,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackageIdentity {
    pub source_kind: PackageSourceKind,
    pub canonical_root: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LoadedPackage {
    pub identity: PackageIdentity,
    pub prepared: PreparedPackage,
    pub program: ResolvedProgram,
}

#[derive(Debug)]
pub struct ResolverSession {
    config: ResolverConfig,
    package_session: PackageSession,
    loaded_packages: BTreeMap<PackageIdentity, LoadedPackage>,
    pub(crate) loading_stack: Vec<PackageIdentity>,
}

impl ResolverSession {
    pub fn new() -> Self {
        Self::with_config(ResolverConfig::default())
    }

    pub fn with_config(config: ResolverConfig) -> Self {
        let config = ResolverConfig {
            std_root: effective_std_root(config.std_root.as_deref()),
            ..config
        };
        Self {
            package_session: PackageSession::with_config(package_config_from_resolver(&config)),
            config,
            loaded_packages: BTreeMap::new(),
            loading_stack: Vec::new(),
        }
    }

    pub fn config(&self) -> &ResolverConfig {
        &self.config
    }

    pub fn cached_package_count(&self) -> usize {
        self.loaded_packages.len()
    }

    #[cfg(test)]
    pub(crate) fn loading_depth(&self) -> usize {
        self.loading_stack.len()
    }

    pub fn resolve_package(&mut self, syntax: ParsedPackage) -> ResolverResult<ResolvedProgram> {
        let prepared = self
            .package_session
            .prepare_entry_package(syntax)
            .map_err(|error| vec![error.into()])?;
        self.resolve_prepared_package(prepared)
    }

    pub fn resolve_package_workspace(
        &mut self,
        syntax: ParsedPackage,
    ) -> ResolverResult<ResolvedWorkspace> {
        let prepared = self
            .package_session
            .prepare_entry_package(syntax)
            .map_err(|error| vec![error.into()])?;
        self.resolve_prepared_workspace(prepared)
    }

    pub fn resolve_prepared_package(
        &mut self,
        prepared: PreparedPackage,
    ) -> ResolverResult<ResolvedProgram> {
        self.resolve_prepared_workspace(prepared)
            .map(|workspace| workspace.entry_package().program.clone())
    }

    pub fn resolve_prepared_workspace(
        &mut self,
        prepared: PreparedPackage,
    ) -> ResolverResult<ResolvedWorkspace> {
        let entry_identity = resolver_package_identity(&prepared.identity);
        let entry_program =
            self.resolve_parsed_package(prepared.syntax.clone(), Some(entry_identity.clone()))?;

        Ok(ResolvedWorkspace::new(
            entry_identity,
            prepared,
            entry_program,
            self.loaded_packages.values().cloned().collect::<Vec<_>>(),
        ))
    }

    pub(crate) fn resolve_parsed_package(
        &mut self,
        syntax: ParsedPackage,
        current_identity: Option<PackageIdentity>,
    ) -> ResolverResult<ResolvedProgram> {
        if let Some(identity) = current_identity {
            if self.loading_stack.contains(&identity) {
                return Err(vec![self.import_cycle_error(&identity)]);
            }
            self.loading_stack.push(identity);
        }

        let mut program = ResolvedProgram::new(syntax);
        let has_build_units = program.build_source_units().next().is_some();
        let stdlib_scope = program.init_build_stdlib_scope();
        if has_build_units && stdlib_scope.is_none() {
            return Err(vec![ResolverError::new(
                ResolverErrorKind::Internal,
                "build stdlib scope could not be initialized for a package with build units",
            )]);
        }
        crate::inject::inject_build_stdlib_types(&mut program);
        let collected = collect::collect_top_level_symbols(&mut program)
            .and_then(|_| imports::resolve_import_targets(self, &mut program))
            .and_then(|_| traverse::collect_routine_scopes(self, &mut program));

        if !self.loading_stack.is_empty() {
            self.loading_stack.pop();
        }

        collected.map(|_| program)
    }

    pub(crate) fn cached_package(&self, identity: &PackageIdentity) -> Option<&LoadedPackage> {
        self.loaded_packages.get(identity)
    }

    pub(crate) fn cache_package(&mut self, package: LoadedPackage) {
        self.loaded_packages
            .insert(package.identity.clone(), package);
    }

    pub(crate) fn load_package_from_directory(
        &mut self,
        directory: &Path,
        source_kind: PackageSourceKind,
    ) -> Result<LoadedPackage, ResolverError> {
        let prepared = self
            .package_session
            .load_directory_package(directory, package_source_kind(source_kind))?;
        self.load_prepared_package(prepared)
    }

    pub(crate) fn load_package_from_store_target(
        &mut self,
        store_root: &Path,
        import_target: &str,
    ) -> Result<LoadedPackage, ResolverError> {
        let prepared = self
            .package_session
            .load_package_from_store_target(store_root, import_target)?;
        self.load_prepared_package(prepared)
    }

    fn load_prepared_package(
        &mut self,
        prepared: fol_package::PreparedPackage,
    ) -> Result<LoadedPackage, ResolverError> {
        let identity = resolver_package_identity(&prepared.identity);

        if self.loading_stack.contains(&identity) {
            return Err(self.import_cycle_error(&identity));
        }

        if let Some(cached) = self.cached_package(&identity) {
            return Ok(cached.clone());
        }

        self.loading_stack.push(identity.clone());

        let loaded_result: Result<LoadedPackage, ResolverError> = (|| {
            let program = self
                .resolve_parsed_package(prepared.syntax.clone(), None)
                .map_err(|errors| {
                    errors.into_iter().next().unwrap_or_else(|| {
                        ResolverError::new(
                            ResolverErrorKind::Internal,
                            format!(
                                "resolver failed to load package root '{}'",
                                prepared.identity.canonical_root
                            ),
                        )
                    })
                })?;
            Ok(LoadedPackage {
                identity,
                prepared,
                program,
            })
        })();

        self.loading_stack.pop();

        let loaded = loaded_result?;
        self.cache_package(loaded.clone());
        Ok(loaded)
    }

    fn import_cycle_error(&self, next: &PackageIdentity) -> ResolverError {
        let cycle = self
            .loading_stack
            .iter()
            .map(|identity| identity.canonical_root.as_str())
            .chain(std::iter::once(next.canonical_root.as_str()))
            .collect::<Vec<_>>()
            .join(" -> ");
        ResolverError::new(
            ResolverErrorKind::ImportCycle,
            format!("import cycle detected while loading package roots: {cycle}"),
        )
    }
}

impl Default for ResolverSession {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn package_source_kind(
    source_kind: PackageSourceKind,
) -> fol_package::PackageSourceKind {
    match source_kind {
        PackageSourceKind::Entry => fol_package::PackageSourceKind::Entry,
        PackageSourceKind::Local => fol_package::PackageSourceKind::Local,
        PackageSourceKind::Standard => fol_package::PackageSourceKind::Standard,
        PackageSourceKind::Package => fol_package::PackageSourceKind::Package,
    }
}

pub(crate) fn package_config_from_resolver(config: &ResolverConfig) -> fol_package::PackageConfig {
    fol_package::PackageConfig {
        std_root: config.std_root.clone(),
        package_store_root: config.package_store_root.clone(),
        package_cache_root: None,
        package_git_cache_root: None,
    }
}

pub(crate) fn resolver_package_identity(
    identity: &fol_package::PackageIdentity,
) -> PackageIdentity {
    PackageIdentity {
        source_kind: match identity.source_kind {
            fol_package::PackageSourceKind::Entry => PackageSourceKind::Entry,
            fol_package::PackageSourceKind::Local => PackageSourceKind::Local,
            fol_package::PackageSourceKind::Standard => PackageSourceKind::Standard,
            fol_package::PackageSourceKind::Package => PackageSourceKind::Package,
        },
        canonical_root: identity.canonical_root.clone(),
        display_name: identity.display_name.clone(),
    }
}

//! Whole-program name resolution for parsed FOL packages.
//!
//! Current scope:
//! - build a resolver-owned scope graph from `fol-parser` package output
//! - collect top-level declarations across package, namespace, and file scopes
//! - resolve plain and qualified value, call, type, and inquiry references
//! - expose imported exported names through plain lookup after supported imports
//! - resolve `use loc` imports against the loaded source set
//! - resolve dependency-rooted `use pkg` imports, including bundled std aliases
//! - permit explicit std-root overrides for development/testing
//! - resolve `use pkg` imports against installed `build.fol` package roots
//! - mount only `build.fol`-declared export roots for consumer-visible `pkg` imports
//! - keep file-private `hid` visibility inside the owning source unit only
//! - treat built-in `str` as a builtin instead of a user-defined type lookup
//! - report unresolved, duplicate, ambiguous, and unsupported-resolution errors
//! - preserve exact syntax origins for supported resolver diagnostics
//!
//! Non-goals for this crate:
//! - type checking or inference
//! - type-directed overload or member selection
//! - ownership/borrowing analysis
//! - runtime lowering or code generation

pub mod collect;
pub mod errors;
pub mod ids;
pub mod imports;
pub mod inject;
pub mod model;
pub mod session;
pub mod traverse;

pub use errors::{ResolverError, ResolverErrorKind};
pub use fol_package::PreparedPackage;
pub use fol_parser::ast::ParsedSourceUnitKind;
pub use ids::{IdTable, ImportId, ReferenceId, ScopeId, SourceUnitId, SymbolId};
pub use inject::inject_build_stdlib_types;
pub use model::{
    MountedSymbolProvenance, ReferenceKind, ResolvedImport, ResolvedPackage, ResolvedProgram,
    ResolvedReference, ResolvedScope, ResolvedSourceUnit, ResolvedSymbol, ResolvedWorkspace,
    ScopeKind, SymbolKind,
};
pub use session::{PackageIdentity, PackageSourceKind, ResolverConfig, ResolverSession};

pub type ResolverResult<T> = Result<T, Vec<ResolverError>>;

#[derive(Debug, Default)]
pub struct Resolver;

impl Resolver {
    /// Create a resolver instance for one package-resolution run.
    pub fn new() -> Self {
        Self
    }

    /// Resolve one parsed package into scopes, symbols, references, and imports.
    ///
    /// This is the legacy single-program surface. New semantic
    /// consumers should prefer the workspace APIs and then read the entry
    /// package from the returned `ResolvedWorkspace`.
    pub fn resolve_package(
        &mut self,
        syntax: fol_parser::ast::ParsedPackage,
    ) -> ResolverResult<ResolvedProgram> {
        ResolverSession::new().resolve_package(syntax)
    }

    /// Resolve one parsed package with an explicit resolver configuration.
    ///
    /// This is the legacy single-program surface.
    pub fn resolve_package_with_config(
        &mut self,
        syntax: fol_parser::ast::ParsedPackage,
        config: ResolverConfig,
    ) -> ResolverResult<ResolvedProgram> {
        ResolverSession::with_config(config).resolve_package(syntax)
    }

    /// Resolve one parsed package into a graph-aware semantic workspace.
    pub fn resolve_package_workspace(
        &mut self,
        syntax: fol_parser::ast::ParsedPackage,
    ) -> ResolverResult<ResolvedWorkspace> {
        ResolverSession::new().resolve_package_workspace(syntax)
    }

    /// Resolve one parsed package into a graph-aware semantic workspace with explicit config.
    pub fn resolve_package_workspace_with_config(
        &mut self,
        syntax: fol_parser::ast::ParsedPackage,
        config: ResolverConfig,
    ) -> ResolverResult<ResolvedWorkspace> {
        ResolverSession::with_config(config).resolve_package_workspace(syntax)
    }

    /// Resolve one fol-package prepared package with a fresh resolver session.
    ///
    /// This is the legacy single-program surface.
    pub fn resolve_prepared_package(
        &mut self,
        prepared: PreparedPackage,
    ) -> ResolverResult<ResolvedProgram> {
        ResolverSession::new().resolve_prepared_package(prepared)
    }

    /// Resolve one fol-package prepared package with an explicit resolver configuration.
    ///
    /// This is the legacy single-program surface.
    pub fn resolve_prepared_package_with_config(
        &mut self,
        prepared: PreparedPackage,
        config: ResolverConfig,
    ) -> ResolverResult<ResolvedProgram> {
        ResolverSession::with_config(config).resolve_prepared_package(prepared)
    }

    /// Resolve one fol-package prepared package into a graph-aware semantic workspace.
    pub fn resolve_prepared_workspace(
        &mut self,
        prepared: PreparedPackage,
    ) -> ResolverResult<ResolvedWorkspace> {
        ResolverSession::new().resolve_prepared_workspace(prepared)
    }

    /// Resolve one fol-package prepared package into a graph-aware semantic workspace with explicit config.
    pub fn resolve_prepared_workspace_with_config(
        &mut self,
        prepared: PreparedPackage,
        config: ResolverConfig,
    ) -> ResolverResult<ResolvedWorkspace> {
        ResolverSession::with_config(config).resolve_prepared_workspace(prepared)
    }
}

/// Resolve one parsed package with a fresh resolver instance.
pub fn resolve_package(syntax: fol_parser::ast::ParsedPackage) -> ResolverResult<ResolvedProgram> {
    Resolver::new().resolve_package(syntax)
}

/// Resolve one parsed package with an explicit resolver configuration.
pub fn resolve_package_with_config(
    syntax: fol_parser::ast::ParsedPackage,
    config: ResolverConfig,
) -> ResolverResult<ResolvedProgram> {
    Resolver::new().resolve_package_with_config(syntax, config)
}

/// Resolve one parsed package into a graph-aware semantic workspace with a fresh resolver instance.
pub fn resolve_package_workspace(
    syntax: fol_parser::ast::ParsedPackage,
) -> ResolverResult<ResolvedWorkspace> {
    Resolver::new().resolve_package_workspace(syntax)
}

/// Resolve one parsed package into a graph-aware semantic workspace with explicit config.
pub fn resolve_package_workspace_with_config(
    syntax: fol_parser::ast::ParsedPackage,
    config: ResolverConfig,
) -> ResolverResult<ResolvedWorkspace> {
    Resolver::new().resolve_package_workspace_with_config(syntax, config)
}

/// Resolve one fol-package prepared package with a fresh resolver instance.
pub fn resolve_prepared_package(prepared: PreparedPackage) -> ResolverResult<ResolvedProgram> {
    Resolver::new().resolve_prepared_package(prepared)
}

/// Resolve one fol-package prepared package with an explicit resolver configuration.
pub fn resolve_prepared_package_with_config(
    prepared: PreparedPackage,
    config: ResolverConfig,
) -> ResolverResult<ResolvedProgram> {
    Resolver::new().resolve_prepared_package_with_config(prepared, config)
}

/// Resolve one fol-package prepared package into a graph-aware semantic workspace with a fresh resolver instance.
pub fn resolve_prepared_workspace(prepared: PreparedPackage) -> ResolverResult<ResolvedWorkspace> {
    Resolver::new().resolve_prepared_workspace(prepared)
}

/// Resolve one fol-package prepared package into a graph-aware semantic workspace with explicit config.
pub fn resolve_prepared_workspace_with_config(
    prepared: PreparedPackage,
    config: ResolverConfig,
) -> ResolverResult<ResolvedWorkspace> {
    Resolver::new().resolve_prepared_workspace_with_config(prepared, config)
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_package, resolve_package_workspace, resolve_prepared_package,
        resolve_prepared_workspace,
    };
    use fol_package::PackageSession;
    use fol_parser::ast::{AstParser, ParsedPackage};
    use fol_stream::FileStream;

    fn parse_package(path: &str) -> ParsedPackage {
        let mut stream = FileStream::from_file(path).expect("Should open resolver smoke fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        parser
            .parse_package(&mut lexer)
            .expect("Resolver smoke fixture should parse as a package")
    }

    #[test]
    fn resolver_smoke_can_lower_a_parsed_package() {
        let resolved = resolve_package(parse_package("../../../test/parser/simple_var.fol"))
            .expect("Resolver foundation should lower parsed packages");

        assert_eq!(resolved.package_name(), "parser");
        assert_eq!(resolved.source_units.len(), 1);
        assert_eq!(resolved.syntax().source_units.len(), 1);
    }

    #[test]
    fn resolver_smoke_can_lower_a_prepared_package() {
        let session = PackageSession::new();
        let prepared = session
            .prepare_entry_package(parse_package("../../../test/parser/simple_var.fol"))
            .expect("Prepared-package smoke fixture should prepare");

        let resolved = resolve_prepared_package(prepared)
            .expect("Resolver foundation should lower prepared packages");

        assert_eq!(resolved.package_name(), "parser");
        assert_eq!(resolved.source_units.len(), 1);
    }

    #[test]
    fn resolver_smoke_can_lower_a_workspace_from_a_parsed_package() {
        let workspace =
            resolve_package_workspace(parse_package("../../../test/parser/simple_var.fol"))
                .expect("Resolver workspace should lower parsed packages");

        assert_eq!(workspace.package_count(), 1);
        assert_eq!(workspace.entry_program().package_name(), "parser");
        assert_eq!(workspace.entry_package().prepared.syntax.package, "parser");
    }

    #[test]
    fn resolver_smoke_can_lower_a_workspace_from_a_prepared_package() {
        let session = PackageSession::new();
        let prepared = session
            .prepare_entry_package(parse_package("../../../test/parser/simple_var.fol"))
            .expect("Prepared-package smoke fixture should prepare");

        let workspace = resolve_prepared_workspace(prepared)
            .expect("Resolver workspace should lower prepared packages");

        assert_eq!(workspace.package_count(), 1);
        assert_eq!(workspace.entry_program().package_name(), "parser");
    }
}

use fol_lexer::lexer::stage3::Elements;
use fol_parser::ast::AstParser;
use fol_resolver::{
    resolve_package, resolve_package_with_config, resolve_package_workspace_with_config,
    ResolvedProgram, ResolvedWorkspace, ResolverConfig, ResolverError,
};
use fol_stream::FileStream;
use std::time::{SystemTime, UNIX_EPOCH};

fn resolve_package_from_file(path: &str) -> ResolvedProgram {
    let mut file_stream = FileStream::from_file(path).expect("Should read resolver test file");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Resolver test fixture should parse as a package");

    resolve_package(parsed).expect("Resolver foundation should lower parsed packages")
}

fn resolve_package_from_folder(path: &str) -> ResolvedProgram {
    try_resolve_package_from_folder(path)
        .expect("Resolver test fixture should resolve successfully")
}

fn try_resolve_package_from_folder(path: &str) -> Result<ResolvedProgram, Vec<ResolverError>> {
    try_resolve_package_from_folder_with_config(path, ResolverConfig::default())
}

fn resolve_package_from_folder_with_config(path: &str, config: ResolverConfig) -> ResolvedProgram {
    try_resolve_package_from_folder_with_config(path, config)
        .expect("Resolver test fixture should resolve successfully")
}

fn try_resolve_package_from_folder_with_config(
    path: &str,
    config: ResolverConfig,
) -> Result<ResolvedProgram, Vec<ResolverError>> {
    let mut file_stream = FileStream::from_folder(path).expect("Should read resolver test folder");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Resolver test fixture should parse as a package");

    resolve_package_with_config(parsed, config)
}

fn resolve_workspace_from_folder_with_config(
    path: &str,
    config: ResolverConfig,
) -> ResolvedWorkspace {
    let mut file_stream = FileStream::from_folder(path).expect("Should read resolver test folder");
    let mut lexer = Elements::init(&mut file_stream);
    let mut parser = AstParser::new();
    let parsed = parser
        .parse_package(&mut lexer)
        .expect("Resolver test fixture should parse as a package");

    resolve_package_workspace_with_config(parsed, config)
        .expect("Resolver workspace fixture should resolve successfully")
}

fn unique_temp_root(label: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "fol_resolver_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ))
}

#[cfg(test)]
#[path = "test_resolver_parts/block_scopes.rs"]
mod block_scopes;
#[cfg(test)]
#[path = "test_resolver_parts/comment_transparency.rs"]
mod comment_transparency;
#[cfg(test)]
#[path = "test_resolver_parts/ambiguity_diagnostics.rs"]
mod ambiguity_diagnostics;
#[cfg(test)]
#[path = "test_resolver_parts/diagnostic_quality.rs"]
mod diagnostic_quality;
#[cfg(test)]
#[path = "test_resolver_parts/file_private_visibility.rs"]
mod file_private_visibility;
#[cfg(test)]
#[path = "test_resolver_parts/foundation.rs"]
mod foundation;
#[cfg(test)]
#[path = "test_resolver_parts/forward_references.rs"]
mod forward_references;
#[cfg(test)]
#[path = "test_resolver_parts/free_calls.rs"]
mod free_calls;
#[cfg(test)]
#[path = "test_resolver_parts/identifier_resolution.rs"]
mod identifier_resolution;
#[cfg(test)]
#[path = "test_resolver_parts/imports.rs"]
mod imports;
#[cfg(test)]
#[path = "test_resolver_parts/import_exposure.rs"]
mod import_exposure;
#[cfg(test)]
#[path = "test_resolver_parts/inquiry_resolution.rs"]
mod inquiry_resolution;
#[cfg(test)]
#[path = "test_resolver_parts/loop_binders.rs"]
mod loop_binders;
#[cfg(test)]
#[path = "test_resolver_parts/qualified_calls.rs"]
mod qualified_calls;
#[cfg(test)]
#[path = "test_resolver_parts/qualified_identifiers.rs"]
mod qualified_identifiers;
#[cfg(test)]
#[path = "test_resolver_parts/qualified_type_resolution.rs"]
mod qualified_type_resolution;
#[cfg(test)]
#[path = "test_resolver_parts/rolling_binders.rs"]
mod rolling_binders;
#[cfg(test)]
#[path = "test_resolver_parts/routine_scopes.rs"]
mod routine_scopes;
#[cfg(test)]
#[path = "test_resolver_parts/source_units.rs"]
mod source_units;
#[cfg(test)]
#[path = "test_resolver_parts/workspace.rs"]
mod workspace;
#[cfg(test)]
#[path = "test_resolver_parts/shadowing_contract.rs"]
mod shadowing_contract;
#[cfg(test)]
#[path = "test_resolver_parts/surface_audit.rs"]
mod surface_audit;
#[cfg(test)]
#[path = "test_resolver_parts/top_level_collection.rs"]
mod top_level_collection;
#[cfg(test)]
#[path = "test_resolver_parts/top_level_duplicates.rs"]
mod top_level_duplicates;
#[cfg(test)]
#[path = "test_resolver_parts/type_resolution.rs"]
mod type_resolution;
#[cfg(test)]
#[path = "test_resolver_parts/generic_routines.rs"]
mod generic_routines;
#[cfg(test)]
#[path = "test_resolver_parts/pkg_resolution.rs"]
mod pkg_resolution;
#[cfg(test)]
#[path = "test_resolver_parts/provider_boundary.rs"]
mod provider_boundary;
#[cfg(test)]
#[path = "test_resolver_parts/std_resolution.rs"]
mod std_resolution;
#[cfg(test)]
#[path = "test_resolver_parts/standards_m2.rs"]
mod standards_m2;
#[cfg(test)]
#[path = "test_resolver_parts/unsupported_imports.rs"]
mod unsupported_imports;
#[cfg(test)]
#[path = "test_resolver_parts/use_loc_resolution.rs"]
mod use_loc_resolution;

//! The one place the frontend derives std and package-store roots.
//!
//! Two chains exist and must not be confused:
//!
//! * the **read** chain (`store_read_root`) — explicit → declared →
//!   `<project>/.fol/pkg` → `$FOL_HOME/pkg` → the toolchain's bundled store;
//! * the **write** chain (`package_store_write_root`) — explicit → declared →
//!   `<project>/.fol/pkg` and nothing else, so `fol pack fetch` can never
//!   write into a shared home store or into an installed toolchain.
//!
//! Explicit roots are merged into the workspace by
//! [`crate::workspace::load_frontend_workspace`], which is the only place that
//! decides precedence; everything here reads the merged workspace.

use crate::FrontendWorkspace;
use fol_package::{StdRootLayers, StoreRootLayers};
use std::path::PathBuf;

/// `load_frontend_workspace` already merges the explicit roots into the
/// workspace, but the explicit layer is passed again here so a hand-built
/// workspace (tests, embedders) still honors a flag or env var.
fn store_layers<'a>(
    config: &'a crate::FrontendConfig,
    workspace: &'a FrontendWorkspace,
) -> StoreRootLayers<'a> {
    StoreRootLayers {
        explicit: config.package_store_root_override.as_deref(),
        declared: workspace.package_store_root_override.as_deref(),
        project_root: Some(workspace.root.root.as_path()),
    }
}

/// Every store a read may consult, most specific first.
pub(crate) fn store_read_chain(
    config: &crate::FrontendConfig,
    workspace: &FrontendWorkspace,
) -> Vec<PathBuf> {
    fol_package::package_store_root_chain(store_layers(config, workspace))
}

/// The store a read resolves against today.
pub(crate) fn store_read_root(
    config: &crate::FrontendConfig,
    workspace: &FrontendWorkspace,
) -> PathBuf {
    store_read_chain(config, workspace)
        .into_iter()
        .next()
        .unwrap_or_else(|| fol_package::project_store_root(&workspace.root.root))
}

/// Where `fol pack fetch` materializes packages.
pub fn package_store_write_root(
    config: &crate::FrontendConfig,
    workspace: &FrontendWorkspace,
) -> PathBuf {
    fol_package::package_store_write_root(store_layers(config, workspace))
        .unwrap_or_else(|| fol_package::project_store_root(&workspace.root.root))
}

/// The standard library this workspace compiles against.
pub(crate) fn std_root(
    config: &crate::FrontendConfig,
    workspace: &FrontendWorkspace,
) -> Option<PathBuf> {
    fol_package::effective_std_root_path(StdRootLayers {
        explicit: config.std_root_override.as_deref(),
        declared: workspace.std_root_override.as_deref(),
    })
}

pub(crate) fn workspace_resolver_config(
    config: &crate::FrontendConfig,
    workspace: &FrontendWorkspace,
) -> fol_resolver::ResolverConfig {
    fol_resolver::ResolverConfig {
        std_root: std_root(config, workspace).map(|path| path.to_string_lossy().to_string()),
        package_store_root: Some(
            store_read_root(config, workspace)
                .to_string_lossy()
                .to_string(),
        ),
    }
}

/// The direct single-file route has no `FrontendWorkspace`, so it derives the
/// same chains from the explicit roots plus whatever project the input sits in.
/// Without this it saw only CLI flags: `FOL_STD_ROOT`/`FOL_PACKAGE_STORE_ROOT`
/// were silently ignored and `use std: pkg = {"std"}` could not resolve at all.
pub(crate) fn direct_resolver_config(
    config: &crate::FrontendConfig,
    input: &std::path::Path,
) -> fol_resolver::ResolverConfig {
    let discovered = crate::discovery::discover_root_upward(input);
    let project_root = discovered.as_ref().map(|root| match root {
        crate::DiscoveredRoot::Workspace(root) => root.root.clone(),
        crate::DiscoveredRoot::Package(root) => root.root.clone(),
    });
    // Read the workspace file directly: loading the full workspace would fail
    // when a *sibling* member is broken, which must not stop a healthy file
    // from compiling.
    let declared = match &discovered {
        Some(crate::DiscoveredRoot::Workspace(root)) => {
            crate::workspace::load_workspace_config(root).ok()
        }
        _ => None,
    };
    let declared_std = declared.as_ref().and_then(|c| c.std_root_override.clone());
    let declared_store = declared
        .as_ref()
        .and_then(|c| c.package_store_root_override.clone());

    let store_root = fol_package::effective_package_store_root(StoreRootLayers {
        explicit: config.package_store_root_override.as_deref(),
        declared: declared_store.as_deref(),
        project_root: project_root.as_deref(),
    });
    fol_resolver::ResolverConfig {
        std_root: fol_package::effective_std_root_path(StdRootLayers {
            explicit: config.std_root_override.as_deref(),
            declared: declared_std.as_deref(),
        })
        .map(|path| path.to_string_lossy().to_string()),
        package_store_root: store_root.map(|path| path.to_string_lossy().to_string()),
    }
}

//! Resolving a build artifact's export allowlist into a `ResolvedAbiSurface`.
//!
//! The allowlist names FOL routines by path; this finds each one in the lowered
//! workspace, projects its signature, and produces the surface the header, the
//! manifest, and the wrappers are all generated from. Generating any of the
//! three from a different source is how a header and a library come to disagree.

use crate::model::EmittedRustFile;
use crate::session::BackendSession;
use crate::{BackendError, BackendErrorKind, BackendResult};
use fol_abi::{ForeignInterfaceTemplate, ResolvedAbiSurface};
use fol_lower::abi::AbiExportRequest;

/// A routine's parameters, result, and error type, as lowering knows them.
type LoweredSignature = (
    Vec<(String, fol_lower::LoweredTypeId)>,
    Option<fol_lower::LoweredTypeId>,
    Option<fol_lower::LoweredTypeId>,
);

/// Find a routine by its FOL path and describe its signature.
///
/// A path is matched on its last segment, because the lowered routine carries
/// its declared name rather than a fully qualified path. An ambiguous match is
/// refused rather than resolved by order.
fn resolve_signature(session: &BackendSession, path: &str) -> Option<LoweredSignature> {
    let wanted = path.rsplit("::").next().unwrap_or(path);
    let workspace = session.workspace();
    let table = workspace.type_table();

    let mut found = None;
    for package in workspace.packages() {
        for routine in package.routine_decls.values() {
            if routine.name != wanted {
                continue;
            }
            if found.is_some() {
                // Two routines share the name; the allowlist cannot say which.
                return None;
            }
            let signature = routine.signature?;
            let Some(fol_lower::types::LoweredType::Routine(signature)) = table.get(signature)
            else {
                return None;
            };
            let params = signature
                .params
                .iter()
                .enumerate()
                .map(|(index, id)| (format!("arg{index}"), *id))
                .collect();
            found = Some((params, signature.return_type, signature.error_type));
        }
    }
    found
}

/// The private Rust path a wrapper calls.
///
/// Routines live at `packages::<package module>::<namespace module>`, so the
/// namespace layout is consulted rather than the path being guessed. The
/// mangled leaf keeps its internal ID; only the wrapper carries a public name.
fn internal_path(session: &BackendSession, path: &str) -> Option<String> {
    let wanted = path.rsplit("::").next().unwrap_or(path);
    let namespaces = crate::layout::plan_namespace_layouts(session);
    for package in session.workspace().packages() {
        for routine in package.routine_decls.values() {
            if routine.name != wanted {
                continue;
            }
            let source_unit_id = routine.source_unit_id?;
            let namespace = namespaces.iter().find(|plan| {
                plan.package_identity == package.identity
                    && plan.source_unit_ids.contains(&source_unit_id)
            })?;
            return Some(format!(
                "crate::packages::{}::{}::{}",
                crate::mangle_package_module_name(&package.identity),
                namespace.module_name,
                crate::mangle_routine_name(&package.identity, routine.id, &routine.name)
            ));
        }
    }
    None
}

/// Build the surface for one artifact's allowlist.
pub fn resolve_surface(
    session: &BackendSession,
    artifact: &str,
    major: u32,
    minor: u32,
    exports: &[AbiExportRequest],
    target: fol_types::ResolvedTarget,
) -> BackendResult<ResolvedAbiSurface> {
    let projection =
        fol_lower::abi::project_exports(session.workspace().type_table(), exports, |path| {
            resolve_signature(session, path)
        });

    if !projection.is_clean() {
        // Every rejection, not just the first: one build round should show a
        // caller everything that has to change.
        let detail = projection
            .rejections
            .iter()
            .map(|classification| format!("  {classification}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("the ABI export surface is not projectable:\n{detail}"),
        ));
    }

    Ok(ResolvedAbiSurface {
        artifact: artifact.to_string(),
        major,
        minor,
        interface: projection.template.resolve(target),
    })
}

/// The generated Rust module holding every exported wrapper.
///
/// Emitted as its own file so the wrappers are visibly separate from the FOL
/// code they call, and so a reader can see the whole C surface in one place.
pub fn emit_wrapper_module(
    session: &BackendSession,
    surface: &ResolvedAbiSurface,
) -> BackendResult<EmittedRustFile> {
    let mut contents = String::new();
    contents.push_str(
        "// Generated C ABI wrappers. Do not edit.\n\
         //\n\
         // Each wrapper owns the uniform status return, inbound scalar validation,\n\
         // panic containment, and the out-parameter rules. The FOL routines they call\n\
         // stay private and ID-mangled: a public symbol carries no internal ID.\n\n",
    );
    contents.push_str("use fol_runtime as rt;\n\n");

    for routine in surface.interface.facing(fol_abi::AbiFacing::Export) {
        let path = internal_path(session, &routine.fol_path).ok_or_else(|| {
            BackendError::new(
                BackendErrorKind::InvalidInput,
                format!(
                    "export '{}' names no routine in this artifact",
                    routine.fol_path
                ),
            )
        })?;
        contents.push_str(&super::wrapper::render_wrapper(
            &surface.interface.types,
            routine,
            &path,
        ));
        contents.push('\n');
    }

    Ok(EmittedRustFile {
        path: "src/abi_exports.rs".to_string(),
        module_name: "abi_exports".to_string(),
        contents,
    })
}

/// An empty template, for an artifact that declares no exports.
pub fn empty_surface(artifact: &str, target: fol_types::ResolvedTarget) -> ResolvedAbiSurface {
    ResolvedAbiSurface {
        artifact: artifact.to_string(),
        major: 0,
        minor: 0,
        interface: ForeignInterfaceTemplate::new().resolve(target),
    }
}

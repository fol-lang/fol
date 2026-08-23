pub mod header;
pub mod status;
pub mod surface;
pub mod wrapper;

#[cfg(test)]
mod tests;

/// The role-tagged files a C surface produces beside the library itself.
///
/// Header, manifest, and symbol allowlist are all rendered from one
/// `ResolvedAbiSurface`, so they cannot describe different things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiSurfaceOutputs {
    pub header: String,
    pub manifest: String,
    /// One symbol per line, sorted, for a linker version script or `-exported_symbols_list`.
    pub symbol_allowlist: String,
    pub interface_fingerprint: String,
    pub build_fingerprint: String,
}

/// Render every C-surface output for one artifact.
pub fn render_surface_outputs(
    surface: &fol_abi::ResolvedAbiSurface,
    provenance: fol_abi::BuildProvenance,
) -> AbiSurfaceOutputs {
    let manifest = fol_abi::AbiManifest {
        surface: surface.clone(),
        provenance,
    };
    let mut allowlist = surface.exported_symbols().join("\n");
    if !allowlist.is_empty() {
        allowlist.push('\n');
    }
    AbiSurfaceOutputs {
        header: header::render_header(surface),
        manifest: manifest.canonical_json(),
        symbol_allowlist: allowlist,
        interface_fingerprint: manifest.interface_fingerprint(),
        build_fingerprint: manifest.build_fingerprint(),
    }
}

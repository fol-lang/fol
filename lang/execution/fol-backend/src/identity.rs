use fol_lower::{render_lowered_workspace, LoweredWorkspace};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendWorkspaceIdentity {
    pub hash: String,
    pub crate_dir_name: String,
}

impl BackendWorkspaceIdentity {
    pub fn for_workspace(workspace: &LoweredWorkspace) -> Self {
        let hash = stable_workspace_hash(workspace);
        let short_entry = truncate_component(&sanitize_component(
            &workspace.entry_identity().display_name,
        ));
        let crate_dir_name = format!("fol-build-{}-{}", short_entry, &hash[..12]);
        Self {
            hash,
            crate_dir_name,
        }
    }
}

pub fn stable_workspace_hash(workspace: &LoweredWorkspace) -> String {
    let rendered = render_lowered_workspace(workspace);
    format!("{:016x}", fnv1a64(rendered.as_bytes()))
}

fn sanitize_component(raw: &str) -> String {
    let mut output = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
        } else {
            output.push('_');
        }
    }
    while output.contains("__") {
        output = output.replace("__", "_");
    }
    output.trim_matches('_').to_string()
}

fn truncate_component(raw: &str) -> String {
    const MAX_LEN: usize = 12;
    if raw.len() <= MAX_LEN {
        return raw.to_string();
    }
    raw[..MAX_LEN].trim_matches('_').to_string()
}

pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{stable_workspace_hash, BackendWorkspaceIdentity};
    use crate::testing::{sample_lowered_workspace, sample_lowered_workspace_named};

    #[test]
    fn backend_workspace_identity_is_deterministic_for_same_input() {
        let workspace = sample_lowered_workspace();

        let first = stable_workspace_hash(&workspace);
        let second = stable_workspace_hash(&workspace);
        let identity = BackendWorkspaceIdentity::for_workspace(&workspace);

        assert_eq!(first, second);
        assert_eq!(identity.hash, first);
        assert!(identity.crate_dir_name.starts_with("fol-build-app-"));
        assert_eq!(identity.crate_dir_name.len(), "fol-build-app-".len() + 12);
    }

    #[test]
    fn backend_workspace_identity_changes_when_workspace_shape_changes() {
        let first = sample_lowered_workspace();
        let second = sample_lowered_workspace_named("demo");

        assert_ne!(
            stable_workspace_hash(&first),
            stable_workspace_hash(&second)
        );
        assert_ne!(
            BackendWorkspaceIdentity::for_workspace(&first).crate_dir_name,
            BackendWorkspaceIdentity::for_workspace(&second).crate_dir_name
        );
    }
}

/// The build fingerprint for one compilation.
///
/// Distinct from `stable_workspace_hash`, which covers only the lowered source
/// and is therefore the *interface* half of section 4.11's two hashes. This is
/// the build half: it moves when the toolchain, the target, the profile, the
/// product kind, or the native link plan moves, and a compiler upgrade must
/// move it without moving the other.
pub fn build_fingerprint(
    workspace_hash: &str,
    product_kind: crate::model::BackendProductKind,
    target: &fol_types::ResolvedTarget,
    profile: crate::config::BackendBuildProfile,
    link_plan: Option<&fol_build::link_plan::NativeLinkPlan>,
) -> String {
    let mut rendered = String::new();
    rendered.push_str(workspace_hash);
    rendered.push('\n');
    rendered.push_str(product_kind.as_str());
    rendered.push('\n');
    rendered.push_str(target.rust_target_triple());
    rendered.push('\n');
    rendered.push_str(profile.as_str());
    rendered.push('\n');
    // The rustc that will run. A compiler upgrade must move this hash.
    rendered.push_str(&rustc_identity());
    rendered.push('\n');
    if let Some(plan) = link_plan {
        rendered.push_str(&plan.fingerprint_rendering());
    }
    format!("{:016x}", fnv1a64(rendered.as_bytes()))
}

/// The compiler's own version string, or a marker when it cannot be read.
fn rustc_identity() -> String {
    std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "rustc-unavailable".to_string())
}

/// The cache directory segment for one product.
///
/// Isolated by kind and target so a static library and a shared library built
/// from one source for one target cannot overwrite each other's intermediate
/// output.
pub fn cache_segment(
    product_kind: crate::model::BackendProductKind,
    target: &fol_types::ResolvedTarget,
    profile: crate::config::BackendBuildProfile,
) -> String {
    format!(
        "{}/{}/{}",
        product_kind.as_str(),
        target.rust_target_directory_name(),
        profile.as_str()
    )
}

#[cfg(test)]
mod build_fingerprint_tests {
    use super::{build_fingerprint, cache_segment};
    use crate::config::BackendBuildProfile;
    use crate::model::BackendProductKind;
    use fol_build::link_plan::{NativeLinkAtom, NativeLinkPlan};
    use fol_build::native::NativeLinkMode;

    fn target(spelling: &str) -> fol_types::ResolvedTarget {
        fol_types::ResolvedTarget::resolve(spelling).unwrap()
    }

    fn fingerprint_of(
        kind: BackendProductKind,
        spelling: &str,
        profile: BackendBuildProfile,
        plan: Option<&NativeLinkPlan>,
    ) -> String {
        build_fingerprint("source-hash", kind, &target(spelling), profile, plan)
    }

    /// Every build-affecting fact moves the fingerprint. This is the half
    /// workspace identity leaves out -- which is why debug and release share a
    /// crate directory name.
    #[test]
    fn the_build_fingerprint_moves_with_kind_target_profile_and_link_plan() {
        let baseline = fingerprint_of(
            BackendProductKind::Executable,
            "x86_64-unknown-linux-gnu",
            BackendBuildProfile::Debug,
            None,
        );

        assert_ne!(
            baseline,
            fingerprint_of(
                BackendProductKind::StaticLibrary,
                "x86_64-unknown-linux-gnu",
                BackendBuildProfile::Debug,
                None
            ),
            "product kind"
        );
        assert_ne!(
            baseline,
            fingerprint_of(
                BackendProductKind::Executable,
                "x86_64-unknown-linux-musl",
                BackendBuildProfile::Debug,
                None
            ),
            "target"
        );
        assert_ne!(
            baseline,
            fingerprint_of(
                BackendProductKind::Executable,
                "x86_64-unknown-linux-gnu",
                BackendBuildProfile::Release,
                None
            ),
            "profile"
        );

        let mut plan = NativeLinkPlan::new("app", target("x86_64-unknown-linux-gnu"));
        plan.push(NativeLinkAtom::system_library(
            "ssl",
            NativeLinkMode::Dynamic,
        ));
        assert_ne!(
            baseline,
            fingerprint_of(
                BackendProductKind::Executable,
                "x86_64-unknown-linux-gnu",
                BackendBuildProfile::Debug,
                Some(&plan)
            ),
            "native link inputs"
        );
    }

    /// The same inputs give the same fingerprint, or nothing can be cached.
    #[test]
    fn the_build_fingerprint_is_stable_for_identical_inputs() {
        let first = fingerprint_of(
            BackendProductKind::Executable,
            "x86_64-unknown-linux-gnu",
            BackendBuildProfile::Debug,
            None,
        );
        let second = fingerprint_of(
            BackendProductKind::Executable,
            "x86_64-unknown-linux-gnu",
            BackendBuildProfile::Debug,
            None,
        );
        assert_eq!(first, second);
    }

    /// Link order reaches the fingerprint.
    #[test]
    fn reordering_the_link_plan_moves_the_build_fingerprint() {
        let alpha = NativeLinkAtom::system_library("alpha", NativeLinkMode::Dynamic);
        let beta = NativeLinkAtom::system_library("beta", NativeLinkMode::Dynamic);

        let mut first = NativeLinkPlan::new("app", target("x86_64-unknown-linux-gnu"));
        first.push(alpha.clone());
        first.push(beta.clone());
        let mut second = NativeLinkPlan::new("app", target("x86_64-unknown-linux-gnu"));
        second.push(beta);
        second.push(alpha);

        assert_ne!(
            fingerprint_of(
                BackendProductKind::Executable,
                "x86_64-unknown-linux-gnu",
                BackendBuildProfile::Debug,
                Some(&first)
            ),
            fingerprint_of(
                BackendProductKind::Executable,
                "x86_64-unknown-linux-gnu",
                BackendBuildProfile::Debug,
                Some(&second)
            )
        );
    }

    /// Two kinds built from one source for one target do not share a cache
    /// directory.
    #[test]
    fn cache_directories_are_isolated_by_kind_and_target() {
        let gnu = target("x86_64-unknown-linux-gnu");
        let musl = target("x86_64-unknown-linux-musl");

        let static_gnu = cache_segment(
            BackendProductKind::StaticLibrary,
            &gnu,
            BackendBuildProfile::Debug,
        );
        let shared_gnu = cache_segment(
            BackendProductKind::SharedLibrary,
            &gnu,
            BackendBuildProfile::Debug,
        );
        let static_musl = cache_segment(
            BackendProductKind::StaticLibrary,
            &musl,
            BackendBuildProfile::Debug,
        );

        assert_ne!(static_gnu, shared_gnu, "kinds must not share a cache");
        assert_ne!(static_gnu, static_musl, "targets must not share a cache");
        assert!(static_gnu.starts_with("static-library/"));
    }
}

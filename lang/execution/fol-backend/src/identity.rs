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

//! The interop components' identity.
//!
//! This used to shell out to `git` at run time and verify that
//! `../{parc,linc,gerc}` were clean checkouts at the locked revisions. That
//! check could only ever pass inside a source tree — the paths were baked in
//! from whichever machine built the binary — and it is meaningless now that the
//! components are git dependencies resolved by cargo.
//!
//! Identity became a build-time property instead: `build.rs` proves that
//! `interop.lock.toml` and `Cargo.lock` agree on every revision and remote, and
//! the schema assertions in `lib.rs` prove the resolved crates expose the
//! contract versions the lock claims. A git revision is content-binding, so a
//! matching revision *is* the evidence.

use crate::lock::{LOCKED_GERC_REVISION, LOCKED_LINC_REVISION, LOCKED_PARC_REVISION};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct VerifiedSiblingRevisions {
    pub parc: &'static str,
    pub linc: &'static str,
    pub gerc: &'static str,
}

/// The revisions this binary was compiled against. Infallible: verification
/// already happened at build time.
pub(crate) fn compiled_component_revisions() -> VerifiedSiblingRevisions {
    VerifiedSiblingRevisions {
        parc: LOCKED_PARC_REVISION,
        linc: LOCKED_LINC_REVISION,
        gerc: LOCKED_GERC_REVISION,
    }
}

#[cfg(test)]
mod tests {
    use super::compiled_component_revisions;

    #[test]
    fn compiled_revisions_are_full_git_hashes() {
        let revisions = compiled_component_revisions();
        for (component, revision) in [
            ("parc", revisions.parc),
            ("linc", revisions.linc),
            ("gerc", revisions.gerc),
        ] {
            assert_eq!(
                revision.len(),
                40,
                "{component} revision should be a full git hash: {revision}"
            );
            assert!(
                revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{component} revision should be hexadecimal: {revision}"
            );
        }
    }
}

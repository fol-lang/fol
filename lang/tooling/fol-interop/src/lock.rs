// The locked interop revisions, produced by `build.rs` after it proves that
// `interop.lock.toml` and `Cargo.lock` agree on every revision and remote. A
// stale lock is therefore a compile error, and there are no sibling paths any
// more — the components are git dependencies.
pub const LOCKED_PARC_REVISION: &str = env!("FOL_LOCKED_PARC_REVISION");
pub const LOCKED_LINC_REVISION: &str = env!("FOL_LOCKED_LINC_REVISION");
pub const LOCKED_GERC_REVISION: &str = env!("FOL_LOCKED_GERC_REVISION");

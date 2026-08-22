// The interop revisions cargo actually resolved from the `rev = "..."` pins in
// `Cargo.toml`, read back out of `Cargo.lock` by `build.rs`. The components are
// git dependencies, so there are no sibling paths and no separate lock file.
pub const LOCKED_PARC_REVISION: &str = env!("FOL_LOCKED_PARC_REVISION");
pub const LOCKED_LINC_REVISION: &str = env!("FOL_LOCKED_LINC_REVISION");
pub const LOCKED_GERC_REVISION: &str = env!("FOL_LOCKED_GERC_REVISION");

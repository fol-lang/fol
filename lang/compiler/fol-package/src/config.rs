use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageConfig {
    pub std_root: Option<String>,
    pub package_store_root: Option<String>,
    pub package_cache_root: Option<String>,
    pub package_git_cache_root: Option<String>,
}

pub fn bundled_std_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../library/std")
}

pub fn available_bundled_std_root() -> Option<PathBuf> {
    let root = bundled_std_root();
    root.is_dir().then_some(root)
}

/// The package store shipped with the toolchain (the directory holding the
/// bundled `std` package), when it exists. Lets `use x: pkg = {...}` imports
/// resolve without `fol pack fetch` or an explicit `--package-store-root`.
pub fn available_bundled_store_root() -> Option<PathBuf> {
    available_bundled_std_root().and_then(|std_root| std_root.parent().map(PathBuf::from))
}

pub fn effective_std_root(explicit: Option<&str>) -> Option<String> {
    explicit
        .map(str::to_string)
        .or_else(|| available_bundled_std_root().map(|path| path.to_string_lossy().to_string()))
}

impl PackageConfig {
    pub fn effective_std_root(&self) -> Option<String> {
        effective_std_root(self.std_root.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::{available_bundled_std_root, bundled_std_root, effective_std_root, PackageConfig};

    #[test]
    fn bundled_std_root_points_at_repo_library_tree() {
        let root = bundled_std_root();

        assert!(root.is_dir(), "bundled std root should exist in the repo");
        let canonical = root
            .canonicalize()
            .expect("bundled std root should canonicalize");
        assert!(canonical.ends_with("lang/library/std"));
    }

    #[test]
    fn available_bundled_std_root_reports_existing_repo_tree() {
        assert!(available_bundled_std_root().is_some());
    }

    #[test]
    fn effective_std_root_prefers_explicit_override() {
        assert_eq!(
            effective_std_root(Some("/tmp/custom-std")),
            Some("/tmp/custom-std".to_string())
        );
    }

    #[test]
    fn package_config_effective_std_root_defaults_to_bundled_tree() {
        let config = PackageConfig::default();

        assert_eq!(
            config.effective_std_root(),
            available_bundled_std_root().map(|path| path.to_string_lossy().to_string())
        );
    }
}

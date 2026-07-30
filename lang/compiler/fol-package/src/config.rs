use std::path::{Path, PathBuf};

/// The per-project directory holding build state and the local package store.
pub const PROJECT_DOT_DIR: &str = ".fol";
/// The store directory name, both per project and inside FOL_HOME.
pub const STORE_DIR_NAME: &str = "pkg";

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

/// The one spelling of `<project>/.fol/pkg`. Every caller goes through this so
/// the path cannot drift between `.fol/pkg` and `.fol` + `pkg` spellings.
pub fn project_store_root(project_root: &Path) -> PathBuf {
    project_root.join(PROJECT_DOT_DIR).join(STORE_DIR_NAME)
}

/// FOL_HOME, when the environment names one. There is deliberately no default:
/// `fol self` never creates a home the user did not ask for, so neither may the
/// engine assume one exists.
pub fn home_root() -> Option<PathBuf> {
    std::env::var_os("FOL_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The shared package store inside FOL_HOME, written by `fol pack fetch
/// --global` and readable by every toolchain.
pub fn home_store_root() -> Option<PathBuf> {
    home_root().map(|home| home.join(STORE_DIR_NAME))
}

/// An installed toolchain ships its payloads next to the running binary
/// (`$FOL_HOME/toolchains/vX.X.X/{folc, std/, runtime/}`), so the binary's own
/// copy wins over the source-tree path compiled into dev builds. `marker` is a
/// file that must exist inside the candidate for it to count.
pub fn toolchain_sibling_root(name: &str, marker: Option<&Path>) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let root = exe.parent()?.join(name);
    match marker {
        Some(marker) => root.join(marker).is_file().then_some(root),
        None => root.is_dir().then_some(root),
    }
}

pub fn available_bundled_std_root() -> Option<PathBuf> {
    if let Some(root) = toolchain_sibling_root("std", None) {
        return Some(root);
    }
    let root = bundled_std_root();
    root.is_dir().then_some(root)
}

/// A store candidate only counts when it actually holds packages. An *empty*
/// directory must never shadow a populated one — `fol self` creates
/// `$FOL_HOME/pkg` eagerly, and gating on mere existence would break
/// `use std: pkg = {"std"}` for everyone who has ever installed a toolchain.
fn store_has_entries(root: &Path) -> bool {
    std::fs::read_dir(root)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// Which layers a caller can offer when resolving a package store.
#[derive(Debug, Clone, Copy, Default)]
pub struct StoreRootLayers<'a> {
    /// A `--package-store-root` flag or `FOL_PACKAGE_STORE_ROOT`.
    pub explicit: Option<&'a Path>,
    /// A `package_store_root` declared by the workspace file.
    pub declared: Option<&'a Path>,
    /// Owner of `<project>/.fol/pkg`; `None` for standalone input with no root.
    pub project_root: Option<&'a Path>,
}

/// The ordered store chain a *read* consults, most specific first:
/// explicit → declared → `<project>/.fol/pkg` → `$FOL_HOME/pkg` → bundled.
///
/// `explicit` and `declared` are always included: a wrong `--package-store-root`
/// must fail loudly rather than silently fall through to a shared store. The
/// remaining candidates are included only when they hold packages.
pub fn package_store_root_chain(layers: StoreRootLayers<'_>) -> Vec<PathBuf> {
    let mut chain: Vec<PathBuf> = Vec::new();
    let mut push = |candidate: PathBuf| {
        if !chain.contains(&candidate) {
            chain.push(candidate);
        }
    };
    if let Some(explicit) = layers.explicit {
        push(explicit.to_path_buf());
    }
    if let Some(declared) = layers.declared {
        push(declared.to_path_buf());
    }
    for candidate in [
        layers.project_root.map(project_store_root),
        home_store_root(),
        available_bundled_store_root(),
    ]
    .into_iter()
    .flatten()
    {
        if store_has_entries(&candidate) {
            push(candidate);
        }
    }
    // Even with nothing on disk, a project resolves to its own store so the
    // failure reads as "package not found in the store" rather than "no store".
    if let Some(write_root) = package_store_write_root(layers) {
        push(write_root);
    }
    chain
}

/// The single store a read resolves against today. Returning the whole chain
/// above keeps a future multi-store search a one-line change per caller.
pub fn effective_package_store_root(layers: StoreRootLayers<'_>) -> Option<PathBuf> {
    package_store_root_chain(layers).into_iter().next()
}

/// Where a fetch may *write*: explicit → declared → `<project>/.fol/pkg`.
/// Never the shared home store and never the toolchain's bundled store, so a
/// project build cannot mutate either.
pub fn package_store_write_root(layers: StoreRootLayers<'_>) -> Option<PathBuf> {
    layers
        .explicit
        .map(Path::to_path_buf)
        .or_else(|| layers.declared.map(Path::to_path_buf))
        .or_else(|| layers.project_root.map(project_store_root))
}

/// Which layers a caller can offer when resolving the standard library root.
#[derive(Debug, Clone, Copy, Default)]
pub struct StdRootLayers<'a> {
    /// A `--std-root` flag or `FOL_STD_ROOT`.
    pub explicit: Option<&'a Path>,
    /// A `std_root` declared by the workspace file.
    pub declared: Option<&'a Path>,
}

/// explicit → declared → the toolchain's own std.
pub fn effective_std_root_path(layers: StdRootLayers<'_>) -> Option<PathBuf> {
    layers
        .explicit
        .map(Path::to_path_buf)
        .or_else(|| layers.declared.map(Path::to_path_buf))
        .or_else(available_bundled_std_root)
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

    // ------------------------------------------------------------ store chain

    use super::{
        available_bundled_store_root, package_store_root_chain, package_store_write_root,
        project_store_root, store_has_entries, StoreRootLayers,
    };
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// FOL_HOME is process-global, so the tests that set it take a lock.
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct HomeGuard {
        previous: Option<std::ffi::OsString>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl HomeGuard {
        fn set(value: Option<&Path>) -> Self {
            let lock = HOME_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = std::env::var_os("FOL_HOME");
            match value {
                Some(path) => std::env::set_var("FOL_HOME", path),
                None => std::env::remove_var("FOL_HOME"),
            }
            Self {
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("FOL_HOME", value),
                None => std::env::remove_var("FOL_HOME"),
            }
        }
    }

    fn scratch(label: &str) -> fol_testkit::TempFixture {
        let root = fol_testkit::TempFixture::new(&format!("fol_store_chain_{label}"));
        std::fs::create_dir_all(&root).expect("scratch root should be creatable");
        root
    }

    fn populate(root: &Path) -> PathBuf {
        std::fs::create_dir_all(root.join("somepkg")).expect("store package should be creatable");
        root.to_path_buf()
    }

    #[test]
    fn an_empty_store_directory_never_shadows_a_populated_one() {
        let scratch = scratch("empty_home");
        let project = scratch.join("project");
        let home = scratch.join("home");
        // `fol self` creates $FOL_HOME/pkg eagerly, so an existence-only gate
        // here would shadow the bundled std for every toolchain user.
        std::fs::create_dir_all(home.join("pkg")).expect("empty home store should be creatable");
        let _home = HomeGuard::set(Some(&home));

        let chain = package_store_root_chain(StoreRootLayers {
            project_root: Some(&project),
            ..StoreRootLayers::default()
        });

        assert!(
            !chain.contains(&home.join("pkg")),
            "an empty home store must not appear in the chain: {chain:?}"
        );
        assert_eq!(
            chain.first(),
            available_bundled_store_root().as_ref(),
            "a populated bundled store should win over empty candidates: {chain:?}"
        );
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn a_populated_home_store_precedes_the_bundled_store() {
        let scratch = scratch("home_store");
        let project = scratch.join("project");
        let home = scratch.join("home");
        let home_store = populate(&home.join("pkg"));
        let _home = HomeGuard::set(Some(&home));

        let chain = package_store_root_chain(StoreRootLayers {
            project_root: Some(&project),
            ..StoreRootLayers::default()
        });

        assert_eq!(chain.first(), Some(&home_store));
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn a_populated_project_store_precedes_the_home_store() {
        let scratch = scratch("project_store");
        let project = scratch.join("project");
        let project_store = populate(&project_store_root(&project));
        let home = scratch.join("home");
        populate(&home.join("pkg"));
        let _home = HomeGuard::set(Some(&home));

        let chain = package_store_root_chain(StoreRootLayers {
            project_root: Some(&project),
            ..StoreRootLayers::default()
        });

        assert_eq!(chain.first(), Some(&project_store));
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn an_explicit_store_always_wins_even_when_it_does_not_exist() {
        let scratch = scratch("explicit_store");
        let project = scratch.join("project");
        populate(&project_store_root(&project));
        let missing = scratch.join("nowhere");
        let _home = HomeGuard::set(None);

        let chain = package_store_root_chain(StoreRootLayers {
            explicit: Some(&missing),
            project_root: Some(&project),
            ..StoreRootLayers::default()
        });

        assert_eq!(
            chain.first(),
            Some(&missing),
            "a wrong --package-store-root must fail loudly, not fall through"
        );
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn an_unset_or_empty_home_contributes_no_layer() {
        let scratch = scratch("no_home");
        let project = scratch.join("project");
        for value in [None, Some(Path::new(""))] {
            let _home = HomeGuard::set(value);
            let chain = package_store_root_chain(StoreRootLayers {
                project_root: Some(&project),
                ..StoreRootLayers::default()
            });
            assert!(
                chain.iter().all(|root| !root.ends_with("home/pkg")),
                "no home layer should appear: {chain:?}"
            );
        }
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn the_write_root_never_becomes_the_home_or_bundled_store() {
        let scratch = scratch("write_root");
        let project = scratch.join("project");
        let home = scratch.join("home");
        populate(&home.join("pkg"));
        let _home = HomeGuard::set(Some(&home));

        let write_root = package_store_write_root(StoreRootLayers {
            project_root: Some(&project),
            ..StoreRootLayers::default()
        });

        assert_eq!(
            write_root,
            Some(project_store_root(&project)),
            "a fetch must never write into a shared or toolchain store"
        );
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn store_entry_detection_distinguishes_missing_empty_and_populated() {
        let scratch = scratch("entries");
        let empty = scratch.join("empty");
        std::fs::create_dir_all(&empty).expect("empty dir should be creatable");

        assert!(!store_has_entries(&scratch.join("missing")));
        assert!(!store_has_entries(&empty));
        assert!(store_has_entries(&populate(&scratch.join("full"))));
        std::fs::remove_dir_all(&scratch).ok();
    }

    #[test]
    fn project_store_root_has_one_spelling() {
        assert_eq!(
            project_store_root(Path::new("/w")),
            PathBuf::from("/w/.fol/pkg")
        );
    }
}

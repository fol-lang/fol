use crate::{FrontendError, FrontendErrorKind, FrontendResult};
use std::path::PathBuf;

pub const WORKSPACE_FILE_NAME: &str = "fol.work.yaml";
pub const PACKAGE_FILE_NAME: &str = "build.fol";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRoot {
    pub root: PathBuf,
    pub config_file: PathBuf,
}

impl WorkspaceRoot {
    pub fn new(root: PathBuf) -> Self {
        Self {
            config_file: root.join(WORKSPACE_FILE_NAME),
            root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageRoot {
    pub root: PathBuf,
    pub control_file: PathBuf,
}

impl PackageRoot {
    pub fn new(root: PathBuf) -> Self {
        Self {
            control_file: root.join(PACKAGE_FILE_NAME),
            root,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveredRoot {
    Workspace(WorkspaceRoot),
    Package(PackageRoot),
}

pub fn discover_root_upward(start: &std::path::Path) -> Option<DiscoveredRoot> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        let workspace = current.join(WORKSPACE_FILE_NAME);
        if workspace.is_file() {
            return Some(DiscoveredRoot::Workspace(WorkspaceRoot::new(current)));
        }

        let package = current.join(PACKAGE_FILE_NAME);
        if package.is_file() {
            return Some(DiscoveredRoot::Package(PackageRoot::new(current)));
        }

        if !current.pop() {
            return None;
        }
    }
}

pub fn discover_root_from_explicit_path(path: &std::path::Path) -> Option<DiscoveredRoot> {
    discover_root_upward(path)
}

pub fn require_discovered_root(path: &std::path::Path) -> FrontendResult<DiscoveredRoot> {
    discover_root_from_explicit_path(path).ok_or_else(|| {
        FrontendError::new(
            FrontendErrorKind::WorkspaceNotFound,
            format!(
                "could not find a FOL workspace or package root from '{}'",
                path.display()
            ),
        )
        .with_note("run `fol work init --bin` to initialize a package in the current directory")
        .with_note("run `fol work init --workspace` to initialize a workspace root")
    })
}

#[cfg(test)]
mod tests {
    use super::{
        discover_root_from_explicit_path, discover_root_upward, require_discovered_root,
        DiscoveredRoot, PackageRoot, WorkspaceRoot, PACKAGE_FILE_NAME, WORKSPACE_FILE_NAME,
    };
    use std::{fs, path::PathBuf};

    #[test]
    fn workspace_root_model_uses_canonical_workspace_filename() {
        let root = WorkspaceRoot::new(PathBuf::from("/tmp/demo"));

        assert_eq!(WORKSPACE_FILE_NAME, "fol.work.yaml");
        assert_eq!(root.root, PathBuf::from("/tmp/demo"));
        assert_eq!(
            root.config_file,
            PathBuf::from("/tmp/demo").join("fol.work.yaml")
        );
    }

    #[test]
    fn package_root_model_uses_canonical_package_filename() {
        let root = PackageRoot::new(PathBuf::from("/tmp/demo"));

        assert_eq!(PACKAGE_FILE_NAME, "build.fol");
        assert_eq!(root.root, PathBuf::from("/tmp/demo"));
        assert_eq!(
            root.control_file,
            PathBuf::from("/tmp/demo").join("build.fol")
        );
    }

    #[test]
    fn upward_discovery_prefers_workspace_then_package_roots() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_discovery_{}", std::process::id()));
        let package_root = root.join("pkg");
        let nested = package_root.join("src").join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("fol.work.yaml"), "members: []\n").unwrap();
        fs::write(
            package_root.join("build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"demo\", version = \"0.1.0\" });\n    return;\n};\n",
        )
        .unwrap();

        let discovered = discover_root_upward(&nested).unwrap();

        assert_eq!(
            discovered,
            DiscoveredRoot::Package(PackageRoot::new(package_root.clone()))
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn explicit_path_selection_reuses_root_discovery() {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_explicit_discovery_{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("fol.work.yaml"), "members: []\n").unwrap();

        let discovered = discover_root_from_explicit_path(&root).unwrap();

        assert_eq!(
            discovered,
            DiscoveredRoot::Workspace(WorkspaceRoot::new(root.clone()))
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn missing_roots_lower_into_frontend_workspace_not_found_errors() {
        let root =
            std::env::temp_dir().join(format!("fol_frontend_missing_root_{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();

        let error = require_discovered_root(&root).unwrap_err();

        assert_eq!(error.kind(), crate::FrontendErrorKind::WorkspaceNotFound);
        assert!(error
            .message()
            .contains("could not find a FOL workspace or package root"));
        assert_eq!(error.notes().len(), 2);
        assert!(error.notes()[0].contains("fol work init --bin"));

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_discovery_handles_starting_from_files() {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_file_discovery_{}",
            std::process::id()
        ));
        let nested = root.join("pkg").join("src");
        let main_file = nested.join("main.fol");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("fol.work.yaml"), "members: []\n").unwrap();
        fs::write(
            root.join("pkg").join("build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"demo\", version = \"0.1.0\" });\n    return;\n};\n",
        )
        .unwrap();
        fs::write(&main_file, "fun[] main(): int = { return 0 }\n").unwrap();

        let discovered = discover_root_upward(&main_file).unwrap();

        assert_eq!(
            discovered,
            DiscoveredRoot::Package(PackageRoot::new(root.join("pkg")))
        );

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_file_takes_priority_over_outer_package_file() {
        let root = std::env::temp_dir().join(format!(
            "fol_frontend_workspace_priority_{}",
            std::process::id()
        ));
        let workspace = root.join("ws");
        let nested = workspace.join("member");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            root.join("build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"outer\", version = \"0.1.0\" });\n    return;\n};\n",
        )
        .unwrap();
        fs::write(workspace.join("fol.work.yaml"), "members: []\n").unwrap();
        fs::write(
            nested.join("build.fol"),
            "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"inner\", version = \"0.1.0\" });\n    return;\n};\n",
        )
        .unwrap();

        let discovered = discover_root_upward(&nested).unwrap();

        assert_eq!(
            discovered,
            DiscoveredRoot::Package(PackageRoot::new(nested.clone()))
        );

        fs::remove_dir_all(root).ok();
    }
}

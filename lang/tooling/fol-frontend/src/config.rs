use crate::{FrontendOutputConfig, FrontendProfile, OutputMode};
use fol_backend::BackendMachineTarget;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendConfig {
    pub working_directory: PathBuf,
    pub output: FrontendOutputConfig,
    pub profile_override: Option<FrontendProfile>,
    pub std_root_override: Option<PathBuf>,
    pub package_store_root_override: Option<PathBuf>,
    pub build_root_override: Option<PathBuf>,
    pub cache_root_override: Option<PathBuf>,
    pub git_cache_root_override: Option<PathBuf>,
    pub install_prefix_override: Option<PathBuf>,
    pub build_target_override: Option<String>,
    pub build_optimize_override: Option<String>,
    pub build_option_overrides: Vec<String>,
    pub build_step_override: Option<String>,
    pub keep_build_dir: bool,
    pub locked_fetch: bool,
    pub offline_fetch: bool,
    pub refresh_fetch: bool,
}

impl Default for FrontendConfig {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            output: FrontendOutputConfig::default(),
            profile_override: None,
            std_root_override: None,
            package_store_root_override: None,
            build_root_override: None,
            cache_root_override: None,
            git_cache_root_override: None,
            install_prefix_override: None,
            build_target_override: None,
            build_optimize_override: None,
            build_option_overrides: Vec::new(),
            build_step_override: None,
            keep_build_dir: false,
            locked_fetch: false,
            offline_fetch: false,
            refresh_fetch: false,
        }
    }
}

impl FrontendConfig {
    pub fn host_rust_target_triple() -> Option<&'static str> {
        if cfg!(target_arch = "x86_64") && cfg!(target_os = "linux") && cfg!(target_env = "gnu") {
            Some("x86_64-unknown-linux-gnu")
        } else if cfg!(target_arch = "x86_64")
            && cfg!(target_os = "linux")
            && cfg!(target_env = "musl")
        {
            Some("x86_64-unknown-linux-musl")
        } else if cfg!(target_arch = "aarch64")
            && cfg!(target_os = "linux")
            && cfg!(target_env = "gnu")
        {
            Some("aarch64-unknown-linux-gnu")
        } else if cfg!(target_arch = "aarch64")
            && cfg!(target_os = "linux")
            && cfg!(target_env = "musl")
        {
            Some("aarch64-unknown-linux-musl")
        } else if cfg!(target_arch = "x86_64")
            && cfg!(target_os = "windows")
            && cfg!(target_env = "gnu")
        {
            Some("x86_64-pc-windows-gnu")
        } else if cfg!(target_arch = "x86_64")
            && cfg!(target_os = "windows")
            && cfg!(target_env = "msvc")
        {
            Some("x86_64-pc-windows-msvc")
        } else if cfg!(target_arch = "aarch64")
            && cfg!(target_os = "windows")
            && cfg!(target_env = "msvc")
        {
            Some("aarch64-pc-windows-msvc")
        } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "macos") {
            Some("x86_64-apple-darwin")
        } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
            Some("aarch64-apple-darwin")
        } else {
            None
        }
    }

    pub fn backend_machine_target(&self) -> BackendMachineTarget {
        self.build_target_override
            .as_deref()
            .map(BackendMachineTarget::normalize_input)
            .unwrap_or(BackendMachineTarget::Host)
    }

    pub fn machine_target_runs_on_host(&self) -> bool {
        let machine_target = self.backend_machine_target();
        if machine_target.is_host() {
            return true;
        }
        match (
            machine_target.rust_target_triple(),
            Self::host_rust_target_triple(),
        ) {
            (Some(target), Some(host)) => target == host,
            _ => false,
        }
    }

    pub fn from_env() -> Self {
        let mut config = Self::default();
        config.output.mode = match std::env::var("FOL_OUTPUT").ok().as_deref() {
            Some("plain") => OutputMode::Plain,
            Some("json") => OutputMode::Json,
            _ => OutputMode::Human,
        };
        config.profile_override = match std::env::var("FOL_PROFILE").ok().as_deref() {
            Some("release") => Some(FrontendProfile::Release),
            Some("debug") => Some(FrontendProfile::Debug),
            _ => None,
        };
        config.std_root_override = std::env::var_os("FOL_STD_ROOT").map(PathBuf::from);
        config.package_store_root_override =
            std::env::var_os("FOL_PACKAGE_STORE_ROOT").map(PathBuf::from);
        config.build_root_override = std::env::var_os("FOL_BUILD_ROOT").map(PathBuf::from);
        config.cache_root_override = std::env::var_os("FOL_CACHE_ROOT").map(PathBuf::from);
        config.git_cache_root_override = std::env::var_os("FOL_GIT_CACHE_ROOT").map(PathBuf::from);
        config.install_prefix_override =
            std::env::var_os("FOL_INSTALL_PREFIX").map(PathBuf::from);
        config.build_target_override = std::env::var("FOL_BUILD_TARGET").ok();
        config.build_optimize_override = std::env::var("FOL_BUILD_OPTIMIZE").ok();
        config.build_step_override = std::env::var("FOL_BUILD_STEP").ok();
        config.build_option_overrides = std::env::var("FOL_BUILD_OPTIONS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .filter(|entry| !entry.is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        config.keep_build_dir = std::env::var_os("FOL_KEEP_BUILD_DIR")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        config.locked_fetch = std::env::var_os("FOL_LOCKED")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        config.offline_fetch = std::env::var_os("FOL_OFFLINE")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        config.refresh_fetch = std::env::var_os("FOL_REFRESH")
            .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        config
    }
}

#[cfg(test)]
mod tests {
    use super::FrontendConfig;
    use crate::{FrontendProfile, OutputMode};
    use fol_backend::BackendMachineTarget;

    #[test]
    fn frontend_config_defaults_to_current_working_defaults() {
        let config = FrontendConfig::default();

        assert_eq!(config.output.mode, OutputMode::Human);
        assert!(config.profile_override.is_none());
        assert!(config.std_root_override.is_none());
        assert!(config.package_store_root_override.is_none());
        assert!(config.build_root_override.is_none());
        assert!(config.cache_root_override.is_none());
        assert!(config.git_cache_root_override.is_none());
        assert!(config.install_prefix_override.is_none());
        assert!(config.build_target_override.is_none());
        assert!(config.build_optimize_override.is_none());
        assert!(config.build_option_overrides.is_empty());
        assert!(config.build_step_override.is_none());
        assert!(!config.keep_build_dir);
        assert!(!config.locked_fetch);
        assert!(!config.offline_fetch);
        assert!(!config.refresh_fetch);
    }

    #[test]
    fn frontend_config_reads_root_overrides_from_environment() {
        std::env::set_var("FOL_STD_ROOT", "/tmp/std");
        std::env::set_var("FOL_PACKAGE_STORE_ROOT", "/tmp/pkg");
        std::env::set_var("FOL_BUILD_ROOT", "/tmp/build");
        std::env::set_var("FOL_CACHE_ROOT", "/tmp/cache");
        std::env::set_var("FOL_GIT_CACHE_ROOT", "/tmp/git-cache");
        std::env::set_var("FOL_INSTALL_PREFIX", "/tmp/install");
        std::env::set_var("FOL_BUILD_TARGET", "aarch64-macos-gnu");
        std::env::set_var("FOL_BUILD_OPTIMIZE", "release-fast");
        std::env::set_var("FOL_BUILD_STEP", "docs");
        std::env::set_var("FOL_BUILD_OPTIONS", "jobs=16,strip=true");
        std::env::set_var("FOL_KEEP_BUILD_DIR", "true");
        std::env::set_var("FOL_LOCKED", "true");
        std::env::set_var("FOL_OFFLINE", "true");
        std::env::set_var("FOL_REFRESH", "true");
        std::env::set_var("FOL_OUTPUT", "json");
        std::env::set_var("FOL_PROFILE", "release");

        let config = FrontendConfig::from_env();

        assert_eq!(config.output.mode, OutputMode::Json);
        assert_eq!(config.profile_override, Some(FrontendProfile::Release));
        assert_eq!(
            config.std_root_override,
            Some(std::path::PathBuf::from("/tmp/std"))
        );
        assert_eq!(
            config.package_store_root_override,
            Some(std::path::PathBuf::from("/tmp/pkg"))
        );
        assert_eq!(
            config.build_root_override,
            Some(std::path::PathBuf::from("/tmp/build"))
        );
        assert_eq!(
            config.install_prefix_override,
            Some(std::path::PathBuf::from("/tmp/install"))
        );
        assert_eq!(
            config.cache_root_override,
            Some(std::path::PathBuf::from("/tmp/cache"))
        );
        assert_eq!(
            config.git_cache_root_override,
            Some(std::path::PathBuf::from("/tmp/git-cache"))
        );
        assert_eq!(
            config.build_target_override.as_deref(),
            Some("aarch64-macos-gnu")
        );
        assert_eq!(
            config.build_optimize_override.as_deref(),
            Some("release-fast")
        );
        assert_eq!(config.build_step_override.as_deref(), Some("docs"));
        assert_eq!(
            config.build_option_overrides,
            vec!["jobs=16".to_string(), "strip=true".to_string()]
        );
        assert!(config.keep_build_dir);
        assert!(config.locked_fetch);
        assert!(config.offline_fetch);
        assert!(config.refresh_fetch);

        std::env::remove_var("FOL_STD_ROOT");
        std::env::remove_var("FOL_PACKAGE_STORE_ROOT");
        std::env::remove_var("FOL_BUILD_ROOT");
        std::env::remove_var("FOL_CACHE_ROOT");
        std::env::remove_var("FOL_GIT_CACHE_ROOT");
        std::env::remove_var("FOL_BUILD_TARGET");
        std::env::remove_var("FOL_BUILD_OPTIMIZE");
        std::env::remove_var("FOL_BUILD_STEP");
        std::env::remove_var("FOL_BUILD_OPTIONS");
        std::env::remove_var("FOL_KEEP_BUILD_DIR");
        std::env::remove_var("FOL_LOCKED");
        std::env::remove_var("FOL_OFFLINE");
        std::env::remove_var("FOL_REFRESH");
        std::env::remove_var("FOL_OUTPUT");
        std::env::remove_var("FOL_PROFILE");
    }

    #[test]
    fn frontend_config_reports_host_machine_target_by_default() {
        let config = FrontendConfig::default();

        assert_eq!(config.backend_machine_target(), BackendMachineTarget::Host);
    }

    #[test]
    fn frontend_config_normalizes_machine_target_override_for_backend() {
        let config = FrontendConfig {
            build_target_override: Some("  aarch64-macos-gnu  ".to_string()),
            ..FrontendConfig::default()
        };

        assert_eq!(
            config.backend_machine_target(),
            BackendMachineTarget::Triple("aarch64-macos-gnu".to_string())
        );
    }

    #[test]
    fn frontend_config_exposes_the_current_host_target_triple() {
        assert!(FrontendConfig::host_rust_target_triple().is_some());
    }

    #[test]
    fn frontend_config_reports_host_compatibility_for_host_and_non_host_targets() {
        let host_config = FrontendConfig::default();
        let cross_config = FrontendConfig {
            build_target_override: Some("aarch64-macos-gnu".to_string()),
            ..FrontendConfig::default()
        };

        assert!(host_config.machine_target_runs_on_host());
        if FrontendConfig::host_rust_target_triple() != Some("aarch64-apple-darwin") {
            assert!(!cross_config.machine_target_runs_on_host());
        }
    }
}

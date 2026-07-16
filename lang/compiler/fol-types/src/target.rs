//! Canonical, concrete compilation targets shared by the build, frontend, and
//! backend layers.

/// A supported machine target resolved to the exact Rust target triple used by
/// the backend and by interop fingerprints.
///
/// This type never represents `host`, `native`, an unknown spelling, or an
/// omitted target. Those inputs are resolved at the boundary before a value is
/// constructed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResolvedTarget {
    rust_triple: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveTargetError {
    UnsupportedTarget(String),
    UnsupportedHost,
}

impl std::fmt::Display for ResolveTargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedTarget(target) => {
                write!(f, "unsupported explicit machine target '{target}'")
            }
            Self::UnsupportedHost => write!(
                f,
                "the current host does not map to a supported concrete machine target"
            ),
        }
    }
}

impl std::error::Error for ResolveTargetError {}

impl ResolvedTarget {
    /// Resolve a host alias, FOL target spelling, or supported Rust target
    /// triple. Unknown inputs fail instead of falling back to the host.
    pub fn resolve(raw: &str) -> Result<Self, ResolveTargetError> {
        let raw = raw.trim();
        if matches!(raw, "host" | "native") {
            return Self::host();
        }
        if raw.is_empty() {
            return Err(ResolveTargetError::UnsupportedTarget(String::new()));
        }

        let rust_triple = match raw {
            "x86_64-linux-gnu" | "x86_64-unknown-linux-gnu" => "x86_64-unknown-linux-gnu",
            "x86_64-linux-musl" | "x86_64-unknown-linux-musl" => "x86_64-unknown-linux-musl",
            "aarch64-linux-gnu" | "aarch64-unknown-linux-gnu" => "aarch64-unknown-linux-gnu",
            "aarch64-linux-musl" | "aarch64-unknown-linux-musl" => "aarch64-unknown-linux-musl",
            "x86_64-windows-gnu" | "x86_64-pc-windows-gnu" => "x86_64-pc-windows-gnu",
            "x86_64-windows-msvc" | "x86_64-pc-windows-msvc" => "x86_64-pc-windows-msvc",
            "aarch64-windows-msvc" | "aarch64-pc-windows-msvc" => "aarch64-pc-windows-msvc",
            "x86_64-macos-gnu" | "x86_64-apple-darwin" => "x86_64-apple-darwin",
            "aarch64-macos-gnu" | "aarch64-apple-darwin" => "aarch64-apple-darwin",
            _ => return Err(ResolveTargetError::UnsupportedTarget(raw.to_string())),
        };

        Ok(Self {
            rust_triple: rust_triple.to_string(),
        })
    }

    /// Compatibility parser for existing option APIs. New boundary code
    /// should use [`Self::resolve`] so it can preserve the failure reason.
    pub fn parse(raw: &str) -> Option<Self> {
        Self::resolve(raw).ok()
    }

    pub fn host() -> Result<Self, ResolveTargetError> {
        Ok(Self {
            rust_triple: Self::host_rust_triple()?.to_string(),
        })
    }

    pub fn host_rust_triple() -> Result<&'static str, ResolveTargetError> {
        let rust_triple = if cfg!(target_arch = "x86_64")
            && cfg!(target_os = "linux")
            && cfg!(target_env = "gnu")
        {
            "x86_64-unknown-linux-gnu"
        } else if cfg!(target_arch = "x86_64")
            && cfg!(target_os = "linux")
            && cfg!(target_env = "musl")
        {
            "x86_64-unknown-linux-musl"
        } else if cfg!(target_arch = "aarch64")
            && cfg!(target_os = "linux")
            && cfg!(target_env = "gnu")
        {
            "aarch64-unknown-linux-gnu"
        } else if cfg!(target_arch = "aarch64")
            && cfg!(target_os = "linux")
            && cfg!(target_env = "musl")
        {
            "aarch64-unknown-linux-musl"
        } else if cfg!(target_arch = "x86_64")
            && cfg!(target_os = "windows")
            && cfg!(target_env = "gnu")
        {
            "x86_64-pc-windows-gnu"
        } else if cfg!(target_arch = "x86_64")
            && cfg!(target_os = "windows")
            && cfg!(target_env = "msvc")
        {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_arch = "aarch64")
            && cfg!(target_os = "windows")
            && cfg!(target_env = "msvc")
        {
            "aarch64-pc-windows-msvc"
        } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "macos") {
            "x86_64-apple-darwin"
        } else if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
            "aarch64-apple-darwin"
        } else {
            return Err(ResolveTargetError::UnsupportedHost);
        };

        Ok(rust_triple)
    }

    pub fn as_str(&self) -> &str {
        &self.rust_triple
    }

    /// Render the stable FOL build-option spelling.
    ///
    /// Build programs have historically compared `standard_target()` values
    /// with compact spellings such as `x86_64-linux-gnu`.  Backend and interop
    /// consumers must use [`Self::rust_target_triple`] instead; changing the
    /// build-language value to a Rust vendor triple would silently change
    /// `when(target == ...)` and `case(...)` behavior.
    pub fn render(&self) -> String {
        match self.rust_triple.as_str() {
            "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
            "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
            "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
            "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
            "x86_64-pc-windows-gnu" => "x86_64-windows-gnu",
            "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
            "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
            "x86_64-apple-darwin" => "x86_64-macos-gnu",
            "aarch64-apple-darwin" => "aarch64-macos-gnu",
            _ => unreachable!("ResolvedTarget only stores supported Rust triples"),
        }
        .to_string()
    }

    pub fn rust_target_triple(&self) -> &str {
        self.as_str()
    }

    pub fn rust_target_directory_name(&self) -> &str {
        self.as_str()
    }

    pub fn runs_on_host(&self) -> Result<bool, ResolveTargetError> {
        Ok(*self == Self::host()?)
    }
}

impl std::fmt::Display for ResolvedTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{ResolveTargetError, ResolvedTarget};

    #[test]
    fn resolves_all_supported_spellings_to_rust_triples() {
        assert_eq!(
            ResolvedTarget::resolve("x86_64-linux-gnu")
                .unwrap()
                .as_str(),
            "x86_64-unknown-linux-gnu"
        );
        assert_eq!(
            ResolvedTarget::resolve(" aarch64-apple-darwin ")
                .unwrap()
                .as_str(),
            "aarch64-apple-darwin"
        );
    }

    #[test]
    fn renders_stable_fol_option_values_without_losing_rust_identity() {
        let target = ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap();

        assert_eq!(target.render(), "x86_64-linux-gnu");
        assert_eq!(target.rust_target_triple(), "x86_64-unknown-linux-gnu");
        assert_eq!(ResolvedTarget::resolve(&target.render()).unwrap(), target);
    }

    #[test]
    fn host_aliases_resolve_to_one_concrete_value() {
        let host = ResolvedTarget::host().unwrap();
        assert_eq!(ResolvedTarget::resolve("host").unwrap(), host);
        assert_eq!(ResolvedTarget::resolve("native").unwrap(), host);
        assert!(!host.as_str().is_empty());
    }

    #[test]
    fn unknown_targets_do_not_fall_back() {
        assert_eq!(
            ResolvedTarget::resolve("mystery-vendor-os"),
            Err(ResolveTargetError::UnsupportedTarget(
                "mystery-vendor-os".to_string()
            ))
        );
        assert_eq!(
            ResolvedTarget::resolve("   "),
            Err(ResolveTargetError::UnsupportedTarget(String::new()))
        );
    }
}

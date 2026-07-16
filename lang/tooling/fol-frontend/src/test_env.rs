use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) const FRONTEND_ENV_KEYS: &[&str] = &[
    "FOL_OUTPUT",
    "FOL_PROFILE",
    "FOL_STD_ROOT",
    "FOL_PACKAGE_STORE_ROOT",
    "FOL_BUILD_ROOT",
    "FOL_CACHE_ROOT",
    "FOL_GIT_CACHE_ROOT",
    "FOL_INSTALL_PREFIX",
    "FOL_BUILD_TARGET",
    "FOL_BUILD_OPTIMIZE",
    "FOL_BUILD_STEP",
    "FOL_INTEROP_GCC",
    "FOL_INTEROP_TEMP",
    "FOL_BUILD_OPTIONS",
    "FOL_KEEP_BUILD_DIR",
    "FOL_LOCKED",
    "FOL_OFFLINE",
    "FOL_REFRESH",
];

pub(crate) struct EnvironmentGuard {
    previous: Vec<(&'static str, Option<OsString>)>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvironmentGuard {
    pub(crate) fn removed(keys: &[&'static str]) -> Self {
        let lock = environment_lock();
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect();
        for key in keys {
            std::env::remove_var(key);
        }
        Self {
            previous,
            _lock: lock,
        }
    }

    pub(crate) fn set(values: &[(&'static str, &'static str)]) -> Self {
        let lock = environment_lock();
        let previous = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect();
        for (key, value) in values {
            std::env::set_var(key, value);
        }
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn environment_lock() -> MutexGuard<'static, ()> {
    static ENVIRONMENT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENVIRONMENT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

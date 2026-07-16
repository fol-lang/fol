use fol_frontend::{run_command_from_args_in_dir, FrontendCli, FrontendProfile, OutputMode};
use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn env_lock() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct EnvGuard {
    key: &'static str,
    old: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, old }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old {
            Some(value) => {
                std::env::set_var(self.key, value);
            }
            None => {
                std::env::remove_var(self.key);
            }
        }
    }
}

#[test]
fn frontend_dispatch_uses_env_defaults_for_output() {
    let _env = env_lock();
    let _output = EnvGuard::set("FOL_OUTPUT", "plain");
    let _profile = EnvGuard::set("FOL_PROFILE", "release");

    let (output, _) =
        run_command_from_args_in_dir(["fol", "_complete"], std::env::temp_dir()).unwrap();

    assert_eq!(output.config().mode, OutputMode::Plain);

    let cli = FrontendCli::parse_from(["fol", "build"]);
    assert_eq!(cli.selected_profile(), FrontendProfile::Release);
}

#[test]
fn frontend_dispatch_flags_override_env_defaults_for_output_and_profile() {
    let _env = env_lock();
    let _output = EnvGuard::set("FOL_OUTPUT", "plain");
    let _profile = EnvGuard::set("FOL_PROFILE", "release");

    let (output, _) = run_command_from_args_in_dir(
        ["fol", "--output", "json", "_complete"],
        std::env::temp_dir(),
    )
    .unwrap();

    assert_eq!(output.config().mode, OutputMode::Json);

    let cli = FrontendCli::parse_from(["fol", "--debug", "build"]);
    assert_eq!(cli.selected_profile(), FrontendProfile::Debug);
}

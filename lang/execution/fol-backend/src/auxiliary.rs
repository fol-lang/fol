use crate::config::{BackendBuildProfile, BackendMachineTarget};
use crate::error::{BackendError, BackendErrorKind};
use crate::BackendResult;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// One pre-generated, `no_std` Rust crate compiled as part of a backend build.
///
/// `source_root` is the exact root module passed to `rustc` (normally a
/// generated `lib.rs`). Dependencies name earlier crates in the same ordered
/// [`BackendAuxiliaryRustPlan`]. The backend never discovers dependencies from
/// source text or from the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendAuxiliaryRustCrate {
    crate_name: String,
    source_root: PathBuf,
    dependencies: Vec<String>,
}

impl BackendAuxiliaryRustCrate {
    pub fn try_new(
        crate_name: impl Into<String>,
        source_root: PathBuf,
        dependencies: Vec<String>,
    ) -> BackendResult<Self> {
        let crate_name = crate_name.into();
        validate_rust_identifier(&crate_name, "auxiliary crate name")?;
        validate_source_root(&source_root)?;

        let mut seen_dependencies = BTreeSet::new();
        for dependency in &dependencies {
            validate_rust_identifier(dependency, "auxiliary dependency name")?;
            if dependency == &crate_name {
                return Err(invalid_plan(format!(
                    "auxiliary crate '{crate_name}' cannot depend on itself"
                )));
            }
            if !seen_dependencies.insert(dependency.as_str()) {
                return Err(invalid_plan(format!(
                    "auxiliary crate '{crate_name}' repeats dependency '{dependency}'"
                )));
            }
        }

        Ok(Self {
            crate_name,
            source_root,
            dependencies,
        })
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    fn validate_current_source(&self) -> BackendResult<()> {
        validate_source_root(&self.source_root)
    }
}

/// How generated `main.rs` observes the result of a safe auxiliary entry call.
///
/// The default preserves the historical backend behavior: invoke the entry,
/// pass its result through `black_box`, and discard it. `StdoutI32` is an
/// explicit narrow observation lane for system tests and other checked
/// consumers that need to prove the called native value reached the final
/// executable. Its generated `i32` binding makes a mismatched return type a
/// Rust compile error rather than an implicit conversion.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BackendMainEntryResultObservation {
    #[default]
    Discard,
    StdoutI32,
}

/// A safe function in the final auxiliary crate that generated `main.rs`
/// calls directly.
///
/// Every path segment is a conservative Rust identifier. Raw identifiers,
/// keywords, expressions, generic arguments, and `unsafe` call syntax cannot
/// enter generated source through this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendMainEntryCall {
    crate_name: String,
    function_path: Vec<String>,
    result_observation: BackendMainEntryResultObservation,
}

impl BackendMainEntryCall {
    pub fn try_new(
        crate_name: impl Into<String>,
        function_path: Vec<String>,
    ) -> BackendResult<Self> {
        Self::try_new_with_result_observation(
            crate_name,
            function_path,
            BackendMainEntryResultObservation::Discard,
        )
    }

    pub fn try_new_with_result_observation(
        crate_name: impl Into<String>,
        function_path: Vec<String>,
        result_observation: BackendMainEntryResultObservation,
    ) -> BackendResult<Self> {
        let crate_name = crate_name.into();
        validate_rust_identifier(&crate_name, "auxiliary entry crate name")?;
        if function_path.is_empty() {
            return Err(invalid_plan(
                "auxiliary main entry call must contain a function path",
            ));
        }
        for segment in &function_path {
            validate_rust_identifier(segment, "auxiliary entry function path segment")?;
        }
        Ok(Self {
            crate_name,
            function_path,
            result_observation,
        })
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn function_path(&self) -> &[String] {
        &self.function_path
    }

    pub const fn result_observation(&self) -> BackendMainEntryResultObservation {
        self.result_observation
    }

    pub(crate) fn render_rust_path(&self) -> String {
        let mut rendered = self.crate_name.clone();
        for segment in &self.function_path {
            rendered.push_str("::");
            rendered.push_str(segment);
        }
        rendered
    }
}

/// Validated, ordered auxiliary Rust compilation attached to one backend
/// target/profile pair.
///
/// The final rustc argv is deliberately opaque. Its [`OsString`] values are
/// retained in order and later appended one-for-one to the final binary
/// command; the backend never parses, joins, normalizes, or deduplicates them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendAuxiliaryRustPlan {
    machine_target: BackendMachineTarget,
    build_profile: BackendBuildProfile,
    crates: Vec<BackendAuxiliaryRustCrate>,
    entry_call: BackendMainEntryCall,
    final_rustc_argv: Vec<OsString>,
}

impl BackendAuxiliaryRustPlan {
    pub fn try_new(
        machine_target: BackendMachineTarget,
        build_profile: BackendBuildProfile,
        crates: Vec<BackendAuxiliaryRustCrate>,
        entry_call: BackendMainEntryCall,
        final_rustc_argv: Vec<OsString>,
    ) -> BackendResult<Self> {
        let plan = Self {
            machine_target,
            build_profile,
            crates,
            entry_call,
            final_rustc_argv,
        };
        plan.validate_structure_and_sources()?;
        Ok(plan)
    }

    pub fn machine_target(&self) -> &BackendMachineTarget {
        &self.machine_target
    }

    pub fn build_profile(&self) -> BackendBuildProfile {
        self.build_profile
    }

    pub fn crates(&self) -> &[BackendAuxiliaryRustCrate] {
        &self.crates
    }

    pub fn entry_call(&self) -> &BackendMainEntryCall {
        &self.entry_call
    }

    pub fn final_rustc_argv(&self) -> &[OsString] {
        &self.final_rustc_argv
    }

    /// Revalidate the pinned build identity and every source before any rustc
    /// process is launched. This closes the gap between plan construction and
    /// execution when a generated source has been moved or removed.
    pub fn validate_for_build(
        &self,
        machine_target: &BackendMachineTarget,
        build_profile: BackendBuildProfile,
    ) -> BackendResult<()> {
        if &self.machine_target != machine_target {
            return Err(invalid_plan(format!(
                "auxiliary plan target '{}' does not match backend target '{}'",
                self.machine_target, machine_target
            )));
        }
        if self.build_profile != build_profile {
            return Err(invalid_plan(format!(
                "auxiliary plan profile '{}' does not match backend profile '{}'",
                self.build_profile.as_str(),
                build_profile.as_str()
            )));
        }
        self.validate_structure_and_sources()
    }

    fn validate_structure_and_sources(&self) -> BackendResult<()> {
        if self.crates.is_empty() {
            return Err(invalid_plan(
                "auxiliary Rust plan must contain at least one crate",
            ));
        }

        let mut earlier_crates = BTreeSet::new();
        for auxiliary_crate in &self.crates {
            validate_rust_identifier(auxiliary_crate.crate_name(), "auxiliary crate name")?;
            auxiliary_crate.validate_current_source()?;
            if earlier_crates.contains(auxiliary_crate.crate_name()) {
                return Err(invalid_plan(format!(
                    "auxiliary Rust plan repeats crate '{}'",
                    auxiliary_crate.crate_name()
                )));
            }
            for dependency in auxiliary_crate.dependencies() {
                if !earlier_crates.contains(dependency.as_str()) {
                    return Err(invalid_plan(format!(
                        "auxiliary crate '{}' dependency '{}' is not an earlier crate in the plan",
                        auxiliary_crate.crate_name(),
                        dependency
                    )));
                }
            }
            earlier_crates.insert(auxiliary_crate.crate_name());
        }

        let final_crate = self
            .crates
            .last()
            .expect("non-empty auxiliary plan checked above");
        if self.entry_call.crate_name() != final_crate.crate_name() {
            return Err(invalid_plan(format!(
                "auxiliary main entry crate '{}' is not the final plan crate '{}'",
                self.entry_call.crate_name(),
                final_crate.crate_name()
            )));
        }
        Ok(())
    }
}

fn validate_source_root(source_root: &Path) -> BackendResult<()> {
    if !source_root.is_absolute() {
        return Err(invalid_plan(format!(
            "auxiliary crate source root '{}' must be absolute",
            source_root.display()
        )));
    }
    if source_root
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(invalid_plan(format!(
            "auxiliary crate source root '{}' must not contain '.' or '..' components",
            source_root.display()
        )));
    }

    let link_metadata = fs::symlink_metadata(source_root).map_err(|error| {
        invalid_plan(format!(
            "auxiliary crate source root '{}' is unavailable: {error}",
            source_root.display()
        ))
    })?;
    if link_metadata.file_type().is_symlink() {
        return Err(invalid_plan(format!(
            "auxiliary crate source root '{}' must not be a symbolic link",
            source_root.display()
        )));
    }
    if !link_metadata.is_file() {
        return Err(invalid_plan(format!(
            "auxiliary crate source root '{}' is not a regular file",
            source_root.display()
        )));
    }
    fs::File::open(source_root).map_err(|error| {
        invalid_plan(format!(
            "auxiliary crate source root '{}' is not readable: {error}",
            source_root.display()
        ))
    })?;
    Ok(())
}

fn validate_rust_identifier(value: &str, role: &str) -> BackendResult<()> {
    let mut chars = value.chars();
    let starts_safely = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic());
    let continues_safely =
        chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if !starts_safely || !continues_safely || value == "_" || is_reserved_rust_identifier(value) {
        return Err(invalid_plan(format!(
            "{role} '{value}' is not a safe Rust identifier"
        )));
    }
    Ok(())
}

fn is_reserved_rust_identifier(value: &str) -> bool {
    matches!(
        value,
        "Self"
            | "abstract"
            | "alignof"
            | "as"
            | "async"
            | "await"
            | "become"
            | "box"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "do"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "final"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "macro"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "override"
            | "offsetof"
            | "priv"
            | "proc"
            | "pub"
            | "pure"
            | "ref"
            | "return"
            | "self"
            | "sizeof"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "try"
            | "type"
            | "typeof"
            | "union"
            | "unsafe"
            | "unsized"
            | "use"
            | "virtual"
            | "where"
            | "while"
            | "yield"
    )
}

fn invalid_plan(message: impl Into<String>) -> BackendError {
    BackendError::new(BackendErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::{
        BackendAuxiliaryRustCrate, BackendAuxiliaryRustPlan, BackendMainEntryCall,
        BackendMainEntryResultObservation,
    };
    use crate::{BackendBuildProfile, BackendMachineTarget};
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("fol_backend_aux_{label}_{unique}"))
    }

    fn write_source(root: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(root).expect("temp source root");
        let path = root.join(name);
        fs::write(&path, "#![no_std]\n").expect("temp source");
        path
    }

    #[test]
    fn entry_result_observation_defaults_to_discard_and_requires_explicit_i32_stdout() {
        let discarded =
            BackendMainEntryCall::try_new("fol_anchor", vec!["invoke".to_string()]).unwrap();
        let observed = BackendMainEntryCall::try_new_with_result_observation(
            "fol_anchor",
            vec!["invoke".to_string()],
            BackendMainEntryResultObservation::StdoutI32,
        )
        .unwrap();

        assert_eq!(
            discarded.result_observation(),
            BackendMainEntryResultObservation::Discard
        );
        assert_eq!(
            observed.result_observation(),
            BackendMainEntryResultObservation::StdoutI32
        );
    }

    #[test]
    fn plan_preserves_ordered_crates_and_opaque_final_argv() {
        let root = temp_root("order");
        let raw =
            BackendAuxiliaryRustCrate::try_new("fol_raw", write_source(&root, "raw.rs"), vec![])
                .unwrap();
        let anchor = BackendAuxiliaryRustCrate::try_new(
            "fol_anchor",
            write_source(&root, "anchor.rs"),
            vec!["fol_raw".to_string()],
        )
        .unwrap();
        let argv = vec![
            OsString::from("-C"),
            OsString::from("link-arg=/tmp/provider with spaces.o"),
            OsString::from("-C"),
            OsString::from("link-arg=/tmp/provider with spaces.o"),
        ];
        let plan = BackendAuxiliaryRustPlan::try_new(
            BackendMachineTarget::host().unwrap(),
            BackendBuildProfile::Debug,
            vec![raw, anchor],
            BackendMainEntryCall::try_new("fol_anchor", vec!["invoke".to_string()]).unwrap(),
            argv.clone(),
        )
        .unwrap();

        assert_eq!(
            plan.crates()
                .iter()
                .map(BackendAuxiliaryRustCrate::crate_name)
                .collect::<Vec<_>>(),
            ["fol_raw", "fol_anchor"]
        );
        assert_eq!(plan.final_rustc_argv(), argv);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn crates_reject_relative_missing_and_unsafe_inputs() {
        let root = temp_root("invalid");
        let valid_source = write_source(&root, "lib.rs");

        assert!(BackendAuxiliaryRustCrate::try_new(
            "fol_raw",
            PathBuf::from("relative/lib.rs"),
            vec![]
        )
        .is_err());
        assert!(
            BackendAuxiliaryRustCrate::try_new("fol_raw", root.join("missing.rs"), vec![]).is_err()
        );
        assert!(BackendAuxiliaryRustCrate::try_new("match", valid_source.clone(), vec![]).is_err());
        assert!(
            BackendMainEntryCall::try_new("fol_anchor", vec!["safe(); unsafe".to_string()])
                .is_err()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn plan_rejects_duplicates_forward_dependencies_and_wrong_entry_crate() {
        let root = temp_root("structure");
        let raw_path = write_source(&root, "raw.rs");
        let anchor_path = write_source(&root, "anchor.rs");
        let target = BackendMachineTarget::host().unwrap();
        let duplicate = BackendAuxiliaryRustPlan::try_new(
            target.clone(),
            BackendBuildProfile::Debug,
            vec![
                BackendAuxiliaryRustCrate::try_new("fol_raw", raw_path.clone(), vec![]).unwrap(),
                BackendAuxiliaryRustCrate::try_new("fol_raw", anchor_path.clone(), vec![]).unwrap(),
            ],
            BackendMainEntryCall::try_new("fol_raw", vec!["invoke".to_string()]).unwrap(),
            vec![],
        );
        assert!(duplicate.is_err());

        let forward = BackendAuxiliaryRustPlan::try_new(
            target.clone(),
            BackendBuildProfile::Debug,
            vec![
                BackendAuxiliaryRustCrate::try_new(
                    "fol_anchor",
                    anchor_path.clone(),
                    vec!["fol_raw".to_string()],
                )
                .unwrap(),
                BackendAuxiliaryRustCrate::try_new("fol_raw", raw_path.clone(), vec![]).unwrap(),
            ],
            BackendMainEntryCall::try_new("fol_raw", vec!["invoke".to_string()]).unwrap(),
            vec![],
        );
        assert!(forward.is_err());

        let wrong_entry = BackendAuxiliaryRustPlan::try_new(
            target,
            BackendBuildProfile::Debug,
            vec![
                BackendAuxiliaryRustCrate::try_new("fol_raw", raw_path, vec![]).unwrap(),
                BackendAuxiliaryRustCrate::try_new(
                    "fol_anchor",
                    anchor_path,
                    vec!["fol_raw".to_string()],
                )
                .unwrap(),
            ],
            BackendMainEntryCall::try_new("fol_raw", vec!["invoke".to_string()]).unwrap(),
            vec![],
        );
        assert!(wrong_entry.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn plan_revalidates_target_profile_and_removed_sources() {
        let root = temp_root("revalidate");
        let source = write_source(&root, "anchor.rs");
        let target = BackendMachineTarget::host().unwrap();
        let plan = BackendAuxiliaryRustPlan::try_new(
            target.clone(),
            BackendBuildProfile::Debug,
            vec![BackendAuxiliaryRustCrate::try_new("fol_anchor", source.clone(), vec![]).unwrap()],
            BackendMainEntryCall::try_new("fol_anchor", vec!["invoke".to_string()]).unwrap(),
            vec![],
        )
        .unwrap();

        assert!(plan
            .validate_for_build(&target, BackendBuildProfile::Release)
            .is_err());
        let other_target = if target.as_str() == "aarch64-unknown-linux-gnu" {
            BackendMachineTarget::resolve("x86_64-unknown-linux-gnu").unwrap()
        } else {
            BackendMachineTarget::resolve("aarch64-unknown-linux-gnu").unwrap()
        };
        assert!(plan
            .validate_for_build(&other_target, BackendBuildProfile::Debug)
            .is_err());
        fs::remove_file(source).unwrap();
        assert!(plan
            .validate_for_build(&target, BackendBuildProfile::Debug)
            .is_err());
        let _ = fs::remove_dir_all(root);
    }
}

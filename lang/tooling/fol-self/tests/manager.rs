//! Behavioral tests for the `fol` toolchain manager.
//!
//! Every test drives the real binary in a private FOL_HOME and passes its
//! environment per child (never `set_var`), which is what keeps them
//! parallel-safe. Network installs are exercised through PATH shims for
//! `curl`/`tar`, so nothing here reaches the internet.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

// ------------------------------------------------------------------ harness

fn temp_root(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after the unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "fol_self_{}_{}_{}",
        label,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&root).expect("temp root should be creatable");
    root
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory should be creatable");
    }
    fs::write(path, contents).expect("file should be writable");
}

fn write_executable(path: &Path, contents: &str) {
    write(path, contents);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .expect("script should be made executable");
}

/// A source tree shaped like the fol repository, with a stub `folc` that echoes
/// its arguments and its environment on request.
fn fake_repo(root: &Path) -> PathBuf {
    let repo = root.join("repo");
    write_executable(
        &repo.join("target/debug/folc"),
        "#!/bin/sh\n\
         if [ \"$1\" = \"--print-env\" ]; then\n\
         echo \"FOL_STD_ROOT=${FOL_STD_ROOT-}\"\n\
         echo \"FOL_DISPATCH_DEPTH=${FOL_DISPATCH_DEPTH-}\"\n\
         echo \"FOL_DISPATCHED=${FOL_DISPATCHED-}\"\n\
         exit 0\n\
         fi\n\
         if [ \"$1\" = \"--exit-seven\" ]; then echo \"args: $*\"; exit 7; fi\n\
         echo \"args: $*\"\n",
    );
    write(&repo.join("lang/library/std/lib.fol"), "// staged std\n");
    write(&repo.join("lang/library/std/marker.fol"), "// marker\n");
    write(
        &repo.join("lang/execution/fol-runtime/Cargo.toml"),
        "[package]\nname = \"fol-runtime\"\n",
    );
    write(
        &repo.join("lang/execution/fol-runtime/src/lib.rs"),
        "// staged runtime\n",
    );
    repo
}

fn manager() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fol"))
}

fn fol(home: &Path, cwd: &Path, args: &[&str]) -> Output {
    manager()
        .args(args)
        .env("FOL_HOME", home)
        .env_remove("FOL_TOOLCHAIN")
        .env_remove("FOL_STD_ROOT")
        .env_remove("FOL_DISPATCH_DEPTH")
        .current_dir(cwd)
        .output()
        .expect("the fol manager should run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn assert_failed(output: &Output, what: &str) {
    assert!(
        !output.status.success(),
        "{what} should have failed\nstdout=\n{}\nstderr=\n{}",
        stdout_of(output),
        stderr_of(output)
    );
}

fn assert_succeeded(output: &Output, what: &str) {
    assert!(
        output.status.success(),
        "{what} should have succeeded\nstdout=\n{}\nstderr=\n{}",
        stdout_of(output),
        stderr_of(output)
    );
}

fn install_from(home: &Path, cwd: &Path, version: &str, repo: &Path) -> Output {
    fol(
        home,
        cwd,
        &[
            "self",
            "install",
            version,
            "--from",
            repo.to_str().expect("repo path should be utf-8"),
        ],
    )
}

fn toolchain_dir(home: &Path, version: &str) -> PathBuf {
    home.join("toolchains").join(format!("v{version}"))
}

fn bookkeeping_entries(home: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(home.join("toolchains")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with('.'))
        .collect()
}

// ------------------------------------------- traversal and containment gates

#[test]
fn remove_rejects_a_parent_traversal_spec() {
    let root = temp_root("remove_traversal");
    let home = root.join("home");
    let victim = root.join("victim/precious");
    write(&victim.join("file.txt"), "do not delete\n");

    let output = fol(
        &home,
        &root,
        &["self", "remove", "0.2.0/../../../victim/precious"],
    );

    assert_failed(&output, "removing a traversal spec");
    assert!(
        stderr_of(&output).contains("not a valid toolchain spec"),
        "stderr should explain the rejection: {}",
        stderr_of(&output)
    );
    assert!(
        !stdout_of(&output).contains("removed toolchain"),
        "the manager must not claim it removed anything"
    );
    assert!(
        victim.join("file.txt").is_file(),
        "the directory outside FOL_HOME must survive"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn install_rejects_traversal_and_absolute_specs() {
    let root = temp_root("install_traversal");
    let home = root.join("home");
    let repo = fake_repo(&root);

    for spec in ["0.9.9/../../../../escaped", "/tmp/pwn", "../evil", "."] {
        let output = install_from(&home, &root, spec, &repo);
        assert_failed(&output, &format!("installing spec {spec:?}"));
    }
    assert!(
        !root.join("escaped").exists(),
        "nothing may be written outside the toolchains directory"
    );
    let installed = fs::read_dir(home.join("toolchains"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(installed, 0, "no toolchain should have been created");

    fs::remove_dir_all(&root).ok();
}

#[test]
fn link_rejects_traversal_empty_and_version_shaped_names() {
    let root = temp_root("link_names");
    let home = root.join("home");
    let repo = fake_repo(&root);
    let repo_arg = repo.to_str().expect("repo path should be utf-8");

    for name in ["../../pwned", "", "0.2.0", ".hidden"] {
        let output = fol(&home, &root, &["self", "link", name, repo_arg]);
        assert_failed(&output, &format!("linking name {name:?}"));
    }
    assert!(
        !root.join("../pwned.toml").exists(),
        "no manifest may be written outside the toolchains directory"
    );
    let manifests = fs::read_dir(home.join("toolchains"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().ends_with(".toml"))
                .count()
        })
        .unwrap_or(0);
    assert_eq!(manifests, 0, "no link manifest should exist");

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_repository_pin_cannot_smuggle_a_path_or_trigger_a_fetch() {
    let root = temp_root("pin_validation");
    let home = root.join("home");
    let project = root.join("project");
    write(&project.join("build.fol"), "//fol ../../evil\n");
    write(
        &project.join("main.fol"),
        "fun[] main(): int = { return 0; };\n",
    );

    let pinned = fol(&home, &project, &["code", "build"]);
    assert_failed(&pinned, "building with a traversal pin");
    let message = stderr_of(&pinned);
    assert!(
        message.contains("not a valid toolchain spec"),
        "stderr should reject the pin: {message}"
    );
    assert!(
        !message.contains("fetching"),
        "an invalid pin must never start a download: {message}"
    );

    let overridden = fol(&home, &project, &["+../../evil", "code", "build"]);
    assert_failed(&overridden, "building with a traversal override");
    assert!(
        !stderr_of(&overridden).contains("fetching"),
        "an invalid override must never start a download"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn list_ignores_junk_entries_and_sorts_versions_numerically() {
    let root = temp_root("list_shape");
    let home = root.join("home");
    let repo = fake_repo(&root);
    for version in ["0.2.0", "0.10.0", "1.0.0"] {
        assert_succeeded(
            &install_from(&home, &root, version, &repo),
            &format!("installing {version}"),
        );
    }
    let toolchains = home.join("toolchains");
    write(&toolchains.join(".download-v9.9.9.tar.gz"), "junk\n");
    write(&toolchains.join("notes.txt"), "junk\n");
    write(&toolchains.join(".toml"), "repo = \"/nowhere\"\n");
    fs::create_dir_all(toolchains.join(".staging-leftover")).ok();
    fs::create_dir_all(toolchains.join("v3.0.0")).ok(); // no folc inside

    let output = fol(&home, &root, &["self", "list"]);
    assert_succeeded(&output, "listing toolchains");
    let listed: Vec<String> = stdout_of(&output)
        .lines()
        .filter(|line| line.starts_with("  v") || line.starts_with("  dev"))
        .map(|line| line.trim().to_string())
        .collect();

    assert_eq!(
        listed,
        vec![
            "v0.2.0".to_string(),
            "v0.10.0".to_string(),
            "v1.0.0".to_string()
        ],
        "only real toolchains, ordered numerically"
    );

    fs::remove_dir_all(&root).ok();
}

// -------------------------------------------------------- atomic installs

#[test]
fn a_failed_source_install_leaves_the_previous_toolchain_intact() {
    let root = temp_root("atomic_source");
    let home = root.join("home");
    let repo = fake_repo(&root);

    assert_succeeded(
        &install_from(&home, &root, "0.2.0", &repo),
        "the first install",
    );
    let installed = toolchain_dir(&home, "0.2.0");
    write(&installed.join("MARKER"), "original\n");
    let original_folc = fs::read(installed.join("folc")).expect("folc should be readable");

    // Remove the runtime payload: validation must reject the tree before the
    // existing toolchain is touched.
    fs::remove_dir_all(repo.join("lang/execution/fol-runtime"))
        .expect("runtime should be removable");
    let output = install_from(&home, &root, "0.2.0", &repo);

    assert_failed(&output, "installing a tree without runtime sources");
    assert!(
        stderr_of(&output).contains("fol-runtime"),
        "the error should name the missing payload: {}",
        stderr_of(&output)
    );
    assert!(
        installed.join("MARKER").is_file(),
        "the previous toolchain must survive untouched"
    );
    assert_eq!(
        fs::read(installed.join("folc")).expect("folc should still be readable"),
        original_folc,
        "the previous folc must be byte-identical"
    );
    assert_eq!(
        bookkeeping_entries(&home),
        Vec::<String>::new(),
        "no staging or trash entries may be left behind"
    );

    let listed = fol(&home, &root, &["self", "list"]);
    assert!(
        stdout_of(&listed).contains("v0.2.0"),
        "the surviving toolchain should still be listed"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_source_install_without_std_fails_before_creating_anything() {
    let root = temp_root("atomic_std");
    let home = root.join("home");
    let repo = fake_repo(&root);
    fs::remove_dir_all(repo.join("lang/library/std")).expect("std should be removable");

    let output = install_from(&home, &root, "0.2.0", &repo);

    assert_failed(&output, "installing a tree without std");
    assert!(
        !toolchain_dir(&home, "0.2.0").exists(),
        "no destination directory may be created"
    );
    assert_eq!(bookkeeping_entries(&home), Vec::<String>::new());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn staging_refuses_symlinks_and_leaves_no_partial_tree() {
    let root = temp_root("symlink_refusal");
    let home = root.join("home");
    let repo = fake_repo(&root);
    std::os::unix::fs::symlink("..", repo.join("lang/library/std/loop"))
        .expect("symlink should be creatable");

    let output = install_from(&home, &root, "0.2.0", &repo);

    assert_failed(&output, "installing a tree containing a symlink");
    assert!(
        stderr_of(&output).contains("symlink"),
        "the error should name the symlink: {}",
        stderr_of(&output)
    );
    assert!(!toolchain_dir(&home, "0.2.0").exists());
    assert_eq!(bookkeeping_entries(&home), Vec::<String>::new());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_second_install_replaces_the_toolchain_wholesale() {
    let root = temp_root("replace");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "first install");
    let installed = toolchain_dir(&home, "0.2.0");
    write(&installed.join("STALE"), "left over\n");

    write(&repo.join("lang/library/std/marker.fol"), "// second\n");
    assert_succeeded(
        &install_from(&home, &root, "0.2.0", &repo),
        "second install",
    );

    assert!(
        !installed.join("STALE").exists(),
        "residue from the previous toolchain must be gone"
    );
    let marker = fs::read_to_string(installed.join("std/marker.fol"))
        .expect("staged std marker should exist");
    assert!(
        marker.contains("second"),
        "the new payload should be in place"
    );
    assert_eq!(bookkeeping_entries(&home), Vec::<String>::new());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn install_rejects_duplicate_and_unknown_arguments() {
    let root = temp_root("install_args");
    let home = root.join("home");
    let repo = fake_repo(&root);
    let repo_arg = repo.to_str().expect("repo path should be utf-8");

    for args in [
        vec!["self", "install", "0.2.0", "0.3.0", "--from", repo_arg],
        vec![
            "self", "install", "0.2.0", "--from", repo_arg, "--from", repo_arg,
        ],
        vec!["self", "install", "--bogus", "0.2.0"],
        vec!["self", "install"],
    ] {
        let output = fol(&home, &root, &args);
        assert_failed(&output, &format!("running {args:?}"));
    }

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------- network installs

/// PATH shims: `curl` serves prepared fixtures, `tar` records its arguments
/// and delegates to the real binary.
struct Shims {
    dir: PathBuf,
    fixtures: PathBuf,
    curl_log: PathBuf,
    tar_log: PathBuf,
}

fn real_tool(name: &str) -> PathBuf {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .output()
        .expect("command -v should run");
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(!path.is_empty(), "{name} must exist for these tests");
    PathBuf::from(path)
}

fn shims(root: &Path) -> Shims {
    let dir = root.join("shims");
    let fixtures = root.join("fixtures");
    fs::create_dir_all(&dir).expect("shim dir should be creatable");
    fs::create_dir_all(&fixtures).expect("fixture dir should be creatable");
    let curl_log = root.join("curl.log");
    let tar_log = root.join("tar.log");

    write_executable(
        &dir.join("curl"),
        &format!(
            "#!/bin/sh\n\
             printf '%s\\n' \"$*\" >> {log}\n\
             url=\"\"; out=\"\"\n\
             while [ $# -gt 0 ]; do\n\
             case \"$1\" in\n\
             -o) out=\"$2\"; shift 2 ;;\n\
             http*) url=\"$1\"; shift ;;\n\
             *) shift ;;\n\
             esac\n\
             done\n\
             case \"$url\" in\n\
             *SHA256SUMS*) src={fixtures}/sums ;;\n\
             *.tar.gz) src={fixtures}/archive.tar.gz ;;\n\
             *) exit 22 ;;\n\
             esac\n\
             [ -f \"$src\" ] || exit 22\n\
             cp \"$src\" \"$out\"\n",
            log = curl_log.display(),
            fixtures = fixtures.display()
        ),
    );
    write_executable(
        &dir.join("tar"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {log}\nexec {real} \"$@\"\n",
            log = tar_log.display(),
            real = real_tool("tar").display()
        ),
    );

    Shims {
        dir,
        fixtures,
        curl_log,
        tar_log,
    }
}

impl Shims {
    /// Build a release-shaped tarball whose root holds `{folc, std/, runtime/}`.
    fn publish(&self, payload: &Path, with_sums: bool, corrupt_digest: bool) {
        let archive = self.fixtures.join("archive.tar.gz");
        let status = Command::new(real_tool("tar"))
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(payload)
            .arg(".")
            .status()
            .expect("tar should package the fixture");
        assert!(status.success(), "fixture packaging should succeed");

        if !with_sums {
            let _ = fs::remove_file(self.fixtures.join("sums"));
            return;
        }
        let digest = if corrupt_digest {
            "0".repeat(64)
        } else {
            let output = Command::new(real_tool("sha256sum"))
                .arg(&archive)
                .output()
                .expect("sha256sum should run");
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
                .expect("sha256sum should print a digest")
                .to_string()
        };
        let target = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
        write(
            &self.fixtures.join("sums"),
            &format!("{digest}  fol-compiler-and-lib-v9.9.9-{target}.tar.gz\n"),
        );
    }

    fn fetch_install(&self, home: &Path, cwd: &Path, version: &str) -> Output {
        manager()
            .args(["self", "install", version])
            .env("FOL_HOME", home)
            .env("PATH", self.path_value())
            .env_remove("FOL_TOOLCHAIN")
            .current_dir(cwd)
            .output()
            .expect("the fol manager should run")
    }

    fn path_value(&self) -> String {
        let existing = std::env::var("PATH").unwrap_or_default();
        format!("{}:{existing}", self.dir.display())
    }
}

/// A directory shaped like an unpacked toolchain.
fn payload_tree(root: &Path, complete: bool) -> PathBuf {
    let payload = root.join("payload");
    write_executable(&payload.join("folc"), "#!/bin/sh\necho fetched\n");
    write(&payload.join("runtime/Cargo.toml"), "[package]\n");
    write(&payload.join("runtime/src/lib.rs"), "// runtime\n");
    if complete {
        write(&payload.join("std/lib.fol"), "// std\n");
    }
    payload
}

#[test]
fn a_verified_download_installs_and_leaves_no_archive_behind() {
    let root = temp_root("net_ok");
    let home = root.join("home");
    let shims = shims(&root);
    shims.publish(&payload_tree(&root, true), true, false);

    let output = shims.fetch_install(&home, &root, "9.9.9");

    assert_succeeded(&output, "a verified network install");
    let installed = toolchain_dir(&home, "9.9.9");
    assert!(installed.join("folc").is_file(), "folc should be installed");
    assert!(installed.join("std").is_dir(), "std should be installed");
    assert!(
        installed.join("runtime/src/lib.rs").is_file(),
        "runtime sources should be installed"
    );
    assert_eq!(
        bookkeeping_entries(&home),
        Vec::<String>::new(),
        "no download or staging residue may remain"
    );

    let curl_argv = fs::read_to_string(&shims.curl_log).expect("curl log should exist");
    assert!(
        curl_argv.contains("--max-time") && curl_argv.contains("--connect-timeout"),
        "curl should be called with timeouts: {curl_argv}"
    );
    let tar_argv = fs::read_to_string(&shims.tar_log).expect("tar log should exist");
    assert!(
        tar_argv.contains("--no-same-owner") && tar_argv.contains("--no-same-permissions"),
        "tar should be called with hardening flags: {tar_argv}"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_checksum_mismatch_refuses_to_extract() {
    let root = temp_root("net_mismatch");
    let home = root.join("home");
    let shims = shims(&root);
    shims.publish(&payload_tree(&root, true), true, true);

    let output = shims.fetch_install(&home, &root, "9.9.9");

    assert_failed(&output, "installing a tampered archive");
    let message = stderr_of(&output);
    assert!(
        message.contains("checksum mismatch") && message.contains("refusing to extract"),
        "the error should report the mismatch: {message}"
    );
    assert!(
        !toolchain_dir(&home, "9.9.9").exists(),
        "nothing may be installed"
    );
    assert_eq!(bookkeeping_entries(&home), Vec::<String>::new());
    assert!(
        !fs::read_to_string(&shims.tar_log)
            .unwrap_or_default()
            .contains("-xzf"),
        "extraction must not be attempted"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_release_without_published_checksums_is_refused() {
    let root = temp_root("net_nosums");
    let home = root.join("home");
    let shims = shims(&root);
    shims.publish(&payload_tree(&root, true), false, false);

    let output = shims.fetch_install(&home, &root, "9.9.9");

    assert_failed(&output, "installing an unverifiable release");
    assert!(
        stderr_of(&output).contains("SHA256SUMS"),
        "the error should name the missing checksums: {}",
        stderr_of(&output)
    );
    assert!(!toolchain_dir(&home, "9.9.9").exists());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_incomplete_archive_preserves_the_installed_toolchain() {
    let root = temp_root("net_incomplete");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(
        &install_from(&home, &root, "9.9.9", &repo),
        "the initial source install",
    );
    let installed = toolchain_dir(&home, "9.9.9");
    write(&installed.join("MARKER"), "original\n");

    let shims = shims(&root);
    shims.publish(&payload_tree(&root, false), true, false); // no std/ inside
    let output = shims.fetch_install(&home, &root, "9.9.9");

    assert_failed(&output, "installing an archive without std");
    assert!(
        installed.join("MARKER").is_file(),
        "the installed toolchain must survive an incomplete download"
    );
    assert_eq!(bookkeeping_entries(&home), Vec::<String>::new());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn unsafe_archive_members_are_rejected() {
    let root = temp_root("net_unsafe");
    let home = root.join("home");
    let shims = shims(&root);

    // Package a tree that also carries an escaping member.
    let payload = payload_tree(&root, true);
    write(&root.join("outside.txt"), "escaped\n");
    let archive = shims.fixtures.join("archive.tar.gz");
    let status = Command::new(real_tool("tar"))
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&payload)
        .arg(".")
        .arg("-C")
        .arg(&root)
        .arg("../outside.txt")
        .status()
        .expect("tar should run");
    assert!(
        status.success() || !status.success(),
        "tar attempt recorded"
    );
    if !status.success() {
        // Some tar builds refuse to store the member at all; synthesize the
        // listing case with a transform instead.
        let status = Command::new(real_tool("tar"))
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(&payload)
            .arg("--transform")
            .arg("s,^\\./folc,../escape,")
            .arg(".")
            .status()
            .expect("tar should run");
        assert!(status.success(), "fixture packaging should succeed");
    }
    let digest = String::from_utf8_lossy(
        &Command::new(real_tool("sha256sum"))
            .arg(&archive)
            .output()
            .expect("sha256sum should run")
            .stdout,
    )
    .split_whitespace()
    .next()
    .expect("digest")
    .to_string();
    let target = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
    write(
        &shims.fixtures.join("sums"),
        &format!("{digest}  fol-compiler-and-lib-v9.9.9-{target}.tar.gz\n"),
    );

    let output = shims.fetch_install(&home, &root, "9.9.9");

    assert_failed(&output, "installing an archive with an escaping member");
    assert!(
        stderr_of(&output).contains("archive member"),
        "the error should name the offending member: {}",
        stderr_of(&output)
    );
    assert!(!root.join("escape").exists(), "nothing may escape");

    fs::remove_dir_all(&root).ok();
}

#[test]
fn concurrent_installs_do_not_collide() {
    let root = temp_root("net_concurrent");
    let home = root.join("home");
    let shims = shims(&root);
    let payload = payload_tree(&root, true);

    // One shared fixture, two different versions: the download paths must not
    // be a fixed shared name.
    let target = format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS);
    let archive = shims.fixtures.join("archive.tar.gz");
    let status = Command::new(real_tool("tar"))
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&payload)
        .arg(".")
        .status()
        .expect("tar should run");
    assert!(status.success());
    let digest = String::from_utf8_lossy(
        &Command::new(real_tool("sha256sum"))
            .arg(&archive)
            .output()
            .expect("sha256sum should run")
            .stdout,
    )
    .split_whitespace()
    .next()
    .expect("digest")
    .to_string();
    write(
        &shims.fixtures.join("sums"),
        &format!(
            "{digest}  fol-compiler-and-lib-v9.9.8-{target}.tar.gz\n{digest}  fol-compiler-and-lib-v9.9.9-{target}.tar.gz\n"
        ),
    );

    let mut children = Vec::new();
    for version in ["9.9.8", "9.9.9"] {
        children.push(
            manager()
                .args(["self", "install", version])
                .env("FOL_HOME", &home)
                .env("PATH", shims.path_value())
                .current_dir(&root)
                .spawn()
                .expect("install should start"),
        );
    }
    for mut child in children {
        let status = child.wait().expect("install should finish");
        assert!(status.success(), "concurrent installs should both succeed");
    }
    assert!(toolchain_dir(&home, "9.9.8").join("folc").is_file());
    assert!(toolchain_dir(&home, "9.9.9").join("folc").is_file());
    assert_eq!(bookkeeping_entries(&home), Vec::<String>::new());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_missing_digest_tool_is_reported_without_extracting() {
    let root = temp_root("net_nodigest");
    let home = root.join("home");
    let shims = shims(&root);
    shims.publish(&payload_tree(&root, true), true, false);

    // Shadow every digest tool with a failing stub, so none is usable.
    for tool in ["sha256sum", "shasum", "openssl"] {
        write_executable(&shims.dir.join(tool), "#!/bin/sh\nexit 127\n");
    }
    let output = shims.fetch_install(&home, &root, "9.9.9");

    assert_failed(&output, "installing without a digest tool");
    assert!(
        stderr_of(&output).contains("sha256sum"),
        "the error should name the missing tool: {}",
        stderr_of(&output)
    );
    assert!(!toolchain_dir(&home, "9.9.9").exists());
    assert!(
        !fs::read_to_string(&shims.tar_log)
            .unwrap_or_default()
            .contains("-xzf"),
        "extraction must not be attempted"
    );

    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------- dispatch

#[test]
fn dispatch_forwards_arguments_and_the_exit_code() {
    let root = temp_root("dispatch_args");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "install");
    assert_succeeded(
        &fol(&home, &root, &["self", "default", "0.2.0"]),
        "setting the default",
    );

    let output = fol(&home, &root, &["--exit-seven", "code", "build"]);

    assert_eq!(
        output.status.code(),
        Some(7),
        "the child's exit code should be preserved"
    );
    assert!(
        stdout_of(&output).contains("args: --exit-seven code build"),
        "arguments should be forwarded verbatim: {}",
        stdout_of(&output)
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_uses_a_depth_counter_instead_of_poisoning_descendants() {
    let root = temp_root("dispatch_depth");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "install");
    assert_succeeded(
        &fol(&home, &root, &["self", "default", "0.2.0"]),
        "setting the default",
    );

    let output = fol(&home, &root, &["--print-env"]);
    let printed = stdout_of(&output);

    assert!(
        printed.contains("FOL_DISPATCH_DEPTH=1"),
        "the child should see depth 1: {printed}"
    );
    assert!(
        printed.lines().any(|line| line == "FOL_DISPATCHED="),
        "the old poison variable should be gone: {printed}"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_manager_binary_installed_as_folc_is_refused() {
    let root = temp_root("dispatch_self");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "install");
    assert_succeeded(
        &fol(&home, &root, &["self", "default", "0.2.0"]),
        "setting the default",
    );

    // Point the engine at the manager: metadata follows the link, so the
    // same-inode guard fires exactly as it would for a copied binary.
    let folc = toolchain_dir(&home, "0.2.0").join("folc");
    fs::remove_file(&folc).expect("stub folc should be removable");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_fol"), &folc)
        .expect("symlink should be creatable");

    let output = fol(&home, &root, &["code", "build"]);

    assert_failed(&output, "dispatching to the manager itself");
    assert!(
        stderr_of(&output).contains("manager itself"),
        "the error should explain the misconfiguration: {}",
        stderr_of(&output)
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_linked_std_override_is_exported_to_the_engine() {
    let root = temp_root("linked_std");
    let home = root.join("home");
    let repo = fake_repo(&root);
    let elsewhere = root.join("elsewhere/std");
    write(&elsewhere.join("lib.fol"), "// external std\n");

    assert_succeeded(
        &fol(
            &home,
            &root,
            &[
                "self",
                "link",
                "dev",
                repo.to_str().expect("repo path should be utf-8"),
                "--std",
                elsewhere.to_str().expect("std path should be utf-8"),
            ],
        ),
        "linking with an explicit std",
    );

    let output = fol(&home, &root, &["+dev", "--print-env"]);
    assert!(
        stdout_of(&output).contains(&format!("FOL_STD_ROOT={}", elsewhere.display())),
        "the linked std should be forwarded: {}",
        stdout_of(&output)
    );

    // A link without --std must not set the variable.
    assert_succeeded(
        &fol(
            &home,
            &root,
            &[
                "self",
                "link",
                "plain",
                repo.to_str().expect("repo path should be utf-8"),
            ],
        ),
        "linking without an explicit std",
    );
    let plain = fol(&home, &root, &["+plain", "--print-env"]);
    assert!(
        stdout_of(&plain)
            .lines()
            .any(|line| line == "FOL_STD_ROOT="),
        "no std override should be forwarded: {}",
        stdout_of(&plain)
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn self_subcommands_accept_an_override_only_where_it_makes_sense() {
    let root = temp_root("self_override");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "install");

    let which = fol(&home, &root, &["+0.2.0", "self", "which"]);
    assert_succeeded(&which, "which with an override");
    assert!(
        stdout_of(&which).contains("v0.2.0"),
        "the override should select the toolchain: {}",
        stdout_of(&which)
    );

    let removed = fol(&home, &root, &["+0.2.0", "self", "remove", "0.2.0"]);
    assert_failed(&removed, "remove with an override");
    assert!(
        stderr_of(&removed).contains("not accepted"),
        "the error should reject the override: {}",
        stderr_of(&removed)
    );

    fs::remove_dir_all(&root).ok();
}

// ------------------------------------------------------------------ config

#[test]
fn the_default_survives_malformed_and_lookalike_config_lines() {
    let root = temp_root("config_junk");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "install");
    write(
        &home.join("config"),
        "# a comment\ndefaults = wrong\ngarbage\ndefault = v0.2.0\nother = 1\n",
    );

    let output = fol(&home, &root, &["self", "which"]);

    assert_succeeded(&output, "resolving through a messy config");
    assert!(
        stdout_of(&output).contains("v0.2.0"),
        "the real default key should be honored: {}",
        stdout_of(&output)
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn setting_the_default_preserves_other_config_keys() {
    let root = temp_root("config_keys");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "install");
    write(
        &home.join("config"),
        "# keep me\nkeep = yes\ndefault = old\ntrailing = 1\n",
    );

    assert_succeeded(
        &fol(&home, &root, &["self", "default", "0.2.0"]),
        "setting the default",
    );

    let config = fs::read_to_string(home.join("config")).expect("config should exist");
    assert!(
        config.contains("# keep me"),
        "comments should survive: {config}"
    );
    assert!(config.contains("keep = yes"), "other keys should survive");
    assert!(config.contains("trailing = 1"), "other keys should survive");
    assert!(
        config.contains("default = 0.2.0"),
        "the default should change"
    );
    assert!(
        !config.contains("default = old"),
        "the old default should be gone"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn removing_the_default_toolchain_clears_the_default() {
    let root = temp_root("config_clear");
    let home = root.join("home");
    let repo = fake_repo(&root);
    assert_succeeded(&install_from(&home, &root, "0.2.0", &repo), "install");
    assert_succeeded(
        &fol(&home, &root, &["self", "default", "0.2.0"]),
        "setting the default",
    );

    let removed = fol(&home, &root, &["self", "remove", "0.2.0"]);
    assert_succeeded(&removed, "removing the default toolchain");
    assert!(
        stderr_of(&removed).contains("default has been cleared"),
        "the user should be warned: {}",
        stderr_of(&removed)
    );

    let config = fs::read_to_string(home.join("config")).unwrap_or_default();
    assert!(
        !config.contains("default ="),
        "the dangling default should be gone: {config}"
    );

    // And nothing may silently re-download the removed toolchain.
    let build = fol(&home, &root, &["code", "build"]);
    assert_failed(&build, "building without a default");
    assert!(
        !stderr_of(&build).contains("fetching"),
        "no download may be attempted: {}",
        stderr_of(&build)
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_default_must_name_an_installed_toolchain() {
    let root = temp_root("config_requires");
    let home = root.join("home");

    let output = fol(&home, &root, &["self", "default", "9.9.9"]);

    assert_failed(&output, "defaulting to a missing toolchain");
    assert!(
        !home.join("config").exists()
            || !fs::read_to_string(home.join("config"))
                .unwrap_or_default()
                .contains("9.9.9"),
        "the config must not record an uninstalled toolchain"
    );

    fs::remove_dir_all(&root).ok();
}

// ----------------------------------------------------------------- surface

#[test]
fn self_version_reports_the_manager_version() {
    let root = temp_root("self_version");
    let home = root.join("home");

    let output = manager()
        .args(["self", "version"])
        .env_remove("FOL_HOME")
        .current_dir(&root)
        .output()
        .expect("the fol manager should run");

    assert_succeeded(&output, "printing the manager version");
    assert!(
        stdout_of(&output).contains(env!("CARGO_PKG_VERSION")),
        "the version should be printed: {}",
        stdout_of(&output)
    );
    assert!(
        !home.exists(),
        "no home should be created just to print a version"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn self_subcommands_reject_stray_arguments() {
    let root = temp_root("self_stray");
    let home = root.join("home");

    for args in [
        vec!["self", "list", "extra"],
        vec!["self", "which", "extra"],
        vec!["self", "remove", "a", "b"],
        vec!["self", "default", "a", "b"],
        vec!["self", "nonsense"],
    ] {
        let output = fol(&home, &root, &args);
        assert_failed(&output, &format!("running {args:?}"));
    }

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_project_without_fol_home_uses_its_own_dot_fol_directory() {
    let root = temp_root("project_home");
    let project = root.join("project");
    write(&project.join("build.fol"), "//fol 0.2.0\n");
    let repo = fake_repo(&root);

    let output = manager()
        .args([
            "self",
            "install",
            "0.2.0",
            "--from",
            repo.to_str().expect("repo path should be utf-8"),
        ])
        .env_remove("FOL_HOME")
        .current_dir(&project)
        .output()
        .expect("the fol manager should run");

    assert_succeeded(&output, "installing into a project-local home");
    assert!(
        project
            .join(".fol/toolchain/toolchains/v0.2.0/folc")
            .is_file(),
        "the toolchain should land under <project>/.fol/toolchain"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn without_a_home_or_a_project_the_manager_explains_itself() {
    let root = temp_root("no_home");

    let output = manager()
        .args(["self", "list"])
        .env_remove("FOL_HOME")
        .current_dir(&root)
        .output()
        .expect("the fol manager should run");

    assert_failed(&output, "listing without a home");
    assert!(
        stderr_of(&output).contains("FOL_HOME is not set"),
        "the error should name FOL_HOME: {}",
        stderr_of(&output)
    );

    fs::remove_dir_all(&root).ok();
}

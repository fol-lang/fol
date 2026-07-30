//! `fol` — the FOL toolchain manager.
//!
//! This is the only binary a distribution packages. It owns `fol self …`
//! (installing, linking, and selecting toolchains inside FOL_HOME) and
//! forwards every other command to the selected toolchain's `folc` binary.
//! It contains no language logic.
//!
//! Every user-supplied toolchain spec passes through [`validated_spec`] before
//! it can reach a path join, and every destructive filesystem operation is
//! guarded by [`assert_removable_child`]: a spec is attacker-controlled the
//! moment a cloned repository's `build.fol` carries a `//fol` pin.

#[cfg(not(unix))]
compile_error!("fol is linux-only");

use std::env;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const DISPATCH_DEPTH_ENV: &str = "FOL_DISPATCH_DEPTH";
const MAX_DISPATCH_DEPTH: u32 = 4;
const TOOLCHAIN_ENV: &str = "FOL_TOOLCHAIN";
const STD_ROOT_ENV: &str = "FOL_STD_ROOT";
const RELEASE_URL_BASE: &str = "https://github.com/fol-lang/fol/releases/download";
const MAX_SPEC_LEN: usize = 64;
const MAX_COPY_DEPTH: u32 = 64;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code);
}

fn run(args: &[String]) -> i32 {
    let (override_spec, rest) = split_toolchain_override(args);
    match rest.first().map(String::as_str) {
        Some("self") | Some("s") => run_self(override_spec.as_deref(), &rest[1..]),
        _ => dispatch(override_spec.as_deref(), rest),
    }
}

/// Peel a leading `+<toolchain>` argument. Handled before the `self` split so
/// `fol +dev self which` reaches the manager instead of dead-ending.
fn split_toolchain_override(args: &[String]) -> (Option<String>, &[String]) {
    match args.first().and_then(|arg| arg.strip_prefix('+')) {
        Some(spec) => (Some(spec.to_string()), &args[1..]),
        None => (None, args),
    }
}

// Styling that matches the frontend's help look (colors off when stdout is
// not a terminal, same as fol-frontend's ansi module).
fn styled(code: &str, text: &str) -> String {
    use std::io::IsTerminal;
    if std::io::stdout().is_terminal() {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn section(text: &str) -> String {
    styled("33;1", text)
}

fn bold_pad(text: &str, width: usize) -> String {
    let pad = " ".repeat(width.saturating_sub(text.len()));
    format!("{}{pad}", styled("1", text))
}

fn dim(text: &str) -> String {
    styled("2", text)
}

/// Shown only when no toolchain can be resolved — otherwise the resolved
/// folc renders the real root help.
fn print_fallback_help() {
    println!("User-facing frontend for the FOL toolchain");
    println!();
    println!("{} fol [+<toolchain>] [COMMAND]", section("Usage:"));
    println!();
    println!("{}", section("Commands:"));
    println!(
        "  {}  {}  Workspace management",
        bold_pad("work", 4),
        dim("[aliases: w]")
    );
    println!(
        "  {}  {}  Package management",
        bold_pad("pack", 4),
        dim("[aliases: p]")
    );
    println!(
        "  {}  {}  Build, run, test, check",
        bold_pad("code", 4),
        dim("[aliases: c]")
    );
    println!(
        "  {}  {}  Editor tools, LSP, completion",
        bold_pad("tool", 4),
        dim("[aliases: t]")
    );
    println!(
        "  {}  {}  Toolchain management (install, link, default)",
        bold_pad("self", 4),
        dim("[aliases: s]")
    );
    println!();
    println!("{}", section("Options:"));
    println!(
        "  {}, {}     Print help",
        bold_pad("-h", 2),
        bold_pad("--help", 6)
    );
    println!(
        "  {}, {}  Print version",
        bold_pad("-V", 2),
        bold_pad("--version", 9)
    );
    println!();
    println!(
        "{}",
        dim("Run `fol <group> <command> --help` for command-specific usage.")
    );
    println!();
    println!(
        "{}",
        dim("no toolchain is installed yet — run `fol self install <version>` or `fol self link dev <repo>`.")
    );
}

fn print_self_help() {
    println!("{} fol self <COMMAND>", section("Usage:"));
    println!();
    println!("{}", section("Commands:"));
    println!(
        "  {}  Install a toolchain ({} copies a built source tree)",
        bold_pad("install", 7),
        dim("--from <repo>")
    );
    println!(
        "  {}  Register a source checkout as a named toolchain",
        bold_pad("link", 7)
    );
    println!(
        "  {}  Set, show, or {} the default toolchain",
        bold_pad("default", 7),
        dim("--unset")
    );
    println!("  {}  Show installed toolchains", bold_pad("list", 7));
    println!(
        "  {}  Delete an installed toolchain or link",
        bold_pad("remove", 7)
    );
    println!(
        "  {}  Print the folc binary this directory resolves to",
        bold_pad("which", 7)
    );
    println!("  {}  Print the manager version", bold_pad("version", 7));
    println!();
    println!("{}", section("Options:"));
    println!(
        "  {}, {}  Print help",
        bold_pad("-h", 2),
        bold_pad("--help", 6)
    );
    println!();
    println!(
        "{}",
        dim("selection order: +<toolchain> arg, FOL_TOOLCHAIN env, `//fol <version>` pin in build.fol, configured default.")
    );
    println!(
        "{}",
        dim("downloaded toolchains are verified against the release SHA256SUMS before they are unpacked.")
    );
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    1
}

// ---------------------------------------------------------------- FOL_HOME

fn fol_home() -> Result<PathBuf, String> {
    let home = if let Some(value) = env::var_os("FOL_HOME").filter(|value| !value.is_empty()) {
        PathBuf::from(value)
    } else if let Some(project) = env::current_dir()
        .ok()
        .and_then(|cwd| find_build_manifest(&cwd))
        .and_then(|manifest| manifest.parent().map(Path::to_path_buf))
    {
        // Without FOL_HOME, a project keeps its toolchains next to its build
        // artifacts, under the .fol/ directory beside build.fol.
        project.join(".fol").join("toolchain")
    } else {
        return Err(
            "FOL_HOME is not set and no build.fol was found from here upward\n\n  \
             fol keeps toolchains in FOL_HOME, or — inside a project — in\n  \
             <project>/.fol/toolchain. enter a project, or set FOL_HOME in your\n  \
             shell profile, for example:\n\n    \
             export FOL_HOME=\"$HOME/.fol\""
                .to_string(),
        );
    };
    Ok(home)
}

fn toolchains_root(home: &Path) -> PathBuf {
    home.join("toolchains")
}

fn ensure_home_layout(home: &Path) -> Result<(), String> {
    for dir in [toolchains_root(home), home.join("pkg")] {
        fs::create_dir_all(&dir)
            .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
    }
    Ok(())
}

/// Canonical toolchains root, so containment checks compare real paths and
/// error messages print absolute ones.
fn resolved_toolchains_root(home: &Path) -> Result<PathBuf, String> {
    let root = toolchains_root(home);
    root.canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", root.display()))
}

// --------------------------------------------------------- toolchain specs

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SpecKind {
    Version,
    Name,
}

/// A toolchain spec that has passed validation. Path constructors accept only
/// this type, so an unvalidated string cannot reach the filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Spec {
    raw: String,
    kind: SpecKind,
}

impl Spec {
    fn bare(&self) -> &str {
        match self.kind {
            SpecKind::Version => self.raw.strip_prefix('v').unwrap_or(&self.raw),
            SpecKind::Name => &self.raw,
        }
    }

    fn dir_name(&self) -> String {
        format!("v{}", self.bare())
    }

    fn is_version(&self) -> bool {
        self.kind == SpecKind::Version
    }

    /// Compare against a stored string (config value, directory name) ignoring
    /// a leading `v` on either side.
    fn matches(&self, other: &str) -> bool {
        let other_bare = other.strip_prefix('v').unwrap_or(other);
        self.bare() == other_bare
    }
}

impl std::fmt::Display for Spec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.raw)
    }
}

fn is_version_shape(bare: &str) -> bool {
    let (core, pre) = match bare.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (bare, None),
    };
    if core.is_empty() {
        return false;
    }
    for segment in core.split('.') {
        if segment.is_empty() || !segment.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    match pre {
        Some(pre) => {
            !pre.is_empty()
                && pre
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'.')
        }
        None => true,
    }
}

/// The single gate every user-supplied spec passes through. The character set
/// alone rejects `/`, `\`, NUL, whitespace and therefore all path traversal;
/// the leading-dot rule reserves the manager's own `.staging-*`/`.trash-*`
/// bookkeeping namespace.
fn validated_spec(raw: &str) -> Result<Spec, String> {
    let reason = |why: &str| format!("'{raw}' is not a valid toolchain spec: {why}");
    if raw.is_empty() {
        return Err(reason("it is empty"));
    }
    if raw.len() > MAX_SPEC_LEN {
        return Err(reason("it is longer than 64 characters"));
    }
    if !raw
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
    {
        return Err(reason(
            "only letters, digits, '.', '_', '+' and '-' are allowed",
        ));
    }
    if raw.starts_with('.') {
        return Err(reason("it may not start with '.'"));
    }
    let bare = raw.strip_prefix('v').unwrap_or(raw);
    if is_version_shape(bare) {
        return Ok(Spec {
            raw: raw.to_string(),
            kind: SpecKind::Version,
        });
    }
    if raw.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return Ok(Spec {
            raw: raw.to_string(),
            kind: SpecKind::Name,
        });
    }
    Err(reason(
        "versions look like 1.2.3 and names start with a letter",
    ))
}

fn validated_version(raw: &str) -> Result<Spec, String> {
    let spec = validated_spec(raw)?;
    if !spec.is_version() {
        return Err(format!("'{raw}' is not a version like 1.2.3"));
    }
    Ok(spec)
}

fn validated_name(raw: &str) -> Result<Spec, String> {
    let spec = validated_spec(raw)?;
    if spec.is_version() {
        return Err(format!(
            "'{raw}' looks like a version; links need a name like 'dev'"
        ));
    }
    Ok(spec)
}

/// Numeric-segment sort so v0.2.0 orders before v0.10.0.
fn version_sort_key(bare: &str) -> (Vec<u64>, String) {
    let core = bare.split('-').next().unwrap_or(bare);
    let numbers = core
        .split('.')
        .map(|segment| segment.parse::<u64>().unwrap_or(0))
        .collect();
    (numbers, bare.to_string())
}

// ------------------------------------------------------------------- paths

fn contained_in(root: &Path, candidate: &Path) -> Result<(), String> {
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(format!(
            "refusing to touch {}: it escapes {}",
            candidate.display(),
            root.display()
        ));
    }
    if !candidate.starts_with(root) {
        return Err(format!(
            "refusing to touch {}: it is outside {}",
            candidate.display(),
            root.display()
        ));
    }
    Ok(())
}

/// Guards every remove/rename: the target must be a direct child of `root`
/// and must not itself be a symlink (which `remove_dir_all` would follow).
fn assert_removable_child(root: &Path, target: &Path) -> Result<(), String> {
    contained_in(root, target)?;
    if target.parent() != Some(root) {
        return Err(format!(
            "refusing to touch {}: it is not a direct entry of {}",
            target.display(),
            root.display()
        ));
    }
    let metadata = fs::symlink_metadata(target)
        .map_err(|error| format!("cannot inspect {}: {error}", target.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing to touch {}: it is a symlink",
            target.display()
        ));
    }
    Ok(())
}

fn toolchain_dir(home: &Path, spec: &Spec) -> Result<PathBuf, String> {
    if !spec.is_version() {
        return Err(format!(
            "'{spec}' is a linked toolchain name, not an installable version"
        ));
    }
    let root = resolved_toolchains_root(home)?;
    let dir = root.join(spec.dir_name());
    contained_in(&root, &dir)?;
    Ok(dir)
}

fn link_manifest_path(home: &Path, spec: &Spec) -> Result<PathBuf, String> {
    let root = resolved_toolchains_root(home)?;
    let path = root.join(format!("{}.toml", spec.raw));
    contained_in(&root, &path)?;
    Ok(path)
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{}-{}", std::process::id(), nanos)
}

// ------------------------------------------------------------------ config

/// The config file verbatim. Unknown keys and comments survive a rewrite.
struct Config {
    lines: Vec<String>,
}

fn config_path(home: &Path) -> PathBuf {
    home.join("config")
}

fn read_config(home: &Path) -> Config {
    let lines = fs::read_to_string(config_path(home))
        .map(|content| content.lines().map(str::to_string).collect())
        .unwrap_or_default();
    Config { lines }
}

/// Exact key match, and a malformed line skips instead of aborting the scan.
fn config_get(config: &Config, key: &str) -> Option<String> {
    for line in &config.lines {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((found, value)) = line.split_once('=') else {
            continue;
        };
        if found.trim() != key {
            continue;
        }
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

fn config_set_default(home: &Path, spec: Option<&Spec>) -> Result<(), String> {
    let mut config = read_config(home);
    let mut replaced = false;
    let mut kept = Vec::with_capacity(config.lines.len() + 1);
    for line in config.lines.drain(..) {
        let is_default = line
            .trim()
            .split_once('=')
            .is_some_and(|(key, _)| key.trim() == "default");
        if !is_default {
            kept.push(line);
            continue;
        }
        if let Some(spec) = spec {
            if !replaced {
                kept.push(format!("default = {spec}"));
                replaced = true;
            }
        }
    }
    if let Some(spec) = spec {
        if !replaced {
            kept.push(format!("default = {spec}"));
        }
    }

    let path = config_path(home);
    let temporary = home.join(format!(".config-tmp-{}", unique_suffix()));
    let mut body = kept.join("\n");
    if !body.is_empty() {
        body.push('\n');
    }
    fs::write(&temporary, body)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, &path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("cannot update {}: {error}", path.display())
    })
}

fn default_spec(home: &Path) -> Option<Spec> {
    let stored = config_get(&read_config(home), "default")?;
    validated_spec(&stored).ok()
}

// ---------------------------------------------------------------- pin scan

fn find_build_manifest(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join("build.fol");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Scan the leading comment/blank block of build.fol for a `//fol <version>`
/// pin. Stops at the first line of real code.
fn parse_toolchain_pin(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }
        // The first line of real code ends the pin block.
        let comment = line.strip_prefix("//")?.trim_start();
        if let Some(rest) = comment.strip_prefix("fol") {
            if rest.starts_with(char::is_whitespace) {
                if let Some(version) = rest.split_whitespace().next() {
                    return Some(version.to_string());
                }
            }
        }
    }
    None
}

fn pinned_toolchain() -> Option<String> {
    let cwd = env::current_dir().ok()?;
    let manifest = find_build_manifest(&cwd)?;
    let content = fs::read_to_string(manifest).ok()?;
    parse_toolchain_pin(&content)
}

// ----------------------------------------------------------- toolchain model

struct ResolvedToolchain {
    spec: Spec,
    bin: PathBuf,
    std_root: PathBuf,
    /// A linked manifest named `std` explicitly; the manager forwards it so a
    /// checkout-backed toolchain can point at a std tree outside the repo.
    std_explicit: bool,
    linked: bool,
}

fn parse_link_manifest(content: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim().trim_matches('"');
            entries.push((key.trim().to_string(), value.to_string()));
        }
    }
    entries
}

fn resolve_linked(home: &Path, spec: &Spec) -> Result<Option<ResolvedToolchain>, String> {
    let manifest_path = link_manifest_path(home, spec)?;
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    let entries = parse_link_manifest(&content);
    let lookup = |key: &str| {
        entries
            .iter()
            .find(|(found, _)| found == key)
            .map(|(_, value)| PathBuf::from(value))
    };
    let repo = lookup("repo");
    let explicit_bin = lookup("bin");
    let explicit_std = lookup("std");
    if repo.is_none() && (explicit_bin.is_none() || explicit_std.is_none()) {
        return Err(format!(
            "link '{spec}' needs `repo`, or both `bin` and `std`, in {}",
            manifest_path.display()
        ));
    }
    let bin = match explicit_bin {
        Some(bin) => bin,
        None => {
            let repo = repo.clone().expect("repo presence checked above");
            newest_built_folc(&repo).ok_or_else(|| {
                format!(
                    "link '{spec}' points at {} but no folc binary exists there\n  build one: cargo build --bin folc",
                    repo.display()
                )
            })?
        }
    };
    let std_explicit = explicit_std.is_some();
    let std_root = match explicit_std {
        Some(std_root) => std_root,
        None => repo
            .expect("repo presence checked above")
            .join("lang/library/std"),
    };
    if !bin.is_file() {
        return Err(format!(
            "link '{spec}': binary {} does not exist",
            bin.display()
        ));
    }
    if !std_root.is_dir() {
        return Err(format!(
            "link '{spec}': std root {} does not exist",
            std_root.display()
        ));
    }
    Ok(Some(ResolvedToolchain {
        spec: spec.clone(),
        bin,
        std_root,
        std_explicit,
        linked: true,
    }))
}

fn resolve_installed(home: &Path, spec: &Spec) -> Option<ResolvedToolchain> {
    if !spec.is_version() {
        return None;
    }
    let dir = toolchain_dir(home, spec).ok()?;
    let bin = dir.join("folc");
    if !bin.is_file() {
        return None;
    }
    Some(ResolvedToolchain {
        spec: spec.clone(),
        bin,
        std_root: dir.join("std"),
        std_explicit: false,
        linked: false,
    })
}

fn resolve_toolchain(
    home: &Path,
    spec: &Spec,
    allow_fetch: bool,
) -> Result<ResolvedToolchain, String> {
    if let Some(linked) = resolve_linked(home, spec)? {
        return Ok(linked);
    }
    if let Some(installed) = resolve_installed(home, spec) {
        return Ok(installed);
    }
    if spec.is_version() && allow_fetch {
        eprintln!("toolchain {spec} not installed, fetching...");
        install_from_network(home, spec)?;
        return resolve_installed(home, spec)
            .ok_or_else(|| format!("toolchain {spec} was fetched but folc is missing from it"));
    }
    Err(format!(
        "toolchain '{spec}' is not installed\n  run: fol self install {spec}",
    ))
}

struct Toolchain {
    spec: Spec,
    linked: bool,
}

/// Only real toolchains: a version directory holding `folc`, or a `<name>.toml`
/// link whose stem is a valid name. Dotfiles and the manager's own staging and
/// download bookkeeping are skipped.
fn installed_toolchains(home: &Path) -> Vec<Toolchain> {
    let mut toolchains = Vec::new();
    let Ok(entries) = fs::read_dir(toolchains_root(home)) else {
        return toolchains;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            let Ok(spec) = validated_spec(&name) else {
                continue;
            };
            if spec.is_version() && path.join("folc").is_file() {
                toolchains.push(Toolchain {
                    spec,
                    linked: false,
                });
            }
            continue;
        }
        if let Some(stem) = name.strip_suffix(".toml") {
            let Ok(spec) = validated_name(stem) else {
                continue;
            };
            toolchains.push(Toolchain { spec, linked: true });
        }
    }
    toolchains.sort_by(|left, right| {
        let left_key = (left.spec.kind, version_sort_key(left.spec.bare()));
        let right_key = (right.spec.kind, version_sort_key(right.spec.bare()));
        left_key.cmp(&right_key)
    });
    toolchains
}

fn select_toolchain_spec(home: &Path, override_spec: Option<&str>) -> Result<Spec, String> {
    if let Some(spec) = override_spec {
        return validated_spec(spec);
    }
    if let Ok(spec) = env::var(TOOLCHAIN_ENV) {
        if !spec.is_empty() {
            return validated_spec(&spec);
        }
    }
    // A pin comes from repository content, so it is validated exactly like a
    // command-line argument before it can select or fetch anything.
    if let Some(pin) = pinned_toolchain() {
        return validated_spec(&pin)
            .map_err(|error| format!("{error}\n  fix the `//fol <version>` pin in build.fol"));
    }
    if let Some(default) = default_spec(home) {
        return Ok(default);
    }
    let installed = installed_toolchains(home);
    if installed.len() == 1 {
        return Ok(installed[0].spec.clone());
    }
    Err("no toolchain selected and no default configured\n  \
         run: fol self default <version|name>"
        .to_string())
}

// --------------------------------------------------------------- dispatch

fn dispatch_depth() -> u32 {
    env::var(DISPATCH_DEPTH_ENV)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

/// Catches the common misconfiguration of a manager binary installed as a
/// toolchain's `folc`, which would otherwise re-enter the manager.
fn ensure_not_self(bin: &Path) -> Result<(), String> {
    let Ok(current) = env::current_exe() else {
        return Ok(());
    };
    let (Ok(current_meta), Ok(target_meta)) = (fs::metadata(&current), fs::metadata(bin)) else {
        return Ok(());
    };
    if current_meta.dev() == target_meta.dev() && current_meta.ino() == target_meta.ino() {
        return Err(format!(
            "{} is the fol manager itself, not a folc engine\n  reinstall the toolchain: fol self install <version>",
            bin.display()
        ));
    }
    Ok(())
}

fn dispatch(override_spec: Option<&str>, rest: &[String]) -> i32 {
    let depth = dispatch_depth();
    if depth >= MAX_DISPATCH_DEPTH {
        return fail(&format!(
            "recursive fol dispatch detected ({depth} levels deep)\n  \
             this usually means a manager binary was installed as a toolchain's folc"
        ));
    }

    // Help/version/no-args forward to the resolved folc so the user always
    // sees the real frontend surface; the manager's own help is only a
    // fallback for when nothing is resolvable yet.
    let help_like =
        rest.is_empty() || matches!(rest[0].as_str(), "-h" | "--help" | "-V" | "--version");

    let resolved = fol_home().and_then(|home| {
        ensure_home_layout(&home)?;
        let spec = select_toolchain_spec(&home, override_spec)?;
        resolve_toolchain(&home, &spec, !help_like)
    });
    let toolchain = match resolved {
        Ok(toolchain) => toolchain,
        Err(message) => {
            if help_like {
                print_fallback_help();
                return 0;
            }
            return fail(&message);
        }
    };
    if let Err(message) = ensure_not_self(&toolchain.bin) {
        return fail(&message);
    }

    // Installed toolchains need no std wiring: folc resolves std/ next to its
    // own binary. Only an explicitly redirected linked std must be forwarded.
    let mut command = Command::new(&toolchain.bin);
    command.args(rest);
    command.env(DISPATCH_DEPTH_ENV, (depth + 1).to_string());
    if toolchain.std_explicit && env::var_os(STD_ROOT_ENV).is_none() {
        command.env(STD_ROOT_ENV, &toolchain.std_root);
    }

    use std::os::unix::process::CommandExt;
    let error = command.exec();
    fail(&format!("cannot run {}: {error}", toolchain.bin.display()))
}

// ------------------------------------------------------------- fol self …

fn run_self(override_spec: Option<&str>, args: &[String]) -> i32 {
    let subcommand = match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print_self_help();
            return 0;
        }
        Some("version") | Some("-V") | Some("--version") => {
            println!("fol {} (toolchain manager)", env!("CARGO_PKG_VERSION"));
            return 0;
        }
        Some(subcommand) => subcommand,
    };
    if override_spec.is_some() && matches!(subcommand, "install" | "link" | "default" | "remove") {
        return fail(&format!(
            "+<toolchain> is not accepted by 'fol self {subcommand}'"
        ));
    }
    let home = match fol_home() {
        Ok(home) => home,
        Err(message) => return fail(&message),
    };
    if let Err(message) = ensure_home_layout(&home) {
        return fail(&message);
    }
    let rest = &args[1..];
    let result = match subcommand {
        "install" => self_install(&home, rest),
        "link" => self_link(&home, rest),
        "default" => self_default(&home, rest),
        "list" => self_list(&home, rest),
        "remove" => self_remove(&home, rest),
        "which" => self_which(&home, override_spec, rest),
        other => Err(format!(
            "unknown self subcommand '{other}'\n  run: fol self"
        )),
    };
    match result {
        Ok(()) => 0,
        Err(message) => fail(&message),
    }
}

fn no_extra_args(args: &[String], usage: &str) -> Result<(), String> {
    if args.is_empty() {
        return Ok(());
    }
    Err(format!(
        "unexpected argument '{}'\n  usage: {usage}",
        args[0]
    ))
}

fn self_install(home: &Path, args: &[String]) -> Result<(), String> {
    const USAGE: &str = "fol self install <version> [--from <repo>]";
    let mut version: Option<String> = None;
    let mut from: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--from" {
            if from.is_some() {
                return Err(format!("--from was given twice\n  usage: {USAGE}"));
            }
            let value = iter
                .next()
                .ok_or_else(|| "--from needs a path to a built fol source tree".to_string())?;
            from = Some(PathBuf::from(value));
        } else if arg.starts_with('-') {
            return Err(format!("unknown option '{arg}'\n  usage: {USAGE}"));
        } else if version.is_none() {
            version = Some(arg.clone());
        } else {
            return Err(format!("unexpected argument '{arg}'\n  usage: {USAGE}"));
        }
    }
    let version = version.ok_or_else(|| format!("usage: {USAGE}"))?;
    let spec = validated_version(&version)?;
    match from {
        Some(repo) => install_from_source(home, &spec, &repo),
        None => install_from_network(home, &spec),
    }
}

// ----------------------------------------------------------------- staging

/// A scratch directory inside `toolchains/` that removes itself on drop, so a
/// failed install leaves no partial tree and no orphaned download behind. It
/// is a sibling of the final destination, which keeps the closing rename on
/// one filesystem.
struct Staging {
    path: PathBuf,
}

impl Staging {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn staging(root: &Path, label: &str) -> Result<Staging, String> {
    let path = root.join(format!(".staging-{label}-{}", unique_suffix()));
    fs::create_dir_all(&path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    Ok(Staging { path })
}

fn directory_has_entries(path: &Path) -> bool {
    fs::read_dir(path)
        .map(|mut entries| entries.next().is_some())
        .unwrap_or(false)
}

/// A toolchain is only usable with all three payloads present: the engine, the
/// standard library it compiles against, and the runtime crate sources the
/// backend feeds to rustc.
fn validate_toolchain_layout(dir: &Path) -> Result<(), String> {
    let folc = dir.join("folc");
    let metadata =
        fs::metadata(&folc).map_err(|_| format!("{} has no folc binary", dir.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file", folc.display()));
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(format!("{} is not executable", folc.display()));
    }
    let std_root = dir.join("std");
    if !std_root.is_dir() || !directory_has_entries(&std_root) {
        return Err(format!("{} has no std/ library", dir.display()));
    }
    let runtime = dir.join("runtime");
    if !runtime.join("Cargo.toml").is_file() || !runtime.join("src").is_dir() {
        return Err(format!("{} has no runtime/ crate sources", dir.display()));
    }
    Ok(())
}

fn swap_into_place(staged: &Path, destination: &Path, root: &Path) -> Result<(), String> {
    contained_in(root, destination)?;
    let backup = root.join(format!(".trash-{}", unique_suffix()));
    let had_previous = destination.exists();
    if had_previous {
        assert_removable_child(root, destination)?;
        fs::rename(destination, &backup).map_err(|error| {
            format!(
                "cannot move the existing toolchain aside from {}: {error}",
                destination.display()
            )
        })?;
    }
    match fs::rename(staged, destination) {
        Ok(()) => {
            if had_previous {
                let _ = fs::remove_dir_all(&backup);
            }
            Ok(())
        }
        Err(error) => {
            if had_previous {
                let _ = fs::rename(&backup, destination);
            }
            Err(format!(
                "cannot move the staged toolchain into {}: {error}",
                destination.display()
            ))
        }
    }
}

fn install_from_source(home: &Path, spec: &Spec, repo: &Path) -> Result<(), String> {
    let repo = repo
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", repo.display()))?;

    // Validate every payload before anything is created or replaced: an
    // install that fails must leave the previous toolchain untouched.
    let bin = newest_built_folc(&repo).ok_or_else(|| {
        format!(
            "no folc binary in {}\n  build one: cargo build --bin folc (or --release)",
            repo.display()
        )
    })?;
    let std_source = repo.join("lang/library/std");
    if !std_source.is_dir() {
        return Err(format!("{} has no lang/library/std", repo.display()));
    }
    let runtime_source = repo.join("lang/execution/fol-runtime");
    if !runtime_source.join("Cargo.toml").is_file() || !runtime_source.join("src").is_dir() {
        return Err(format!(
            "{} has no lang/execution/fol-runtime crate sources",
            repo.display()
        ));
    }

    let root = resolved_toolchains_root(home)?;
    let destination = toolchain_dir(home, spec)?;
    let staged = staging(&root, "src")?;
    let staged_root = staged.path();

    copy_file_preserving_mode(&bin, &staged_root.join("folc"))?;
    copy_tree(&std_source, &staged_root.join("std"), 0)?;
    let runtime_destination = staged_root.join("runtime");
    fs::create_dir_all(&runtime_destination)
        .map_err(|error| format!("cannot create {}: {error}", runtime_destination.display()))?;
    copy_file_preserving_mode(
        &runtime_source.join("Cargo.toml"),
        &runtime_destination.join("Cargo.toml"),
    )?;
    copy_tree(
        &runtime_source.join("src"),
        &runtime_destination.join("src"),
        0,
    )?;

    validate_toolchain_layout(staged_root)?;
    swap_into_place(staged_root, &destination, &root)?;
    println!(
        "installed fol {} -> {} (from {})",
        spec.bare(),
        destination.display(),
        repo.display()
    );
    Ok(())
}

fn release_target() -> String {
    format!("{}-{}", env::consts::ARCH, env::consts::OS)
}

fn install_from_network(home: &Path, spec: &Spec) -> Result<(), String> {
    let version = spec.bare();
    let target = release_target();
    let archive_name = format!("fol-compiler-and-lib-v{version}-{target}.tar.gz");
    let archive_url = format!("{RELEASE_URL_BASE}/v{version}/{archive_name}");
    let sums_name = format!("SHA256SUMS-{target}");
    let sums_url = format!("{RELEASE_URL_BASE}/v{version}/{sums_name}");

    let root = resolved_toolchains_root(home)?;
    let destination = toolchain_dir(home, spec)?;
    let staged = staging(&root, "net")?;
    let archive = staged.path().join(&archive_name);
    let sums = staged.path().join(&sums_name);

    fetch(&archive_url, &archive).map_err(|error| {
        format!(
            "cannot fetch fol {version} from {archive_url}\n  {error}\n  \
             if you have a source checkout, use: fol self install {version} --from <repo>"
        )
    })?;
    fetch(&sums_url, &sums).map_err(|error| {
        format!(
            "cannot fetch the checksums for fol {version} from {sums_url}\n  {error}\n  \
             a release without {sums_name} cannot be verified and will not be installed"
        )
    })?;
    verify_download(&archive, &sums, &archive_name)?;
    audit_archive_members(&archive)?;

    let unpacked = staged.path().join("unpack");
    fs::create_dir_all(&unpacked)
        .map_err(|error| format!("cannot create {}: {error}", unpacked.display()))?;
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(&unpacked)
        .arg("--no-same-owner")
        .arg("--no-same-permissions")
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            return Err(format!(
                "tar failed with {status} while unpacking {archive_url}"
            ))
        }
        Err(error) => return Err(format!("cannot run tar: {error}")),
    }

    validate_toolchain_layout(&unpacked)
        .map_err(|error| format!("the archive from {archive_url} is incomplete: {error}"))?;
    swap_into_place(&unpacked, &destination, &root)?;
    println!("fetched fol {version} -> {}", destination.display());
    Ok(())
}

fn fetch(url: &str, output: &Path) -> Result<(), String> {
    let curl = Command::new("curl")
        .args([
            "-fL",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--connect-timeout",
            "10",
            "--max-time",
            "300",
            "--retry",
            "2",
            url,
            "-o",
        ])
        .arg(output)
        .status();
    match curl {
        Ok(status) if status.success() => return Ok(()),
        Ok(_) => return Err("download failed (curl)".to_string()),
        Err(_) => {}
    }
    let wget = Command::new("wget")
        .args(["-q", "--timeout=300", "--tries=2", url, "-O"])
        .arg(output)
        .status();
    match wget {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err("download failed (wget)".to_string()),
        Err(_) => Err("neither curl nor wget is available".to_string()),
    }
}

// ---------------------------------------------------------------- checksums

fn first_hex64(text: &str) -> Option<String> {
    text.split_whitespace()
        .find(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(|token| token.to_ascii_lowercase())
}

/// Digest via an external tool so the manager keeps zero dependencies.
fn sha256_of_file(path: &Path) -> Result<String, String> {
    let candidates: [(&str, &[&str]); 3] = [
        ("sha256sum", &[]),
        ("shasum", &["-a", "256"]),
        ("openssl", &["dgst", "-sha256"]),
    ];
    for (program, args) in candidates {
        let output = Command::new(program).args(args).arg(path).output();
        match output {
            Ok(output) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                return first_hex64(&text)
                    .ok_or_else(|| format!("{program} produced no digest for {}", path.display()));
            }
            _ => continue,
        }
    }
    Err(
        "cannot verify the download: none of sha256sum, shasum, or openssl is available\n  \
         install coreutils (sha256sum) and retry"
            .to_string(),
    )
}

fn parse_sha256sums(content: &str, file_name: &str) -> Option<String> {
    for line in content.lines() {
        let mut fields = line.split_whitespace();
        let digest = fields.next()?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        let Some(name) = fields.next() else {
            continue;
        };
        let name = name.trim_start_matches('*');
        let base = Path::new(name)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| name.to_string());
        if base == file_name {
            return Some(digest.to_ascii_lowercase());
        }
    }
    None
}

fn verify_download(archive: &Path, sums: &Path, file_name: &str) -> Result<(), String> {
    let content = fs::read_to_string(sums)
        .map_err(|error| format!("cannot read {}: {error}", sums.display()))?;
    let expected = parse_sha256sums(&content, file_name)
        .ok_or_else(|| format!("the published checksums have no entry for {file_name}"))?;
    let actual = sha256_of_file(archive)?;
    if actual != expected {
        return Err(format!(
            "checksum mismatch for {file_name}\n  expected {expected}\n  actual   {actual}\n  refusing to extract"
        ));
    }
    Ok(())
}

/// Reject archives that would write outside the toolchain directory or carry
/// anything beyond the three expected payloads.
fn audit_archive_members(archive: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .arg("-tzf")
        .arg(archive)
        .output()
        .map_err(|error| format!("cannot run tar: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cannot list the archive members of {}",
            archive.display()
        ));
    }
    let listing = String::from_utf8_lossy(&output.stdout);
    for member in listing.lines() {
        let member = member.trim();
        if member.is_empty() {
            continue;
        }
        let path = Path::new(member);
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(format!("unsafe archive member '{member}'"));
        }
        let first = path
            .components()
            .next()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .unwrap_or_default();
        if !matches!(first.as_str(), "folc" | "std" | "runtime" | "." | "") {
            return Err(format!("unexpected archive member '{member}'"));
        }
    }
    Ok(())
}

// ------------------------------------------------------------ self: link …

fn self_link(home: &Path, args: &[String]) -> Result<(), String> {
    const USAGE: &str = "fol self link <name> <repo-root> [--bin <path>] [--std <path>]";
    let mut positional: Vec<&String> = Vec::new();
    let mut explicit_bin: Option<PathBuf> = None;
    let mut explicit_std: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--bin" | "--std" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("{arg} needs a path\n  usage: {USAGE}"))?;
                let path = PathBuf::from(value)
                    .canonicalize()
                    .map_err(|error| format!("cannot resolve {value}: {error}"))?;
                let slot = if arg == "--bin" {
                    &mut explicit_bin
                } else {
                    &mut explicit_std
                };
                if slot.is_some() {
                    return Err(format!("{arg} was given twice\n  usage: {USAGE}"));
                }
                *slot = Some(path);
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option '{other}'\n  usage: {USAGE}"))
            }
            _ => positional.push(arg),
        }
    }
    let [name, repo] = positional.as_slice() else {
        return Err(format!("usage: {USAGE}"));
    };
    let spec = validated_name(name)?;
    let repo = PathBuf::from(repo.as_str())
        .canonicalize()
        .map_err(|error| format!("cannot resolve {repo}: {error}"))?;
    if !repo.join("lang/library/std").is_dir() {
        return Err(format!(
            "{} does not look like a fol source tree (no lang/library/std)",
            repo.display()
        ));
    }
    for (label, value) in [("bin", &explicit_bin), ("std", &explicit_std)] {
        if let Some(path) = value {
            let text = path.to_string_lossy();
            if text.contains('"') || text.contains('\n') {
                return Err(format!(
                    "the --{label} path may not contain quotes or newlines"
                ));
            }
        }
    }

    let manifest = link_manifest_path(home, &spec)?;
    let mut body = format!("repo = \"{}\"\n", repo.display());
    if let Some(bin) = &explicit_bin {
        body.push_str(&format!("bin = \"{}\"\n", bin.display()));
    }
    if let Some(std_root) = &explicit_std {
        body.push_str(&format!("std = \"{}\"\n", std_root.display()));
    }
    fs::write(&manifest, body)
        .map_err(|error| format!("cannot write {}: {error}", manifest.display()))?;
    println!("linked toolchain '{spec}' -> {}", repo.display());
    Ok(())
}

fn self_default(home: &Path, args: &[String]) -> Result<(), String> {
    const USAGE: &str = "fol self default [<version|name> | --unset]";
    match args.first().map(String::as_str) {
        None => {
            match default_spec(home) {
                Some(spec) => println!("{spec}"),
                None => println!("no default toolchain configured"),
            }
            Ok(())
        }
        Some("--unset") => {
            no_extra_args(&args[1..], USAGE)?;
            config_set_default(home, None)?;
            println!("default toolchain cleared");
            Ok(())
        }
        Some(raw) => {
            no_extra_args(&args[1..], USAGE)?;
            let spec = validated_spec(raw)?;
            resolve_toolchain(home, &spec, false)?;
            if !installed_toolchains(home)
                .iter()
                .any(|toolchain| toolchain.spec.matches(&spec.raw))
            {
                return Err(format!(
                    "toolchain '{spec}' is not installed\n  run: fol self install {spec}"
                ));
            }
            config_set_default(home, Some(&spec))?;
            println!("default toolchain is now {spec}");
            Ok(())
        }
    }
}

fn self_list(home: &Path, args: &[String]) -> Result<(), String> {
    no_extra_args(args, "fol self list")?;
    let toolchains = installed_toolchains(home);
    if toolchains.is_empty() {
        println!(
            "no toolchains installed in {}",
            toolchains_root(home).display()
        );
        println!("  install one: fol self install <version>");
        println!("  or link a source tree: fol self link dev <repo-root>");
        return Ok(());
    }
    let default = default_spec(home);
    println!("toolchains in {}:", toolchains_root(home).display());
    for toolchain in toolchains {
        let name = if toolchain.linked {
            toolchain.spec.raw.clone()
        } else {
            toolchain.spec.dir_name()
        };
        let is_default = default
            .as_ref()
            .is_some_and(|spec| spec.matches(&toolchain.spec.raw));
        let mut line = format!("  {name}");
        if toolchain.linked {
            match resolve_linked(home, &toolchain.spec) {
                Ok(Some(resolved)) => line.push_str(&format!(" -> {}", resolved.bin.display())),
                _ => line.push_str(" (broken link)"),
            }
        }
        if is_default {
            line.push_str("   [default]");
        }
        println!("{line}");
    }
    Ok(())
}

fn self_remove(home: &Path, args: &[String]) -> Result<(), String> {
    const USAGE: &str = "fol self remove <version|name>";
    let [raw] = args else {
        return Err(format!("usage: {USAGE}"));
    };
    let spec = validated_spec(raw)?;
    let root = resolved_toolchains_root(home)?;
    let was_default = default_spec(home).is_some_and(|default| default.matches(&spec.raw));

    let manifest = link_manifest_path(home, &spec)?;
    if manifest.is_file() {
        assert_removable_child(&root, &manifest)?;
        fs::remove_file(&manifest)
            .map_err(|error| format!("cannot remove {}: {error}", manifest.display()))?;
        println!("removed link '{spec}'");
        return clear_default_if_removed(home, was_default);
    }
    if spec.is_version() {
        let dir = toolchain_dir(home, &spec)?;
        if dir.is_dir() {
            assert_removable_child(&root, &dir)?;
            fs::remove_dir_all(&dir)
                .map_err(|error| format!("cannot remove {}: {error}", dir.display()))?;
            println!("removed toolchain {spec}");
            return clear_default_if_removed(home, was_default);
        }
    }
    Err(format!("toolchain '{spec}' is not installed"))
}

fn clear_default_if_removed(home: &Path, was_default: bool) -> Result<(), String> {
    if !was_default {
        return Ok(());
    }
    config_set_default(home, None)?;
    eprintln!(
        "warning: that was the default toolchain — the default has been cleared\n  \
         run: fol self default <version|name>"
    );
    Ok(())
}

fn self_which(home: &Path, override_spec: Option<&str>, args: &[String]) -> Result<(), String> {
    no_extra_args(args, "fol self which")?;
    let spec = select_toolchain_spec(home, override_spec)?;
    let toolchain = resolve_toolchain(home, &spec, false)?;
    let kind = if toolchain.linked {
        "linked"
    } else {
        "installed"
    };
    println!(
        "{} ({kind} toolchain '{}')",
        toolchain.bin.display(),
        toolchain.spec
    );
    if toolchain.linked {
        println!("  std: {}", toolchain.std_root.display());
    }
    Ok(())
}

// ------------------------------------------------------------------ util

/// Pick the most recently built folc between release and debug — blindly
/// preferring release serves stale binaries after debug-only rebuilds.
fn newest_built_folc(repo: &Path) -> Option<PathBuf> {
    [
        repo.join("target/release/folc"),
        repo.join("target/debug/folc"),
    ]
    .into_iter()
    .filter(|path| path.is_file())
    .max_by_key(|path| fs::metadata(path).and_then(|meta| meta.modified()).ok())
}

fn copy_file_preserving_mode(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to).map_err(|error| format!("cannot copy {}: {error}", from.display()))?;
    let mode = fs::metadata(from)
        .map(|metadata| metadata.permissions().mode())
        .map_err(|error| format!("cannot inspect {}: {error}", from.display()))?;
    fs::set_permissions(to, fs::Permissions::from_mode(mode))
        .map_err(|error| format!("cannot set permissions on {}: {error}", to.display()))
}

/// Copy a tree without ever following a symlink: a staged toolchain must be a
/// self-contained set of real files, and following links recurses forever.
fn copy_tree(source: &Path, destination: &Path, depth: u32) -> Result<(), String> {
    if depth > MAX_COPY_DEPTH {
        return Err(format!(
            "{} nests deeper than {MAX_COPY_DEPTH} directories",
            source.display()
        ));
    }
    fs::create_dir_all(destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    let entries = fs::read_dir(source)
        .map_err(|error| format!("cannot read {}: {error}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", source.display()))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", from.display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "refusing to copy symlink {} while staging a toolchain",
                from.display()
            ));
        }
        if file_type.is_dir() {
            copy_tree(&from, &to, depth + 1)?;
        } else {
            copy_file_preserving_mode(&from, &to)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_is_read_from_the_leading_comment_block() {
        let content = "// build.fol — my project\n\n//fol 0.2.0\n\nvar build = graph.package(\"x\", \"1.0.0\")\n";
        assert_eq!(parse_toolchain_pin(content), Some("0.2.0".to_string()));
    }

    #[test]
    fn pin_accepts_spacing_variants() {
        assert_eq!(
            parse_toolchain_pin("//fol 0.4.0\n"),
            Some("0.4.0".to_string())
        );
        assert_eq!(
            parse_toolchain_pin("// fol   v0.4.0\n"),
            Some("v0.4.0".to_string())
        );
        assert_eq!(
            parse_toolchain_pin("//   fol\tdev\n"),
            Some("dev".to_string())
        );
    }

    #[test]
    fn pin_survives_a_byte_order_mark() {
        assert_eq!(
            parse_toolchain_pin("\u{feff}//fol 0.2.0\n"),
            Some("0.2.0".to_string())
        );
    }

    #[test]
    fn pin_stops_at_the_first_code_line() {
        let content = "var build = graph.package(\"x\", \"1.0.0\")\n//fol 0.2.0\n";
        assert_eq!(parse_toolchain_pin(content), None);
    }

    #[test]
    fn pin_ignores_comments_that_merely_mention_fol() {
        assert_eq!(parse_toolchain_pin("// the fol build manifest\n"), None);
        assert_eq!(parse_toolchain_pin("//folly 1.0\n"), None);
    }

    #[test]
    fn version_dirs_are_normalized_with_a_v_prefix() {
        assert_eq!(validated_spec("0.2.0").unwrap().dir_name(), "v0.2.0");
        assert_eq!(validated_spec("v0.2.0").unwrap().dir_name(), "v0.2.0");
    }

    #[test]
    fn version_shapes_are_distinguished_from_names() {
        for raw in ["0.2.0", "v0.2.0", "1", "0.3.0-rc.1", "2026.1.0"] {
            assert!(
                validated_spec(raw).unwrap().is_version(),
                "{raw} should be a version"
            );
        }
        for raw in ["dev", "nightly-2026-01-01", "my_branch"] {
            assert!(
                !validated_spec(raw).unwrap().is_version(),
                "{raw} should be a name"
            );
        }
    }

    #[test]
    fn specs_with_path_syntax_are_rejected() {
        for raw in [
            "",
            ".",
            "..",
            "/",
            "0.2.0/../../escape",
            "../evil",
            "./x",
            "a\\b",
            "with space",
            ".hidden",
            "-dash",
            "0.2.0\0",
        ] {
            assert!(
                validated_spec(raw).is_err(),
                "{raw:?} should be rejected as a spec"
            );
        }
        assert!(validated_spec(&"a".repeat(65)).is_err());
    }

    #[test]
    fn containment_rejects_escapes_and_foreign_roots() {
        let root = Path::new("/home/fol/toolchains");
        assert!(contained_in(root, &root.join("v0.2.0")).is_ok());
        assert!(contained_in(root, Path::new("/home/fol/toolchains/../evil")).is_err());
        assert!(contained_in(root, Path::new("/elsewhere/v0.2.0")).is_err());
    }

    #[test]
    fn versions_sort_numerically_not_lexically() {
        let mut versions = ["0.10.0", "0.2.0", "1.0.0"];
        versions.sort_by_key(|version| version_sort_key(version));
        assert_eq!(versions, ["0.2.0", "0.10.0", "1.0.0"]);
    }

    #[test]
    fn link_manifests_parse_keys_and_quoted_values() {
        let entries = parse_link_manifest("# comment\nrepo = \"/some/path\"\nbin=\"/x/folc\"\n");
        assert_eq!(
            entries,
            vec![
                ("repo".to_string(), "/some/path".to_string()),
                ("bin".to_string(), "/x/folc".to_string()),
            ]
        );
    }

    #[test]
    fn config_lookup_needs_an_exact_key_and_survives_junk() {
        let config = Config {
            lines: vec![
                "# a comment".to_string(),
                "defaults = wrong".to_string(),
                "garbage".to_string(),
                "default = v0.2.0".to_string(),
                "other = 1".to_string(),
            ],
        };
        assert_eq!(config_get(&config, "default"), Some("v0.2.0".to_string()));
        assert_eq!(config_get(&config, "other"), Some("1".to_string()));
        assert_eq!(config_get(&config, "missing"), None);
    }

    #[test]
    fn checksum_lines_are_matched_by_basename() {
        let content = "\
00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff  other.tar.gz\n\
ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100 *./dist/wanted.tar.gz\n";
        assert_eq!(
            parse_sha256sums(content, "wanted.tar.gz"),
            Some("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100".to_string())
        );
        assert_eq!(parse_sha256sums(content, "absent.tar.gz"), None);
    }

    #[test]
    fn digests_are_read_out_of_tool_output() {
        assert_eq!(
            first_hex64("0000000000000000000000000000000000000000000000000000000000000001  file\n"),
            Some("0000000000000000000000000000000000000000000000000000000000000001".to_string())
        );
        assert_eq!(first_hex64("SHA256(file)= DEADBEEF\n"), None);
    }
}

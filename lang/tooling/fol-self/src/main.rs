//! `fol` — the FOL toolchain manager.
//!
//! This is the only binary a distribution packages. It owns `fol self …`
//! (installing, linking, and selecting toolchains inside FOL_HOME) and
//! forwards every other command to the selected toolchain's `folc` binary.
//! It contains no language logic.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DISPATCH_GUARD: &str = "FOL_DISPATCHED";
const TOOLCHAIN_ENV: &str = "FOL_TOOLCHAIN";
const RELEASE_URL_BASE: &str = "https://github.com/fol-lang/fol/releases/download";

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let code = run(&args);
    std::process::exit(code);
}

fn run(args: &[String]) -> i32 {
    match args.first().map(String::as_str) {
        Some("self") | Some("s") => run_self(&args[1..]),
        _ => dispatch(args),
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
    println!("  {}  {}  Workspace management", bold_pad("work", 4), dim("[aliases: w]"));
    println!("  {}  {}  Package management", bold_pad("pack", 4), dim("[aliases: p]"));
    println!("  {}  {}  Build, run, test, check", bold_pad("code", 4), dim("[aliases: c]"));
    println!("  {}  {}  Editor tools, LSP, completion", bold_pad("tool", 4), dim("[aliases: t]"));
    println!("  {}  {}  Toolchain management (install, link, default)", bold_pad("self", 4), dim("[aliases: s]"));
    println!();
    println!("{}", section("Options:"));
    println!("  {}, {}     Print help", bold_pad("-h", 2), bold_pad("--help", 6));
    println!("  {}, {}  Print version", bold_pad("-V", 2), bold_pad("--version", 9));
    println!();
    println!("{}", dim("Run `fol <group> <command> --help` for command-specific usage."));
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
    println!("  {}  Install a toolchain ({} copies a built source tree)", bold_pad("install", 7), dim("--from <repo>"));
    println!("  {}  Register a source checkout as a named toolchain", bold_pad("link", 7));
    println!("  {}  Set the default toolchain", bold_pad("default", 7));
    println!("  {}  Show installed toolchains", bold_pad("list", 7));
    println!("  {}  Delete an installed toolchain or link", bold_pad("remove", 7));
    println!("  {}  Print the folc binary this directory resolves to", bold_pad("which", 7));
    println!();
    println!("{}", section("Options:"));
    println!("  {}, {}  Print help", bold_pad("-h", 2), bold_pad("--help", 6));
    println!();
    println!(
        "{}",
        dim("selection order: +<toolchain> arg, FOL_TOOLCHAIN env, `//fol <version>` pin in build.fol, configured default.")
    );
}

fn fail(message: &str) -> i32 {
    eprintln!("error: {message}");
    1
}

// ---------------------------------------------------------------- FOL_HOME

fn fol_home() -> Result<PathBuf, String> {
    if let Some(value) = env::var_os("FOL_HOME") {
        if !value.is_empty() {
            return Ok(PathBuf::from(value));
        }
    }
    // Without FOL_HOME, a project keeps its toolchains next to its build
    // artifacts, under the .fol/ directory beside build.fol.
    if let Some(manifest) = env::current_dir()
        .ok()
        .and_then(|cwd| find_build_manifest(&cwd))
    {
        if let Some(project) = manifest.parent() {
            return Ok(project.join(".fol").join("toolchain"));
        }
    }
    Err(
        "FOL_HOME is not set and no build.fol was found from here upward\n\n  \
         fol keeps toolchains in FOL_HOME, or — inside a project — in\n  \
         <project>/.fol/toolchain. enter a project, or set FOL_HOME in your\n  \
         shell profile, for example:\n\n    \
         export FOL_HOME=\"$HOME/.fol\""
            .to_string(),
    )
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

// ------------------------------------------------------------------ config

fn read_default_toolchain(home: &Path) -> Option<String> {
    let content = fs::read_to_string(home.join("config")).ok()?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("default") {
            let value = value.trim_start().strip_prefix('=')?.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn write_default_toolchain(home: &Path, spec: &str) -> Result<(), String> {
    let path = home.join("config");
    fs::write(&path, format!("default = {spec}\n"))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
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
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(comment) = line.strip_prefix("//") else {
            return None;
        };
        let comment = comment.trim_start();
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
    spec: String,
    bin: PathBuf,
    linked: bool,
}

fn normalize_version_dir(spec: &str) -> String {
    let bare = spec.strip_prefix('v').unwrap_or(spec);
    format!("v{bare}")
}

fn looks_like_version(spec: &str) -> bool {
    let bare = spec.strip_prefix('v').unwrap_or(spec);
    bare.chars().next().is_some_and(|c| c.is_ascii_digit())
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

fn resolve_linked(home: &Path, spec: &str) -> Result<Option<ResolvedToolchain>, String> {
    let manifest_path = toolchains_root(home).join(format!("{spec}.toml"));
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let content = fs::read_to_string(&manifest_path)
        .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
    let entries = parse_link_manifest(&content);
    let lookup = |key: &str| {
        entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| PathBuf::from(v))
    };
    let repo = lookup("repo");
    let bin = match lookup("bin") {
        Some(bin) => bin,
        None => {
            let repo = repo.clone().ok_or_else(|| {
                format!("link '{spec}' has neither `bin` nor `repo` in {}", manifest_path.display())
            })?;
            newest_built_folc(&repo).ok_or_else(|| {
                format!(
                    "link '{spec}' points at {} but no folc binary exists there\n  build one: cargo build --bin folc",
                    repo.display()
                )
            })?
        }
    };
    let std_root = match lookup("std") {
        Some(std_root) => std_root,
        None => {
            let repo = repo.ok_or_else(|| {
                format!("link '{spec}' has neither `std` nor `repo` in {}", manifest_path.display())
            })?;
            repo.join("lang/library/std")
        }
    };
    if !bin.is_file() {
        return Err(format!("link '{spec}': binary {} does not exist", bin.display()));
    }
    if !std_root.is_dir() {
        return Err(format!("link '{spec}': std root {} does not exist", std_root.display()));
    }
    Ok(Some(ResolvedToolchain {
        spec: spec.to_string(),
        bin,
        linked: true,
    }))
}

fn resolve_installed(home: &Path, spec: &str) -> Option<ResolvedToolchain> {
    let dir = toolchains_root(home).join(normalize_version_dir(spec));
    let bin = dir.join("folc");
    if !bin.is_file() {
        return None;
    }
    Some(ResolvedToolchain {
        spec: spec.to_string(),
        bin,
        linked: false,
    })
}

fn resolve_toolchain(home: &Path, spec: &str, allow_fetch: bool) -> Result<ResolvedToolchain, String> {
    if let Some(linked) = resolve_linked(home, spec)? {
        return Ok(linked);
    }
    if let Some(installed) = resolve_installed(home, spec) {
        return Ok(installed);
    }
    if looks_like_version(spec) && allow_fetch {
        eprintln!("toolchain {spec} not installed, fetching...");
        install_from_network(home, spec.strip_prefix('v').unwrap_or(spec))?;
        return resolve_installed(home, spec)
            .ok_or_else(|| format!("toolchain {spec} was fetched but folc is missing from it"));
    }
    Err(format!(
        "toolchain '{spec}' is not installed\n  run: fol self install {spec}",
    ))
}

fn installed_toolchains(home: &Path) -> Vec<(String, bool)> {
    let mut toolchains = Vec::new();
    let Ok(entries) = fs::read_dir(toolchains_root(home)) else {
        return toolchains;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        if path.is_dir() && path.join("folc").is_file() {
            toolchains.push((name, false));
        } else if let Some(link_name) = name.strip_suffix(".toml") {
            toolchains.push((link_name.to_string(), true));
        }
    }
    toolchains.sort();
    toolchains
}

fn select_toolchain_spec(home: &Path, override_spec: Option<&str>) -> Result<String, String> {
    if let Some(spec) = override_spec {
        return Ok(spec.to_string());
    }
    if let Ok(spec) = env::var(TOOLCHAIN_ENV) {
        if !spec.is_empty() {
            return Ok(spec);
        }
    }
    if let Some(pin) = pinned_toolchain() {
        return Ok(pin);
    }
    if let Some(default) = read_default_toolchain(home) {
        return Ok(default);
    }
    let installed = installed_toolchains(home);
    if installed.len() == 1 {
        return Ok(installed[0].0.clone());
    }
    Err(
        "no toolchain selected and no default configured\n  \
         run: fol self default <version|name>"
            .to_string(),
    )
}

// --------------------------------------------------------------- dispatch

fn dispatch(args: &[String]) -> i32 {
    if env::var_os(DISPATCH_GUARD).is_some() {
        return fail(
            "recursive fol dispatch detected — a toolchain binary invoked `fol` again.\n  \
             this usually means a manager binary was installed as a toolchain's folc",
        );
    }
    let (override_spec, rest) = match args.first().and_then(|arg| arg.strip_prefix('+')) {
        Some(spec) => (Some(spec.to_string()), &args[1..]),
        None => (None, args),
    };

    // Help/version/no-args forward to the resolved folc so the user always
    // sees the real frontend surface; the manager's own help is only a
    // fallback for when nothing is resolvable yet.
    let help_like = rest.is_empty()
        || matches!(rest[0].as_str(), "-h" | "--help" | "-V" | "--version");

    let resolved = fol_home().and_then(|home| {
        ensure_home_layout(&home)?;
        let spec = select_toolchain_spec(&home, override_spec.as_deref())?;
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

    // std wiring needs no env: folc resolves std/ next to its own binary
    // (installed toolchains) or falls back to its compiled-in source tree
    // (dev builds and linked checkouts).
    let mut command = Command::new(&toolchain.bin);
    command.args(rest);
    command.env(DISPATCH_GUARD, "1");

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let error = command.exec();
        fail(&format!("cannot run {}: {error}", toolchain.bin.display()))
    }
    #[cfg(not(unix))]
    {
        match command.status() {
            Ok(status) => status.code().unwrap_or(1),
            Err(error) => fail(&format!("cannot run {}: {error}", toolchain.bin.display())),
        }
    }
}

// ------------------------------------------------------------- fol self …

fn run_self(args: &[String]) -> i32 {
    let subcommand = match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print_self_help();
            return 0;
        }
        Some(subcommand) => subcommand,
    };
    let home = match fol_home() {
        Ok(home) => home,
        Err(message) => return fail(&message),
    };
    if let Err(message) = ensure_home_layout(&home) {
        return fail(&message);
    }
    let result = match subcommand {
        "install" => self_install(&home, &args[1..]),
        "link" => self_link(&home, &args[1..]),
        "default" => self_default(&home, &args[1..]),
        "list" => self_list(&home),
        "remove" => self_remove(&home, &args[1..]),
        "which" => self_which(&home),
        other => Err(format!("unknown self subcommand '{other}'\n  run: fol self")),
    };
    match result {
        Ok(()) => 0,
        Err(message) => fail(&message),
    }
}

fn self_install(home: &Path, args: &[String]) -> Result<(), String> {
    let mut version: Option<String> = None;
    let mut from: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--from" {
            let value = iter
                .next()
                .ok_or_else(|| "--from needs a path to a built fol source tree".to_string())?;
            from = Some(PathBuf::from(value));
        } else if version.is_none() {
            version = Some(arg.strip_prefix('v').unwrap_or(arg).to_string());
        } else {
            return Err(format!("unexpected argument '{arg}'"));
        }
    }
    let version = version.ok_or_else(|| "usage: fol self install <version> [--from <repo>]".to_string())?;
    if !looks_like_version(&version) {
        return Err(format!("'{version}' does not look like a version"));
    }
    match from {
        Some(repo) => install_from_source(home, &version, &repo),
        None => install_from_network(home, &version),
    }
}

fn install_from_source(home: &Path, version: &str, repo: &Path) -> Result<(), String> {
    let repo = repo
        .canonicalize()
        .map_err(|error| format!("cannot resolve {}: {error}", repo.display()))?;
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

    let destination = toolchains_root(home).join(normalize_version_dir(version));
    if destination.exists() {
        fs::remove_dir_all(&destination)
            .map_err(|error| format!("cannot replace {}: {error}", destination.display()))?;
    }
    fs::create_dir_all(&destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    fs::copy(&bin, destination.join("folc"))
        .map_err(|error| format!("cannot copy {}: {error}", bin.display()))?;
    copy_dir(&std_source, &destination.join("std"))?;
    println!(
        "installed fol {version} -> {} (from {})",
        destination.display(),
        repo.display()
    );
    Ok(())
}

fn install_from_network(home: &Path, version: &str) -> Result<(), String> {
    let target = format!("{}-{}", env::consts::ARCH, env::consts::OS);
    let url = format!("{RELEASE_URL_BASE}/v{version}/fol-compiler-and-lib-v{version}-{target}.tar.gz");
    let destination = toolchains_root(home).join(normalize_version_dir(version));
    let tarball = toolchains_root(home).join(format!(".download-v{version}.tar.gz"));

    let fetched = fetch(&url, &tarball);
    if let Err(error) = fetched {
        let _ = fs::remove_file(&tarball);
        return Err(format!(
            "cannot fetch fol {version} from {url}\n  {error}\n  \
             if you have a source checkout, use: fol self install {version} --from <repo>"
        ));
    }
    if destination.exists() {
        fs::remove_dir_all(&destination)
            .map_err(|error| format!("cannot replace {}: {error}", destination.display()))?;
    }
    fs::create_dir_all(&destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    let status = Command::new("tar")
        .args(["-xzf"])
        .arg(&tarball)
        .arg("-C")
        .arg(&destination)
        .status();
    let _ = fs::remove_file(&tarball);
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => return Err(format!("tar failed with {status} while unpacking {url}")),
        Err(error) => return Err(format!("cannot run tar: {error}")),
    }
    if !destination.join("folc").is_file() {
        return Err(format!(
            "archive from {url} did not contain a folc binary; removing it is safe: fol self remove {version}"
        ));
    }
    println!("fetched fol {version} -> {}", destination.display());
    Ok(())
}

fn fetch(url: &str, output: &Path) -> Result<(), String> {
    let curl = Command::new("curl")
        .args(["-fL", "--silent", "--show-error", url, "-o"])
        .arg(output)
        .status();
    match curl {
        Ok(status) if status.success() => return Ok(()),
        Ok(_) => return Err("download failed (curl)".to_string()),
        Err(_) => {}
    }
    let wget = Command::new("wget")
        .args(["-q", url, "-O"])
        .arg(output)
        .status();
    match wget {
        Ok(status) if status.success() => Ok(()),
        Ok(_) => Err("download failed (wget)".to_string()),
        Err(_) => Err("neither curl nor wget is available".to_string()),
    }
}

fn self_link(home: &Path, args: &[String]) -> Result<(), String> {
    let [name, repo] = args else {
        return Err("usage: fol self link <name> <repo-root>".to_string());
    };
    if looks_like_version(name) {
        return Err(format!(
            "'{name}' looks like a version; links need a name like 'dev'"
        ));
    }
    let repo = PathBuf::from(repo)
        .canonicalize()
        .map_err(|error| format!("cannot resolve {repo}: {error}"))?;
    if !repo.join("lang/library/std").is_dir() {
        return Err(format!(
            "{} does not look like a fol source tree (no lang/library/std)",
            repo.display()
        ));
    }
    let manifest = toolchains_root(home).join(format!("{name}.toml"));
    fs::write(&manifest, format!("repo = \"{}\"\n", repo.display()))
        .map_err(|error| format!("cannot write {}: {error}", manifest.display()))?;
    println!("linked toolchain '{name}' -> {}", repo.display());
    Ok(())
}

fn self_default(home: &Path, args: &[String]) -> Result<(), String> {
    let [spec] = args else {
        return Err("usage: fol self default <version|name>".to_string());
    };
    resolve_toolchain(home, spec, false)?;
    write_default_toolchain(home, spec)?;
    println!("default toolchain is now {spec}");
    Ok(())
}

fn self_list(home: &Path) -> Result<(), String> {
    let toolchains = installed_toolchains(home);
    if toolchains.is_empty() {
        println!("no toolchains installed in {}", toolchains_root(home).display());
        println!("  install one: fol self install <version>");
        println!("  or link a source tree: fol self link dev <repo-root>");
        return Ok(());
    }
    let default = read_default_toolchain(home);
    println!("toolchains in {}:", toolchains_root(home).display());
    for (name, linked) in toolchains {
        let bare = name.strip_prefix('v').unwrap_or(&name);
        let is_default = default
            .as_deref()
            .is_some_and(|d| d == name || d.strip_prefix('v').unwrap_or(d) == bare);
        let mut line = format!("  {name}");
        if linked {
            if let Ok(Some(toolchain)) = resolve_linked(home, &name) {
                line.push_str(&format!(" -> {}", toolchain.bin.display()));
            } else {
                line.push_str(" (broken link)");
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
    let [spec] = args else {
        return Err("usage: fol self remove <version|name>".to_string());
    };
    let manifest = toolchains_root(home).join(format!("{spec}.toml"));
    if manifest.is_file() {
        fs::remove_file(&manifest)
            .map_err(|error| format!("cannot remove {}: {error}", manifest.display()))?;
        println!("removed link '{spec}'");
        return Ok(());
    }
    let dir = toolchains_root(home).join(normalize_version_dir(spec));
    if dir.is_dir() {
        fs::remove_dir_all(&dir)
            .map_err(|error| format!("cannot remove {}: {error}", dir.display()))?;
        println!("removed toolchain {spec}");
        return Ok(());
    }
    Err(format!("toolchain '{spec}' is not installed"))
}

fn self_which(home: &Path) -> Result<(), String> {
    let spec = select_toolchain_spec(home, None)?;
    let toolchain = resolve_toolchain(home, &spec, false)?;
    let kind = if toolchain.linked { "linked" } else { "installed" };
    println!("{} ({kind} toolchain '{}')", toolchain.bin.display(), toolchain.spec);
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

fn copy_dir(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("cannot create {}: {error}", destination.display()))?;
    let entries = fs::read_dir(source)
        .map_err(|error| format!("cannot read {}: {error}", source.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", source.display()))?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map_err(|error| format!("cannot copy {}: {error}", from.display()))?;
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
        assert_eq!(parse_toolchain_pin("//fol 0.4.0\n"), Some("0.4.0".to_string()));
        assert_eq!(parse_toolchain_pin("// fol   v0.4.0\n"), Some("v0.4.0".to_string()));
        assert_eq!(parse_toolchain_pin("//   fol\tdev\n"), Some("dev".to_string()));
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
        assert_eq!(normalize_version_dir("0.2.0"), "v0.2.0");
        assert_eq!(normalize_version_dir("v0.2.0"), "v0.2.0");
    }

    #[test]
    fn version_shapes_are_distinguished_from_names() {
        assert!(looks_like_version("0.2.0"));
        assert!(looks_like_version("v0.2.0"));
        assert!(!looks_like_version("dev"));
        assert!(!looks_like_version("nightly"));
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
}

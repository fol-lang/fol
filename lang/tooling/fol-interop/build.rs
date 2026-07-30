//! Compile-time interop provenance.
//!
//! The pipeline used to verify sibling checkouts at run time with `git`. That
//! only ever worked in a source tree — a released binary carried the build
//! machine's paths baked in — and it cannot work at all now that the interop
//! crates are git dependencies rather than sibling directories.
//!
//! Provenance instead becomes a build-time invariant: the revisions recorded in
//! `interop.lock.toml` must equal the ones cargo actually resolved in
//! `Cargo.lock`. A stale lock is therefore a compile error, and the verified
//! revisions are handed to the crate as environment variables.

use std::path::{Path, PathBuf};

const COMPONENTS: [(&str, &str); 3] = [
    ("parc", "follang-parc"),
    ("linc", "follang-linc"),
    ("gerc", "follang-gerc"),
];

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo should provide CARGO_MANIFEST_DIR"),
    );
    let repo_root = manifest_dir.join("../../..");
    let lock_path = repo_root.join("interop.lock.toml");
    let cargo_lock_path = repo_root.join("Cargo.lock");

    println!("cargo:rerun-if-changed={}", lock_path.display());
    println!("cargo:rerun-if-changed={}", cargo_lock_path.display());

    let lock = read(&lock_path);
    let cargo_lock = read(&cargo_lock_path);

    for (component, package) in COMPONENTS {
        let locked_revision = lock_field(&lock, component, "revision").unwrap_or_else(|| {
            panic!("interop.lock.toml is missing [{component}] revision");
        });
        let locked_repository = lock_field(&lock, component, "url")
            .or_else(|| lock_field(&lock, component, "repository"))
            .unwrap_or_else(|| panic!("interop.lock.toml is missing [{component}] url"));

        let resolved = cargo_lock_source(&cargo_lock, package).unwrap_or_else(|| {
            panic!(
                "Cargo.lock has no git source for {package}; run `cargo fetch` and commit the lock"
            )
        });
        let resolved_revision = resolved
            .split("rev=")
            .nth(1)
            .and_then(|tail| tail.split('#').next())
            .unwrap_or_else(|| {
                panic!("Cargo.lock source for {package} pins no revision: {resolved}")
            });

        if resolved_revision != locked_revision {
            panic!(
                "{component} revision drift: interop.lock.toml says {locked_revision}, Cargo.lock resolved {resolved_revision}"
            );
        }
        if !resolved.contains(&normalize_repository(&locked_repository)) {
            panic!(
                "{component} repository drift: interop.lock.toml says {locked_repository}, Cargo.lock resolved {resolved}"
            );
        }

        println!(
            "cargo:rustc-env=FOL_LOCKED_{}_REVISION={locked_revision}",
            component.to_uppercase()
        );
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// `github.com/fol-lang/parc` and `https://github.com/fol-lang/parc` are the
/// same remote; compare on the host-and-path part.
fn normalize_repository(repository: &str) -> String {
    repository
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("git@")
        .replace(':', "/")
}

fn lock_field(lock: &str, section: &str, key: &str) -> Option<String> {
    let mut active = false;
    for line in lock.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            active = line == format!("[{section}]");
            continue;
        }
        if !active {
            continue;
        }
        let Some((found, value)) = line.split_once('=') else {
            continue;
        };
        if found.trim() == key {
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn cargo_lock_source(cargo_lock: &str, package: &str) -> Option<String> {
    let mut in_package = false;
    for line in cargo_lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_package = false;
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            if key == "name" {
                in_package = value == package;
            } else if key == "source" && in_package {
                return Some(value.to_string());
            }
        }
    }
    None
}

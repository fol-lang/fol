//! Compile-time interop provenance.
//!
//! The revisions are pinned in `Cargo.toml` with `rev = "..."`, and `Cargo.lock`
//! records what cargo resolved from them. That is the whole source of truth, so
//! this script only reads the resolved revisions back out and hands them to the
//! crate as environment variables. There is no second lock file to drift.

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
    let cargo_lock_path = manifest_dir.join("../../..").join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", cargo_lock_path.display());

    let cargo_lock = read(&cargo_lock_path);

    for (component, package) in COMPONENTS {
        let resolved = cargo_lock_source(&cargo_lock, package).unwrap_or_else(|| {
            panic!(
                "Cargo.lock has no git source for {package}; run `cargo fetch` and commit the lock"
            )
        });
        let revision = resolved
            .split("rev=")
            .nth(1)
            .and_then(|tail| tail.split('#').next())
            .unwrap_or_else(|| {
                panic!("Cargo.lock source for {package} pins no revision: {resolved}")
            });

        println!(
            "cargo:rustc-env=FOL_LOCKED_{}_REVISION={revision}",
            component.to_uppercase()
        );
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
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

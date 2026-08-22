//! V4 M0 boundary freezes.
//!
//! Each test below asserts an absence. The V4 plan commits to a single foreign
//! contract -- the target-specific C ABI -- and to importing without a general
//! unsafe escape. Both are true today, so the risk is not that they are broken
//! but that a later milestone erodes them one convenience at a time. These
//! guards fail when that starts.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn collect_files(root: &Path, name: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, name, out);
        } else if path.file_name().is_some_and(|file| file == name) {
            out.push(path);
        }
    }
}

/// V4 exposes one foreign-language contract. A Rust-specific export name,
/// language selector, Cargo role, or Rust provider in the build schema,
/// manifest, or examples would make it two.
#[test]
fn v4_build_surface_names_no_rust_specific_field() {
    // Each token is a field or value a Rust seam would need. Section 4.14 of
    // plan/V4_PLAN.md excludes all of them from V4.
    let forbidden = [
        "rust_name",
        "rust_provider",
        "provider_kind = \"rust\"",
        "language_selector",
        "cargo_role",
        "proc_macro",
    ];

    let mut sources = vec![
        repo_root().join("lang/execution/fol-build/src/semantic.rs"),
        repo_root().join("lang/execution/fol-build/src/graph.rs"),
        repo_root().join("lang/execution/fol-build/src/artifact.rs"),
    ];
    collect_files(&repo_root().join("examples"), "build.fol", &mut sources);
    collect_files(
        &repo_root().join("test/apps/showcases"),
        "build.fol",
        &mut sources,
    );

    assert!(
        sources.len() > 50,
        "the sweep found only {} files; the layout moved and this guard stopped guarding",
        sources.len()
    );

    let mut found = Vec::new();
    for path in &sources {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        for token in forbidden {
            if text.contains(token) {
                found.push(format!("{}: {token}", path.display()));
            }
        }
    }

    assert!(
        found.is_empty(),
        "V4 is C-ABI-only; a Rust-specific surface appeared:\n{}",
        found.join("\n")
    );
}

/// There is no general unsafe block in V4. A raw C declaration that cannot be
/// projected into the safe canonical shapes stays uncallable and needs a C
/// adapter, rather than an escape hatch that makes it callable unchecked.
#[test]
fn fol_has_no_general_unsafe_keyword() {
    let all_keywords: Vec<&str> = fol_lexer::token::buildin::DECLARATION_KEYWORDS
        .iter()
        .chain(fol_lexer::token::buildin::CONTROL_KEYWORDS)
        .chain(fol_lexer::token::buildin::OPERATOR_KEYWORDS)
        .chain(fol_lexer::token::buildin::LITERAL_KEYWORDS)
        .chain(fol_lexer::token::buildin::DIAGNOSTIC_KEYWORDS)
        .chain(fol_lexer::token::buildin::OTHER_KEYWORDS)
        .copied()
        .collect();

    assert!(
        !all_keywords.is_empty(),
        "the keyword tables are empty; this guard stopped guarding"
    );
    for spelling in ["unsafe", "unchecked", "transmute", "extern"] {
        assert!(
            !all_keywords.contains(&spelling),
            "'{spelling}' became a FOL keyword; V4 has no general unsafe escape \
             (plan/V4_PLAN.md section 4.8)"
        );
    }
}

/// PARC, LINC, and GERC stay usable without FOL. The pipeline is a general C
/// toolchain stack that FOL happens to consume; the moment one of them depends
/// on a `fol-*` crate it becomes a FOL-specific library, which section 4.12
/// forbids and which would make the three unpublishable on their own.
#[test]
fn sibling_interop_crates_do_not_depend_on_fol() {
    let lock =
        fs::read_to_string(repo_root().join("Cargo.lock")).expect("Cargo.lock should be readable");

    let mut checked = 0;
    for sibling in ["follang-parc", "follang-linc", "follang-gerc"] {
        let marker = format!("name = \"{sibling}\"");
        let start = lock
            .find(&marker)
            .unwrap_or_else(|| panic!("{sibling} should be locked; the interop pins moved"));
        // One lock entry runs to the next `[[package]]` header.
        let rest = &lock[start..];
        let entry = match rest.find("\n[[package]]") {
            Some(end) => &rest[..end],
            None => rest,
        };
        checked += 1;

        let offenders: Vec<&str> = entry
            .lines()
            .map(|line| line.trim().trim_matches(&[' ', '"', ','][..]))
            .filter(|dep| dep.starts_with("fol-") || *dep == "fol")
            .collect();
        assert!(
            offenders.is_empty(),
            "{sibling} depends on {offenders:?}; the interop stack must stay usable \
             without FOL types (plan/V4_PLAN.md section 4.12)"
        );
    }

    assert_eq!(checked, 3, "all three siblings should have been checked");
}

/// The dependency direction is one-way: PARC knows nothing of LINC or GERC, and
/// LINC knows nothing of GERC. A back-edge would make the pipeline a cycle and
/// destroy the staged-upgrade rule in section 4.12.
#[test]
fn sibling_interop_dependency_direction_stays_one_way() {
    let lock =
        fs::read_to_string(repo_root().join("Cargo.lock")).expect("Cargo.lock should be readable");

    let entry_for = |name: &str| -> String {
        let start = lock
            .find(&format!("name = \"{name}\""))
            .unwrap_or_else(|| panic!("{name} should be locked"));
        let rest = &lock[start..];
        match rest.find("\n[[package]]") {
            Some(end) => rest[..end].to_string(),
            None => rest.to_string(),
        }
    };

    let parc = entry_for("follang-parc");
    assert!(
        !parc.contains("follang-linc"),
        "parc must not depend on linc"
    );
    assert!(
        !parc.contains("follang-gerc"),
        "parc must not depend on gerc"
    );

    let linc = entry_for("follang-linc");
    assert!(
        !linc.contains("follang-gerc"),
        "linc must not depend on gerc"
    );
    assert!(
        linc.contains("follang-parc"),
        "linc should consume parc; the pipeline shape changed"
    );
}

/// The normative C header and install layout, frozen by section 4.16.
///
/// Nothing generates a header yet, so this checks the reference rather than an
/// output. It exists so that M5's emitter has one shape to match, and so that
/// editing the reference is a deliberate act rather than a drift.
#[test]
fn v4_contract_header_freezes_naming_guard_and_status() {
    let dir = repo_root().join("examples/v4_contract_header");
    let header = fs::read_to_string(dir.join("demo.h")).expect("demo.h should be readable");

    // Guard: FOL_<ARTIFACT>_H, opened once and closed once.
    assert!(header.contains("#ifndef FOL_DEMO_H"));
    assert!(header.contains("#define FOL_DEMO_H"));
    assert_eq!(
        header.matches("FOL_DEMO_H").count(),
        3,
        "the guard should appear exactly three times: ifndef, define, and the \
         closing comment"
    );

    for (typedef, underlying) in [
        ("fol_status_t", "int32_t"),
        ("fol_bool_t", "uint8_t"),
        ("fol_char_t", "uint32_t"),
    ] {
        assert!(
            header.contains(&format!("typedef {underlying} {typedef};")),
            "{typedef} must be {underlying}"
        );
    }

    // Section 4.7's reserved status values, with the negative ones
    // parenthesized so they survive being pasted into a larger expression.
    for (macro_name, value) in [
        ("FOL_STATUS_OK", "0"),
        ("FOL_STATUS_REPORT", "1"),
        ("FOL_STATUS_INVALID_ARGUMENT", "(-1)"),
        ("FOL_STATUS_PANIC", "(-2)"),
        ("FOL_STATUS_INTERNAL", "(-3)"),
    ] {
        let line = header
            .lines()
            .find(|line| line.contains(&format!("#define {macro_name}")))
            .unwrap_or_else(|| panic!("{macro_name} should be defined"));
        assert!(
            line.trim_end().ends_with(value),
            "{macro_name} should be {value}, got: {line}"
        );
    }

    assert!(
        header.contains("#ifdef __cplusplus") && header.contains("extern \"C\" {"),
        "the header must be usable from C++"
    );
}

/// The install roles section 4.10 calls "M0-frozen" and never stated.
#[test]
fn v4_contract_install_layout_covers_every_output_role() {
    let layout = fs::read_to_string(repo_root().join("examples/v4_contract_header/install.txt"))
        .expect("install.txt should be readable");

    let rows: Vec<(&str, &str)> = layout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once(char::is_whitespace))
        .map(|(role, path)| (role, path.trim()))
        .collect();

    assert_eq!(
        rows,
        vec![
            ("executable", "bin/<artifact>"),
            ("include", "include/<artifact>.h"),
            ("library", "lib/lib<artifact>.a"),
            ("library", "lib/lib<artifact>.so"),
            ("abi", "share/fol/abi/<artifact>.folabi.json"),
        ],
        "the frozen install layout changed; update plan/V4_PLAN.md section 4.16 \
         in the same commit"
    );
}

/// The frozen header has to actually compile. A normative reference that a C
/// compiler rejects would freeze a shape the emitter cannot produce.
///
/// Skipped when no clang is on PATH, so the suite still runs outside the nix
/// shell; CI runs inside it, where the compiler is pinned.
#[test]
fn v4_contract_header_compiles_as_c_and_cxx() {
    use std::process::Command;

    let Some(cc) = std::env::var_os("FOL_H7_CLANG").or_else(|| {
        Command::new("clang")
            .arg("--version")
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|_| "clang".into())
    }) else {
        eprintln!("skipping: no clang on PATH");
        return;
    };

    let dir = repo_root().join("examples/v4_contract_header");
    let scratch = std::env::temp_dir().join(format!("fol_header_freeze_{}", std::process::id()));
    fs::create_dir_all(&scratch).expect("scratch dir should be creatable");

    for (name, source, std_flag, driver) in [
        (
            "probe.c",
            "#include \"demo.h\"\nint main(void){ return FOL_STATUS_OK; }\n",
            "-std=c17",
            cc.clone(),
        ),
        (
            "probe.cpp",
            "#include \"demo.h\"\nint main(){ return FOL_STATUS_OK; }\n",
            "-std=c++17",
            cc.clone(),
        ),
    ] {
        let path = scratch.join(name);
        fs::write(&path, source).expect("probe should be writable");
        // Compile to an object rather than -fsyntax-only: a wrapped clang (nix,
        // and most distro toolchains) injects linker flags that a
        // syntax-only run leaves unused, which -Werror then rejects.
        let object = scratch.join(format!("{name}.o"));
        let output = Command::new(&driver)
            .args([std_flag, "-Wall", "-Wextra", "-Werror", "-c"])
            .arg("-o")
            .arg(&object)
            .arg("-I")
            .arg(&dir)
            .arg(&path)
            .output()
            .expect("the compiler should run");
        assert!(
            output.status.success(),
            "the frozen header failed to compile {name}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

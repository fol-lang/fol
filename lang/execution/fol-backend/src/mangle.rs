use fol_lower::{LoweredGlobalId, LoweredLocalId, LoweredRoutineId, LoweredTypeId};
use fol_resolver::{PackageIdentity, PackageSourceKind};

pub fn sanitize_backend_ident(raw: &str) -> String {
    let mut output = String::new();
    let mut last_was_underscore = false;

    for ch in raw.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if next == '_' {
            if !last_was_underscore {
                output.push(next);
            }
            last_was_underscore = true;
        } else {
            output.push(next);
            last_was_underscore = false;
        }
    }

    let output = output.trim_matches('_').to_string();
    if output.is_empty() {
        "_".to_string()
    } else if output.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("_{output}")
    } else if RUST_KEYWORDS.contains(&output.as_str()) {
        // Namespace/module segments become Rust identifiers verbatim, so a
        // FOL name that collides with a Rust keyword must be escaped.
        format!("{output}_kw")
    } else {
        output
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while", "yield",
];

/// Escape a FOL field or entry-variant name for a Rust IDENTIFIER position
/// (struct field, enum variant, field access). FOL's keyword set does not
/// overlap Rust's, so names like `type` or `match` are valid FOL field names
/// but reserved in Rust — emit them as raw identifiers. `crate`/`self`/
/// `super`/`Self` cannot be raw identifiers, so those few fall back to the
/// `_kw` suffix (the same escape module names use). String-literal positions
/// (runtime field labels) must keep the raw FOL name and NOT use this.
pub fn escape_rust_field_ident(name: &str) -> String {
    match name {
        "crate" | "self" | "super" | "Self" => format!("{name}_kw"),
        _ if RUST_KEYWORDS.contains(&name) => format!("r#{name}"),
        _ => name.to_string(),
    }
}

/// The auxiliary crate that carries every import's generated safe adapters.
///
/// One crate holds them all, each import in its own module, because the
/// auxiliary plan's last crate is the one generated `main` links directly.
/// `fol-interop` writes it under the same name; the two are checked against
/// each other by `the_adapter_crate_name_matches_the_interop_spelling`.
pub const FOREIGN_ADAPTER_CRATE: &str = "fol_h7_anchor";

/// The private module holding one C import's FOL-owned safe adapters.
///
/// Named from the import alias, which the build layer already constrains to
/// `[a-z][a-z0-9_]*`, so no sanitizing is needed -- but it is prefixed, so an
/// import cannot collide with a package module or a generated type.
pub fn foreign_adapter_module_name(alias: &str) -> String {
    format!("cimp__{}", sanitize_backend_ident(alias))
}

pub fn mangle_package_module_name(identity: &PackageIdentity) -> String {
    let name = sanitize_backend_ident(&identity.display_name);
    let tag = package_kind_tag(identity.source_kind);
    // A local package is named by its directory, so two of them can share a
    // basename -- `alpha/util` and `beta/util` both read as `util`. Every other
    // kind is named from a registry, a URL, or the workspace itself, where the
    // name is already unique. Their canonical roots cannot collide, so that is
    // what keeps the two modules (and every symbol mangled through them) apart.
    match identity.source_kind {
        PackageSourceKind::Local => format!(
            "pkg__{tag}__{name}__{:08x}",
            crate::identity::fnv1a64(identity.canonical_root.as_bytes()) as u32
        ),
        _ => format!("pkg__{tag}__{name}"),
    }
}

pub fn mangle_type_name(identity: &PackageIdentity, type_id: LoweredTypeId, name: &str) -> String {
    format!(
        "ty__{}__t{}__{}",
        mangle_package_module_name(identity),
        type_id.0,
        sanitize_backend_ident(name)
    )
}

pub fn mangle_global_name(
    identity: &PackageIdentity,
    global_id: LoweredGlobalId,
    name: &str,
) -> String {
    format!(
        "g__{}__g{}__{}",
        mangle_package_module_name(identity),
        global_id.0,
        sanitize_backend_ident(name)
    )
}

pub fn mangle_routine_name(
    identity: &PackageIdentity,
    routine_id: LoweredRoutineId,
    name: &str,
) -> String {
    format!(
        "r__{}__r{}__{}",
        mangle_package_module_name(identity),
        routine_id.0,
        sanitize_backend_ident(name)
    )
}

pub fn mangle_local_name(
    identity: &PackageIdentity,
    routine_id: LoweredRoutineId,
    local_id: LoweredLocalId,
    name: Option<&str>,
) -> String {
    format!(
        "l__{}__r{}__l{}__{}",
        mangle_package_module_name(identity),
        routine_id.0,
        local_id.0,
        sanitize_backend_ident(name.unwrap_or("tmp"))
    )
}

fn package_kind_tag(kind: PackageSourceKind) -> &'static str {
    match kind {
        PackageSourceKind::Entry => "entry",
        PackageSourceKind::Local => "local",
        PackageSourceKind::Standard => "std",
        PackageSourceKind::Package => "pkg",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        mangle_global_name, mangle_local_name, mangle_package_module_name, mangle_routine_name,
        mangle_type_name, sanitize_backend_ident,
    };
    use crate::testing::package_identity;
    use fol_lower::{LoweredGlobalId, LoweredLocalId, LoweredRoutineId, LoweredTypeId};
    use fol_resolver::PackageSourceKind;

    #[test]
    fn backend_name_mangling_keeps_package_and_symbol_ids_stable() {
        let identity = package_identity("my-app", PackageSourceKind::Entry, "/workspace/my-app");

        assert_eq!(sanitize_backend_ident("Hello-World"), "hello_world");
        assert_eq!(mangle_package_module_name(&identity), "pkg__entry__my_app");
        assert_eq!(
            mangle_type_name(&identity, LoweredTypeId(3), "User"),
            "ty__pkg__entry__my_app__t3__user"
        );
        assert_eq!(
            mangle_global_name(&identity, LoweredGlobalId(4), "default-name"),
            "g__pkg__entry__my_app__g4__default_name"
        );
        assert_eq!(
            mangle_routine_name(&identity, LoweredRoutineId(5), "run"),
            "r__pkg__entry__my_app__r5__run"
        );
        assert_eq!(
            mangle_local_name(
                &identity,
                LoweredRoutineId(5),
                LoweredLocalId(2),
                Some("Flag")
            ),
            "l__pkg__entry__my_app__r5__l2__flag"
        );
    }

    #[test]
    fn backend_name_mangling_is_deterministic_and_source_kind_sensitive() {
        let entry_identity = package_identity("shared", PackageSourceKind::Entry, "/workspace/app");
        let local_identity =
            package_identity("shared", PackageSourceKind::Local, "/workspace/shared");

        let entry_module = mangle_package_module_name(&entry_identity);
        let local_module = mangle_package_module_name(&local_identity);

        assert_eq!(entry_module, mangle_package_module_name(&entry_identity));
        assert_ne!(entry_module, local_module);
        assert_eq!(sanitize_backend_ident("99-bottles"), "_99_bottles");
        assert_eq!(
            mangle_local_name(
                &entry_identity,
                LoweredRoutineId(0),
                LoweredLocalId(1),
                None
            ),
            "l__pkg__entry__shared__r0__l1__tmp"
        );
    }
}

#[cfg(test)]
mod keyword_tests {
    use super::sanitize_backend_ident;

    #[test]
    fn sanitize_escapes_rust_keywords() {
        assert_eq!(sanitize_backend_ident("mod"), "mod_kw");
        assert_eq!(sanitize_backend_ident("impl"), "impl_kw");
        assert_eq!(sanitize_backend_ident("type"), "type_kw");
        assert_eq!(sanitize_backend_ident("helper"), "helper");
    }
}

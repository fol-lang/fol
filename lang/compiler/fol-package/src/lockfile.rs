use crate::{PackageDependencySourceKind, PackageError, PackageErrorKind};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const LOCKFILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockfile {
    pub version: u32,
    pub entries: Vec<PackageLockEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockEntry {
    pub alias: String,
    pub source_kind: PackageDependencySourceKind,
    pub locator: String,
    pub selected_revision: String,
    pub materialized_root: String,
}

impl PackageLockfile {
    pub fn new(entries: Vec<PackageLockEntry>) -> Self {
        Self {
            version: LOCKFILE_VERSION,
            entries,
        }
    }

    pub fn identity_hash(&self) -> String {
        let mut hasher = DefaultHasher::new();
        self.version.hash(&mut hasher);
        for entry in &self.entries {
            entry.hash(&mut hasher);
        }
        format!("{:016x}", hasher.finish())
    }
}

impl Hash for PackageLockEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.alias.hash(state);
        self.source_kind.hash(state);
        self.locator.hash(state);
        self.selected_revision.hash(state);
        self.materialized_root.hash(state);
    }
}

pub fn render_package_lockfile(lockfile: &PackageLockfile) -> String {
    let mut rendered = format!("version: {}\n", lockfile.version);
    for entry in &lockfile.entries {
        rendered.push_str("- alias: ");
        rendered.push_str(&entry.alias);
        rendered.push('\n');
        rendered.push_str("  source: ");
        rendered.push_str(source_kind_label(entry.source_kind));
        rendered.push('\n');
        rendered.push_str("  locator: ");
        rendered.push_str(&entry.locator);
        rendered.push('\n');
        rendered.push_str("  revision: ");
        rendered.push_str(&entry.selected_revision);
        rendered.push('\n');
        rendered.push_str("  root: ");
        rendered.push_str(&entry.materialized_root);
        rendered.push('\n');
    }
    rendered
}

pub fn parse_package_lockfile(raw: &str) -> Result<PackageLockfile, PackageError> {
    let mut version = None;
    let mut entries = Vec::new();
    let mut current = PendingLockEntry::default();

    for line in raw.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("version: ") {
            version = Some(rest.trim().parse::<u32>().map_err(|_| {
                PackageError::new(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "package lockfile version '{}' is not a valid integer",
                        rest.trim()
                    ),
                )
            })?);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- alias: ") {
            if current.is_complete() {
                entries.push(current.finish()?);
                current = PendingLockEntry::default();
            } else if current.has_any_field() {
                return Err(PackageError::new(
                    PackageErrorKind::InvalidInput,
                    "package lockfile entry started before the previous entry was complete",
                ));
            }
            current.alias = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("  source: ") {
            current.source_kind = Some(parse_source_kind(rest.trim())?);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("  locator: ") {
            current.locator = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("  revision: ") {
            current.selected_revision = Some(rest.trim().to_string());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("  root: ") {
            current.materialized_root = Some(rest.trim().to_string());
            continue;
        }
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package lockfile contains an unrecognized line '{}'",
                trimmed
            ),
        ));
    }

    if current.has_any_field() {
        entries.push(current.finish()?);
    }

    let version = version.ok_or_else(|| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            "package lockfile is missing required version header",
        )
    })?;

    if version != LOCKFILE_VERSION {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package lockfile version {} is not compatible with supported version {}",
                version, LOCKFILE_VERSION
            ),
        ));
    }

    Ok(PackageLockfile { version, entries })
}

#[derive(Default)]
struct PendingLockEntry {
    alias: Option<String>,
    source_kind: Option<PackageDependencySourceKind>,
    locator: Option<String>,
    selected_revision: Option<String>,
    materialized_root: Option<String>,
}

impl PendingLockEntry {
    fn has_any_field(&self) -> bool {
        self.alias.is_some()
            || self.source_kind.is_some()
            || self.locator.is_some()
            || self.selected_revision.is_some()
            || self.materialized_root.is_some()
    }

    fn is_complete(&self) -> bool {
        self.alias.is_some()
            && self.source_kind.is_some()
            && self.locator.is_some()
            && self.selected_revision.is_some()
            && self.materialized_root.is_some()
    }

    fn finish(self) -> Result<PackageLockEntry, PackageError> {
        Ok(PackageLockEntry {
            alias: self.alias.ok_or_else(missing_entry_field("alias"))?,
            source_kind: self.source_kind.ok_or_else(missing_entry_field("source"))?,
            locator: self.locator.ok_or_else(missing_entry_field("locator"))?,
            selected_revision: self
                .selected_revision
                .ok_or_else(missing_entry_field("revision"))?,
            materialized_root: self
                .materialized_root
                .ok_or_else(missing_entry_field("root"))?,
        })
    }
}

fn missing_entry_field(field: &'static str) -> impl FnOnce() -> PackageError {
    move || {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!("package lockfile entry is missing required field '{field}'"),
        )
    }
}

fn source_kind_label(kind: PackageDependencySourceKind) -> &'static str {
    match kind {
        PackageDependencySourceKind::Local => "loc",
        PackageDependencySourceKind::PackageStore => "pkg",
        PackageDependencySourceKind::Git => "git",
        PackageDependencySourceKind::Internal => "internal",
    }
}

fn parse_source_kind(raw: &str) -> Result<PackageDependencySourceKind, PackageError> {
    match raw {
        "loc" => Ok(PackageDependencySourceKind::Local),
        "pkg" => Ok(PackageDependencySourceKind::PackageStore),
        "git" => Ok(PackageDependencySourceKind::Git),
        "internal" => Ok(PackageDependencySourceKind::Internal),
        _ => Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!("package lockfile uses unsupported source kind '{raw}'"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_package_lockfile, render_package_lockfile, PackageLockEntry, PackageLockfile,
    };
    use crate::PackageDependencySourceKind;

    fn sample_lockfile() -> PackageLockfile {
        PackageLockfile::new(vec![PackageLockEntry {
            alias: "logtiny".to_string(),
            source_kind: PackageDependencySourceKind::Git,
            locator: "https://github.com/bresilla/logtiny.git".to_string(),
            selected_revision: "abc123def456".to_string(),
            materialized_root: ".fol/pkg/git/github.com/bresilla/logtiny/rev_abc123def456"
                .to_string(),
        }])
    }

    #[test]
    fn package_lockfile_renders_stably() {
        let rendered = render_package_lockfile(&sample_lockfile());

        assert_eq!(
            rendered,
            "version: 1\n- alias: logtiny\n  source: git\n  locator: https://github.com/bresilla/logtiny.git\n  revision: abc123def456\n  root: .fol/pkg/git/github.com/bresilla/logtiny/rev_abc123def456\n"
        );
    }

    #[test]
    fn package_lockfile_roundtrips_through_text_format() {
        let lockfile = sample_lockfile();
        let reparsed = parse_package_lockfile(&render_package_lockfile(&lockfile))
            .expect("lockfile should roundtrip");

        assert_eq!(reparsed, lockfile);
    }

    #[test]
    fn package_lockfile_identity_hash_is_stable_for_identical_content() {
        let left = sample_lockfile();
        let right = sample_lockfile();

        assert_eq!(left.identity_hash(), right.identity_hash());
    }

    #[test]
    fn package_lockfile_rejects_incompatible_version() {
        let error = parse_package_lockfile("version: 99\n")
            .expect_err("incompatible lockfile version should fail");

        assert!(error.message().contains("not compatible with supported version"));
    }

    #[test]
    fn package_lockfile_rejects_missing_fields() {
        let error = parse_package_lockfile(
            "version: 1\n- alias: logtiny\n  source: git\n  locator: https://github.com/bresilla/logtiny.git\n  revision: abc123\n",
        )
        .expect_err("missing fields should fail");

        assert!(error.message().contains("missing required field 'root'"));
    }

    #[test]
    fn package_lockfile_keeps_structured_git_locator_strings() {
        let lockfile = PackageLockfile::new(vec![PackageLockEntry {
            alias: "logtiny".to_string(),
            source_kind: PackageDependencySourceKind::Git,
            locator: "git+https://github.com/bresilla/logtiny.git#tag:v0.1.2#hash:f49abfa1038f"
                .to_string(),
            selected_revision: "f49abfa1038f0f5a3a2e09b9b3dd533182551006".to_string(),
            materialized_root: ".fol/pkg/git/github.com/bresilla/logtiny/rev_f49abfa1038f0f5a3a2e09b9b3dd533182551006"
                .to_string(),
        }]);

        let rendered = render_package_lockfile(&lockfile);
        let reparsed = parse_package_lockfile(&rendered)
            .expect("structured git lockfile locator should roundtrip");

        assert!(rendered.contains("#tag:v0.1.2#hash:f49abfa1038f"));
        assert_eq!(reparsed, lockfile);
    }
}

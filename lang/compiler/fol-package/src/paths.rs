use crate::PackageLocator;
use std::path::{Path, PathBuf};

pub fn git_cache_path(root: &Path, locator: &PackageLocator) -> PathBuf {
    let identity = locator
        .normalized_git_identity()
        .expect("git cache paths require git locators");
    let mut path = root.to_path_buf();
    let mut segments = identity.split('/').collect::<Vec<_>>();
    let repo = segments
        .pop()
        .expect("normalized git identities should have a repository segment");
    for segment in segments {
        path.push(segment);
    }
    path.push(format!("{repo}.git"));
    path
}

pub fn git_store_path(root: &Path, locator: &PackageLocator, revision: &str) -> PathBuf {
    let identity = locator
        .normalized_git_identity()
        .expect("git store paths require git locators");
    let mut path = root.to_path_buf();
    let mut segments = identity.split('/').collect::<Vec<_>>();
    let repo = segments
        .pop()
        .expect("normalized git identities should have a repository segment");
    for segment in segments {
        path.push(segment);
    }
    path.push(repo.trim_end_matches(".git"));
    path.push(format!("rev_{}", sanitize_revision_segment(revision)));
    path
}

fn sanitize_revision_segment(revision: &str) -> String {
    revision
        .trim()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{git_cache_path, git_store_path};
    use crate::{parse_package_locator, PackageLocatorKind};
    use std::path::PathBuf;

    #[test]
    fn git_cache_paths_are_derived_from_normalized_locator_identity() {
        let root = PathBuf::from("/tmp/demo/.fol/cache/git");
        let locator = parse_package_locator("git@GitHub.com:bresilla/logtiny.git")
            .expect("git locator should parse");

        assert_eq!(locator.kind, PackageLocatorKind::Git);
        assert_eq!(
            git_cache_path(&root, &locator),
            PathBuf::from("/tmp/demo/.fol/cache/git/github.com/bresilla/logtiny.git")
        );
    }

    #[test]
    fn git_store_paths_use_revision_aware_materialized_roots() {
        let root = PathBuf::from("/tmp/demo/.fol/pkg/git");
        let locator = parse_package_locator("https://github.com/bresilla/logtiny.git")
            .expect("git locator should parse");

        assert_eq!(locator.kind, PackageLocatorKind::Git);
        assert_eq!(
            git_store_path(&root, &locator, "abc123def456"),
            PathBuf::from("/tmp/demo/.fol/pkg/git/github.com/bresilla/logtiny/rev_abc123def456")
        );
    }

    #[test]
    fn git_store_paths_sanitize_revision_segments() {
        let root = PathBuf::from("/tmp/demo/.fol/pkg/git");
        let locator = parse_package_locator("git@GitHub.com:bresilla/logtiny.git")
            .expect("git locator should parse");

        assert_eq!(
            git_store_path(&root, &locator, "refs/tags/v1.0.0"),
            PathBuf::from(
                "/tmp/demo/.fol/pkg/git/github.com/bresilla/logtiny/rev_refs_tags_v1_0_0"
            )
        );
    }

    #[test]
    fn sanitize_revision_segment_blocks_path_traversal_vectors() {
        use super::sanitize_revision_segment;

        // forward-slash traversal
        assert_eq!(sanitize_revision_segment("../evil"), "___evil");
        // backslash traversal
        assert_eq!(sanitize_revision_segment("..\\evil"), "___evil");
        // null byte
        assert_eq!(sanitize_revision_segment("abc\x00def"), "abc_def");
        // combined traversal
        assert_eq!(
            sanitize_revision_segment("../../etc/passwd"),
            "______etc_passwd"
        );
    }
}

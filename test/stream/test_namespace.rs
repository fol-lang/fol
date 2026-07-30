// Tests for namespace functionality in fol-stream

use fol_stream::{Source, SourceType};

#[cfg(test)]
mod namespace_tests {
    use super::*;
    use std::fs;

    fn unique_temp_root(label: &str) -> crate::fixture::TempFixture {
        crate::fixture::TempFixture::new(&format!("fol_stream_{label}"))
    }

    #[test]
    fn test_package_name_detection() {
        // Test that package name comes from the explicit folder entry root.
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create sources with namespace");

        assert!(!sources.is_empty(), "Should have sources");

        // All sources should have the same package name
        let package_name = &sources[0].package;
        assert_eq!(
            package_name, "main",
            "Package name should come from the folder entry root"
        );

        for source in &sources {
            assert_eq!(
                source.package, *package_name,
                "All sources should have same package name"
            );
        }

        println!("Detected package name: {}", package_name);
    }

    #[test]
    fn test_root_namespace() {
        // Test files in root directory get root namespace (package name)
        let sources = Source::init("test/stream/fixture/main/main.fol", SourceType::File)
            .expect("Should create source");

        assert_eq!(sources.len(), 1, "Should have one source");
        let source = &sources[0];

        assert_eq!(source.package, "main", "Package should be 'main'");
        assert_eq!(
            source.namespace, "main",
            "Root file should have root namespace"
        );

        println!("Root file namespace: {}", source.namespace);
    }

    #[test]
    fn test_subdirectory_namespace() {
        // Test files in subdirectories get proper namespace
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create sources");

        // Find sources in subdirectories
        let subdir_sources: Vec<_> = sources
            .iter()
            .filter(|s| s.namespace != "main") // Not root namespace
            .collect();

        assert!(
            !subdir_sources.is_empty(),
            "Should have sources in subdirectories"
        );

        for source in &subdir_sources {
            println!("File: {} -> Namespace: {}", source.path, source.namespace);

            // Namespace should start with package name
            assert!(
                source.namespace.starts_with("main::"),
                "Namespace should start with package name: {}",
                source.namespace
            );

            // Should contain directory structure
            if source.path.contains("/var/") {
                assert!(
                    source.namespace.contains("var"),
                    "Should contain 'var' in namespace: {}",
                    source.namespace
                );
            }

            if source.path.contains("/single/") {
                assert!(
                    source.namespace.contains("single"),
                    "Should contain 'single' in namespace: {}",
                    source.namespace
                );
            }
        }
    }

    #[test]
    fn test_namespace_structure_mapping() {
        // Test specific directory to namespace mappings

        // Create a test directory structure for precise testing
        use std::fs;

        let test_dir = "test/stream/namespace_test";
        if !std::path::Path::new(test_dir).exists() {
            fs::create_dir_all(format!("{}/math/vector", test_dir)).ok();
            fs::create_dir_all(format!("{}/utils", test_dir)).ok();

            // Create test files
            fs::write(format!("{}/main.fol", test_dir), "// Root file").ok();
            fs::write(format!("{}/math/mod.fol", test_dir), "// Math module").ok();
            fs::write(
                format!("{}/math/vector/vec3.fol", test_dir),
                "// Vector3 implementation",
            )
            .ok();
            fs::write(
                format!("{}/utils/helper.fol", test_dir),
                "// Utility functions",
            )
            .ok();
        }

        let sources = Source::init_with_package(test_dir, SourceType::Folder, "myproject")
            .expect("Should create sources");

        // Verify namespace mappings
        for source in &sources {
            let filename = std::path::Path::new(&source.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            match filename {
                "main.fol" => {
                    assert_eq!(
                        source.namespace, "myproject",
                        "Root file should have root namespace"
                    );
                }
                "mod.fol" => {
                    assert_eq!(
                        source.namespace, "myproject::math",
                        "Math module should be in math namespace"
                    );
                }
                "vec3.fol" => {
                    assert_eq!(
                        source.namespace, "myproject::math::vector",
                        "Vector file should be in math::vector namespace"
                    );
                }
                "helper.fol" => {
                    assert_eq!(
                        source.namespace, "myproject::utils",
                        "Helper should be in utils namespace"
                    );
                }
                _ => {}
            }

            println!("File: {} -> Namespace: {}", filename, source.namespace);
        }

        // Clean up
        fs::remove_dir_all(test_dir).ok();
    }

    #[test]
    fn test_mod_directories_excluded_from_namespace() {
        // Test that .mod directories don't appear in namespaces
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create sources");

        // No namespace should contain ".mod"
        for source in &sources {
            assert!(
                !source.namespace.contains(".mod"),
                "Namespace should not contain .mod: {}",
                source.namespace
            );

            // But the files from .mod directories should not be present at all
            assert!(
                !source.path.contains(".mod/"),
                "Should not have files from .mod directories: {}",
                source.path
            );
        }

        println!("All namespaces are clean of .mod references");
    }

    #[test]
    fn test_namespace_consistency() {
        // Test that all files in the same directory have the same namespace
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create sources");

        use std::collections::HashMap;
        let mut dir_to_namespace: HashMap<String, String> = HashMap::new();

        for source in &sources {
            let dir = std::path::Path::new(&source.path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .to_string_lossy()
                .to_string();

            if let Some(existing_namespace) = dir_to_namespace.get(&dir) {
                assert_eq!(
                    existing_namespace, &source.namespace,
                    "All files in same directory should have same namespace: {} vs {} in {}",
                    existing_namespace, source.namespace, dir
                );
            } else {
                dir_to_namespace.insert(dir, source.namespace.clone());
            }
        }

        println!("Directory to namespace mappings:");
        for (dir, namespace) in &dir_to_namespace {
            println!("  {} -> {}", dir, namespace);
        }
    }

    #[test]
    fn test_explicit_package_name() {
        // Test using explicit package name
        let sources = Source::init_with_package(
            "test/stream/fixture/main/main.fol",
            SourceType::File,
            "custom_package",
        )
        .expect("Should create source with custom package");

        assert_eq!(sources.len(), 1, "Should have one source");
        let source = &sources[0];

        assert_eq!(
            source.package, "custom_package",
            "Should use explicit package name"
        );
        assert_eq!(
            source.namespace, "custom_package",
            "Should use custom package as root namespace"
        );
    }

    #[test]
    fn test_detached_folder_falls_back_to_folder_name() {
        let temp_root = unique_temp_root("detached_folder_package");
        fs::create_dir_all(&temp_root).expect("Should create detached temp directory");
        fs::write(temp_root.join("main.fol"), "var answer = 42")
            .expect("Should create detached fol file");

        let sources = Source::init(
            temp_root
                .to_str()
                .expect("Detached temp directory should be utf-8"),
            SourceType::Folder,
        )
        .expect("Detached folder should still produce sources");

        assert_eq!(sources.len(), 1, "Detached folder should expose one source");
        assert_eq!(
            sources[0].package,
            temp_root
                .file_name()
                .and_then(|name| name.to_str())
                .expect("Detached temp directory should have a folder name"),
            "Detached folder package name should come from the folder itself"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_detached_file_falls_back_to_parent_folder_name() {
        let temp_root = unique_temp_root("detached_file_package");
        fs::create_dir_all(&temp_root).expect("Should create detached temp directory");
        let file_path = temp_root.join("main.fol");
        fs::write(&file_path, "var answer = 42").expect("Should create detached fol file");

        let sources = Source::init(
            file_path
                .to_str()
                .expect("Detached temp file path should be utf-8"),
            SourceType::File,
        )
        .expect("Detached file should still produce a source");

        assert_eq!(sources.len(), 1, "Detached file should expose one source");
        assert_eq!(
            sources[0].package,
            temp_root
                .file_name()
                .and_then(|name| name.to_str())
                .expect("Detached temp directory should have a folder name"),
            "Detached file package name should come from the parent folder"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_detached_folder_with_invalid_name_is_rejected_as_package() {
        let temp_root = unique_temp_root("invalid_folder_package");
        let invalid_root = temp_root.join("bad-dir");
        fs::create_dir_all(&invalid_root).expect("Should create invalid package directory");
        fs::write(invalid_root.join("main.fol"), "var answer = 42")
            .expect("Should create detached fol file");

        let result = Source::init(
            invalid_root
                .to_str()
                .expect("Invalid package directory should be utf-8"),
            SourceType::Folder,
        );

        assert!(
            result.is_err(),
            "Invalid derived package names should be rejected"
        );
        let error = format!(
            "{}",
            result.expect_err("Invalid package root should fail source discovery")
        );
        assert!(
            error.contains("Invalid package name 'bad-dir'"),
            "Package validation error should mention the invalid derived folder name: {}",
            error
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_detached_file_with_invalid_parent_name_is_rejected_as_package() {
        let temp_root = unique_temp_root("invalid_file_package");
        let invalid_root = temp_root.join("123bad");
        fs::create_dir_all(&invalid_root).expect("Should create invalid parent directory");
        let file_path = invalid_root.join("main.fol");
        fs::write(&file_path, "var answer = 42").expect("Should create detached fol file");

        let result = Source::init(
            file_path
                .to_str()
                .expect("Invalid package file path should be utf-8"),
            SourceType::File,
        );

        assert!(
            result.is_err(),
            "Invalid parent-derived package names should be rejected"
        );
        let error = format!(
            "{}",
            result.expect_err("Invalid parent package root should fail source discovery")
        );
        assert!(
            error.contains("Invalid package name '123bad'"),
            "Package validation error should mention the invalid derived parent folder name: {}",
            error
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_source_identity_uses_canonical_path_and_preserves_call_site() {
        let file_sources = Source::init("test/stream/fixture/main/main.fol", SourceType::File)
            .expect("Should create file-backed source");
        let folder_sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create folder-backed sources");
        let folder_main = folder_sources
            .iter()
            .find(|source| source.path.ends_with("test/stream/fixture/main/main.fol"))
            .expect("Folder-backed sources should include main.fol");
        let file_main = &file_sources[0];
        let canonical = std::fs::canonicalize("test/stream/fixture/main/main.fol")
            .expect("Should canonicalize main.fol")
            .to_string_lossy()
            .to_string();

        assert_eq!(file_main.call, "test/stream/fixture/main/main.fol");
        assert_eq!(folder_main.call, "test/stream/fixture/main");
        assert_eq!(
            file_main.path, canonical,
            "File source path should be canonical"
        );
        assert_eq!(
            folder_main.path, canonical,
            "Folder and file source discovery should agree on canonical identity"
        );
        assert_eq!(
            file_main.namespace, folder_main.namespace,
            "The same physical file should keep the same namespace"
        );
        assert_eq!(
            file_main.package, folder_main.package,
            "The same physical file should keep the same package"
        );
    }

    #[test]
    fn test_source_identity_excludes_call_site_and_matches_stream_contract() {
        let file_sources = Source::init("test/stream/fixture/main/main.fol", SourceType::File)
            .expect("Should create file-backed source");
        let folder_sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create folder-backed sources");
        let folder_main = folder_sources
            .iter()
            .find(|source| source.path.ends_with("test/stream/fixture/main/main.fol"))
            .expect("Folder-backed sources should include main.fol");
        let file_main = &file_sources[0];

        assert_ne!(
            file_main.call, folder_main.call,
            "Call site should preserve how discovery started"
        );
        assert_eq!(
            file_main.identity(),
            folder_main.identity(),
            "Stream identity should be canonical path plus package and namespace, not the original call path"
        );
    }

    #[test]
    fn test_source_identity_keeps_raw_call_spelling_separate_from_canonical_identity() {
        let direct = Source::init("test/stream/fixture/main/main.fol", SourceType::File)
            .expect("Should create direct file-backed source");
        let dotted = Source::init("./test/stream/fixture/main/main.fol", SourceType::File)
            .expect("Should create dotted file-backed source");

        assert_eq!(direct.len(), 1);
        assert_eq!(dotted.len(), 1);
        assert_ne!(
            direct[0].call, dotted[0].call,
            "Raw call spelling should preserve how the entry path was provided"
        );
        assert_eq!(
            direct[0].path, dotted[0].path,
            "Canonical source paths should collapse path-spelling aliases"
        );
        assert_eq!(
            direct[0].identity(),
            dotted[0].identity(),
            "Logical source identity should use canonical path plus package and namespace rather than raw call spelling"
        );
    }

    #[test]
    fn test_source_identity_changes_when_canonical_entry_path_changes() {
        let temp_root = unique_temp_root("renamed_identity");
        let before_dir = temp_root.join("entry_before");
        let before_file = before_dir.join("main.fol");
        let after_dir = temp_root.join("entry_after");
        let after_file = after_dir.join("main.fol");

        fs::create_dir_all(&before_dir).expect("Should create initial entry folder");
        fs::write(&before_file, "fun main() => 1\n").expect("Should write initial entry file");

        let before = Source::init_with_package(
            before_file
                .to_str()
                .expect("Initial file path should be UTF-8"),
            SourceType::File,
            "fixed_pkg",
        )
        .expect("Should create initial explicit-package source");

        fs::rename(&before_dir, &after_dir).expect("Should rename entry folder");

        let after = Source::init_with_package(
            after_file
                .to_str()
                .expect("Renamed file path should be UTF-8"),
            SourceType::File,
            "fixed_pkg",
        )
        .expect("Should create renamed explicit-package source");

        assert_eq!(before.len(), 1);
        assert_eq!(after.len(), 1);
        assert_eq!(
            before[0].package, after[0].package,
            "Explicit package overrides should isolate this test to path identity"
        );
        assert_eq!(
            before[0].namespace, after[0].namespace,
            "Explicit package overrides should keep namespace stable while canonical path changes"
        );
        assert_ne!(
            before[0].path, after[0].path,
            "Renaming the entry path should change the canonical file path"
        );
        assert_ne!(
            before[0].identity(),
            after[0].identity(),
            "Logical identity should change when the canonical file path changes even if package and namespace stay fixed"
        );

        fs::remove_dir_all(&temp_root).expect("Should clean up renamed identity temp root");
    }

    #[test]
    fn test_invalid_namespace_components_are_rejected_during_source_discovery() {
        let temp_root = unique_temp_root("namespace_components");
        let cases = [
            ("leading_digit", "good/123bad", "123bad"),
            ("dotted", "bad.dir", "bad.dir"),
            ("hyphenated", "bad-dir", "bad-dir"),
            ("repeated_underscore", "bad__name", "bad__name"),
        ];

        for (label, relative_dir, invalid_component) in cases {
            let case_root = temp_root.join(label);
            fs::create_dir_all(case_root.join(relative_dir))
                .expect("Should create invalid namespace directory");
            fs::write(
                case_root.join(relative_dir).join("value.fol"),
                "var value = 1",
            )
            .expect("Should write invalid namespace source");

            let result = Source::init_with_package(
                case_root.to_str().expect("Case root should be utf-8"),
                SourceType::Folder,
                "pkg",
            );

            assert!(
                result.is_err(),
                "Invalid namespace component '{}' should fail source discovery",
                invalid_component
            );
            let error = format!(
                "{}",
                result.expect_err("Invalid namespace component should be rejected")
            );
            assert!(
                error.contains(&format!(
                    "Invalid namespace component '{}'",
                    invalid_component
                )),
                "Namespace validation error should mention the offending component: {}",
                error
            );
        }

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_valid_namespace_components_allow_underscores_and_nonleading_digits() {
        let temp_root = unique_temp_root("namespace_valid_components");
        fs::create_dir_all(temp_root.join("good_2/more3")).expect("Should create valid dirs");
        fs::write(temp_root.join("good_2/more3/value.fol"), "var valid = 1")
            .expect("Should write nested file");

        let sources = Source::init_with_package(
            temp_root.to_str().expect("Temp root should be utf-8"),
            SourceType::Folder,
            "pkg",
        )
        .expect("Should create sources from valid namespace root");

        let valid = sources
            .iter()
            .find(|source| source.path.ends_with("good_2/more3/value.fol"))
            .expect("Should include valid-component file");

        assert_eq!(valid.namespace, "pkg::good_2::more3");

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_non_ascii_namespace_components_are_rejected() {
        let temp_root = unique_temp_root("namespace_non_ascii_components");
        fs::create_dir_all(temp_root.join("ascii/café"))
            .expect("Should create mixed ascii/unicode dirs");
        fs::write(temp_root.join("ascii/café/value.fol"), "var unicode = 1")
            .expect("Should write unicode-path file");

        let result = Source::init_with_package(
            temp_root.to_str().expect("Temp root should be utf-8"),
            SourceType::Folder,
            "pkg",
        );

        assert!(
            result.is_err(),
            "Non-ASCII namespace components should fail source discovery"
        );
        let error = format!(
            "{}",
            result.expect_err("Non-ASCII namespace component should be rejected")
        );
        assert!(
            error.contains("Invalid namespace component 'café'"),
            "Namespace validation error should mention the non-ASCII component: {}",
            error
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_mod_directories_do_not_contribute_sources_or_namespace_segments() {
        let temp_root = unique_temp_root("namespace_mod_interaction");
        fs::create_dir_all(temp_root.join("alpha")).expect("Should create regular dir");
        fs::create_dir_all(temp_root.join("alpha.mod/hidden"))
            .expect("Should create skipped .mod dir");

        fs::write(temp_root.join("alpha/value.fol"), "var visible = 1")
            .expect("Should write regular source");
        fs::write(
            temp_root.join("alpha.mod/hidden/value.fol"),
            "var hidden = 1",
        )
        .expect("Should write skipped source");

        let sources = Source::init_with_package(
            temp_root.to_str().expect("Temp root should be utf-8"),
            SourceType::Folder,
            "pkg",
        )
        .expect("Should create sources from temp root");

        assert_eq!(
            sources.len(),
            1,
            ".mod directories should be skipped before source collection"
        );
        assert!(sources[0].path.ends_with("alpha/value.fol"));
        assert_eq!(sources[0].namespace, "pkg::alpha");

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_explicit_package_override_changes_logical_identity_without_changing_path() {
        let pkg_a = Source::init_with_package(
            "test/stream/fixture/main/main.fol",
            SourceType::File,
            "alpha_pkg",
        )
        .expect("Should create source for alpha package");
        let pkg_b = Source::init_with_package(
            "test/stream/fixture/main/main.fol",
            SourceType::File,
            "beta_pkg",
        )
        .expect("Should create source for beta package");

        assert_eq!(pkg_a[0].path, pkg_b[0].path);
        assert_ne!(pkg_a[0].package, pkg_b[0].package);
        assert_ne!(pkg_a[0].namespace, pkg_b[0].namespace);
        assert_eq!(pkg_a[0].namespace, "alpha_pkg");
        assert_eq!(pkg_b[0].namespace, "beta_pkg");
    }

    #[test]
    fn test_invalid_explicit_package_override_is_rejected() {
        let result = Source::init_with_package(
            "test/stream/fixture/main/main.fol",
            SourceType::File,
            "bad-dir",
        );

        assert!(
            result.is_err(),
            "Invalid explicit package overrides should be rejected"
        );
        let error = format!(
            "{}",
            result.expect_err("Invalid explicit package override should fail")
        );
        assert!(
            error.contains("Invalid package name 'bad-dir'"),
            "Explicit package validation error should mention the invalid override: {}",
            error
        );
    }

    #[test]
    fn test_nested_manifest_folder_ignores_cargo_package_files() {
        let temp_root = unique_temp_root("nested_manifest_folder");
        let outer_dir = temp_root.join("outer");
        let inner_dir = outer_dir.join("inner");
        let input_dir = inner_dir.join("src");

        fs::create_dir_all(&input_dir).expect("Should create nested manifest tree");
        fs::write(
            outer_dir.join("Cargo.toml"),
            "[package]\nname = \"outer_pkg\"\n",
        )
        .expect("Should write outer manifest");
        fs::write(
            inner_dir.join("Cargo.toml"),
            "[package]\nname = \"inner_pkg\"\n",
        )
        .expect("Should write inner manifest");
        fs::write(input_dir.join("main.fol"), "var answer = 42").expect("Should write fol source");

        let sources = Source::init(
            input_dir.to_str().expect("Input directory should be utf-8"),
            SourceType::Folder,
        )
        .expect("Nested manifest folder should produce sources");

        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0].package, "src",
            "Folder entry should use its own name even when Cargo.toml files are present above it"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_nested_manifest_file_ignores_cargo_package_files() {
        let temp_root = unique_temp_root("nested_manifest_file");
        let outer_dir = temp_root.join("outer");
        let inner_dir = outer_dir.join("inner");
        let input_file = inner_dir.join("src/main.fol");

        fs::create_dir_all(input_file.parent().expect("Input file should have parent"))
            .expect("Should create nested manifest tree");
        fs::write(
            outer_dir.join("Cargo.toml"),
            "[package]\nname = \"outer_pkg\"\n",
        )
        .expect("Should write outer manifest");
        fs::write(
            inner_dir.join("Cargo.toml"),
            "[package]\nname = \"inner_pkg\"\n",
        )
        .expect("Should write inner manifest");
        fs::write(&input_file, "var answer = 42").expect("Should write fol source");

        let sources = Source::init(
            input_file.to_str().expect("Input file should be utf-8"),
            SourceType::File,
        )
        .expect("Nested manifest file should produce sources");

        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0].package, "src",
            "File entry should use its parent folder name even when Cargo.toml files are present above it"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_single_file_input_keeps_root_namespace_even_in_nested_folders() {
        let sources = Source::init(
            "test/stream/fixture/main/single/subpak/input1.fol",
            SourceType::File,
        )
        .expect("Should create single nested file source");

        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0].namespace, "subpak",
            "Single-file entry should keep the detached-file root namespace instead of inheriting higher folder segments"
        );
    }

    #[test]
    fn test_folder_input_uses_nested_namespace_segments_for_nested_files() {
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create sources");
        let nested = sources
            .iter()
            .find(|source| {
                source
                    .path
                    .ends_with("test/stream/fixture/main/single/subpak/input1.fol")
            })
            .expect("Folder input should include nested file");

        assert_eq!(
            nested.namespace, "main::single::subpak",
            "Folder input should derive namespace segments from nested directories"
        );
    }

    #[test]
    fn test_namespace_output_integration() {
        // Test that the namespace information is properly integrated
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should create sources");

        // Verify all expected properties are present
        for source in &sources {
            assert!(
                !source.namespace.is_empty(),
                "Namespace should not be empty"
            );
            assert!(!source.package.is_empty(), "Package should not be empty");
            assert!(!source.path.is_empty(), "Path should not be empty");

            // Namespace should be valid format
            assert!(
                source
                    .namespace
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == ':' || c == '_'),
                "Namespace should only contain valid characters: {}",
                source.namespace
            );

            // If namespace contains ::, it should have at least package::something
            if source.namespace.contains("::") {
                let parts: Vec<&str> = source.namespace.split("::").collect();
                assert!(
                    parts.len() >= 2,
                    "Namespaced item should have at least 2 parts"
                );
                assert_eq!(
                    parts[0], source.package,
                    "First part should be package name"
                );
            }
        }

        println!("All sources have valid namespace information");
    }
}

// Tests for sophisticated .mod directory handling in fol-stream

use fol_stream::{CharacterProvider, FileStream, Source, SourceType};
use std::fs;

#[cfg(test)]
mod mod_handling_tests {
    use super::*;

    #[test]
    fn test_mod_directory_skipping() {
        // Test that .mod directories are properly skipped during folder traversal
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should process test/stream/fixture/main directory");

        // Print all found sources for debugging
        println!("Found {} sources:", sources.len());
        for source in &sources {
            println!("  {}", source.path);
        }

        // Check that we found .fol files but no files from .mod directories
        assert!(!sources.is_empty(), "Should find some .fol files");

        // Verify no paths contain .mod directories
        for source in &sources {
            assert!(
                !source.path.contains(".mod/"),
                "Should not include files from .mod directories: {}",
                source.path
            );
        }

        // Should find main.fol and files from regular directories
        let has_main_fol = sources.iter().any(|s| s.path.ends_with("main.fol"));
        assert!(has_main_fol, "Should find main.fol in root");

        // Should find files from regular subdirectories but not .mod ones
        let has_subdir_files = sources
            .iter()
            .any(|s| s.path.contains("single/") || s.path.contains("var/"));
        assert!(
            has_subdir_files,
            "Should find files from non-.mod subdirectories"
        );
    }

    #[test]
    fn test_folder_traversal_order_is_deterministic() {
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should process test/stream/fixture/main directory");
        let root = std::fs::canonicalize("test/stream/fixture/main").expect("Should resolve test root");

        let relative_paths: Vec<String> = sources
            .iter()
            .map(|source| {
                std::path::Path::new(&source.path)
                    .strip_prefix(&root)
                    .expect("Source should stay under the root")
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        assert_eq!(
            relative_paths,
            vec![
                "main.fol",
                "single/input1.fol",
                "single/input2.fol",
                "single/subpak/input1.fol",
                "single/var.fol",
                "var/binding.fol",
                "var/var.fol",
                "var2/number_fol/file.fol",
                "var2/var.fol",
            ],
            "Folder traversal order should stay stable for downstream phases"
        );
    }

    #[test]
    fn test_dot_mod_directories_are_skipped_without_hiding_normal_directories() {
        let temp_root = std::env::temp_dir().join(format!(
            "fol_stream_mod_suffix_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time should be after unix epoch")
                .as_nanos()
        ));

        fs::create_dir_all(temp_root.join("alpha.mod/hidden"))
            .expect("Should create skipped .mod directory");
        fs::create_dir_all(temp_root.join("alpha_mod/visible"))
            .expect("Should create retained normal directory");
        fs::write(temp_root.join("root.fol"), "var root = 1").expect("Should write root source");
        fs::write(temp_root.join("alpha.mod/hidden/value.fol"), "var hidden = 2")
            .expect("Should write skipped .mod source");
        fs::write(temp_root.join("alpha_mod/visible/value.fol"), "var visible = 3")
            .expect("Should write retained normal-directory source");

        let sources = Source::init(
            temp_root
                .to_str()
                .expect("Temp root path should be valid UTF-8"),
            SourceType::Folder,
        )
        .expect("Should discover sources while applying exact .mod suffix filtering");

        assert!(
            sources.iter().all(|source| !source.path.contains("alpha.mod/")),
            "Directories ending with '.mod' should still be skipped"
        );
        assert!(
            sources
                .iter()
                .any(|source| source.path.contains("alpha_mod/visible/value.fol")),
            "Normal directories should still be traversed while '.mod' directories are skipped"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_folder_stream_creation() {
        // Test creating a stream from a folder with .mod directories
        let stream = FileStream::from_folder("test/stream/fixture/main")
            .expect("Should create stream from folder with .mod dirs");

        let sources = stream.sources();
        assert!(!sources.is_empty(), "Should have multiple sources");

        // Verify each source has content loaded
        for source in sources {
            assert!(
                !source.data.is_empty(),
                "Source should have data loaded: {}",
                source.path
            );
            assert!(!source.path.is_empty(), "Source should have valid path");
        }

        println!("Created stream with {} sources", sources.len());
    }

    #[test]
    fn test_from_sources_eagerly_loads_all_source_contents() {
        let sources = Source::init("test/stream/fixture/main", SourceType::Folder)
            .expect("Should discover sources for eager loading test");
        let stream = FileStream::from_sources(sources).expect("Should build stream from sources");

        assert!(
            stream.sources().iter().all(|source| !source.data.is_empty()),
            "FileStream::from_sources should eagerly load every source body before streaming starts"
        );
    }

    #[test]
    fn test_multi_file_stream_keeps_draining_after_backing_files_are_removed() {
        let temp_root = std::env::temp_dir().join(format!(
            "fol_stream_eager_loading_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time should be after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(temp_root.join("nested/deeper"))
            .expect("Should create nested temp folders");

        let file_specs = vec![
            (temp_root.join("main.fol"), "fun main() => 10\n"),
            (temp_root.join("alpha.fol"), "var[imu] alpha = 11\n"),
            (
                temp_root.join("nested/beta.fol"),
                "fun beta() {\n    return 12\n}\n",
            ),
            (
                temp_root.join("nested/deeper/gamma.fol"),
                "log gamma(flag: bool): bool => flag\n",
            ),
        ];

        let mut expected_total = 0usize;
        for (path, contents) in &file_specs {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("Should create temp file parent folders");
            }
            fs::write(path, contents).expect("Should write temp source file");
            expected_total += contents.chars().count();
        }

        let sources = Source::init(
            temp_root
                .to_str()
                .expect("Temp root should render as UTF-8"),
            SourceType::Folder,
        )
        .expect("Should discover temp eager-loading sources");
        let mut stream =
            FileStream::from_sources(sources).expect("Should build eager stream from temp sources");

        fs::remove_dir_all(&temp_root)
            .expect("Should remove backing files after stream construction");

        let mut drained = String::new();
        while let Some((ch, _loc)) = stream.next_char() {
            drained.push(ch);
        }

        assert_eq!(
            drained.chars().count(),
            expected_total,
            "Eager source loading should preserve the full multi-file stream even after the backing files disappear"
        );
        assert!(
            drained.contains("fun main() => 10")
                && drained.contains("var[imu] alpha = 11")
                && drained.contains("fun beta()")
                && drained.contains("log gamma(flag: bool): bool => flag"),
            "The drained stream should still contain content from every eagerly loaded file"
        );
    }

    #[test]
    fn test_multi_source_character_streaming() {
        // Test that character streaming works across multiple sources
        let mut stream =
            FileStream::from_folder("test/stream/fixture/main").expect("Should create multi-source stream");

        let mut char_count = 0;
        let mut file_switches = 0;
        let mut current_file: Option<String> = None;

        // Stream characters and track file transitions
        while let Some((_ch, loc)) = stream.next_char() {
            char_count += 1;

            if current_file.as_ref() != loc.file.as_ref() {
                if current_file.is_some() {
                    file_switches += 1;
                }
                current_file = loc.file.clone();
                if let Some(ref file) = current_file {
                    println!("Switched to file: {}", file);
                }
            }

            // Verify location tracking
            assert!(loc.row >= 1, "Row should be at least 1");
            assert!(loc.col >= 1, "Column should be at least 1");

        }

        assert!(char_count > 100, "Should stream substantial content");
        assert!(file_switches > 0, "Should switch between multiple files");

        println!(
            "Streamed {} characters across {} file switches",
            char_count, file_switches
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_folder_traversal_propagates_recursive_directory_read_failures() {
        use std::os::unix::fs::PermissionsExt;

        let temp_root = std::env::temp_dir().join(format!(
            "fol_stream_recursive_read_error_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time should be after unix epoch")
                .as_nanos()
        ));
        let blocked_dir = temp_root.join("blocked");
        let root_file = temp_root.join("main.fol");

        fs::create_dir_all(&blocked_dir).expect("Should create blocked temp directory");
        fs::write(&root_file, "fun main() => 1\n").expect("Should write readable root source");

        let mut permissions = fs::metadata(&blocked_dir)
            .expect("Should read blocked directory metadata")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&blocked_dir, permissions)
            .expect("Should remove blocked directory permissions");

        let result = Source::init(
            temp_root
                .to_str()
                .expect("Blocked temp root should be valid UTF-8"),
            SourceType::Folder,
        );

        let mut cleanup_permissions = fs::metadata(&blocked_dir)
            .expect("Should re-read blocked directory metadata")
            .permissions();
        cleanup_permissions.set_mode(0o755);
        fs::set_permissions(&blocked_dir, cleanup_permissions)
            .expect("Should restore blocked directory permissions for cleanup");
        fs::remove_dir_all(&temp_root).ok();

        assert!(
            result.is_err(),
            "Folder traversal should report unreadable nested directories instead of silently dropping them"
        );
    }

    #[test]
    fn test_mod_directory_contents_verification() {
        // Verify that .mod directories exist and contain expected mixed content

        // Check main.mod exists and has mixed file types
        let main_mod_path = "test/stream/fixture/main/main.mod";
        assert!(
            std::path::Path::new(main_mod_path).exists(),
            "main.mod directory should exist"
        );

        let main_mod_files = fs::read_dir(main_mod_path)
            .expect("Should read main.mod directory")
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        println!("Files in main.mod: {:?}", main_mod_files);

        // Should contain mixed file types
        let has_fol = main_mod_files.iter().any(|f| f.ends_with(".fol"));
        let has_other = main_mod_files.iter().any(|f| !f.ends_with(".fol"));

        assert!(has_fol, "main.mod should contain .fol files");
        assert!(
            has_other,
            "main.mod should contain non-.fol files (mixed content)"
        );
    }

    #[test]
    fn test_source_path_methods() {
        // Test Source path manipulation methods
        let sources = Source::init("test/stream/fixture/main/main.fol", SourceType::File)
            .expect("Should create source from file");

        assert_eq!(sources.len(), 1, "Should have one source");
        let source = &sources[0];

        // Test path methods
        let abs_path = source.path(true);
        let rel_path = source.path(false);
        let module = source.module();

        assert!(
            abs_path.contains("main.fol"),
            "Absolute path should contain filename"
        );
        assert!(!abs_path.is_empty(), "Absolute path should not be empty");

        println!("Absolute path: {}", abs_path);
        println!("Relative path: {}", rel_path);
        println!("Module: {}", module);
    }

    #[test]
    fn test_empty_directory_handling() {
        // Test behavior with directory that has no .fol files (only .mod dirs)

        // Create a test directory structure
        let test_dir = "test/stream/test_empty_with_mod";
        if !std::path::Path::new(test_dir).exists() {
            fs::create_dir_all(test_dir).expect("Should create test directory");
            fs::create_dir_all(format!("{}/empty.mod", test_dir)).expect("Should create .mod dir");
            fs::write(format!("{}/empty.mod/test.txt", test_dir), "test content")
                .expect("Should create file in .mod dir");
        }

        // Should fail to create sources since no .fol files found
        let result = Source::init(test_dir, SourceType::Folder);
        assert!(result.is_err(), "Should fail when no .fol files found");

        // Clean up
        fs::remove_dir_all(test_dir).ok();
    }

    #[test]
    fn test_sophisticated_stream_features() {
        // Test advanced features of the sophisticated stream implementation
        let stream =
            FileStream::from_folder("test/stream/fixture/main").expect("Should create sophisticated stream");

        // Test current_source method
        let current = stream.current_source();
        assert!(current.is_some(), "Should have current source");

        if let Some(source) = current {
            assert!(!source.call.is_empty(), "Should have call path");
            assert!(!source.path.is_empty(), "Should have file path");
            assert!(!source.data.is_empty(), "Should have file data");
        }

        // Test sources method
        let all_sources = stream.sources();
        assert!(!all_sources.is_empty(), "Should have multiple sources");

        // Verify all sources are .fol files
        for source in all_sources {
            assert!(
                source.path.ends_with(".fol"),
                "All sources should be .fol files: {}",
                source.path
            );
        }
    }
}

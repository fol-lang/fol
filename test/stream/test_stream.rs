// Comprehensive tests for fol-stream module

use fol_stream::{CharacterProvider, FileStream};

// Include sophisticated .mod handling tests
mod test_mod_handling;

// Include namespace tests
mod test_namespace;

#[cfg(test)]
mod stream_tests {
    use super::*;

    #[test]
    fn test_basic_file_reading() {
        let mut stream = FileStream::from_file("test/stream/basic.fol")
            .expect("Should be able to read basic.fol");

        let expected = "hello world".chars().collect::<Vec<_>>();
        let mut actual = Vec::new();

        while let Some((ch, _loc)) = stream.next_char() {
            actual.push(ch);
        }

        assert_eq!(actual, expected, "Should read file contents correctly");
    }

    #[test]
    fn test_location_tracking_single_line() {
        let mut stream = FileStream::from_file("test/stream/basic.fol")
            .expect("Should be able to read basic.fol");

        // Test first character location
        if let Some((ch, loc)) = stream.next_char() {
            assert_eq!(ch, 'h');
            assert_eq!(loc.row, 1);
            assert_eq!(loc.col, 1);
            let file = loc.file.expect("Location should include a file path");
            assert!(
                file.ends_with("test/stream/basic.fol"),
                "Unexpected file path: {}",
                file
            );
        } else {
            panic!("Should have at least one character");
        }

        // Test a few more characters
        if let Some((ch, loc)) = stream.next_char() {
            assert_eq!(ch, 'e');
            assert_eq!(loc.row, 1);
            assert_eq!(loc.col, 2);
        }

        // Skip to space
        for _ in 0..3 {
            stream.next_char();
        } // l, l, o
        if let Some((ch, loc)) = stream.next_char() {
            assert_eq!(ch, ' ');
            assert_eq!(loc.row, 1);
            assert_eq!(loc.col, 6);
        }
    }

    #[test]
    fn test_multiline_location_tracking() {
        let mut stream = FileStream::from_file("test/stream/multiline.fol")
            .expect("Should be able to read multiline.fol");

        let mut chars_and_locations = Vec::new();
        while let Some((ch, loc)) = stream.next_char() {
            chars_and_locations.push((ch, loc.clone()));
        }

        // Verify we have the right content
        let chars: String = chars_and_locations.iter().map(|(ch, _)| *ch).collect();
        assert_eq!(chars, "line 1\nline 2\nline 3");

        // Check specific locations
        // First character of line 1
        assert_eq!(chars_and_locations[0].1.row, 1);
        assert_eq!(chars_and_locations[0].1.col, 1);

        // Newline at end of line 1 (position 6, 0-indexed)
        assert_eq!(chars_and_locations[6].0, '\n');
        assert_eq!(chars_and_locations[6].1.row, 1);
        assert_eq!(chars_and_locations[6].1.col, 7);

        // First character of line 2 (position 7)
        assert_eq!(chars_and_locations[7].0, 'l');
        assert_eq!(chars_and_locations[7].1.row, 2);
        assert_eq!(chars_and_locations[7].1.col, 1);

        // First character of line 3 (position 14)
        assert_eq!(chars_and_locations[14].0, 'l');
        assert_eq!(chars_and_locations[14].1.row, 3);
        assert_eq!(chars_and_locations[14].1.col, 1);
    }

    #[test]
    fn test_stream_locations_are_one_based() {
        let mut stream = FileStream::from_file("test/stream/multiline.fol")
            .expect("Should be able to read multiline.fol");

        while let Some((_ch, loc)) = stream.next_char() {
            assert!(loc.row >= 1, "Rows should be one-based");
            assert!(loc.col >= 1, "Columns should be one-based");
        }
    }

    #[test]
    fn test_carriage_return_only_advances_column_and_line_feed_advances_row() {
        let temp_root = fol_testkit::TempFixture::new("fol_stream_crlf");
        std::fs::create_dir_all(&temp_root).expect("Should create CRLF test fixture root");
        let temp_path = temp_root.join("fixture.fol");
        std::fs::write(&temp_path, "a\r\nb").expect("Should write CRLF test file");

        let mut stream = FileStream::from_file(
            temp_path
                .to_str()
                .expect("Temp CRLF path should be valid utf-8"),
        )
        .expect("Should read CRLF test file");

        let mut seen = Vec::new();
        while let Some((ch, loc)) = stream.next_char() {
            seen.push((ch, loc.row, loc.col));
        }

        assert_eq!(
            seen,
            vec![('a', 1, 1), ('\r', 1, 2), ('\n', 1, 3), ('b', 2, 1)]
        );

        std::fs::remove_file(&temp_path).ok();
        std::fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn test_unicode_handling() {
        let mut stream = FileStream::from_file("test/stream/unicode.fol")
            .expect("Should be able to read unicode.fol");

        let mut chars = Vec::new();
        while let Some((ch, _loc)) = stream.next_char() {
            chars.push(ch);
        }

        let content: String = chars.iter().collect();
        assert!(content.contains("世界"), "Should handle Chinese characters");
        assert!(content.contains('🌍'), "Should handle emoji");
        assert!(
            content.contains("café"),
            "Should handle accented characters"
        );
        assert!(content.contains("résumé"), "Should handle multiple accents");
        assert!(content.contains("naïve"), "Should handle diaeresis");
    }

    #[test]
    fn test_whitespace_handling() {
        let mut stream = FileStream::from_file("test/stream/whitespace.fol")
            .expect("Should be able to read whitespace.fol");

        let mut chars_and_locations = Vec::new();
        while let Some((ch, loc)) = stream.next_char() {
            chars_and_locations.push((ch, loc.clone()));
        }

        // Check that tabs and spaces are preserved
        assert_eq!(
            chars_and_locations[0].0, '\t',
            "Should preserve leading tab"
        );

        // Find spaces in content
        let space_positions: Vec<_> = chars_and_locations
            .iter()
            .enumerate()
            .filter(|(_, (ch, _))| *ch == ' ')
            .collect();

        assert!(
            !space_positions.is_empty(),
            "Should have spaces in the file"
        );

        // Verify location tracking works with tabs and spaces
        for (pos, (ch, loc)) in chars_and_locations.iter().enumerate() {
            if *ch == '\n' {
                // Next character should be on new line
                if pos + 1 < chars_and_locations.len() {
                    assert_eq!(chars_and_locations[pos + 1].1.row, loc.row + 1);
                    assert_eq!(chars_and_locations[pos + 1].1.col, 1);
                }
            }
        }
    }

    #[test]
    fn test_empty_file() {
        let mut stream = FileStream::from_file("test/stream/empty.fol")
            .expect("Should be able to read empty.fol");

        let result = stream.next_char();
        assert!(
            result.is_none(),
            "Empty file should return None immediately"
        );
    }

    #[test]
    fn test_nonexistent_file() {
        let result = FileStream::from_file("test/stream/nonexistent.fol");
        assert!(result.is_err(), "Should return error for nonexistent file");
    }

    #[test]
    fn test_sources_helper_returns_iterable_sources_on_success() {
        let sources = fol_stream::sources(
            "test/stream/basic.fol".to_string(),
            fol_stream::SourceType::File,
        )
        .expect("sources helper should return sources for a valid file entry")
        .collect::<Vec<_>>();

        assert_eq!(
            sources.len(),
            1,
            "Single-file helper should yield one source"
        );
        assert!(
            sources[0].path.ends_with("test/stream/basic.fol"),
            "Helper should preserve the requested source path"
        );
    }

    #[test]
    fn test_sources_helper_propagates_initialization_errors() {
        let result = fol_stream::sources(
            "test/stream/nonexistent.fol".to_string(),
            fol_stream::SourceType::File,
        );

        assert!(
            result.is_err(),
            "sources helper should report initialization failures instead of returning an empty iterator"
        );
    }

    #[test]
    fn test_file_input_used_as_folder_source_uses_parent_directory() {
        let from_file_as_folder = fol_stream::Source::init(
            "test/stream/fixture/main/main.fol",
            fol_stream::SourceType::Folder,
        )
        .expect("File path used as folder source should resolve through its parent directory");
        let from_folder =
            fol_stream::Source::init("test/stream/fixture/main", fol_stream::SourceType::Folder)
                .expect("Folder source should resolve normally");

        let file_as_folder_paths: Vec<_> = from_file_as_folder
            .iter()
            .map(|source| &source.path)
            .collect();
        let folder_paths: Vec<_> = from_folder.iter().map(|source| &source.path).collect();
        let file_as_folder_namespaces: Vec<_> = from_file_as_folder
            .iter()
            .map(|source| &source.namespace)
            .collect();
        let folder_namespaces: Vec<_> =
            from_folder.iter().map(|source| &source.namespace).collect();

        assert_eq!(
            file_as_folder_paths, folder_paths,
            "Folder-mode file input should discover the same parent-directory files"
        );
        assert_eq!(
            file_as_folder_namespaces, folder_namespaces,
            "Folder-mode file input should derive the same namespaces as the parent directory"
        );
        assert!(
            from_file_as_folder
                .iter()
                .all(|source| source.call == "test/stream/fixture/main/main.fol"),
            "Folder-mode file input should preserve the original file call site"
        );
        assert!(
            from_folder
                .iter()
                .all(|source| source.call == "test/stream/fixture/main"),
            "Direct folder input should preserve the folder call site"
        );
    }

    #[test]
    fn test_file_path_preservation() {
        let file_path = "test/stream/basic.fol";
        let mut stream = FileStream::from_file(file_path).expect("Should be able to read file");

        if let Some((_ch, loc)) = stream.next_char() {
            let file = loc.file.expect("Location should include a file path");
            assert!(
                file.ends_with(file_path),
                "Should preserve input suffix path, got {}",
                file
            );
        }
    }

    #[test]
    fn test_complete_file_processing() {
        let mut stream = FileStream::from_file("test/stream/multiline.fol")
            .expect("Should be able to read multiline.fol");

        let mut char_count = 0;
        let mut line_count = 0;
        let mut max_col = 0;
        let mut newline_loc: Option<(usize, usize)> = None;

        while let Some((ch, loc)) = stream.next_char() {
            char_count += 1;
            line_count = line_count.max(loc.row);
            max_col = max_col.max(loc.col);

            if let Some((prev_row, _prev_col)) = newline_loc {
                assert_eq!(loc.row, prev_row + 1, "Row should advance after newline");
                assert_eq!(loc.col, 1, "Column should reset to 1 after newline");
                newline_loc = None;
            }

            if ch == '\n' {
                newline_loc = Some((loc.row, loc.col));
            }
        }

        assert_eq!(
            char_count, 20,
            "Should count all characters including newlines"
        ); // "line 1\nline 2\nline 3" = 20 chars
        assert_eq!(line_count, 3, "Should track all three lines");
        assert!(max_col >= 6, "Should track column positions correctly");
    }

    #[test]
    fn test_file_boundary_resets_location_to_line_one_column_one() {
        let mut stream = FileStream::from_folder("test/stream/fixture/main")
            .expect("Should build stream from multiple sources");
        let mut previous_file = None;

        while let Some((_, loc)) = stream.next_char() {
            if previous_file.as_ref() != loc.file.as_ref() {
                if previous_file.is_some() {
                    assert_eq!(loc.row, 1, "New source should restart at row 1");
                    assert_eq!(loc.col, 1, "New source should restart at column 1");
                    return;
                }
                previous_file = loc.file.clone();
                continue;
            }

            previous_file = loc.file.clone();
        }

        panic!("Expected stream to advance into a second source");
    }
}

// Performance and stress tests
#[cfg(test)]
mod stream_performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_large_file_performance() {
        // Create a larger test file
        use std::fs::File;
        use std::io::Write;

        let large_file_path = "test/stream/large_test.fol";
        {
            let mut file = File::create(large_file_path).expect("Should create test file");
            for i in 0..1000 {
                writeln!(file, "line {} with some content to make it longer", i)
                    .expect("Should write line");
            }
        }

        let start = Instant::now();
        let mut stream = FileStream::from_file(large_file_path).expect("Should read large file");

        let mut char_count = 0;
        while let Some((_ch, _loc)) = stream.next_char() {
            char_count += 1;
        }

        let duration = start.elapsed();

        // Clean up
        std::fs::remove_file(large_file_path).ok();

        println!("Processed {} characters in {:?}", char_count, duration);
        assert!(
            char_count > 40000,
            "Should process substantial number of characters"
        );
        assert!(
            duration.as_secs() < 1,
            "Should process large file reasonably quickly"
        );
    }
}

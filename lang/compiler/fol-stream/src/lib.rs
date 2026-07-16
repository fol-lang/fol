// FOL Stream - Sophisticated file/folder to character stream conversion
// Handles .mod directories specially and creates unified character streams

use std::ffi::OsStr;
use std::path::Path;

/// Stream-local error type for file I/O failures.
#[derive(Debug, Clone)]
pub enum StreamError {
    NoSources,
    ReadFailed(String),
    InvalidPath(String),
    InvalidNamespace(String),
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::NoSources => write!(f, "No sources provided"),
            StreamError::ReadFailed(msg) => write!(f, "{msg}"),
            StreamError::InvalidPath(msg) => write!(f, "{msg}"),
            StreamError::InvalidNamespace(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for StreamError {}

/// Character provider trait for streaming characters with location info
pub trait CharacterProvider {
    fn next_char(&mut self) -> Option<(char, Location)>;
}

/// Stream source trait for converting paths to character providers
pub trait StreamSource {
    type Provider: CharacterProvider;
    fn into_provider(self) -> Result<Self::Provider, StreamError>;
}

/// Location tracking for characters in source files
#[derive(Debug, Clone)]
pub struct Location {
    pub row: usize,
    pub col: usize,
    pub file: Option<String>,
}

/// Source file representation with namespace information
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Source {
    pub call: String,      // Original call path
    pub path: String,      // Absolute file path
    pub data: String,      // File contents
    pub namespace: String, // Namespace path (e.g., "math::vector")
    pub package: String,   // Package name (root namespace)
}

/// Source type for path validation
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceType {
    File,
    Folder,
}

impl Source {
    /// Initialize sources from input path with package name
    pub fn init(input: &str, source_type: SourceType) -> Result<Vec<Self>, StreamError> {
        let package_name = detect_package_name(input)?;
        source(input, source_type, &package_name)
    }

    /// Initialize sources with explicit package name
    pub fn init_with_package(
        input: &str,
        source_type: SourceType,
        package_name: &str,
    ) -> Result<Vec<Self>, StreamError> {
        let package_name = validate_package_name(package_name)?;
        source(input, source_type, &package_name)
    }

    /// Get the full or relative path
    pub fn path(&self, abs: bool) -> String {
        if abs {
            self.path.clone()
        } else {
            self.rel_path()
        }
    }

    /// Stream-boundary identity excludes the original call site and keeps the
    /// canonical file plus the logical package/namespace chosen for this run.
    pub fn identity(&self) -> (String, String, String) {
        (
            self.path.clone(),
            self.package.clone(),
            self.namespace.clone(),
        )
    }

    fn rel_path(&self) -> String {
        std::fs::canonicalize(&self.path)
            .unwrap_or_else(|_| self.path.clone().into())
            .as_path()
            .to_str()
            .unwrap_or(&self.path)
            .trim_start_matches(&self.abs_path())
            .to_string()
    }

    fn abs_path(&self) -> String {
        std::fs::canonicalize(&self.call)
            .unwrap_or_else(|_| self.call.clone().into())
            .as_path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_str()
            .unwrap_or(".")
            .to_string()
    }

    pub fn module(&self) -> String {
        std::fs::canonicalize(&self.path)
            .unwrap_or_else(|_| self.path.clone().into())
            .as_path()
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_str()
            .unwrap_or(".")
            .trim_start_matches(&self.abs_path())
            .to_string()
    }
}

/// Multi-source file stream for handling complex folder structures
pub struct FileStream {
    sources: Vec<Source>,
    char_buffers: Vec<Box<[char]>>,
    current_source: usize,
    position: usize,
    location: Location,
}

impl Default for FileStream {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            char_buffers: Vec::new(),
            current_source: 0,
            position: 0,
            location: Location {
                row: 1,
                col: 1,
                file: None,
            },
        }
    }
}

impl FileStream {
    /// Create from a single file path
    pub fn from_file(path: &str) -> Result<Self, StreamError> {
        let sources = Source::init(path, SourceType::File)?;
        Self::from_sources(sources)
    }

    /// Create from a folder path (with sophisticated .mod handling)
    pub fn from_folder(path: &str) -> Result<Self, StreamError> {
        let sources = Source::init(path, SourceType::Folder)?;
        Self::from_sources(sources)
    }

    /// Create from a list of sources
    pub fn from_sources(mut sources: Vec<Source>) -> Result<Self, StreamError> {
        if sources.is_empty() {
            return Err(StreamError::NoSources);
        }

        // Eager loading is intentional for this hardening cycle so stream identity,
        // traversal order, and per-file location resets are fixed before later phases.
        for source in &mut sources {
            source.data = std::fs::read_to_string(&source.path).map_err(|e| {
                StreamError::ReadFailed(format!("Failed to read file {}: {}", source.path, e))
            })?;
        }

        let char_buffers = sources
            .iter()
            .map(|source| source.data.chars().collect::<Vec<_>>().into_boxed_slice())
            .collect::<Vec<_>>();
        let first_file = sources[0].path.clone();
        Ok(Self {
            sources,
            char_buffers,
            current_source: 0,
            position: 0,
            location: Location {
                row: 1,
                col: 1,
                file: Some(first_file),
            },
        })
    }

    /// Create from pre-loaded sources without reading from disk.
    ///
    /// Each source must have its `data` field already populated. The `path`
    /// field is used for location reporting but the file is never read.
    pub fn from_preloaded(sources: Vec<Source>) -> Result<Self, StreamError> {
        if sources.is_empty() {
            return Err(StreamError::NoSources);
        }
        let char_buffers = sources
            .iter()
            .map(|source| source.data.chars().collect::<Vec<_>>().into_boxed_slice())
            .collect::<Vec<_>>();
        let first_file = sources[0].path.clone();
        Ok(Self {
            sources,
            char_buffers,
            current_source: 0,
            position: 0,
            location: Location {
                row: 1,
                col: 1,
                file: Some(first_file),
            },
        })
    }

    /// Get current source being processed
    pub fn current_source(&self) -> Option<&Source> {
        self.sources.get(self.current_source)
    }

    /// Get all sources
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }
}

impl CharacterProvider for FileStream {
    fn next_char(&mut self) -> Option<(char, Location)> {
        // Check if we need to move to next source
        while self.current_source < self.sources.len() {
            if self.position >= self.char_buffers[self.current_source].len() {
                // Move to next source
                self.current_source += 1;
                self.position = 0;

                if self.current_source < self.sources.len() {
                    // Update location for new file
                    self.location = Location {
                        row: 1,
                        col: 1,
                        file: Some(self.sources[self.current_source].path.clone()),
                    };
                }
                continue;
            }

            // Get character from current position
            let ch = self.char_buffers[self.current_source][self.position];
            let loc = self.location.clone();

            self.position += 1;
            if ch == '\n' {
                self.location.row += 1;
                self.location.col = 1;
            } else {
                self.location.col += 1;
            }

            return Some((ch, loc));
        }

        None
    }
}

/// Create sources from input path and type with namespace support
fn source(
    input: &str,
    source_type: SourceType,
    package_name: &str,
) -> Result<Vec<Source>, StreamError> {
    let mut sources = Vec::new();
    let validated_path = check_validity(input, source_type.clone())?;

    match source_type {
        SourceType::File => {
            let namespace = compute_namespace(&validated_path, &validated_path, package_name)?;
            sources.push(Source {
                call: input.to_string(),
                path: validated_path,
                data: String::new(), // Will be loaded later
                namespace,
                package: package_name.to_string(),
            });
        }
        SourceType::Folder => {
            let discovered_files = from_dir(&validated_path)?;
            if discovered_files.is_empty() {
                return Err(StreamError::InvalidPath(
                    "No .fol files found".to_string(),
                ));
            }

            for file_path in discovered_files {
                let namespace = compute_namespace(&file_path, &validated_path, package_name)?;
                sources.push(Source {
                    call: input.to_string(),
                    path: file_path,
                    data: String::new(), // Will be loaded later
                    namespace,
                    package: package_name.to_string(),
                });
            }
        }
    }

    Ok(sources)
}

/// Sophisticated directory traversal with .mod handling
///
/// Key innovation: .mod directories are SKIPPED during traversal!
/// This allows for modular organization where .mod directories contain
/// mixed file types and module-specific code that's handled separately.
fn from_dir(directory: &str) -> Result<Vec<String>, StreamError> {
    let paths = std::fs::read_dir(directory).map_err(|e| {
        StreamError::ReadFailed(format!("Cannot read directory {}: {}", directory, e))
    })?;
    let mut entries = paths.collect::<Result<Vec<_>, _>>().map_err(|e| {
        StreamError::ReadFailed(format!("Cannot read directory entry: {}", e))
    })?;
    entries.sort_by(|left, right| {
        left.file_name()
            .to_string_lossy()
            .cmp(&right.file_name().to_string_lossy())
    });

    let mut files = Vec::new();

    for entry in entries {
        let filepath = entry.path().to_string_lossy().to_string();
        let filename = entry.file_name().to_string_lossy().to_string();

        if entry.path().is_dir() {
            // CRITICAL: Skip .mod directories - this is the key innovation!
            // .mod directories contain module-specific mixed content
            if filename.ends_with(".mod") || filename == ".fol" {
                continue;
            }

            // Recursively process non-.mod directories
            let recursive_files = from_dir(&filepath)?;
            files.extend(recursive_files);
        } else {
            // Only include .fol files
            if let Some(extension) = entry.path().extension().and_then(OsStr::to_str) {
                if extension == "fol" {
                    files.push(filepath);
                }
            }
        }
    }

    Ok(files)
}

/// Validate input path and return canonical path
fn check_validity(input: &str, source_type: SourceType) -> Result<String, StreamError> {
    let path = Path::new(input);

    if !path.exists() {
        return Err(StreamError::InvalidPath(format!(
            "Path {} does not exist",
            input
        )));
    }

    let canonical_path = std::fs::canonicalize(path)
        .map_err(|e| {
            StreamError::InvalidPath(format!("Cannot resolve path {}: {}", input, e))
        })?
        .to_string_lossy()
        .to_string();

    match (path.is_dir(), source_type) {
        (true, SourceType::Folder) => Ok(canonical_path),
        (false, SourceType::File) => Ok(canonical_path),
        (false, SourceType::Folder) => {
            // If file provided but folder expected, use parent directory
            if let Some(parent) = path.parent() {
                let parent_canonical = std::fs::canonicalize(parent)
                    .map_err(|e| {
                        StreamError::InvalidPath(format!(
                            "Cannot resolve parent path: {}",
                            e
                        ))
                    })?
                    .to_string_lossy()
                    .to_string();
                Ok(parent_canonical)
            } else {
                Err(StreamError::InvalidPath(format!(
                    "Invalid path for folder source: {}",
                    input
                )))
            }
        }
        (true, SourceType::File) => Err(StreamError::InvalidPath(format!(
            "Expected file but got directory: {}",
            input
        ))),
    }
}

/// Create iterator over sources from input path
pub fn sources(
    input: String,
    source_type: SourceType,
) -> Result<impl Iterator<Item = Source>, StreamError> {
    let sources = Source::init(&input, source_type)?;
    Ok(sources.into_iter())
}

/// Detect package name from the explicit entry root instead of host build files.
fn detect_package_name(input_path: &str) -> Result<String, StreamError> {
    let path = std::path::Path::new(input_path);
    let fallback_root = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let fallback_name = fallback_root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| {
            path.file_stem()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "root".to_string());

    validate_package_name(&fallback_name)
}

fn validate_package_name(package_name: &str) -> Result<String, StreamError> {
    if is_valid_namespace_component(package_name) {
        Ok(package_name.to_string())
    } else {
        Err(StreamError::InvalidNamespace(format!(
            "Invalid package name '{}': package names must follow namespace identifier rules",
            package_name
        )))
    }
}

/// Compute namespace from file path relative to project root
fn compute_namespace(
    file_path: &str,
    root_path: &str,
    package_name: &str,
) -> Result<String, StreamError> {
    let file_path = std::path::Path::new(file_path);
    let root_path = std::path::Path::new(root_path);

    // Get the directory containing the file (not the file itself)
    let file_dir = file_path.parent().unwrap_or(root_path);

    // Compute relative path from root to file directory
    let relative_path = if file_dir == root_path {
        // File is in root directory
        None
    } else {
        // Try to get relative path
        file_dir.strip_prefix(root_path).ok()
    };

    match relative_path {
        None => {
            // File is in root directory -> root namespace (package name)
            Ok(package_name.to_string())
        }
        Some(rel_path) => {
            // File is in subdirectory -> build namespace path
            let mut namespace_parts = vec![package_name.to_string()];

            for component in rel_path.components() {
                let name = component
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| {
                        StreamError::InvalidNamespace(
                            "Invalid namespace component: path segment is not valid UTF-8"
                                .to_string(),
                        )
                    })?;

                // Skip .mod directories in namespace (they were already filtered out)
                if name.ends_with(".mod") {
                    continue;
                }

                if !is_valid_namespace_component(name) {
                    return Err(StreamError::InvalidNamespace(format!(
                        "Invalid namespace component '{}': namespace components must follow identifier rules",
                        name
                    )));
                }

                namespace_parts.push(name.to_string());
            }

            Ok(namespace_parts.join("::"))
        }
    }
}

/// Check if a name is a valid namespace component (no dots, valid identifier)
fn is_valid_namespace_component(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !first.is_ascii() || first.is_ascii_digit() || !(first.is_ascii_alphabetic() || first == '_')
    {
        return false;
    }

    if name.contains("__") {
        return false;
    }

    chars.all(|ch| ch.is_ascii() && (ch.is_ascii_alphanumeric() || ch == '_'))
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use std::fs;

    #[test]
    fn from_sources_precomputes_char_buffers_for_every_source() {
        let temp_root = std::env::temp_dir().join(format!(
            "fol_stream_char_buffers_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time should be after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(temp_root.join("nested"))
            .expect("Should create temp folders for char-buffer test");
        fs::write(temp_root.join("alpha.fol"), "alpha\n").expect("Should write alpha temp source");
        fs::write(temp_root.join("nested/beta.fol"), "beta\u{1f30d}\n")
            .expect("Should write beta temp source");

        let sources = Source::init(
            temp_root
                .to_str()
                .expect("Temp root path should be valid UTF-8"),
            SourceType::Folder,
        )
        .expect("Should discover temp sources for char-buffer test");
        let mut stream =
            FileStream::from_sources(sources).expect("Should build stream with cached buffers");

        assert_eq!(
            stream.char_buffers.len(),
            stream.sources.len(),
            "FileStream should precompute one reusable character buffer per source"
        );
        for (source, buffer) in stream.sources.iter().zip(stream.char_buffers.iter()) {
            let buffered: String = buffer.iter().collect();
            assert_eq!(
                buffered, source.data,
                "Each cached character buffer should mirror the eagerly loaded source text"
            );
        }

        let expected_buffers = stream
            .char_buffers
            .iter()
            .map(|buffer| buffer.to_vec())
            .collect::<Vec<_>>();
        while let Some((_ch, _loc)) = stream.next_char() {}

        assert_eq!(
            stream
                .char_buffers
                .iter()
                .map(|buffer| buffer.to_vec())
                .collect::<Vec<_>>(),
            expected_buffers,
            "Streaming across file boundaries should reuse precomputed character buffers without rebuilding them"
        );

        fs::remove_dir_all(&temp_root).ok();
    }

    #[test]
    fn from_dir_skips_internal_fol_workspace_directory() {
        let temp_root = std::env::temp_dir().join(format!(
            "fol_stream_skip_internal_fol_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time should be after unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(temp_root.join(".fol/cache"))
            .expect("Should create internal .fol directory");
        fs::write(temp_root.join("src.fol"), "fun[] main(): int = {\n    return 7;\n};\n")
            .expect("Should write visible source file");
        fs::write(
            temp_root.join(".fol/cache/generated.fol"),
            "fun[] ghost(): int = {\n    return 0;\n};\n",
        )
        .expect("Should write hidden internal source file");

        let discovered = from_dir(
            temp_root
                .to_str()
                .expect("Temp root path should be valid UTF-8"),
        )
        .expect("Directory traversal should succeed");

        assert_eq!(discovered.len(), 1);
        assert!(
            discovered[0].ends_with("src.fol"),
            "Only visible package sources should be discovered: {:?}",
            discovered
        );

        fs::remove_dir_all(&temp_root).ok();
    }
}

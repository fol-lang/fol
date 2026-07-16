use crate::{
    build_dependency::{project_dependency_surface, DependencyBuildSurfaceSet},
    metadata::PackageDependencySourceKind,
    PackageBuildDefinition, PackageBuildMode, PackageConfig, PackageError, PackageErrorKind,
    PackageIdentity, PackageSourceKind, PreparedPackage,
};
use fol_lexer::lexer::stage3::Elements;
use fol_parser::ast::{AstParser, ParsedPackage, SyntaxOrigin, UsePathSegment};
use fol_stream::{FileStream, Source, SourceType};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct PackageSession {
    config: PackageConfig,
    prepared_packages: BTreeMap<PackageIdentity, PreparedPackage>,
    loading_stack: Vec<PackageIdentity>,
}

impl PackageSession {
    pub fn new() -> Self {
        Self::with_config(PackageConfig::default())
    }

    pub fn with_config(config: PackageConfig) -> Self {
        let config = PackageConfig {
            std_root: config.effective_std_root(),
            ..config
        };
        Self {
            config,
            prepared_packages: BTreeMap::new(),
            loading_stack: Vec::new(),
        }
    }

    pub fn config(&self) -> &PackageConfig {
        &self.config
    }

    pub fn cached_package_count(&self) -> usize {
        self.prepared_packages.len()
    }

    pub fn prepare_entry_package(
        &self,
        syntax: ParsedPackage,
    ) -> Result<PreparedPackage, PackageError> {
        let inferred_root = infer_package_root(&syntax)?;
        let display_name = inferred_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("root")
            .to_string();
        Ok(PreparedPackage::new(
            PackageIdentity {
                source_kind: PackageSourceKind::Entry,
                canonical_root: inferred_root.to_string_lossy().to_string(),
                display_name,
            },
            syntax,
        ))
    }

    pub fn load_directory_package(
        &mut self,
        directory: &Path,
        source_kind: PackageSourceKind,
    ) -> Result<PreparedPackage, PackageError> {
        let canonical_root = canonical_directory_root(directory, source_kind)?;
        reject_formal_package_roots_for_directory_imports(canonical_root.as_path(), source_kind)?;
        let display_name = canonical_root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                PackageError::new(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "package loader could not derive a package name from '{}'",
                        canonical_root.display()
                    ),
                )
            })?
            .to_string();
        let identity = PackageIdentity {
            source_kind,
            canonical_root: canonical_root.to_string_lossy().to_string(),
            display_name: display_name.clone(),
        };

        self.begin_loading(&identity)?;

        if let Some(cached) = self.cached_package(&identity).cloned() {
            self.finish_loading();
            return Ok(cached);
        }

        let prepared_result =
            parse_directory_package_syntax(canonical_root.as_path(), &display_name, source_kind)
                .map(|syntax| PreparedPackage::new(identity.clone(), syntax));

        self.finish_loading();

        let prepared = prepared_result?;
        self.cache_package(prepared.clone());
        Ok(prepared)
    }

    pub fn load_package_from_store(
        &mut self,
        store_root: &Path,
        path_segments: &[UsePathSegment],
    ) -> Result<PreparedPackage, PackageError> {
        let target_root = resolve_directory_path(store_root, path_segments);
        let canonical_root =
            canonical_directory_root(target_root.as_path(), PackageSourceKind::Package)?;
        let canonical_root_str = canonical_root.to_string_lossy().to_string();
        if let Some(cached) =
            self.cached_package_by_root(PackageSourceKind::Package, &canonical_root_str)
        {
            return Ok(cached);
        }
        let build_path = canonical_root.join("build.fol");
        if !build_path.is_file() {
            return Err(PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package pkg import target '{}' is missing required package build file '{}'",
                    canonical_root.display(),
                    build_path.display()
                ),
            ));
        }

        self.load_formal_package_root(
            canonical_root.as_path(),
            canonical_root_str,
            PackageSourceKind::Package,
            Some(store_root),
        )
    }

    pub fn load_package_from_store_target(
        &mut self,
        store_root: &Path,
        import_target: &str,
    ) -> Result<PreparedPackage, PackageError> {
        let target_root = resolve_directory_target(store_root, import_target);
        let canonical_root =
            canonical_directory_root(target_root.as_path(), PackageSourceKind::Package)?;
        let canonical_root_str = canonical_root.to_string_lossy().to_string();
        if let Some(cached) =
            self.cached_package_by_root(PackageSourceKind::Package, &canonical_root_str)
        {
            return Ok(cached);
        }
        let build_path = canonical_root.join("build.fol");
        if !build_path.is_file() {
            return Err(PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package pkg import target '{}' is missing required package build file '{}'",
                    canonical_root.display(),
                    build_path.display()
                ),
            ));
        }

        self.load_formal_package_root(
            canonical_root.as_path(),
            canonical_root_str,
            PackageSourceKind::Package,
            Some(store_root),
        )
    }

    pub fn load_materialized_package(
        &mut self,
        package_root: &Path,
    ) -> Result<PreparedPackage, PackageError> {
        let canonical_root = canonical_directory_root(package_root, PackageSourceKind::Package)?;
        let canonical_root_str = canonical_root.to_string_lossy().to_string();
        self.load_formal_package_root(
            canonical_root.as_path(),
            canonical_root_str,
            PackageSourceKind::Package,
            None,
        )
    }

    pub fn load_internal_standard_package(&mut self) -> Result<PreparedPackage, PackageError> {
        let std_root = self.config.std_root.clone().ok_or_else(|| {
            let bundled = crate::bundled_std_root();
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "internal dependency 'standard' requires bundled std at '{}' or an explicit --std-root <DIR> override",
                    bundled.display()
                ),
            )
        })?;
        self.load_directory_package(Path::new(&std_root), PackageSourceKind::Standard)
    }

    #[cfg(test)]
    pub(crate) fn loading_depth(&self) -> usize {
        self.loading_stack.len()
    }

    pub(crate) fn cached_package(&self, identity: &PackageIdentity) -> Option<&PreparedPackage> {
        self.prepared_packages.get(identity)
    }

    fn cached_package_by_root(
        &self,
        source_kind: PackageSourceKind,
        canonical_root: &str,
    ) -> Option<PreparedPackage> {
        self.prepared_packages
            .iter()
            .find_map(|(identity, package)| {
                (identity.source_kind == source_kind && identity.canonical_root == canonical_root)
                    .then(|| package.clone())
            })
    }

    pub(crate) fn cache_package(&mut self, package: PreparedPackage) {
        self.prepared_packages
            .insert(package.identity.clone(), package);
    }

    pub(crate) fn begin_loading(&mut self, identity: &PackageIdentity) -> Result<(), PackageError> {
        if self.loading_stack.contains(identity) {
            return Err(self.import_cycle_error(identity));
        }
        self.loading_stack.push(identity.clone());
        Ok(())
    }

    pub(crate) fn finish_loading(&mut self) {
        self.loading_stack.pop();
    }

    fn import_cycle_error(&self, next: &PackageIdentity) -> PackageError {
        let cycle = self
            .loading_stack
            .iter()
            .map(|identity| identity.canonical_root.as_str())
            .chain(std::iter::once(next.canonical_root.as_str()))
            .collect::<Vec<_>>()
            .join(" -> ");
        PackageError::new(
            PackageErrorKind::ImportCycle,
            format!("package import cycle detected while loading package roots: {cycle}"),
        )
    }

    fn load_formal_package_root(
        &mut self,
        canonical_root: &Path,
        canonical_root_str: String,
        source_kind: PackageSourceKind,
        store_root: Option<&Path>,
    ) -> Result<PreparedPackage, PackageError> {
        if let Some(cached) = self.cached_package_by_root(source_kind, &canonical_root_str) {
            return Ok(cached);
        }
        let build_path = canonical_root.join("build.fol");
        if !build_path.is_file() {
            return Err(PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package pkg import target '{}' is missing required package build file '{}'",
                    canonical_root.display(),
                    build_path.display()
                ),
            ));
        }

        let metadata = crate::parse_package_metadata_from_build(build_path.as_path())?;
        let build = PackageBuildDefinition {
            mode: PackageBuildMode::ModernOnly,
        };
        let identity = PackageIdentity {
            source_kind,
            canonical_root: canonical_root_str,
            display_name: metadata.name.clone(),
        };

        self.begin_loading(&identity)?;

        if let Some(cached) = self.cached_package(&identity).cloned() {
            self.finish_loading();
            return Ok(cached);
        }

        if let Some(store_root) = store_root {
            for dependency in metadata.dependencies.iter().filter(|dependency| {
                dependency.evaluation_mode == fol_build::DependencyBuildEvaluationMode::Eager
            }) {
                match dependency.source_kind {
                    PackageDependencySourceKind::PackageStore => {
                        let path_segments = dependency
                            .target
                            .split('/')
                            .filter(|segment| !segment.trim().is_empty())
                            .enumerate()
                            .map(|(index, part)| UsePathSegment {
                                separator: (index > 0)
                                    .then_some(fol_parser::ast::UsePathSeparator::Slash),
                                spelling: part.to_string(),
                            })
                            .collect::<Vec<_>>();
                        self.load_package_from_store(store_root, &path_segments)?;
                    }
                    PackageDependencySourceKind::Internal if dependency.target == "standard" => {
                        self.load_internal_standard_package()?;
                    }
                    _ => {}
                }
            }
        }

        let prepared_result =
            parse_directory_package_syntax(canonical_root, &metadata.name, source_kind).and_then(
                |syntax| {
                    let mut dependency_surfaces = DependencyBuildSurfaceSet::new();
                    dependency_surfaces.add(project_dependency_surface(
                        &metadata.name,
                        canonical_root,
                        &syntax,
                    )?);
                    Ok(PreparedPackage::with_controls(
                        identity.clone(),
                        metadata,
                        build,
                        Vec::new(),
                        Some(dependency_surfaces),
                        None,
                        syntax,
                    ))
                },
            );

        self.finish_loading();

        let prepared = prepared_result?;
        self.cache_package(prepared.clone());
        Ok(prepared)
    }
}

impl Default for PackageSession {
    fn default() -> Self {
        Self::new()
    }
}

pub fn infer_package_root(syntax: &ParsedPackage) -> Result<PathBuf, PackageError> {
    let mut parents = syntax
        .source_units
        .iter()
        .filter_map(|unit| Path::new(&unit.path).parent().map(Path::to_path_buf));

    let Some(mut common_root) = parents.next() else {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            "package loading requires at least one parsed source unit",
        ));
    };

    for parent in parents {
        while !parent.starts_with(&common_root) {
            if !common_root.pop() {
                return Err(PackageError::new(
                    PackageErrorKind::Internal,
                    "package loading could not infer a common package root from parsed source paths",
                ));
            }
        }
    }

    Ok(common_root)
}

pub fn parse_directory_package_syntax(
    root: &Path,
    display_name: &str,
    _source_kind: PackageSourceKind,
) -> Result<ParsedPackage, PackageError> {
    let root_str = root.to_str().ok_or_else(|| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!("package root '{}' is not valid UTF-8", root.display()),
        )
    })?;
    let sources =
        Source::init_with_package(root_str, SourceType::Folder, display_name).map_err(|error| {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package loader could not initialize package sources from '{}': {}",
                    root.display(),
                    error
                ),
            )
        })?;
    let mut stream = FileStream::from_sources(sources).map_err(|error| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package loader could not read package sources from '{}': {}",
                root.display(),
                error
            ),
        )
    })?;
    let mut lexer = Elements::init(&mut stream);
    let mut parser = AstParser::new();

    parser.parse_package(&mut lexer).map_err(|diagnostics| {
        let mut iter = diagnostics.into_iter();
        let first = iter
            .next()
            .expect("parse_package should produce at least one error");
        let origin = first.primary_location().map(|loc| SyntaxOrigin {
            file: loc.file.clone(),
            line: loc.line,
            column: loc.column,
            length: loc.length.unwrap_or(1),
        });
        let message = format!(
            "package loader could not parse imported package '{}': {}",
            root.display(),
            first.message
        );
        let mut error = match origin {
            Some(origin) => {
                PackageError::with_origin(PackageErrorKind::InvalidInput, message, origin)
            }
            None => PackageError::new(PackageErrorKind::InvalidInput, message),
        };
        for extra in iter {
            if let Some(loc) = extra.primary_location() {
                error = error.with_related_origin(
                    SyntaxOrigin {
                        file: loc.file.clone(),
                        line: loc.line,
                        column: loc.column,
                        length: loc.length.unwrap_or(1),
                    },
                    extra.message.clone(),
                );
            }
        }
        error
    })
}

pub fn canonical_directory_root(
    directory: &Path,
    source_kind: PackageSourceKind,
) -> Result<PathBuf, PackageError> {
    let metadata = std::fs::metadata(directory).map_err(|error| {
        let label = import_source_label(source_kind);
        if error.kind() == std::io::ErrorKind::NotFound {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package {label} import target '{}' does not exist",
                    directory.display(),
                ),
            )
        } else {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package loader could not inspect {label} import target '{}': {}",
                    directory.display(),
                    error
                ),
            )
        }
    })?;

    if metadata.is_file() {
        let label = import_source_label(source_kind);
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package {label} import target '{}' must point to a directory, not a file",
                directory.display(),
            ),
        ));
    }

    if !metadata.is_dir() {
        let label = import_source_label(source_kind);
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package {label} import target '{}' must point to a directory",
                directory.display(),
            ),
        ));
    }

    std::fs::canonicalize(directory).map_err(|error| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package loader could not canonicalize {} import target '{}': {}",
                import_source_label(source_kind),
                directory.display(),
                error
            ),
        )
    })
}

pub fn resolve_directory_path(source_dir: &Path, path_segments: &[UsePathSegment]) -> PathBuf {
    let mut relative = PathBuf::new();
    for segment in path_segments {
        relative.push(&segment.spelling);
    }

    if relative.is_absolute() {
        relative
    } else {
        source_dir.join(relative)
    }
}

pub fn resolve_directory_target(source_dir: &Path, import_target: &str) -> PathBuf {
    let mut relative = PathBuf::new();
    for segment in import_target_segments(import_target) {
        relative.push(segment);
    }
    if relative.is_absolute() {
        relative
    } else {
        source_dir.join(relative)
    }
}

fn import_target_segments(import_target: &str) -> Vec<&str> {
    if import_target.contains("://") {
        return vec![import_target];
    }

    let mut parts = Vec::new();
    let mut start = 0usize;
    let bytes = import_target.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'/' {
            if start != i {
                parts.push(&import_target[start..i]);
            }
            start = i + 1;
            i += 1;
            continue;
        }

        if bytes[i] == b':' && i + 1 < bytes.len() && bytes[i + 1] == b':' {
            if start != i {
                parts.push(&import_target[start..i]);
            }
            start = i + 2;
            i += 2;
            continue;
        }

        i += 1;
    }

    if start < import_target.len() {
        parts.push(&import_target[start..]);
    }

    parts
}

fn import_source_label(source_kind: PackageSourceKind) -> &'static str {
    match source_kind {
        PackageSourceKind::Local => "loc",
        PackageSourceKind::Standard => "std",
        PackageSourceKind::Package => "pkg",
        PackageSourceKind::Entry => "entry",
    }
}

fn reject_formal_package_roots_for_directory_imports(
    root: &Path,
    source_kind: PackageSourceKind,
) -> Result<(), PackageError> {
    if source_kind != PackageSourceKind::Local {
        return Ok(());
    }

    let build_path = root.join("build.fol");
    if build_path.is_file() {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package loc import target '{}' contains '{}'; formal package roots must be imported with pkg instead of loc",
                root.display(),
                build_path.display()
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests;

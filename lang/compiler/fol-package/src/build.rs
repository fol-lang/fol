use crate::{
    build_entry::{
        validate_parsed_build_entry, BuildEntrySignatureExpectation, BuildEntryValidationError,
    },
    PackageError, PackageErrorKind,
};
use fol_lexer::lexer::stage3::Elements;
use fol_parser::ast::{AstParser, ParsedPackage, SyntaxOrigin};
use fol_stream::FileStream;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageBuildMode {
    Empty,
    ModernOnly,
}

impl PackageBuildMode {
    pub fn has_semantic_build_entry(self) -> bool {
        matches!(self, Self::ModernOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageBuildDefinition {
    pub mode: PackageBuildMode,
}

impl Default for PackageBuildDefinition {
    fn default() -> Self {
        Self {
            mode: PackageBuildMode::Empty,
        }
    }
}

impl PackageBuildDefinition {
    pub fn mode(&self) -> PackageBuildMode {
        self.mode
    }

    fn semantic_only() -> Self {
        Self {
            mode: PackageBuildMode::ModernOnly,
        }
    }
}

pub fn classify_semantic_build_mode(parsed: &ParsedPackage, _unused: bool) -> PackageBuildMode {
    if validate_parsed_build_entry(parsed, &BuildEntrySignatureExpectation::canonical()).is_ok() {
        PackageBuildMode::ModernOnly
    } else {
        PackageBuildMode::Empty
    }
}

pub fn parse_package_build(path: &Path) -> Result<PackageBuildDefinition, PackageError> {
    let path_str = path.to_str().ok_or_else(|| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!("package build file '{}' is not valid UTF-8", path.display()),
        )
    })?;
    let source = std::fs::read_to_string(path).map_err(|error| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package loader could not read package build file '{}': {}",
                path.display(),
                error
            ),
        )
    })?;
    let mut stream = FileStream::from_file(path_str).map_err(|error| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package loader could not read package build file '{}': {}",
                path.display(),
                error
            ),
        )
    })?;
    let mut lexer = Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let parsed = parser
        .parse_package(&mut lexer)
        .map_err(|errors| build_parse_error(path, errors))?;

    if !source.contains("pro[] build(") {
        return Err(missing_canonical_build_error(None));
    }

    extract_package_build_definition(&parsed)
}

pub fn parse_package_build_mode(path: &Path) -> Result<PackageBuildMode, PackageError> {
    parse_package_build(path).map(|build| build.mode())
}

fn build_parse_error(path: &Path, diagnostics: Vec<fol_diagnostics::Diagnostic>) -> PackageError {
    let mut iter = diagnostics.into_iter();
    let first = iter
        .next()
        .expect("build parser should produce at least one error");
    let origin = first.primary_location().map(|loc| SyntaxOrigin {
        file: loc.file.clone(),
        line: loc.line,
        column: loc.column,
        length: loc.length.unwrap_or(1),
    });
    let message = format!(
        "package loader could not parse package build file '{}': {}",
        path.display(),
        first.message
    );
    let mut error = match origin {
        Some(origin) => PackageError::with_origin(PackageErrorKind::InvalidInput, message, origin),
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
}

pub fn extract_package_build_definition(
    parsed: &ParsedPackage,
) -> Result<PackageBuildDefinition, PackageError> {
    validate_parsed_build_entry(parsed, &BuildEntrySignatureExpectation::canonical())
        .map_err(package_error_from_build_entry)?;
    Ok(PackageBuildDefinition::semantic_only())
}

fn package_error_from_build_entry(errors: Vec<BuildEntryValidationError>) -> PackageError {
    let error = errors
        .into_iter()
        .next()
        .unwrap_or_else(|| missing_canonical_build_error(None).into_validation_error());
    match error.origin {
        Some(origin) => {
            PackageError::with_origin(PackageErrorKind::InvalidInput, error.message, origin)
        }
        None => PackageError::new(PackageErrorKind::InvalidInput, error.message),
    }
}

fn missing_canonical_build_error(origin: Option<SyntaxOrigin>) -> PackageError {
    match origin {
        Some(origin) => PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            "build.fol must declare exactly one canonical `pro[] build(): non` entry",
            origin,
        ),
        None => PackageError::new(
            PackageErrorKind::InvalidInput,
            "build.fol must declare exactly one canonical `pro[] build(): non` entry",
        ),
    }
}

trait IntoValidationError {
    fn into_validation_error(self) -> BuildEntryValidationError;
}

impl IntoValidationError for PackageError {
    fn into_validation_error(self) -> BuildEntryValidationError {
        BuildEntryValidationError {
            kind: crate::build_entry::BuildEntryValidationErrorKind::MissingEntry,
            message: self.message().to_string(),
            origin: self.origin().cloned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_semantic_build_mode, extract_package_build_definition, parse_package_build,
        parse_package_build_mode, PackageBuildDefinition, PackageBuildMode,
    };
    use crate::PackageErrorKind;
    use fol_parser::ast::AstParser;
    use fol_stream::FileStream;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(label: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fol_package_build_definition_{}_{}_{}",
            label,
            std::process::id(),
            stamp
        ))
    }

    fn write_build_fixture(label: &str, source: &str) -> std::path::PathBuf {
        let temp_root = unique_temp_root(label);
        fs::create_dir_all(&temp_root).expect("Should create temporary build fixture root");
        let build_path = temp_root.join("build.fol");
        fs::write(&build_path, source).expect("Should write build fixture");
        build_path
    }

    #[test]
    fn package_build_parser_accepts_canonical_semantic_build_files() {
        let build_path = write_build_fixture(
            "semantic_modern_build",
            "pro[] build(): non = {\n    return;\n};\n",
        );

        let build =
            parse_package_build(&build_path).expect("Semantic build fixture should parse cleanly");

        assert_eq!(build, PackageBuildDefinition::semantic_only());

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_build_parser_rejects_old_root_defs() {
        let build_path = write_build_fixture("legacy_root_def", "def root: loc = \"src\";\n");

        let error = parse_package_build(&build_path).expect_err("Old root defs should be rejected");

        assert_eq!(error.kind(), PackageErrorKind::InvalidInput);
        assert!(error
            .to_string()
            .contains("canonical `pro[] build(): non` entry"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_build_parser_rejects_old_def_build_headers() {
        let build_path = write_build_fixture("legacy_def_build", "def build(): non = graph;\n");

        let error =
            parse_package_build(&build_path).expect_err("Old def build headers should fail");

        assert_eq!(error.kind(), PackageErrorKind::InvalidInput);
        assert!(error
            .to_string()
            .contains("package loader could not parse package build file"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_build_parser_rejects_plain_pro_build_headers() {
        let build_path =
            write_build_fixture("plain_pro_build", "pro build(): non = {\n    return;\n};\n");

        let error =
            parse_package_build(&build_path).expect_err("Plain pro build headers should fail");

        assert_eq!(error.kind(), PackageErrorKind::InvalidInput);
        assert!(error
            .to_string()
            .contains("canonical `pro[] build(): non` entry"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_build_parser_rejects_wrong_build_signatures_with_exact_origins() {
        let build_path = write_build_fixture(
            "wrong_signature",
            "pro[] build(graph: int): non = {\n    return;\n};\n",
        );

        let error =
            parse_package_build(&build_path).expect_err("Wrong semantic build signatures fail");

        assert_eq!(error.kind(), PackageErrorKind::InvalidInput);
        assert!(error.to_string().contains("must not declare parameters"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_build_parser_keeps_exact_origins_for_parse_failures() {
        let build_path =
            write_build_fixture("build_parse_origin", "pro[] build(): non = 42 @ 7;\n");

        let error = parse_package_build(&build_path)
            .expect_err("Malformed build files should preserve parse-error origins");

        assert_eq!(error.kind(), PackageErrorKind::InvalidInput);
        let origin = error
            .origin()
            .expect("Malformed build parse failures should keep exact origins");
        assert_eq!(origin.file.as_deref(), build_path.to_str());
        assert_eq!(origin.line, 1);
        assert!(origin.column >= 1);

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_build_parser_rejects_build_files_without_canonical_entry() {
        let build_path = write_build_fixture(
            "helper_only",
            "fun[] helper(): int = {\n    return 1;\n};\n",
        );

        let error = parse_package_build(&build_path)
            .expect_err("Build files without canonical build entry should fail");

        assert_eq!(error.kind(), PackageErrorKind::InvalidInput);
        assert!(error
            .to_string()
            .contains("canonical `pro[] build(): non` entry"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn semantic_build_mode_classification_prefers_validated_build_entries() {
        let build_path = write_build_fixture(
            "semantic_build_mode",
            "pro[] build(): non = {\n    return;\n};\n",
        );
        let mut stream = FileStream::from_file(
            build_path
                .to_str()
                .expect("Temporary build fixture path should be valid UTF-8"),
        )
        .expect("Should open the semantic build-mode fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let parsed = parser
            .parse_package(&mut lexer)
            .expect("semantic build-mode fixture should parse");

        assert_eq!(
            classify_semantic_build_mode(&parsed, false),
            PackageBuildMode::ModernOnly
        );

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn shared_build_mode_parser_returns_classified_mode() {
        let build_path = write_build_fixture(
            "build_mode_parser",
            "pro[] build(): non = {\n    return;\n};\n",
        );

        let mode = parse_package_build_mode(&build_path)
            .expect("Shared build mode parser should classify semantic build files");

        assert_eq!(mode, PackageBuildMode::ModernOnly);

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_build_mode_helpers_expose_semantic_only_participation() {
        assert!(!PackageBuildMode::Empty.has_semantic_build_entry());
        assert!(PackageBuildMode::ModernOnly.has_semantic_build_entry());
    }

    #[test]
    fn ast_build_extraction_requires_the_canonical_semantic_entry() {
        let build_path = write_build_fixture(
            "ast_semantic_only",
            "pro[] build(): non = {\n    return;\n};\n",
        );
        let mut stream = FileStream::from_file(
            build_path
                .to_str()
                .expect("Temporary build fixture path should be valid UTF-8"),
        )
        .expect("Should open the semantic extraction fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let parsed = parser
            .parse_package(&mut lexer)
            .expect("semantic extraction fixture should parse");

        let build = extract_package_build_definition(&parsed)
            .expect("AST build extraction should validate the canonical entry");

        assert_eq!(build, PackageBuildDefinition::semantic_only());

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }
}

use crate::{PackageError, PackageErrorKind};
use fol_build::DependencyBuildEvaluationMode;
use fol_lexer::lexer::stage3::Elements;
use fol_parser::ast::{AstNode, AstParser, CallSurface, ParsedPackage, SyntaxOrigin};
use fol_stream::FileStream;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedPackageMetadataField {
    pub name: String,
    pub value: String,
    pub origin: Option<SyntaxOrigin>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedPackageDependencyDecl {
    pub alias: String,
    pub source: String,
    pub target: String,
    pub evaluation_mode: DependencyBuildEvaluationMode,
    pub git_version: Option<String>,
    pub git_hash: Option<String>,
    pub origin: Option<SyntaxOrigin>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PackageDependencySourceKind {
    Local,
    PackageStore,
    Git,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyDecl {
    pub alias: String,
    pub source_kind: PackageDependencySourceKind,
    pub target: String,
    pub evaluation_mode: DependencyBuildEvaluationMode,
    pub git_version: Option<fol_build::GitDependencyVersionSelector>,
    pub git_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<PackageDependencyDecl>,
}

impl PackageDependencyDecl {
    pub fn git_locator_string(&self) -> String {
        let mut rendered = self.target.clone();
        if let Some(version) = &self.git_version {
            rendered.push('#');
            rendered.push_str(&version.render());
        }
        if let Some(hash) = &self.git_hash {
            rendered.push('#');
            rendered.push_str("hash:");
            rendered.push_str(hash);
        }
        rendered
    }
}

pub fn parse_package_metadata_from_build(path: &Path) -> Result<PackageMetadata, PackageError> {
    let fields = extract_package_metadata_fields_from_build(path)?;
    let dependencies = extract_package_dependencies_from_build(path)?;
    materialize_package_metadata_from_build(path, fields, dependencies)
}

pub fn extract_package_metadata_fields_from_build(
    path: &Path,
) -> Result<Vec<ExtractedPackageMetadataField>, PackageError> {
    let parsed = parse_build_package_for_metadata(path)?;
    let build_body = canonical_build_body(&parsed).ok_or_else(|| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "build.fol '{}' must declare exactly one canonical `pro[] build(): non` entry",
                path.display()
            ),
        )
    })?;
    let mut fields = Vec::new();
    let mut build_aliases = BTreeSet::new();
    let mut string_bindings = BTreeMap::new();
    collect_metadata_fields(
        build_body,
        &parsed,
        &mut build_aliases,
        &mut string_bindings,
        &mut fields,
    );
    Ok(fields)
}

pub fn extract_package_dependencies_from_build(
    path: &Path,
) -> Result<Vec<ExtractedPackageDependencyDecl>, PackageError> {
    let parsed = parse_build_package_for_metadata(path)?;
    let build_body = canonical_build_body(&parsed).ok_or_else(|| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "build.fol '{}' must declare exactly one canonical `pro[] build(): non` entry",
                path.display()
            ),
        )
    })?;
    let mut dependencies = Vec::new();
    let mut build_aliases = BTreeSet::new();
    let mut string_bindings = BTreeMap::new();
    collect_dependency_decls(
        build_body,
        &parsed,
        &mut build_aliases,
        &mut string_bindings,
        &mut dependencies,
    );
    Ok(dependencies)
}

fn parse_build_package_for_metadata(path: &Path) -> Result<ParsedPackage, PackageError> {
    let path_str = path.to_str().ok_or_else(|| {
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!("package build file '{}' is not valid UTF-8", path.display()),
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
    parser.parse_package(&mut lexer).map_err(|diagnostics| {
        let message = diagnostics
            .into_iter()
            .next()
            .map(|d| d.message)
            .unwrap_or_else(|| "unknown parse error".to_string());
        PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package loader could not parse package build file '{}': {}",
                path.display(),
                message
            ),
        )
    })
}

fn canonical_build_body<'a>(parsed: &'a ParsedPackage) -> Option<&'a [AstNode]> {
    let mut found = None;
    for unit in &parsed.source_units {
        for item in &unit.items {
            let AstNode::ProDecl {
                name,
                params,
                return_type,
                body,
                ..
            } = &item.node
            else {
                continue;
            };
            if name != "build" || !params.is_empty() {
                continue;
            }
            let Some(return_type_name) = (match return_type.as_ref() {
                Some(fol_parser::ast::FolType::None) => Some("non".to_string()),
                Some(ty) => ty.named_text(),
                None => None,
            }) else {
                continue;
            };
            if return_type_name != "non" && return_type_name != "none" {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(body.as_slice());
        }
    }
    found
}

fn collect_metadata_fields(
    nodes: &[AstNode],
    parsed: &ParsedPackage,
    build_aliases: &mut BTreeSet<String>,
    string_bindings: &mut BTreeMap<String, String>,
    fields: &mut Vec<ExtractedPackageMetadataField>,
) {
    for node in nodes {
        match node {
            AstNode::VarDecl {
                name,
                value: Some(value),
                ..
            } => {
                if is_ambient_build(value) {
                    build_aliases.insert(name.clone());
                } else if let Some(literal) = resolve_string_value(value, string_bindings) {
                    string_bindings.insert(name.clone(), literal);
                }
            }
            AstNode::MethodCall {
                object,
                method,
                args,
                ..
            } if method == "meta" => {
                if !is_build_receiver(object, build_aliases) {
                    continue;
                }
                let [AstNode::RecordInit { syntax_id, fields: record_fields, .. }] = &args[..] else {
                    continue;
                };
                let origin = syntax_id.and_then(|id| parsed.syntax_index.origin(id).cloned());
                for field in record_fields {
                    if let Some(value) = resolve_string_value(&field.value, string_bindings) {
                        fields.push(ExtractedPackageMetadataField {
                            name: field.name.clone(),
                            value,
                            origin: origin.clone(),
                        });
                    }
                }
            }
            AstNode::When { cases, default, .. } => {
                for case in cases {
                    if let fol_parser::ast::WhenCase::Case { body, .. } = case {
                        let mut aliases = build_aliases.clone();
                        let mut bindings = string_bindings.clone();
                        collect_metadata_fields(body, parsed, &mut aliases, &mut bindings, fields);
                    }
                }
                if let Some(default_body) = default {
                    let mut aliases = build_aliases.clone();
                    let mut bindings = string_bindings.clone();
                    collect_metadata_fields(default_body, parsed, &mut aliases, &mut bindings, fields);
                }
            }
            AstNode::Loop { body, .. } | AstNode::Defer { body, .. } => {
                let mut aliases = build_aliases.clone();
                let mut bindings = string_bindings.clone();
                collect_metadata_fields(body, parsed, &mut aliases, &mut bindings, fields);
            }
            AstNode::Block { statements, .. } => {
                let mut aliases = build_aliases.clone();
                let mut bindings = string_bindings.clone();
                collect_metadata_fields(statements, parsed, &mut aliases, &mut bindings, fields);
            }
            _ => {}
        }
    }
}

fn collect_dependency_decls(
    nodes: &[AstNode],
    parsed: &ParsedPackage,
    build_aliases: &mut BTreeSet<String>,
    string_bindings: &mut BTreeMap<String, String>,
    dependencies: &mut Vec<ExtractedPackageDependencyDecl>,
) {
    for node in nodes {
        match node {
            AstNode::VarDecl {
                name,
                value: Some(value),
                ..
            } => {
                if is_ambient_build(value) {
                    build_aliases.insert(name.clone());
                } else if let Some(literal) = resolve_string_value(value, string_bindings) {
                    string_bindings.insert(name.clone(), literal);
                }
            }
            AstNode::MethodCall {
                object,
                method,
                args,
                ..
            } if method == "add_dep" => {
                if !is_build_receiver(object, build_aliases) {
                    continue;
                }
                let [AstNode::RecordInit { syntax_id, fields: record_fields, .. }] = &args[..] else {
                    continue;
                };
                let origin = syntax_id.and_then(|id| parsed.syntax_index.origin(id).cloned());
                let mut alias = None;
                let mut source = None;
                let mut target = None;
                let mut evaluation_mode = DependencyBuildEvaluationMode::Eager;
                let mut git_version = None;
                let mut git_hash = None;
                for field in record_fields {
                    let value = resolve_string_value(&field.value, string_bindings);
                    match field.name.as_str() {
                        "alias" => alias = value,
                        "source" => source = value,
                        "target" => target = value,
                        "version" => {
                            git_version = value;
                        }
                        "hash" => git_hash = value,
                        "mode" => {
                            if let Some(value) = value {
                                if let Some(parsed_mode) =
                                    DependencyBuildEvaluationMode::parse(value.as_str())
                                {
                                    evaluation_mode = parsed_mode;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if let (Some(alias), Some(source), Some(target)) = (alias, source, target) {
                    dependencies.push(ExtractedPackageDependencyDecl {
                        alias,
                        source,
                        target,
                        evaluation_mode,
                        git_version,
                        git_hash,
                        origin,
                    });
                }
            }
            AstNode::When { cases, default, .. } => {
                for case in cases {
                    if let fol_parser::ast::WhenCase::Case { body, .. } = case {
                        let mut aliases = build_aliases.clone();
                        let mut bindings = string_bindings.clone();
                        collect_dependency_decls(
                            body,
                            parsed,
                            &mut aliases,
                            &mut bindings,
                            dependencies,
                        );
                    }
                }
                if let Some(default_body) = default {
                    let mut aliases = build_aliases.clone();
                    let mut bindings = string_bindings.clone();
                    collect_dependency_decls(
                        default_body,
                        parsed,
                        &mut aliases,
                        &mut bindings,
                        dependencies,
                    );
                }
            }
            AstNode::Loop { body, .. } | AstNode::Defer { body, .. } => {
                let mut aliases = build_aliases.clone();
                let mut bindings = string_bindings.clone();
                collect_dependency_decls(body, parsed, &mut aliases, &mut bindings, dependencies);
            }
            AstNode::Block { statements, .. } => {
                let mut aliases = build_aliases.clone();
                let mut bindings = string_bindings.clone();
                collect_dependency_decls(
                    statements,
                    parsed,
                    &mut aliases,
                    &mut bindings,
                    dependencies,
                );
            }
            _ => {}
        }
    }
}

fn materialize_package_metadata_from_build(
    path: &Path,
    fields: Vec<ExtractedPackageMetadataField>,
    dependencies: Vec<ExtractedPackageDependencyDecl>,
) -> Result<PackageMetadata, PackageError> {
    let supported_fields = BTreeSet::from(["name", "version", "kind", "description", "license"]);
    let mut values = BTreeMap::<String, (String, Option<SyntaxOrigin>)>::new();

    for field in fields {
        if !supported_fields.contains(field.name.as_str()) {
            let error = match field.origin {
                Some(origin) => PackageError::with_origin(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "unsupported package metadata field '{}'; expected only name, version, kind, description, or license",
                        field.name
                    ),
                    origin,
                ),
                None => PackageError::new(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "unsupported package metadata field '{}'; expected only name, version, kind, description, or license",
                        field.name
                    ),
                ),
            };
            return Err(error);
        }
        if let Some((_, first_origin)) = values.insert(
            field.name.clone(),
            (field.value.clone(), field.origin.clone()),
        ) {
            let error = match field.origin {
                Some(origin) => PackageError::with_origin(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "package metadata field '{}' may only be declared once",
                        field.name
                    ),
                    origin,
                ),
                None => PackageError::new(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "package metadata field '{}' may only be declared once",
                        field.name
                    ),
                ),
            };
            return Err(match first_origin {
                Some(origin) => error
                    .with_related_origin(origin, "first package metadata field declaration"),
                None => error,
            });
        }
    }

    let name = values
        .remove("name")
        .map(|(value, _)| value)
        .ok_or_else(|| {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package metadata '{}' is missing required field 'name'",
                    path.display()
                ),
            )
        })?;
    if !is_valid_package_name(&name) {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package metadata '{}' has invalid package name '{}'; package names must follow namespace identifier rules",
                path.display(),
                name
            ),
        ));
    }

    let version = values
        .remove("version")
        .map(|(value, _)| value)
        .ok_or_else(|| {
            PackageError::new(
                PackageErrorKind::InvalidInput,
                format!(
                    "package metadata '{}' is missing required field 'version'",
                    path.display()
                ),
            )
        })?;
    if version.trim().is_empty() {
        return Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package metadata '{}' has an empty version string",
                path.display()
            ),
        ));
    }

    let mut materialized_deps = Vec::new();
    let mut dep_aliases = BTreeMap::<String, Option<SyntaxOrigin>>::new();
    for dependency in dependencies {
        if let Some(first_origin) =
            dep_aliases.insert(dependency.alias.clone(), dependency.origin.clone())
        {
            let error = match dependency.origin {
                Some(origin) => PackageError::with_origin(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "package dependency alias '{}' in '{}' may only be declared once",
                        dependency.alias,
                        path.display()
                    ),
                    origin,
                ),
                None => PackageError::new(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "package dependency alias '{}' in '{}' may only be declared once",
                        dependency.alias,
                        path.display()
                    ),
                ),
            };
            return Err(match first_origin {
                Some(origin) => error
                    .with_related_origin(origin, "first package dependency alias declaration"),
                None => error,
            });
        }
        let source_target = format!("{}:{}", dependency.source, dependency.target);
        let dependency_origin = dependency.origin.clone().unwrap_or_else(|| SyntaxOrigin {
            file: Some(path.to_string_lossy().to_string()),
            line: 1,
            column: 1,
            length: dependency.alias.len().max(1),
        });
        let mut parsed = parse_dependency_decl(
            path,
            &dependency.alias,
            &source_target,
            dependency_origin.clone(),
        )?;
        parsed.evaluation_mode = dependency.evaluation_mode;
        parsed.git_version = match dependency.git_version.as_deref() {
            Some(raw) => Some(
                fol_build::GitDependencyVersionSelector::parse(raw).ok_or_else(|| {
                    PackageError::with_origin(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency '{}' in '{}' must use version 'branch:<name>', 'tag:<name>', or 'commit:<sha>' (got '{}')",
                            parsed.alias,
                            path.display(),
                            raw
                        ),
                        dependency_origin.clone(),
                    )
                })?,
            ),
            None => None,
        };
        parsed.git_hash = match dependency.git_hash.clone() {
            Some(hash) if hash.trim().is_empty() => {
                return Err(PackageError::with_origin(
                    PackageErrorKind::InvalidInput,
                    format!(
                        "package dependency '{}' in '{}' must not use an empty git hash",
                        parsed.alias,
                        path.display()
                    ),
                    dependency_origin.clone(),
                ));
            }
            other => other,
        };
        match parsed.source_kind {
            PackageDependencySourceKind::Git => {}
            PackageDependencySourceKind::Internal => {
                if parsed.target != "standard" {
                    return Err(PackageError::with_origin(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency '{}' in '{}' may use internal source only with target 'standard'",
                            parsed.alias,
                            path.display()
                        ),
                        dependency_origin.clone(),
                    ));
                }
            }
            _ => {
                if parsed.git_version.is_some() {
                    return Err(PackageError::with_origin(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency '{}' in '{}' may use 'version' only for git sources",
                            parsed.alias,
                            path.display()
                        ),
                        dependency_origin.clone(),
                    ));
                }
                if parsed.git_hash.is_some() {
                    return Err(PackageError::with_origin(
                        PackageErrorKind::InvalidInput,
                        format!(
                            "package dependency '{}' in '{}' may use 'hash' only for git sources",
                            parsed.alias,
                            path.display()
                        ),
                        dependency_origin.clone(),
                    ));
                }
            }
        }
        materialized_deps.push(parsed);
    }

    Ok(PackageMetadata {
        name,
        version,
        kind: non_empty_optional_field(
            path,
            "kind",
            values.remove("kind").map(|(value, _)| value),
        )?,
        description: non_empty_optional_field(
            path,
            "description",
            values.remove("description").map(|(value, _)| value),
        )?,
        license: non_empty_optional_field(
            path,
            "license",
            values.remove("license").map(|(value, _)| value),
        )?,
        dependencies: materialized_deps,
    })
}

fn is_ambient_build(node: &AstNode) -> bool {
    matches!(
        node,
        AstNode::FunctionCall {
            surface: CallSurface::DotIntrinsic,
            name,
            args,
            ..
        } if name == "build" && args.is_empty()
    )
}

fn is_build_receiver(node: &AstNode, build_aliases: &BTreeSet<String>) -> bool {
    is_ambient_build(node)
        || matches!(
            node,
            AstNode::Identifier { name, .. } if build_aliases.contains(name)
        )
}

fn resolve_string_value(node: &AstNode, string_bindings: &BTreeMap<String, String>) -> Option<String> {
    match node {
        AstNode::Literal(fol_parser::ast::Literal::String(value)) => Some(value.clone()),
        // Single-element double-quoted literals width-classify as characters;
        // metadata string fields accept them as one-char strings.
        AstNode::Literal(fol_parser::ast::Literal::Character(value)) => Some(value.to_string()),
        AstNode::Identifier { name, .. } => string_bindings.get(name).cloned(),
        _ => None,
    }
}

fn parse_dependency_decl(
    path: &Path,
    alias: &str,
    value: &str,
    origin: SyntaxOrigin,
) -> Result<PackageDependencyDecl, PackageError> {
    if !is_valid_package_name(alias) {
        return Err(PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            format!(
                "package metadata '{}' has invalid dependency alias '{}'; dependency aliases must follow namespace identifier rules",
                path.display(),
                alias
            ),
            origin,
        ));
    }
    let Some((source_raw, target_raw)) = value.split_once(':') else {
        return Err(PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            format!(
                "package dependency '{}' in '{}' must use 'source:target' form",
                alias,
                path.display()
            ),
            origin,
        ));
    };
    let source_kind = match source_raw.trim() {
        "loc" => PackageDependencySourceKind::Local,
        "pkg" => PackageDependencySourceKind::PackageStore,
        "git" => PackageDependencySourceKind::Git,
        "internal" => PackageDependencySourceKind::Internal,
        other => {
            return Err(PackageError::with_origin(
                PackageErrorKind::InvalidInput,
                format!(
                    "package dependency '{}' in '{}' uses unsupported source '{}'; expected loc, pkg, git, or internal",
                    alias,
                    path.display(),
                    other
                ),
                origin,
            ))
        }
    };
    let target = target_raw.trim();
    if target.is_empty() {
        return Err(PackageError::with_origin(
            PackageErrorKind::InvalidInput,
            format!(
                "package dependency '{}' in '{}' has an empty target",
                alias,
                path.display()
            ),
            origin,
        ));
    }
    Ok(PackageDependencyDecl {
        alias: alias.to_string(),
        source_kind,
        target: target.to_string(),
        evaluation_mode: DependencyBuildEvaluationMode::Eager,
        git_version: None,
        git_hash: None,
    })
}

fn non_empty_optional_field(
    path: &Path,
    field_name: &str,
    value: Option<String>,
) -> Result<Option<String>, PackageError> {
    match value {
        Some(value) if value.trim().is_empty() => Err(PackageError::new(
            PackageErrorKind::InvalidInput,
            format!(
                "package metadata '{}' has an empty '{}' field",
                path.display(),
                field_name
            ),
        )),
        other => Ok(other),
    }
}

fn is_valid_package_name(package_name: &str) -> bool {
    if package_name.len() > 256 {
        return false;
    }
    let mut chars = package_name.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !first.is_ascii() || first.is_ascii_digit() || !(first.is_ascii_alphabetic() || first == '_')
    {
        return false;
    }

    if package_name.contains("__") {
        return false;
    }

    chars.all(|ch| ch.is_ascii() && (ch.is_ascii_alphanumeric() || ch == '_'))
}

#[cfg(test)]
mod tests {
    use super::{
        extract_package_dependencies_from_build, extract_package_metadata_fields_from_build,
        parse_package_metadata_from_build, PackageDependencyDecl, PackageDependencySourceKind,
        PackageMetadata,
    };
    use fol_build::DependencyBuildEvaluationMode;
    use fol_diagnostics::ToDiagnostic;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(label: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("System time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "fol_package_metadata_{}_{}_{}",
            label,
            std::process::id(),
            stamp
        ))
    }

    fn write_build_fixture(label: &str, source: &str) -> std::path::PathBuf {
        let temp_root = unique_temp_root(label);
        fs::create_dir_all(&temp_root).expect("Should create temporary build fixture root");
        let build_path = temp_root.join("build.fol");
        fs::write(&build_path, source).expect("Should write the build fixture");
        build_path
    }

    #[test]
    fn build_metadata_extractor_reads_direct_meta_fields() {
        let build_path = write_build_fixture(
            "build_meta_direct",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({\n",
                "        name = \"demo\",\n",
                "        version = \"1.0.0\",\n",
                "        kind = \"exe\",\n",
                "    });\n",
                "};\n",
            ),
        );

        let fields = extract_package_metadata_fields_from_build(&build_path)
            .expect("build metadata should extract from direct meta call");

        assert!(fields.iter().any(|field| field.name == "name" && field.value == "demo"));
        assert!(fields
            .iter()
            .any(|field| field.name == "version" && field.value == "1.0.0"));
        assert!(fields.iter().any(|field| field.name == "kind" && field.value == "exe"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_metadata_extractor_reads_string_bindings_through_build_local() {
        let build_path = write_build_fixture(
            "build_meta_local_bindings",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    var name = \"demo\";\n",
                "    var version = \"1.0.0\";\n",
                "    build.meta({\n",
                "        name = name,\n",
                "        version = version,\n",
                "    });\n",
                "};\n",
            ),
        );

        let fields = extract_package_metadata_fields_from_build(&build_path)
            .expect("build metadata should extract through inferred build locals");

        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|field| field.name == "name" && field.value == "demo"));
        assert!(fields
            .iter()
            .any(|field| field.name == "version" && field.value == "1.0.0"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_metadata_materializer_requires_name_and_version() {
        let build_path = write_build_fixture(
            "build_meta_missing_required",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({\n",
                "        description = \"demo\",\n",
                "    });\n",
                "};\n",
            ),
        );

        let error = parse_package_metadata_from_build(&build_path)
            .expect_err("missing required build metadata should be rejected");

        assert!(error
            .to_string()
            .contains("missing required field 'name'"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_metadata_materializer_rejects_duplicate_fields() {
        let build_path = write_build_fixture(
            "build_meta_duplicate_field",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"1.0.0\" });\n",
                "    build.meta({ name = \"other\" });\n",
                "};\n",
            ),
        );

        let diagnostic = parse_package_metadata_from_build(&build_path)
            .expect_err("duplicate build metadata fields should be rejected")
            .to_diagnostic();

        assert_eq!(diagnostic.labels.len(), 2);
        assert_eq!(
            diagnostic.labels[1].message.as_deref(),
            Some("first package metadata field declaration")
        );

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_metadata_materializer_rejects_invalid_package_names() {
        let build_path = write_build_fixture(
            "build_meta_invalid_name",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"9demo\", version = \"1.0.0\" });\n",
                "};\n",
            ),
        );

        let error = parse_package_metadata_from_build(&build_path)
            .expect_err("invalid package names should be rejected");

        assert!(error.to_string().contains("invalid package name '9demo'"));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_dependency_extractor_reads_direct_dependency_configs() {
        let build_path = write_build_fixture(
            "build_dep_extract_direct",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core/tools\" });\n",
                "    build.add_dep({ alias = \"shared\", source = \"loc\", target = \"../shared\" });\n",
                "};\n",
            ),
        );

        let deps = extract_package_dependencies_from_build(&build_path)
            .expect("build dependencies should extract from direct calls");

        assert_eq!(deps.len(), 2);
        assert!(deps
            .iter()
            .any(|dep| dep.alias == "core"
                && dep.source == "pkg"
                && dep.target == "core/tools"
                && dep.evaluation_mode == DependencyBuildEvaluationMode::Eager));
        assert!(deps
            .iter()
            .any(|dep| dep.alias == "shared"
                && dep.source == "loc"
                && dep.target == "../shared"
                && dep.evaluation_mode == DependencyBuildEvaluationMode::Eager));

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_metadata_materializer_materializes_direct_dependencies() {
        let build_path = write_build_fixture(
            "build_dep_materialize",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"1.0.0\" });\n",
                "    build.add_dep({ alias = \"core\", source = \"pkg\", target = \"core\", mode = \"lazy\" });\n",
                "    build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"https://github.com/bresilla/logtiny.git\", mode = \"on-demand\" });\n",
                "};\n",
            ),
        );

        let metadata = parse_package_metadata_from_build(&build_path)
            .expect("build metadata should materialize direct dependencies");

        assert_eq!(metadata.name, "demo");
        assert_eq!(metadata.version, "1.0.0");
        assert_eq!(metadata.dependencies.len(), 2);
        assert_eq!(metadata.dependencies[0].alias, "core");
        assert_eq!(
            metadata.dependencies[0].source_kind,
            PackageDependencySourceKind::PackageStore
        );
        assert_eq!(
            metadata.dependencies[0].evaluation_mode,
            DependencyBuildEvaluationMode::Lazy
        );
        assert_eq!(metadata.dependencies[1].alias, "logtiny");
        assert_eq!(
            metadata.dependencies[1].source_kind,
            PackageDependencySourceKind::Git
        );
        assert_eq!(
            metadata.dependencies[1].evaluation_mode,
            DependencyBuildEvaluationMode::OnDemand
        );
        assert_eq!(metadata.dependencies[1].git_version, None);
        assert_eq!(metadata.dependencies[1].git_hash, None);

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_metadata_materializer_keeps_internal_standard_dependencies() {
        let build_path = write_build_fixture(
            "build_dep_internal_standard",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"1.0.0\" });\n",
                "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
                "};\n",
            ),
        );

        let metadata = parse_package_metadata_from_build(&build_path)
            .expect("internal standard dependency should materialize");

        assert_eq!(metadata.dependencies.len(), 1);
        assert_eq!(metadata.dependencies[0].alias, "std");
        assert_eq!(
            metadata.dependencies[0].source_kind,
            PackageDependencySourceKind::Internal
        );
        assert_eq!(metadata.dependencies[0].target, "standard");

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn build_metadata_materializer_keeps_structured_git_dependency_fields() {
        let build_path = write_build_fixture(
            "build_dep_materialize_git_fields",
            concat!(
                "pro[] build(): non = {\n",
                "    var build = .build();\n",
                "    build.meta({ name = \"demo\", version = \"1.0.0\" });\n",
                "    build.add_dep({ alias = \"logtiny\", source = \"git\", target = \"git+https://github.com/bresilla/logtiny.git\", version = \"tag:v0.1.2\", hash = \"f49abfa1038f\" });\n",
                "};\n",
            ),
        );

        let metadata = parse_package_metadata_from_build(&build_path)
            .expect("build metadata should materialize structured git dependency fields");

        assert_eq!(metadata.dependencies.len(), 1);
        // NOTE: the materializer stores the raw declared target and does not
        // strip the `git+` scheme prefix; the locator layer re-parses it. This
        // pins the current crate behavior (suspected product gap: no `git+`
        // normalization at materialization time).
        assert_eq!(
            metadata.dependencies[0].target,
            "git+https://github.com/bresilla/logtiny.git"
        );
        assert_eq!(
            metadata.dependencies[0].git_version,
            Some(fol_build::GitDependencyVersionSelector::Tag(
                "v0.1.2".to_string()
            ))
        );
        assert_eq!(
            metadata.dependencies[0].git_hash.as_deref(),
            Some("f49abfa1038f")
        );
        assert_eq!(
            metadata.dependencies[0].git_locator_string(),
            "git+https://github.com/bresilla/logtiny.git#tag:v0.1.2#hash:f49abfa1038f"
        );

        fs::remove_dir_all(build_path.parent().unwrap()).ok();
    }

    #[test]
    fn package_name_validation_rejects_names_longer_than_256_characters() {
        use super::is_valid_package_name;

        let long_name = "a".repeat(257);
        assert!(!is_valid_package_name(&long_name));
        let max_name = "a".repeat(256);
        assert!(is_valid_package_name(&max_name));
    }

}

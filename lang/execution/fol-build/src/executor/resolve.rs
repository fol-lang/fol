use crate::api::PathHandleClass;
use crate::artifact::BuildArtifactFolModel;
use crate::eval::{BuildEvaluationError, BuildEvaluationErrorKind};
use crate::runtime::BuildRuntimeGeneratedFileKind;
use fol_parser::ast::{AstNode, Literal, RecordInitField};
use std::collections::BTreeMap;

use super::core::BuildBodyExecutor;
use super::types::{ExecArtifact, ExecConfigValue, ExecValue, ResolvedPathHandle};

impl BuildBodyExecutor {
    fn generated_handle_class(kind: BuildRuntimeGeneratedFileKind) -> PathHandleClass {
        match kind {
            BuildRuntimeGeneratedFileKind::GeneratedDir => PathHandleClass::Dir,
            BuildRuntimeGeneratedFileKind::Write
            | BuildRuntimeGeneratedFileKind::Copy
            | BuildRuntimeGeneratedFileKind::ToolOutput
            | BuildRuntimeGeneratedFileKind::CodegenOutput => PathHandleClass::File,
        }
    }

    fn resolve_exec_path_handle(value: &ExecValue) -> Option<ResolvedPathHandle> {
        match value {
            ExecValue::SourceFile { path, provenance } => {
                Some(ResolvedPathHandle::file(path.clone(), *provenance))
            }
            ExecValue::SourceDir { path, provenance } => {
                Some(ResolvedPathHandle::dir(path.clone(), *provenance))
            }
            ExecValue::GeneratedFile {
                name,
                path,
                kind,
                provenance,
            } => {
                let mut resolved =
                    ResolvedPathHandle::generated(path.clone(), *provenance, name.clone());
                resolved.descriptor.class = Self::generated_handle_class(*kind);
                Some(resolved)
            }
            _ => None,
        }
    }

    pub(super) fn resolve_path_handle(&self, node: &AstNode) -> Option<ResolvedPathHandle> {
        let AstNode::Identifier { name, .. } = node else {
            return None;
        };
        self.scope
            .get(name.as_str())
            .and_then(Self::resolve_exec_path_handle)
    }

    pub(super) fn resolve_string(&self, node: &AstNode) -> Option<String> {
        match node {
            AstNode::Literal(Literal::String(s)) => Some(s.clone()),
            // Single-element double-quoted literals width-classify as
            // characters; in string positions they are one-char strings.
            AstNode::Literal(Literal::Character(c)) => Some(c.to_string()),
            AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                Some(ExecValue::Target(s)) => Some(s.clone()),
                Some(ExecValue::Optimize(s)) => Some(s.clone()),
                Some(ExecValue::Str(s)) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn resolve_field_string(
        &self,
        fields: &[RecordInitField],
        field_name: &str,
    ) -> Option<String> {
        fields
            .iter()
            .find(|f| f.name == field_name)
            .and_then(|f| self.resolve_string(&f.value))
    }

    pub(super) fn parse_config_value(
        &self,
        node: &AstNode,
        allowed_kinds: &[&str],
    ) -> Option<ExecConfigValue> {
        match node {
            AstNode::Literal(Literal::String(s)) => Some(ExecConfigValue::Literal(s.clone())),
            AstNode::Literal(Literal::Character(c)) => {
                Some(ExecConfigValue::Literal(c.to_string()))
            }
            AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                Some(ExecValue::Target(option_name)) if allowed_kinds.contains(&"target") => {
                    Some(ExecConfigValue::OptionRef {
                        name: option_name.clone(),
                        kind: crate::graph::BuildOptionKind::Target,
                    })
                }
                Some(ExecValue::Optimize(option_name)) if allowed_kinds.contains(&"optimize") => {
                    Some(ExecConfigValue::OptionRef {
                        name: option_name.clone(),
                        kind: crate::graph::BuildOptionKind::Optimize,
                    })
                }
                Some(ExecValue::OptionRef { name, kind })
                    if option_kind_is_allowed(*kind, allowed_kinds) =>
                {
                    Some(ExecConfigValue::OptionRef {
                        name: name.clone(),
                        kind: *kind,
                    })
                }
                Some(ExecValue::Str(s)) => Some(ExecConfigValue::Literal(s.clone())),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn resolve_artifact_ref(&self, node: &AstNode) -> Option<ExecArtifact> {
        match node {
            AstNode::Literal(Literal::Character(c)) => Some(ExecArtifact {
                name: c.to_string(),
                root_module: ExecConfigValue::Literal(String::new()),
                fol_model: BuildArtifactFolModel::Memo,
                target: None,
                optimize: None,
            }),
            AstNode::Literal(Literal::String(s)) => Some(ExecArtifact {
                name: s.clone(),
                root_module: ExecConfigValue::Literal(String::new()),
                fol_model: BuildArtifactFolModel::Memo,
                target: None,
                optimize: None,
            }),
            AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                Some(ExecValue::Artifact(a)) => Some(a.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn resolve_step_ref(&self, node: &AstNode) -> Option<String> {
        match node {
            AstNode::Literal(Literal::String(s)) => Some(s.clone()),
            // Single-element double-quoted literals width-classify as
            // characters; in string positions they are one-char strings.
            AstNode::Literal(Literal::Character(c)) => Some(c.to_string()),
            AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                Some(ExecValue::Step { name }) => Some(name.clone()),
                Some(ExecValue::Run { name }) => Some(name.clone()),
                Some(ExecValue::Install { name }) => Some(name.clone()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn resolve_dependency_args(
        &mut self,
        fields: &[RecordInitField],
    ) -> Result<
        Option<BTreeMap<String, crate::api::DependencyArgValue>>,
        crate::eval::BuildEvaluationError,
    > {
        let Some(args_field) = fields.iter().find(|field| field.name == "args") else {
            return Ok(None);
        };
        let AstNode::RecordInit {
            fields: arg_fields, ..
        } = &args_field.value
        else {
            return Err(crate::eval::BuildEvaluationError::new(
                crate::eval::BuildEvaluationErrorKind::InvalidInput,
                "build.add_dep config is invalid: dependency 'args' must be a record".to_string(),
            ));
        };
        let mut args = BTreeMap::new();
        for field in arg_fields {
            // Integer args are matched syntactically: the expression
            // evaluator has no integer value class of its own.
            if let AstNode::Literal(Literal::Integer(value)) = &field.value {
                args.insert(
                    field.name.clone(),
                    crate::api::DependencyArgValue::Int(*value),
                );
                continue;
            }
            let value = match self.eval_expr(&field.value)? {
                Some(ExecValue::Bool(value)) => crate::api::DependencyArgValue::Bool(value),
                Some(ExecValue::Str(value)) => crate::api::DependencyArgValue::String(value),
                Some(ExecValue::Target(option_name))
                | Some(ExecValue::Optimize(option_name)) => {
                    crate::api::DependencyArgValue::OptionRef(option_name)
                }
                Some(ExecValue::OptionRef { name, .. }) => {
                    crate::api::DependencyArgValue::OptionRef(name)
                }
                Some(_) | None => {
                    return Err(crate::eval::BuildEvaluationError::new(
                        crate::eval::BuildEvaluationErrorKind::InvalidInput,
                        format!(
                            "build.add_dep config is invalid: dependency arg '{}' must evaluate to bool, int, str, or an option handle",
                            field.name
                        ),
                    ))
                }
            };
            args.insert(field.name.clone(), value);
        }
        Ok(Some(args))
    }

    pub(super) fn resolve_field_string_list(
        &self,
        fields: &[RecordInitField],
        field_name: &str,
    ) -> Result<Vec<String>, BuildEvaluationError> {
        let Some(field) = fields.iter().find(|field| field.name == field_name) else {
            return Ok(Vec::new());
        };
        let items = self.eval_iterable(&field.value)?;
        let mut resolved = Vec::with_capacity(items.len());
        for item in items {
            match item {
                ExecValue::Str(value) => resolved.push(value),
                _ => {
                    return Err(BuildEvaluationError::new(
                        BuildEvaluationErrorKind::InvalidInput,
                        format!(
                        "build config is invalid: '{field_name}' must contain only string values"
                    ),
                    ))
                }
            }
        }
        Ok(resolved)
    }

    pub(super) fn resolve_field_path_list(
        &self,
        fields: &[RecordInitField],
        field_name: &str,
    ) -> Result<Vec<String>, BuildEvaluationError> {
        let Some(field) = fields.iter().find(|field| field.name == field_name) else {
            return Ok(Vec::new());
        };
        let items = self.eval_iterable(&field.value)?;
        let mut resolved = Vec::with_capacity(items.len());
        for item in items {
            match item {
                ExecValue::SourceFile { path, .. } | ExecValue::GeneratedFile { path, .. } => {
                    resolved.push(path)
                }
                _ => {
                    return Err(BuildEvaluationError::new(
                        BuildEvaluationErrorKind::InvalidInput,
                        format!(
                            "build config is invalid: '{field_name}' must contain only source-file or generated-output handles"
                        ),
                    ))
                }
            }
        }
        Ok(resolved)
    }

    pub(super) fn resolve_field_string_map(
        &self,
        fields: &[RecordInitField],
        field_name: &str,
    ) -> Result<BTreeMap<String, String>, BuildEvaluationError> {
        let Some(field) = fields.iter().find(|field| field.name == field_name) else {
            return Ok(BTreeMap::new());
        };
        let AstNode::RecordInit {
            fields: map_fields, ..
        } = &field.value
        else {
            return Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!("build config is invalid: '{field_name}' must be a record"),
            ));
        };
        let mut resolved = BTreeMap::new();
        for map_field in map_fields {
            let Some(value) = self.resolve_string(&map_field.value) else {
                return Err(BuildEvaluationError::new(
                    BuildEvaluationErrorKind::InvalidInput,
                    format!("build config is invalid: '{field_name}' values must be strings"),
                ));
            };
            resolved.insert(map_field.name.clone(), value);
        }
        Ok(resolved)
    }
}

fn option_kind_is_allowed(kind: crate::graph::BuildOptionKind, allowed: &[&str]) -> bool {
    let label = match kind {
        crate::graph::BuildOptionKind::Target => "target",
        crate::graph::BuildOptionKind::Optimize => "optimize",
        crate::graph::BuildOptionKind::Bool => "bool",
        crate::graph::BuildOptionKind::Int => "int",
        crate::graph::BuildOptionKind::String => "string",
        crate::graph::BuildOptionKind::Enum => "enum",
        crate::graph::BuildOptionKind::Path => "path",
    };
    allowed.contains(&label)
}

#[cfg(test)]
mod tests {
    use super::{BuildBodyExecutor, ExecValue};
    use crate::api::{PathHandleClass, PathHandleProvenance};
    use crate::runtime::BuildRuntimeGeneratedFileKind;

    #[test]
    fn c_import_path_resolution_uses_explicit_provenance_not_name_prefixes() {
        let prefix_shaped_local = ExecValue::GeneratedFile {
            name: "dep::looks::remote".to_string(),
            path: "gen/local.o".to_string(),
            kind: BuildRuntimeGeneratedFileKind::ToolOutput,
            provenance: PathHandleProvenance::Generated,
        };
        let dependency_file_without_a_prefix = ExecValue::SourceFile {
            path: "plain/provider.o".to_string(),
            provenance: PathHandleProvenance::DependencyFile,
        };

        let local = BuildBodyExecutor::resolve_exec_path_handle(&prefix_shaped_local)
            .expect("generated values should resolve");
        let dependency =
            BuildBodyExecutor::resolve_exec_path_handle(&dependency_file_without_a_prefix)
                .expect("dependency files should resolve");

        assert_eq!(local.generated_name.as_deref(), Some("dep::looks::remote"));
        assert_eq!(local.descriptor.class, PathHandleClass::File);
        assert_eq!(local.descriptor.provenance, PathHandleProvenance::Generated);
        assert_eq!(dependency.generated_name, None);
        assert_eq!(dependency.descriptor.class, PathHandleClass::File);
        assert_eq!(
            dependency.descriptor.provenance,
            PathHandleProvenance::DependencyFile
        );
    }
}

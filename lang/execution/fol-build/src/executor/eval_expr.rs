use crate::eval::{BuildEvaluationError, BuildEvaluationErrorKind};
use fol_parser::ast::{AstNode, BinaryOperator, CallSurface, Literal};
use std::collections::BTreeMap;

use super::core::{BuildBodyExecutor, MAX_EVAL_DEPTH};
use super::types::ExecValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum WhenValue {
    String(String),
    Bool(bool),
    Int(i64),
    Nil,
}

impl BuildBodyExecutor {
    fn build_node_label(node: &AstNode) -> &'static str {
        match node {
            AstNode::Identifier { .. } => "identifier",
            AstNode::FunctionCall { .. } => "function_call",
            AstNode::MethodCall { .. } => "method_call",
            AstNode::ContainerLiteral { .. } => "container_literal",
            AstNode::Literal(_) => "literal",
            AstNode::BinaryOp { .. } => "binary_op",
            AstNode::UnaryOp { .. } => "unary_op",
            AstNode::When { .. } => "when",
            AstNode::Loop { .. } => "loop",
            AstNode::Assignment { .. } => "assignment",
            AstNode::Return { .. } => "return",
            _ => "ast_node",
        }
    }

    pub(super) fn eval_iterable(
        &self,
        node: &AstNode,
    ) -> Result<Vec<ExecValue>, BuildEvaluationError> {
        match node {
            AstNode::ContainerLiteral { elements, .. } => {
                let mut result = Vec::with_capacity(elements.len());
                for elem in elements {
                    match elem {
                        AstNode::Literal(Literal::String(s)) => {
                            result.push(ExecValue::Str(s.clone()));
                        }
                        AstNode::Literal(Literal::Boolean(b)) => {
                            result.push(ExecValue::Bool(*b));
                        }
                        AstNode::Identifier { name, .. } => {
                            if let Some(v) = self.scope.get(name.as_str()) {
                                result.push(v.clone());
                            }
                        }
                        _ => {}
                    }
                }
                Ok(result)
            }
            AstNode::Identifier { name, .. } => match self.scope.get(name.as_str()) {
                Some(ExecValue::List(items)) => Ok(items.clone()),
                _ => Ok(Vec::new()),
            },
            _ => Ok(Vec::new()),
        }
    }

    pub(super) fn eval_condition(&self, cond: &AstNode) -> Result<bool, BuildEvaluationError> {
        match cond {
            AstNode::Literal(Literal::Boolean(b)) => Ok(*b),
            AstNode::Identifier { name, .. } => {
                if let Some(v) = self.scope.get(name.as_str()) {
                    match v {
                        ExecValue::Bool(b) => Ok(*b),
                        ExecValue::OptionRef {
                            name: option_name,
                            kind: crate::graph::BuildOptionKind::Bool,
                        } => match self.resolved_inputs.get(option_name.as_str()) {
                            Some(value) if value == "true" => Ok(true),
                            Some(value) if value == "false" => Ok(false),
                            Some(value) => Err(BuildEvaluationError::new(
                                BuildEvaluationErrorKind::InvalidInput,
                                format!(
                                    "resolved bool option '{option_name}' has invalid value '{value}'"
                                ),
                            )),
                            None => Ok(false),
                        },
                        ExecValue::OptionRef {
                            name: option_name,
                            kind,
                        } => Err(BuildEvaluationError::new(
                            BuildEvaluationErrorKind::InvalidInput,
                            format!(
                                "build condition option '{option_name}' must be bool, not {:?}",
                                kind
                            ),
                        )),
                        ExecValue::Target(option_name) | ExecValue::Optimize(option_name) => {
                            Err(BuildEvaluationError::new(
                                BuildEvaluationErrorKind::InvalidInput,
                                format!(
                                    "build condition option '{option_name}' must be compared to a value"
                                ),
                            ))
                        }
                        _ => Err(BuildEvaluationError::new(
                            BuildEvaluationErrorKind::InvalidInput,
                            format!("build condition identifier '{name}' is not boolean"),
                        )),
                    }
                } else {
                    Ok(false)
                }
            }
            AstNode::BinaryOp {
                op: BinaryOperator::Eq,
                left,
                right,
            } => {
                let lhs = self.eval_when_value(left)?;
                let rhs = self.eval_when_value(right)?;
                match (lhs, rhs) {
                    (Some(l), Some(r)) => Ok(l == r),
                    _ => Ok(false),
                }
            }
            AstNode::BinaryOp {
                op: BinaryOperator::Ne,
                left,
                right,
            } => {
                let lhs = self.eval_when_value(left)?;
                let rhs = self.eval_when_value(right)?;
                match (lhs, rhs) {
                    (Some(l), Some(r)) => Ok(l != r),
                    _ => Ok(false),
                }
            }
            AstNode::BinaryOp {
                op: BinaryOperator::And,
                left,
                right,
            } => Ok(self.eval_condition(left)? && self.eval_condition(right)?),
            AstNode::BinaryOp {
                op: BinaryOperator::Or,
                left,
                right,
            } => Ok(self.eval_condition(left)? || self.eval_condition(right)?),
            AstNode::UnaryOp {
                op: fol_parser::ast::UnaryOperator::Not,
                operand,
            } => Ok(!self.eval_condition(operand)?),
            other => Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!(
                    "build evaluation does not support condition node '{}'",
                    Self::build_node_label(other)
                ),
            )),
        }
    }

    pub(super) fn eval_when_value(
        &self,
        value: &AstNode,
    ) -> Result<Option<WhenValue>, BuildEvaluationError> {
        match value {
            AstNode::Literal(Literal::String(value)) => Ok(Some(WhenValue::String(value.clone()))),
            // Single-element double-quoted literals width-classify as
            // characters; build option matching still treats them as strings.
            AstNode::Literal(Literal::Character(value)) => {
                Ok(Some(WhenValue::String(value.to_string())))
            }
            AstNode::Literal(Literal::Boolean(value)) => Ok(Some(WhenValue::Bool(*value))),
            AstNode::Literal(Literal::Integer(value)) => Ok(Some(WhenValue::Int(*value))),
            AstNode::Literal(Literal::Nil) => Ok(Some(WhenValue::Nil)),
            AstNode::Identifier { name, .. } => {
                let Some(value) = self.scope.get(name.as_str()) else {
                    return Ok(None);
                };
                match value {
                    ExecValue::Str(value) => Ok(Some(WhenValue::String(value.clone()))),
                    ExecValue::Bool(value) => Ok(Some(WhenValue::Bool(*value))),
                    ExecValue::Target(option_name) | ExecValue::Optimize(option_name) => Ok(self
                        .resolved_inputs
                        .get(option_name.as_str())
                        .cloned()
                        .map(WhenValue::String)),
                    ExecValue::OptionRef {
                        name: option_name,
                        kind,
                    } => {
                        let Some(raw) = self.resolved_inputs.get(option_name.as_str()) else {
                            return Ok(None);
                        };
                        match kind {
                            crate::graph::BuildOptionKind::Bool => match raw.as_str() {
                                "true" => Ok(Some(WhenValue::Bool(true))),
                                "false" => Ok(Some(WhenValue::Bool(false))),
                                _ => Err(BuildEvaluationError::new(
                                    BuildEvaluationErrorKind::InvalidInput,
                                    format!(
                                        "resolved bool option '{option_name}' has invalid value '{raw}'"
                                    ),
                                )),
                            },
                            crate::graph::BuildOptionKind::Int => raw
                                .parse::<i64>()
                                .map(WhenValue::Int)
                                .map(Some)
                                .map_err(|_| {
                                    BuildEvaluationError::new(
                                        BuildEvaluationErrorKind::InvalidInput,
                                        format!(
                                            "resolved int option '{option_name}' has invalid value '{raw}'"
                                        ),
                                    )
                                }),
                            crate::graph::BuildOptionKind::Target
                            | crate::graph::BuildOptionKind::Optimize
                            | crate::graph::BuildOptionKind::String
                            | crate::graph::BuildOptionKind::Enum
                            | crate::graph::BuildOptionKind::Path => {
                                Ok(Some(WhenValue::String(raw.clone())))
                            }
                        }
                    }
                    _ => Err(BuildEvaluationError::new(
                        BuildEvaluationErrorKind::InvalidInput,
                        format!("build when selector '{name}' is not a matchable scalar value"),
                    )),
                }
            }
            AstNode::BinaryOp {
                op:
                    BinaryOperator::Eq | BinaryOperator::Ne | BinaryOperator::And | BinaryOperator::Or,
                ..
            }
            | AstNode::UnaryOp {
                op: fol_parser::ast::UnaryOperator::Not,
                ..
            } => Ok(Some(WhenValue::Bool(self.eval_condition(value)?))),
            other => Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!(
                    "build evaluation does not support when value node '{}'",
                    Self::build_node_label(other)
                ),
            )),
        }
    }

    pub(super) fn eval_expr(
        &mut self,
        expr: &AstNode,
    ) -> Result<Option<ExecValue>, BuildEvaluationError> {
        self.recursion_depth += 1;
        if self.recursion_depth > MAX_EVAL_DEPTH {
            self.recursion_depth -= 1;
            return Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!("build script exceeded maximum recursion depth ({MAX_EVAL_DEPTH})"),
            ));
        }
        let result = self.eval_expr_inner(expr);
        self.recursion_depth -= 1;
        result
    }

    fn eval_expr_inner(
        &mut self,
        expr: &AstNode,
    ) -> Result<Option<ExecValue>, BuildEvaluationError> {
        match expr {
            AstNode::Identifier { name, .. } => Ok(self.scope.get(name.as_str()).cloned()),

            AstNode::FunctionCall {
                surface: CallSurface::DotIntrinsic,
                name,
                args,
                ..
            } if name == "build" => {
                if !args.is_empty() {
                    return Err(self.unsupported(name));
                }
                Ok(Some(ExecValue::Build))
            }

            AstNode::FunctionCall {
                surface: CallSurface::DotIntrinsic,
                name,
                args,
                ..
            } if name == "graph" => {
                if !args.is_empty() {
                    return Err(self.unsupported(name));
                }
                Ok(Some(ExecValue::Graph))
            }

            AstNode::FunctionCall {
                surface: CallSurface::DotIntrinsic,
                name,
                args,
                ..
            } => {
                let Some(receiver) = self.last_value.clone() else {
                    return Err(self.unsupported(name));
                };
                self.eval_handle_method(receiver, name, args)
            }

            AstNode::MethodCall {
                object,
                method,
                args,
                ..
            } => {
                let Some(receiver) = self.eval_expr(object)? else {
                    return Ok(None);
                };
                if matches!(receiver, ExecValue::Graph) {
                    return self.eval_graph_method(method, args);
                }
                self.eval_handle_method(receiver, method, args)
            }

            AstNode::FunctionCall {
                surface: CallSurface::Plain,
                name,
                args,
                ..
            } => {
                // Could be a helper routine call
                if self.helpers.contains_key(name.as_str()) {
                    self.eval_helper_call(name, args)
                } else {
                    Err(BuildEvaluationError::new(
                        BuildEvaluationErrorKind::InvalidInput,
                        format!("unknown build helper routine '{name}'"),
                    ))
                }
            }

            AstNode::ContainerLiteral { elements, .. } => {
                let items = self.eval_iterable(&AstNode::ContainerLiteral {
                    container_type: fol_parser::ast::ContainerType::Array,
                    elements: elements.clone(),
                })?;
                Ok(Some(ExecValue::List(items)))
            }

            AstNode::Literal(Literal::String(s)) => Ok(Some(ExecValue::Str(s.clone()))),
            AstNode::Literal(Literal::Character(c)) => Ok(Some(ExecValue::Str(c.to_string()))),
            AstNode::Literal(Literal::Boolean(b)) => Ok(Some(ExecValue::Bool(*b))),

            other => Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!(
                    "build evaluation does not support expression node '{}'",
                    Self::build_node_label(other)
                ),
            )),
        }
    }

    pub(super) fn eval_helper_call(
        &mut self,
        name: &str,
        args: &[AstNode],
    ) -> Result<Option<ExecValue>, BuildEvaluationError> {
        // Evaluate args in current scope
        let evaluated_args: Vec<Option<ExecValue>> = args
            .iter()
            .map(|arg| self.eval_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;

        let Some(helper) = self.helpers.get(name) else {
            return Ok(None);
        };

        // Build a child scope with parameter bindings
        let mut child_scope: BTreeMap<String, ExecValue> = BTreeMap::new();

        for (param_name, value) in helper.params.iter().zip(evaluated_args.iter()) {
            if let Some(v) = value {
                child_scope.insert(param_name.clone(), v.clone());
            }
        }

        let helper_body = helper.body.clone();

        // Save current scope and last_value, install helper scope
        let saved_scope = std::mem::replace(&mut self.scope, child_scope);
        let saved_last = self.last_value.take();
        let result = self.exec_body_with_return(&helper_body);
        self.scope = saved_scope;
        self.last_value = saved_last;
        result
    }
}

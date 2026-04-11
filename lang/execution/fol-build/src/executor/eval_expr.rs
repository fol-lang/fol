use crate::eval::{BuildEvaluationError, BuildEvaluationErrorKind};
use fol_parser::ast::{AstNode, BinaryOperator, CallSurface, Literal};
use std::collections::BTreeMap;

use super::core::{BuildBodyExecutor, MAX_EVAL_DEPTH};
use super::types::ExecValue;

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

    pub(super) fn eval_iterable(&self, node: &AstNode) -> Result<Vec<ExecValue>, BuildEvaluationError> {
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
            AstNode::Identifier { name, .. } => {
                match self.scope.get(name.as_str()) {
                    Some(ExecValue::List(items)) => Ok(items.clone()),
                    _ => Ok(Vec::new()),
                }
            }
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
                        _ => Ok(true),
                    }
                } else {
                    Ok(false)
                }
            }
            AstNode::BinaryOp { op: BinaryOperator::Eq, left, right } => {
                let lhs = self.eval_value_str(left);
                let rhs = self.eval_value_str(right);
                match (lhs, rhs) {
                    (Some(l), Some(r)) => Ok(l == r),
                    _ => Ok(false),
                }
            }
            AstNode::BinaryOp { op: BinaryOperator::Ne, left, right } => {
                let lhs = self.eval_value_str(left);
                let rhs = self.eval_value_str(right);
                match (lhs, rhs) {
                    (Some(l), Some(r)) => Ok(l != r),
                    _ => Ok(false),
                }
            }
            AstNode::BinaryOp { op: BinaryOperator::And, left, right } => {
                Ok(self.eval_condition(left)? && self.eval_condition(right)?)
            }
            AstNode::BinaryOp { op: BinaryOperator::Or, left, right } => {
                Ok(self.eval_condition(left)? || self.eval_condition(right)?)
            }
            AstNode::UnaryOp { op: fol_parser::ast::UnaryOperator::Not, operand } => {
                Ok(!self.eval_condition(operand)?)
            }
            other => Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!(
                    "build evaluation does not support condition node '{}'",
                    Self::build_node_label(other)
                ),
            )),
        }
    }

    pub(super) fn eval_value_str(&self, node: &AstNode) -> Option<String> {
        match node {
            AstNode::Literal(Literal::String(s)) => Some(s.clone()),
            AstNode::Literal(Literal::Boolean(b)) => Some(b.to_string()),
            AstNode::Identifier { name, .. } => {
                match self.scope.get(name.as_str()) {
                    Some(ExecValue::Target(option_name)) => {
                        // Resolve actual target value from inputs if available
                        self.resolved_inputs
                            .get(option_name.as_str())
                            .cloned()
                            .or_else(|| Some(option_name.clone()))
                    }
                    Some(ExecValue::Optimize(option_name)) => {
                        self.resolved_inputs
                            .get(option_name.as_str())
                            .cloned()
                            .or_else(|| Some(option_name.clone()))
                    }
                    Some(ExecValue::OptionRef(option_name)) => {
                        self.resolved_inputs
                            .get(option_name.as_str())
                            .cloned()
                    }
                    Some(ExecValue::Str(s)) => Some(s.clone()),
                    Some(ExecValue::Bool(b)) => Some(b.to_string()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub(super) fn eval_expr(&mut self, expr: &AstNode) -> Result<Option<ExecValue>, BuildEvaluationError> {
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

    fn eval_expr_inner(&mut self, expr: &AstNode) -> Result<Option<ExecValue>, BuildEvaluationError> {
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

            AstNode::FunctionCall { surface: CallSurface::Plain, name, args, .. } => {
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

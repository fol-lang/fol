use crate::eval::{BuildEvaluationError, BuildEvaluationErrorKind, BuildEvaluationOperationKind};
use fol_lexer::lexer::stage3::Elements;
use fol_parser::ast::{
    AstNode, CallSurface, FolType, LoopCondition, Parameter, ParsedPackage, ParsedTopLevel,
    WhenCase,
};
use fol_stream::FileStream;
use std::collections::BTreeMap;
use std::path::Path;

use super::output::ExecutionOutput;
use super::types::{ExecValue, HelperRoutine};

pub(super) const MAX_EVAL_DEPTH: usize = 256;
pub(super) const MAX_SCOPE_SIZE: usize = 10_000;

pub(super) fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

// ---- Main executor ---

pub struct BuildBodyExecutor {
    pub(super) scope: BTreeMap<String, ExecValue>,
    pub(super) helpers: BTreeMap<String, HelperRoutine>,
    pub(super) output: ExecutionOutput,
    pub(super) build_path_str: String,
    pub(super) package_root_str: String,
    pub(super) install_prefix_str: String,
    pub(super) next_run_index: usize,
    pub(super) next_install_index: usize,
    pub(super) last_value: Option<ExecValue>,
    /// Resolved values for standard options (target, optimize), used when evaluating `when` conditions.
    pub(super) resolved_inputs: BTreeMap<String, String>,
    pub(super) recursion_depth: usize,
}

impl BuildBodyExecutor {
    /// Parse a build.fol file and create an executor ready to run the `build` routine.
    /// Returns None if no `build` entry is found.
    pub fn from_file(
        build_path: &Path,
    ) -> Result<Option<(BuildBodyExecutor, Vec<AstNode>)>, BuildEvaluationError> {
        let path_str = build_path.to_str().ok_or_else(|| {
            BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!(
                    "build file path '{}' is not valid UTF-8",
                    build_path.display()
                ),
            )
        })?;

        let mut stream = FileStream::from_file(path_str).map_err(|error| {
            BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!(
                    "could not open build file '{}': {}",
                    build_path.display(),
                    error
                ),
            )
        })?;

        let mut lexer = Elements::init(&mut stream);
        let mut parser = fol_parser::ast::AstParser::new();
        let parsed = parser.parse_package(&mut lexer).map_err(|diagnostics| {
            let message = diagnostics
                .into_iter()
                .next()
                .map(|d| d.message)
                .unwrap_or_else(|| "unknown parse error".to_string());
            BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!(
                    "build file '{}' failed to parse: {}",
                    build_path.display(),
                    message
                ),
            )
        })?;

        Self::from_parsed_package(&parsed, build_path)
    }

    /// Build an executor from an already-parsed package.
    /// Returns None if no `build` entry is found.
    pub fn from_parsed_package(
        package: &ParsedPackage,
        build_path: &Path,
    ) -> Result<Option<(BuildBodyExecutor, Vec<AstNode>)>, BuildEvaluationError> {
        validate_build_public_surface(package)?;
        let mut helpers: BTreeMap<String, HelperRoutine> = BTreeMap::new();
        let mut build_entry: Option<Vec<AstNode>> = None;

        for unit in &package.source_units {
            for item in &unit.items {
                match &item.node {
                    AstNode::ProDecl { name, body, .. } if name == "build" => {
                        build_entry = Some(body.clone());
                    }
                    AstNode::FunDecl {
                        name, params, body, ..
                    }
                    | AstNode::ProDecl {
                        name, params, body, ..
                    } if name != "build" => {
                        helpers.insert(
                            name.clone(),
                            HelperRoutine {
                                params: params.iter().map(|p| p.name.clone()).collect(),
                                body: body.clone(),
                            },
                        );
                    }
                    _ => {}
                }
            }
        }

        let Some(body) = build_entry else {
            return Ok(None);
        };

        let executor = BuildBodyExecutor {
            scope: BTreeMap::new(),
            helpers,
            output: ExecutionOutput::default(),
            build_path_str: build_path.display().to_string(),
            package_root_str: build_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .display()
                .to_string(),
            install_prefix_str: build_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(".fol/install")
                .display()
                .to_string(),
            next_run_index: 0,
            next_install_index: 0,
            last_value: None,
            resolved_inputs: BTreeMap::new(),
            recursion_depth: 0,
        };

        Ok(Some((executor, body)))
    }

    /// Execute the build routine body and return the collected output.
    pub fn execute(mut self, body: &[AstNode]) -> Result<ExecutionOutput, BuildEvaluationError> {
        self.exec_body(body)?;
        Ok(self.output)
    }

    /// Set resolved option values used when evaluating `when` conditions.
    pub fn with_resolved_inputs(mut self, inputs: BTreeMap<String, String>) -> Self {
        self.resolved_inputs = inputs;
        self
    }

    pub fn with_install_prefix(mut self, install_prefix: impl Into<String>) -> Self {
        self.install_prefix_str = install_prefix.into();
        self
    }

    pub(super) fn exec_body(&mut self, stmts: &[AstNode]) -> Result<(), BuildEvaluationError> {
        self.recursion_depth += 1;
        if self.recursion_depth > MAX_EVAL_DEPTH {
            self.recursion_depth -= 1;
            return Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!("build script exceeded maximum recursion depth ({MAX_EVAL_DEPTH})"),
            ));
        }
        for stmt in stmts {
            self.exec_stmt(stmt)?;
        }
        self.recursion_depth -= 1;
        Ok(())
    }

    pub(super) fn exec_stmt(&mut self, stmt: &AstNode) -> Result<(), BuildEvaluationError> {
        match stmt {
            AstNode::VarDecl { name, value, .. } => {
                let Some(value) = value.as_deref() else {
                    self.last_value = None;
                    return Ok(());
                };
                if let Some(v) = self.eval_expr(value)? {
                    if !is_valid_identifier(name) {
                        return Err(BuildEvaluationError::new(
                            BuildEvaluationErrorKind::InvalidInput,
                            format!(
                                "invalid variable name '{}': names must match [a-z][a-z0-9_-]*",
                                name
                            ),
                        ));
                    }
                    if self.scope.len() >= MAX_SCOPE_SIZE {
                        return Err(BuildEvaluationError::new(
                            BuildEvaluationErrorKind::InvalidInput,
                            format!("build script exceeded maximum scope size ({MAX_SCOPE_SIZE})"),
                        ));
                    }
                    self.scope.insert(name.clone(), v.clone());
                    self.last_value = Some(v);
                } else {
                    self.last_value = None;
                }
                Ok(())
            }

            AstNode::FunctionCall {
                surface: CallSurface::DotIntrinsic,
                name: _,
                args: _,
                ..
            } => {
                self.last_value = self.eval_expr(stmt)?;
                Ok(())
            }

            AstNode::Return { .. } | AstNode::Break => {
                self.last_value = None;
                Ok(())
            }

            AstNode::When {
                expr,
                cases,
                default,
            } => {
                self.exec_when(expr, cases, default.as_deref())?;
                self.last_value = None;
                Ok(())
            }

            AstNode::Loop {
                condition, body, ..
            } => {
                self.exec_loop(condition, body)?;
                self.last_value = None;
                Ok(())
            }

            other => {
                self.last_value = self.eval_expr(other)?;
                Ok(())
            }
        }
    }

    pub(super) fn exec_when(
        &mut self,
        expr: &AstNode,
        cases: &[WhenCase],
        default: Option<&[AstNode]>,
    ) -> Result<(), BuildEvaluationError> {
        // When there are no case sub-clauses, the outer `expr` acts as a boolean gate:
        // `when(condition) { { body } }` — run body only when condition is truthy.
        // `when(condition) { stmts }` — the parser puts direct statements into `default`.
        if cases.is_empty() {
            if self.eval_condition(expr)? {
                if let Some(default_body) = default {
                    self.exec_body(default_body)?;
                }
            }
            return Ok(());
        }

        // When case sub-clauses are present, the outer expr is the match subject.
        // Each `case(value)` is compared with that subject; the first match wins.
        let subject = self.eval_when_value(expr)?;
        let mut matched = false;
        for case in cases {
            match case {
                WhenCase::Case { condition, body } => {
                    let case_value = self.eval_when_value(condition)?;
                    if matches!(
                        (&subject, &case_value),
                        (Some(subject), Some(case_value)) if subject == case_value
                    ) {
                        self.exec_body(body)?;
                        matched = true;
                        break;
                    }
                }
                _ => {
                    // Is/In/Has/On — not supported in build evaluation
                }
            }
        }
        if !matched {
            if let Some(default_body) = default {
                self.exec_body(default_body)?;
            }
        }
        Ok(())
    }

    pub(super) fn exec_loop(
        &mut self,
        condition: &LoopCondition,
        body: &[AstNode],
    ) -> Result<(), BuildEvaluationError> {
        match condition {
            LoopCondition::Iteration { var, iterable, .. } => {
                let items = self.eval_iterable(iterable)?;
                for item in items {
                    if self.scope.len() >= MAX_SCOPE_SIZE {
                        return Err(BuildEvaluationError::new(
                            BuildEvaluationErrorKind::InvalidInput,
                            format!("build script exceeded maximum scope size ({MAX_SCOPE_SIZE})"),
                        ));
                    }
                    self.scope.insert(var.clone(), item);
                    self.exec_body(body)?;
                }
            }
            LoopCondition::Condition(_) => {
                return Err(BuildEvaluationError::new(
                    BuildEvaluationErrorKind::InvalidInput,
                    "condition-based loops are not supported in build evaluation",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn exec_body_with_return(
        &mut self,
        stmts: &[AstNode],
    ) -> Result<Option<ExecValue>, BuildEvaluationError> {
        for stmt in stmts {
            match stmt {
                AstNode::Return { value } => {
                    if let Some(value) = value.as_deref() {
                        return self.eval_expr(value);
                    }
                    return Ok(None);
                }
                _ => {
                    self.exec_stmt(stmt)?;
                }
            }
        }
        Ok(self.last_value.clone())
    }

    pub(super) fn append_step_dependencies(
        &mut self,
        step_name: &str,
        depends_on: &[String],
    ) -> Result<(), BuildEvaluationError> {
        let Some(op) = self
            .output
            .operations
            .iter_mut()
            .rev()
            .find(|op| match &op.kind {
                BuildEvaluationOperationKind::Step(r) => r.name == step_name,
                BuildEvaluationOperationKind::AddRun(r) => r.name == step_name,
                BuildEvaluationOperationKind::InstallArtifact(r) => r.name == step_name,
                _ => false,
            })
        else {
            return Err(BuildEvaluationError::new(
                BuildEvaluationErrorKind::InvalidInput,
                format!("unknown chained step '{step_name}'"),
            ));
        };
        match &mut op.kind {
            BuildEvaluationOperationKind::Step(r) => {
                r.depends_on.extend(depends_on.iter().cloned())
            }
            BuildEvaluationOperationKind::AddRun(r) => {
                r.depends_on.extend(depends_on.iter().cloned())
            }
            BuildEvaluationOperationKind::InstallArtifact(r) => {
                r.depends_on.extend(depends_on.iter().cloned())
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn unsupported(&self, name: &str) -> BuildEvaluationError {
        BuildEvaluationError::with_origin(
            BuildEvaluationErrorKind::Unsupported,
            format!(
                "unsupported build API call in '{}': {}",
                self.build_path_str,
                name.trim()
            ),
            fol_parser::ast::SyntaxOrigin {
                file: Some(self.build_path_str.clone()),
                line: 1,
                column: 1,
                length: name.len(),
            },
        )
    }
}

fn validate_build_public_surface(package: &ParsedPackage) -> Result<(), BuildEvaluationError> {
    for unit in &package.source_units {
        for item in &unit.items {
            validate_top_level_public_surface(package, item)?;
        }
    }
    Ok(())
}

fn validate_top_level_public_surface(
    package: &ParsedPackage,
    item: &ParsedTopLevel,
) -> Result<(), BuildEvaluationError> {
    validate_node_public_surface(package, &item.node)?;
    Ok(())
}

fn validate_node_public_surface(
    package: &ParsedPackage,
    node: &AstNode,
) -> Result<(), BuildEvaluationError> {
    match node {
        AstNode::VarDecl {
            type_hint, value, ..
        }
        | AstNode::LabDecl {
            type_hint, value, ..
        } => {
            if let Some(type_hint) = type_hint {
                validate_type_public_surface(package, type_hint)?;
            }
            if let Some(value) = value {
                validate_node_public_surface(package, value)?;
            }
        }
        AstNode::DestructureDecl {
            type_hint, value, ..
        } => {
            if let Some(type_hint) = type_hint {
                validate_type_public_surface(package, type_hint)?;
            }
            validate_node_public_surface(package, value)?;
        }
        AstNode::FunDecl {
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::ProDecl {
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::LogDecl {
            receiver_type,
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            if let Some(receiver_type) = receiver_type.as_ref() {
                validate_type_public_surface(package, receiver_type)?;
            }
            for param in params {
                validate_parameter_public_surface(package, param)?;
            }
            if let Some(return_type) = return_type {
                validate_type_public_surface(package, return_type)?;
            }
            if let Some(error_type) = error_type {
                validate_type_public_surface(package, error_type)?;
            }
            for stmt in body {
                validate_node_public_surface(package, stmt)?;
            }
            for inquiry in inquiries {
                validate_node_public_surface(package, inquiry)?;
            }
        }
        AstNode::AnonymousFun {
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::AnonymousPro {
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        }
        | AstNode::AnonymousLog {
            params,
            return_type,
            error_type,
            body,
            inquiries,
            ..
        } => {
            for param in params {
                validate_parameter_public_surface(package, param)?;
            }
            if let Some(return_type) = return_type {
                validate_type_public_surface(package, return_type)?;
            }
            if let Some(error_type) = error_type {
                validate_type_public_surface(package, error_type)?;
            }
            for stmt in body {
                validate_node_public_surface(package, stmt)?;
            }
            for inquiry in inquiries {
                validate_node_public_surface(package, inquiry)?;
            }
        }
        AstNode::TypeDecl {
            contracts,
            type_def,
            ..
        } => {
            for contract in contracts {
                validate_type_public_surface(package, contract)?;
            }
            match type_def {
                fol_parser::ast::TypeDefinition::Record {
                    fields, members, ..
                } => {
                    for field_type in fields.values() {
                        validate_type_public_surface(package, field_type)?;
                    }
                    for member in members {
                        validate_node_public_surface(package, member)?;
                    }
                }
                fol_parser::ast::TypeDefinition::Entry {
                    variants, members, ..
                } => {
                    for variant in variants.values().flatten() {
                        validate_type_public_surface(package, variant)?;
                    }
                    for member in members {
                        validate_node_public_surface(package, member)?;
                    }
                }
                fol_parser::ast::TypeDefinition::Alias { target } => {
                    validate_type_public_surface(package, target)?;
                }
            }
        }
        AstNode::UseDecl { path_type, .. } => {
            validate_type_public_surface(package, path_type)?;
        }
        AstNode::AliasDecl { target, .. } => {
            validate_type_public_surface(package, target)?;
        }
        AstNode::DefDecl {
            params,
            def_type,
            body,
            ..
        } => {
            validate_type_public_surface(package, def_type)?;
            for param in params {
                validate_parameter_public_surface(package, param)?;
            }
            for stmt in body {
                validate_node_public_surface(package, stmt)?;
            }
        }
        AstNode::SegDecl { seg_type, body, .. } => {
            validate_type_public_surface(package, seg_type)?;
            for stmt in body {
                validate_node_public_surface(package, stmt)?;
            }
        }
        AstNode::StdDecl { body, .. }
        | AstNode::Inquiry { body, .. }
        | AstNode::Block {
            statements: body, ..
        }
        | AstNode::Dfr { body, .. }
        | AstNode::Edf { body, .. } => {
            for stmt in body {
                validate_node_public_surface(package, stmt)?;
            }
        }
        AstNode::Select { arms, default, .. } => {
            for arm in arms {
                validate_node_public_surface(package, &arm.channel)?;
                for stmt in &arm.body {
                    validate_node_public_surface(package, stmt)?;
                }
            }
            if let Some(default) = default {
                for stmt in default {
                    validate_node_public_surface(package, stmt)?;
                }
            }
        }
        AstNode::FunctionCall { args, .. }
        | AstNode::QualifiedFunctionCall { args, .. }
        | AstNode::MethodCall { args, .. }
        | AstNode::Invoke { args, .. }
        | AstNode::ContainerLiteral { elements: args, .. } => {
            for arg in args {
                validate_node_public_surface(package, arg)?;
            }
        }
        AstNode::NamedArgument { value, .. }
        | AstNode::Unpack { value }
        | AstNode::Spawn { task: value, .. }
        | AstNode::Yield { value }
        | AstNode::Return { value: Some(value) } => validate_node_public_surface(package, value)?,
        AstNode::Assignment { target, value } => {
            validate_node_public_surface(package, target)?;
            validate_node_public_surface(package, value)?;
        }
        AstNode::BinaryOp { left, right, .. } => {
            validate_node_public_surface(package, left)?;
            validate_node_public_surface(package, right)?;
        }
        AstNode::UnaryOp { operand, .. }
        | AstNode::TemplateCall {
            object: operand, ..
        }
        | AstNode::FieldAccess {
            object: operand, ..
        }
        | AstNode::AvailabilityAccess { target: operand }
        | AstNode::ChannelAccess {
            channel: operand, ..
        } => {
            validate_node_public_surface(package, operand)?;
        }
        AstNode::IndexAccess { container, index } => {
            validate_node_public_surface(package, container)?;
            validate_node_public_surface(package, index)?;
        }
        AstNode::SliceAccess {
            container,
            start,
            end,
            ..
        } => {
            validate_node_public_surface(package, container)?;
            if let Some(start) = start {
                validate_node_public_surface(package, start)?;
            }
            if let Some(end) = end {
                validate_node_public_surface(package, end)?;
            }
        }
        AstNode::PatternAccess {
            container,
            patterns,
        } => {
            validate_node_public_surface(package, container)?;
            for pattern in patterns {
                validate_node_public_surface(package, pattern)?;
            }
        }
        AstNode::PatternCapture { pattern, .. } => {
            validate_node_public_surface(package, pattern)?;
        }
        AstNode::RecordInit { fields, .. } => {
            for field in fields {
                validate_node_public_surface(package, &field.value)?;
            }
        }
        AstNode::Rolling {
            expr,
            bindings,
            condition,
        } => {
            validate_node_public_surface(package, expr)?;
            for binding in bindings {
                if let Some(type_hint) = &binding.type_hint {
                    validate_type_public_surface(package, type_hint)?;
                }
                validate_node_public_surface(package, &binding.iterable)?;
            }
            if let Some(condition) = condition {
                validate_node_public_surface(package, condition)?;
            }
        }
        AstNode::Range { start, end, .. } => {
            if let Some(start) = start {
                validate_node_public_surface(package, start)?;
            }
            if let Some(end) = end {
                validate_node_public_surface(package, end)?;
            }
        }
        AstNode::When {
            expr,
            cases,
            default,
        } => {
            validate_node_public_surface(package, expr)?;
            for case in cases {
                match case {
                    WhenCase::Case { condition, body } => {
                        validate_node_public_surface(package, condition)?;
                        for stmt in body {
                            validate_node_public_surface(package, stmt)?;
                        }
                    }
                    WhenCase::Is { value, body }
                    | WhenCase::In { range: value, body }
                    | WhenCase::Has {
                        member: value,
                        body,
                    }
                    | WhenCase::On {
                        channel: value,
                        body,
                    } => {
                        validate_node_public_surface(package, value)?;
                        for stmt in body {
                            validate_node_public_surface(package, stmt)?;
                        }
                    }
                    WhenCase::Of { type_match, body } => {
                        validate_type_public_surface(package, type_match)?;
                        for stmt in body {
                            validate_node_public_surface(package, stmt)?;
                        }
                    }
                }
            }
            if let Some(default) = default {
                for stmt in default {
                    validate_node_public_surface(package, stmt)?;
                }
            }
        }
        AstNode::Loop {
            condition, body, ..
        } => {
            match condition.as_ref() {
                LoopCondition::Condition(expr) => validate_node_public_surface(package, expr)?,
                LoopCondition::Iteration {
                    type_hint,
                    iterable,
                    condition,
                    ..
                } => {
                    if let Some(type_hint) = type_hint {
                        validate_type_public_surface(package, type_hint)?;
                    }
                    validate_node_public_surface(package, iterable)?;
                    if let Some(condition) = condition {
                        validate_node_public_surface(package, condition)?;
                    }
                }
            }
            for stmt in body {
                validate_node_public_surface(package, stmt)?;
            }
        }
        AstNode::Comment { .. }
        | AstNode::Return { value: None }
        | AstNode::Break
        | AstNode::AsyncStage
        | AstNode::AwaitStage
        | AstNode::Identifier { .. }
        | AstNode::QualifiedIdentifier { .. }
        | AstNode::Literal(_)
        | AstNode::PatternWildcard
        | AstNode::Program { .. } => {}
        AstNode::Commented { node, .. } => {
            validate_node_public_surface(package, node)?;
        }
        AstNode::OwnershipOp { operand, .. } => {
            validate_node_public_surface(package, operand)?;
        }
    }
    Ok(())
}

fn validate_parameter_public_surface(
    package: &ParsedPackage,
    param: &Parameter,
) -> Result<(), BuildEvaluationError> {
    validate_type_public_surface(package, &param.param_type)?;
    if let Some(default) = &param.default {
        validate_node_public_surface(package, default)?;
    }
    Ok(())
}

fn validate_type_public_surface(
    package: &ParsedPackage,
    typ: &FolType,
) -> Result<(), BuildEvaluationError> {
    match typ {
        FolType::Named { name, syntax_id } if name == "Graph" => {
            return Err(public_graph_error(
                package,
                *syntax_id,
                "public `Graph` type syntax is not part of build.fol; use `.build().graph()` and inferred locals instead",
            ));
        }
        FolType::QualifiedNamed { path } if path.joined() == "Graph" => {
            return Err(public_graph_error(
                package,
                path.syntax_id,
                "public `Graph` type syntax is not part of build.fol; use `.build().graph()` and inferred locals instead",
            ));
        }
        FolType::Array { element_type, .. }
        | FolType::Vector { element_type }
        | FolType::Sequence { element_type }
        | FolType::Matrix { element_type, .. }
        | FolType::Channel { element_type }
        | FolType::ChannelSender { element_type }
        | FolType::ChannelReceiver { element_type }
        | FolType::Mutex {
            inner: element_type,
        }
        | FolType::Optional {
            inner: element_type,
        }
        | FolType::Owned {
            inner: element_type,
        }
        | FolType::Borrowed {
            inner: element_type,
            ..
        }
        | FolType::Pointer {
            target: element_type,
            ..
        } => validate_type_public_surface(package, element_type)?,
        FolType::Set { types } | FolType::Multiple { types } | FolType::Union { types } => {
            for member in types {
                validate_type_public_surface(package, member)?;
            }
        }
        FolType::Map {
            key_type,
            value_type,
        } => {
            validate_type_public_surface(package, key_type)?;
            validate_type_public_surface(package, value_type)?;
        }
        FolType::Record { fields } => {
            for field_type in fields.values() {
                validate_type_public_surface(package, field_type)?;
            }
        }
        FolType::Entry { variants } => {
            for variant in variants.values().flatten() {
                validate_type_public_surface(package, variant)?;
            }
        }
        FolType::Error { inner } => {
            if let Some(inner) = inner {
                validate_type_public_surface(package, inner)?;
            }
        }
        FolType::Eventual {
            value_type,
            error_type,
            ..
        } => {
            validate_type_public_surface(package, value_type)?;
            if let Some(error_type) = error_type {
                validate_type_public_surface(package, error_type)?;
            }
        }
        FolType::Limited { base, limits } => {
            validate_type_public_surface(package, base)?;
            for limit in limits {
                validate_node_public_surface(package, limit)?;
            }
        }
        FolType::Function {
            params,
            return_type,
            env_lifetime: _,
        } => {
            for param in params {
                validate_type_public_surface(package, param)?;
            }
            validate_type_public_surface(package, return_type)?;
        }
        FolType::Generic { constraints, .. } => {
            for constraint in constraints {
                validate_type_public_surface(package, constraint)?;
            }
        }
        FolType::Int { .. }
        | FolType::Float { .. }
        | FolType::Char { .. }
        | FolType::Bool
        | FolType::Never
        | FolType::Any
        | FolType::None
        | FolType::Module { .. }
        | FolType::Block { .. }
        | FolType::Test { .. }
        | FolType::Package { .. }
        | FolType::Location { .. }
        | FolType::Named { .. }
        | FolType::QualifiedNamed { .. } => {}
    }
    Ok(())
}

fn public_graph_error(
    package: &ParsedPackage,
    syntax_id: Option<fol_parser::ast::SyntaxNodeId>,
    message: &str,
) -> BuildEvaluationError {
    let origin = syntax_id.and_then(|syntax_id| package.syntax_index.origin(syntax_id).cloned());
    match origin {
        Some(origin) => BuildEvaluationError::with_origin(
            BuildEvaluationErrorKind::InvalidInput,
            message,
            origin,
        ),
        None => BuildEvaluationError::new(BuildEvaluationErrorKind::InvalidInput, message),
    }
}

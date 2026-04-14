use super::options::{
    decl_visibility, fun_decl_visibility, type_decl_visibility, use_decl_visibility,
    var_decl_visibility, BinaryOperator, CallSurface, CharEncoding, CommentKind, ContainerType,
    DeclOption, FunOption, Literal, StandardKind, TypeOption, UnaryOperator, VarOption,
};
use super::syntax::{ParsedDeclScope, ParsedDeclVisibility, SyntaxNodeId};
use super::types::{
    BindingPattern, ChannelEndpoint, FolType, Generic, InquiryTarget, LoopCondition, Parameter,
    QualifiedPath, RecordInitField, RollingBinding, TypeDefinition, WhenCase,
};

/// Core AST node types for FOL language
#[derive(Debug, Clone, PartialEq)]
pub enum AstNode {
    // ==== DECLARATIONS ====
    /// Variable declaration: var[options] name: type = value
    VarDecl {
        options: Vec<VarOption>,
        name: String,
        type_hint: Option<FolType>,
        value: Option<Box<AstNode>>,
    },

    /// Destructuring binding declaration: var pattern = value
    DestructureDecl {
        options: Vec<VarOption>,
        is_label: bool,
        pattern: BindingPattern,
        type_hint: Option<FolType>,
        value: Box<AstNode>,
    },

    /// Function declaration: fun[options] name(params): return_type = { body }
    FunDecl {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<FunOption>,
        generics: Vec<Generic>,
        name: String,
        receiver_type: Option<FolType>,
        captures: Vec<String>,
        params: Vec<Parameter>,
        return_type: Option<FolType>,
        error_type: Option<FolType>,
        body: Vec<AstNode>,
        inquiries: Vec<AstNode>,
    },

    /// Procedure declaration: pro[options] name(params): return_type = { body }
    ProDecl {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<FunOption>,
        generics: Vec<Generic>,
        name: String,
        receiver_type: Option<FolType>,
        captures: Vec<String>,
        params: Vec<Parameter>,
        return_type: Option<FolType>,
        error_type: Option<FolType>,
        body: Vec<AstNode>,
        inquiries: Vec<AstNode>,
    },

    /// Logical declaration: log[options] name(params): return_type = { body }
    LogDecl {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<FunOption>,
        generics: Vec<Generic>,
        name: String,
        receiver_type: Option<FolType>,
        captures: Vec<String>,
        params: Vec<Parameter>,
        return_type: Option<FolType>,
        error_type: Option<FolType>,
        body: Vec<AstNode>,
        inquiries: Vec<AstNode>,
    },

    /// Type declaration: typ name: definition
    TypeDecl {
        options: Vec<TypeOption>,
        generics: Vec<Generic>,
        contracts: Vec<FolType>,
        explicit_contracts: Vec<FolType>,
        name: String,
        type_def: TypeDefinition,
    },

    /// Use declaration: use[options] name: type = { "path" }
    ///
    /// `import_target` is the canonical quoted import target payload.
    /// `path_segments` is retained temporarily as parser structure while the
    /// rest of the pipeline finishes moving off the old segment-first model.
    UseDecl {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<super::options::UseOption>,
        name: String,
        path_type: FolType,
        import_target: String,
        path_segments: Vec<super::options::UsePathSegment>,
    },

    /// Alias declaration: ali name: target_type
    AliasDecl {
        name: String,
        target: FolType,
    },

    /// Definition declaration: def name: mod[...] = { body } / def name: blk[...] = { body }
    DefDecl {
        options: Vec<DeclOption>,
        name: String,
        params: Vec<Parameter>,
        def_type: FolType,
        body: Vec<AstNode>,
    },

    /// Segment declaration: seg name: mod[...] = { body }
    SegDecl {
        options: Vec<DeclOption>,
        name: String,
        seg_type: FolType,
        body: Vec<AstNode>,
    },

    /// Standard declaration: std name: pro|blu|ext = { body }
    StdDecl {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<DeclOption>,
        name: String,
        kind: StandardKind,
        kind_options: Vec<DeclOption>,
        body: Vec<AstNode>,
    },

    Comment {
        kind: CommentKind,
        raw: String,
    },

    Commented {
        leading_comments: Vec<AstNode>,
        node: Box<AstNode>,
        trailing_comments: Vec<AstNode>,
    },

    // ==== EXPRESSIONS ====
    /// Binary operation: (left op right)
    BinaryOp {
        op: BinaryOperator,
        left: Box<AstNode>,
        right: Box<AstNode>,
    },

    /// Unary operation: (op operand)
    UnaryOp {
        op: UnaryOperator,
        operand: Box<AstNode>,
    },

    /// Function call: function_name(args)
    FunctionCall {
        syntax_id: Option<SyntaxNodeId>,
        surface: CallSurface,
        name: String,
        args: Vec<AstNode>,
    },

    /// Qualified function call: a::b::call(args)
    QualifiedFunctionCall {
        path: QualifiedPath,
        args: Vec<AstNode>,
    },

    /// Named call argument: name = value
    NamedArgument {
        name: String,
        value: Box<AstNode>,
    },

    /// Call-site unpack argument: ...value
    Unpack {
        value: Box<AstNode>,
    },

    /// Processor pipe stage: async
    AsyncStage,

    /// Processor pipe stage: await
    AwaitStage,

    /// Coroutine spawn expression: [>]expr
    Spawn {
        task: Box<AstNode>,
    },

    /// General invocation: callee(args)
    Invoke {
        callee: Box<AstNode>,
        args: Vec<AstNode>,
    },

    /// Anonymous function expression: fun (...) : T / E = { ... }
    AnonymousFun {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<FunOption>,
        captures: Vec<String>,
        params: Vec<Parameter>,
        return_type: Option<FolType>,
        error_type: Option<FolType>,
        body: Vec<AstNode>,
        inquiries: Vec<AstNode>,
    },

    /// Anonymous procedure expression: pro (...) : T / E = { ... }
    AnonymousPro {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<FunOption>,
        captures: Vec<String>,
        params: Vec<Parameter>,
        return_type: Option<FolType>,
        error_type: Option<FolType>,
        body: Vec<AstNode>,
        inquiries: Vec<AstNode>,
    },

    /// Anonymous logical expression: log (...) : bol / E = { ... }
    AnonymousLog {
        syntax_id: Option<SyntaxNodeId>,
        options: Vec<FunOption>,
        captures: Vec<String>,
        params: Vec<Parameter>,
        return_type: Option<FolType>,
        error_type: Option<FolType>,
        body: Vec<AstNode>,
        inquiries: Vec<AstNode>,
    },

    /// Method call: object.method(args)
    MethodCall {
        syntax_id: Option<SyntaxNodeId>,
        object: Box<AstNode>,
        method: String,
        args: Vec<AstNode>,
    },

    /// Postfix template access: object$
    TemplateCall {
        object: Box<AstNode>,
        template: String,
    },

    /// Array/Container access: container[index]
    IndexAccess {
        container: Box<AstNode>,
        index: Box<AstNode>,
    },

    /// Channel endpoint access: channel[tx] / channel[rx]
    ChannelAccess {
        channel: Box<AstNode>,
        endpoint: ChannelEndpoint,
    },

    /// Slice access: container[start:end] / container[start::end]
    SliceAccess {
        container: Box<AstNode>,
        start: Option<Box<AstNode>>,
        end: Option<Box<AstNode>>,
        reverse: bool,
    },

    /// Pattern access: container[a, b, ...]
    PatternAccess {
        container: Box<AstNode>,
        patterns: Vec<AstNode>,
    },

    /// Wildcard access pattern: *
    PatternWildcard,

    /// Capturing access pattern: pattern => Name / * => Name
    PatternCapture {
        pattern: Box<AstNode>,
        binding: String,
    },

    /// Availability access: container:[pattern] / access_expr:
    AvailabilityAccess {
        target: Box<AstNode>,
    },

    /// Field access: object.field
    FieldAccess {
        object: Box<AstNode>,
        field: String,
    },

    /// Identifier reference
    Identifier {
        syntax_id: Option<SyntaxNodeId>,
        name: String,
    },

    /// Qualified identifier reference
    QualifiedIdentifier {
        path: QualifiedPath,
    },

    /// Literal values
    Literal(Literal),

    /// Container literal: { elem1, elem2, ... }
    ContainerLiteral {
        container_type: ContainerType,
        elements: Vec<AstNode>,
    },

    /// Record/object initializer: { field = value, ... }
    RecordInit {
        syntax_id: Option<SyntaxNodeId>,
        fields: Vec<RecordInitField>,
    },

    /// Rolling/list-comprehension expression: { expr for x in iterable if cond }
    Rolling {
        expr: Box<AstNode>,
        bindings: Vec<RollingBinding>,
        condition: Option<Box<AstNode>>,
    },

    /// Range expression: {start..end}
    Range {
        start: Option<Box<AstNode>>,
        end: Option<Box<AstNode>>,
        inclusive: bool,
    },

    // ==== STATEMENTS ====
    /// Assignment: target = value
    Assignment {
        target: Box<AstNode>,
        value: Box<AstNode>,
    },

    /// Label declaration: lab name: type = value
    LabDecl {
        options: Vec<VarOption>,
        name: String,
        type_hint: Option<FolType>,
        value: Option<Box<AstNode>>,
    },

    /// When statement (FOL's if/match): when(expr) { case(condition){} * {} }
    When {
        expr: Box<AstNode>,
        cases: Vec<WhenCase>,
        default: Option<Vec<AstNode>>,
    },

    /// Loop statement: loop(condition) { body }
    Loop {
        condition: Box<LoopCondition>,
        body: Vec<AstNode>,
    },

    /// Select statement: select(channel as binding) { body }
    Select {
        channel: Box<AstNode>,
        binding: Option<String>,
        body: Vec<AstNode>,
    },

    /// Return statement: return value
    Return {
        value: Option<Box<AstNode>>,
    },

    /// Break statement
    Break,

    /// Yield statement: yield value
    Yield {
        value: Box<AstNode>,
    },

    /// Defer statement: defer { body }
    Defer {
        syntax_id: Option<SyntaxNodeId>,
        body: Vec<AstNode>,
    },

    /// Block: { statements }
    Block {
        syntax_id: Option<SyntaxNodeId>,
        statements: Vec<AstNode>,
    },

    /// Inquiry clause attached to a routine
    Inquiry {
        target: InquiryTarget,
        body: Vec<AstNode>,
    },

    /// Program root.
    ///
    /// The `declarations` list is intentionally source-ordered and mixed: it may
    /// contain declarations, executable root nodes, and retained standalone comment
    /// nodes accepted at file scope.
    Program {
        declarations: Vec<AstNode>,
    },
}

impl AstNode {
    pub fn declaration_visibility(&self) -> Option<ParsedDeclVisibility> {
        match self {
            AstNode::VarDecl { options, .. }
            | AstNode::LabDecl { options, .. }
            | AstNode::DestructureDecl { options, .. } => Some(var_decl_visibility(options)),
            AstNode::FunDecl { options, .. }
            | AstNode::ProDecl { options, .. }
            | AstNode::LogDecl { options, .. } => Some(fun_decl_visibility(options)),
            AstNode::TypeDecl { options, .. } => Some(type_decl_visibility(options)),
            AstNode::UseDecl { options, .. } => Some(use_decl_visibility(options)),
            AstNode::DefDecl { options, .. }
            | AstNode::SegDecl { options, .. }
            | AstNode::StdDecl { options, .. } => Some(decl_visibility(options)),
            AstNode::AliasDecl { .. } => Some(ParsedDeclVisibility::Normal),
            AstNode::Commented { node, .. } => node.declaration_visibility(),
            _ => None,
        }
    }

    pub fn declaration_scope(&self, package: &str, namespace: &str) -> Option<ParsedDeclScope> {
        let visibility = self.declaration_visibility()?;
        if visibility == ParsedDeclVisibility::Hidden {
            Some(ParsedDeclScope::File)
        } else if namespace == package {
            Some(ParsedDeclScope::Package)
        } else {
            Some(ParsedDeclScope::Namespace)
        }
    }

    pub fn syntax_id(&self) -> Option<SyntaxNodeId> {
        match self {
            AstNode::FunDecl { syntax_id, .. }
            | AstNode::ProDecl { syntax_id, .. }
            | AstNode::LogDecl { syntax_id, .. }
            | AstNode::UseDecl { syntax_id, .. }
            | AstNode::Identifier { syntax_id, .. }
            | AstNode::FunctionCall { syntax_id, .. }
            | AstNode::MethodCall { syntax_id, .. }
            | AstNode::RecordInit { syntax_id, .. }
            | AstNode::Defer { syntax_id, .. }
            | AstNode::Block { syntax_id, .. } => *syntax_id,
            AstNode::Commented { node, .. } => node.syntax_id(),
            _ => None,
        }
    }

    /// Return the direct body nodes for routine-like AST nodes.
    pub fn routine_body(&self) -> Option<&[AstNode]> {
        match self {
            AstNode::FunDecl { body, .. }
            | AstNode::ProDecl { body, .. }
            | AstNode::LogDecl { body, .. }
            | AstNode::AnonymousFun { body, .. }
            | AstNode::AnonymousPro { body, .. }
            | AstNode::AnonymousLog { body, .. } => Some(body.as_slice()),
            AstNode::Commented { node, .. } => node.routine_body(),
            _ => None,
        }
    }

    /// Return a parser-local syntactic type hint when one is obvious from this node alone.
    ///
    /// This is not semantic analysis and must not be treated as an authoritative type.
    pub fn syntactic_type_hint(&self) -> Option<FolType> {
        match self {
            AstNode::Literal(Literal::Integer(_)) => Some(FolType::Int {
                size: None,
                signed: true,
            }),
            AstNode::Literal(Literal::Float(_)) => Some(FolType::Float { size: None }),
            AstNode::Literal(Literal::String(_)) => Some(FolType::Named {
                syntax_id: None,
                name: "str".to_string(),
            }),
            AstNode::Literal(Literal::Character(_)) => Some(FolType::Char {
                encoding: CharEncoding::Utf8,
            }),
            AstNode::Literal(Literal::Boolean(_)) => Some(FolType::Bool),
            AstNode::Literal(Literal::Nil) => Some(FolType::None),

            AstNode::VarDecl { type_hint, .. }
            | AstNode::LabDecl { type_hint, .. }
            | AstNode::DestructureDecl { type_hint, .. } => type_hint.clone(),
            AstNode::FunDecl { return_type, .. } => return_type.clone(),
            AstNode::ProDecl { return_type, .. } => return_type.clone(),
            AstNode::LogDecl { return_type, .. } => return_type.clone().or(Some(FolType::Bool)),
            AstNode::DefDecl { def_type, .. } => Some(def_type.clone()),
            AstNode::SegDecl { seg_type, .. } => Some(seg_type.clone()),
            AstNode::StdDecl { .. } => None,
            AstNode::Comment { .. } => None,
            AstNode::Commented { node, .. } => node.syntactic_type_hint(),

            AstNode::BinaryOp { op, left, right } => {
                match op {
                    BinaryOperator::Add
                    | BinaryOperator::Sub
                    | BinaryOperator::Mul
                    | BinaryOperator::Div
                    | BinaryOperator::Mod
                    | BinaryOperator::Pow => {
                        left.syntactic_type_hint()
                            .or_else(|| right.syntactic_type_hint())
                    }
                    BinaryOperator::Eq
                    | BinaryOperator::Ne
                    | BinaryOperator::Lt
                    | BinaryOperator::Le
                    | BinaryOperator::Gt
                    | BinaryOperator::Ge
                    | BinaryOperator::In
                    | BinaryOperator::Has
                    | BinaryOperator::Is => Some(FolType::Bool),
                    BinaryOperator::And | BinaryOperator::Or | BinaryOperator::Xor => {
                        Some(FolType::Bool)
                    }
                    _ => None,
                }
            }

            AstNode::UnaryOp { op, operand } => match op {
                UnaryOperator::Neg => operand.syntactic_type_hint(),
                UnaryOperator::Not => Some(FolType::Bool),
                UnaryOperator::Ref => operand.syntactic_type_hint().map(|t| FolType::Pointer {
                    target: Box::new(t),
                }),
                UnaryOperator::Deref => {
                    if let Some(FolType::Pointer { target }) = operand.syntactic_type_hint() {
                        Some(*target)
                    } else {
                        None
                    }
                }
                UnaryOperator::Unwrap => {
                    if let Some(FolType::Optional { inner }) = operand.syntactic_type_hint() {
                        Some(*inner)
                    } else {
                        operand.syntactic_type_hint()
                    }
                }
            },
            AstNode::Invoke { callee, .. } => {
                if let Some(FolType::Function { return_type, .. }) = callee.syntactic_type_hint() {
                    Some(*return_type)
                } else {
                    None
                }
            }
            AstNode::NamedArgument { value, .. } => value.syntactic_type_hint(),
            AstNode::Unpack { value } => value.syntactic_type_hint(),
            AstNode::AsyncStage | AstNode::AwaitStage => None,
            AstNode::Spawn { task } => task.syntactic_type_hint(),
            AstNode::AnonymousFun {
                params,
                return_type,
                ..
            } => Some(FolType::Function {
                params: params
                    .iter()
                    .map(|param| param.param_type.clone())
                    .collect(),
                return_type: Box::new(return_type.clone().unwrap_or(FolType::Any)),
            }),
            AstNode::AnonymousPro {
                params,
                return_type,
                ..
            } => Some(FolType::Function {
                params: params
                    .iter()
                    .map(|param| param.param_type.clone())
                    .collect(),
                return_type: Box::new(return_type.clone().unwrap_or(FolType::Any)),
            }),
            AstNode::AnonymousLog {
                params,
                return_type,
                ..
            } => Some(FolType::Function {
                params: params
                    .iter()
                    .map(|param| param.param_type.clone())
                    .collect(),
                return_type: Box::new(return_type.clone().unwrap_or(FolType::Bool)),
            }),
            AstNode::AvailabilityAccess { .. } => Some(FolType::Bool),
            AstNode::Inquiry { .. } => None,
            AstNode::PatternWildcard => None,
            AstNode::PatternCapture { pattern, .. } => pattern.syntactic_type_hint(),
            AstNode::Defer { .. } => None,
            AstNode::RecordInit { .. } => None,
            AstNode::TemplateCall { .. } => None,

            _ => None,
        }
    }

    /// Compatibility shim for older parser-era tests and callers.
    ///
    /// Prefer `syntactic_type_hint()` for any new code so this helper is not mistaken
    /// for whole-program semantic typing.
    pub fn get_type(&self) -> Option<FolType> {
        self.syntactic_type_hint()
    }

    /// Get all child nodes for tree traversal
    pub fn children(&self) -> Vec<&AstNode> {
        match self {
            AstNode::VarDecl { value, .. } | AstNode::LabDecl { value, .. } => {
                value.as_ref().map(|v| vec![v.as_ref()]).unwrap_or_default()
            }
            AstNode::DestructureDecl { value, .. } => vec![value.as_ref()],
            AstNode::FunDecl {
                body, inquiries, ..
            }
            | AstNode::ProDecl {
                body, inquiries, ..
            }
            | AstNode::LogDecl {
                body, inquiries, ..
            } => {
                let mut children: Vec<&AstNode> = body.iter().collect();
                children.extend(inquiries.iter());
                children
            }
            AstNode::DefDecl { body, .. }
            | AstNode::SegDecl { body, .. }
            | AstNode::StdDecl { body, .. } => body.iter().collect(),
            AstNode::Inquiry { body, .. } => body.iter().collect(),
            AstNode::BinaryOp { left, right, .. } => {
                vec![left.as_ref(), right.as_ref()]
            }
            AstNode::UnaryOp { operand, .. } => {
                vec![operand.as_ref()]
            }
            AstNode::NamedArgument { value, .. } => {
                vec![value.as_ref()]
            }
            AstNode::Unpack { value } => {
                vec![value.as_ref()]
            }
            AstNode::AsyncStage | AstNode::AwaitStage => vec![],
            AstNode::Spawn { task } => vec![task.as_ref()],
            AstNode::FunctionCall { args, .. } | AstNode::QualifiedFunctionCall { args, .. } => {
                args.iter().collect()
            }
            AstNode::MethodCall { object, args, .. } => {
                let mut children = vec![object.as_ref()];
                children.extend(args.iter());
                children
            }
            AstNode::Invoke { callee, args } => {
                let mut children = vec![callee.as_ref()];
                children.extend(args.iter());
                children
            }
            AstNode::TemplateCall { object, .. } => vec![object.as_ref()],
            AstNode::AnonymousFun {
                body, inquiries, ..
            }
            | AstNode::AnonymousPro {
                body, inquiries, ..
            }
            | AstNode::AnonymousLog {
                body, inquiries, ..
            } => {
                let mut children: Vec<&AstNode> = body.iter().collect();
                children.extend(inquiries.iter());
                children
            }
            AstNode::Assignment { target, value } => {
                vec![target.as_ref(), value.as_ref()]
            }
            AstNode::When {
                expr,
                cases,
                default,
            } => {
                let mut children = vec![expr.as_ref()];
                for case in cases {
                    match case {
                        WhenCase::Case { condition, body }
                        | WhenCase::Is {
                            value: condition,
                            body,
                        }
                        | WhenCase::In {
                            range: condition,
                            body,
                        }
                        | WhenCase::Has {
                            member: condition,
                            body,
                        }
                        | WhenCase::On {
                            channel: condition,
                            body,
                        } => {
                            children.push(condition);
                            children.extend(body.iter());
                        }
                        WhenCase::Of { body, .. } => {
                            children.extend(body.iter());
                        }
                    }
                }
                if let Some(default_body) = default {
                    children.extend(default_body.iter());
                }
                children
            }
            AstNode::Loop { condition, body } => {
                let mut children: Vec<&AstNode> = body.iter().collect();
                match condition.as_ref() {
                    LoopCondition::Condition(cond) => children.push(cond.as_ref()),
                    LoopCondition::Iteration {
                        iterable,
                        condition: iter_cond,
                        ..
                    } => {
                        children.push(iterable.as_ref());
                        if let Some(cond) = iter_cond {
                            children.push(cond.as_ref());
                        }
                    }
                }
                children
            }
            AstNode::Select { channel, body, .. } => {
                let mut children = vec![channel.as_ref()];
                children.extend(body.iter());
                children
            }
            AstNode::Block { statements, .. } => statements.iter().collect(),
            AstNode::Program { declarations } => declarations.iter().collect(),
            AstNode::Comment { .. } => vec![],
            AstNode::Commented {
                leading_comments,
                node,
                trailing_comments,
            } => {
                let mut children: Vec<&AstNode> = leading_comments.iter().collect();
                children.push(node.as_ref());
                children.extend(trailing_comments.iter());
                children
            }
            AstNode::ContainerLiteral { elements, .. } => elements.iter().collect(),
            AstNode::RecordInit { fields, .. } => fields.iter().map(|field| &field.value).collect(),
            AstNode::Rolling {
                expr,
                bindings,
                condition,
            } => {
                let mut children = vec![expr.as_ref()];
                children.extend(bindings.iter().map(|binding| &binding.iterable));
                if let Some(cond) = condition {
                    children.push(cond.as_ref());
                }
                children
            }
            AstNode::IndexAccess { container, index } => {
                vec![container.as_ref(), index.as_ref()]
            }
            AstNode::ChannelAccess { channel, .. } => vec![channel.as_ref()],
            AstNode::SliceAccess {
                container,
                start,
                end,
                ..
            } => {
                let mut children = vec![container.as_ref()];
                if let Some(start) = start {
                    children.push(start.as_ref());
                }
                if let Some(end) = end {
                    children.push(end.as_ref());
                }
                children
            }
            AstNode::PatternAccess {
                container,
                patterns,
            } => {
                let mut children = vec![container.as_ref()];
                children.extend(patterns.iter());
                children
            }
            AstNode::PatternCapture { pattern, .. } => vec![pattern.as_ref()],
            AstNode::AvailabilityAccess { target } => {
                vec![target.as_ref()]
            }
            AstNode::FieldAccess { object, .. } => {
                vec![object.as_ref()]
            }
            AstNode::Return { value } => {
                value.as_ref().map(|v| vec![v.as_ref()]).unwrap_or_default()
            }
            AstNode::Yield { value } => {
                vec![value.as_ref()]
            }
            AstNode::Defer { body, .. } => body.iter().collect(),
            AstNode::Range { start, end, .. } => {
                let mut children = Vec::new();
                if let Some(s) = start {
                    children.push(s.as_ref());
                }
                if let Some(e) = end {
                    children.push(e.as_ref());
                }
                children
            }
            _ => vec![],
        }
    }

    /// Accept a visitor for the visitor pattern
    pub fn accept<V: super::visitor::AstVisitor>(&self, visitor: &mut V) {
        visitor.visit(self);
    }
}

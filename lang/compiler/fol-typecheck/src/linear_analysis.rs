//! Linear resource flow analysis.
//!
//! A type claiming `lin` holds a resource whose release **can fail and whose
//! failure means something** -- `fclose` losing buffered writes, `close`
//! returning `EIO`, `sqlite3_close` refusing while statements are open. FOL's
//! `fin` finalization cannot carry that failure anywhere: scope-exit cleanup
//! has no caller waiting on a result. So `lin` values are not finalized at all.
//! They must be consumed exactly once, explicitly, on every path, by either an
//! ownership transfer (`[mov]`, whose release is an ordinary fallible call) or
//! `[fin]`, which states in the source that the failure is being discarded.
//!
//! The rules, each of which exists because the alternative loses information
//! silently:
//!
//! - A scope that reaches its end still holding one is a leak.
//! - `report` while holding one is refused rather than guessing whether the
//!   body's error or the release's error should win.
//! - `when` arms must agree: consumed on one path and not another means the
//!   join has no single answer.
//! - A resource acquired outside a loop cannot be consumed inside it, because
//!   the second iteration would consume it again.
//! - `dfr`/`edf` cannot consume one: those blocks run at scope exit with no
//!   caller to report a release failure to, which is the case `lin` exists to
//!   avoid.
//!
//! The decision this implements, including why the `report` rule is the
//! restrictive option on purpose, is `plan/V4_LINEAR_RESOURCES.md`.

use crate::{CheckedType, TypecheckError, TypecheckErrorKind, TypedProgram};
use fol_parser::ast::{AstNode, OwnershipOption, ParsedSourceUnitKind, SyntaxNodeId, SyntaxOrigin};
use fol_resolver::{ReferenceKind, ResolvedProgram, ScopeId, SourceUnitId, SymbolId, SymbolKind};
use std::collections::{BTreeMap, BTreeSet};

/// One linear binding and where it came from.
#[derive(Clone)]
struct Slot {
    name: String,
    acquired: Option<SyntaxOrigin>,
    held: bool,
}

/// What every linear binding in scope is doing at one program point.
type State = BTreeMap<SymbolId, Slot>;

/// A routine body to analyse.
struct RoutineScope {
    name: String,
    source_unit: SourceUnitId,
    scope: ScopeId,
    body: Vec<AstNode>,
    params: Vec<(SymbolId, String, Option<SyntaxOrigin>)>,
}

/// Where in the routine the walk currently is.
///
/// `loop_entry` is the set of bindings that already existed when the innermost
/// enclosing loop began; consuming one of those inside the loop is the
/// double-release the analysis is looking for.
#[derive(Clone, Default)]
struct Context {
    loop_entry: Option<BTreeSet<SymbolId>>,
    in_deferred: bool,
}

/// Check every routine in the program for linear-resource discipline.
pub(crate) fn validate_linear_resources(typed: &TypedProgram) -> Result<(), TypecheckError> {
    let resolved = typed.resolved().clone();
    let syntax = resolved.syntax().clone();

    for (source_index, unit) in syntax.source_units.iter().enumerate() {
        if unit.kind == ParsedSourceUnitKind::Build {
            continue;
        }
        let source_unit = SourceUnitId(source_index);
        let mut routines = Vec::new();
        for item in &unit.items {
            collect_routines(&resolved, source_unit, &item.node, &mut routines);
        }
        for routine in &routines {
            analyze_routine(typed, &resolved, routine)?;
        }
    }
    Ok(())
}

fn collect_routines(
    resolved: &ResolvedProgram,
    source_unit: SourceUnitId,
    node: &AstNode,
    routines: &mut Vec<RoutineScope>,
) {
    let node = strip_comments(node);
    match node {
        AstNode::FunDecl {
            syntax_id,
            name,
            params,
            body,
            ..
        }
        | AstNode::ProDecl {
            syntax_id,
            name,
            params,
            body,
            ..
        }
        | AstNode::LogDecl {
            syntax_id,
            name,
            params,
            body,
            ..
        } => {
            // A routine whose signature scope cannot be recovered is skipped
            // rather than guessed at: without the scope, parameter symbols
            // cannot be told apart from same-named bindings elsewhere.
            if let Some(scope) = syntax_id.and_then(|id| resolved.scope_for_syntax(id)) {
                let params = params
                    .iter()
                    .filter_map(|param| {
                        parameter_symbol(resolved, source_unit, scope, &param.name)
                            .map(|symbol| (symbol, param.name.clone(), None))
                    })
                    .collect();
                routines.push(RoutineScope {
                    name: name.clone(),
                    source_unit,
                    scope,
                    body: body.clone(),
                    params,
                });
            }
            for child in body {
                collect_routines(resolved, source_unit, child, routines);
            }
        }
        other => {
            for child in other.children() {
                collect_routines(resolved, source_unit, child, routines);
            }
        }
    }
}

fn parameter_symbol(
    resolved: &ResolvedProgram,
    source_unit: SourceUnitId,
    scope: ScopeId,
    name: &str,
) -> Option<SymbolId> {
    resolved
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| {
            symbol.source_unit == source_unit
                && symbol.scope == scope
                && symbol.kind == SymbolKind::Parameter
                && symbol.name == name
        })
        .map(|(id, _)| id)
}

fn analyze_routine(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    routine: &RoutineScope,
) -> Result<(), TypecheckError> {
    // An owned linear parameter arrives with its obligation attached: the
    // caller transferred it, so this routine now owes the release. A borrowed
    // one does not, which is why `type_resolves_to_lin` refuses to peel a
    // borrow.
    let mut state = State::new();
    for (symbol, name, origin) in &routine.params {
        if symbol_is_linear(typed, *symbol) {
            state.insert(
                *symbol,
                Slot {
                    name: name.clone(),
                    acquired: origin.clone().or_else(|| symbol_origin(resolved, *symbol)),
                    held: true,
                },
            );
        }
    }

    let entry: BTreeSet<SymbolId> = state.keys().copied().collect();
    let context = Context::default();
    if let Some(state) = walk_block(typed, resolved, routine, &context, &routine.body, state)? {
        // Falling off the end of a routine still holding one is the plain leak
        // case, and the one a reader is most likely to write by accident.
        reject_first_held(&state, &entry, |slot| {
            (
                format!(
                    "routine '{}' ends while still holding the linear resource '{}'; release it with a consuming call or discard the failure with '[fin]{}'",
                    routine.name, slot.name, slot.name
                ),
                slot.acquired.clone(),
            )
        })?;
    }
    Ok(())
}

/// Walk a statement list.
///
/// Returns the state on the fall-through path, or `None` when every path out
/// of the block diverges (returns, reports, breaks, or panics).
fn walk_block(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    routine: &RoutineScope,
    context: &Context,
    statements: &[AstNode],
    mut state: State,
) -> Result<Option<State>, TypecheckError> {
    let declared_before: BTreeSet<SymbolId> = state.keys().copied().collect();

    for statement in statements {
        match walk_statement(typed, resolved, routine, context, statement, state)? {
            Some(next) => state = next,
            None => return Ok(None),
        }
    }

    // Anything acquired inside this block must be gone by its end. Bindings
    // from an enclosing scope are the caller's problem, not this block's.
    reject_first_held_in(&state, &declared_before, |slot| {
        (
            format!(
                "the scope ends while still holding the linear resource '{}'; release it with a consuming call or discard the failure with '[fin]{}'",
                slot.name, slot.name
            ),
            slot.acquired.clone(),
        )
    })?;

    for symbol in state.keys().copied().collect::<Vec<_>>() {
        if !declared_before.contains(&symbol) {
            state.remove(&symbol);
        }
    }
    Ok(Some(state))
}

fn walk_statement(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    routine: &RoutineScope,
    context: &Context,
    node: &AstNode,
    mut state: State,
) -> Result<Option<State>, TypecheckError> {
    let node = strip_comments(node);
    match node {
        AstNode::VarDecl {
            name,
            syntax_id,
            value,
            ..
        }
        | AstNode::LabDecl {
            name,
            syntax_id,
            value,
            ..
        } => {
            if let Some(value) = value {
                walk_expr(resolved, context, value, &mut state)?;
            }
            if let Some(symbol) = binding_symbol(resolved, routine, name, *syntax_id) {
                if symbol_is_linear(typed, symbol) {
                    state.insert(
                        symbol,
                        Slot {
                            name: name.clone(),
                            acquired: syntax_id
                                .and_then(|id| resolved.syntax_index().origin(id).cloned()),
                            held: true,
                        },
                    );
                }
            }
            Ok(Some(state))
        }

        AstNode::Assignment { target, value } => {
            walk_expr(resolved, context, value, &mut state)?;
            if let Some(symbol) = identifier_symbol(resolved, target) {
                if let Some(slot) = state.get_mut(&symbol) {
                    if slot.held {
                        return Err(error_at(
                            format!(
                                "assigning over '{}' would drop a linear resource without releasing it",
                                slot.name
                            ),
                            slot.acquired.clone(),
                        ));
                    }
                    // Re-acquisition after a release is fine: the obligation
                    // simply starts again from here.
                    slot.held = true;
                }
            }
            Ok(Some(state))
        }

        AstNode::Return { value, syntax_id } => {
            if let Some(value) = value {
                walk_expr(resolved, context, value, &mut state)?;
            }
            let origin = syntax_id.and_then(|id| resolved.syntax_index().origin(id).cloned());
            reject_any_held(&state, |slot| {
                (
                    format!(
                        "returning here would abandon the linear resource '{}'; release it before the return",
                        slot.name
                    ),
                    origin.clone().or_else(|| slot.acquired.clone()),
                )
            })?;
            Ok(None)
        }

        AstNode::Break => {
            let inside_loop = context.loop_entry.clone().unwrap_or_default();
            reject_first_held_in(&state, &inside_loop, |slot| {
                (
                    format!(
                        "breaking here would abandon the linear resource '{}'; release it before the break",
                        slot.name
                    ),
                    slot.acquired.clone(),
                )
            })?;
            Ok(None)
        }

        AstNode::When {
            expr,
            cases,
            default,
        } => {
            walk_expr(resolved, context, expr, &mut state)?;
            walk_when(typed, resolved, routine, context, cases, default, state)
        }

        AstNode::Loop {
            condition, body, ..
        } => {
            match condition.as_ref() {
                fol_parser::ast::LoopCondition::Condition(expr) => {
                    walk_expr(resolved, context, expr, &mut state)?;
                }
                fol_parser::ast::LoopCondition::Iteration {
                    iterable,
                    condition,
                    ..
                } => {
                    walk_expr(resolved, context, iterable, &mut state)?;
                    if let Some(condition) = condition {
                        walk_expr(resolved, context, condition, &mut state)?;
                    }
                }
            }

            let entry: BTreeSet<SymbolId> = state
                .iter()
                .filter(|(_, slot)| slot.held)
                .map(|(symbol, _)| *symbol)
                .collect();
            let inner = Context {
                loop_entry: Some(state.keys().copied().collect()),
                in_deferred: context.in_deferred,
            };
            if let Some(after) = walk_block(typed, resolved, routine, &inner, body, state.clone())?
            {
                for symbol in &entry {
                    if after.get(symbol).is_some_and(|slot| !slot.held) {
                        let slot = &state[symbol];
                        return Err(error_at(
                            format!(
                                "'{}' is released inside a loop, so a second iteration would release it again; move the release after the loop",
                                slot.name
                            ),
                            slot.acquired.clone(),
                        ));
                    }
                }
            }
            // The loop body may not run at all, so the state after the loop is
            // the state before it.
            Ok(Some(state))
        }

        AstNode::Block { statements, .. } => {
            walk_block(typed, resolved, routine, context, statements, state)
        }

        AstNode::Dfr { body, .. } | AstNode::Edf { body, .. } => {
            let inner = Context {
                loop_entry: context.loop_entry.clone(),
                in_deferred: true,
            };
            walk_block(typed, resolved, routine, &inner, body, state.clone())?;
            Ok(Some(state))
        }

        AstNode::FunctionCall { name, args, .. } if name == "report" => {
            for arg in args {
                walk_expr(resolved, context, arg, &mut state)?;
            }
            // Resolution 4 of the decision record: the language refuses the
            // case it cannot represent honestly. Reporting here would leave
            // two errors -- the body's and the release's -- competing for one
            // result channel, and whichever won, the other would vanish.
            //
            // The caret goes on the `report` and the acquisition is a secondary
            // label, because a reader who wrote the report needs to see both
            // ends before the rule stops looking arbitrary.
            if let Some(slot) = state.values().find(|slot| slot.held) {
                let mut error = error_at(
                    format!(
                        "cannot report while holding the linear resource '{}': the release can fail too, and only one error can be returned; release '{}' before reporting",
                        slot.name, slot.name
                    ),
                    node_origin(resolved, node).or_else(|| slot.acquired.clone()),
                );
                if let Some(acquired) = slot.acquired.clone() {
                    error = error.with_related_origin(
                        acquired,
                        format!("'{}' is acquired here and still held", slot.name),
                    );
                }
                return Err(error);
            }
            Ok(None)
        }

        AstNode::FunctionCall { name, args, .. } if name == "panic" => {
            for arg in args {
                walk_expr(resolved, context, arg, &mut state)?;
            }
            // A panic abandons the frame entirely; there is no path on which
            // the release could have been written.
            Ok(None)
        }

        other => {
            // A call whose result is linear and whose result nobody binds
            // leaks it at the call site, before the flow walk has anything to
            // track. FOL has no must-use rule to lean on, so the check is here.
            if let Some(name) = discarded_linear_call(typed, resolved, other) {
                return Err(error_at(
                    format!(
                        "'{name}' returns a linear resource and its result is discarded; bind it so it can be released"
                    ),
                    node_origin(resolved, other),
                ));
            }
            walk_expr(resolved, context, other, &mut state)?;
            Ok(Some(state))
        }
    }
}

/// The name of a called routine whose linear result this statement throws away.
fn discarded_linear_call(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    node: &AstNode,
) -> Option<String> {
    let (symbol, name) = match strip_comments(node) {
        AstNode::FunctionCall {
            syntax_id: Some(syntax_id),
            name,
            ..
        } => (
            call_target(resolved, *syntax_id, ReferenceKind::FunctionCall),
            name.clone(),
        ),
        AstNode::QualifiedFunctionCall { path, .. } => (
            path.final_syntax_id
                .or(path.syntax_id)
                .and_then(|id| call_target(resolved, id, ReferenceKind::QualifiedFunctionCall)),
            path.segments.join("::"),
        ),
        AstNode::MethodCall {
            syntax_id: Some(syntax_id),
            method,
            ..
        } => (typed.method_call_target(*syntax_id), method.clone()),
        _ => return None,
    };
    let symbol = symbol?;
    routine_returns_linear(typed, symbol).then_some(name)
}

fn call_target(
    resolved: &ResolvedProgram,
    syntax_id: SyntaxNodeId,
    kind: ReferenceKind,
) -> Option<SymbolId> {
    resolved
        .references
        .iter()
        .find(|reference| reference.syntax_id == Some(syntax_id) && reference.kind == kind)
        .and_then(|reference| reference.resolved)
}

fn node_origin(resolved: &ResolvedProgram, node: &AstNode) -> Option<SyntaxOrigin> {
    node.syntax_id()
        .and_then(|id| resolved.syntax_index().origin(id).cloned())
}

#[allow(clippy::too_many_arguments)]
fn walk_when(
    typed: &TypedProgram,
    resolved: &ResolvedProgram,
    routine: &RoutineScope,
    context: &Context,
    cases: &[fol_parser::ast::WhenCase],
    default: &Option<Vec<AstNode>>,
    state: State,
) -> Result<Option<State>, TypecheckError> {
    let mut arms: Vec<&[AstNode]> = Vec::new();
    for case in cases {
        arms.push(case_body(case));
    }
    match default {
        Some(body) => arms.push(body),
        // With no `*` arm the statement can fall through untouched, which is
        // itself a path and must agree with the others.
        None => arms.push(&[]),
    }

    let mut survivors: Vec<State> = Vec::new();
    for body in arms {
        if let Some(after) = walk_block(typed, resolved, routine, context, body, state.clone())? {
            survivors.push(after);
        }
    }

    let Some(first) = survivors.first().cloned() else {
        return Ok(None);
    };
    for other in survivors.iter().skip(1) {
        for (symbol, slot) in &first {
            let matching = other.get(symbol).is_some_and(|peer| peer.held == slot.held);
            if !matching {
                return Err(error_at(
                    format!(
                        "'{}' is released on some branches of this 'when' but not others; every path must agree on whether a linear resource has been released",
                        slot.name
                    ),
                    slot.acquired.clone(),
                ));
            }
        }
    }
    Ok(Some(first))
}

fn case_body(case: &fol_parser::ast::WhenCase) -> &[AstNode] {
    match case {
        fol_parser::ast::WhenCase::Case { body, .. }
        | fol_parser::ast::WhenCase::Is { body, .. }
        | fol_parser::ast::WhenCase::In { body, .. }
        | fol_parser::ast::WhenCase::Has { body, .. }
        | fol_parser::ast::WhenCase::Of { body, .. }
        | fol_parser::ast::WhenCase::On { body, .. } => body,
    }
}

/// Walk an expression, recording consumptions and refusing bad uses.
fn walk_expr(
    resolved: &ResolvedProgram,
    context: &Context,
    node: &AstNode,
    state: &mut State,
) -> Result<(), TypecheckError> {
    let node = strip_comments(node);

    // A nested routine or closure has its own frame; a linear resource cannot
    // travel into one, because the obligation would leave the scope that owes
    // it with nothing left to prove.
    if is_capturing_node(node) {
        return reject_linear_capture(resolved, node, state);
    }

    if let AstNode::OwnershipOp {
        options, operand, ..
    } = node
    {
        let consuming = options
            .iter()
            .any(|option| matches!(option, OwnershipOption::Move | OwnershipOption::Finalize));
        if consuming {
            if let Some(symbol) = place_root_symbol(resolved, operand) {
                if state.contains_key(&symbol) {
                    return consume(context, state, symbol);
                }
            }
        }
        if options.iter().any(|option| {
            matches!(
                option,
                OwnershipOption::Copy | OwnershipOption::Clone | OwnershipOption::Weak
            )
        }) {
            if let Some(symbol) = place_root_symbol(resolved, operand) {
                if let Some(slot) = state.get(&symbol) {
                    return Err(error_at(
                        format!(
                            "'{}' is a linear resource and cannot be duplicated; only '[mov]' and '[fin]' consume one",
                            slot.name
                        ),
                        slot.acquired.clone(),
                    ));
                }
            }
        }
    }

    if let Some(symbol) = identifier_symbol(resolved, node) {
        if let Some(slot) = state.get(&symbol) {
            if !slot.held {
                return Err(error_at(
                    format!("'{}' has already been released", slot.name),
                    slot.acquired.clone(),
                ));
            }
        }
    }

    for child in node.children() {
        walk_expr(resolved, context, child, state)?;
    }
    Ok(())
}

fn consume(context: &Context, state: &mut State, symbol: SymbolId) -> Result<(), TypecheckError> {
    let slot = state.get_mut(&symbol).expect("checked by the caller");
    if !slot.held {
        return Err(error_at(
            format!("'{}' has already been released", slot.name),
            slot.acquired.clone(),
        ));
    }
    if context.in_deferred {
        // A deferred block runs at scope exit with nobody waiting on a result,
        // which is exactly the shape `lin` exists to rule out: the release can
        // fail and the failure would have nowhere to go.
        return Err(error_at(
            format!(
                "'{}' cannot be released inside a deferred block: the block runs at scope exit, where a failed release has no caller to report to",
                slot.name
            ),
            slot.acquired.clone(),
        ));
    }
    slot.held = false;
    Ok(())
}

/// Refuse a closure, spawn, or nested routine that captures a linear resource.
fn reject_linear_capture(
    resolved: &ResolvedProgram,
    node: &AstNode,
    state: &State,
) -> Result<(), TypecheckError> {
    let mut seen = BTreeSet::new();
    collect_identifier_symbols(resolved, node, &mut seen);
    for symbol in seen {
        if let Some(slot) = state.get(&symbol) {
            return Err(error_at(
                format!(
                    "'{}' is a linear resource and cannot be captured: the release obligation would leave the scope that owes it",
                    slot.name
                ),
                slot.acquired.clone(),
            ));
        }
    }
    Ok(())
}

fn collect_identifier_symbols(
    resolved: &ResolvedProgram,
    node: &AstNode,
    found: &mut BTreeSet<SymbolId>,
) {
    if let Some(symbol) = identifier_symbol(resolved, node) {
        found.insert(symbol);
    }
    if let AstNode::AnonymousFun { captures, .. }
    | AstNode::AnonymousPro { captures, .. }
    | AstNode::AnonymousLog { captures, .. } = strip_comments(node)
    {
        for capture in captures {
            if let Some(symbol) = capture
                .syntax_id
                .and_then(|id| reference_symbol(resolved, id))
            {
                found.insert(symbol);
            }
        }
    }
    for child in node.children() {
        collect_identifier_symbols(resolved, child, found);
    }
}

fn is_capturing_node(node: &AstNode) -> bool {
    matches!(
        node,
        AstNode::Spawn { .. }
            | AstNode::AnonymousFun { .. }
            | AstNode::AnonymousPro { .. }
            | AstNode::AnonymousLog { .. }
            | AstNode::FunDecl { .. }
            | AstNode::ProDecl { .. }
            | AstNode::LogDecl { .. }
    )
}

fn reject_any_held(
    state: &State,
    message: impl Fn(&Slot) -> (String, Option<SyntaxOrigin>),
) -> Result<(), TypecheckError> {
    for slot in state.values() {
        if slot.held {
            let (text, origin) = message(slot);
            return Err(error_at(text, origin));
        }
    }
    Ok(())
}

/// Refuse the first held binding that is **not** in `exempt`.
fn reject_first_held_in(
    state: &State,
    exempt: &BTreeSet<SymbolId>,
    message: impl Fn(&Slot) -> (String, Option<SyntaxOrigin>),
) -> Result<(), TypecheckError> {
    for (symbol, slot) in state {
        if slot.held && !exempt.contains(symbol) {
            let (text, origin) = message(slot);
            return Err(error_at(text, origin));
        }
    }
    Ok(())
}

fn reject_first_held(
    state: &State,
    _entry: &BTreeSet<SymbolId>,
    message: impl Fn(&Slot) -> (String, Option<SyntaxOrigin>),
) -> Result<(), TypecheckError> {
    reject_any_held(state, message)
}

fn error_at(message: String, origin: Option<SyntaxOrigin>) -> TypecheckError {
    match origin {
        Some(origin) => {
            TypecheckError::with_origin(TypecheckErrorKind::InvalidInput, message, origin)
        }
        None => TypecheckError::new(TypecheckErrorKind::InvalidInput, message),
    }
}

fn symbol_is_linear(typed: &TypedProgram, symbol: SymbolId) -> bool {
    typed
        .typed_symbol(symbol)
        .and_then(|symbol| symbol.declared_type)
        .is_some_and(|type_id| typed.type_resolves_to_lin(type_id))
}

fn symbol_origin(resolved: &ResolvedProgram, symbol: SymbolId) -> Option<SyntaxOrigin> {
    resolved
        .symbol(symbol)
        .and_then(|symbol| symbol.origin.clone())
}

/// The binding a place expression is rooted at: `handle`, `handle.field`, and
/// `handle[0]` all root at `handle`.
fn place_root_symbol(resolved: &ResolvedProgram, node: &AstNode) -> Option<SymbolId> {
    match strip_comments(node) {
        AstNode::Identifier { .. } => identifier_symbol(resolved, node),
        AstNode::FieldAccess { object, .. } => place_root_symbol(resolved, object),
        AstNode::IndexAccess { container, .. } => place_root_symbol(resolved, container),
        AstNode::MethodCall { object, .. } => place_root_symbol(resolved, object),
        AstNode::OwnershipOp { operand, .. } => place_root_symbol(resolved, operand),
        _ => None,
    }
}

fn binding_symbol(
    resolved: &ResolvedProgram,
    routine: &RoutineScope,
    name: &str,
    syntax_id: Option<SyntaxNodeId>,
) -> Option<SymbolId> {
    let origin = syntax_id.and_then(|id| resolved.syntax_index().origin(id));
    let candidates = resolved
        .symbols
        .iter_with_ids()
        .filter(|(_, symbol)| {
            symbol.source_unit == routine.source_unit
                && symbol.name == name
                && matches!(
                    symbol.kind,
                    SymbolKind::ValueBinding | SymbolKind::LabelBinding
                )
                && scope_descends_from(resolved, symbol.scope, routine.scope)
        })
        .collect::<Vec<_>>();
    if let Some(origin) = origin {
        if let Some((id, _)) = candidates
            .iter()
            .copied()
            .find(|(_, symbol)| symbol.origin.as_ref() == Some(origin))
        {
            return Some(id);
        }
    }
    if candidates.len() == 1 {
        Some(candidates[0].0)
    } else {
        None
    }
}

fn scope_descends_from(resolved: &ResolvedProgram, mut scope: ScopeId, ancestor: ScopeId) -> bool {
    loop {
        if scope == ancestor {
            return true;
        }
        let Some(parent) = resolved.scope(scope).and_then(|scope| scope.parent) else {
            return false;
        };
        scope = parent;
    }
}

fn identifier_symbol(resolved: &ResolvedProgram, node: &AstNode) -> Option<SymbolId> {
    let AstNode::Identifier {
        syntax_id: Some(syntax_id),
        ..
    } = strip_comments(node)
    else {
        return None;
    };
    reference_symbol(resolved, *syntax_id)
}

fn reference_symbol(resolved: &ResolvedProgram, syntax_id: SyntaxNodeId) -> Option<SymbolId> {
    resolved
        .references
        .iter()
        .find(|reference| {
            reference.syntax_id == Some(syntax_id) && reference.kind == ReferenceKind::Identifier
        })
        .and_then(|reference| reference.resolved)
}

fn strip_comments(mut node: &AstNode) -> &AstNode {
    while let AstNode::Commented { node: inner, .. } = node {
        node = inner;
    }
    node
}

/// Whether a routine hands a linear resource back to its caller.
///
/// Unused by the walk today -- a `return [mov]handle` consumes through the
/// ordinary move path -- but kept as the one place that knows how to read a
/// callee's linear result, which the discarded-result check will need.
#[allow(dead_code)]
fn routine_returns_linear(typed: &TypedProgram, symbol: SymbolId) -> bool {
    let Some(CheckedType::Routine(routine)) = typed
        .typed_symbol(symbol)
        .and_then(|symbol| symbol.declared_type)
        .and_then(|type_id| typed.type_table().get(type_id))
    else {
        return false;
    };
    routine
        .return_type
        .is_some_and(|type_id| typed.type_resolves_to_lin(type_id))
}

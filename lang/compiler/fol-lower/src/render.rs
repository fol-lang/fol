use crate::{ids::LoweredTypeId, LoweredRecoverableAbi, LoweredWorkspace};
use std::fmt::Write;

pub fn render_lowered_workspace(workspace: &LoweredWorkspace) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "workspace entry={} packages={} types={}",
        workspace.entry_identity().display_name,
        workspace.package_count(),
        workspace.type_table().len()
    );
    match workspace.recoverable_abi() {
        LoweredRecoverableAbi::TaggedResultObject {
            tag_type,
            success_tag,
            error_tag,
            success_slot,
            error_slot,
        } => {
            let _ = writeln!(
                output,
                "recoverable-abi kind=tagged-result-object tag=t{} success-tag={} error-tag={} success-slot={} error-slot={}",
                tag_type.0,
                success_tag,
                error_tag,
                success_slot,
                error_slot
            );
            let _ = writeln!(
                output,
                "recoverable-abi semantics {}",
                workspace.recoverable_abi().success_runtime_meaning()
            );
            let _ = writeln!(
                output,
                "recoverable-abi semantics {}",
                workspace.recoverable_abi().failure_runtime_meaning()
            );
            let _ = writeln!(
                output,
                "recoverable-abi semantics {}",
                workspace.recoverable_abi().propagation_runtime_meaning()
            );
            let _ = writeln!(
                output,
                "recoverable-abi semantics {}",
                workspace.recoverable_abi().panic_runtime_meaning()
            );
        }
    }
    for type_index in 0..workspace.type_table().len() {
        let type_id = LoweredTypeId(type_index);
        if let Some(ty) = workspace.type_table().get(type_id) {
            let _ = writeln!(output, "type t{} = {:?}", type_id.0, ty);
        }
    }
    for package in workspace.packages() {
        let _ = writeln!(
            output,
            "package {} id={} kind={:?} root={}",
            package.identity.display_name,
            package.id.0,
            package.identity.source_kind,
            package.identity.canonical_root
        );
        for export in &package.exports {
            let _ = writeln!(
                output,
                "  export source={} mounted={}",
                export.source_namespace,
                export
                    .mounted_namespace_suffix
                    .as_deref()
                    .unwrap_or("<root>")
            );
        }
        for source_unit in &package.source_units {
            let _ = writeln!(
                output,
                "  source {} path={} namespace={}",
                source_unit.source_unit_id.0, source_unit.path, source_unit.namespace
            );
        }
        for type_decl in package.type_decls.values() {
            let _ = writeln!(
                output,
                "  type-decl {} symbol={} runtime=t{} kind={:?}",
                type_decl.name, type_decl.symbol_id.0, type_decl.runtime_type.0, type_decl.kind
            );
        }
        for standard in package.standards.values() {
            let _ = writeln!(
                output,
                "  standard {} symbol={} kind={:?}",
                standard.name,
                standard.symbol_id.0,
                standard.kind
            );
            for requirement in &standard.required_routines {
                let _ = writeln!(
                    output,
                    "    requires {} symbol={} params={:?} return={:?} error={:?}",
                    requirement.name,
                    requirement.symbol_id.0,
                    requirement
                        .params
                        .iter()
                        .map(|type_id| format!("t{}", type_id.0))
                        .collect::<Vec<_>>(),
                    requirement.return_type.map(|type_id| format!("t{}", type_id.0)),
                    requirement.error_type.map(|type_id| format!("t{}", type_id.0)),
                );
            }
        }
        for conformance in package.conformances.values() {
            let _ = writeln!(
                output,
                "  conformance type={} standards={}",
                conformance.type_symbol_id.0,
                conformance
                    .standard_symbol_ids
                    .iter()
                    .map(|symbol_id| symbol_id.0.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        for global in package.global_decls.values() {
            let _ = writeln!(
                output,
                "  global g{} {} symbol={} type=t{} mutable={}",
                global.id.0, global.name, global.symbol_id.0, global.type_id.0, global.mutable
            );
        }
        for routine in package.routine_decls.values() {
            let _ = writeln!(
                output,
                "  routine r{} {} symbol={:?} signature={:?} receiver={:?} entry=b{} body={:?}",
                routine.id.0,
                routine.name,
                routine.symbol_id.map(|id| id.0),
                routine.signature.map(|id| format!("t{}", id.0)),
                routine.receiver_type.map(|id| format!("t{}", id.0)),
                routine.entry_block.0,
                routine.body_result.map(|id| format!("l{}", id.0))
            );
            let _ = writeln!(
                output,
                "    params [{}]",
                routine
                    .params
                    .iter()
                    .map(|local| format!("l{}", local.0))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            for (local_id, local) in routine.locals.iter_with_ids() {
                let _ = writeln!(
                    output,
                    "    local l{} name={} type={}",
                    local_id.0,
                    local.name.as_deref().unwrap_or("_"),
                    local
                        .type_id
                        .map(|type_id| format!("t{}", type_id.0))
                        .unwrap_or_else(|| "_".to_string())
                );
            }
            for (block_id, block) in routine.blocks.iter_with_ids() {
                let _ = writeln!(output, "    block b{}", block_id.0);
                for instr_id in &block.instructions {
                    if let Some(instr) = routine.instructions.get(*instr_id) {
                        let _ = writeln!(
                            output,
                            "      i{} result={} {}",
                            instr.id.0,
                            instr
                                .result
                                .map(|local| format!("l{}", local.0))
                                .unwrap_or_else(|| "_".to_string()),
                            render_instr_kind(&instr.kind)
                        );
                    }
                }
                let _ = writeln!(
                    output,
                    "      term {}",
                    render_terminator(block.terminator.as_ref())
                );
            }
        }
    }
    if !workspace.entry_candidates().is_empty() {
        let _ = writeln!(output, "entry-candidates");
        for entry in workspace.entry_candidates() {
            let _ = writeln!(
                output,
                "  {}::{} -> r{}",
                entry.package_identity.display_name, entry.name, entry.routine_id.0
            );
        }
    }
    output
}

fn render_instr_kind(kind: &crate::LoweredInstrKind) -> String {
    match kind {
        crate::LoweredInstrKind::IntrinsicCall { intrinsic, args } => {
            let name = fol_intrinsics::intrinsic_by_id(*intrinsic)
                .map(|entry| entry.name)
                .unwrap_or("<missing>");
            let role = fol_intrinsics::backend_role_for_intrinsic(*intrinsic)
                .map(|role| role.as_str())
                .unwrap_or("unclassified");
            format!(
                "IntrinsicCall intrinsic=.{} role={} args={}",
                name,
                role,
                render_local_list(args)
            )
        }
        crate::LoweredInstrKind::RuntimeHook { intrinsic, args } => {
            let name = fol_intrinsics::intrinsic_by_id(*intrinsic)
                .map(|entry| entry.name)
                .unwrap_or("<missing>");
            let role = fol_intrinsics::backend_role_for_intrinsic(*intrinsic)
                .map(|role| role.as_str())
                .unwrap_or("unclassified");
            format!(
                "RuntimeHook intrinsic=.{} role={} args={}",
                name,
                role,
                render_local_list(args)
            )
        }
        crate::LoweredInstrKind::LengthOf { operand } => {
            format!(
                "LengthOf intrinsic=.len role={} operand=l{}",
                fol_intrinsics::IntrinsicBackendRole::TargetHelper.as_str(),
                operand.0
            )
        }
        crate::LoweredInstrKind::CheckRecoverable { operand } => {
            format!(
                "CheckRecoverable intrinsic=check role={} operand=l{}",
                fol_intrinsics::IntrinsicBackendRole::TargetHelper.as_str(),
                operand.0
            )
        }
        _ => format!("{kind:?}"),
    }
}

fn render_terminator(terminator: Option<&crate::LoweredTerminator>) -> String {
    match terminator {
        Some(crate::LoweredTerminator::Panic { value }) => format!(
            "Panic intrinsic=panic role={} value={}",
            fol_intrinsics::IntrinsicBackendRole::ControlEffect.as_str(),
            value
                .map(|local| format!("l{}", local.0))
                .unwrap_or_else(|| "_".to_string())
        ),
        Some(other) => format!("{other:?}"),
        None => "None".to_string(),
    }
}

fn render_local_list(locals: &[crate::LoweredLocalId]) -> String {
    format!(
        "[{}]",
        locals
            .iter()
            .map(|local| format!("l{}", local.0))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::render_lowered_workspace;
    use crate::Lowerer;
    use fol_parser::ast::AstParser;
    use fol_resolver::resolve_package_workspace;
    use fol_stream::FileStream;
    use fol_typecheck::Typechecker;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn safe_temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("fol_test");
        std::fs::create_dir_all(&dir).expect("should create test temp root");
        dir
    }

    #[test]
    fn lowered_workspace_snapshot_is_stable_and_human_readable() {
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../test/parser/simple_var.fol"
        );
        let mut stream = FileStream::from_file(fixture_path).expect("Should open lowering fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("snapshot fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("snapshot fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("snapshot fixture should typecheck");
        let lowered = Lowerer::new()
            .lower_typed_workspace(typed)
            .expect("snapshot fixture should lower");

        let first = render_lowered_workspace(&lowered);
        let second = render_lowered_workspace(&lowered);

        assert_eq!(first, second);
        assert!(first.contains("workspace entry=parser"));
        assert!(first.contains("recoverable-abi kind=tagged-result-object"));
        assert!(first.contains("recoverable-abi semantics success =>"));
        assert!(first.contains("recoverable-abi semantics failure =>"));
        assert!(first.contains("recoverable-abi semantics propagation =>"));
        assert!(first.contains("recoverable-abi semantics panic =>"));
        assert!(first.contains("package parser"));
        assert!(first.contains("routine"));
        assert!(first.contains("block b0"));
    }

    #[test]
    fn rendered_workspace_makes_intrinsic_names_and_roles_explicit() {
        let fixture = safe_temp_dir().join(format!(
            "fol_lower_render_intrinsics_{}.fol",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should advance for temp names")
                .as_nanos()
        ));
        std::fs::write(
            &fixture,
            concat!(
                "fun[] main(flag: bol, items: seq[int]): bol = {\n",
                "    .echo(.len(items))\n",
                "    return .eq(flag, .not(false))\n",
                "}\n",
            ),
        )
        .expect("should write intrinsic render fixture");

        let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
            .expect("Should open intrinsic render fixture");
        let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
        let mut parser = AstParser::new();
        let syntax = parser
            .parse_package(&mut lexer)
            .expect("intrinsic render fixture should parse");
        let resolved = resolve_package_workspace(syntax).expect("intrinsic render fixture should resolve");
        let typed = Typechecker::new()
            .check_resolved_workspace(resolved)
            .expect("intrinsic render fixture should typecheck");
        let lowered = Lowerer::new()
            .lower_typed_workspace(typed)
            .expect("intrinsic render fixture should lower");

        let rendered = render_lowered_workspace(&lowered);

        assert!(rendered.contains("IntrinsicCall intrinsic=.eq role=pure-op"));
        assert!(rendered.contains("IntrinsicCall intrinsic=.not role=pure-op"));
        assert!(rendered.contains("LengthOf intrinsic=.len role=target-helper"));
        assert!(rendered.contains("RuntimeHook intrinsic=.echo role=runtime-hook"));
    }
}

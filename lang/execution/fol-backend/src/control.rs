use crate::{mangle_local_name, BackendError, BackendErrorKind, BackendResult};
use fol_lower::{LoweredRoutine, LoweredTerminator, LoweredType, LoweredTypeTable};
use fol_resolver::PackageIdentity;

pub fn render_terminator(
    package_identity: &PackageIdentity,
    type_table: &LoweredTypeTable,
    routine: &LoweredRoutine,
    terminator: &LoweredTerminator,
) -> BackendResult<String> {
    match terminator {
        LoweredTerminator::Jump { target } => {
            Ok(format!("__fol_next_block = {}; continue;", target.0))
        }
        LoweredTerminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            let condition = render_local_name(package_identity, routine, *condition)?;
            Ok(format!(
                "if {condition} {{ __fol_next_block = {}; }} else {{ __fol_next_block = {}; }} continue;",
                then_block.0, else_block.0
            ))
        }
        LoweredTerminator::Return { value } => match value {
            Some(value) => {
                let value = render_local_name(package_identity, routine, *value)?;
                if routine_returns_recoverable(type_table, routine)? {
                    Ok(format!("return rt::FolRecover::ok({value});"))
                } else {
                    Ok(format!("return {value};"))
                }
            }
            None => {
                if routine_returns_recoverable(type_table, routine)? {
                    Ok("return rt::FolRecover::ok(());".to_string())
                } else {
                    Ok("return;".to_string())
                }
            }
        },
        LoweredTerminator::Report { value } => match value {
            Some(value) => Ok(format!(
                "return rt::FolRecover::err({});",
                render_local_name(package_identity, routine, *value)?
            )),
            None => Ok("return rt::FolRecover::err(());".to_string()),
        },
        LoweredTerminator::Panic { value } => match value {
            Some(value) => {
                let name = render_local_name(package_identity, routine, *value)?;
                Ok(format!("panic!(\"{{}}\", rt::render_echo(&{name}));"))
            }
            None => Ok("panic!(\"panic\");".to_string()),
        },
        LoweredTerminator::Unreachable => Ok("unreachable!();".to_string()),
    }
}

fn routine_returns_recoverable(
    type_table: &LoweredTypeTable,
    routine: &LoweredRoutine,
) -> BackendResult<bool> {
    let Some(signature_id) = routine.signature else {
        return Ok(false);
    };
    match type_table.get(signature_id) {
        Some(LoweredType::Routine(signature)) => Ok(signature.error_type.is_some()),
        Some(other) => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("routine signature id did not point to a routine type: {other:?}"),
        )),
        None => Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("routine signature type {:?} is missing", signature_id),
        )),
    }
}

fn render_local_name(
    package_identity: &PackageIdentity,
    routine: &LoweredRoutine,
    local_id: fol_lower::LoweredLocalId,
) -> BackendResult<String> {
    let Some(local) = routine.locals.get(local_id) else {
        return Err(BackendError::new(
            BackendErrorKind::InvalidInput,
            format!("lowered local {:?} is missing", local_id),
        ));
    };
    Ok(mangle_local_name(
        package_identity,
        routine.id,
        local_id,
        local.name.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::render_terminator;
    use crate::testing::package_identity;
    use fol_lower::{
        LoweredBlockId, LoweredBuiltinType, LoweredLocal, LoweredLocalId, LoweredRoutine,
        LoweredRoutineId, LoweredTerminator, LoweredType, LoweredTypeTable,
    };
    use fol_resolver::PackageSourceKind;

    #[test]
    fn terminator_rendering_emits_jump_shells() {
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let table = LoweredTypeTable::new();
        let routine = LoweredRoutine::new(LoweredRoutineId(0), "main", LoweredBlockId(0));

        let rendered = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Jump {
                target: LoweredBlockId(7),
            },
        )
        .expect("jump");

        assert_eq!(rendered, "__fol_next_block = 7; continue;");
    }

    #[test]
    fn terminator_rendering_emits_branch_shells() {
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let mut table = LoweredTypeTable::new();
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let mut routine = LoweredRoutine::new(LoweredRoutineId(1), "main", LoweredBlockId(0));
        let condition = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(bool_id),
            name: Some("flag".to_string()),
        });

        let rendered = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Branch {
                condition,
                then_block: LoweredBlockId(1),
                else_block: LoweredBlockId(2),
            },
        )
        .expect("branch");

        assert_eq!(
            rendered,
            "if l__pkg__entry__app__r1__l0__flag { __fol_next_block = 1; } else { __fol_next_block = 2; } continue;"
        );
    }

    #[test]
    fn terminator_rendering_emits_return_shells() {
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let mut routine = LoweredRoutine::new(LoweredRoutineId(2), "main", LoweredBlockId(0));
        let value = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(int_id),
            name: Some("value".to_string()),
        });

        let with_value = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Return { value: Some(value) },
        )
        .expect("return value");
        let empty = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Return { value: None },
        )
        .expect("return empty");

        assert_eq!(with_value, "return l__pkg__entry__app__r2__l0__value;");
        assert_eq!(empty, "return;");
    }

    #[test]
    fn terminator_rendering_emits_report_and_panic_shells() {
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let mut table = LoweredTypeTable::new();
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let mut routine = LoweredRoutine::new(LoweredRoutineId(3), "main", LoweredBlockId(0));
        let value = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(str_id),
            name: Some("message".to_string()),
        });

        let report = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Report { value: Some(value) },
        )
        .expect("report");
        let panic = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Panic { value: Some(value) },
        )
        .expect("panic");

        assert_eq!(
            report,
            "return rt::FolRecover::err(l__pkg__entry__app__r3__l0__message);"
        );
        assert_eq!(
            panic,
            "panic!(\"{}\", rt::render_echo(&l__pkg__entry__app__r3__l0__message));"
        );
    }

    #[test]
    fn terminator_rendering_emits_unreachable_shells() {
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let table = LoweredTypeTable::new();
        let routine = LoweredRoutine::new(LoweredRoutineId(4), "main", LoweredBlockId(0));

        let rendered = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Unreachable,
        )
        .expect("unreachable");

        assert_eq!(rendered, "unreachable!();");
    }

    #[test]
    fn control_flow_snapshot_stays_stable() {
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let mut table = LoweredTypeTable::new();
        let bool_id = table.intern_builtin(LoweredBuiltinType::Bool);
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let mut routine = LoweredRoutine::new(LoweredRoutineId(5), "main", LoweredBlockId(0));
        routine.signature = Some(table.intern(LoweredType::Routine(
            fol_lower::LoweredRoutineType {
                params: vec![bool_id],
                return_type: Some(int_id),
                error_type: Some(str_id),
            },
        )));
        let flag = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(bool_id),
            name: Some("flag".to_string()),
        });
        let value = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(1),
            type_id: Some(int_id),
            name: Some("value".to_string()),
        });
        let message = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(2),
            type_id: Some(str_id),
            name: Some("message".to_string()),
        });

        let rendered = [
            LoweredTerminator::Jump {
                target: LoweredBlockId(3),
            },
            LoweredTerminator::Branch {
                condition: flag,
                then_block: LoweredBlockId(1),
                else_block: LoweredBlockId(2),
            },
            LoweredTerminator::Return { value: Some(value) },
            LoweredTerminator::Report {
                value: Some(message),
            },
            LoweredTerminator::Panic {
                value: Some(message),
            },
            LoweredTerminator::Unreachable,
        ]
        .iter()
        .map(|terminator| render_terminator(&package_identity, &table, &routine, terminator))
        .collect::<Result<Vec<_>, _>>()
        .expect("control snapshot renders")
        .join("\n");

        assert_eq!(
            rendered,
            concat!(
                "__fol_next_block = 3; continue;\n",
                "if l__pkg__entry__app__r5__l0__flag { __fol_next_block = 1; } else { __fol_next_block = 2; } continue;\n",
                "return rt::FolRecover::ok(l__pkg__entry__app__r5__l1__value);\n",
                "return rt::FolRecover::err(l__pkg__entry__app__r5__l2__message);\n",
                "panic!(\"{}\", rt::render_echo(&l__pkg__entry__app__r5__l2__message));\n",
                "unreachable!();"
            )
        );
    }

    #[test]
    fn recoverable_return_rendering_wraps_success_values_in_runtime_abi() {
        let package_identity = package_identity("app", PackageSourceKind::Entry, "/workspace/app");
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        let signature_id = table.intern(LoweredType::Routine(fol_lower::LoweredRoutineType {
            params: vec![],
            return_type: Some(int_id),
            error_type: Some(str_id),
        }));
        let mut routine = LoweredRoutine::new(LoweredRoutineId(6), "main", LoweredBlockId(0));
        routine.signature = Some(signature_id);
        let value = routine.locals.push(LoweredLocal {
            id: LoweredLocalId(0),
            type_id: Some(int_id),
            name: Some("value".to_string()),
        });

        let rendered = render_terminator(
            &package_identity,
            &table,
            &routine,
            &LoweredTerminator::Return { value: Some(value) },
        )
        .expect("recoverable return");

        assert_eq!(
            rendered,
            "return rt::FolRecover::ok(l__pkg__entry__app__r6__l0__value);"
        );
    }
}

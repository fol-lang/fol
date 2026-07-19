use super::lower_fixture_workspace;
use crate::{LoweredInstrKind, LoweredType};

const GENERIC_RECEIVER_FIXTURE: &str = "\
typ Box(T): rec = {
    value: T
};

fun (Box[T])unwrap(T)(fallback: T): T = {
    return self.value;
};

fun[] main(): int = {
    var numbers: Box[int] = { value = 7 };
    var flags: Box[bol] = { value = true };
    var picked: bol = flags.unwrap(false);
    return numbers.unwrap(3);
};
";

fn type_contains_generic(
    workspace: &crate::LoweredWorkspace,
    type_id: crate::LoweredTypeId,
) -> bool {
    match workspace.type_table().get(type_id) {
        Some(LoweredType::GenericParameter { .. }) => true,
        Some(LoweredType::Record { fields, .. }) => fields
            .values()
            .any(|field| type_contains_generic(workspace, *field)),
        Some(LoweredType::Routine(signature)) => {
            signature
                .params
                .iter()
                .any(|param| type_contains_generic(workspace, *param))
                || signature
                    .return_type
                    .is_some_and(|ret| type_contains_generic(workspace, ret))
        }
        _ => false,
    }
}

#[test]
fn generic_receiver_routines_monomorphize_into_concrete_clones() {
    let workspace = lower_fixture_workspace(GENERIC_RECEIVER_FIXTURE);
    let package = workspace.entry_package();

    // The generic template must be gone: no remaining routine keeps a
    // receiver type that still mentions a generic parameter.
    for routine in package.routine_decls.values() {
        if let Some(receiver_type) = routine.receiver_type {
            assert!(
                !type_contains_generic(&workspace, receiver_type),
                "routine '{}' kept a generic receiver after monomorphization",
                routine.name
            );
        }
        if let Some(signature) = routine.signature {
            assert!(
                !type_contains_generic(&workspace, signature),
                "routine '{}' kept a generic signature after monomorphization",
                routine.name
            );
        }
    }

    // Two distinct instantiations (Box[int] and Box[bol]) produce two
    // concrete clones of `unwrap`.
    let unwrap_clones = package
        .routine_decls
        .values()
        .filter(|routine| routine.name == "unwrap")
        .count();
    assert_eq!(
        unwrap_clones, 2,
        "each generic-receiver instantiation should synthesize one concrete clone"
    );

    // Every call in main resolves to a routine that still exists.
    let main = package
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main should lower");
    for instr in main.instructions.iter() {
        if let LoweredInstrKind::Call { callee, .. } = &instr.kind {
            assert!(
                package.routine_decls.contains_key(callee),
                "main calls routine {callee:?} that no longer exists"
            );
        }
    }
}

#[test]
fn same_named_receiver_routines_lower_with_their_own_selves() {
    // Regression: routine symbol/body pairing was name-based, so two
    // receiver routines sharing a name (method sugar on different types)
    // lowered against the first symbol and lost the second `self` mapping.
    let workspace = lower_fixture_workspace(
        "typ Left: rec = {\n\
             var value: int;\n\
         };\n\
         typ Right: rec = {\n\
             var item: int;\n\
         };\n\
         fun (Left)take(): int = {\n\
             return self.value;\n\
         };\n\
         fun (Right)take(): int = {\n\
             return self.item;\n\
         };\n\
         fun[] main(): int = {\n\
             var l: Left = { value = 1 };\n\
             var r: Right = { item = 2 };\n\
             return l.take() + r.take();\n\
         };\n",
    );
    let package = workspace.entry_package();
    let takes = package
        .routine_decls
        .values()
        .filter(|routine| routine.name == "take")
        .count();
    assert_eq!(takes, 2, "both same-named receiver routines should lower");
}

#[test]
fn same_named_generic_receiver_routines_monomorphize_independently() {
    let workspace = lower_fixture_workspace(
        "typ Left(T): rec = {\n\
             value: T\n\
         };\n\
         typ Right(U): rec = {\n\
             item: U\n\
         };\n\
         fun (Left[T])take(T)(): T = {\n\
             return self.value;\n\
         };\n\
         fun (Right[U])take(U)(): U = {\n\
             return self.item;\n\
         };\n\
         fun[] main(): int = {\n\
             var l: Left[int] = { value = 1 };\n\
             var r: Right[int] = { item = 2 };\n\
             return l.take() + r.take();\n\
         };\n",
    );
    let package = workspace.entry_package();
    // Each template monomorphizes into its own concrete clone.
    let takes = package
        .routine_decls
        .values()
        .filter(|routine| routine.name == "take")
        .count();
    assert_eq!(takes, 2);
}

#[test]
fn two_level_constrained_templates_propagate_bindings() {
    // Regression: a constrained generic routine (`outer`) that forwards its
    // generic argument to another constrained template (`inner`) must itself
    // be templated and monomorphized first. Otherwise mono tried to
    // instantiate `inner` while `outer`'s parameter was still the generic `T`,
    // yielding empty bindings and an L1002 "not determined by the call
    // arguments" failure even though the argument determines `T`.
    let workspace = lower_fixture_workspace(
        "std geo: pro = {\n\
             fun area(): int;\n\
         };\n\
         typ Rect()(geo): rec = {\n\
             var w: int;\n\
         };\n\
         fun (Rect)area(): int = {\n\
             return 5;\n\
         };\n\
         fun inner(T: geo)(v: T): int = {\n\
             return v.area();\n\
         };\n\
         fun outer(T: geo)(v: T): int = {\n\
             return inner(v);\n\
         };\n\
         fun[] main(): int = {\n\
             var r: Rect = { w = 1 };\n\
             return outer(r);\n\
         };\n",
    );
    let package = workspace.entry_package();

    // Both templates monomorphized: no surviving routine keeps a generic
    // parameter in its signature or locals.
    for routine in package.routine_decls.values() {
        if let Some(signature) = routine.signature {
            assert!(
                !type_contains_generic(&workspace, signature),
                "routine '{}' kept a generic signature after monomorphization",
                routine.name
            );
        }
        for local in routine.locals.iter() {
            if let Some(type_id) = local.type_id {
                assert!(
                    !type_contains_generic(&workspace, type_id),
                    "routine '{}' kept a generic local after monomorphization",
                    routine.name
                );
            }
        }
    }

    // Every call in every surviving routine targets a routine that still
    // exists (the nested `inner(v)` was rewritten to the concrete clone).
    for routine in package.routine_decls.values() {
        for instr in routine.instructions.iter() {
            if let LoweredInstrKind::Call { callee, .. } = &instr.kind {
                assert!(
                    package.routine_decls.contains_key(callee),
                    "routine '{}' calls {callee:?} that no longer exists",
                    routine.name
                );
            }
        }
    }

    // Concrete clones of both constrained templates were synthesized.
    let inner_clones = package
        .routine_decls
        .values()
        .filter(|routine| routine.name == "inner")
        .count();
    let outer_clones = package
        .routine_decls
        .values()
        .filter(|routine| routine.name == "outer")
        .count();
    assert_eq!(inner_clones, 1, "inner should monomorphize once for T=Rect");
    assert_eq!(outer_clones, 1, "outer should monomorphize once for T=Rect");
}

#[test]
fn processor_calls_to_generic_templates_are_rewritten() {
    let workspace = lower_fixture_workspace(
        "typ Box(T): rec = {\n\
             value: T\n\
         };\n\
         fun consume(T)(value: Box[T]): non = {\n\
             return;\n\
         };\n\
         fun reveal(T)(value: Box[T]): T = {\n\
             return value.value;\n\
         };\n\
         fun[] main(): int = {\n\
             var first: Box[int] = { value = 1 };\n\
             var second: Box[int] = { value = 2 };\n\
             [>]consume(first);\n\
             var pending = reveal(second) | async;\n\
             return pending | await;\n\
         };\n",
    );
    let package = workspace.entry_package();
    let main = package
        .routine_decls
        .values()
        .find(|routine| routine.name == "main")
        .expect("main should lower");

    let processor_targets = main
        .instructions
        .iter()
        .filter_map(|instr| match instr.kind {
            LoweredInstrKind::SpawnCall { callee, .. }
            | LoweredInstrKind::AsyncCall { callee, .. } => Some(callee),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(processor_targets.len(), 2);
    for callee in processor_targets {
        let concrete = package
            .routine_decls
            .get(&callee)
            .expect("processor call should target a surviving concrete clone");
        assert!(
            concrete
                .signature
                .is_none_or(|signature| !type_contains_generic(&workspace, signature)),
            "processor call target '{}' kept a generic signature",
            concrete.name
        );
    }
}

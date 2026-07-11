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

fn type_contains_generic(workspace: &crate::LoweredWorkspace, type_id: crate::LoweredTypeId) -> bool {
    match workspace.type_table().get(type_id) {
        Some(LoweredType::GenericParameter { .. }) => true,
        Some(LoweredType::Record { fields }) => fields
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

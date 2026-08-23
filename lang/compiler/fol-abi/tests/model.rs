//! The ABI model's own guarantees.

use fol_abi::*;

fn target() -> fol_types::ResolvedTarget {
    fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap()
}

fn surface_with(routines: Vec<ForeignRoutine>, types: AbiTypeTable) -> ResolvedAbiSurface {
    ResolvedAbiSurface {
        artifact: "demo".to_string(),
        major: 1,
        minor: 0,
        interface: ForeignInterfaceTemplate { types, routines }.resolve(target()),
    }
}

fn add_routine(table: &mut AbiTypeTable, symbol: &str) -> ForeignRoutine {
    let int = table.intern_int(fol_types::IntWidth::I64);
    ForeignRoutine {
        fol_path: format!("api::{symbol}"),
        symbol: symbol.to_string(),
        facing: AbiFacing::Export,
        convention: AbiCallingConvention::C,
        parameters: vec![
            AbiParameter {
                name: "a".to_string(),
                type_id: int,
                direction: AbiDirection::In,
            },
            AbiParameter {
                name: "out_result".to_string(),
                type_id: int,
                direction: AbiDirection::Out,
            },
        ],
        result: int,
        error: AbiErrorContract::Infallible,
        origin: AbiSourceOrigin::default(),
    }
}

/// Scalars project to the C spellings in the section 4.6 matrix.
#[test]
fn scalars_project_to_their_c_types() {
    assert_eq!(AbiScalar::Int(fol_types::IntWidth::I32).c_type(), "int32_t");
    assert_eq!(AbiScalar::Int(fol_types::IntWidth::U8).c_type(), "uint8_t");
    assert_eq!(AbiScalar::Int(fol_types::IntWidth::I64).c_type(), "int64_t");
    assert_eq!(
        AbiScalar::Float(fol_types::FloatWidth::F32).c_type(),
        "float"
    );
    assert_eq!(
        AbiScalar::Float(fol_types::FloatWidth::F64).c_type(),
        "double"
    );
    assert_eq!(AbiScalar::Bool.c_type(), "fol_bool_t");
    assert_eq!(AbiScalar::Char.c_type(), "fol_char_t");
}

/// Interning gives one id per distinct type and reuses it.
#[test]
fn the_type_table_interns_by_structure() {
    let mut table = AbiTypeTable::new();
    let first = table.intern_int(fol_types::IntWidth::I32);
    let second = table.intern_int(fol_types::IntWidth::I32);
    let other = table.intern_int(fol_types::IntWidth::U32);

    assert_eq!(first, second);
    assert_ne!(first, other);
    assert_eq!(table.len(), 2);
}

/// Record field order is preserved exactly, because it decides every offset.
#[test]
fn record_field_order_is_preserved_in_the_canonical_encoding() {
    let mut table = AbiTypeTable::new();
    let int = table.intern_int(fol_types::IntWidth::I64);
    let record = table.intern(AbiType::Record {
        name: "Header".to_string(),
        fields: vec![
            AbiField {
                name: "zulu".to_string(),
                type_id: int,
            },
            AbiField {
                name: "alpha".to_string(),
                type_id: int,
            },
            AbiField {
                name: "mike".to_string(),
                type_id: int,
            },
        ],
    });
    let _ = record;

    let json = canonical_type_table_json(&table);
    let zulu = json.find("zulu").expect("zulu should appear");
    let alpha = json.find("alpha").expect("alpha should appear");
    let mike = json.find("mike").expect("mike should appear");
    assert!(
        zulu < alpha && alpha < mike,
        "field order was sorted; it is semantic and must stay as declared:\n{json}"
    );
}

/// Repeated clean encodings are byte-identical.
#[test]
fn canonical_encoding_is_byte_identical_across_runs() {
    let build = || {
        let mut table = AbiTypeTable::new();
        let routine = add_routine(&mut table, "fol_demo_add");
        canonical_interface_json(&surface_with(vec![routine], table))
    };
    assert_eq!(build(), build());
}

/// Declaration order of routines is not an ABI fact.
#[test]
fn reordering_routines_does_not_change_the_interface_fingerprint() {
    let mut table = AbiTypeTable::new();
    let first = add_routine(&mut table, "fol_demo_add");
    let second = add_routine(&mut table, "fol_demo_sub");

    let forward = surface_with(vec![first.clone(), second.clone()], table.clone());
    let reversed = surface_with(vec![second, first], table);

    assert_eq!(
        canonical_interface_json(&forward),
        canonical_interface_json(&reversed)
    );
}

/// The two fingerprints are independent: a compiler upgrade moves the build
/// fingerprint and must not move the interface one.
#[test]
fn a_compiler_upgrade_moves_only_the_build_fingerprint() {
    let mut table = AbiTypeTable::new();
    let routine = add_routine(&mut table, "fol_demo_add");
    let surface = surface_with(vec![routine], table);

    let first = AbiManifest {
        surface: surface.clone(),
        provenance: BuildProvenance {
            compiler: "rustc 1.89.0".to_string(),
            runtime: "fol-runtime 0.2.6".to_string(),
            profile: "debug".to_string(),
            native_inputs: Vec::new(),
        },
    };
    let second = AbiManifest {
        surface,
        provenance: BuildProvenance {
            compiler: "rustc 1.90.0".to_string(),
            ..first.provenance.clone()
        },
    };

    assert_eq!(
        first.interface_fingerprint(),
        second.interface_fingerprint(),
        "a compiler upgrade must not look like an ABI break"
    );
    assert_ne!(first.build_fingerprint(), second.build_fingerprint());
}

/// Link order reaches the build fingerprint.
#[test]
fn native_input_order_moves_the_build_fingerprint() {
    let mut table = AbiTypeTable::new();
    let routine = add_routine(&mut table, "fol_demo_add");
    let surface = surface_with(vec![routine], table);

    let make = |inputs: Vec<String>| AbiManifest {
        surface: surface.clone(),
        provenance: BuildProvenance {
            compiler: "rustc".to_string(),
            runtime: "rt".to_string(),
            profile: "debug".to_string(),
            native_inputs: inputs,
        },
    };
    let forward = make(vec!["libа.a".to_string(), "libb.a".to_string()]);
    let reversed = make(vec!["libb.a".to_string(), "libа.a".to_string()]);
    assert_ne!(forward.build_fingerprint(), reversed.build_fingerprint());
}

/// Adding a disjoint symbol is minor-compatible; changing one is breaking.
#[test]
fn compatibility_distinguishes_addition_from_change() {
    let mut table = AbiTypeTable::new();
    let add = add_routine(&mut table, "fol_demo_add");
    let baseline = surface_with(vec![add.clone()], table.clone());

    assert_eq!(
        compare_surfaces(&baseline, &baseline),
        AbiCompatibility::Identical
    );

    let sub = add_routine(&mut table, "fol_demo_sub");
    let widened = surface_with(vec![add.clone(), sub], table.clone());
    assert_eq!(
        compare_surfaces(&baseline, &widened),
        AbiCompatibility::MinorCompatible
    );

    // Removing a symbol is breaking.
    let mut empty_table = AbiTypeTable::new();
    let other = add_routine(&mut empty_table, "fol_demo_other");
    let removed = surface_with(vec![other], empty_table);
    assert_eq!(
        compare_surfaces(&baseline, &removed),
        AbiCompatibility::Breaking
    );

    // Changing a signature is breaking.
    let mut changed_table = AbiTypeTable::new();
    let mut changed = add_routine(&mut changed_table, "fol_demo_add");
    changed.parameters[0].type_id = changed_table.intern_int(fol_types::IntWidth::I32);
    let changed = surface_with(vec![changed], changed_table);
    assert_eq!(
        compare_surfaces(&baseline, &changed),
        AbiCompatibility::Breaking
    );
}

/// Two targets are never compared as if layout-compatible.
#[test]
fn cross_target_surfaces_are_always_breaking() {
    let mut table = AbiTypeTable::new();
    let routine = add_routine(&mut table, "fol_demo_add");
    let baseline = surface_with(vec![routine.clone()], table.clone());

    let other = ResolvedAbiSurface {
        artifact: "demo".to_string(),
        major: 1,
        minor: 0,
        interface: ForeignInterfaceTemplate {
            types: table,
            routines: vec![routine],
        }
        .resolve(fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-musl").unwrap()),
    };
    assert_eq!(
        compare_surfaces(&baseline, &other),
        AbiCompatibility::Breaking
    );
}

/// Type ids are positions in a table, not ABI facts.
///
/// Inserting an unrelated type renumbers them; the public interface must not
/// move because of it.
#[test]
fn internal_type_ids_do_not_reach_compatibility_comparison() {
    let mut baseline_table = AbiTypeTable::new();
    let baseline_routine = add_routine(&mut baseline_table, "fol_demo_add");
    let baseline = surface_with(vec![baseline_routine], baseline_table);

    let mut shifted_table = AbiTypeTable::new();
    // An unrelated type first, so every later id shifts.
    shifted_table.intern(AbiType::BorrowedString);
    let shifted_routine = add_routine(&mut shifted_table, "fol_demo_add");
    let shifted = surface_with(vec![shifted_routine], shifted_table);

    assert_eq!(
        compare_surfaces(&baseline, &shifted),
        AbiCompatibility::MinorCompatible,
        "a renumbered type id must not read as a signature change"
    );
}

/// Exported symbols come out sorted, so two identical surfaces produce the same
/// allowlist file.
#[test]
fn exported_symbols_are_sorted() {
    let mut table = AbiTypeTable::new();
    let sub = add_routine(&mut table, "fol_demo_sub");
    let add = add_routine(&mut table, "fol_demo_add");
    let surface = surface_with(vec![sub, add], table);
    assert_eq!(
        surface.exported_symbols(),
        vec!["fol_demo_add", "fol_demo_sub"]
    );
}

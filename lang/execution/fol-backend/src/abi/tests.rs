use super::{header, status, wrapper};
use fol_abi::*;

fn target() -> fol_types::ResolvedTarget {
    fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu").unwrap()
}

/// A surface with no records needs no internal record paths.
fn no_records() -> std::collections::BTreeMap<String, String> {
    std::collections::BTreeMap::new()
}

fn scalar_routine(
    table: &mut AbiTypeTable,
    symbol: &str,
    param: AbiScalar,
    result: Option<AbiScalar>,
    error: Option<AbiScalar>,
) -> ForeignRoutine {
    let param_id = table.intern(AbiType::Scalar(param));
    let result_id = match result {
        Some(scalar) => table.intern(AbiType::Scalar(scalar)),
        None => table.intern(AbiType::Void),
    };
    let error_contract = match error {
        Some(scalar) => AbiErrorContract::Recoverable {
            error_type: table.intern(AbiType::Scalar(scalar)),
        },
        None => AbiErrorContract::Infallible,
    };
    ForeignRoutine {
        fol_path: format!("api::{symbol}"),
        symbol: symbol.to_string(),
        facing: AbiFacing::Export,
        convention: AbiCallingConvention::C,
        parameters: vec![AbiParameter {
            name: "value".to_string(),
            type_id: param_id,
            direction: AbiDirection::In,
        }],
        result: result_id,
        error: error_contract,
        selection: ExportSelection {
            package_visible: true,
            abi_selected: true,
        },
        effects: AbiEffects::default(),
        origin: AbiSourceOrigin::default(),
    }
}

fn surface(routines: Vec<ForeignRoutine>, types: AbiTypeTable) -> ResolvedAbiSurface {
    ResolvedAbiSurface {
        artifact: "demo".to_string(),
        major: 1,
        minor: 0,
        interface: ForeignInterfaceTemplate { types, routines }.resolve(target()),
    }
}

/// A scalar export becomes a public wrapper calling a private FOL routine.
#[test]
fn a_scalar_export_renders_a_wrapper() {
    let mut table = AbiTypeTable::new();
    let routine = scalar_routine(
        &mut table,
        "fol_demo_double",
        AbiScalar::Int(fol_types::IntWidth::I64),
        Some(AbiScalar::Int(fol_types::IntWidth::I64)),
        None,
    );
    let rendered = wrapper::render_wrapper(
        &table,
        &routine,
        "packages::api::fn__double__r7",
        &no_records(),
    );

    assert!(rendered.contains("#[unsafe(no_mangle)]"));
    assert!(rendered.contains("pub unsafe extern \"C\" fn fol_demo_double("));
    assert!(rendered.contains("value: i64"));
    assert!(rendered.contains("out_result: *mut i64"));
    assert!(rendered.contains("-> i32"));
    // The public symbol carries no internal ID; the private call does.
    assert!(rendered.contains("packages::api::fn__double__r7"));
    assert!(!rendered.contains("fn fol_demo_double__r7"));
}

/// A null required out pointer is refused before any work runs.
#[test]
fn a_null_out_pointer_returns_invalid_argument() {
    let mut table = AbiTypeTable::new();
    let routine = scalar_routine(
        &mut table,
        "fol_demo_id",
        AbiScalar::Int(fol_types::IntWidth::I32),
        Some(AbiScalar::Int(fol_types::IntWidth::I32)),
        None,
    );
    let rendered = wrapper::render_wrapper(&table, &routine, "internal", &no_records());

    let guard = rendered
        .find("out_result.is_null()")
        .expect("a required out pointer must be checked");
    let call = rendered.find("catch_unwind").expect("the call is wrapped");
    assert!(
        guard < call,
        "the null check must run before the call, so a failing call has no side effects"
    );
    assert!(rendered.contains(status::INVALID_ARGUMENT));
}

/// Inbound booleans and characters are validated, not transmuted.
#[test]
fn inbound_scalars_are_validated() {
    let mut table = AbiTypeTable::new();
    let boolean = scalar_routine(
        &mut table,
        "fol_demo_not",
        AbiScalar::Bool,
        Some(AbiScalar::Bool),
        None,
    );
    let rendered = wrapper::render_wrapper(&table, &boolean, "internal", &no_records());
    assert!(
        rendered.contains("value: u8"),
        "a C boolean crosses as uint8_t"
    );
    assert!(rendered.contains("0 => false"));
    assert!(rendered.contains("1 => true"));
    assert!(
        rendered.contains(&format!("_ => return {}", status::INVALID_ARGUMENT)),
        "any other bit pattern must be refused"
    );

    let mut table = AbiTypeTable::new();
    let character = scalar_routine(
        &mut table,
        "fol_demo_upper",
        AbiScalar::Char,
        Some(AbiScalar::Char),
        None,
    );
    let rendered = wrapper::render_wrapper(&table, &character, "internal", &no_records());
    assert!(rendered.contains("value: u32"));
    assert!(
        rendered.contains("char::from_u32"),
        "a surrogate or out-of-range code point is not a Unicode scalar value"
    );
}

/// A panic is contained and reported, never unwound into C.
#[test]
fn a_panic_is_contained() {
    let mut table = AbiTypeTable::new();
    let routine = scalar_routine(
        &mut table,
        "fol_demo_boom",
        AbiScalar::Int(fol_types::IntWidth::I64),
        None,
        None,
    );
    let rendered = wrapper::render_wrapper(&table, &routine, "internal", &no_records());
    assert!(rendered.contains("catch_unwind"));
    assert!(rendered.contains(&format!("Err(_) => {}", status::PANIC)));
}

/// A report writes only the error out; the success out stays uninitialized.
#[test]
fn a_report_initializes_only_the_error_out() {
    let mut table = AbiTypeTable::new();
    let routine = scalar_routine(
        &mut table,
        "fol_demo_checked",
        AbiScalar::Int(fol_types::IntWidth::I64),
        Some(AbiScalar::Int(fol_types::IntWidth::I64)),
        Some(AbiScalar::Int(fol_types::IntWidth::I64)),
    );
    let rendered = wrapper::render_wrapper(&table, &routine, "internal", &no_records());

    assert!(rendered.contains("out_error: *mut i64"));
    assert!(rendered.contains("rt::abi::split_recoverable"));
    let error_arm = rendered
        .split("Err(__fol_error)")
        .nth(1)
        .expect("the error arm should exist");
    assert!(error_arm.contains("*out_error"));
    assert!(
        !error_arm.contains("*out_result"),
        "a report must not write the success out"
    );
    assert!(error_arm.contains(status::REPORT));
}

/// A no-value routine still returns a status.
#[test]
fn a_no_value_routine_still_returns_a_status() {
    let mut table = AbiTypeTable::new();
    let routine = scalar_routine(
        &mut table,
        "fol_demo_ping",
        AbiScalar::Int(fol_types::IntWidth::I32),
        None,
        None,
    );
    let rendered = wrapper::render_wrapper(&table, &routine, "internal", &no_records());
    assert!(!rendered.contains("out_result"));
    assert!(rendered.contains("-> i32"));
    assert!(rendered.contains(&format!("Ok(_) => {}", status::OK)));
}

/// The header matches the frozen section 4.16 shape.
#[test]
fn the_header_matches_the_frozen_shape() {
    let mut table = AbiTypeTable::new();
    let routine = scalar_routine(
        &mut table,
        "fol_demo_add",
        AbiScalar::Int(fol_types::IntWidth::I64),
        Some(AbiScalar::Int(fol_types::IntWidth::I64)),
        None,
    );
    let rendered = header::render_header(&surface(vec![routine], table));

    assert_eq!(header::include_guard("demo"), "FOL_DEMO_H");
    assert!(rendered.contains("#ifndef FOL_DEMO_H"));
    assert!(rendered.contains("typedef int32_t fol_status_t;"));
    assert!(rendered.contains("typedef uint8_t fol_bool_t;"));
    assert!(rendered.contains("typedef uint32_t fol_char_t;"));
    assert!(rendered.contains("#define FOL_STATUS_OK 0"));
    assert!(rendered.contains("#define FOL_STATUS_INVALID_ARGUMENT (-1)"));
    assert!(rendered.contains("extern \"C\" {"));
    assert!(rendered.contains("fol_status_t fol_demo_add(int64_t value, int64_t *out_result);"));
    assert!(rendered.ends_with("#endif /* FOL_DEMO_H */\n"));
}

/// A guard cannot collide across artifacts, and non-identifier characters are
/// replaced.
#[test]
fn include_guards_are_derived_safely() {
    assert_eq!(header::include_guard("my-lib"), "FOL_MY_LIB_H");
    assert_eq!(header::include_guard("a.b"), "FOL_A_B_H");
    assert_ne!(
        header::include_guard("demo"),
        header::include_guard("demo2")
    );
}

/// Two clean renders are byte-identical, and routine declaration order does not
/// change the header.
#[test]
fn the_header_is_deterministic_and_order_independent() {
    let build = |reversed: bool| {
        let mut table = AbiTypeTable::new();
        let add = scalar_routine(
            &mut table,
            "fol_demo_add",
            AbiScalar::Int(fol_types::IntWidth::I64),
            Some(AbiScalar::Int(fol_types::IntWidth::I64)),
            None,
        );
        let sub = scalar_routine(
            &mut table,
            "fol_demo_sub",
            AbiScalar::Int(fol_types::IntWidth::I64),
            Some(AbiScalar::Int(fol_types::IntWidth::I64)),
            None,
        );
        let routines = if reversed {
            vec![sub, add]
        } else {
            vec![add, sub]
        };
        header::render_header(&surface(routines, table))
    };
    assert_eq!(build(false), build(false));
    assert_eq!(build(false), build(true));
}

/// The status values agree across all three places they appear.
#[test]
fn status_values_agree_across_rust_c_and_metadata() {
    let expected = [
        ("FOL_STATUS_OK", 0, status::OK),
        ("FOL_STATUS_REPORT", 1, status::REPORT),
        ("FOL_STATUS_INVALID_ARGUMENT", -1, status::INVALID_ARGUMENT),
        ("FOL_STATUS_PANIC", -2, status::PANIC),
        ("FOL_STATUS_INTERNAL", -3, status::INTERNAL),
    ];
    for (name, value, rust) in expected {
        assert_eq!(rust, format!("{value}i32"), "{name} rust literal");
        let listed = fol_abi::STATUS_VALUES
            .iter()
            .find(|(listed, _, _)| *listed == name)
            .unwrap_or_else(|| panic!("{name} missing from the metadata API"));
        assert_eq!(listed.1, value, "{name} metadata value");
    }
}

/// A borrowed `str` parameter renders a view struct and validates it.
///
/// The three checks are the whole safety argument for the inbound direction:
/// a null pointer is only legal when the length is zero, a length no
/// allocation could have is refused before it can become a slice, and the
/// bytes are decoded rather than trusted. Each refusal is a status, not a
/// panic, because a caller passing bad text is not a FOL fault.
#[test]
fn a_borrowed_string_parameter_is_validated_on_entry() {
    let mut table = AbiTypeTable::new();
    let view = table.intern(AbiType::BorrowedString);
    let result = table.intern(AbiType::Scalar(AbiScalar::Int(fol_types::IntWidth::I64)));
    let routine = ForeignRoutine {
        fol_path: "api::text_length".to_string(),
        symbol: "fol_demo_text_length".to_string(),
        facing: AbiFacing::Export,
        convention: AbiCallingConvention::C,
        parameters: vec![AbiParameter {
            name: "text".to_string(),
            type_id: view,
            direction: AbiDirection::In,
        }],
        result,
        error: AbiErrorContract::Infallible,
        selection: ExportSelection {
            package_visible: true,
            abi_selected: true,
        },
        effects: AbiEffects::default(),
        origin: AbiSourceOrigin::default(),
    };

    let structs = wrapper::render_record_structs(&table);
    assert!(structs.contains("pub struct FolAbiStrView"));
    assert!(structs.contains("pub ptr: *const u8"));
    assert!(structs.contains("pub len: usize"));

    let rendered = wrapper::render_wrapper(&table, &routine, "internal", &no_records());
    assert!(rendered.contains("text: FolAbiStrView"));
    assert!(rendered.contains("text.ptr.is_null() && text.len != 0"));
    assert!(rendered.contains("text.len > isize::MAX as usize"));
    assert!(rendered.contains("core::str::from_utf8"));
    assert!(
        rendered.matches(status::INVALID_ARGUMENT).count() >= 3,
        "each rejection must return a status rather than panic:\n{rendered}"
    );

    // The header side of the same contract.
    let header = header::render_header(&surface(vec![routine], table));
    assert!(header.contains("} fol_str_view_t;"));
    assert!(header.contains("fol_str_view_t text"));
    assert!(header.contains("never retains `ptr`"));
}

/// A `str` may be lent into a call but not handed back out of one.
///
/// Returning one would give C a pointer into FOL's own storage with no answer
/// to who frees it, which is the owned-buffer contract of section 12.4 rather
/// than a borrow. The rejection names the position so the message is about
/// the return, not about `str` in general.
#[test]
fn a_borrowed_string_cannot_outlive_the_call() {
    use fol_abi::{verify_type_at, AbiPosition, CandidateType};

    let inbound = verify_type_at(
        "text_length",
        &CandidateType::BorrowedString,
        AbiPosition::Parameter,
    );
    assert!(
        inbound.is_empty(),
        "lending text into a call is the supported direction: {inbound:?}"
    );

    for position in [AbiPosition::Result, AbiPosition::Error] {
        let outbound = verify_type_at("text_length", &CandidateType::BorrowedString, position);
        let rejection = outbound
            .first()
            .unwrap_or_else(|| panic!("a borrowed view in {position:?} position must be refused"));
        assert_eq!(rejection.rejection.reason(), "borrowed-view-outlives-call");
        assert!(rejection.to_string().contains(position.as_str()));
    }
}

/// The generated header must compile as C11 and be includable from C++.
///
/// Section 4.16 freezes the shape; this proves a compiler accepts it. The C++
/// side is an `extern "C"` smoke test only -- it is not C++ ABI support.
mod compiles {
    use super::*;
    use std::process::Command;

    fn compiler() -> Option<String> {
        if let Some(clang) = std::env::var_os("FOL_H7_CLANG") {
            return clang.to_str().map(str::to_string);
        }
        for candidate in ["clang", "cc", "gcc"] {
            if Command::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
            {
                return Some(candidate.to_string());
            }
        }
        None
    }

    /// One export of every scalar in the M5 slice.
    fn slice_surface() -> ResolvedAbiSurface {
        let mut table = AbiTypeTable::new();
        let void_id = table.intern(AbiType::Void);
        let mut routines = Vec::new();
        let mut add = |name: &str, param: AbiScalar, result: Option<AbiTypeId>| {
            let param_id = table.intern(AbiType::Scalar(param));
            routines.push(ForeignRoutine {
                fol_path: format!("api::{name}"),
                symbol: name.to_string(),
                facing: AbiFacing::Export,
                convention: AbiCallingConvention::C,
                parameters: vec![AbiParameter {
                    name: "value".to_string(),
                    type_id: param_id,
                    direction: AbiDirection::In,
                }],
                result: result.unwrap_or(param_id),
                error: AbiErrorContract::Infallible,
                selection: ExportSelection {
                    package_visible: true,
                    abi_selected: true,
                },
                effects: AbiEffects::default(),
                origin: AbiSourceOrigin::default(),
            });
        };
        for width in [
            fol_types::IntWidth::I8,
            fol_types::IntWidth::I16,
            fol_types::IntWidth::I32,
            fol_types::IntWidth::I64,
            fol_types::IntWidth::U8,
            fol_types::IntWidth::U16,
            fol_types::IntWidth::U32,
            fol_types::IntWidth::U64,
        ] {
            add(
                &format!("fol_demo_{}", width.as_str()),
                AbiScalar::Int(width),
                None,
            );
        }
        add(
            "fol_demo_f32",
            AbiScalar::Float(fol_types::FloatWidth::F32),
            None,
        );
        add(
            "fol_demo_f64",
            AbiScalar::Float(fol_types::FloatWidth::F64),
            None,
        );
        add("fol_demo_flag", AbiScalar::Bool, None);
        add("fol_demo_glyph", AbiScalar::Char, None);
        add(
            "fol_demo_ping",
            AbiScalar::Int(fol_types::IntWidth::I64),
            Some(void_id),
        );

        ResolvedAbiSurface {
            artifact: "demo".to_string(),
            major: 1,
            minor: 0,
            interface: ForeignInterfaceTemplate {
                types: table,
                routines,
            }
            .resolve(target()),
        }
    }

    fn compile(cc: &str, dir: &std::path::Path, source: &str, name: &str, std_flag: &str) {
        let path = dir.join(name);
        std::fs::write(&path, source).expect("the probe should be writable");
        let output = Command::new(cc)
            .args([std_flag, "-Wall", "-Wextra", "-Werror", "-c", "-o"])
            .arg(dir.join(format!("{name}.o")))
            .arg("-I")
            .arg(dir)
            .arg(&path)
            .output()
            .expect("the compiler should run");
        assert!(
            output.status.success(),
            "the generated header failed to compile {name}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write_header(dir: &std::path::Path) {
        std::fs::create_dir_all(dir).expect("fixture root");
        std::fs::write(dir.join("demo.h"), header::render_header(&slice_surface()))
            .expect("the header should be writable");
    }

    #[test]
    fn the_generated_header_compiles_as_c11() {
        let Some(cc) = compiler() else {
            eprintln!("skipping: no C compiler on PATH");
            return;
        };
        let fixture = fol_testkit::TempFixture::new("fol_generated_header_c11");
        write_header(fixture.path());
        compile(
            &cc,
            fixture.path(),
            "#include \"demo.h\"\n\
             int probe(void) {\n\
             \x20   int64_t out = 0;\n\
             \x20   if (fol_demo_i64(21, &out) != FOL_STATUS_OK) { return 1; }\n\
             \x20   fol_bool_t flag = 1;\n\
             \x20   fol_char_t ch = 65;\n\
             \x20   (void)flag; (void)ch;\n\
             \x20   return (int)out;\n\
             }\n",
            "probe.c",
            "-std=c11",
        );
    }

    #[test]
    fn the_generated_header_is_includable_from_cxx() {
        let Some(cc) = compiler() else {
            eprintln!("skipping: no C compiler on PATH");
            return;
        };
        let cxx = if cc.ends_with("clang") {
            cc.replace("clang", "clang++")
        } else if cc.ends_with("gcc") {
            cc.replace("gcc", "g++")
        } else {
            "c++".to_string()
        };
        if Command::new(&cxx).arg("--version").output().is_err() {
            eprintln!("skipping: no C++ compiler on PATH");
            return;
        }
        let fixture = fol_testkit::TempFixture::new("fol_generated_header_cxx");
        write_header(fixture.path());
        compile(
            &cxx,
            fixture.path(),
            "#include \"demo.h\"\n\
             int probe() {\n\
             \x20   int64_t out = 0;\n\
             \x20   return fol_demo_i64(21, &out) == FOL_STATUS_OK ? (int)out : 1;\n\
             }\n",
            "probe.cpp",
            "-std=c++17",
        );
    }
}

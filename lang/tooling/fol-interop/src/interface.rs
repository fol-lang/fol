//! Projecting one accepted GERC bundle into a FOL-callable surface.
//!
//! This is the join between three things written by different parties: the
//! header (the provider author), the overlay (the FOL author), and the
//! measurement (the provider's own compiler, through PARC/LINC/GERC). A
//! declaration becomes callable only when all three agree.
//!
//! FOL does not re-derive any of it. GERC decides what the raw declaration is
//! and LINC decides which provider defines it; what happens here is acceptance,
//! and every path out is either an `ImportedRoutine` or a refusal that names
//! the symbol.

use fol_abi::{
    scalar_for_measured_float, scalar_for_measured_integer, verify_effects, verify_status_mapping,
    AbiCallingConvention, AbiDirection, AbiEscape, AbiMutability, AbiNullability, AbiOwnership,
    AbiParameter, AbiSourceOrigin, AbiType, AbiTypeId, AbiTypeTable, AnnotationOverlay,
    CapabilityModel, ImportRejection, ImportedInterface, ImportedRoutine,
};
use gerc::{GenerationBundle, RustAbi, RustFunction, RustItem, RustScalar, RustType, RustTypeKind};
use parc::contract::CompleteSourcePackage;

/// Build the accepted surface for one import.
///
/// Rejections are collected rather than returned one at a time: a header with
/// four unimportable declarations should report four problems, not make the
/// author rediscover them one build at a time.
pub fn project_imported_interface(
    alias: &str,
    target: fol_types::ResolvedTarget,
    source: &CompleteSourcePackage,
    bundle: &GenerationBundle,
    overlay: &AnnotationOverlay,
    model: CapabilityModel,
) -> Result<ImportedInterface, Vec<ImportRejection>> {
    let mut types = AbiTypeTable::new();
    let mut routines = Vec::new();
    let mut rejections = Vec::new();

    let mut projected: Vec<&str> = Vec::new();
    for item in bundle.projection().items() {
        let RustItem::Function(function) = item else {
            // Records, enums, and aliases reach FOL only through a routine
            // that uses them; a bare type declaration exports nothing.
            continue;
        };
        let symbol = function.link_name();
        projected.push(symbol);
        // A declaration the overlay does not name is not callable. This is
        // what lets a header carry more than the FOL surface.
        let Some(annotation) = overlay.routine(symbol) else {
            continue;
        };
        match project_routine(function, annotation, source, &mut types, model) {
            Ok(routine) => routines.push(routine),
            Err(rejection) => rejections.push(rejection),
        }
    }

    // The reverse direction: an overlay entry with no declaration behind it is
    // a typo that would otherwise fail much later, as a missing symbol.
    for annotation in overlay.routines() {
        if !projected.contains(&annotation.symbol.as_str()) {
            rejections.push(ImportRejection::UnknownDeclaration {
                symbol: annotation.symbol.clone(),
            });
        }
    }

    if !rejections.is_empty() {
        rejections.sort_by(|left, right| left.symbol().cmp(right.symbol()));
        return Err(rejections);
    }

    routines.sort_by(|left, right| left.symbol.cmp(&right.symbol));
    Ok(ImportedInterface {
        alias: alias.to_string(),
        target,
        types,
        routines,
    })
}

fn project_routine(
    function: &RustFunction,
    annotation: &fol_abi::RoutineAnnotation,
    source: &CompleteSourcePackage,
    types: &mut AbiTypeTable,
    model: CapabilityModel,
) -> Result<ImportedRoutine, ImportRejection> {
    let symbol = function.link_name().to_string();

    // PARC/GERC already decided this declaration is not fully understood;
    // section 4.13 makes that a hard gate rather than a warning.
    if !function.source().support().is_supported() {
        return Err(ImportRejection::UnsupportedDeclaration {
            symbol,
            detail: "the C front end could not fully model this declaration".to_string(),
        });
    }
    if function.variadic() {
        return Err(ImportRejection::Variadic { symbol });
    }
    if function.abi() != RustAbi::C {
        return Err(ImportRejection::UnsupportedConvention {
            symbol,
            convention: format!("{:?}", function.abi()),
        });
    }

    // Where the handle sits, decided before any type is projected.
    //
    // A handle's pointee is a C incomplete type, which `project_type` refuses
    // by design -- nothing may read through it. So the handle position is
    // identified first and skipped, rather than projected and then replaced:
    // projecting it would mean refusing the one shape the overlay just
    // declared legal.
    let handle_positions = handle_positions(&symbol, function, annotation)?;
    // The callback's two positions, and the `AbiType::Callback` its function
    // pointer becomes. Resolved before projection for the same reason a handle
    // is: `project_type` refuses a function pointer by design, so replacing it
    // afterwards would mean refusing the shape the overlay just declared legal.
    let callback = callback_positions(&symbol, function, annotation, types)?;

    let result = match handle_positions.result {
        Some(domain) => types.intern(AbiType::OpaqueHandle { name: domain }),
        None => project_type(&symbol, function.return_type(), types)?,
    };
    let mut parameters = Vec::new();
    for (index, parameter) in function.parameters().iter().enumerate() {
        let type_id = match handle_positions.parameter.as_ref() {
            Some((position, domain)) if *position == index => types.intern(AbiType::OpaqueHandle {
                name: domain.clone(),
            }),
            _ if callback.as_ref().is_some_and(|c| c.parameter == index) => {
                callback.as_ref().expect("checked").type_id
            }
            _ => project_type(&symbol, parameter.ty(), types)?,
        };
        // A C parameter may be unnamed; the position is the only stable
        // identity then, and the overlay has to be able to name it.
        let name = parameter
            .source_name()
            .map(|name| name.original.clone())
            .unwrap_or_else(|| format!("arg{index}"));
        parameters.push(AbiParameter {
            name,
            type_id,
            direction: direction_for(parameter.ty()),
        });
    }

    verify_status_mapping(annotation, &parameters, result, types)?;
    verify_effects(annotation, model)?;

    Ok(ImportedRoutine {
        symbol,
        fol_name: annotation.fol_name.clone(),
        convention: AbiCallingConvention::C,
        parameters,
        result,
        error: annotation.error.clone(),
        effects: annotation.effects,
        handle: annotation.handle.clone(),
        callback: annotation.callback.clone(),
        origin: origin_for(function, source),
    })
}

/// Where a routine's callback sits, and what its FOL-visible type is.
struct CallbackPositions {
    /// The index of the function-pointer parameter.
    parameter: usize,
    /// The interned `AbiType::Callback`.
    type_id: AbiTypeId,
}

/// Validate the canonical callback shape and intern the callback type.
///
/// V4 imports exactly one shape: a function pointer whose own first parameter
/// is the `void *` context, plus a separate `void *` parameter carrying that
/// context. Every other arrangement is refused rather than guessed at, because
/// guessing which argument is the context is how a provider gets handed an
/// address that is not one.
fn callback_positions(
    symbol: &str,
    function: &RustFunction,
    annotation: &fol_abi::RoutineAnnotation,
    types: &mut AbiTypeTable,
) -> Result<Option<CallbackPositions>, ImportRejection> {
    let Some(use_) = &annotation.callback else {
        return Ok(None);
    };

    let named = |wanted: &str| -> Option<usize> {
        function.parameters().iter().position(|parameter| {
            parameter
                .source_name()
                .is_some_and(|name| name.original == wanted)
        })
    };
    let parameter =
        named(&use_.parameter).ok_or_else(|| ImportRejection::UnknownCallbackParameter {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
            role: "function pointer",
        })?;
    let context =
        named(&use_.context).ok_or_else(|| ImportRejection::UnknownCallbackParameter {
            symbol: symbol.to_string(),
            parameter: use_.context.clone(),
            role: "context",
        })?;

    // The context must be a pointer FOL can hand an arbitrary address through.
    if !matches!(
        function.parameters()[context].ty().kind(),
        RustTypeKind::Pointer(_)
    ) {
        return Err(ImportRejection::CallbackShapeMismatch {
            symbol: symbol.to_string(),
            parameter: use_.context.clone(),
            expected: "a pointer",
        });
    }

    let RustTypeKind::FunctionPointer {
        abi,
        parameters: signature,
        return_type,
        variadic,
    } = function.parameters()[parameter].ty().kind()
    else {
        return Err(ImportRejection::CallbackShapeMismatch {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
            expected: "a function pointer",
        });
    };

    if *variadic {
        return Err(ImportRejection::UnsupportedCallbackSignature {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
            detail: "it is variadic, and FOL has no call form that supplies a variadic list"
                .to_string(),
        });
    }
    if *abi != RustAbi::C {
        return Err(ImportRejection::UnsupportedCallbackSignature {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
            detail: format!("its calling convention is {abi:?}, and only C is imported"),
        });
    }
    // The first parameter is the context handed back. Without it the provider
    // has nowhere to return the pointer FOL gave it, so a FOL closure could
    // never be recovered.
    let Some(first) = signature.first() else {
        return Err(ImportRejection::CallbackContextNotFirst {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
        });
    };
    if !matches!(first.kind(), RustTypeKind::Pointer(_)) {
        return Err(ImportRejection::CallbackContextNotFirst {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
        });
    }

    // Everything after the context is what a FOL routine value receives.
    let mut projected = Vec::new();
    for argument in signature.iter().skip(1) {
        projected.push(project_type(symbol, argument, types)?);
    }
    let result = project_type(symbol, return_type, types)?;
    let type_id = types.intern(AbiType::Callback {
        parameters: projected,
        result,
    });
    Ok(Some(CallbackPositions { parameter, type_id }))
}

/// Which of a routine's positions carry the handle.
#[derive(Default)]
struct HandlePositions {
    /// The domain, when the C result is the handle.
    result: Option<String>,
    /// The parameter index and domain, when a parameter is the handle.
    parameter: Option<(usize, String)>,
}

/// Decide which position is the handle, or say why it is not decidable.
///
/// Getting this wrong means calling a destroy on the wrong address, so it is
/// never resolved by position: a producer's handle is its result, and a
/// borrower's or consumer's is its *single* pointer parameter. Anything else
/// is refused.
fn handle_positions(
    symbol: &str,
    function: &RustFunction,
    annotation: &fol_abi::RoutineAnnotation,
) -> Result<HandlePositions, ImportRejection> {
    let Some(use_) = &annotation.handle else {
        return Ok(HandlePositions::default());
    };

    if use_.role == fol_abi::HandleRole::Produces {
        if !matches!(function.return_type().kind(), RustTypeKind::Pointer(_)) {
            return Err(ImportRejection::HandleResultIsNotAPointer {
                symbol: symbol.to_string(),
                domain: use_.domain.clone(),
                found: format!("{:?}", function.return_type().kind()),
            });
        }
        return Ok(HandlePositions {
            result: Some(use_.domain.clone()),
            parameter: None,
        });
    }

    let pointers: Vec<usize> = function
        .parameters()
        .iter()
        .enumerate()
        .filter(|(_, parameter)| matches!(parameter.ty().kind(), RustTypeKind::Pointer(_)))
        .map(|(index, _)| index)
        .collect();
    let [index] = pointers[..] else {
        return Err(ImportRejection::AmbiguousHandleParameter {
            symbol: symbol.to_string(),
            domain: use_.domain.clone(),
            found: pointers.len(),
        });
    };
    Ok(HandlePositions {
        result: None,
        parameter: Some((index, use_.domain.clone())),
    })
}

/// A `const` pointer is an input; a writable one may be written through.
///
/// This is a default, not a promise: the overlay's status mapping is what
/// actually makes a parameter an out-parameter, and it is checked separately.
fn direction_for(ty: &RustType) -> AbiDirection {
    match ty.kind() {
        RustTypeKind::Pointer(target) if !target.qualifiers().is_const => AbiDirection::Out,
        _ => AbiDirection::In,
    }
}

/// Resolve a declaration's header location into a path and a line.
///
/// PARC records a byte offset against a file id; a diagnostic needs a path and
/// a line, and `line_starts` is what converts between them. Doing it here
/// means every later consumer gets a location it can print or navigate to.
fn origin_for(function: &RustFunction, source: &CompleteSourcePackage) -> AbiSourceOrigin {
    let Some(occurrence) = function.source().occurrences().first() else {
        return AbiSourceOrigin::default();
    };
    let range = occurrence.range;
    let Some(file) = source
        .source()
        .files()
        .iter()
        .find(|file| file.id == range.file)
    else {
        return AbiSourceOrigin::default();
    };
    // `line_starts` is sorted, so the last start at or before the offset is
    // the line the offset falls on.
    let line_index = file
        .line_starts
        .partition_point(|start| *start <= range.start)
        .saturating_sub(1);
    let line_start = file.line_starts.get(line_index).copied().unwrap_or(0);
    AbiSourceOrigin {
        file: file.logical_path.clone(),
        line: (line_index as u32).saturating_add(1),
        column: u32::try_from(range.start.saturating_sub(line_start))
            .unwrap_or(u32::MAX)
            .saturating_add(1),
    }
}

/// Intern one measured C type, or say why it does not cross.
fn project_type(
    symbol: &str,
    ty: &RustType,
    types: &mut AbiTypeTable,
) -> Result<AbiTypeId, ImportRejection> {
    if !ty.support().is_supported() {
        return Err(ImportRejection::UnsupportedDeclaration {
            symbol: symbol.to_string(),
            detail: "one of its types could not be fully modelled".to_string(),
        });
    }
    // A volatile or atomic value has access rules FOL does not reproduce, so
    // reading one through an ordinary load would be wrong rather than slow.
    let qualifiers = ty.qualifiers();
    if qualifiers.is_volatile || qualifiers.is_atomic {
        return Err(ImportRejection::UnsupportedDeclaration {
            symbol: symbol.to_string(),
            detail: "a volatile or atomic type does not have FOL's access rules".to_string(),
        });
    }

    let abi_type = match ty.kind() {
        RustTypeKind::Void => AbiType::Void,
        RustTypeKind::Scalar(scalar) => AbiType::Scalar(project_scalar(symbol, *scalar)?),
        RustTypeKind::Pointer(target) => {
            let target_id = project_type(symbol, target, types)?;
            AbiType::Pointer {
                target: target_id,
                mutability: if target.qualifiers().is_const {
                    AbiMutability::Const
                } else {
                    AbiMutability::Mutable
                },
                // M6 imports scalars and the out-parameters that carry them.
                // Section 4.8's nullability and ownership vocabulary is
                // recorded, and M7 is what lets an overlay set it.
                nullability: AbiNullability::NonNull,
                ownership: AbiOwnership::Borrowed,
                escape: AbiEscape::CallScoped,
                destructor: None,
            }
        }
        RustTypeKind::FunctionPointer { .. } => {
            return Err(ImportRejection::UnsupportedDeclaration {
                symbol: symbol.to_string(),
                detail: "a function pointer parameter has no FOL counterpart yet".to_string(),
            })
        }
        RustTypeKind::FlexibleArray { .. } => {
            return Err(ImportRejection::UnsupportedDeclaration {
                symbol: symbol.to_string(),
                detail: "a flexible array member has no fixed size".to_string(),
            })
        }
        RustTypeKind::FixedArray { .. } => {
            return Err(ImportRejection::UnsupportedDeclaration {
                symbol: symbol.to_string(),
                detail: "an array parameter decays to a pointer whose length C does not carry"
                    .to_string(),
            })
        }
        RustTypeKind::Named { rust_name, .. } => {
            return Err(ImportRejection::UnsupportedDeclaration {
                symbol: symbol.to_string(),
                detail: format!(
                    "'{}' is a named aggregate; M7 is what imports records and handles",
                    rust_name.as_str()
                ),
            })
        }
    };
    Ok(types.intern(abi_type))
}

fn project_scalar(symbol: &str, scalar: RustScalar) -> Result<fol_abi::AbiScalar, ImportRejection> {
    // Signedness and width come from the measurement, never from the C
    // spelling: `char` is signed on one target and unsigned on another, and
    // the whole point of measuring is not to have to know which.
    let (signed, storage_bits, alignment_bits) = match scalar {
        RustScalar::Bool => return Ok(fol_abi::AbiScalar::Bool),
        RustScalar::CChar {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CSignedChar {
            storage_bits,
            alignment_bits,
        } => (true, storage_bits, alignment_bits),
        RustScalar::CUnsignedChar {
            storage_bits,
            alignment_bits,
        } => (false, storage_bits, alignment_bits),
        RustScalar::CShort {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CInt {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CLong {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CLongLong {
            storage_bits,
            alignment_bits,
        } => (true, storage_bits, alignment_bits),
        RustScalar::CUnsignedShort {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CUnsignedInt {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CUnsignedLong {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CUnsignedLongLong {
            storage_bits,
            alignment_bits,
        } => (false, storage_bits, alignment_bits),
        RustScalar::CFloat {
            storage_bits,
            alignment_bits,
        }
        | RustScalar::CDouble {
            storage_bits,
            alignment_bits,
        } => return scalar_for_measured_float(symbol, storage_bits, alignment_bits),
        RustScalar::I8 => (true, 8, 8),
        RustScalar::U8 => (false, 8, 8),
        RustScalar::I16 => (true, 16, 16),
        RustScalar::U16 => (false, 16, 16),
        RustScalar::I32 => (true, 32, 32),
        RustScalar::U32 => (false, 32, 32),
        other => {
            return Err(ImportRejection::UnsupportedDeclaration {
                symbol: symbol.to_string(),
                detail: format!("scalar {other:?} has no FOL counterpart"),
            })
        }
    };
    scalar_for_measured_integer(symbol, signed, storage_bits, alignment_bits)
}

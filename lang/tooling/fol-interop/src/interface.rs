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

    // A C typedef is a spelling, not a type: `fol_status_t` and `int32_t` are
    // both `int` on this target. Without resolving them every stdint-typed
    // declaration -- which is most real headers, and every header FOL itself
    // generates -- would be refused as a named aggregate.
    let shapes = collect_shapes(bundle);

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
        match project_routine(
            function, annotation, overlay, source, &mut types, model, &shapes,
        ) {
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
    overlay: &AnnotationOverlay,
    source: &CompleteSourcePackage,
    types: &mut AbiTypeTable,
    model: CapabilityModel,
    shapes: &Shapes<'_>,
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
    let callback = callback_positions(&symbol, function, annotation, types, shapes)?;
    // The buffer's two positions and the slice its address becomes. Resolved
    // before projection because the pair is one FOL value: projecting the
    // pointer on its own would produce the two unrelated parameters the
    // pairing exists to replace.
    let buffer = buffer_positions(&symbol, function, annotation, types, shapes)?;

    // An owned buffer's result is a transferred pointer carrying the symbol
    // that frees it. Resolved before the ordinary projection because a bare
    // pointer result has no ownership and would be refused.
    let owned = owned_buffer_result(&symbol, function, annotation, overlay, types, shapes)?;

    let result = match handle_positions.result {
        Some(domain) => types.intern(AbiType::OpaqueHandle { name: domain }),
        None if owned.is_some() => owned.as_ref().expect("checked").type_id,
        None => project_type(
            &symbol,
            function.return_type(),
            types,
            shapes,
            &fol_abi::PointerContract::default(),
        )?,
    };
    let mut parameters = Vec::new();
    let default_contract = fol_abi::PointerContract::default();
    for (index, parameter) in function.parameters().iter().enumerate() {
        // A C parameter may be unnamed; the position is the only stable
        // identity then, and the overlay has to be able to name it. Computed
        // before projection because the overlay's pointer contract is keyed by
        // this same name.
        let name = parameter
            .source_name()
            .map(|name| name.original.clone())
            .unwrap_or_else(|| format!("arg{index}"));
        let type_id = match handle_positions.parameter.as_ref() {
            Some((position, domain)) if *position == index => types.intern(AbiType::OpaqueHandle {
                name: domain.clone(),
            }),
            _ if callback.as_ref().is_some_and(|c| c.parameter == index) => {
                callback.as_ref().expect("checked").type_id
            }
            _ if buffer.as_ref().is_some_and(|b| b.parameter == index) => {
                buffer.as_ref().expect("checked").type_id
            }
            // A declared C string. Checked here rather than projected as an
            // ordinary pointer, because a pointer is what the projection would
            // produce and a pointer is what FOL cannot surface.
            _ if annotation.strings.contains(&name) => {
                string_parameter(&symbol, &name, parameter.ty(), shapes, types)?
            }
            _ => project_type(
                &symbol,
                parameter.ty(),
                types,
                shapes,
                annotation.pointers.get(&name).unwrap_or(&default_contract),
            )?,
        };
        let direction = direction_for(
            &symbol,
            &name,
            parameter.ty(),
            annotation.pointers.get(&name).unwrap_or(&default_contract),
        )?;
        parameters.push(AbiParameter {
            name,
            type_id,
            direction,
        });
    }

    // A record *parameter* is flattened into its fields at the call and rebuilt
    // inside the adapter. A record *result* would need the reverse conversion,
    // which is not built: the adapter would have to return the provider's own
    // struct, and nothing on FOL's side can name that type.
    if let Some(AbiType::Record { name, .. }) = types.get(result) {
        return Err(ImportRejection::UnsupportedDeclaration {
            symbol: symbol.clone(),
            detail: format!(
                "returns record '{name}', which does not cross yet; a record crosses as a \
                 parameter, where it is rebuilt from its fields inside the adapter"
            ),
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
        buffer: annotation.buffer.clone(),
        strings: annotation.strings.clone(),
        owned_buffer: annotation.owned_buffer.clone(),
        owned_destroy: annotation
            .owned_buffer
            .as_ref()
            .and_then(|use_| overlay.buffer(&use_.domain))
            .map(|domain| domain.destroy.clone()),
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
    shapes: &Shapes<'_>,
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

    // Both halves are read through their aliases first. A real header almost
    // always names its callback type -- `typedef int (*F)(void *, int)` -- and
    // asking whether a `Named` type is a function pointer answers no for every
    // one of them. The scalar path already resolves; this one did not, so the
    // shape a provider actually ships was refused as "not a function pointer".
    let context_type = resolve_alias(function.parameters()[context].ty(), &shapes.aliases);
    let callback_type = resolve_alias(function.parameters()[parameter].ty(), &shapes.aliases);

    // The context must be a pointer FOL can hand an arbitrary address through.
    if !matches!(context_type.kind(), RustTypeKind::Pointer(_)) {
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
    } = callback_type.kind()
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
        projected.push(project_type(
            symbol,
            argument,
            types,
            shapes,
            &fol_abi::PointerContract::default(),
        )?);
    }
    let result = project_type(
        symbol,
        return_type,
        types,
        shapes,
        &fol_abi::PointerContract::default(),
    )?;
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
/// One parameter's direction: what the overlay declared, or what constness
/// suggests when it declared nothing.
///
/// Constness is a poor witness. `void *base` in `qsort` is read and written,
/// `char *dst` in `strcpy` is only written, and a mutable pointer a provider
/// happens never to write is indistinguishable from either. The inference
/// stays as the default because most parameters are unremarkable, but a
/// declaration overrides it -- and a declaration C contradicts is refused
/// rather than silently believed.
fn direction_for(
    symbol: &str,
    name: &str,
    ty: &RustType,
    contract: &fol_abi::PointerContract,
) -> Result<AbiDirection, ImportRejection> {
    let Some(declared) = contract.direction else {
        return Ok(match ty.kind() {
            RustTypeKind::Pointer(target) if !target.qualifiers().is_const => AbiDirection::Out,
            _ => AbiDirection::In,
        });
    };
    let RustTypeKind::Pointer(target) = ty.kind() else {
        // A by-value parameter is read and nothing else; there is no second
        // reading for a direction to select between.
        return Err(ImportRejection::ContradictoryDirection {
            symbol: symbol.to_string(),
            parameter: name.to_string(),
            declared: declared.as_str(),
            detail: "only a pointer parameter has a direction",
        });
    };
    if declared != AbiDirection::In && target.qualifiers().is_const {
        return Err(ImportRejection::ContradictoryDirection {
            symbol: symbol.to_string(),
            parameter: name.to_string(),
            declared: declared.as_str(),
            detail: "C declares its pointee const, so the provider cannot write through it",
        });
    }
    Ok(declared)
}

/// A parameter the overlay declared a NUL-terminated C string.
///
/// The declaration is the whole of it: `const char *` is how C spells text and
/// equally how it spells a pointer to one byte, so nothing measured can tell
/// them apart. What is checked here is that the declaration is not absurd --
/// the parameter has to be a pointer to a character-sized scalar.
fn string_parameter(
    symbol: &str,
    name: &str,
    ty: &RustType,
    shapes: &Shapes<'_>,
    types: &mut AbiTypeTable,
) -> Result<fol_abi::AbiTypeId, ImportRejection> {
    let ty = resolve_alias(ty, &shapes.aliases);
    let RustTypeKind::Pointer(target) = ty.kind() else {
        return Err(ImportRejection::BufferShapeMismatch {
            symbol: symbol.to_string(),
            parameter: name.to_string(),
            expected: "a pointer, so it can carry a string",
        });
    };
    let element = project_type(
        symbol,
        target,
        types,
        shapes,
        &fol_abi::PointerContract::default(),
    )?;
    let byte_sized = matches!(
        types.get(element),
        Some(AbiType::Scalar(fol_abi::AbiScalar::Int(width)))
            if width.bits() == Some(8)
    );
    if !byte_sized {
        return Err(ImportRejection::BufferShapeMismatch {
            symbol: symbol.to_string(),
            parameter: name.to_string(),
            expected: "a pointer to a character-sized scalar",
        });
    }
    Ok(types.intern(AbiType::CString {
        mutability: if target.qualifiers().is_const {
            fol_abi::AbiMutability::Const
        } else {
            fol_abi::AbiMutability::Mutable
        },
    }))
}

/// The result type of a routine that produces an owned buffer.
///
/// Modelled as a transferred pointer carrying its destroy symbol, which is
/// what `AbiType::Pointer` was built to say and what nothing had yet used: the
/// address is memory FOL did not allocate and must not free itself.
struct OwnedBufferResult {
    type_id: fol_abi::AbiTypeId,
}

fn owned_buffer_result(
    symbol: &str,
    function: &RustFunction,
    annotation: &fol_abi::RoutineAnnotation,
    overlay: &AnnotationOverlay,
    types: &mut AbiTypeTable,
    shapes: &Shapes<'_>,
) -> Result<Option<OwnedBufferResult>, ImportRejection> {
    let Some(use_) = &annotation.owned_buffer else {
        return Ok(None);
    };
    // Only a producer has a buffer-shaped result. The destroy takes one back,
    // and FOL never calls it: the adapter does, before returning the copy.
    if use_.role != fol_abi::BufferRole::Produces {
        return Ok(None);
    }
    let Some(domain) = overlay.buffer(&use_.domain) else {
        return Err(ImportRejection::UnknownBufferParameter {
            symbol: symbol.to_string(),
            parameter: use_.domain.clone(),
            role: "domain",
        });
    };

    let named = |wanted: &str| -> Option<usize> {
        function.parameters().iter().position(|parameter| {
            parameter
                .source_name()
                .is_some_and(|name| name.original == wanted)
        })
    };
    // The length and the capacity come back through out-parameters, so each
    // has to be a pointer to a count rather than a count.
    for (name, role) in [
        (Some(&use_.length), "length"),
        (use_.capacity.as_ref(), "capacity"),
    ] {
        let Some(name) = name else { continue };
        let Some(index) = named(name) else {
            return Err(ImportRejection::UnknownBufferParameter {
                symbol: symbol.to_string(),
                parameter: name.clone(),
                role,
            });
        };
        let RustTypeKind::Pointer(target) = function.parameters()[index].ty().kind() else {
            return Err(ImportRejection::BufferShapeMismatch {
                symbol: symbol.to_string(),
                parameter: name.clone(),
                expected: "a pointer, so the provider can report through it",
            });
        };
        let reported = project_type(
            symbol,
            target,
            types,
            shapes,
            &fol_abi::PointerContract::default(),
        )?;
        if !matches!(
            types.get(reported),
            Some(AbiType::Scalar(fol_abi::AbiScalar::Int(width))) if !width.is_signed()
        ) {
            return Err(ImportRejection::BufferShapeMismatch {
                symbol: symbol.to_string(),
                parameter: name.clone(),
                expected: "a pointer to an unsigned integer",
            });
        }
    }

    let RustTypeKind::Pointer(target) = function.return_type().kind() else {
        return Err(ImportRejection::BufferShapeMismatch {
            symbol: symbol.to_string(),
            parameter: "<result>".to_string(),
            expected: "a pointer",
        });
    };
    let element = project_type(
        symbol,
        target,
        types,
        shapes,
        &fol_abi::PointerContract::default(),
    )?;
    if !matches!(types.get(element), Some(AbiType::Scalar(_))) {
        return Err(ImportRejection::BufferShapeMismatch {
            symbol: symbol.to_string(),
            parameter: "<result>".to_string(),
            expected: "a pointer to a sized scalar",
        });
    }

    Ok(Some(OwnedBufferResult {
        type_id: types.intern(AbiType::Pointer {
            target: element,
            mutability: fol_abi::AbiMutability::Mutable,
            // A provider is entitled to report failure by returning NULL, and
            // the adapter checks rather than assuming.
            nullability: fol_abi::AbiNullability::Nullable,
            ownership: fol_abi::AbiOwnership::Transferred,
            escape: fol_abi::AbiEscape::CallScoped,
            destructor: Some(domain.destroy.clone()),
        }),
    }))
}

/// Where a paired buffer sits, and the slice type its address becomes.
struct BufferPositions {
    parameter: usize,
    type_id: fol_abi::AbiTypeId,
}

/// Resolve and check the overlay's pointer/length pair.
///
/// Both halves have to be real parameters of this signature, the address has
/// to point at something with a size, and the length has to be a count. None
/// of that is inferable: C's two parameters are as unrelated to each other as
/// any other two.
fn buffer_positions(
    symbol: &str,
    function: &RustFunction,
    annotation: &fol_abi::RoutineAnnotation,
    types: &mut AbiTypeTable,
    shapes: &Shapes<'_>,
) -> Result<Option<BufferPositions>, ImportRejection> {
    let Some(use_) = &annotation.buffer else {
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
        named(&use_.parameter).ok_or_else(|| ImportRejection::UnknownBufferParameter {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
            role: "address",
        })?;
    let length = named(&use_.length).ok_or_else(|| ImportRejection::UnknownBufferParameter {
        symbol: symbol.to_string(),
        parameter: use_.length.clone(),
        role: "length",
    })?;

    // A count, not a number: a signed length can be negative, and there is no
    // buffer of -1 elements for FOL to hand over.
    let length_type = project_type(
        symbol,
        function.parameters()[length].ty(),
        types,
        shapes,
        &fol_abi::PointerContract::default(),
    )?;
    let counts = matches!(
        types.get(length_type),
        Some(AbiType::Scalar(fol_abi::AbiScalar::Int(width))) if !width.is_signed()
    );
    if !counts {
        return Err(ImportRejection::BufferShapeMismatch {
            symbol: symbol.to_string(),
            parameter: use_.length.clone(),
            expected: "an unsigned integer",
        });
    }

    let RustTypeKind::Pointer(target) = function.parameters()[parameter].ty().kind() else {
        return Err(ImportRejection::BufferShapeMismatch {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
            expected: "a pointer",
        });
    };
    // `void *` is the common C spelling for an untyped buffer and the one
    // shape a slice cannot be: with no element there is no element size, so
    // the length counts nothing measurable.
    let element = project_type(
        symbol,
        target,
        types,
        shapes,
        &fol_abi::PointerContract::default(),
    )?;
    if !matches!(types.get(element), Some(AbiType::Scalar(_))) {
        return Err(ImportRejection::BufferShapeMismatch {
            symbol: symbol.to_string(),
            parameter: use_.parameter.clone(),
            expected: "a pointer to a sized scalar",
        });
    }

    let mutability = if target.qualifiers().is_const {
        fol_abi::AbiMutability::Const
    } else {
        fol_abi::AbiMutability::Mutable
    };
    Ok(Some(BufferPositions {
        parameter,
        type_id: types.intern(AbiType::BorrowedSlice {
            element,
            mutability,
        }),
    }))
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

/// Every `typedef` in the projection, by the declaration a `Named` refers to.
type TypeAliases<'a> = std::collections::HashMap<parc::contract::DeclarationId, &'a RustType>;

/// The named declarations a `Named` type can resolve to.
///
/// Collected once per import rather than searched per occurrence: a header of
/// any size has every type referenced many times over.
struct Shapes<'a> {
    aliases: TypeAliases<'a>,
    records: std::collections::HashMap<parc::contract::DeclarationId, &'a gerc::RustRecord>,
    enums: std::collections::HashMap<parc::contract::DeclarationId, &'a gerc::RustEnum>,
    /// Records currently being projected, innermost last.
    ///
    /// `struct node { struct node *next; }` is an ordinary header shape and a
    /// cycle in the type graph: the pointer's target resolves back to the
    /// record being projected. Without this the projection recurses until the
    /// stack ends.
    in_progress: std::cell::RefCell<Vec<parc::contract::DeclarationId>>,
}

fn collect_shapes(bundle: &GenerationBundle) -> Shapes<'_> {
    let mut shapes = Shapes {
        aliases: TypeAliases::new(),
        records: std::collections::HashMap::new(),
        enums: std::collections::HashMap::new(),
        in_progress: std::cell::RefCell::new(Vec::new()),
    };
    for item in bundle.projection().items() {
        match item {
            RustItem::TypeAlias(alias) => {
                shapes.aliases.insert(alias.declaration(), alias.target());
            }
            RustItem::Record(record) => {
                shapes.records.insert(record.declaration(), record);
            }
            RustItem::Enum(entry) => {
                shapes.enums.insert(entry.declaration(), entry);
            }
            _ => {}
        }
    }
    shapes
}

/// Follow a chain of typedefs to the type they finally name.
///
/// `fol_status_t` is `int32_t` is `int`, so one hop is not enough. The bound is
/// the alias count: C forbids a cycle, but a malformed projection must not
/// spin here.
fn resolve_alias<'a>(ty: &'a RustType, aliases: &TypeAliases<'a>) -> &'a RustType {
    let mut current = ty;
    for _ in 0..aliases.len().saturating_add(1) {
        let RustTypeKind::Named { declaration, .. } = current.kind() else {
            return current;
        };
        match aliases.get(declaration) {
            Some(target) => current = target,
            None => return current,
        }
    }
    current
}

/// Resolve a `Named` type to the declaration behind it.
///
/// A typedef has already been followed by `resolve_alias`, so what is left is
/// a record, an enum, or a name with no definition in this projection.
fn project_named(
    symbol: &str,
    declaration: parc::contract::DeclarationId,
    rust_name: &gerc::RustName,
    types: &mut AbiTypeTable,
    shapes: &Shapes<'_>,
) -> Result<AbiTypeId, ImportRejection> {
    if let Some(record) = shapes.records.get(&declaration) {
        return project_record(symbol, record, types, shapes);
    }
    if let Some(entry) = shapes.enums.get(&declaration) {
        // A C enum is an integer with named constants, not a tagged union. It
        // crosses as the integer the target actually gave it -- projecting it
        // as a FOL entry would invent a discriminant contract C never made.
        return Ok(types.intern(AbiType::Scalar(project_scalar(symbol, entry.storage())?)));
    }
    Err(ImportRejection::UnsupportedDeclaration {
        symbol: symbol.to_string(),
        detail: format!(
            "'{}' is named but not defined in this header, so its size and layout are unknown; \
             an incomplete type crosses only as an opaque handle",
            rust_name.as_str()
        ),
    })
}

/// Project one C struct as a FOL-visible record, or say why it does not cross.
///
/// Only the shape Section 4.13 admits: a `struct` with natural `repr(C)`
/// layout and byte-aligned fields. A union has no discriminant to read, an
/// incomplete record is the handle path and must not be read through, and a
/// packed or bitfield layout has no independently addressable field.
fn project_record(
    symbol: &str,
    record: &gerc::RustRecord,
    types: &mut AbiTypeTable,
    shapes: &Shapes<'_>,
) -> Result<AbiTypeId, ImportRejection> {
    let name = record.rust_name().as_str().to_owned();
    let reject = |detail: String| ImportRejection::UnsupportedDeclaration {
        symbol: symbol.to_string(),
        detail,
    };

    match record.kind() {
        gerc::RustRecordKind::Struct => {}
        gerc::RustRecordKind::Union => {
            return Err(reject(format!(
                "'{name}' is a union: C carries no discriminant, so nothing says which member \
                 is live"
            )))
        }
        gerc::RustRecordKind::Opaque => {
            return Err(reject(format!(
                "'{name}' is incomplete, so it cannot be read through; declare it as a handle \
                 domain to pass it as an address"
            )))
        }
    }
    if record.packing_bits().is_some() {
        return Err(reject(format!(
            "'{name}' has a packed layout, whose fields are not independently addressable"
        )));
    }

    // A record reachable from itself has no finite projection. C forbids
    // containing itself *by value*, but a pointer back to it is ordinary --
    // a list node -- and that is the cycle to stop.
    if shapes.in_progress.borrow().contains(&record.declaration()) {
        return Err(reject(format!(
            "'{name}' refers to itself, so it has no finite FOL shape; pass it as a pointer \
             the overlay declares, or as a handle domain"
        )));
    }
    shapes.in_progress.borrow_mut().push(record.declaration());
    let projected = project_record_fields(symbol, record, &name, types, shapes);
    shapes.in_progress.borrow_mut().pop();
    let fields = projected?;

    Ok(types.intern(AbiType::Record { name, fields }))
}

/// The field walk, split out so the cycle guard above is released on every
/// path including a rejection.
fn project_record_fields(
    symbol: &str,
    record: &gerc::RustRecord,
    name: &str,
    types: &mut AbiTypeTable,
    shapes: &Shapes<'_>,
) -> Result<Vec<fol_abi::AbiField>, ImportRejection> {
    let reject = |detail: String| ImportRejection::UnsupportedDeclaration {
        symbol: symbol.to_string(),
        detail,
    };
    let mut fields = Vec::new();
    for field in record.fields() {
        let field_name = field
            .source_name()
            .map(|source| source.original.clone())
            .unwrap_or_else(|| field.rust_name().as_str().to_owned());
        if !field.support().is_supported() {
            return Err(reject(format!(
                "'{name}' has field '{field_name}', which the C front end could not model"
            )));
        }
        // A bitfield neither starts nor ends on a byte, which is exactly why
        // it cannot be projected: there is no field to take the address of.
        if field.offset_bits() % 8 != 0 || field.size_bits() % 8 != 0 {
            return Err(reject(format!(
                "'{name}' has bitfield '{field_name}', which has no addressable storage"
            )));
        }
        let type_id = project_type(
            symbol,
            field.ty(),
            types,
            shapes,
            &fol_abi::PointerContract::default(),
        )?;
        // A record crosses as a parameter by being flattened into its fields,
        // which the adapter rebuilds the provider's struct from. That
        // flattening is one level deep: a field that is itself a record has no
        // scalar to pass, and the adapter fails on it much later with a
        // message that says only "a record type" -- which reads as though no
        // record crossed at all.
        //
        // An anonymous member arrives here as exactly this shape, and worse:
        // C makes its fields part of the outer struct, so `o.x` is valid
        // there, while the projection puts them behind a GERC-generated field
        // name that is a content hash. FOL's view of the struct would not be
        // C's, and the name is not one a program could write.
        if matches!(types.get(type_id), Some(AbiType::Record { .. })) {
            let anonymous = field.source_name().is_none() || field_name.starts_with("__gerc_");
            return Err(reject(if anonymous {
                format!(
                    "'{name}' has an anonymous struct member. In C its fields belong to '{name}' itself, and the projection cannot reproduce that: give the member a name, or lift it into a named struct"
                )
            } else {
                format!(
                    "'{name}' has field '{field_name}', which is itself a record. A record crosses as a parameter by being flattened into its fields, and that flattening does not nest"
                )
            }));
        }
        fields.push(fol_abi::AbiField {
            name: field_name,
            type_id,
        });
    }
    Ok(fields)
}

/// Intern one measured C type, or say why it does not cross.
fn project_type(
    symbol: &str,
    ty: &RustType,
    types: &mut AbiTypeTable,
    shapes: &Shapes<'_>,
    contract: &fol_abi::PointerContract,
) -> Result<AbiTypeId, ImportRejection> {
    // Resolved first: a typedef's own qualifiers and support status are the
    // spelling's, and what matters is the type underneath.
    let ty = resolve_alias(ty, &shapes.aliases);
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
            let target_id = project_type(symbol, target, types, shapes, contract)?;
            AbiType::Pointer {
                target: target_id,
                mutability: if target.qualifiers().is_const {
                    AbiMutability::Const
                } else {
                    AbiMutability::Mutable
                },
                // C says none of this, so the overlay does. Undeclared means
                // the conservative reading: a pointer that must not be null,
                // is not owned, and does not outlive the call.
                nullability: if contract.nullable {
                    AbiNullability::Nullable
                } else {
                    AbiNullability::NonNull
                },
                ownership: if contract.transferred {
                    AbiOwnership::Transferred
                } else {
                    AbiOwnership::Borrowed
                },
                escape: if contract.retained {
                    AbiEscape::Retained
                } else {
                    AbiEscape::CallScoped
                },
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
        RustTypeKind::Named {
            declaration,
            rust_name,
        } => return project_named(symbol, *declaration, rust_name, types, shapes),
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

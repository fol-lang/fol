//! The C -> FOL direction.
//!
//! Section 4.14 makes the two directions deliberately asymmetric. An export
//! always uses FOL's own uniform `fol_status_t` contract, so its shape is
//! decided here. An import preserves whatever signature the provider actually
//! has, so its shape is measured -- and the only thing FOL gets to decide is
//! whether it accepts the result.
//!
//! That is why an import is a separate type rather than a `ForeignRoutine`
//! with a flag. Every rule below is about a signature FOL did not choose.

use crate::annotation::{ImportEffects, ImportErrorConvention, RoutineAnnotation};
use crate::interface::{AbiCallingConvention, AbiParameter, AbiSourceOrigin};
use crate::types::{AbiScalar, AbiTypeId, AbiTypeTable};

/// One C declaration FOL may call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedRoutine {
    /// The exact provider symbol. Never mangled, never inferred.
    pub symbol: String,
    /// The name FOL reaches it under, inside the import's namespace.
    pub fol_name: String,
    pub convention: AbiCallingConvention,
    pub parameters: Vec<AbiParameter>,
    /// The C return type, or `Void`.
    pub result: AbiTypeId,
    pub error: ImportErrorConvention,
    pub effects: ImportEffects,
    /// The handle domain this routine produces, borrows, or consumes.
    ///
    /// Carried on the routine rather than inferred from the types, because a
    /// C pointer to an incomplete type says nothing about ownership: the same
    /// `sqlite3 *` is produced by one call, lent to many, and released by one.
    pub handle: Option<crate::annotation::HandleUse>,
    /// The synchronous callback this routine invokes, when it takes one.
    pub callback: Option<crate::annotation::CallbackUse>,
    /// The pointer/length pair this routine takes as one buffer.
    ///
    /// Carried on the routine for the same reason the handle is: C's two
    /// parameters say nothing about belonging together, and no measurement of
    /// the types can recover the pairing.
    pub buffer: Option<crate::annotation::BufferUse>,
    /// Parameters the overlay declared NUL-terminated C strings.
    pub strings: std::collections::BTreeSet<String>,
    /// The owned buffer domain this routine produces or consumes.
    ///
    /// A producer's result is memory FOL must not free itself, so the adapter
    /// validates it, copies it, and calls the domain's release before
    /// returning. A consumer exists only to be that release.
    pub owned_buffer: Option<crate::annotation::OwnedBufferUse>,
    /// The symbol that releases this routine's owned buffer.
    ///
    /// Resolved from the domain at bind time so the adapter never re-searches
    /// an overlay it no longer has.
    pub owned_destroy: Option<String>,
    /// The header location, for diagnostics and navigation.
    pub origin: AbiSourceOrigin,
}

impl ImportedRoutine {
    /// The parameter carrying the success value under a status mapping.
    ///
    /// Returned as an index because lowering needs the position, and resolving
    /// it once here means the backend never re-searches by name.
    pub fn out_parameter_index(&self) -> Option<usize> {
        let ImportErrorConvention::Status { out_parameter, .. } = &self.error else {
            return None;
        };
        self.parameters
            .iter()
            .position(|parameter| &parameter.name == out_parameter)
    }

    /// The parameter carrying a callback's opaque context, by index.
    pub fn callback_context_index(&self) -> Option<usize> {
        let context = self.callback.as_ref()?.context.as_ref()?;
        self.parameters
            .iter()
            .position(|parameter| &parameter.name == context)
    }

    /// The parameter carrying a paired buffer's length, by index.
    pub fn buffer_length_index(&self) -> Option<usize> {
        let use_ = self.buffer.as_ref()?;
        self.parameters
            .iter()
            .position(|parameter| parameter.name == use_.length)
    }

    /// The out-parameter an owned buffer reports its length through.
    pub fn owned_length_index(&self) -> Option<usize> {
        let use_ = self.owned_buffer.as_ref()?;
        self.parameters
            .iter()
            .position(|parameter| parameter.name == use_.length)
    }

    /// The out-parameter an owned buffer reports its capacity through.
    pub fn owned_capacity_index(&self) -> Option<usize> {
        let capacity = self.owned_buffer.as_ref()?.capacity.as_ref()?;
        self.parameters
            .iter()
            .position(|parameter| &parameter.name == capacity)
    }

    /// Whether FOL mounts this routine under a name a program can call.
    ///
    /// A buffer domain's release is not one. FOL never owns the provider's
    /// memory -- the adapter copies out of it and releases it before
    /// returning -- so a FOL program has nothing to release and no address to
    /// release it with.
    pub fn is_mountable(&self) -> bool {
        !self
            .owned_buffer
            .as_ref()
            .is_some_and(|use_| use_.role == crate::annotation::BufferRole::Consumes)
    }

    /// Parameters a FOL caller passes, in order.
    ///
    /// Two positions are hidden, for the same reason in both cases: FOL owns
    /// the storage, so a caller passing it would be answering its own question.
    /// Under a status mapping that is the out-parameter; with a callback it is
    /// the context, which FOL fills with a pointer to the closure it is about
    /// to lend. A paired buffer hides its length for the same reason: the FOL
    /// value already carries its own extent, and a caller passing a second
    /// number could contradict it.
    pub fn call_parameters(&self) -> Vec<&AbiParameter> {
        let out = self.out_parameter_index();
        let context = self.callback_context_index();
        let length = self.buffer_length_index();
        // An owned buffer's length and capacity are reported *by* the
        // provider, so a FOL caller has nothing to pass: the adapter supplies
        // the storage and reads them back.
        let reported = self.owned_length_index();
        let capacity = self.owned_capacity_index();
        self.parameters
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                Some(*index) != out
                    && Some(*index) != context
                    && Some(*index) != length
                    && Some(*index) != reported
                    && Some(*index) != capacity
            })
            .map(|(_, parameter)| parameter)
            .collect()
    }
}

/// One import's accepted surface, resolved against a target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedInterface {
    /// The FOL namespace this surface mounts under.
    pub alias: String,
    pub target: fol_types::ResolvedTarget,
    pub types: AbiTypeTable,
    /// Accepted routines, in symbol order.
    pub routines: Vec<ImportedRoutine>,
}

impl ImportedInterface {
    /// Look one up by the name FOL calls it.
    pub fn routine(&self, fol_name: &str) -> Option<&ImportedRoutine> {
        self.routines
            .iter()
            .find(|routine| routine.fol_name == fol_name)
    }

    /// Every handle domain this interface produces, in name order.
    pub fn handle_domains(&self) -> Vec<&str> {
        let mut domains: Vec<&str> = self
            .routines
            .iter()
            .filter_map(|routine| routine.handle.as_ref())
            .map(|use_| use_.domain.as_str())
            .collect();
        domains.sort_unstable();
        domains.dedup();
        domains
    }

    /// Every C record this interface mounts as a FOL type, in name order.
    ///
    /// A record reaches FOL as a nominal type, so the name has to become a
    /// symbol before any signature refers to it -- the ordering handle domains
    /// need, for the same reason.
    pub fn record_shapes(&self) -> Vec<(&str, &[crate::types::AbiField])> {
        let mut shapes: Vec<(&str, &[crate::types::AbiField])> = self
            .types
            .iter()
            .filter_map(|(_, ty)| match ty {
                crate::types::AbiType::Record { name, fields } => {
                    Some((name.as_str(), fields.as_slice()))
                }
                _ => None,
            })
            .collect();
        shapes.sort_by_key(|(name, _)| *name);
        shapes.dedup_by_key(|(name, _)| *name);
        shapes
    }

    /// Every symbol this interface requires the provider to define.
    ///
    /// Sorted, because it is compared against the provider's symbol table and
    /// declaration order would make two identical interfaces differ.
    pub fn required_symbols(&self) -> Vec<&str> {
        let mut symbols: Vec<&str> = self
            .routines
            .iter()
            .map(|routine| routine.symbol.as_str())
            .collect();
        symbols.sort_unstable();
        symbols
    }
}

/// Why an imported declaration cannot become a callable FOL routine.
///
/// Separate from `AbiRejection` because these are facts about a provider and a
/// header, not about a FOL type: "this record contains a `vec`" and "this
/// provider was built for another architecture" are not the same kind of
/// problem and should not share a code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRejection {
    /// A measured integer whose width has no FOL counterpart.
    UnsupportedIntegerWidth { symbol: String, bits: u16 },
    /// A measured float whose width has no FOL counterpart.
    UnsupportedFloatWidth { symbol: String, bits: u16 },
    /// A scalar whose alignment is not its storage width.
    OverAlignedScalar {
        symbol: String,
        storage_bits: u16,
        alignment_bits: u16,
    },
    /// A C shape the initial subset does not admit.
    UnsupportedDeclaration { symbol: String, detail: String },
    /// A variadic function: FOL has no call form for one.
    Variadic { symbol: String },
    /// A calling convention other than C.
    UnsupportedConvention { symbol: String, convention: String },
    /// The overlay names a symbol the header does not declare.
    UnknownDeclaration { symbol: String },
    /// The overlay's status mapping names a parameter the signature lacks.
    UnknownOutParameter { symbol: String, parameter: String },
    /// A status mapping on a routine that returns no status.
    NonIntegerStatus { symbol: String, detail: String },
    /// The out-parameter is not a writable pointer.
    UnwritableOutParameter { symbol: String, parameter: String },
    /// An effect the artifact's capability model does not permit.
    CapabilityTooStrong {
        symbol: String,
        effect: String,
        model: String,
    },
    /// A routine annotated as producing a handle does not return a pointer.
    HandleResultIsNotAPointer {
        symbol: String,
        domain: String,
        found: String,
    },
    /// A routine that borrows or consumes a handle has no single pointer
    /// parameter to identify as the handle.
    AmbiguousHandleParameter {
        symbol: String,
        domain: String,
        found: usize,
    },
    /// The overlay names a callback or context parameter the signature lacks.
    UnknownCallbackParameter {
        symbol: String,
        parameter: String,
        role: &'static str,
    },
    /// The named callback parameter is not a function pointer, or the named
    /// context parameter is not a `void *`.
    CallbackShapeMismatch {
        symbol: String,
        parameter: String,
        expected: &'static str,
    },
    /// A callback whose own first parameter is not the context it is handed.
    ///
    /// The canonical shape is `f(void *context, ...)`. A provider that puts its
    /// context last is not importable in V4 rather than being guessed at.
    CallbackContextNotFirst { symbol: String, parameter: String },
    /// A callback that is variadic, or one whose signature FOL cannot carry.
    UnsupportedCallbackSignature {
        symbol: String,
        parameter: String,
        detail: String,
    },
    /// The overlay names a buffer or length parameter the signature lacks.
    UnknownBufferParameter {
        symbol: String,
        parameter: String,
        role: &'static str,
    },
    /// The named buffer is not a pointer to a sized element, or the named
    /// length is not an unsigned integer.
    BufferShapeMismatch {
        symbol: String,
        parameter: String,
        expected: &'static str,
    },
    /// A declared direction the parameter's own type contradicts.
    ContradictoryDirection {
        symbol: String,
        parameter: String,
        declared: &'static str,
        detail: &'static str,
    },
}

impl ImportRejection {
    /// The registered diagnostic code.
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::CapabilityTooStrong { .. } => "A1008",
            Self::UnknownDeclaration { .. }
            | Self::UnknownOutParameter { .. }
            | Self::NonIntegerStatus { .. }
            | Self::UnwritableOutParameter { .. } => "A1007",
            _ => "A1006",
        }
    }

    /// A short, stable reason code for tests.
    pub const fn reason(&self) -> &'static str {
        match self {
            Self::UnsupportedIntegerWidth { .. } => "unsupported-integer-width",
            Self::UnsupportedFloatWidth { .. } => "unsupported-float-width",
            Self::OverAlignedScalar { .. } => "over-aligned-scalar",
            Self::UnsupportedDeclaration { .. } => "unsupported-declaration",
            Self::Variadic { .. } => "variadic",
            Self::UnsupportedConvention { .. } => "unsupported-convention",
            Self::UnknownDeclaration { .. } => "unknown-declaration",
            Self::UnknownOutParameter { .. } => "unknown-out-parameter",
            Self::NonIntegerStatus { .. } => "non-integer-status",
            Self::UnwritableOutParameter { .. } => "unwritable-out-parameter",
            Self::CapabilityTooStrong { .. } => "capability-too-strong",
            Self::HandleResultIsNotAPointer { .. } => "handle-result-is-not-a-pointer",
            Self::AmbiguousHandleParameter { .. } => "ambiguous-handle-parameter",
            Self::UnknownCallbackParameter { .. } => "unknown-callback-parameter",
            Self::UnknownBufferParameter { .. } => "unknown-buffer-parameter",
            Self::BufferShapeMismatch { .. } => "buffer-shape-mismatch",
            Self::ContradictoryDirection { .. } => "contradictory-direction",
            Self::CallbackShapeMismatch { .. } => "callback-shape-mismatch",
            Self::CallbackContextNotFirst { .. } => "callback-context-not-first",
            Self::UnsupportedCallbackSignature { .. } => "unsupported-callback-signature",
        }
    }

    /// The symbol the rejection is about.
    pub fn symbol(&self) -> &str {
        match self {
            Self::UnsupportedIntegerWidth { symbol, .. }
            | Self::UnsupportedFloatWidth { symbol, .. }
            | Self::OverAlignedScalar { symbol, .. }
            | Self::UnsupportedDeclaration { symbol, .. }
            | Self::Variadic { symbol }
            | Self::UnsupportedConvention { symbol, .. }
            | Self::UnknownDeclaration { symbol }
            | Self::UnknownOutParameter { symbol, .. }
            | Self::NonIntegerStatus { symbol, .. }
            | Self::UnwritableOutParameter { symbol, .. }
            | Self::CapabilityTooStrong { symbol, .. }
            | Self::HandleResultIsNotAPointer { symbol, .. }
            | Self::AmbiguousHandleParameter { symbol, .. }
            | Self::UnknownCallbackParameter { symbol, .. }
            | Self::UnknownBufferParameter { symbol, .. }
            | Self::BufferShapeMismatch { symbol, .. }
            | Self::ContradictoryDirection { symbol, .. }
            | Self::CallbackShapeMismatch { symbol, .. }
            | Self::CallbackContextNotFirst { symbol, .. }
            | Self::UnsupportedCallbackSignature { symbol, .. } => symbol,
        }
    }
}

impl std::fmt::Display for ImportRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedIntegerWidth { symbol, bits } => write!(
                f,
                "'{symbol}' uses a {bits}-bit integer, which has no FOL counterpart; the C \
                 boundary carries 8, 16, 32, and 64-bit integers"
            ),
            Self::UnsupportedFloatWidth { symbol, bits } => write!(
                f,
                "'{symbol}' uses a {bits}-bit float; the C boundary carries 32 and 64-bit floats"
            ),
            Self::OverAlignedScalar {
                symbol,
                storage_bits,
                alignment_bits,
            } => write!(
                f,
                "'{symbol}' has a {storage_bits}-bit scalar aligned to {alignment_bits} bits; a \
                 packed or over-aligned scalar does not have the layout its FOL counterpart does"
            ),
            Self::UnsupportedDeclaration { symbol, detail } => {
                write!(f, "'{symbol}' is not importable: {detail}")
            }
            Self::Variadic { symbol } => write!(
                f,
                "'{symbol}' is variadic, and FOL has no call form that can supply a variadic \
                 argument list with checked types"
            ),
            Self::UnsupportedConvention { symbol, convention } => write!(
                f,
                "'{symbol}' uses calling convention '{convention}'; only the C convention is \
                 imported"
            ),
            Self::UnknownDeclaration { symbol } => write!(
                f,
                "the annotation overlay selects '{symbol}', which the entry headers do not \
                 declare"
            ),
            Self::UnknownOutParameter { symbol, parameter } => write!(
                f,
                "'{symbol}' maps its result to out-parameter '{parameter}', which is not one of \
                 its parameters"
            ),
            Self::NonIntegerStatus { symbol, detail } => write!(
                f,
                "'{symbol}' is annotated with a status mapping but {detail}"
            ),
            Self::UnwritableOutParameter { symbol, parameter } => write!(
                f,
                "'{symbol}' names '{parameter}' as its out-parameter, but that parameter is not \
                 a pointer FOL may write through"
            ),
            Self::CapabilityTooStrong {
                symbol,
                effect,
                model,
            } => write!(
                f,
                "'{symbol}' declares the '{effect}' effect, which a '{model}' artifact does not \
                 permit"
            ),
            Self::HandleResultIsNotAPointer {
                symbol,
                domain,
                found,
            } => write!(
                f,
                "'{symbol}' is annotated as producing handle domain '{domain}', but it returns \
                 {found} rather than a pointer; a handle is the address the destroy takes back"
            ),
            Self::AmbiguousHandleParameter {
                symbol,
                domain,
                found,
            } => write!(
                f,
                "'{symbol}' uses handle domain '{domain}' but has {found} pointer parameters; \
                 exactly one is needed so the handle is not chosen by position"
            ),
            Self::UnknownCallbackParameter {
                symbol,
                parameter,
                role,
            } => write!(
                f,
                "'{symbol}' names '{parameter}' as its callback {role}, which is not one of its \
                 parameters"
            ),
            Self::CallbackShapeMismatch {
                symbol,
                parameter,
                expected,
            } => write!(
                f,
                "'{symbol}' names '{parameter}' as part of its callback, but that parameter is \
                 not {expected}"
            ),
            Self::UnknownBufferParameter {
                symbol,
                parameter,
                role,
            } => write!(
                f,
                "'{symbol}' names '{parameter}' as its buffer {role}, which is not one of its \
                 parameters"
            ),
            Self::BufferShapeMismatch {
                symbol,
                parameter,
                expected,
            } => write!(
                f,
                "'{symbol}' names '{parameter}' as part of its buffer, but that parameter is \
                 not {expected}"
            ),
            Self::ContradictoryDirection {
                symbol,
                parameter,
                declared,
                detail,
            } => write!(
                f,
                "'{symbol}' declares '{parameter}' as '{declared}', but {detail}"
            ),
            Self::CallbackContextNotFirst { symbol, parameter } => write!(
                f,
                "'{symbol}' passes a callback '{parameter}' whose first parameter is not the \
                 context it is handed; V4 imports the canonical shape \
                 `f(void *context, ...)` and refuses a context in any other position rather \
                 than guessing which argument it is"
            ),
            Self::UnsupportedCallbackSignature {
                symbol,
                parameter,
                detail,
            } => write!(
                f,
                "'{symbol}' passes a callback '{parameter}' FOL cannot carry: {detail}"
            ),
        }
    }
}

impl std::error::Error for ImportRejection {}

/// Map a measured C integer onto the FOL width with that layout.
///
/// The measurement comes from the provider's own compiler, so this is a
/// lookup, not a guess: if it does not land on a width FOL has, the
/// declaration does not cross.
pub fn scalar_for_measured_integer(
    symbol: &str,
    signed: bool,
    storage_bits: u16,
    alignment_bits: u16,
) -> Result<AbiScalar, ImportRejection> {
    if storage_bits != alignment_bits {
        return Err(ImportRejection::OverAlignedScalar {
            symbol: symbol.to_string(),
            storage_bits,
            alignment_bits,
        });
    }
    let width = match (signed, storage_bits) {
        (true, 8) => fol_types::IntWidth::I8,
        (true, 16) => fol_types::IntWidth::I16,
        (true, 32) => fol_types::IntWidth::I32,
        (true, 64) => fol_types::IntWidth::I64,
        (false, 8) => fol_types::IntWidth::U8,
        (false, 16) => fol_types::IntWidth::U16,
        (false, 32) => fol_types::IntWidth::U32,
        (false, 64) => fol_types::IntWidth::U64,
        _ => {
            return Err(ImportRejection::UnsupportedIntegerWidth {
                symbol: symbol.to_string(),
                bits: storage_bits,
            })
        }
    };
    Ok(AbiScalar::Int(width))
}

/// Map a measured C float onto its FOL width.
pub fn scalar_for_measured_float(
    symbol: &str,
    storage_bits: u16,
    alignment_bits: u16,
) -> Result<AbiScalar, ImportRejection> {
    if storage_bits != alignment_bits {
        return Err(ImportRejection::OverAlignedScalar {
            symbol: symbol.to_string(),
            storage_bits,
            alignment_bits,
        });
    }
    match storage_bits {
        32 => Ok(AbiScalar::Float(fol_types::FloatWidth::F32)),
        64 => Ok(AbiScalar::Float(fol_types::FloatWidth::F64)),
        bits => Err(ImportRejection::UnsupportedFloatWidth {
            symbol: symbol.to_string(),
            bits,
        }),
    }
}

/// Check one annotated routine against the signature that was measured for it.
///
/// The overlay and the header are written by different people at different
/// times; this is where the two are made to agree before anything is callable.
pub fn verify_status_mapping(
    annotation: &RoutineAnnotation,
    parameters: &[AbiParameter],
    result: AbiTypeId,
    types: &AbiTypeTable,
) -> Result<(), ImportRejection> {
    let ImportErrorConvention::Status { out_parameter, .. } = &annotation.error else {
        return Ok(());
    };
    let symbol = annotation.symbol.clone();

    // The status has to be readable as an integer, or the codes the overlay
    // enumerates cannot be compared against anything.
    match types.get(result) {
        Some(crate::types::AbiType::Scalar(AbiScalar::Int(_))) => {}
        Some(other) => {
            return Err(ImportRejection::NonIntegerStatus {
                symbol,
                detail: format!("it returns {}, not an integer status", other.kind_name()),
            })
        }
        None => {
            return Err(ImportRejection::NonIntegerStatus {
                symbol,
                detail: "its return type was not measured".to_string(),
            })
        }
    }

    let Some(parameter) = parameters
        .iter()
        .find(|parameter| &parameter.name == out_parameter)
    else {
        return Err(ImportRejection::UnknownOutParameter {
            symbol,
            parameter: out_parameter.clone(),
        });
    };

    let writable = matches!(
        types.get(parameter.type_id),
        Some(crate::types::AbiType::Pointer {
            mutability: crate::types::AbiMutability::Mutable,
            ..
        })
    );
    if !writable {
        return Err(ImportRejection::UnwritableOutParameter {
            symbol,
            parameter: out_parameter.clone(),
        });
    }
    Ok(())
}

/// Check an import's effects against the artifact's capability model.
///
/// Section 4.13's STOP is explicit that an unknown effect is not shippable, so
/// this never upgrades a model to fit a declaration.
pub fn verify_effects(
    annotation: &RoutineAnnotation,
    model: CapabilityModel,
) -> Result<(), ImportRejection> {
    let effects = annotation.effects;
    for (present, effect, permitted) in [
        (effects.allocates, "allocates", model.permits_allocation()),
        (effects.hosted, "hosted", model.permits_hosted()),
    ] {
        if present && !permitted {
            return Err(ImportRejection::CapabilityTooStrong {
                symbol: annotation.symbol.clone(),
                effect: effect.to_string(),
                model: model.as_str().to_string(),
            });
        }
    }
    Ok(())
}

/// The artifact capability model an import is checked against.
///
/// Mirrors the tiers the rest of the compiler already uses; it is repeated
/// here as a small enum so `fol-abi` keeps its single dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityModel {
    Core,
    Memo,
    Std,
}

impl CapabilityModel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Memo => "memo",
            Self::Std => "std",
        }
    }

    /// `core` has no allocator, so an allocating import is not reachable from
    /// it whatever the provider does.
    pub const fn permits_allocation(self) -> bool {
        matches!(self, Self::Memo | Self::Std)
    }

    /// Only `std` is hosted.
    pub const fn permits_hosted(self) -> bool {
        matches!(self, Self::Std)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotation::AnnotationOverlay;
    use crate::interface::AbiDirection;
    use crate::types::{AbiEscape, AbiMutability, AbiNullability, AbiOwnership, AbiType};

    fn overlay(text: &str) -> AnnotationOverlay {
        AnnotationOverlay::parse(text).expect("test overlay should parse")
    }

    #[test]
    fn measured_c_integers_map_onto_the_fol_width_with_the_same_layout() {
        for (signed, bits, expected) in [
            (true, 8, fol_types::IntWidth::I8),
            (true, 16, fol_types::IntWidth::I16),
            (true, 32, fol_types::IntWidth::I32),
            (true, 64, fol_types::IntWidth::I64),
            (false, 8, fol_types::IntWidth::U8),
            (false, 32, fol_types::IntWidth::U32),
            (false, 64, fol_types::IntWidth::U64),
        ] {
            assert_eq!(
                scalar_for_measured_integer("f", signed, bits, bits),
                Ok(AbiScalar::Int(expected))
            );
        }
    }

    #[test]
    fn an_integer_width_fol_does_not_have_is_refused() {
        assert_eq!(
            scalar_for_measured_integer("f", true, 128, 128),
            Err(ImportRejection::UnsupportedIntegerWidth {
                symbol: "f".to_string(),
                bits: 128,
            })
        );
        // A 24-bit measurement is not a rounding opportunity.
        assert_eq!(
            scalar_for_measured_integer("f", false, 24, 24),
            Err(ImportRejection::UnsupportedIntegerWidth {
                symbol: "f".to_string(),
                bits: 24,
            })
        );
    }

    #[test]
    fn a_packed_or_over_aligned_scalar_is_refused_before_its_width_is_considered() {
        assert_eq!(
            scalar_for_measured_integer("f", true, 32, 8),
            Err(ImportRejection::OverAlignedScalar {
                symbol: "f".to_string(),
                storage_bits: 32,
                alignment_bits: 8,
            })
        );
        assert_eq!(
            scalar_for_measured_float("f", 64, 128),
            Err(ImportRejection::OverAlignedScalar {
                symbol: "f".to_string(),
                storage_bits: 64,
                alignment_bits: 128,
            })
        );
    }

    #[test]
    fn measured_c_floats_map_onto_their_fol_widths() {
        assert_eq!(
            scalar_for_measured_float("f", 32, 32),
            Ok(AbiScalar::Float(fol_types::FloatWidth::F32))
        );
        assert_eq!(
            scalar_for_measured_float("f", 64, 64),
            Ok(AbiScalar::Float(fol_types::FloatWidth::F64))
        );
        // `long double` is measured at 80 or 128 bits and has no FOL width.
        assert_eq!(
            scalar_for_measured_float("f", 128, 128),
            Err(ImportRejection::UnsupportedFloatWidth {
                symbol: "f".to_string(),
                bits: 128,
            })
        );
    }

    fn status_fixture() -> (AbiTypeTable, Vec<AbiParameter>, AbiTypeId) {
        let mut types = AbiTypeTable::new();
        let i32_id = types.intern_int(fol_types::IntWidth::I32);
        let out_id = types.intern(AbiType::Pointer {
            target: i32_id,
            mutability: AbiMutability::Mutable,
            nullability: AbiNullability::NonNull,
            ownership: AbiOwnership::Borrowed,
            escape: AbiEscape::CallScoped,
            destructor: None,
        });
        let parameters = vec![
            AbiParameter {
                name: "lhs".to_string(),
                type_id: i32_id,
                direction: AbiDirection::In,
            },
            AbiParameter {
                name: "result".to_string(),
                type_id: out_id,
                direction: AbiDirection::Out,
            },
        ];
        (types, parameters, i32_id)
    }

    const STATUS_OVERLAY: &str = "version = 1\n[routine.div]\nerror = \"status\"\n\
                                  status_ok = [0]\nstatus_error = [1]\nout = \"result\"\n";

    #[test]
    fn a_complete_status_mapping_matches_its_measured_signature() {
        let (types, parameters, result) = status_fixture();
        let overlay = overlay(STATUS_OVERLAY);

        assert_eq!(
            verify_status_mapping(
                overlay.routine("div").expect("selected"),
                &parameters,
                result,
                &types
            ),
            Ok(())
        );
    }

    #[test]
    fn a_status_mapping_naming_a_parameter_the_signature_lacks_is_refused() {
        let (types, parameters, result) = status_fixture();
        let text = STATUS_OVERLAY.replace("\"result\"", "\"answer\"");
        let overlay = overlay(&text);

        assert_eq!(
            verify_status_mapping(
                overlay.routine("div").expect("selected"),
                &parameters,
                result,
                &types
            ),
            Err(ImportRejection::UnknownOutParameter {
                symbol: "div".to_string(),
                parameter: "answer".to_string(),
            })
        );
    }

    #[test]
    fn a_status_mapping_on_a_routine_that_returns_no_integer_is_refused() {
        let (mut types, parameters, _) = status_fixture();
        let void = types.intern(AbiType::Void);
        let overlay = overlay(STATUS_OVERLAY);

        let error = verify_status_mapping(
            overlay.routine("div").expect("selected"),
            &parameters,
            void,
            &types,
        )
        .expect_err("a void return carries no status");
        assert!(matches!(error, ImportRejection::NonIntegerStatus { .. }));
    }

    #[test]
    fn an_out_parameter_that_is_not_writable_is_refused() {
        let mut types = AbiTypeTable::new();
        let i32_id = types.intern_int(fol_types::IntWidth::I32);
        let const_ptr = types.intern(AbiType::Pointer {
            target: i32_id,
            mutability: AbiMutability::Const,
            nullability: AbiNullability::NonNull,
            ownership: AbiOwnership::Borrowed,
            escape: AbiEscape::CallScoped,
            destructor: None,
        });
        let parameters = vec![AbiParameter {
            name: "result".to_string(),
            type_id: const_ptr,
            direction: AbiDirection::Out,
        }];
        let overlay = overlay(STATUS_OVERLAY);

        assert_eq!(
            verify_status_mapping(
                overlay.routine("div").expect("selected"),
                &parameters,
                i32_id,
                &types
            ),
            Err(ImportRejection::UnwritableOutParameter {
                symbol: "div".to_string(),
                parameter: "result".to_string(),
            })
        );
    }

    #[test]
    fn an_infallible_routine_needs_no_status_check() {
        let (types, parameters, result) = status_fixture();
        let overlay = overlay("version = 1\n[routine.div]\nerror = \"infallible\"\n");

        assert_eq!(
            verify_status_mapping(
                overlay.routine("div").expect("selected"),
                &parameters,
                result,
                &types
            ),
            Ok(())
        );
    }

    #[test]
    fn a_core_artifact_cannot_reach_an_allocating_import() {
        let overlay = overlay(
            "version = 1\n[routine.f]\nerror = \"infallible\"\neffects = [\"allocates\"]\n",
        );
        let routine = overlay.routine("f").expect("selected");

        assert_eq!(
            verify_effects(routine, CapabilityModel::Core),
            Err(ImportRejection::CapabilityTooStrong {
                symbol: "f".to_string(),
                effect: "allocates".to_string(),
                model: "core".to_string(),
            })
        );
        assert_eq!(verify_effects(routine, CapabilityModel::Memo), Ok(()));
        assert_eq!(verify_effects(routine, CapabilityModel::Std), Ok(()));
    }

    #[test]
    fn only_std_reaches_a_hosted_import() {
        let overlay =
            overlay("version = 1\n[routine.f]\nerror = \"infallible\"\neffects = [\"hosted\"]\n");
        let routine = overlay.routine("f").expect("selected");

        for model in [CapabilityModel::Core, CapabilityModel::Memo] {
            assert_eq!(
                verify_effects(routine, model),
                Err(ImportRejection::CapabilityTooStrong {
                    symbol: "f".to_string(),
                    effect: "hosted".to_string(),
                    model: model.as_str().to_string(),
                })
            );
        }
        assert_eq!(verify_effects(routine, CapabilityModel::Std), Ok(()));
    }

    #[test]
    fn a_declared_core_safe_scalar_call_is_accepted_by_every_model() {
        let overlay = overlay("version = 1\n[routine.f]\nerror = \"infallible\"\n");
        let routine = overlay.routine("f").expect("selected");

        for model in [
            CapabilityModel::Core,
            CapabilityModel::Memo,
            CapabilityModel::Std,
        ] {
            assert_eq!(verify_effects(routine, model), Ok(()));
        }
    }

    #[test]
    fn a_status_call_hides_its_out_parameter_from_fol_callers() {
        let (types, parameters, result) = status_fixture();
        let overlay = overlay(STATUS_OVERLAY);
        let annotation = overlay.routine("div").expect("selected");
        verify_status_mapping(annotation, &parameters, result, &types).expect("mapping is valid");

        let routine = ImportedRoutine {
            symbol: "div".to_string(),
            fol_name: "div".to_string(),
            convention: AbiCallingConvention::C,
            parameters,
            result,
            error: annotation.error.clone(),
            effects: annotation.effects,
            handle: None,
            callback: None,
            buffer: None,
            strings: Default::default(),
            owned_buffer: None,
            owned_destroy: None,
            origin: AbiSourceOrigin::default(),
        };

        assert_eq!(routine.out_parameter_index(), Some(1));
        let names: Vec<&str> = routine
            .call_parameters()
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        assert_eq!(names, vec!["lhs"]);
    }

    #[test]
    fn an_infallible_call_passes_every_parameter_through() {
        let (types, parameters, result) = status_fixture();
        let _ = types;
        let routine = ImportedRoutine {
            symbol: "f".to_string(),
            fol_name: "f".to_string(),
            convention: AbiCallingConvention::C,
            parameters,
            result,
            error: ImportErrorConvention::Infallible,
            effects: ImportEffects::default(),
            handle: None,
            callback: None,
            buffer: None,
            strings: Default::default(),
            owned_buffer: None,
            owned_destroy: None,
            origin: AbiSourceOrigin::default(),
        };

        assert_eq!(routine.out_parameter_index(), None);
        assert_eq!(routine.call_parameters().len(), 2);
    }

    #[test]
    fn required_symbols_are_sorted_so_two_identical_interfaces_agree() {
        let interface = ImportedInterface {
            alias: "c_math".to_string(),
            target: fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu")
                .expect("certified target"),
            types: AbiTypeTable::new(),
            routines: ["zeta", "alpha", "mid"]
                .into_iter()
                .map(|name| ImportedRoutine {
                    symbol: name.to_string(),
                    fol_name: name.to_string(),
                    convention: AbiCallingConvention::C,
                    parameters: Vec::new(),
                    result: AbiTypeId(0),
                    error: ImportErrorConvention::Infallible,
                    effects: ImportEffects::default(),
                    handle: None,
                    callback: None,
                    buffer: None,
                    strings: Default::default(),
                    owned_buffer: None,
                    owned_destroy: None,
                    origin: AbiSourceOrigin::default(),
                })
                .collect(),
        };

        assert_eq!(interface.required_symbols(), vec!["alpha", "mid", "zeta"]);
        assert_eq!(
            interface
                .routine("mid")
                .map(|routine| routine.symbol.as_str()),
            Some("mid")
        );
        assert!(interface.routine("absent").is_none());
    }
}

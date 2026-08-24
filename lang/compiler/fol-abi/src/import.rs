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

    /// Parameters a FOL caller passes, in order.
    ///
    /// Under a status mapping the out-parameter is not one of them: FOL
    /// supplies the storage and reads it back as the result, so a caller
    /// passing it would be writing the answer to its own question.
    pub fn call_parameters(&self) -> Vec<&AbiParameter> {
        let out = self.out_parameter_index();
        self.parameters
            .iter()
            .enumerate()
            .filter(|(index, _)| Some(*index) != out)
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
            | Self::CapabilityTooStrong { symbol, .. } => symbol,
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

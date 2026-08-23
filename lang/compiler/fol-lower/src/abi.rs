//! Projecting lowered FOL into the canonical ABI model.
//!
//! This is where a routine the build program named in its export allowlist
//! becomes a `ForeignRoutine` with real types. Anything that cannot be
//! projected is reported here, with the path to the offending piece, so the
//! backend never has to decide whether a shape is legal.

use crate::ids::LoweredTypeId;
use crate::types::{LoweredBuiltinType, LoweredType, LoweredTypeTable};
use fol_abi::{
    AbiClassification, AbiDirection, AbiEffects, AbiErrorContract, AbiFacing, AbiParameter,
    AbiRejection, AbiScalar, AbiSourceOrigin, AbiType, AbiTypeId, AbiTypeTable, CandidateType,
    ExportSelection, ForeignInterfaceTemplate, ForeignRoutine,
};

/// One entry of the export allowlist, as the build program wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiExportRequest {
    pub routine: String,
    pub symbol: String,
}

/// What the projection produced, or why it could not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiProjection {
    pub template: ForeignInterfaceTemplate,
    pub rejections: Vec<AbiClassification>,
}

impl AbiProjection {
    pub fn is_clean(&self) -> bool {
        self.rejections.is_empty()
    }
}

/// Describe a lowered type for the classifier.
///
/// The classifier lives in `fol-abi`, which cannot see `LoweredType`, so the
/// shape is translated here and the legality decision stays there.
pub fn describe(table: &LoweredTypeTable, id: LoweredTypeId) -> CandidateType {
    match table.get(id) {
        Some(LoweredType::Builtin(builtin)) => match builtin {
            LoweredBuiltinType::Int(width) => CandidateType::Int {
                spelling: width.as_str().to_string(),
                bits: width.bits(),
            },
            LoweredBuiltinType::Float(width) => CandidateType::Float {
                spelling: width.as_str().to_string(),
            },
            LoweredBuiltinType::Bool => CandidateType::Bool,
            LoweredBuiltinType::Char(encoding) => CandidateType::Char {
                encoding: encoding.as_str().to_string(),
            },
            LoweredBuiltinType::Str => CandidateType::Container {
                spelling: "str".to_string(),
            },
            LoweredBuiltinType::Never => CandidateType::Void,
        },
        Some(LoweredType::Pointer {
            raw: true,
            target,
            mutable,
            ..
        }) => {
            CandidateType::RawPointer {
                target: Box::new(describe(table, *target)),
                mutability: Some(*mutable),
                // A bare `ptr[raw, T]` is non-null; `opt` wrapping is the
                // nullable form, and an `opt` never reaches here because the
                // optional wrapper is described as a container.
                nullability: Some(false),
                // Ownership and escape are declaration facts an M6 annotation
                // supplies. Until then a raw pointer in an export is refused
                // with the missing contract named, rather than defaulted.
                ownership: None,
                escape: None,
                destructor: None,
            }
        }
        Some(LoweredType::Pointer { .. }) => CandidateType::ManagedPointer {
            spelling: table.render_type(id),
        },
        Some(LoweredType::Record { .. }) | Some(LoweredType::Entry { .. }) => {
            // A structural aggregate has no declared name to give the C type.
            CandidateType::Record {
                name: None,
                fields: Vec::new(),
            }
        }
        Some(LoweredType::Named { name, .. }) => CandidateType::Record {
            name: Some(name.clone()),
            fields: Vec::new(),
        },
        Some(LoweredType::Routine(_)) => CandidateType::RoutineObject {
            spelling: table.render_type(id),
        },
        Some(_) => CandidateType::Container {
            spelling: table.render_type(id),
        },
        None => CandidateType::Container {
            spelling: "<unknown>".to_string(),
        },
    }
}

/// Intern a lowered type into the ABI table, assuming it already verified.
fn intern(
    abi: &mut AbiTypeTable,
    table: &LoweredTypeTable,
    id: LoweredTypeId,
) -> Option<AbiTypeId> {
    let scalar = match table.get(id)? {
        LoweredType::Builtin(LoweredBuiltinType::Int(width)) => AbiScalar::Int(*width),
        LoweredType::Builtin(LoweredBuiltinType::Float(width)) => AbiScalar::Float(*width),
        LoweredType::Builtin(LoweredBuiltinType::Bool) => AbiScalar::Bool,
        LoweredType::Builtin(LoweredBuiltinType::Char(_)) => AbiScalar::Char,
        LoweredType::Builtin(LoweredBuiltinType::Never) => return Some(abi.intern(AbiType::Void)),
        _ => return None,
    };
    Some(abi.intern(AbiType::Scalar(scalar)))
}

/// A routine's parameters, result, and error type.
pub type LoweredSignature = (
    Vec<(String, LoweredTypeId)>,
    Option<LoweredTypeId>,
    Option<LoweredTypeId>,
);

/// Project a set of allowlisted routines into a foreign interface template.
///
/// `resolve` maps a fully qualified FOL path to its lowered signature. The
/// caller supplies it because how a path resolves is a workspace question, and
/// this function's job is the ABI decision.
pub fn project_exports(
    table: &LoweredTypeTable,
    requests: &[AbiExportRequest],
    mut resolve: impl FnMut(
        &str,
    ) -> Option<(
        Vec<(String, LoweredTypeId)>,
        Option<LoweredTypeId>,
        Option<LoweredTypeId>,
    )>,
) -> AbiProjection {
    let mut template = ForeignInterfaceTemplate::new();
    let mut rejections = Vec::new();

    for request in requests {
        if let Some(rejection) = fol_abi::classify_external_symbol(&request.symbol) {
            rejections.push(AbiClassification::new(
                vec![request.routine.clone()],
                rejection,
            ));
            continue;
        }

        let Some((params, result, error)) = resolve(&request.routine) else {
            rejections.push(AbiClassification::new(
                vec![request.routine.clone()],
                AbiRejection::InvalidExternalSymbol {
                    symbol: request.symbol.clone(),
                    reason: format!(
                        "no routine named '{}' is visible in this artifact",
                        request.routine
                    ),
                },
            ));
            continue;
        };

        // Verify every type before interning any of them, so a rejected export
        // leaves no partial entry in the table.
        let mut clean = true;
        for (name, type_id) in &params {
            let found = fol_abi::verify_type(
                &format!("{}.{name}", request.routine),
                &describe(table, *type_id),
            );
            clean &= found.is_empty();
            rejections.extend(found);
        }
        if let Some(result) = result {
            let found = fol_abi::verify_type(
                &format!("{}.<result>", request.routine),
                &describe(table, result),
            );
            clean &= found.is_empty();
            rejections.extend(found);
        }
        if let Some(error) = error {
            let found = fol_abi::verify_type(
                &format!("{}.<error>", request.routine),
                &describe(table, error),
            );
            clean &= found.is_empty();
            rejections.extend(found);
        }
        if !clean {
            continue;
        }

        let mut parameters = Vec::new();
        for (name, type_id) in &params {
            let Some(abi_id) = intern(&mut template.types, table, *type_id) else {
                continue;
            };
            parameters.push(AbiParameter {
                name: name.clone(),
                type_id: abi_id,
                direction: AbiDirection::In,
            });
        }
        let result_id = result
            .and_then(|id| intern(&mut template.types, table, id))
            .unwrap_or_else(|| template.types.intern(AbiType::Void));
        let error_contract = match error.and_then(|id| intern(&mut template.types, table, id)) {
            Some(error_type) => AbiErrorContract::Recoverable { error_type },
            None => AbiErrorContract::Infallible,
        };

        template.push_routine(ForeignRoutine {
            fol_path: request.routine.clone(),
            symbol: request.symbol.clone(),
            facing: AbiFacing::Export,
            convention: Default::default(),
            parameters,
            result: result_id,
            error: error_contract,
            selection: ExportSelection {
                package_visible: true,
                abi_selected: true,
            },
            effects: AbiEffects {
                allocates: false,
                may_panic: true,
                reports_error: error.is_some(),
            },
            origin: AbiSourceOrigin::default(),
        });
    }

    // Duplicates are checked across the whole set rather than per entry.
    let symbols: Vec<String> = requests.iter().map(|r| r.symbol.clone()).collect();
    rejections.extend(fol_abi::classify_duplicate_symbols(&symbols));

    AbiProjection {
        template,
        rejections,
    }
}

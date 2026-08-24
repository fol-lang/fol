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
    /// The handle domain this routine produces, borrows, or consumes.
    pub handle: Option<AbiExportHandle>,
}

/// How one exported routine relates to a handle domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbiExportHandle {
    pub domain: String,
    pub role: AbiExportHandleRole,
    /// On the producing routine, the symbol that releases what it made.
    pub destroy: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiExportHandleRole {
    Produces,
    Borrows,
    Consumes,
}

impl AbiExportHandleRole {
    pub fn from_keyword(word: &str) -> Option<Self> {
        match word {
            "produces" => Some(Self::Produces),
            "borrows" => Some(Self::Borrows),
            "consumes" => Some(Self::Consumes),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Produces => "produces",
            Self::Borrows => "borrows",
            Self::Consumes => "consumes",
        }
    }
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

/// One named aggregate declaration, in source order.
///
/// The ordered member list is the *declaration's*, not the interned type's:
/// `LoweredType::Record` and `LoweredType::Entry` hold `BTreeMap`s because that
/// is their structural identity, and two records whose fields differ only in
/// order are the same interned type but different C structs. So order can only
/// come from the declaration, which `fol-lower/src/decls/type_decls.rs` already
/// builds in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiAggregateDecl {
    Record {
        name: String,
        fields: Vec<(String, LoweredTypeId)>,
    },
    Entry {
        name: String,
        /// Each variant's declared tag and optional payload. The tag is
        /// explicit rather than positional, so inserting a variant cannot
        /// renumber the ones after it.
        variants: Vec<(String, i64, Option<LoweredTypeId>)>,
    },
}

impl AbiAggregateDecl {
    pub fn name(&self) -> &str {
        match self {
            Self::Record { name, .. } | Self::Entry { name, .. } => name,
        }
    }
}

/// Named aggregate declarations, keyed by the type they declare.
pub type AbiRecordMap = std::collections::BTreeMap<LoweredTypeId, AbiAggregateDecl>;

/// The type behind a loan, or the type itself.
fn peel_loan(table: &LoweredTypeTable, id: LoweredTypeId) -> LoweredTypeId {
    match table.get(id) {
        Some(LoweredType::Borrowed { inner, .. }) | Some(LoweredType::Owned { inner }) => {
            peel_loan(table, *inner)
        }
        _ => id,
    }
}

/// Where a handle sits in one exported routine's signature.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ExportHandlePositions {
    /// The result is the handle.
    result: bool,
    /// The index of the parameter carrying the handle.
    parameter: Option<usize>,
}

/// Decide which positions carry the handle, or say why none does.
///
/// Never by position: a producer's handle is its result, and a borrower's or
/// consumer's is the single parameter whose FOL type is the domain's. Anything
/// else is ambiguous, and guessing would hand C an address into the wrong
/// value.
fn handle_positions(
    table: &LoweredTypeTable,
    request: &AbiExportRequest,
    params: &[(String, LoweredTypeId)],
    result: Option<LoweredTypeId>,
    domain_types: &std::collections::BTreeMap<String, LoweredTypeId>,
) -> Result<ExportHandlePositions, AbiClassification> {
    let Some(handle) = &request.handle else {
        return Ok(ExportHandlePositions::default());
    };
    let reject = |reason: String| {
        AbiClassification::new(
            vec![request.routine.clone()],
            AbiRejection::InvalidExternalSymbol {
                symbol: request.symbol.clone(),
                reason,
            },
        )
    };

    if handle.role == AbiExportHandleRole::Produces {
        if result.is_none() {
            return Err(reject(format!(
                "produces handle domain '{}' but returns nothing",
                handle.domain
            )));
        }
        return Ok(ExportHandlePositions {
            result: true,
            parameter: None,
        });
    }

    let Some(domain_type) = domain_types.get(&handle.domain) else {
        return Err(reject(format!(
            "names handle domain '{}', which no exported routine produces",
            handle.domain
        )));
    };
    // A borrower writes `Session[bor]`, so the loan is peeled before the
    // comparison: it is the same domain either way, and the role is what says
    // whether the wrapper lends or takes back.
    let carrying: Vec<usize> = params
        .iter()
        .enumerate()
        .filter(|(_, (_, type_id))| peel_loan(table, *type_id) == *domain_type)
        .map(|(index, _)| index)
        .collect();
    let [index] = carrying[..] else {
        return Err(reject(format!(
            "takes {} parameters of handle domain '{}'; a borrower or consumer takes exactly one",
            carrying.len(),
            handle.domain
        )));
    };
    Ok(ExportHandlePositions {
        result: false,
        parameter: Some(index),
    })
}

/// Every rule a handle domain owes, checked across the whole allowlist.
///
/// The same shape the import overlay enforces, for the same reason: a resource
/// C can only hold as an address needs exactly one routine that makes it and
/// exactly one that releases it, and C itself can state neither.
fn verify_handle_pairing(requests: &[AbiExportRequest]) -> Option<Vec<AbiClassification>> {
    let mut rejections = Vec::new();
    let mut producers: std::collections::BTreeMap<&str, &AbiExportRequest> =
        std::collections::BTreeMap::new();
    let mut consumers: std::collections::BTreeMap<&str, Vec<&AbiExportRequest>> =
        std::collections::BTreeMap::new();

    for request in requests {
        let Some(handle) = &request.handle else {
            continue;
        };
        match handle.role {
            AbiExportHandleRole::Produces => {
                if producers.insert(handle.domain.as_str(), request).is_some() {
                    rejections.push(AbiClassification::new(
                        vec![request.routine.clone()],
                        AbiRejection::InvalidExternalSymbol {
                            symbol: request.symbol.clone(),
                            reason: format!(
                                "handle domain '{}' already has a producing routine; a domain \
                                 has exactly one",
                                handle.domain
                            ),
                        },
                    ));
                }
            }
            AbiExportHandleRole::Consumes => {
                consumers
                    .entry(handle.domain.as_str())
                    .or_default()
                    .push(request);
            }
            AbiExportHandleRole::Borrows => {}
        }
    }

    for (domain, producer) in &producers {
        let Some(destroy) = producer.handle.as_ref().and_then(|h| h.destroy.as_deref()) else {
            rejections.push(AbiClassification::new(
                vec![producer.routine.clone()],
                AbiRejection::IncompletePointerContract {
                    missing: format!(
                        "a destroy symbol for handle domain '{domain}': a consumer that receives \
                         one has no way to release it otherwise"
                    ),
                },
            ));
            continue;
        };
        let released = consumers
            .get(domain)
            .map(|found| found.iter().any(|request| request.symbol == destroy))
            .unwrap_or(false);
        if !released {
            rejections.push(AbiClassification::new(
                vec![producer.routine.clone()],
                AbiRejection::InvalidExternalSymbol {
                    symbol: destroy.to_string(),
                    reason: format!(
                        "is named as the destroy for handle domain '{domain}' but is not an \
                         exported routine consuming that domain"
                    ),
                },
            ));
        }
    }

    // A consumer with no producer would release something this library never
    // made, which is a different provider's resource or none at all.
    for (domain, found) in &consumers {
        if !producers.contains_key(domain) {
            for request in found {
                rejections.push(AbiClassification::new(
                    vec![request.routine.clone()],
                    AbiRejection::InvalidExternalSymbol {
                        symbol: request.symbol.clone(),
                        reason: format!(
                            "consumes handle domain '{domain}', which no exported routine produces"
                        ),
                    },
                ));
            }
        }
    }

    (!rejections.is_empty()).then_some(rejections)
}

/// Describe a lowered type for the classifier.
///
/// The classifier lives in `fol-abi`, which cannot see `LoweredType`, so the
/// shape is translated here and the legality decision stays there.
pub fn describe(
    table: &LoweredTypeTable,
    records: &AbiRecordMap,
    id: LoweredTypeId,
) -> CandidateType {
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
            // A `str` crosses as a borrowed view: a pointer and a length the
            // caller owns. Whether that is legal depends on the position, and
            // the verifier decides -- inbound it is, outbound it is not.
            LoweredBuiltinType::Str => CandidateType::BorrowedString,
            LoweredBuiltinType::Never => CandidateType::Void,
        },
        Some(LoweredType::Pointer {
            raw: true,
            target,
            mutable,
            ..
        }) => {
            CandidateType::RawPointer {
                target: Box::new(describe(table, records, *target)),
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
        // A declared aggregate projects with its source order, which is what
        // decides every offset in the generated struct and every tag value in
        // the generated union.
        Some(LoweredType::Record { .. }) | Some(LoweredType::Entry { .. }) => {
            match records.get(&id) {
                Some(AbiAggregateDecl::Record { name, fields }) => CandidateType::Record {
                    name: Some(name.clone()),
                    fields: fields
                        .iter()
                        .map(|(field, field_type)| {
                            (field.clone(), describe(table, records, *field_type))
                        })
                        .collect(),
                },
                Some(AbiAggregateDecl::Entry { name, variants }) => CandidateType::Entry {
                    name: Some(name.clone()),
                    variants: variants
                        .iter()
                        .map(|(variant, _, payload)| {
                            (
                                variant.clone(),
                                // `None`: FOL has no syntax for an explicit ABI
                                // discriminant, and the tag it uses internally
                                // is positional -- inserting a variant would
                                // renumber every later one, which is a silent
                                // ABI break. The verifier turns this into
                                // `UnstableEntryTag`, so an entry is refused
                                // with the reason rather than shipped with a
                                // tag that cannot be promised. See
                                // `fol-typecheck`'s `explicit_variant_tag`.
                                None,
                                payload.map(|id| describe(table, records, id)),
                            )
                        })
                        .collect(),
                },
                // A structural aggregate has no declared name to give the C type.
                None => CandidateType::Record {
                    name: None,
                    fields: Vec::new(),
                },
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
    records: &AbiRecordMap,
    id: LoweredTypeId,
) -> Option<AbiTypeId> {
    // An aggregate interns its members first, in declaration order, so the ABI
    // table's list is the one the C type is generated from.
    if matches!(
        table.get(id),
        Some(LoweredType::Record { .. }) | Some(LoweredType::Entry { .. })
    ) {
        return match records.get(&id)? {
            AbiAggregateDecl::Record { name, fields } => {
                let mut interned = Vec::with_capacity(fields.len());
                for (field, field_type) in fields {
                    interned.push(fol_abi::AbiField {
                        name: field.clone(),
                        type_id: intern(abi, table, records, *field_type)?,
                    });
                }
                Some(abi.intern(AbiType::Record {
                    name: name.clone(),
                    fields: interned,
                }))
            }
            AbiAggregateDecl::Entry { name, variants } => {
                let mut interned = Vec::with_capacity(variants.len());
                for (variant, tag, payload) in variants {
                    interned.push(fol_abi::AbiVariant {
                        name: variant.clone(),
                        discriminant: *tag,
                        payload: match payload {
                            Some(payload) => Some(intern(abi, table, records, *payload)?),
                            None => None,
                        },
                    });
                }
                Some(abi.intern(AbiType::Entry {
                    name: name.clone(),
                    // A fixed 32-bit tag, matching C's `int` enum convention.
                    // Fixed rather than derived from the values present, so
                    // adding a variant cannot silently widen the tag and
                    // change the struct's layout.
                    tag: fol_types::IntWidth::I32,
                    variants: interned,
                }))
            }
        };
    }

    let scalar = match table.get(id)? {
        LoweredType::Builtin(LoweredBuiltinType::Int(width)) => AbiScalar::Int(*width),
        LoweredType::Builtin(LoweredBuiltinType::Float(width)) => AbiScalar::Float(*width),
        LoweredType::Builtin(LoweredBuiltinType::Bool) => AbiScalar::Bool,
        LoweredType::Builtin(LoweredBuiltinType::Char(_)) => AbiScalar::Char,
        LoweredType::Builtin(LoweredBuiltinType::Never) => return Some(abi.intern(AbiType::Void)),
        LoweredType::Builtin(LoweredBuiltinType::Str) => {
            return Some(abi.intern(AbiType::BorrowedString))
        }
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
    records: &AbiRecordMap,
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

    // Which FOL type each handle domain *is*, taken from the routine that
    // produces it. A borrower and a consumer name the same domain, and their
    // handle parameter is the one carrying that type -- C sees an address
    // either way, so nothing in the signature could say which otherwise.
    let mut domain_types: std::collections::BTreeMap<String, LoweredTypeId> =
        std::collections::BTreeMap::new();
    for request in requests {
        let Some(handle) = &request.handle else {
            continue;
        };
        if handle.role != AbiExportHandleRole::Produces {
            continue;
        }
        if let Some((_, Some(result), _)) = resolve(&request.routine) {
            domain_types.insert(handle.domain.clone(), result);
        }
    }
    if let Some(found) = verify_handle_pairing(requests) {
        rejections.extend(found);
    }

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

        // Which positions carry the handle, decided before any type is
        // verified. A handle's FOL type is an ordinary FOL value that C may not
        // look inside, so verifying it would refuse the one shape the build
        // program just declared legal.
        let handle_positions =
            match handle_positions(table, request, &params, result, &domain_types) {
                Ok(found) => found,
                Err(rejection) => {
                    rejections.push(rejection);
                    continue;
                }
            };

        // Verify every type before interning any of them, so a rejected export
        // leaves no partial entry in the table.
        let mut clean = true;
        for (index, (name, type_id)) in params.iter().enumerate() {
            if handle_positions.parameter == Some(index) {
                continue;
            }
            let found = fol_abi::verify_type_at(
                &format!("{}.{name}", request.routine),
                &describe(table, records, *type_id),
                fol_abi::AbiPosition::Parameter,
            );
            clean &= found.is_empty();
            rejections.extend(found);
        }
        if let Some(result) = result.filter(|_| !handle_positions.result) {
            let found = fol_abi::verify_type_at(
                &format!("{}.<result>", request.routine),
                &describe(table, records, result),
                fol_abi::AbiPosition::Result,
            );
            clean &= found.is_empty();
            rejections.extend(found);
        }
        if let Some(error) = error {
            let found = fol_abi::verify_type_at(
                &format!("{}.<error>", request.routine),
                &describe(table, records, error),
                fol_abi::AbiPosition::Error,
            );
            clean &= found.is_empty();
            rejections.extend(found);
        }
        if !clean {
            continue;
        }

        // `verify` above accepted every type, so `intern` should handle all of
        // them. The two lists are maintained separately, though, and a
        // divergence must surface as a diagnostic: dropping the parameter
        // would emit a wrapper whose signature silently disagrees with the
        // header, and defaulting a result to `void` would discard a value.
        let mut parameters = Vec::new();
        let mut internable = true;
        for (index, (name, type_id)) in params.iter().enumerate() {
            // The handle position is a name, not a shape: C receives an address
            // and may only hand it back, so nothing about the FOL value behind
            // it is described.
            if handle_positions.parameter == Some(index) {
                let domain = request
                    .handle
                    .as_ref()
                    .expect("a handle position implies a declared domain")
                    .domain
                    .clone();
                parameters.push(AbiParameter {
                    name: name.clone(),
                    type_id: template
                        .types
                        .intern(AbiType::OpaqueHandle { name: domain }),
                    direction: AbiDirection::In,
                });
                continue;
            }
            match intern(&mut template.types, table, records, *type_id) {
                Some(abi_id) => parameters.push(AbiParameter {
                    name: name.clone(),
                    type_id: abi_id,
                    direction: AbiDirection::In,
                }),
                None => {
                    internable = false;
                    rejections.push(AbiClassification::new(
                        vec![request.routine.clone()],
                        AbiRejection::UnsupportedLayout {
                            detail: format!(
                                "parameter '{name}' passed verification but has no ABI \
                                 projection; this is a compiler inconsistency"
                            ),
                        },
                    ));
                }
            }
        }
        let result_id = if handle_positions.result {
            let domain = request
                .handle
                .as_ref()
                .expect("a handle result implies a declared domain")
                .domain
                .clone();
            template
                .types
                .intern(AbiType::OpaqueHandle { name: domain })
        } else {
            match result {
                Some(id) => match intern(&mut template.types, table, records, id) {
                    Some(abi_id) => abi_id,
                    None => {
                        internable = false;
                        rejections.push(AbiClassification::new(
                            vec![request.routine.clone()],
                            AbiRejection::UnsupportedLayout {
                                detail:
                                    "the result passed verification but has no ABI projection; \
                                     this is a compiler inconsistency"
                                        .to_string(),
                            },
                        ));
                        template.types.intern(AbiType::Void)
                    }
                },
                None => template.types.intern(AbiType::Void),
            }
        };
        if !internable {
            continue;
        }
        let error_contract =
            match error.and_then(|id| intern(&mut template.types, table, records, id)) {
                Some(error_type) => AbiErrorContract::Recoverable { error_type },
                None => AbiErrorContract::Infallible,
            };

        template.push_routine(ForeignRoutine {
            handle: request.handle.as_ref().map(|handle| fol_abi::HandleUse {
                domain: handle.domain.clone(),
                role: match handle.role {
                    AbiExportHandleRole::Produces => fol_abi::HandleRole::Produces,
                    AbiExportHandleRole::Borrows => fol_abi::HandleRole::Borrows,
                    AbiExportHandleRole::Consumes => fol_abi::HandleRole::Consumes,
                },
            }),
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

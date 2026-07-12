use crate::{
    LoweredConformance, LoweredPackage, LoweredStandard, LoweredStandardRoutine, LoweringError,
    LoweringErrorKind, LoweringResult,
};

pub fn lower_standard_declarations(
    typed_package: &fol_typecheck::TypedPackage,
    lowered_package: &mut LoweredPackage,
) -> LoweringResult<()> {
    let mut errors = Vec::new();

    for standard in typed_package.program.all_typed_standards() {
        let Some(symbol) = typed_package.program.resolved().symbol(standard.symbol_id) else {
            errors.push(LoweringError::with_kind(
                LoweringErrorKind::InvalidInput,
                format!(
                    "typed standard {} disappeared from resolver output",
                    standard.symbol_id.0
                ),
            ));
            continue;
        };

        let mut lowered_required_routines = Vec::new();
        let mut standard_failed = false;
        for requirement in &standard.required_routines {
            let params = match requirement
                .params
                .iter()
                .map(|type_id| {
                    lowered_package
                        .checked_type_map
                        .get(type_id)
                        .copied()
                        .ok_or_else(|| {
                            LoweringError::with_kind(
                                LoweringErrorKind::InvalidInput,
                                format!(
                                    "standard '{}' requirement '{}' lost parameter type {} during lowering",
                                    symbol.name, requirement.name, type_id.0
                                ),
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(params) => params,
                Err(error) => {
                    errors.push(error);
                    standard_failed = true;
                    break;
                }
            };
            let return_type = match requirement.return_type {
                Some(type_id) => match lowered_package.checked_type_map.get(&type_id).copied() {
                    Some(type_id) => Some(type_id),
                    None => {
                        errors.push(LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!(
                                "standard '{}' requirement '{}' lost return type {} during lowering",
                                symbol.name, requirement.name, type_id.0
                            ),
                        ));
                        standard_failed = true;
                        break;
                    }
                },
                None => None,
            };
            let error_type = match requirement.error_type {
                Some(type_id) => match lowered_package.checked_type_map.get(&type_id).copied() {
                    Some(type_id) => Some(type_id),
                    None => {
                        errors.push(LoweringError::with_kind(
                            LoweringErrorKind::InvalidInput,
                            format!(
                                "standard '{}' requirement '{}' lost error type {} during lowering",
                                symbol.name, requirement.name, type_id.0
                            ),
                        ));
                        standard_failed = true;
                        break;
                    }
                },
                None => None,
            };
            lowered_required_routines.push(LoweredStandardRoutine {
                symbol_id: requirement.symbol_id,
                name: requirement.name.clone(),
                params,
                return_type,
                error_type,
            });
        }

        if standard_failed {
            continue;
        }

        lowered_package.standards.insert(
            standard.symbol_id,
            LoweredStandard {
                symbol_id: standard.symbol_id,
                name: symbol.name.clone(),
                source_unit_id: symbol.source_unit,
                scope_id: standard.scope_id,
                kind: standard.kind,
                required_routines: lowered_required_routines,
            },
        );
    }

    for conformance in typed_package.program.all_typed_conformances() {
        lowered_package.conformances.insert(
            conformance.type_symbol_id,
            LoweredConformance {
                type_symbol_id: conformance.type_symbol_id,
                standard_symbol_ids: conformance.standard_symbol_ids.clone(),
            },
        );
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

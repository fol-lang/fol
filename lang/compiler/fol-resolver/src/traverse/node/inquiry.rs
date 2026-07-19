use crate::{model::ResolvedProgram, ResolverError, ResolverErrorKind, ScopeId, SourceUnitId};
use fol_parser::ast::InquiryTarget;

use super::super::references::record_inquiry_target_reference;
use super::super::resolve::{qualified_path_origin, resolve_visible_symbol_of_kinds};
use super::RoutineContext;

pub fn resolve_inquiry_target(
    program: &mut ResolvedProgram,
    source_unit_id: SourceUnitId,
    scope_id: ScopeId,
    target: &InquiryTarget,
    routine_context: Option<RoutineContext>,
) -> Result<(), ResolverError> {
    match target {
        InquiryTarget::SelfValue => {
            if routine_context.is_none() {
                return Err(ResolverError::new(
                    ResolverErrorKind::InvalidInput,
                    "inquiry target 'self' requires an enclosing routine context",
                ));
            }
        }
        InquiryTarget::ThisValue => {
            if !routine_context.is_some_and(|context| context.this_available) {
                return Err(ResolverError::new(
                    ResolverErrorKind::InvalidInput,
                    "inquiry target 'this' requires an enclosing routine with a declared return type",
                ));
            }
        }
        InquiryTarget::Named(name) | InquiryTarget::Quoted(name) => {
            let symbol_id = resolve_visible_symbol_of_kinds(
                program,
                scope_id,
                name,
                &[],
                Some("inquiry target"),
                None,
            )?;
            record_inquiry_target_reference(program, source_unit_id, scope_id, name, symbol_id);
        }
        InquiryTarget::Qualified(path) => {
            let symbol_id = super::super::resolve::resolve_qualified_symbol(
                program,
                scope_id,
                path,
                &[],
                "qualified inquiry target",
                qualified_path_origin(program, path),
            )?;
            record_inquiry_target_reference(
                program,
                source_unit_id,
                scope_id,
                &path.joined(),
                symbol_id,
            );
        }
    }

    Ok(())
}

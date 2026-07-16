mod node;
mod references;
mod resolve;
mod scope;

pub use node::{traverse_node, traverse_top_level_item};

use crate::{
    collect::top_level_scope_id, model::ResolvedProgram, ResolverError, ResolverSession,
    SourceUnitId,
};

pub fn collect_routine_scopes(
    session: &mut ResolverSession,
    program: &mut ResolvedProgram,
) -> Result<(), Vec<ResolverError>> {
    let mut errors = Vec::new();
    let work_items = program
        .syntax()
        .source_units
        .iter()
        .enumerate()
        .flat_map(|(source_unit_id, syntax_unit)| {
            syntax_unit
                .items
                .iter()
                .cloned()
                .map(move |item| (SourceUnitId(source_unit_id), item))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (source_unit_id, item) in work_items {
        let scope_id = match top_level_scope_id(program, source_unit_id, &item) {
            Ok(id) => id,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        if let Err(error) =
            traverse_top_level_item(session, program, source_unit_id, scope_id, &item)
        {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

//! Synthesizing a foreign namespace from a checked import manifest.
//!
//! Section 4.13 is explicit that V4 adds no `extern` grammar: FOL source calls
//! an imported C routine through ordinary namespace lookup, exactly as it
//! calls a routine from any other package. That means the routines have to
//! exist as resolver symbols without any declaration behind them, which is
//! what this module builds.
//!
//! The symbols carry no syntax origin, because there is no FOL source to point
//! at. Their real origin is the header, which the manifest records per routine
//! and which the typed layer surfaces for hover and navigation.

use std::collections::BTreeMap;

use fol_abi::ImportedInterface;
use fol_parser::ast::ParsedSourceUnitKind;

use crate::{
    model::{
        ForeignHandleBinding, ForeignRecordBinding, ForeignRoutineBinding, ResolvedProgram,
        ResolvedScope, ResolvedSourceUnit, ResolvedSymbol, ScopeKind, SymbolKind,
    },
    ScopeId, SourceUnitId, SymbolId,
};

/// The synthetic source-unit path an import's namespace reports.
///
/// It is not a real file. It is spelled like one so every consumer that groups
/// diagnostics or symbols by source unit keeps working, and spelled distinctly
/// so nothing mistakes it for FOL source on disk.
pub fn synthetic_source_path(alias: &str) -> String {
    format!("<c-import:{alias}>")
}

/// Mount each checked import as a namespace of routine symbols.
///
/// Idempotent: an alias already present is left alone, so repeated resolution
/// of the same program does not accumulate duplicates.
pub fn inject_c_import_namespaces(program: &mut ResolvedProgram, interfaces: &[ImportedInterface]) {
    for interface in interfaces {
        inject_one(program, interface);
    }
}

fn inject_one(program: &mut ResolvedProgram, interface: &ImportedInterface) {
    if program.namespace_scope(&interface.alias).is_some() {
        return;
    }

    let source_unit_id = program.source_units.push(ResolvedSourceUnit {
        id: SourceUnitId(0),
        path: synthetic_source_path(&interface.alias),
        package: interface.alias.clone(),
        namespace: interface.alias.clone(),
        kind: ParsedSourceUnitKind::Ordinary,
        scope_id: ScopeId(0),
        top_level_nodes: Vec::new(),
    });
    if let Some(unit) = program.source_units.get_mut(source_unit_id) {
        unit.id = source_unit_id;
    }

    let root_scope = program.scopes.push(ResolvedScope {
        id: ScopeId(0),
        kind: ScopeKind::ProgramRoot {
            package: interface.alias.clone(),
        },
        parent: None,
        source_unit: Some(source_unit_id),
        symbols: Vec::new(),
        symbol_keys: BTreeMap::new(),
    });
    if let Some(scope) = program.scopes.get_mut(root_scope) {
        scope.id = root_scope;
    }
    if let Some(unit) = program.source_units.get_mut(source_unit_id) {
        unit.scope_id = root_scope;
    }
    program.register_namespace_scope(interface.alias.clone(), root_scope);

    // Handle domains first: a routine's signature names one, and a FOL author
    // writes `alias::Widget` in type position, so the type has to exist as a
    // symbol before anything refers to it.
    for domain in interface.handle_domains() {
        let canonical_name = fol_types::canonical_identifier_key(domain);
        let symbol_id = program.symbols.push(ResolvedSymbol {
            id: SymbolId(0),
            name: domain.to_string(),
            canonical_name: canonical_name.clone(),
            duplicate_key: format!("type#{canonical_name}"),
            kind: SymbolKind::Type,
            scope: root_scope,
            source_unit: source_unit_id,
            origin: None,
            visibility: Some(fol_parser::ast::ParsedDeclVisibility::Exported),
            declaration_scope: None,
            mounted_from: None,
            is_mutable: false,
        });
        if let Some(symbol) = program.symbols.get_mut(symbol_id) {
            symbol.id = symbol_id;
        }
        if let Some(scope) = program.scopes.get_mut(root_scope) {
            scope.symbols.push(symbol_id);
            scope
                .symbol_keys
                .entry(canonical_name)
                .or_default()
                .push(symbol_id);
        }
        program.register_foreign_handle(
            symbol_id,
            ForeignHandleBinding {
                alias: interface.alias.clone(),
                domain: domain.to_string(),
            },
        );
    }

    // Records next, for the same reason handle domains come first: a routine's
    // signature names one, and a FOL author writes `alias::point` in type
    // position, so the symbol has to exist before anything refers to it.
    for (name, _) in interface.record_shapes() {
        let canonical_name = fol_types::canonical_identifier_key(name);
        let symbol_id = program.symbols.push(ResolvedSymbol {
            id: SymbolId(0),
            name: name.to_string(),
            canonical_name: canonical_name.clone(),
            duplicate_key: format!("type#{canonical_name}"),
            kind: SymbolKind::Type,
            scope: root_scope,
            source_unit: source_unit_id,
            origin: None,
            visibility: Some(fol_parser::ast::ParsedDeclVisibility::Exported),
            declaration_scope: None,
            mounted_from: None,
            is_mutable: false,
        });
        if let Some(symbol) = program.symbols.get_mut(symbol_id) {
            symbol.id = symbol_id;
        }
        if let Some(scope) = program.scopes.get_mut(root_scope) {
            scope.symbols.push(symbol_id);
            scope
                .symbol_keys
                .entry(canonical_name)
                .or_default()
                .push(symbol_id);
        }
        program.register_foreign_record(
            symbol_id,
            ForeignRecordBinding {
                alias: interface.alias.clone(),
                name: name.to_string(),
            },
        );
    }

    for routine in &interface.routines {
        // A buffer domain's release is not mounted: FOL never holds the
        // provider's memory, so there is nothing for a program to release.
        if !routine.is_mountable() {
            continue;
        }
        let canonical_name = fol_types::canonical_identifier_key(&routine.fol_name);
        let symbol_id = program.symbols.push(ResolvedSymbol {
            id: SymbolId(0),
            name: routine.fol_name.clone(),
            canonical_name: canonical_name.clone(),
            duplicate_key: format!("routine#{canonical_name}"),
            kind: SymbolKind::Routine,
            scope: root_scope,
            source_unit: source_unit_id,
            // No FOL declaration exists, so there is no syntax to anchor to.
            origin: None,
            // Exported, or an importing package could not call it -- which is
            // the entire point of synthesizing the namespace.
            visibility: Some(fol_parser::ast::ParsedDeclVisibility::Exported),
            declaration_scope: None,
            mounted_from: None,
            is_mutable: false,
        });
        if let Some(symbol) = program.symbols.get_mut(symbol_id) {
            symbol.id = symbol_id;
        }
        if let Some(scope) = program.scopes.get_mut(root_scope) {
            scope.symbols.push(symbol_id);
            scope
                .symbol_keys
                .entry(canonical_name)
                .or_default()
                .push(symbol_id);
        }
        program.register_foreign_routine(
            symbol_id,
            ForeignRoutineBinding {
                alias: interface.alias.clone(),
                symbol: routine.symbol.clone(),
                fol_name: routine.fol_name.clone(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fol_abi::{
        AbiCallingConvention, AbiSourceOrigin, AbiTypeTable, ImportEffects, ImportErrorConvention,
        ImportedRoutine,
    };

    fn interface(alias: &str, names: &[&str]) -> ImportedInterface {
        let mut types = AbiTypeTable::new();
        let int_id = types.intern_int(fol_types::IntWidth::I32);
        ImportedInterface {
            alias: alias.to_string(),
            target: fol_types::ResolvedTarget::resolve("x86_64-unknown-linux-gnu")
                .expect("certified target"),
            types,
            routines: names
                .iter()
                .map(|name| ImportedRoutine {
                    symbol: format!("c_{name}"),
                    fol_name: (*name).to_string(),
                    convention: AbiCallingConvention::C,
                    parameters: Vec::new(),
                    result: int_id,
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
        }
    }

    fn empty_program() -> ResolvedProgram {
        ResolvedProgram::new(fol_parser::ast::ParsedPackage {
            package: "demo".to_string(),
            source_units: vec![fol_parser::ast::ParsedSourceUnit {
                path: "lib.fol".to_string(),
                package: "demo".to_string(),
                namespace: "demo".to_string(),
                kind: ParsedSourceUnitKind::Ordinary,
                items: Vec::new(),
            }],
            syntax_index: Default::default(),
        })
    }

    #[test]
    fn an_import_becomes_a_namespace_of_routine_symbols() {
        let mut program = empty_program();
        inject_c_import_namespaces(&mut program, &[interface("c_math", &["add_one", "square"])]);

        let scope = program
            .namespace_scope("c_math")
            .expect("the alias should become a namespace");
        let names: Vec<&str> = program
            .symbols_in_scope(scope)
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect();
        assert_eq!(names, vec!["add_one", "square"]);
        assert!(program
            .symbols_in_scope(scope)
            .iter()
            .all(|symbol| symbol.kind == SymbolKind::Routine));
    }

    #[test]
    fn injection_is_idempotent() {
        let mut program = empty_program();
        let interfaces = [interface("c_math", &["add_one"])];
        inject_c_import_namespaces(&mut program, &interfaces);
        let after_first = program.symbols.len();
        inject_c_import_namespaces(&mut program, &interfaces);

        assert_eq!(
            program.symbols.len(),
            after_first,
            "re-injecting the same alias must not duplicate its routines"
        );
    }

    #[test]
    fn two_imports_get_separate_namespaces() {
        let mut program = empty_program();
        inject_c_import_namespaces(
            &mut program,
            &[interface("c_math", &["add"]), interface("c_str", &["add"])],
        );

        let math = program.namespace_scope("c_math").expect("c_math");
        let string = program.namespace_scope("c_str").expect("c_str");
        assert_ne!(math, string, "distinct aliases are distinct namespaces");
        // The same FOL name in two imports is two symbols, not a collision.
        assert_eq!(program.symbols_in_scope(math).len(), 1);
        assert_eq!(program.symbols_in_scope(string).len(), 1);
    }

    #[test]
    fn a_foreign_routine_carries_no_syntax_origin() {
        let mut program = empty_program();
        inject_c_import_namespaces(&mut program, &[interface("c_math", &["add_one"])]);

        let scope = program.namespace_scope("c_math").expect("namespace");
        let symbol = program.symbols_in_scope(scope)[0].clone();
        // There is no FOL source for a C declaration; pointing at one would be
        // a lie the editor would then try to open.
        assert!(symbol.origin.is_none());
        assert_eq!(
            program
                .source_unit(symbol.source_unit)
                .map(|unit| unit.path.as_str()),
            Some("<c-import:c_math>")
        );
    }
}

use crate::{
    model::{ResolvedProgram, ResolvedSymbol, SymbolKind},
    SourceUnitId, SymbolId,
};
use fol_build::BuildStdlibScope;

/// Injects synthetic `Type` symbols for every type in `BuildStdlibScope::canonical()`
/// into the `build_stdlib_scope` of `program`.
///
/// Idempotent: skips symbols that are already present.  No-op if the program
/// has no build stdlib scope (i.e. no `Build` source units).
pub fn inject_build_stdlib_types(program: &mut ResolvedProgram) {
    let Some(stdlib_scope) = program.build_stdlib_scope else {
        return;
    };

    let build_unit_id = program
        .build_source_units()
        .next()
        .map(|unit| unit.id)
        .unwrap_or(SourceUnitId(0));

    let scope = BuildStdlibScope::canonical();
    for typ in &scope.types {
        let canonical_name = fol_types::canonical_identifier_key(&typ.name);

        if program
            .symbols_named_in_scope(stdlib_scope, &canonical_name)
            .iter()
            .any(|s| s.kind == SymbolKind::Type)
        {
            continue;
        }

        let duplicate_key = format!("type#{}", canonical_name);
        let symbol_id = program.symbols.push(ResolvedSymbol {
            id: SymbolId(0),
            name: typ.name.clone(),
            canonical_name: canonical_name.clone(),
            duplicate_key,
            kind: SymbolKind::Type,
            scope: stdlib_scope,
            source_unit: build_unit_id,
            origin: None,
            visibility: None,
            declaration_scope: None,
            mounted_from: None,
            is_mutable: false,
        });
        if let Some(symbol) = program.symbols.get_mut(symbol_id) {
            symbol.id = symbol_id;
        }
        if let Some(scope) = program.scopes.get_mut(stdlib_scope) {
            scope.symbols.push(symbol_id);
            scope
                .symbol_keys
                .entry(canonical_name)
                .or_default()
                .push(symbol_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::inject_build_stdlib_types;
    use crate::model::{ResolvedProgram, ScopeKind, SymbolKind};
    use fol_parser::ast::{ParsedPackage, ParsedSourceUnit, ParsedSourceUnitKind};

    fn build_only_package(package: &str) -> ParsedPackage {
        ParsedPackage {
            package: package.to_string(),
            source_units: vec![ParsedSourceUnit {
                path: "build.fol".to_string(),
                package: package.to_string(),
                namespace: package.to_string(),
                kind: ParsedSourceUnitKind::Build,
                items: Vec::new(),
            }],
            syntax_index: Default::default(),
        }
    }

    fn ordinary_only_package(package: &str) -> ParsedPackage {
        ParsedPackage {
            package: package.to_string(),
            source_units: vec![ParsedSourceUnit {
                path: "lib.fol".to_string(),
                package: package.to_string(),
                namespace: package.to_string(),
                kind: ParsedSourceUnitKind::Ordinary,
                items: Vec::new(),
            }],
            syntax_index: Default::default(),
        }
    }

    #[test]
    fn inject_keeps_public_graph_type_out_of_build_stdlib_scope() {
        let mut program = ResolvedProgram::new(build_only_package("demo"));
        program.init_build_stdlib_scope();
        inject_build_stdlib_types(&mut program);

        let stdlib_scope = program
            .build_stdlib_scope
            .expect("build_stdlib_scope should be set after init");

        let graph_symbols = program.symbols_named_in_scope(stdlib_scope, "graph");
        assert!(
            graph_symbols.iter().all(|s| s.kind != SymbolKind::Type),
            "Graph must stay internal and not be injected into the build stdlib scope"
        );
    }

    #[test]
    fn inject_covers_all_canonical_build_types() {
        let mut program = ResolvedProgram::new(build_only_package("demo"));
        program.init_build_stdlib_scope();
        inject_build_stdlib_types(&mut program);

        let stdlib_scope = program.build_stdlib_scope.unwrap();
        let type_names: Vec<String> = program
            .symbols_in_scope(stdlib_scope)
            .iter()
            .filter(|s| s.kind == SymbolKind::Type)
            .map(|s| s.name.clone())
            .collect();

        assert!(type_names.contains(&"Artifact".to_string()));
        assert!(type_names.contains(&"Step".to_string()));
        assert!(type_names.contains(&"Run".to_string()));
        assert!(type_names.contains(&"Install".to_string()));
        assert!(type_names.contains(&"Dependency".to_string()));
        assert!(type_names.contains(&"GeneratedFile".to_string()));
    }

    #[test]
    fn inject_is_idempotent() {
        let mut program = ResolvedProgram::new(build_only_package("demo"));
        program.init_build_stdlib_scope();
        inject_build_stdlib_types(&mut program);
        inject_build_stdlib_types(&mut program);

        let stdlib_scope = program.build_stdlib_scope.unwrap();
        let graph_symbols = program.symbols_named_in_scope(stdlib_scope, "graph");
        assert_eq!(
            graph_symbols
                .iter()
                .filter(|s| s.kind == SymbolKind::Type)
                .count(),
            0,
            "Repeated injection should not inject a public Graph type symbol"
        );
    }

    #[test]
    fn inject_is_noop_for_ordinary_only_packages() {
        let mut program = ResolvedProgram::new(ordinary_only_package("lib"));
        program.init_build_stdlib_scope();
        inject_build_stdlib_types(&mut program);

        assert!(
            program.build_stdlib_scope.is_none(),
            "Ordinary packages should have no build stdlib scope"
        );
        let graph_symbols = program.symbols_named_in_scope(program.program_scope, "graph");
        assert!(
            graph_symbols.is_empty(),
            "Ordinary packages should not see an injected Graph type"
        );
    }

    #[test]
    fn build_stdlib_scope_has_no_parent() {
        let mut program = ResolvedProgram::new(build_only_package("demo"));
        program.init_build_stdlib_scope();

        let stdlib_scope = program.build_stdlib_scope.unwrap();
        let scope = program
            .scope(stdlib_scope)
            .expect("build stdlib scope should exist in program");

        assert_eq!(scope.kind, ScopeKind::BuildStdlib);
        assert!(
            scope.parent.is_none(),
            "BuildStdlib scope should have no parent for full isolation"
        );
    }

    #[test]
    fn build_source_unit_scope_parent_is_build_stdlib_not_program_scope() {
        let mut program = ResolvedProgram::new(build_only_package("demo"));
        program.init_build_stdlib_scope();

        let stdlib_scope = program.build_stdlib_scope.unwrap();
        let build_unit = program
            .build_source_units()
            .next()
            .expect("build package should have a build source unit");
        let build_unit_scope = program
            .scope(build_unit.scope_id)
            .expect("build source unit scope should exist");

        assert_eq!(
            build_unit_scope.parent,
            Some(stdlib_scope),
            "Build source unit scope parent should be the BuildStdlib scope"
        );
        assert_ne!(
            build_unit_scope.parent,
            Some(program.program_scope),
            "Build source unit scope should NOT have program_scope as parent"
        );
    }
}

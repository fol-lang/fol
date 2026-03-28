use fol_diagnostics::{DiagnosticCode, DiagnosticLocation, ToDiagnostic};
use fol_parser::ast::{AstParser, SyntaxOrigin};
use fol_resolver::{resolve_package, resolve_package_workspace_with_config};
use fol_resolver::{ReferenceKind, ResolverConfig, SourceUnitId, SymbolId, SymbolKind};
use fol_stream::FileStream;
use fol_typecheck::{
    BuiltinType, BuiltinTypeIds, CheckedType, DeclaredTypeKind, RoutineType, TypeTable,
    TypecheckConfig, TypecheckError, TypecheckErrorKind, Typechecker,
};
use std::collections::BTreeMap;
use std::fs::{create_dir_all, write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn resolve_fixture(path: &str) -> fol_resolver::ResolvedProgram {
    let fixture_path = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path);
    let mut stream = FileStream::from_file(&fixture_path).expect("Should open typecheck fixture");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("Typecheck fixture should parse as a package");

    resolve_package(syntax).expect("Typecheck fixture should resolve cleanly")
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time should be after unix epoch")
        .as_nanos();
    let sequence = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "fol_typecheck_{prefix}_{}_{}_{}",
        std::process::id(),
        nonce,
        sequence
    ))
}

fn write_fixture_files(root: &Path, files: &[(&str, &str)]) {
    for (relative_path, contents) in files {
        let path = root.join(relative_path);
        if let Some(parent) = path.parent() {
            create_dir_all(parent).expect("Fixture parent directories should be creatable");
        }
        write(&path, contents).expect("Fixture file should be writable");
    }
}

#[test]
fn unique_temp_dir_produces_distinct_paths_for_rapid_calls() {
    let first = unique_temp_dir("collision_check");
    let second = unique_temp_dir("collision_check");

    assert_ne!(first, second);
}

fn typecheck_fixture_folder(files: &[(&str, &str)]) -> fol_typecheck::TypedProgram {
    typecheck_fixture_folder_with_config(files, TypecheckConfig::default())
}

fn typecheck_fixture_folder_with_config(
    files: &[(&str, &str)],
    config: TypecheckConfig,
) -> fol_typecheck::TypedProgram {
    let root = unique_temp_dir("package");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(&root, files);

    let mut stream = FileStream::from_folder(root.to_str().expect("fixture path should be utf8"))
        .expect("Fixture folder should stream");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("Fixture folder should parse as a package");
    let resolved = resolve_package(syntax).expect("Fixture folder should resolve cleanly");

    Typechecker::with_config(config)
        .check_resolved_program(resolved)
        .expect("Fixture folder should typecheck declaration signatures")
}

fn typecheck_fixture_folder_errors(files: &[(&str, &str)]) -> Vec<TypecheckError> {
    typecheck_fixture_folder_errors_with_config(files, TypecheckConfig::default())
}

fn typecheck_fixture_folder_errors_with_config(
    files: &[(&str, &str)],
    config: TypecheckConfig,
) -> Vec<TypecheckError> {
    let root = unique_temp_dir("package_errors");
    create_dir_all(&root).expect("Fixture root should be creatable");
    write_fixture_files(&root, files);

    let mut stream = FileStream::from_folder(root.to_str().expect("fixture path should be utf8"))
        .expect("Fixture folder should stream");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("Fixture folder should parse as a package");
    let resolved = resolve_package(syntax).expect("Fixture folder should resolve cleanly");

    Typechecker::with_config(config)
        .check_resolved_program(resolved)
        .expect_err("Fixture folder should fail typechecking")
}

fn typecheck_fixture_entry_with_config(
    root: &Path,
    entry: &str,
    config: ResolverConfig,
) -> Result<fol_typecheck::TypedProgram, Vec<TypecheckError>> {
    let entry_root = root.join(entry);
    let mut stream =
        FileStream::from_folder(entry_root.to_str().expect("fixture path should be utf8"))
            .expect("Fixture folder should stream");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("Fixture folder should parse as a package");
    let resolved = fol_resolver::resolve_package_with_config(syntax, config)
        .expect("Fixture folder should resolve cleanly");

    Typechecker::new().check_resolved_program(resolved)
}

fn typecheck_fixture_workspace_with_config(
    root: &Path,
    entry: &str,
    config: ResolverConfig,
) -> Result<fol_typecheck::TypedWorkspace, Vec<TypecheckError>> {
    typecheck_fixture_workspace_with_models(root, entry, config, TypecheckConfig::default())
}

fn typecheck_fixture_workspace_with_models(
    root: &Path,
    entry: &str,
    config: ResolverConfig,
    typecheck_config: TypecheckConfig,
) -> Result<fol_typecheck::TypedWorkspace, Vec<TypecheckError>> {
    let entry_root = root.join(entry);
    let mut stream =
        FileStream::from_folder(entry_root.to_str().expect("fixture path should be utf8"))
            .expect("Fixture folder should stream");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("Fixture folder should parse as a package");
    let resolved = resolve_package_workspace_with_config(syntax, config)
        .expect("Fixture folder should resolve cleanly as a workspace");

    Typechecker::with_config(typecheck_config).check_resolved_workspace(resolved)
}

fn typecheck_fixture_workspace_entry_with_config(
    root: &Path,
    entry: &str,
    config: ResolverConfig,
) -> Result<fol_typecheck::TypedProgram, Vec<TypecheckError>> {
    typecheck_fixture_workspace_with_config(root, entry, config)
        .map(|typed| typed.entry_program().clone())
}

fn find_typed_symbol<'a>(
    typed: &'a fol_typecheck::TypedProgram,
    name: &str,
    kind: SymbolKind,
) -> (SymbolId, &'a fol_typecheck::TypedSymbol) {
    let symbol_id = typed
        .resolved()
        .symbols
        .iter_with_ids()
        .find(|(_, symbol)| symbol.name == name && symbol.kind == kind)
        .map(|(symbol_id, _)| symbol_id)
        .expect("typed fixture symbol should exist");
    let symbol = typed
        .typed_symbol(symbol_id)
        .expect("typed symbol facts should exist for resolved symbol");

    (symbol_id, symbol)
}

fn find_typed_package<'a>(
    typed: &'a fol_typecheck::TypedWorkspace,
    display_name: &str,
) -> &'a fol_typecheck::TypedPackage {
    typed
        .packages()
        .find(|package| package.identity.display_name == display_name)
        .expect("typed workspace package should exist")
}

fn find_typed_reference<'a>(
    typed: &'a fol_typecheck::TypedProgram,
    name: &str,
    kind: ReferenceKind,
) -> &'a fol_typecheck::TypedReference {
    let reference_id = typed
        .resolved()
        .references
        .iter_with_ids()
        .find(|(_, reference)| reference.name == name && reference.kind == kind)
        .map(|(reference_id, _)| reference_id)
        .expect("typed fixture reference should exist");
    typed
        .typed_reference(reference_id)
        .expect("typed reference facts should exist")
}

fn find_typed_references<'a>(
    typed: &'a fol_typecheck::TypedProgram,
    name: &str,
    kind: ReferenceKind,
) -> Vec<&'a fol_typecheck::TypedReference> {
    typed
        .resolved()
        .references
        .iter_with_ids()
        .filter(|(_, reference)| reference.name == name && reference.kind == kind)
        .map(|(reference_id, _)| {
            typed
                .typed_reference(reference_id)
                .expect("typed reference facts should exist")
        })
        .collect()
}

fn assert_imported_declared_count_binding_and_routine(
    typed: &fol_typecheck::TypedProgram,
    count_symbol: SymbolKind,
) {
    let (count_id, _count) = find_typed_symbol(typed, "Count", count_symbol);
    let (_answer_id, answer) = find_typed_symbol(typed, "answer", SymbolKind::ValueBinding);
    let (_bump_id, bump) = find_typed_symbol(typed, "bump", SymbolKind::Routine);

    assert_eq!(
        typed.type_table().get(
            answer
                .declared_type
                .expect("imported values should keep declared semantic types"),
        ),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: match count_symbol {
                SymbolKind::Alias => DeclaredTypeKind::Alias,
                _ => DeclaredTypeKind::Type,
            },
        })
    );

    let signature = match typed.type_table().get(
        bump.declared_type
            .expect("imported routines should keep translated signatures"),
    ) {
        Some(CheckedType::Routine(signature)) => signature,
        other => panic!("expected imported routine signature, got {other:?}"),
    };
    assert_eq!(signature.params.len(), 1);
    assert_eq!(signature.error_type, None);
    assert_eq!(
        typed.type_table().get(signature.params[0]),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: match count_symbol {
                SymbolKind::Alias => DeclaredTypeKind::Alias,
                _ => DeclaredTypeKind::Type,
            },
        })
    );
    assert_eq!(
        signature
            .return_type
            .and_then(|type_id| typed.type_table().get(type_id)),
        Some(&CheckedType::Declared {
            symbol: count_id,
            name: "Count".to_string(),
            kind: match count_symbol {
                SymbolKind::Alias => DeclaredTypeKind::Alias,
                _ => DeclaredTypeKind::Type,
            },
        })
    );
}

fn find_named_routine_syntax_id(
    typed: &fol_typecheck::TypedProgram,
    name: &str,
) -> fol_parser::ast::SyntaxNodeId {
    typed
        .resolved()
        .syntax()
        .source_units
        .iter()
        .flat_map(|unit| unit.items.iter())
        .find_map(|item| match &item.node {
            fol_parser::ast::AstNode::FunDecl {
                name: routine_name,
                syntax_id: Some(syntax_id),
                ..
            }
            | fol_parser::ast::AstNode::ProDecl {
                name: routine_name,
                syntax_id: Some(syntax_id),
                ..
            }
            | fol_parser::ast::AstNode::LogDecl {
                name: routine_name,
                syntax_id: Some(syntax_id),
                ..
            } if routine_name == name => Some(*syntax_id),
            _ => None,
        })
        .expect("named routine syntax id should exist")
}


#[cfg(test)]
#[path = "test_typecheck_foundation.rs"]
mod typecheck_foundation;

#[cfg(test)]
#[path = "test_typecheck_error_typing.rs"]
mod typecheck_error_typing;

#[cfg(test)]
#[path = "test_typecheck_containers_and_shells.rs"]
mod typecheck_containers_and_shells;

#[cfg(test)]
#[path = "test_typecheck_v1_and_workspace.rs"]
mod typecheck_v1_and_workspace;

#[cfg(test)]
#[path = "test_typecheck_workspace_imports.rs"]
mod typecheck_workspace_imports;

#[cfg(test)]
#[path = "test_typecheck_operators.rs"]
mod typecheck_operators;

#[cfg(test)]
#[path = "test_typecheck_generics_m1.rs"]
mod typecheck_generics_m1;
#[cfg(test)]
#[path = "test_typecheck_standards_m2.rs"]
mod typecheck_standards_m2;

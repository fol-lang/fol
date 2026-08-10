mod calls;
mod containers;
mod flow;
mod literals;
mod mono;
mod operators;

use fol_parser::ast::AstParser;
use fol_resolver::resolve_package_workspace;
use fol_stream::FileStream;
use fol_typecheck::Typechecker;

pub(super) fn lower_folder_fixture_workspace(files: &[(&str, &str)]) -> crate::LoweredWorkspace {
    let root = fol_testkit::TempFixture::new("fol_lower_success_folder");
    std::fs::create_dir_all(&root).expect("should create lowering folder fixture root");
    for (path, source) in files {
        let full_path = root.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .expect("should create lowering folder fixture parent directories");
        }
        std::fs::write(&full_path, source).expect("should write lowering folder fixture");
    }

    let app_root = root.join("app");
    let mut stream = FileStream::from_folder(app_root.to_str().expect("utf8 temp path"))
        .expect("Should open lowering folder fixture");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("Lowering folder fixture should parse");
    let resolved =
        resolve_package_workspace(syntax).expect("Lowering folder fixture should resolve");
    let typed = Typechecker::new()
        .check_resolved_workspace(resolved)
        .expect("Lowering folder fixture should typecheck");
    crate::LoweringSession::new(typed)
        .lower_workspace()
        .expect("folder fixture should lower successfully")
}

pub(super) fn lower_fixture_workspace(source: &str) -> crate::LoweredWorkspace {
    let fixture =
        fol_testkit::TempFixture::new("fol_lower_success").with_file("fol_lower_success.fol");
    std::fs::write(&fixture, source).expect("should write lowering success fixture");

    let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
        .expect("Should open lowering fixture");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("Lowering fixture should parse");
    let resolved = resolve_package_workspace(syntax).expect("Lowering fixture should resolve");
    let typed = Typechecker::new()
        .check_resolved_workspace(resolved)
        .expect("Lowering fixture should typecheck");
    crate::LoweringSession::new(typed)
        .lower_workspace()
        .expect("fixture should lower successfully")
}

/// Lower a fixture that is expected to fail, returning the errors.
fn lowering_errors(source: &str) -> Vec<crate::LoweringError> {
    let fixture = fol_testkit::TempFixture::new("fol_lower_cycle").with_file("fol_lower_cycle.fol");
    std::fs::write(&fixture, source).expect("should write lowering fixture");

    let mut stream = FileStream::from_file(fixture.to_str().expect("utf8 temp path"))
        .expect("Should open lowering fixture");
    let mut lexer = fol_lexer::lexer::stage3::Elements::init(&mut stream);
    let mut parser = AstParser::new();
    let syntax = parser
        .parse_package(&mut lexer)
        .expect("fixture should parse");
    let resolved = resolve_package_workspace(syntax).expect("fixture should resolve");
    let typed = Typechecker::new()
        .check_resolved_workspace(resolved)
        .expect("fixture should typecheck");
    crate::LoweringSession::new(typed)
        .lower_workspace()
        .expect_err("a self-referential type should not lower")
}

#[test]
fn a_type_that_expands_to_itself_is_reported_instead_of_overflowing_the_stack() {
    // Each of these used to recurse until the process died with
    // "fatal runtime error: stack overflow" and exit 134 -- no diagnostic, no
    // file, nothing for the user to act on.
    for source in [
        "ali A: A;\n\nfun[] main(): int = {\n    return 0;\n};\n",
        "ali A: B;\nali B: A;\n\nfun[] main(): int = {\n    return 0;\n};\n",
        "ali A: vec[A];\n\nfun[] main(): int = {\n    return 0;\n};\n",
    ] {
        let errors = lowering_errors(source);
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("defined in terms of itself")),
            "expected a self-reference diagnostic for {source:?}, got: {errors:?}"
        );
    }
}

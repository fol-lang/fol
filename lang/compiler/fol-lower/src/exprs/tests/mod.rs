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

/// Return a temp directory whose leaf name is a valid FOL identifier.
/// NixOS nix-shell creates temp dirs like `nix-shell.vlxfu8` that contain
/// dots/dashes — invalid for FOL package name inference.
pub(super) fn safe_temp_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("fol_test");
    std::fs::create_dir_all(&dir).expect("should create test temp root");
    dir
}

pub(super) fn lower_folder_fixture_workspace(files: &[(&str, &str)]) -> crate::LoweredWorkspace {
    let root = safe_temp_dir().join(format!(
        "fol_lower_success_folder_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
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
    let fixture = safe_temp_dir().join(format!(
        "fol_lower_success_{}.fol",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock should be monotonic enough for tmp names")
            .as_nanos()
    ));
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

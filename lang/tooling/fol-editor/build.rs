use std::path::Path;

fn main() {
    let parser_dir = Path::new("tree-sitter/src");
    let parser = parser_dir.join("parser.c");

    cc::Build::new()
        .include(parser_dir)
        .file(&parser)
        .warnings(false)
        .compile("tree-sitter-fol");

    println!("cargo:rerun-if-changed={}", parser.display());
    println!(
        "cargo:rerun-if-changed={}",
        parser_dir.join("tree_sitter/parser.h").display()
    );
}

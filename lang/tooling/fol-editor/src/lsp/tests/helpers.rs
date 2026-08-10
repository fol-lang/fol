use super::super::{
    EditorLspServer, JsonRpcNotification, LspDidOpenTextDocumentParams,
    LspPublishDiagnosticsParams, LspTextDocumentItem,
};
use std::fs;
use std::path::Path;

pub(super) fn temp_root(label: &str) -> fol_testkit::TempFixture {
    let root = fol_testkit::TempFixture::new(&format!("fol_editor_lsp_{label}"));
    std::fs::create_dir_all(&root).expect("test root should be creatable");
    std::fs::create_dir_all(root.join(".git")).expect("test workspace marker should be creatable");
    root
}

pub(super) fn sample_package_root(label: &str) -> (fol_testkit::TempFixture, String) {
    let root = temp_root(label);
    let src = root.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(
        root.join("build.fol"),
        "pro[] build(): non = {\n    var build = .build();\n    build.meta({ name = \"sample\", version = \"0.1.0\" });\n    var graph = build.graph();\n    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n    return;\n};\n",
    )
    .unwrap();
    let file = src.join("main.fol");
    fs::write(&file, "fun[] main(): int = {\n    return 0;\n};\n").unwrap();
    let uri = format!("file://{}", file.display());
    (root, uri)
}

/// A single-package fixture whose build declares the bundled internal std
/// dependency, so hosted intrinsics such as `.echo` are legal completions.
pub(super) fn hosted_sample_package_root(label: &str) -> (fol_testkit::TempFixture, String) {
    let (root, uri) = sample_package_root(label);
    fs::write(
        root.join("build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"sample\", version = \"0.1.0\" });\n",
            "    build.add_dep({ alias = \"std\", source = \"internal\", target = \"standard\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"sample\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    (root, uri)
}

pub(super) fn sample_loc_workspace_root(label: &str) -> (fol_testkit::TempFixture, String) {
    let root = temp_root(label);
    let app_src = root.join("app/src");
    let shared_src = root.join("shared");
    fs::create_dir_all(&app_src).unwrap();
    fs::create_dir_all(&shared_src).unwrap();
    fs::write(
        root.join("fol.work.yaml"),
        "members:\n  - app\n  - shared\n",
    )
    .unwrap();

    fs::write(
        root.join("app/build.fol"),
        concat!(
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"app\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    graph.add_exe({ name = \"app\", root = \"src/main.fol\", fol_model = \"memo\" });\n",
            "    return;\n",
            "};\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("app/src/main.fol"),
        "use shared: loc = {\"../../shared\"};\n\nfun[] main(): int = {\n    return shared::helper();\n};\n",
    )
    .unwrap();
    fs::write(
        root.join("shared/lib.fol"),
        "fun[exp] helper(): int = {\n    return 9;\n};\n",
    )
    .unwrap();

    let uri = format!("file://{}", root.join("app/src/main.fol").display());
    (root, uri)
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir_all(&from, &to);
        } else {
            fs::copy(&from, &to).unwrap();
        }
    }
}

pub(super) fn copied_example_package_root(
    example_path: &str,
) -> (fol_testkit::TempFixture, String) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(example_path)
        .canonicalize()
        .expect("checked-in example path should canonicalize");
    let root = temp_root(&format!("example_copy_{}", example_path.replace('/', "_")));
    copy_dir_all(&source, &root);
    fs::create_dir_all(root.join(".git")).unwrap();
    let uri = format!("file://{}", root.join("src/main.fol").display());
    (root, uri)
}

pub(super) fn open_document(
    server: &mut EditorLspServer,
    uri: String,
    text: &str,
) -> Vec<LspPublishDiagnosticsParams> {
    server
        .handle_notification(JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didOpen".to_string(),
            params: Some(
                serde_json::to_value(LspDidOpenTextDocumentParams {
                    text_document: LspTextDocumentItem {
                        uri,
                        language_id: "fol".to_string(),
                        version: 1,
                        text: text.to_string(),
                    },
                })
                .unwrap(),
            ),
        })
        .unwrap()
}

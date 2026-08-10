use fol_frontend::{init_root, new_project_with_mode, FrontendArtifactKind, PackageTargetKind};
use std::fs;

fn temp_root(label: &str) -> fol_testkit::TempFixture {
    fol_testkit::TempFixture::new(&format!("fol_frontend_integration_{label}"))
}

#[test]
fn init_root_scaffolds_binary_packages_through_public_api() {
    let root = temp_root("init_bin");
    fs::create_dir_all(&root).expect("should create integration temp root");

    let result = init_root(&root, false, PackageTargetKind::Bin).expect("init should succeed");

    assert_eq!(result.command, "init");
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::PackageRoot);
    assert_eq!(
        fs::read_to_string(root.join("src/main.fol")).expect("should read main"),
        concat!(
            "use std: pkg = {\"std\"};\n",
            "\n",
            "fun[] main(): int = {\n",
            "    std::io::echo_str(\"hello from fol\");\n",
            "    return 0;\n",
            "};\n",
        )
    );
    assert_eq!(
        fs::read_to_string(root.join("build.fol")).expect("should read build file"),
        format!(
            concat!(
            "// build.fol is the package build entry file.\n",
            "pro[] build(): non = {{\n",
            "    var build = .build();\n",
            "    build.meta({{ name = \"{name}\", version = \"0.1.0\" }});\n",
            "    build.add_dep({{ alias = \"std\", source = \"internal\", target = \"standard\" }});\n",
            "    var graph = build.graph();\n",
            "    var app = graph.add_exe({{ name = \"{name}\", root = \"src/main.fol\", fol_model = \"memo\" }});\n",
            "    graph.install(app);\n",
            "    graph.add_run(app);\n",
            "}};\n",
        ),
            name = root.file_name().and_then(|name| name.to_str()).unwrap_or("app")
        )
    );

    fs::remove_dir_all(root).ok();
}

#[test]
fn init_root_scaffolds_workspace_roots_through_public_api() {
    let root = temp_root("init_workspace");
    fs::create_dir_all(&root).expect("should create integration temp root");

    let result =
        init_root(&root, true, PackageTargetKind::Bin).expect("workspace init should succeed");

    assert_eq!(result.command, "init");
    assert_eq!(
        result.artifacts[0].kind,
        FrontendArtifactKind::WorkspaceRoot
    );
    assert!(root.join("fol.work.yaml").is_file());
    assert!(!root.join("build.fol").exists());

    fs::remove_dir_all(root).ok();
}

#[test]
fn new_project_scaffolds_library_packages_through_public_api() {
    let parent = temp_root("new_lib");
    fs::create_dir_all(&parent).expect("should create integration temp parent");

    let result = new_project_with_mode(&parent, "demo", false, PackageTargetKind::Lib)
        .expect("new project should succeed");
    let root = parent.join("demo");

    assert_eq!(result.command, "new");
    assert_eq!(result.artifacts[0].kind, FrontendArtifactKind::PackageRoot);
    assert_eq!(
        fs::read_to_string(root.join("src/lib.fol")).expect("should read lib"),
        "fun[exp] demo(): int = {\n    return 0\n};\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("build.fol")).expect("should read build file"),
        concat!(
            "// build.fol is the package build entry file.\n",
            "pro[] build(): non = {\n",
            "    var build = .build();\n",
            "    build.meta({ name = \"demo\", version = \"0.1.0\" });\n",
            "    var graph = build.graph();\n",
            "    var lib = graph.add_static_lib({ name = \"demo\", root = \"src/lib.fol\" });\n",
            "    graph.install(lib);\n",
            "};\n",
        )
    );

    fs::remove_dir_all(parent).ok();
}

#[test]
fn new_project_scaffolds_workspace_roots_through_public_api() {
    let parent = temp_root("new_workspace");
    fs::create_dir_all(&parent).expect("should create integration temp parent");

    let result = new_project_with_mode(&parent, "demo", true, PackageTargetKind::Bin)
        .expect("workspace new should succeed");
    let root = parent.join("demo");

    assert_eq!(result.command, "new");
    assert_eq!(
        result.artifacts[0].kind,
        FrontendArtifactKind::WorkspaceRoot
    );
    assert!(root.join("fol.work.yaml").is_file());
    assert!(!root.join("build.fol").exists());

    fs::remove_dir_all(parent).ok();
}

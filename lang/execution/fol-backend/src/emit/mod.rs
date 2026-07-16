mod build;
mod runtime;
mod skeleton;
mod tests;

pub use build::{
    backend_build_paths, build_generated_crate_with_auxiliary_plan,
    build_generated_crate_with_rustc, build_generated_crate_with_rustc_args,
    build_runtime_rlib_with_rustc, emit_backend_artifact, prepare_backend_build_paths,
    prepare_generated_build_dir, summarize_emitted_artifact, write_generated_crate,
};
pub use runtime::{
    backend_runtime_build_dir, backend_runtime_manifest_path,
    backend_runtime_manifest_path_with_override, backend_runtime_source_entry,
    backend_runtime_source_entry_with_override, backend_runtime_source_root,
    backend_runtime_source_root_with_override, prepare_backend_runtime_build_dir,
};
#[allow(unused_imports)]
pub use skeleton::{
    emit_cargo_toml, emit_generated_crate_skeleton, emit_generated_crate_skeleton_for_config,
    emit_main_rs, emit_main_rs_for_config, emit_namespace_module_shells,
    emit_namespace_module_shells_for_config, emit_package_module_shells,
};

use crate::eval::BuildEvaluationOperation;
use crate::runtime::{
    BuildRuntimeDependencyExport, BuildRuntimeDependencyQuery, BuildRuntimeGeneratedFile,
};
use std::collections::BTreeMap;

// ---- Execution output container ---

#[derive(Debug, Default)]
pub struct ExecutionOutput {
    pub operations: Vec<BuildEvaluationOperation>,
    pub generated_files: Vec<BuildRuntimeGeneratedFile>,
    pub dependency_exports: Vec<BuildRuntimeDependencyExport>,
    pub dependency_queries: Vec<BuildRuntimeDependencyQuery>,
    pub run_steps: BTreeMap<String, String>,
}

use crate::{
    lsp::analysis::{CachedDiagnosticSnapshot, CachedSemanticSnapshot},
    EditorDocumentStore, EditorWorkspaceMapping, EditorWorkspaceRoots,
};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorConfig {
    pub full_document_sync: bool,
    pub root_markers: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EditorSession {
    pub config: EditorConfig,
    pub documents: EditorDocumentStore,
    pub mappings: BTreeMap<String, EditorWorkspaceMapping>,
    pub(crate) workspace_roots: BTreeMap<PathBuf, EditorWorkspaceRoots>,
    pub(crate) diagnostic_snapshots: BTreeMap<String, CachedDiagnosticSnapshot>,
    pub(crate) semantic_snapshots: BTreeMap<String, CachedSemanticSnapshot>,
    pub shutdown_requested: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            full_document_sync: false,
            root_markers: vec![
                "fol.work.yaml".to_string(),
                "build.fol".to_string(),
                ".git".to_string(),
            ],
        }
    }
}

impl EditorSession {
    pub fn new(config: EditorConfig) -> Self {
        Self {
            config,
            documents: EditorDocumentStore::default(),
            mappings: BTreeMap::new(),
            workspace_roots: BTreeMap::new(),
            diagnostic_snapshots: BTreeMap::new(),
            semantic_snapshots: BTreeMap::new(),
            shutdown_requested: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EditorConfig, EditorSession};

    #[test]
    fn editor_config_defaults_to_incremental_sync_and_standard_root_markers() {
        let config = EditorConfig::default();

        assert!(!config.full_document_sync);
        assert_eq!(
            config.root_markers,
            vec![
                "fol.work.yaml".to_string(),
                "build.fol".to_string(),
                ".git".to_string()
            ]
        );
    }

    #[test]
    fn editor_session_starts_with_an_empty_document_store() {
        let session = EditorSession::new(EditorConfig::default());
        assert!(session.documents.is_empty());
        assert!(session.mappings.is_empty());
        assert!(session.workspace_roots.is_empty());
        assert!(session.diagnostic_snapshots.is_empty());
        assert!(session.semantic_snapshots.is_empty());
        assert!(!session.shutdown_requested);
    }
}

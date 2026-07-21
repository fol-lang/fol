use crate::{
    ids::{LoweredGlobalId, LoweredPackageId, LoweredRoutineId, LoweredTypeId},
    types::LoweredTypeTable,
};
use fol_parser::ast::StandardKind;
use fol_parser::ast::SyntaxOrigin;
use fol_resolver::{MountedSymbolProvenance, PackageIdentity, SourceUnitId, SymbolId};
use fol_typecheck::CheckedTypeId;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoweredSourceSymbol {
    Package(LoweredPackageId),
    Global(LoweredGlobalId),
    Routine(LoweredRoutineId),
    Type(LoweredTypeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceMapEntry {
    pub symbol: LoweredSourceSymbol,
    pub origin: SyntaxOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredEntryCandidate {
    pub package_identity: PackageIdentity,
    pub routine_id: LoweredRoutineId,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredRecoverableAbi {
    TaggedResultObject {
        tag_type: LoweredTypeId,
        success_tag: String,
        error_tag: String,
        success_slot: String,
        error_slot: String,
    },
}

impl LoweredRecoverableAbi {
    pub fn v1(tag_type: LoweredTypeId) -> Self {
        Self::TaggedResultObject {
            tag_type,
            success_tag: "ok".to_string(),
            error_tag: "err".to_string(),
            success_slot: "value".to_string(),
            error_slot: "error".to_string(),
        }
    }

    pub fn success_runtime_meaning(&self) -> String {
        match self {
            Self::TaggedResultObject {
                success_tag,
                success_slot,
                ..
            } => format!(
                "success => tag='{success_tag}' and the recoverable payload lives in slot '{success_slot}'"
            ),
        }
    }

    pub fn failure_runtime_meaning(&self) -> String {
        match self {
            Self::TaggedResultObject {
                error_tag,
                error_slot,
                ..
            } => format!(
                "failure => tag='{error_tag}' and the reported payload lives in slot '{error_slot}'"
            ),
        }
    }

    pub fn propagation_runtime_meaning(&self) -> String {
        match self {
            Self::TaggedResultObject {
                error_tag,
                error_slot,
                ..
            } => format!(
                "propagation => forward tag='{error_tag}' and slot '{error_slot}' into the caller result object"
            ),
        }
    }

    pub fn panic_runtime_meaning(&self) -> &'static str {
        "panic => abort control flow without constructing a recoverable result object"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredExportMount {
    pub source_namespace: String,
    pub mounted_namespace_suffix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredFieldLayout {
    pub name: String,
    pub type_id: LoweredTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredVariantLayout {
    pub name: String,
    pub payload_type: Option<LoweredTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredTypeDeclKind {
    Alias { target_type: LoweredTypeId },
    Record { fields: Vec<LoweredFieldLayout> },
    Entry { variants: Vec<LoweredVariantLayout> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredTypeDecl {
    pub symbol_id: SymbolId,
    pub source_unit_id: SourceUnitId,
    pub name: String,
    pub runtime_type: LoweredTypeId,
    pub kind: LoweredTypeDeclKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredStandardRoutine {
    pub symbol_id: SymbolId,
    pub name: String,
    pub params: Vec<LoweredTypeId>,
    pub return_type: Option<LoweredTypeId>,
    pub error_type: Option<LoweredTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredStandard {
    pub symbol_id: SymbolId,
    pub name: String,
    pub source_unit_id: SourceUnitId,
    pub scope_id: fol_resolver::ScopeId,
    pub kind: StandardKind,
    pub required_routines: Vec<LoweredStandardRoutine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredConformance {
    pub type_symbol_id: SymbolId,
    pub standard_symbol_ids: Vec<SymbolId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredGlobal {
    pub id: LoweredGlobalId,
    pub symbol_id: SymbolId,
    pub source_unit_id: SourceUnitId,
    pub name: String,
    pub type_id: LoweredTypeId,
    pub mutable: bool,
    /// The declared literal initializer. Global reads materialize through it,
    /// so a `con LIMIT: int = 9` reads 9 rather than the type default.
    pub initializer: Option<LoweredGlobalInit>,
}

/// A global's compile-time initializer: a scalar literal, or a record literal
/// whose fields are scalar literals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweredGlobalInit {
    Operand(crate::control::LoweredOperand),
    Record {
        fields: Vec<(String, crate::control::LoweredOperand)>,
    },
    /// Initialized from another global constant (possibly cross-package).
    /// Rendering chains the referenced global's lazy load; constant cycles
    /// are pathological and deadlock the OnceLock at startup.
    GlobalRef {
        package: fol_resolver::PackageIdentity,
        symbol: SymbolId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSourceUnit {
    pub source_unit_id: SourceUnitId,
    pub path: String,
    pub package: String,
    pub namespace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSymbolOwnership {
    pub symbol_id: SymbolId,
    pub source_unit_id: SourceUnitId,
    pub owning_package: PackageIdentity,
    pub mounted_from: Option<MountedSymbolProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoweredSourceMap {
    entries: Vec<LoweredSourceMapEntry>,
}

impl LoweredSourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self) -> &[LoweredSourceMapEntry] {
        &self.entries
    }

    pub fn push(&mut self, entry: LoweredSourceMapEntry) {
        self.entries.push(entry);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPackage {
    pub id: LoweredPackageId,
    pub identity: PackageIdentity,
    pub globals: Vec<LoweredGlobalId>,
    pub routines: Vec<LoweredRoutineId>,
    pub types: Vec<LoweredTypeId>,
    pub exports: Vec<LoweredExportMount>,
    pub source_units: Vec<LoweredSourceUnit>,
    pub symbol_ownership: BTreeMap<SymbolId, LoweredSymbolOwnership>,
    pub checked_type_map: BTreeMap<CheckedTypeId, LoweredTypeId>,
    pub routine_signatures: BTreeMap<SymbolId, LoweredTypeId>,
    pub standards: BTreeMap<SymbolId, LoweredStandard>,
    pub conformances: BTreeMap<SymbolId, LoweredConformance>,
    pub type_decls: BTreeMap<SymbolId, LoweredTypeDecl>,
    pub global_decls: BTreeMap<LoweredGlobalId, LoweredGlobal>,
    pub routine_decls: BTreeMap<LoweredRoutineId, crate::LoweredRoutine>,
}

impl LoweredPackage {
    pub fn new(id: LoweredPackageId, identity: PackageIdentity) -> Self {
        Self {
            id,
            identity,
            globals: Vec::new(),
            routines: Vec::new(),
            types: Vec::new(),
            exports: Vec::new(),
            source_units: Vec::new(),
            symbol_ownership: BTreeMap::new(),
            checked_type_map: BTreeMap::new(),
            routine_signatures: BTreeMap::new(),
            standards: BTreeMap::new(),
            conformances: BTreeMap::new(),
            type_decls: BTreeMap::new(),
            global_decls: BTreeMap::new(),
            routine_decls: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredWorkspace {
    entry_identity: PackageIdentity,
    packages: BTreeMap<PackageIdentity, LoweredPackage>,
    entry_candidates: Vec<LoweredEntryCandidate>,
    type_table: LoweredTypeTable,
    source_map: LoweredSourceMap,
    recoverable_abi: LoweredRecoverableAbi,
}

impl LoweredWorkspace {
    pub fn new(
        entry_identity: PackageIdentity,
        packages: BTreeMap<PackageIdentity, LoweredPackage>,
        entry_candidates: Vec<LoweredEntryCandidate>,
        type_table: LoweredTypeTable,
        source_map: LoweredSourceMap,
        recoverable_abi: LoweredRecoverableAbi,
    ) -> Self {
        Self {
            entry_identity,
            packages,
            entry_candidates,
            type_table,
            source_map,
            recoverable_abi,
        }
    }

    pub fn entry_identity(&self) -> &PackageIdentity {
        &self.entry_identity
    }

    pub fn entry_package(&self) -> &LoweredPackage {
        self.packages
            .get(&self.entry_identity)
            .expect("lowered workspace should retain its entry package")
    }

    pub fn package(&self, identity: &PackageIdentity) -> Option<&LoweredPackage> {
        self.packages.get(identity)
    }

    pub fn packages(&self) -> impl Iterator<Item = &LoweredPackage> {
        self.packages.values()
    }

    pub fn package_count(&self) -> usize {
        self.packages.len()
    }

    pub fn entry_candidates(&self) -> &[LoweredEntryCandidate] {
        &self.entry_candidates
    }

    pub fn with_entry_candidates(&self, entry_candidates: Vec<LoweredEntryCandidate>) -> Self {
        Self {
            entry_identity: self.entry_identity.clone(),
            packages: self.packages.clone(),
            entry_candidates,
            type_table: self.type_table.clone(),
            source_map: self.source_map.clone(),
            recoverable_abi: self.recoverable_abi.clone(),
        }
    }

    pub fn type_table(&self) -> &LoweredTypeTable {
        &self.type_table
    }

    pub fn source_map(&self) -> &LoweredSourceMap {
        &self.source_map
    }

    pub fn recoverable_abi(&self) -> &LoweredRecoverableAbi {
        &self.recoverable_abi
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LoweredEntryCandidate, LoweredPackage, LoweredRecoverableAbi, LoweredSourceMap,
        LoweredSourceUnit, LoweredWorkspace,
    };
    use crate::ids::{LoweredPackageId, LoweredRoutineId};
    use crate::types::{LoweredBuiltinType, LoweredTypeTable};
    use fol_resolver::{PackageIdentity, PackageSourceKind, SourceUnitId};
    use std::collections::BTreeMap;

    fn identity(name: &str, kind: PackageSourceKind) -> PackageIdentity {
        PackageIdentity {
            source_kind: kind,
            canonical_root: format!("/workspace/{name}"),
            display_name: name.to_string(),
        }
    }

    #[test]
    fn lowered_workspace_shell_keeps_entry_package_and_count() {
        let entry_identity = identity("app", PackageSourceKind::Entry);
        let shared_identity = identity("shared", PackageSourceKind::Local);

        let mut entry_package = LoweredPackage::new(LoweredPackageId(0), entry_identity.clone());
        entry_package.source_units.push(LoweredSourceUnit {
            source_unit_id: SourceUnitId(0),
            path: "app/main.fol".to_string(),
            package: "app".to_string(),
            namespace: "app".to_string(),
        });
        let mut packages = BTreeMap::new();
        packages.insert(entry_identity.clone(), entry_package);
        packages.insert(
            shared_identity.clone(),
            LoweredPackage::new(LoweredPackageId(1), shared_identity),
        );

        let mut type_table = LoweredTypeTable::new();
        let recoverable_abi =
            LoweredRecoverableAbi::v1(type_table.intern_builtin(LoweredBuiltinType::Bool));
        let workspace = LoweredWorkspace::new(
            entry_identity.clone(),
            packages,
            vec![LoweredEntryCandidate {
                package_identity: entry_identity.clone(),
                routine_id: LoweredRoutineId(0),
                name: "main".to_string(),
            }],
            type_table,
            LoweredSourceMap::default(),
            recoverable_abi,
        );

        assert_eq!(workspace.entry_identity(), &entry_identity);
        assert_eq!(workspace.entry_package().id, LoweredPackageId(0));
        assert_eq!(workspace.package_count(), 2);
        assert_eq!(workspace.entry_package().source_units.len(), 1);
        assert_eq!(workspace.entry_candidates().len(), 1);
        assert!(matches!(
            workspace.recoverable_abi(),
            LoweredRecoverableAbi::TaggedResultObject {
                success_tag,
                error_tag,
                success_slot,
                error_slot,
                ..
            } if success_tag == "ok"
                && error_tag == "err"
                && success_slot == "value"
                && error_slot == "error"
        ));
        assert_eq!(
            workspace.recoverable_abi().success_runtime_meaning(),
            "success => tag='ok' and the recoverable payload lives in slot 'value'"
        );
        assert_eq!(
            workspace.recoverable_abi().failure_runtime_meaning(),
            "failure => tag='err' and the reported payload lives in slot 'error'"
        );
    }

    #[test]
    fn lowered_source_map_shell_starts_empty() {
        let source_map = LoweredSourceMap::new();

        assert!(source_map.is_empty());
        assert!(source_map.entries().is_empty());
    }
}

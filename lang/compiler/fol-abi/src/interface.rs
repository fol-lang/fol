//! Foreign interfaces: what FOL exports to C and imports from it.
//!
//! A `ForeignInterfaceTemplate` is what the compiler builds while lowering,
//! before a target is chosen. A `ForeignInterface` is the same thing resolved
//! against one target. `ResolvedAbiSurface` is the whole public C surface of
//! one artifact, which is what the header, the manifest, and the symbol
//! allowlist are all generated from.

use crate::types::{AbiTypeId, AbiTypeTable};

/// The C calling convention a symbol uses.
///
/// One variant today. It exists so a future convention is a new variant rather
/// than a new field everywhere, and so a manifest records the convention
/// explicitly instead of leaving it implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum AbiCallingConvention {
    #[default]
    C,
}

impl AbiCallingConvention {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::C => "C",
        }
    }
}

/// Which way a value crosses the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiDirection {
    In,
    Out,
    InOut,
}

impl AbiDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
            Self::InOut => "inout",
        }
    }
}

/// One parameter of a foreign routine.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AbiParameter {
    pub name: String,
    pub type_id: AbiTypeId,
    pub direction: AbiDirection,
}

/// Whether a routine can report a recoverable error.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiErrorContract {
    /// The wrapper returns `FOL_STATUS_OK` or a panic/validation status only.
    Infallible,
    /// The wrapper may return `FOL_STATUS_REPORT` with this error out value.
    Recoverable { error_type: AbiTypeId },
}

/// Which direction a declaration faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiFacing {
    /// FOL implements it; C calls it.
    Export,
    /// C implements it; FOL calls it.
    Import,
}

/// Where a declaration came from, for diagnostics and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct AbiSourceOrigin {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// One routine crossing the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ForeignRoutine {
    /// Fully qualified FOL routine, e.g. `api::add`.
    pub fol_path: String,
    /// The exact external C symbol. Never mangled, never inferred.
    pub symbol: String,
    pub facing: AbiFacing,
    pub convention: AbiCallingConvention,
    pub parameters: Vec<AbiParameter>,
    /// The success value, or `Void`.
    pub result: AbiTypeId,
    pub error: AbiErrorContract,
    /// Visibility and ABI selection, kept separate per section 4.10.
    pub selection: ExportSelection,
    /// What the routine is permitted to do, checked against the artifact's
    /// capability model.
    pub effects: AbiEffects,
    /// The handle domain this routine produces, borrows, or consumes.
    ///
    /// C sees an address either way, so the role is the only thing that says
    /// whether the wrapper should hand out a box, lend what one points at, or
    /// take it back and release it.
    pub handle: Option<crate::annotation::HandleUse>,
    pub origin: AbiSourceOrigin,
}

/// Everything the compiler knows about a foreign surface, before a target is
/// chosen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForeignInterfaceTemplate {
    pub types: AbiTypeTable,
    pub routines: Vec<ForeignRoutine>,
}

impl ForeignInterfaceTemplate {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_routine(&mut self, routine: ForeignRoutine) {
        self.routines.push(routine);
    }

    /// Resolve against one target.
    ///
    /// Separate from the template because the same FOL source produces a
    /// different interface per target, and a manifest describes exactly one.
    pub fn resolve(self, target: fol_types::ResolvedTarget) -> ForeignInterface {
        ForeignInterface {
            target,
            types: self.types,
            routines: self.routines,
        }
    }
}

/// A foreign surface resolved against one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignInterface {
    pub target: fol_types::ResolvedTarget,
    pub types: AbiTypeTable,
    pub routines: Vec<ForeignRoutine>,
}

impl ForeignInterface {
    /// Routines facing one direction, in declaration order.
    pub fn facing(&self, facing: AbiFacing) -> impl Iterator<Item = &ForeignRoutine> {
        self.routines
            .iter()
            .filter(move |routine| routine.facing == facing)
    }
}

/// The complete public C surface of one artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAbiSurface {
    pub artifact: String,
    pub major: u32,
    pub minor: u32,
    pub interface: ForeignInterface,
}

impl ResolvedAbiSurface {
    /// Every exported symbol, sorted, for the linker allowlist.
    ///
    /// Sorted because the allowlist is compared and written to a file;
    /// declaration order would make two identical surfaces differ.
    pub fn exported_symbols(&self) -> Vec<&str> {
        let mut symbols: Vec<&str> = self
            .interface
            .facing(AbiFacing::Export)
            .map(|routine| routine.symbol.as_str())
            .collect();
        symbols.sort_unstable();
        symbols
    }
}

/// Whether a declaration is visible outside its package, and separately whether
/// it was selected for the C ABI.
///
/// Section 4.10: `[exp]` is necessary for a declaration to be *selectable* and
/// never sufficient to export a native symbol. Keeping the two on one value
/// makes the distinction impossible to lose -- a package-public routine is not
/// an ABI export until an allowlist entry names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExportSelection {
    /// `[exp]`: visible to other FOL packages.
    pub package_visible: bool,
    /// Named by the artifact's ABI export allowlist.
    pub abi_selected: bool,
}

impl ExportSelection {
    /// Visible to FOL, not exported to C. The common case.
    pub const PACKAGE_ONLY: Self = Self {
        package_visible: true,
        abi_selected: false,
    };

    /// Whether a native symbol should be emitted.
    ///
    /// Both halves are required: selecting a package-private routine would
    /// export something the package itself does not consider part of its
    /// surface.
    pub const fn emits_native_symbol(self) -> bool {
        self.package_visible && self.abi_selected
    }
}

/// Effects a foreign declaration is permitted to have.
///
/// Recorded on the routine because the artifact's capability model constrains
/// them: a `core` artifact cannot export something that allocates, and the
/// classifier reports that as `CapabilityTooStrong` rather than letting it fail
/// at link time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct AbiEffects {
    pub allocates: bool,
    pub may_panic: bool,
    pub reports_error: bool,
}

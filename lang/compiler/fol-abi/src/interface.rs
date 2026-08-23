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

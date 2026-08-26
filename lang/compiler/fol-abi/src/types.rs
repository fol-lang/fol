//! The canonical ABI type vocabulary.
//!
//! This is the one model both the compiler and the interop stack agree on. A
//! type reaches here only after it has been proven projectable; anything the
//! classifier rejects never becomes an `AbiType`, so a consumer of this table
//! never has to ask whether a shape is legal.
//!
//! Section 4.6 of `plan/V4_PLAN.md` is the normative matrix.

use std::collections::BTreeMap;

/// A type in one `AbiTypeTable`.
///
/// Interning gives aggregates identity without recursion in the type itself,
/// which is what lets a manifest serialize a type graph as a flat table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AbiTypeId(pub usize);

impl std::fmt::Display for AbiTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "abi:{}", self.0)
    }
}

/// How a scalar crosses the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiScalar {
    /// A sized integer. `arch`/`uarch` never reach here: they are
    /// target-dependent by construction, which a stable ABI cannot be.
    Int(fol_types::IntWidth),
    Float(fol_types::FloatWidth),
    /// `uint8_t`, valid only as 0 or 1.
    Bool,
    /// `uint32_t` holding a Unicode scalar value.
    Char,
}

impl AbiScalar {
    /// The C spelling, per the section 4.6 matrix.
    pub fn c_type(self) -> String {
        match self {
            Self::Int(width) => {
                let bits = width.bits().expect("arch widths never reach the ABI");
                if width.is_signed() {
                    format!("int{bits}_t")
                } else {
                    format!("uint{bits}_t")
                }
            }
            Self::Float(fol_types::FloatWidth::F32) => "float".to_string(),
            Self::Float(_) => "double".to_string(),
            Self::Bool => "fol_bool_t".to_string(),
            Self::Char => "fol_char_t".to_string(),
        }
    }
}

/// Whether a raw pointer may be written through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiMutability {
    Const,
    Mutable,
}

/// Whether a pointer may be null.
///
/// Section 4.8 makes optional wrapping the nullability marker: `ptr[raw, T]` is
/// non-null and `opt ptr[raw, T]` is nullable. The distinction is carried here
/// rather than left to a convention, because a C caller cannot see a comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiNullability {
    NonNull,
    Nullable,
}

/// Who releases a pointed-to resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiOwnership {
    /// The callee does not own it and must not release it.
    Borrowed,
    /// Ownership transfers; the receiver releases it through the paired
    /// destructor.
    Transferred,
}

/// Whether a pointer may outlive the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiEscape {
    /// Valid only for the duration of the call.
    CallScoped,
    /// May be retained, with the ownership rule saying who releases it.
    Retained,
}

/// One field of a POD record.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AbiField {
    pub name: String,
    pub type_id: AbiTypeId,
}

/// One variant of an entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AbiVariant {
    pub name: String,
    /// The stable discriminant. Explicit rather than positional, so inserting a
    /// variant cannot silently renumber the ones after it.
    pub discriminant: i64,
    /// The payload, or `None` for a tag-only variant.
    pub payload: Option<AbiTypeId>,
}

/// A canonical ABI type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AbiType {
    Scalar(AbiScalar),
    /// No value. Only valid as a return, where the wrapper still returns a
    /// status.
    Void,
    /// A raw address token.
    Pointer {
        target: AbiTypeId,
        mutability: AbiMutability,
        nullability: AbiNullability,
        ownership: AbiOwnership,
        escape: AbiEscape,
        /// The paired destroy symbol, when ownership transfers.
        destructor: Option<String>,
    },
    /// A named POD record. Fields are in **source declaration order**, which
    /// decides every offset.
    Record {
        name: String,
        fields: Vec<AbiField>,
    },
    /// A named entry: an explicit tag plus a payload union.
    Entry {
        name: String,
        /// Fixed tag width, so the discriminant's size is not a guess.
        tag: fol_types::IntWidth,
        variants: Vec<AbiVariant>,
    },
    /// A borrowed UTF-8 string: `{const uint8_t *ptr; size_t len;}`.
    BorrowedString,
    /// A NUL-terminated C string: `const char *`.
    ///
    /// Distinct from `BorrowedString`, which is a pointer *and* a length. This
    /// one carries its extent in its own bytes, which is what a C API taking a
    /// filename or a format expects, and what nothing in the type says: C has
    /// no way to distinguish text from any other `char *`, so the overlay
    /// declares it.
    CString {
        mutability: AbiMutability,
    },
    /// A borrowed slice of `element`.
    BorrowedSlice {
        element: AbiTypeId,
        mutability: AbiMutability,
    },
    /// An opaque handle a consumer may only pass back.
    OpaqueHandle {
        name: String,
    },
    /// A synchronous callback the provider invokes during one call.
    ///
    /// The C shape is a function pointer plus a separate `void *` context, and
    /// the function pointer's own first parameter is that context handed back.
    /// Neither of those is a FOL-visible parameter: FOL supplies the context
    /// itself, so `parameters` here is what a FOL routine value receives.
    Callback {
        parameters: Vec<AbiTypeId>,
        result: AbiTypeId,
        /// Whether the provider hands the context back as the pointer's own
        /// first argument.
        ///
        /// `false` is `qsort`'s comparator and `lua_CFunction`: no context at
        /// all. The closure is still recovered from a thread-local slot, so
        /// what is lost is one liveness signal, not the mechanism.
        context: bool,
    },
}

impl AbiType {
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Scalar(_) => "scalar",
            Self::Void => "void",
            Self::Pointer { .. } => "pointer",
            Self::Record { .. } => "record",
            Self::Entry { .. } => "entry",
            Self::BorrowedString => "borrowed-string",
            Self::CString { .. } => "c-string",
            Self::BorrowedSlice { .. } => "borrowed-slice",
            Self::OpaqueHandle { .. } => "opaque-handle",
            Self::Callback { .. } => "callback",
        }
    }

    /// The declared name, for the shapes that have one.
    pub fn declared_name(&self) -> Option<&str> {
        match self {
            Self::Record { name, .. } | Self::Entry { name, .. } | Self::OpaqueHandle { name } => {
                Some(name)
            }
            _ => None,
        }
    }
}

/// A flat, interned table of ABI types.
///
/// Flat because a manifest has to serialize the whole type graph
/// deterministically, and a nested representation would have no stable order
/// for shared sub-types.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbiTypeTable {
    types: Vec<AbiType>,
    index: BTreeMap<AbiType, AbiTypeId>,
}

impl AbiTypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intern a type, returning the existing id when it is already present.
    pub fn intern(&mut self, ty: AbiType) -> AbiTypeId {
        if let Some(existing) = self.index.get(&ty) {
            return *existing;
        }
        let id = AbiTypeId(self.types.len());
        self.types.push(ty.clone());
        self.index.insert(ty, id);
        id
    }

    pub fn get(&self, id: AbiTypeId) -> Option<&AbiType> {
        self.types.get(id.0)
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (AbiTypeId, &AbiType)> {
        self.types
            .iter()
            .enumerate()
            .map(|(index, ty)| (AbiTypeId(index), ty))
    }

    /// Convenience for the common scalar cases.
    pub fn intern_int(&mut self, width: fol_types::IntWidth) -> AbiTypeId {
        self.intern(AbiType::Scalar(AbiScalar::Int(width)))
    }
}

use crate::ids::LoweredTypeId;
use fol_resolver::{PackageIdentity, SymbolId};
use fol_types::{FloatWidth, IntWidth};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoweredBuiltinType {
    /// Width and sign survive lowering unchanged; codegen needs them to emit
    /// the right Rust primitive (`plan/V4_SCALAR_WIDTHS.md`).
    Int(IntWidth),
    Float(FloatWidth),
    Bool,
    Char(fol_types::CharEncoding),
    Str,
    Never,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoweredRoutineType {
    pub params: Vec<LoweredTypeId>,
    pub return_type: Option<LoweredTypeId>,
    pub error_type: Option<LoweredTypeId>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoweredType {
    Builtin(LoweredBuiltinType),
    GenericParameter {
        name: String,
    },
    Named {
        package: PackageIdentity,
        symbol: SymbolId,
        name: String,
    },
    Owned {
        inner: LoweredTypeId,
    },
    Borrowed {
        inner: LoweredTypeId,
        mutable: bool,
    },
    Pointer {
        target: LoweredTypeId,
        shared: bool,
        /// A `ptr[weak, T]` weak handle, rendered as `std::rc::Weak<T>`.
        weak: bool,
        /// A `ptr[shared, sync, T]` uses `std::sync::Arc` (thread-safe) instead
        /// of `std::rc::Rc`, so it may cross task boundaries (V3_MEM §8.3).
        sync: bool,
        /// A foreign address token rather than a managed pointer.
        raw: bool,
        /// Whether the pointee may be written through.
        mutable: bool,
    },
    Array {
        element_type: LoweredTypeId,
        size: Option<usize>,
    },
    Vector {
        element_type: LoweredTypeId,
    },
    Sequence {
        element_type: LoweredTypeId,
    },
    Channel {
        element_type: LoweredTypeId,
    },
    ChannelSender {
        element_type: LoweredTypeId,
    },
    ChannelReceiver {
        element_type: LoweredTypeId,
    },
    Eventual {
        value_type: LoweredTypeId,
        error_type: Option<LoweredTypeId>,
    },
    Set {
        member_types: Vec<LoweredTypeId>,
    },
    Map {
        key_type: LoweredTypeId,
        value_type: LoweredTypeId,
    },
    Optional {
        inner: LoweredTypeId,
    },
    Error {
        inner: Option<LoweredTypeId>,
    },
    Record {
        fields: BTreeMap<String, LoweredTypeId>,
        /// True when the record's type claims `fin` custom finalization. This is
        /// part of the interning key so a `fin` record is a distinct move-only
        /// type from a structurally identical non-`fin` record.
        finalized: bool,
        /// The declaring type's qualified name, when the record came from a
        /// named declaration.
        ///
        /// Records are otherwise interned structurally, which collapsed two
        /// declared types with the same field list into one id -- and method
        /// dispatch, which matches a conformer by its receiver's lowered type,
        /// then called whichever conformer it found first. `typ Alpha: rec =
        /// { w: int }` and `typ Beta: rec = { w: int }` are different types, so
        /// their identity has to survive lowering.
        nominal: Option<String>,
    },
    Entry {
        variants: BTreeMap<String, Option<LoweredTypeId>>,
    },
    Routine(LoweredRoutineType),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoweredTypeTable {
    types: Vec<LoweredType>,
    canonical_ids: BTreeMap<LoweredType, LoweredTypeId>,
    /// Declared names for `ent` types, purely so `.type_name(x)` can answer
    /// `Shade` instead of `ent { DARK: int, LIGHT: int }`.
    ///
    /// A side table rather than a `nominal` field on the variant, because that
    /// field is part of the interning key: adding one would make two
    /// structurally identical `ent` declarations distinct types and change
    /// dispatch. This is display metadata and changes nothing about identity —
    /// with the matching limitation that two such declarations share an id, so
    /// the first name recorded is what both report.
    declared_entry_names: BTreeMap<LoweredTypeId, String>,
}

impl LoweredTypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    pub fn get(&self, id: LoweredTypeId) -> Option<&LoweredType> {
        self.types.get(id.0)
    }

    /// Records the source name of a declared `ent` type. First name wins, so
    /// the result does not depend on lowering order.
    pub fn note_declared_entry_name(&mut self, type_id: LoweredTypeId, name: impl Into<String>) {
        self.declared_entry_names
            .entry(type_id)
            .or_insert_with(|| name.into());
    }

    /// The FOL surface spelling of a lowered type, for `.type_name(x)`.
    ///
    /// This mirrors the checked table's renderer used by diagnostics, with one
    /// deliberate difference: an array renders as `arr[int, 8]` rather than
    /// `[int]`, because `arr[T, N]` is how FOL actually spells the type and
    /// this string is read by programs, not only by people.
    ///
    /// A structural record with no declared name renders its fields. Nominal
    /// records short-circuit to their name, which is also what stops a
    /// self-referential type from recursing forever; `depth` is the backstop
    /// for any structural cycle that slips past that.
    pub fn render_type(&self, type_id: LoweredTypeId) -> String {
        self.render_type_at(type_id, 0)
    }

    fn render_type_at(&self, type_id: LoweredTypeId, depth: usize) -> String {
        if depth > 16 {
            return "...".to_string();
        }
        let nested = |inner: LoweredTypeId| self.render_type_at(inner, depth + 1);
        let join = |items: &[LoweredTypeId]| {
            items
                .iter()
                .map(|item| self.render_type_at(*item, depth + 1))
                .collect::<Vec<_>>()
                .join(", ")
        };
        match self.get(type_id) {
            Some(LoweredType::Builtin(builtin)) => match builtin {
                LoweredBuiltinType::Int(width) if *width == IntWidth::DEFAULT => "int".to_string(),
                LoweredBuiltinType::Int(width) => width.as_str().to_string(),
                LoweredBuiltinType::Float(width) if *width == FloatWidth::DEFAULT => {
                    "flt".to_string()
                }
                LoweredBuiltinType::Float(width) => width.as_str().to_string(),
                LoweredBuiltinType::Bool => "bol".to_string(),
                LoweredBuiltinType::Char(encoding) => encoding.fol_spelling().to_string(),
                LoweredBuiltinType::Str => "str".to_string(),
                LoweredBuiltinType::Never => "never".to_string(),
            },
            Some(LoweredType::GenericParameter { name })
            | Some(LoweredType::Named { name, .. }) => name.clone(),
            Some(LoweredType::Owned { inner }) => format!("@{}", nested(*inner)),
            Some(LoweredType::Borrowed { inner, mutable }) => {
                if *mutable {
                    format!("bor[mut, {}]", nested(*inner))
                } else {
                    format!("bor[{}]", nested(*inner))
                }
            }
            Some(LoweredType::Pointer {
                target,
                shared,
                weak,
                sync,
                ..
            }) => {
                if *weak {
                    format!("ptr[weak, {}]", nested(*target))
                } else if *shared && *sync {
                    format!("ptr[shared, sync, {}]", nested(*target))
                } else if *shared {
                    format!("ptr[shared, {}]", nested(*target))
                } else {
                    format!("ptr[{}]", nested(*target))
                }
            }
            Some(LoweredType::Array { element_type, size }) => match size {
                Some(size) => format!("arr[{}, {size}]", nested(*element_type)),
                None => format!("arr[{}]", nested(*element_type)),
            },
            Some(LoweredType::Vector { element_type }) => format!("vec[{}]", nested(*element_type)),
            Some(LoweredType::Sequence { element_type }) => {
                format!("seq[{}]", nested(*element_type))
            }
            Some(LoweredType::Channel { element_type }) => {
                format!("chn[{}]", nested(*element_type))
            }
            Some(LoweredType::ChannelSender { element_type }) => {
                format!("chn[tx, {}]", nested(*element_type))
            }
            Some(LoweredType::ChannelReceiver { element_type }) => {
                format!("chn[rx, {}]", nested(*element_type))
            }
            Some(LoweredType::Eventual {
                value_type,
                error_type,
            }) => match error_type {
                Some(error_type) => {
                    format!("evt[{} / {}]", nested(*value_type), nested(*error_type))
                }
                None => format!("evt[{}]", nested(*value_type)),
            },
            Some(LoweredType::Set { member_types }) => format!("set[{}]", join(member_types)),
            Some(LoweredType::Map {
                key_type,
                value_type,
            }) => format!("map[{}, {}]", nested(*key_type), nested(*value_type)),
            Some(LoweredType::Optional { inner }) => format!("opt[{}]", nested(*inner)),
            Some(LoweredType::Error { inner }) => match inner {
                Some(inner) => format!("err[{}]", nested(*inner)),
                None => "err[]".to_string(),
            },
            Some(LoweredType::Record {
                fields, nominal, ..
            }) => match nominal {
                // `nominal` is package-qualified, and a package identity can be
                // an absolute build path — which must never reach a
                // user-visible string. The declared name is what the source
                // wrote and what `Point` means to the reader.
                Some(name) => name.rsplit("::").next().unwrap_or(name).to_string(),
                None => {
                    let rendered = fields
                        .iter()
                        .map(|(name, field_type)| {
                            format!("{name}: {}", self.render_type_at(*field_type, depth + 1))
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("rec {{ {rendered} }}")
                }
            },
            Some(LoweredType::Entry { variants }) => {
                if let Some(name) = self.declared_entry_names.get(&type_id) {
                    return name.clone();
                }
                let rendered = variants
                    .iter()
                    .map(|(name, payload)| match payload {
                        Some(payload) => {
                            format!("{name}: {}", self.render_type_at(*payload, depth + 1))
                        }
                        None => name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("ent {{ {rendered} }}")
            }
            Some(LoweredType::Routine(routine)) => {
                let params = join(&routine.params);
                let returns = routine
                    .return_type
                    .map(|value| self.render_type_at(value, depth + 1))
                    .unwrap_or_else(|| "non".to_string());
                match routine.error_type {
                    Some(error_type) => format!(
                        "fun({params}): {returns} / {}",
                        self.render_type_at(error_type, depth + 1)
                    ),
                    None => format!("fun({params}): {returns}"),
                }
            }
            None => "?".to_string(),
        }
    }

    pub fn find(&self, ty: &LoweredType) -> Option<LoweredTypeId> {
        self.canonical_ids.get(ty).copied()
    }

    pub fn intern(&mut self, ty: LoweredType) -> LoweredTypeId {
        if let Some(id) = self.canonical_ids.get(&ty) {
            return *id;
        }

        let id = LoweredTypeId(self.types.len());
        self.types.push(ty.clone());
        self.canonical_ids.insert(ty, id);
        id
    }

    pub fn intern_builtin(&mut self, builtin: LoweredBuiltinType) -> LoweredTypeId {
        self.intern(LoweredType::Builtin(builtin))
    }

    /// Whether transferring a value of `id` consumes its source. Uniqueness is
    /// transitive: an aggregate containing an owned value, unique pointer,
    /// eventual, or receiver endpoint must move as a whole rather than clone.
    pub fn moves_on_transfer(&self, id: LoweredTypeId) -> bool {
        fn moves(
            table: &LoweredTypeTable,
            id: LoweredTypeId,
            visiting: &mut BTreeSet<LoweredTypeId>,
        ) -> bool {
            if !visiting.insert(id) {
                return false;
            }
            let result = match table.get(id) {
                // A bare generic parameter has no copy-safety proof inside
                // the generic routine. Treat it as move-only there. Concrete
                // call-site locals still use their concrete lowered type, so
                // scalar callers clone while owned and unique-pointer callers
                // move across the boundary.
                Some(LoweredType::GenericParameter { .. })
                | Some(LoweredType::Owned { .. })
                | Some(LoweredType::Pointer {
                    shared: false,
                    weak: false,
                    ..
                })
                | Some(LoweredType::Eventual { .. })
                | Some(LoweredType::Channel { .. })
                // A `chn[rx, T]` receiver is unique: move-only, never cloned.
                | Some(LoweredType::ChannelReceiver { .. }) => true,
                Some(LoweredType::Array { element_type, .. })
                | Some(LoweredType::Vector { element_type })
                | Some(LoweredType::Sequence { element_type })
                | Some(LoweredType::Optional {
                    inner: element_type,
                }) => moves(table, *element_type, visiting),
                Some(LoweredType::Error { inner }) => {
                    inner.is_some_and(|inner| moves(table, inner, visiting))
                }
                Some(LoweredType::Set { member_types }) => member_types
                    .iter()
                    .any(|member| moves(table, *member, visiting)),
                Some(LoweredType::Map {
                    key_type,
                    value_type,
                }) => {
                    moves(table, *key_type, visiting)
                        || moves(table, *value_type, visiting)
                }
                // A `fin` record is affine: it owns a finalizable resource and
                // must move (never copy) so finalization runs exactly once.
                Some(LoweredType::Record {
                    fields, finalized, ..
                }) => {
                    *finalized || fields.values().any(|field| moves(table, *field, visiting))
                }
                Some(LoweredType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| moves(table, *variant, visiting)),
                Some(LoweredType::Builtin(_))
                | Some(LoweredType::Named { .. })
                | Some(LoweredType::Borrowed { .. })
                | Some(LoweredType::Pointer { shared: true, .. })
                // A weak handle (`Weak<T>`) is clonable, not move-only.
                | Some(LoweredType::Pointer { weak: true, .. })
                | Some(LoweredType::ChannelSender { .. })
                | Some(LoweredType::Routine(_))
                | None => false,
            };
            visiting.remove(&id);
            result
        }

        moves(self, id, &mut BTreeSet::new())
    }

    /// Whether `id` stores a lexical borrow anywhere in its runtime shape.
    pub fn contains_borrowed(&self, id: LoweredTypeId) -> bool {
        self.contains_matching_type(id, |ty| matches!(ty, LoweredType::Borrowed { .. }))
    }

    /// Whether `id` stores an `Rc`-backed shared pointer anywhere in its
    /// runtime shape.
    pub fn contains_shared_pointer(&self, id: LoweredTypeId) -> bool {
        self.contains_matching_type(id, |ty| {
            matches!(ty, LoweredType::Pointer { shared: true, .. })
        })
    }

    fn contains_matching_type(
        &self,
        id: LoweredTypeId,
        predicate: fn(&LoweredType) -> bool,
    ) -> bool {
        fn contains(
            table: &LoweredTypeTable,
            id: LoweredTypeId,
            predicate: fn(&LoweredType) -> bool,
            visiting: &mut BTreeSet<LoweredTypeId>,
        ) -> bool {
            if !visiting.insert(id) {
                return false;
            }
            let result = match table.get(id) {
                Some(ty) if predicate(ty) => true,
                Some(LoweredType::Array { element_type, .. })
                | Some(LoweredType::Vector { element_type })
                | Some(LoweredType::Sequence { element_type })
                | Some(LoweredType::Channel { element_type })
                | Some(LoweredType::ChannelSender { element_type })
                | Some(LoweredType::ChannelReceiver { element_type })
                | Some(LoweredType::Owned {
                    inner: element_type,
                })
                | Some(LoweredType::Borrowed {
                    inner: element_type,
                    ..
                })
                | Some(LoweredType::Pointer {
                    target: element_type,
                    ..
                })
                | Some(LoweredType::Optional {
                    inner: element_type,
                }) => contains(table, *element_type, predicate, visiting),
                Some(LoweredType::Error { inner }) => {
                    inner.is_some_and(|inner| contains(table, inner, predicate, visiting))
                }
                Some(LoweredType::Eventual {
                    value_type,
                    error_type,
                }) => {
                    contains(table, *value_type, predicate, visiting)
                        || error_type
                            .is_some_and(|error| contains(table, error, predicate, visiting))
                }
                Some(LoweredType::Set { member_types }) => member_types
                    .iter()
                    .any(|member| contains(table, *member, predicate, visiting)),
                Some(LoweredType::Map {
                    key_type,
                    value_type,
                }) => {
                    contains(table, *key_type, predicate, visiting)
                        || contains(table, *value_type, predicate, visiting)
                }
                Some(LoweredType::Record { fields, .. }) => fields
                    .values()
                    .any(|field| contains(table, *field, predicate, visiting)),
                Some(LoweredType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| contains(table, *variant, predicate, visiting)),
                Some(LoweredType::Builtin(_))
                | Some(LoweredType::GenericParameter { .. })
                | Some(LoweredType::Named { .. })
                | Some(LoweredType::Routine(_))
                | None => false,
            };
            visiting.remove(&id);
            result
        }

        contains(self, id, predicate, &mut BTreeSet::new())
    }

    /// Whether `id` mentions an unbound generic parameter anywhere in its
    /// structure. Used both to select monomorphization templates and to detect
    /// generic parameters that leaked into concrete positions.
    pub(crate) fn contains_generic_parameter(&self, id: LoweredTypeId) -> bool {
        let Some(lowered_type) = self.get(id) else {
            return false;
        };
        match lowered_type {
            LoweredType::GenericParameter { .. } => true,
            LoweredType::Builtin(_) | LoweredType::Named { .. } => false,
            LoweredType::Array { element_type, .. }
            | LoweredType::Vector { element_type }
            | LoweredType::Sequence { element_type }
            | LoweredType::Channel { element_type }
            | LoweredType::ChannelSender { element_type }
            | LoweredType::ChannelReceiver { element_type }
            | LoweredType::Owned {
                inner: element_type,
            }
            | LoweredType::Borrowed {
                inner: element_type,
                ..
            }
            | LoweredType::Pointer {
                target: element_type,
                ..
            }
            | LoweredType::Optional {
                inner: element_type,
            } => self.contains_generic_parameter(*element_type),
            LoweredType::Error { inner } => {
                inner.is_some_and(|inner| self.contains_generic_parameter(inner))
            }
            LoweredType::Eventual {
                value_type,
                error_type,
            } => {
                self.contains_generic_parameter(*value_type)
                    || error_type
                        .is_some_and(|error_type| self.contains_generic_parameter(error_type))
            }
            LoweredType::Set { member_types } => member_types
                .iter()
                .any(|member_type| self.contains_generic_parameter(*member_type)),
            LoweredType::Map {
                key_type,
                value_type,
            } => {
                self.contains_generic_parameter(*key_type)
                    || self.contains_generic_parameter(*value_type)
            }
            LoweredType::Record { fields, .. } => fields
                .values()
                .any(|field_type| self.contains_generic_parameter(*field_type)),
            LoweredType::Entry { variants } => variants
                .values()
                .flatten()
                .any(|variant_type| self.contains_generic_parameter(*variant_type)),
            LoweredType::Routine(signature) => {
                signature
                    .params
                    .iter()
                    .any(|param| self.contains_generic_parameter(*param))
                    || signature
                        .return_type
                        .is_some_and(|return_type| self.contains_generic_parameter(return_type))
                    || signature
                        .error_type
                        .is_some_and(|error_type| self.contains_generic_parameter(error_type))
            }
        }
    }

    /// Whether `id` contains a record or entry shell that itself mentions a
    /// generic parameter (e.g. `Box[T]` lowered to `Record { value: T }`).
    ///
    /// Such a type needs a backend type declaration, but a declaration mentioning
    /// a generic parameter would require emitting a Rust generic struct, which
    /// the FOL-side monomorphization model forbids. A routine using one must be
    /// monomorphized so the structural type becomes concrete first. Bare generic
    /// parameters and runtime containers (`seq[T]`, `opt[T]`, ...) do not count:
    /// those ride the ordinary Rust-generics path.
    pub(crate) fn contains_generic_structural_type(&self, id: LoweredTypeId) -> bool {
        let Some(lowered_type) = self.get(id) else {
            return false;
        };
        match lowered_type {
            LoweredType::Record { .. } | LoweredType::Entry { .. } => {
                self.contains_generic_parameter(id)
            }
            LoweredType::Array { element_type, .. }
            | LoweredType::Vector { element_type }
            | LoweredType::Sequence { element_type }
            | LoweredType::Channel { element_type }
            | LoweredType::ChannelSender { element_type }
            | LoweredType::ChannelReceiver { element_type }
            | LoweredType::Owned {
                inner: element_type,
            }
            | LoweredType::Borrowed {
                inner: element_type,
                ..
            }
            | LoweredType::Pointer {
                target: element_type,
                ..
            }
            | LoweredType::Optional {
                inner: element_type,
            } => self.contains_generic_structural_type(*element_type),
            LoweredType::Error { inner } => {
                inner.is_some_and(|inner| self.contains_generic_structural_type(inner))
            }
            LoweredType::Eventual {
                value_type,
                error_type,
            } => {
                self.contains_generic_structural_type(*value_type)
                    || error_type
                        .is_some_and(|error_type| self.contains_generic_structural_type(error_type))
            }
            LoweredType::Set { member_types } => member_types
                .iter()
                .any(|member_type| self.contains_generic_structural_type(*member_type)),
            LoweredType::Map {
                key_type,
                value_type,
            } => {
                self.contains_generic_structural_type(*key_type)
                    || self.contains_generic_structural_type(*value_type)
            }
            LoweredType::Routine(signature) => {
                signature
                    .params
                    .iter()
                    .any(|param| self.contains_generic_structural_type(*param))
                    || signature.return_type.is_some_and(|return_type| {
                        self.contains_generic_structural_type(return_type)
                    })
                    || signature
                        .error_type
                        .is_some_and(|error_type| self.contains_generic_structural_type(error_type))
            }
            LoweredType::Builtin(_)
            | LoweredType::Named { .. }
            | LoweredType::GenericParameter { .. } => false,
        }
    }

    /// Collect the names of every generic parameter mentioned by `id`.
    pub(crate) fn collect_generic_parameter_names(
        &self,
        id: LoweredTypeId,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        let Some(lowered_type) = self.get(id) else {
            return;
        };
        match lowered_type {
            LoweredType::GenericParameter { name } => {
                out.insert(name.clone());
            }
            LoweredType::Builtin(_) | LoweredType::Named { .. } => {}
            LoweredType::Array { element_type, .. }
            | LoweredType::Vector { element_type }
            | LoweredType::Sequence { element_type }
            | LoweredType::Channel { element_type }
            | LoweredType::ChannelSender { element_type }
            | LoweredType::ChannelReceiver { element_type }
            | LoweredType::Owned {
                inner: element_type,
            }
            | LoweredType::Borrowed {
                inner: element_type,
                ..
            }
            | LoweredType::Pointer {
                target: element_type,
                ..
            }
            | LoweredType::Optional {
                inner: element_type,
            } => self.collect_generic_parameter_names(*element_type, out),
            LoweredType::Error { inner } => {
                if let Some(inner) = inner {
                    self.collect_generic_parameter_names(*inner, out);
                }
            }
            LoweredType::Eventual {
                value_type,
                error_type,
            } => {
                self.collect_generic_parameter_names(*value_type, out);
                if let Some(error_type) = error_type {
                    self.collect_generic_parameter_names(*error_type, out);
                }
            }
            LoweredType::Set { member_types } => {
                for member_type in member_types {
                    self.collect_generic_parameter_names(*member_type, out);
                }
            }
            LoweredType::Map {
                key_type,
                value_type,
            } => {
                self.collect_generic_parameter_names(*key_type, out);
                self.collect_generic_parameter_names(*value_type, out);
            }
            LoweredType::Record { fields, .. } => {
                for field_type in fields.values() {
                    self.collect_generic_parameter_names(*field_type, out);
                }
            }
            LoweredType::Entry { variants } => {
                for variant_type in variants.values().flatten() {
                    self.collect_generic_parameter_names(*variant_type, out);
                }
            }
            LoweredType::Routine(signature) => {
                for param in &signature.params {
                    self.collect_generic_parameter_names(*param, out);
                }
                if let Some(return_type) = signature.return_type {
                    self.collect_generic_parameter_names(return_type, out);
                }
                if let Some(error_type) = signature.error_type {
                    self.collect_generic_parameter_names(error_type, out);
                }
            }
        }
    }
}

/// The lowered id of a type symbol's own `Declared` node.
///
/// Records intern structurally, so a symbol's `declared_type` (the bare record)
/// is shared with any structural twin; only the `Declared` node carries which
/// declaration it is, and lowering stamps that name on. Every path that turns a
/// named type into a lowered id has to go through here, or one path resolves to
/// the nominal type and another to the structural one, and lookups between them
/// silently miss.
pub(crate) fn declared_node_lowered_type(
    program: &fol_typecheck::TypedProgram,
    checked_type_map: &std::collections::BTreeMap<fol_typecheck::CheckedTypeId, LoweredTypeId>,
    symbol_id: fol_resolver::SymbolId,
) -> Option<LoweredTypeId> {
    let table = program.type_table();
    (0..table.len()).find_map(|raw| {
        let checked_id = fol_typecheck::CheckedTypeId(raw);
        match table.get(checked_id) {
            Some(fol_typecheck::CheckedType::Declared { symbol, args, .. })
                if *symbol == symbol_id && args.is_empty() =>
            {
                checked_type_map.get(&checked_id).copied()
            }
            _ => None,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{LoweredBuiltinType, LoweredRoutineType, LoweredType, LoweredTypeTable};
    use crate::ids::LoweredTypeId;
    use std::collections::BTreeMap;

    #[test]
    fn lowered_type_table_interns_builtin_shapes_canonically() {
        let mut table = LoweredTypeTable::new();

        let first = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let second = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let third = table.intern_builtin(LoweredBuiltinType::Str);

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn lowered_type_table_canonicalizes_structural_shapes() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));

        let mut fields = BTreeMap::new();
        fields.insert("x".to_string(), int_id);
        fields.insert("y".to_string(), int_id);

        let record_first = table.intern(LoweredType::Record {
            fields: fields.clone(),
            finalized: false,
            nominal: None,
        });
        let record_second = table.intern(LoweredType::Record {
            fields,
            finalized: false,
            nominal: None,
        });
        let routine = table.intern(LoweredType::Routine(LoweredRoutineType {
            params: vec![record_first],
            return_type: Some(record_first),
            error_type: Some(LoweredTypeId(0)),
        }));

        assert_eq!(record_first, record_second);
        assert_ne!(record_first, routine);
        assert_eq!(
            table.get(record_first),
            Some(&LoweredType::Record {
                fields: BTreeMap::from([("x".to_string(), int_id), ("y".to_string(), int_id),]),
                finalized: false,
                nominal: None,
            })
        );
    }

    #[test]
    fn aggregate_transfer_is_move_only_when_a_field_is_unique() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let unique = table.intern(LoweredType::Pointer {
            target: int_id,
            shared: false,
            weak: false,
            sync: false,
            raw: false,
            mutable: false,
        });
        let shared = table.intern(LoweredType::Pointer {
            target: int_id,
            shared: true,
            weak: false,
            sync: false,
            raw: false,
            mutable: false,
        });
        let unique_record = table.intern(LoweredType::Record {
            fields: BTreeMap::from([("value".to_string(), unique)]),
            finalized: false,
            nominal: None,
        });
        let shared_record = table.intern(LoweredType::Record {
            fields: BTreeMap::from([("value".to_string(), shared)]),
            finalized: false,
            nominal: None,
        });
        let unique_array = table.intern(LoweredType::Array {
            element_type: unique_record,
            size: Some(1),
        });

        assert!(table.moves_on_transfer(unique_record));
        assert!(table.moves_on_transfer(unique_array));
        assert!(!table.moves_on_transfer(shared_record));
    }

    #[test]
    fn generic_parameters_transfer_conservatively_until_instantiated() {
        let mut table = LoweredTypeTable::new();
        let generic = table.intern(LoweredType::GenericParameter {
            name: "T".to_string(),
        });
        let optional_generic = table.intern(LoweredType::Optional { inner: generic });
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));

        assert!(table.moves_on_transfer(generic));
        assert!(table.moves_on_transfer(optional_generic));
        assert!(!table.moves_on_transfer(int_id));
    }

    #[test]
    fn global_storage_hazards_are_detected_transitively() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let borrowed = table.intern(LoweredType::Borrowed {
            inner: int_id,
            mutable: false,
        });
        let shared = table.intern(LoweredType::Pointer {
            target: int_id,
            shared: true,
            weak: false,
            sync: false,
            raw: false,
            mutable: false,
        });
        let nested = table.intern(LoweredType::Record {
            fields: BTreeMap::from([
                ("view".to_string(), borrowed),
                ("shared".to_string(), shared),
            ]),
            finalized: false,
            nominal: None,
        });

        assert!(table.contains_borrowed(nested));
        assert!(table.contains_shared_pointer(nested));
        assert!(!table.contains_borrowed(int_id));
        assert!(!table.contains_shared_pointer(int_id));
    }
}

#[cfg(test)]
mod render_type_tests {
    use super::*;

    #[test]
    fn renders_builtins_and_containers_in_fol_spelling() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let str_id = table.intern_builtin(LoweredBuiltinType::Str);
        assert_eq!(table.render_type(int_id), "int");
        assert_eq!(table.render_type(str_id), "str");

        let vec_id = table.intern(LoweredType::Vector {
            element_type: int_id,
        });
        assert_eq!(table.render_type(vec_id), "vec[int]");
        let map_id = table.intern(LoweredType::Map {
            key_type: str_id,
            value_type: vec_id,
        });
        assert_eq!(table.render_type(map_id), "map[str, vec[int]]");
        let opt_id = table.intern(LoweredType::Optional { inner: int_id });
        assert_eq!(table.render_type(opt_id), "opt[int]");
    }

    // An array keeps its length, unlike the diagnostic renderer's `[int]`.
    #[test]
    fn renders_arrays_with_their_declared_length() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let sized = table.intern(LoweredType::Array {
            element_type: int_id,
            size: Some(8),
        });
        let unsized_array = table.intern(LoweredType::Array {
            element_type: int_id,
            size: None,
        });
        assert_eq!(table.render_type(sized), "arr[int, 8]");
        assert_eq!(table.render_type(unsized_array), "arr[int]");
    }

    // A package identity can be an absolute build path; only the declared name
    // may reach a user-visible string.
    #[test]
    fn nominal_records_render_the_declared_name_without_the_package_path() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let record = table.intern(LoweredType::Record {
            fields: BTreeMap::from([("x".to_string(), int_id)]),
            finalized: false,
            nominal: Some("/tmp/build/scratch/pkg::Point".to_string()),
        });
        assert_eq!(table.render_type(record), "Point");
    }

    #[test]
    fn structural_records_render_their_fields() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let record = table.intern(LoweredType::Record {
            fields: BTreeMap::from([("x".to_string(), int_id)]),
            finalized: false,
            nominal: None,
        });
        assert_eq!(table.render_type(record), "rec { x: int }");
    }

    #[test]
    fn entries_prefer_a_noted_declared_name_over_their_variants() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let entry = table.intern(LoweredType::Entry {
            variants: BTreeMap::from([("DARK".to_string(), Some(int_id))]),
        });
        assert_eq!(table.render_type(entry), "ent { DARK: int }");
        table.note_declared_entry_name(entry, "Shade");
        assert_eq!(table.render_type(entry), "Shade");
        // First name wins, so lowering order cannot change the answer.
        table.note_declared_entry_name(entry, "Other");
        assert_eq!(table.render_type(entry), "Shade");
    }

    // Borrows are peeled at the call site, not here, so the renderer must still
    // spell them out when asked directly.
    #[test]
    fn renders_loans_and_pointers_distinctly() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let shared = table.intern(LoweredType::Borrowed {
            inner: int_id,
            mutable: false,
        });
        let unique = table.intern(LoweredType::Borrowed {
            inner: int_id,
            mutable: true,
        });
        let weak = table.intern(LoweredType::Pointer {
            target: int_id,
            shared: false,
            weak: true,
            sync: false,
            raw: false,
            mutable: false,
        });
        let sync = table.intern(LoweredType::Pointer {
            target: int_id,
            shared: true,
            weak: false,
            sync: true,
            raw: false,
            mutable: false,
        });
        assert_eq!(table.render_type(shared), "bor[int]");
        assert_eq!(table.render_type(unique), "bor[mut, int]");
        assert_eq!(table.render_type(weak), "ptr[weak, int]");
        assert_eq!(table.render_type(sync), "ptr[shared, sync, int]");
    }
}

use fol_parser::ast::AstNode;
use fol_types::{FloatWidth, IntWidth};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    hash::{Hash, Hasher},
};

use fol_resolver::SymbolId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedTypeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuiltinType {
    /// Integers keep the width and sign they were written with. `int` is
    /// `Int(IntWidth::DEFAULT)`, which is `i64`.
    Int(IntWidth),
    Float(FloatWidth),
    Bool,
    Char(fol_types::CharEncoding),
    Str,
    Never,
}

impl BuiltinType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Int(width) => width.fol_spelling(),
            Self::Float(width) => width.fol_spelling(),
            Self::Bool => "bol",
            Self::Char(encoding) => encoding.fol_spelling(),
            Self::Str => "str",
            Self::Never => "never",
        }
    }

    /// Every spelling that names a builtin scalar, including the sized ones.
    pub const ALL_NAMES: &[&str] = &[
        "int", "flt", "bol", "chr", "str", "never", "utf8", "utf16", "utf32", "i8", "i16", "i32",
        "i64", "i128", "arch", "u8", "u16", "u32", "u64", "u128", "uarch", "f32", "f64",
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclaredTypeKind {
    Type,
    Alias,
    GenericParameter,
}

/// A standard bound on a generic parameter, carrying the type arguments the
/// constraint was written with. For a non-generic standard (`T: geo`) `args` is
/// empty; for a generic standard (`T: Holder[int]`) `args` records `[int]` so a
/// constraint call can substitute the standard's own parameters on demand.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenericConstraint {
    pub standard: SymbolId,
    pub args: Vec<CheckedTypeId>,
}

#[derive(Debug, Clone)]
pub struct RoutineType {
    pub generic_params: Vec<SymbolId>,
    pub generic_constraints: BTreeMap<SymbolId, Vec<GenericConstraint>>,
    pub param_names: Vec<String>,
    pub param_defaults: Vec<Option<AstNode>>,
    pub variadic_index: Option<usize>,
    /// Parameter positions that carry a mutex handle instead of an ordinary
    /// by-value argument. Keeping this in the signature preserves the
    /// capability across package mounts.
    pub mutex_params: BTreeSet<usize>,
    pub params: Vec<CheckedTypeId>,
    pub return_type: Option<CheckedTypeId>,
    pub error_type: Option<CheckedTypeId>,
    /// True when the routine type carries a `[bor=L]` environment lifetime
    /// (V3_MEM section 5.3): its value may hold borrowed captures whose loans
    /// are tied to the caller-provided region.
    pub env_lifetime: bool,
}

impl RoutineType {
    /// Which parameters carry a default. Part of the routine's callable
    /// identity together with `param_names`, so routines that differ only in
    /// parameter names or defaultedness must not collapse to one interned
    /// type. Concrete default ASTs are declaration-owned on `TypedSymbol` and
    /// are overlaid before named-call binding/lowering; equal callable shapes
    /// may legitimately carry different expressions. The default expressions
    /// are also not hashable (`AstNode` is `PartialEq`-only).
    fn default_flags(&self) -> Vec<bool> {
        self.param_defaults
            .iter()
            .map(|default| default.is_some())
            .collect()
    }
}

impl PartialEq for RoutineType {
    fn eq(&self, other: &Self) -> bool {
        self.generic_params == other.generic_params
            && self.generic_constraints == other.generic_constraints
            && self.variadic_index == other.variadic_index
            && self.mutex_params == other.mutex_params
            && self.default_flags() == other.default_flags()
            && self.param_names == other.param_names
            && self.params == other.params
            && self.return_type == other.return_type
            && self.error_type == other.error_type
            && self.env_lifetime == other.env_lifetime
    }
}

impl Eq for RoutineType {}

impl PartialOrd for RoutineType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RoutineType {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            &self.generic_params,
            &self.generic_constraints,
            self.variadic_index,
            &self.mutex_params,
            self.default_flags(),
            &self.param_names,
            &self.params,
            self.return_type,
            self.error_type,
            self.env_lifetime,
        )
            .cmp(&(
                &other.generic_params,
                &other.generic_constraints,
                other.variadic_index,
                &other.mutex_params,
                other.default_flags(),
                &other.param_names,
                &other.params,
                other.return_type,
                other.error_type,
                other.env_lifetime,
            ))
    }
}

impl Hash for RoutineType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.generic_params.hash(state);
        self.generic_constraints.hash(state);
        self.variadic_index.hash(state);
        self.mutex_params.hash(state);
        self.default_flags().hash(state);
        self.param_names.hash(state);
        self.params.hash(state);
        self.return_type.hash(state);
        self.error_type.hash(state);
        self.env_lifetime.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedType {
    Builtin(BuiltinType),
    Declared {
        symbol: SymbolId,
        name: String,
        kind: DeclaredTypeKind,
        /// Type arguments for a generic instantiation (`Box[int]` → `[int]`).
        /// Empty for a plain (non-generic) type reference. Part of the type's
        /// nominal identity: `Box[int]` and `Cup[int]` differ by `symbol`,
        /// `Box[int]` and `Box[str]` differ by `args`, so the structural
        /// collapse that made same-shaped generics indistinguishable is gone.
        /// The structural expansion is resolved on demand via `apparent_type_id`
        /// / the instantiation-shape table, never stored as identity.
        args: Vec<CheckedTypeId>,
    },
    Array {
        element_type: CheckedTypeId,
        size: Option<usize>,
    },
    Vector {
        element_type: CheckedTypeId,
    },
    Sequence {
        element_type: CheckedTypeId,
    },
    Set {
        member_types: Vec<CheckedTypeId>,
    },
    Map {
        key_type: CheckedTypeId,
        value_type: CheckedTypeId,
    },
    Channel {
        element_type: CheckedTypeId,
    },
    ChannelSender {
        element_type: CheckedTypeId,
    },
    ChannelReceiver {
        element_type: CheckedTypeId,
    },
    Eventual {
        value_type: CheckedTypeId,
        error_type: Option<CheckedTypeId>,
    },
    Optional {
        inner: CheckedTypeId,
    },
    Owned {
        inner: CheckedTypeId,
    },
    Borrowed {
        inner: CheckedTypeId,
        mutable: bool,
    },
    Pointer {
        target: CheckedTypeId,
        shared: bool,
        /// A `ptr[weak, T]` weak handle (`std::rc::Weak<T>`): does not keep the
        /// shared allocation alive; created with `[weak]`, read via `[upg]`.
        weak: bool,
        /// A `ptr[shared, sync, T]` uses an `Arc` and is thread-safe, so it may
        /// cross task boundaries; `Rc`-backed shared/weak pointers cannot.
        sync: bool,
        /// A `ptr[raw, T]` or `ptr[raw, mut, T]`: a foreign address token
        /// rather than a managed pointer. Section 4.8 keeps it non-owning.
        raw: bool,
        /// Whether the pointee may be written through. Constness is an ABI fact
        /// a C caller cannot infer, so it is carried rather than defaulted.
        ///
        /// Nullability is *not* here: section 4.8 makes optional wrapping the
        /// nullability marker, so `opt ptr[raw, T]` is the nullable form.
        mutable: bool,
    },
    Error {
        inner: Option<CheckedTypeId>,
    },
    Record {
        fields: BTreeMap<String, CheckedTypeId>,
    },
    Entry {
        variants: BTreeMap<String, Option<CheckedTypeId>>,
    },
    Routine(RoutineType),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypeTable {
    types: Vec<CheckedType>,
    canonical_ids: BTreeMap<CheckedType, CheckedTypeId>,
}

impl TypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    pub fn get(&self, id: CheckedTypeId) -> Option<&CheckedType> {
        self.types.get(id.0)
    }

    pub fn intern(&mut self, ty: CheckedType) -> CheckedTypeId {
        if let Some(id) = self.canonical_ids.get(&ty) {
            return *id;
        }

        let id = CheckedTypeId(self.types.len());
        self.types.push(ty.clone());
        self.canonical_ids.insert(ty, id);
        id
    }

    pub fn intern_builtin(&mut self, builtin: BuiltinType) -> CheckedTypeId {
        self.intern(CheckedType::Builtin(builtin))
    }

    /// Render a checked type for display in editor tooling (hover, completion).
    pub fn render_type(&self, type_id: CheckedTypeId) -> String {
        match self.get(type_id) {
            Some(CheckedType::Builtin(builtin)) => builtin.as_str().to_string(),
            Some(CheckedType::Declared { name, args, .. }) => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let rendered = args
                        .iter()
                        .map(|arg| self.render_type(*arg))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{name}[{rendered}]")
                }
            }
            Some(CheckedType::Channel { element_type }) => {
                format!("chn[{}]", self.render_type(*element_type))
            }
            Some(CheckedType::ChannelSender { element_type }) => {
                format!("chn[tx, {}]", self.render_type(*element_type))
            }
            Some(CheckedType::ChannelReceiver { element_type }) => {
                format!("chn[rx, {}]", self.render_type(*element_type))
            }
            Some(CheckedType::Eventual {
                value_type,
                error_type,
            }) => match error_type {
                Some(error_type) => format!(
                    "evt[{} / {}]",
                    self.render_type(*value_type),
                    self.render_type(*error_type)
                ),
                None => format!("evt[{}]", self.render_type(*value_type)),
            },
            Some(CheckedType::Optional { inner }) => {
                format!("opt[{}]", self.render_type(*inner))
            }
            Some(CheckedType::Owned { inner }) => {
                format!("@{}", self.render_type(*inner))
            }
            Some(CheckedType::Borrowed { inner, mutable }) => {
                if *mutable {
                    format!("bor[mut, {}]", self.render_type(*inner))
                } else {
                    format!("bor[{}]", self.render_type(*inner))
                }
            }
            Some(CheckedType::Pointer {
                target,
                shared,
                weak,
                sync,
                ..
            }) => {
                if *weak {
                    format!("ptr[weak, {}]", self.render_type(*target))
                } else if *shared && *sync {
                    format!("ptr[shared, sync, {}]", self.render_type(*target))
                } else if *shared {
                    format!("ptr[shared, {}]", self.render_type(*target))
                } else {
                    format!("ptr[{}]", self.render_type(*target))
                }
            }
            Some(CheckedType::Error { inner }) => inner
                .map(|inner| format!("err[{}]", self.render_type(inner)))
                .unwrap_or_else(|| "err[]".to_string()),
            Some(CheckedType::Array { element_type, .. }) => {
                format!("[{}]", self.render_type(*element_type))
            }
            Some(CheckedType::Vector { element_type }) => {
                format!("vec[{}]", self.render_type(*element_type))
            }
            Some(CheckedType::Sequence { element_type }) => {
                format!("seq[{}]", self.render_type(*element_type))
            }
            Some(CheckedType::Set { member_types }) => format!(
                "set[{}]",
                member_types
                    .iter()
                    .map(|m| self.render_type(*m))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Some(CheckedType::Map {
                key_type,
                value_type,
            }) => format!(
                "map[{}, {}]",
                self.render_type(*key_type),
                self.render_type(*value_type)
            ),
            Some(CheckedType::Routine(routine)) => {
                let params = routine
                    .params
                    .iter()
                    .map(|p| self.render_type(*p))
                    .collect::<Vec<_>>()
                    .join(", ");
                let returns = routine
                    .return_type
                    .map(|r| self.render_type(r))
                    .unwrap_or_else(|| "void".to_string());
                match routine.error_type {
                    Some(err) => format!("fun({params}): {returns} / {}", self.render_type(err)),
                    None => format!("fun({params}): {returns}"),
                }
            }
            Some(CheckedType::Record { fields }) => {
                let fields = fields
                    .iter()
                    .map(|(name, field_type)| format!("{name}: {}", self.render_type(*field_type)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("rec {{ {fields} }}")
            }
            Some(CheckedType::Entry { variants }) => {
                let variants = variants
                    .iter()
                    .map(|(name, payload)| match payload {
                        Some(payload) => format!("{name}: {}", self.render_type(*payload)),
                        None => name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("ent {{ {variants} }}")
            }
            None => "unknown".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BuiltinType, CheckedType, DeclaredTypeKind, RoutineType, TypeTable};
    use fol_resolver::SymbolId;
    use std::collections::BTreeMap;

    #[test]
    fn type_table_interns_builtin_types_canonically() {
        let mut table = TypeTable::new();

        let first = table.intern_builtin(BuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let second = table.intern_builtin(BuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let third = table.intern_builtin(BuiltinType::Str);

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(table.len(), 2);
        assert_eq!(
            table.get(first),
            Some(&CheckedType::Builtin(BuiltinType::Int(
                fol_types::IntWidth::DEFAULT
            )))
        );
        assert_eq!(
            table.get(third),
            Some(&CheckedType::Builtin(BuiltinType::Str))
        );
    }

    #[test]
    fn type_table_canonicalizes_declared_and_structural_shapes() {
        let mut table = TypeTable::new();
        let int_id = table.intern_builtin(BuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let declared = table.intern(CheckedType::Declared {
            symbol: SymbolId(4),
            name: "Point".to_string(),
            kind: DeclaredTypeKind::Type,
            args: Vec::new(),
        });

        let mut fields = BTreeMap::new();
        fields.insert("x".to_string(), int_id);
        fields.insert("y".to_string(), int_id);
        let record_first = table.intern(CheckedType::Record {
            fields: fields.clone(),
        });
        let record_second = table.intern(CheckedType::Record { fields });
        let routine = table.intern(CheckedType::Routine(RoutineType {
            generic_params: Vec::new(),
            generic_constraints: BTreeMap::new(),
            param_names: vec!["point".to_string(), "count".to_string()],
            param_defaults: vec![None, None],
            variadic_index: None,
            mutex_params: std::collections::BTreeSet::new(),
            params: vec![declared, int_id],
            return_type: Some(declared),
            error_type: None,
            env_lifetime: false,
        }));

        assert_eq!(record_first, record_second);
        assert_ne!(declared, routine);
        assert_eq!(
            table.get(declared),
            Some(&CheckedType::Declared {
                symbol: SymbolId(4),
                name: "Point".to_string(),
                kind: DeclaredTypeKind::Type,
                args: Vec::new(),
            })
        );
    }

    #[test]
    fn builtin_type_as_str_matches_language_spelling() {
        assert_eq!(
            BuiltinType::Int(fol_types::IntWidth::DEFAULT).as_str(),
            "int"
        );
        assert_eq!(
            BuiltinType::Float(fol_types::FloatWidth::DEFAULT).as_str(),
            "flt"
        );
        assert_eq!(BuiltinType::Bool.as_str(), "bol");
        assert_eq!(
            BuiltinType::Char(fol_types::CharEncoding::DEFAULT).as_str(),
            "chr"
        );
        assert_eq!(
            BuiltinType::Char(fol_types::CharEncoding::Utf16).as_str(),
            "utf16"
        );
        assert_eq!(BuiltinType::Str.as_str(), "str");
        assert_eq!(BuiltinType::Never.as_str(), "never");
    }

    #[test]
    fn builtin_type_all_names_covers_every_variant() {
        // Six unsized spellings, every sized integer and float, and the three
        // character encodings.
        assert_eq!(
            BuiltinType::ALL_NAMES.len(),
            6 + fol_types::IntWidth::ALL.len() + 2 + fol_types::CharEncoding::ALL.len()
        );
        for name in BuiltinType::ALL_NAMES {
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn render_type_handles_builtins_and_containers() {
        let mut table = TypeTable::new();
        let int_id = table.intern_builtin(BuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let str_id = table.intern_builtin(BuiltinType::Str);
        let opt_id = table.intern(CheckedType::Optional { inner: int_id });
        let vec_id = table.intern(CheckedType::Vector {
            element_type: str_id,
        });
        let map_id = table.intern(CheckedType::Map {
            key_type: str_id,
            value_type: int_id,
        });

        assert_eq!(table.render_type(int_id), "int");
        assert_eq!(table.render_type(opt_id), "opt[int]");
        assert_eq!(table.render_type(vec_id), "vec[str]");
        assert_eq!(table.render_type(map_id), "map[str, int]");
    }

    #[test]
    fn render_type_handles_routines() {
        let mut table = TypeTable::new();
        let int_id = table.intern_builtin(BuiltinType::Int(fol_types::IntWidth::DEFAULT));
        let str_id = table.intern_builtin(BuiltinType::Str);
        let routine_id = table.intern(CheckedType::Routine(RoutineType {
            generic_params: Vec::new(),
            generic_constraints: BTreeMap::new(),
            param_names: vec!["left".to_string(), "right".to_string()],
            param_defaults: vec![None, None],
            variadic_index: None,
            mutex_params: std::collections::BTreeSet::new(),
            params: vec![int_id, str_id],
            return_type: Some(int_id),
            error_type: None,
            env_lifetime: false,
        }));
        assert_eq!(table.render_type(routine_id), "fun(int, str): int");
    }
}

#[cfg(test)]
mod raw_pointer_tests {
    use super::{CheckedType, TypeTable};

    /// `ptr[raw, T]` and `ptr[raw, mut, T]` are distinct types.
    ///
    /// Constness is an ABI fact a C caller cannot infer, so it is part of type
    /// identity rather than a note somewhere.
    #[test]
    fn raw_and_raw_mut_are_distinct_types() {
        let mut table = TypeTable::new();
        let int = table.intern_builtin(crate::BuiltinType::Int(fol_types::IntWidth::DEFAULT));

        let read_only = table.intern(CheckedType::Pointer {
            target: int,
            shared: false,
            weak: false,
            sync: false,
            raw: true,
            mutable: false,
        });
        let writable = table.intern(CheckedType::Pointer {
            target: int,
            shared: false,
            weak: false,
            sync: false,
            raw: true,
            mutable: true,
        });
        let managed = table.intern(CheckedType::Pointer {
            target: int,
            shared: false,
            weak: false,
            sync: false,
            raw: false,
            mutable: false,
        });

        assert_ne!(read_only, writable, "constness must change type identity");
        assert_ne!(read_only, managed, "raw-ness must change type identity");
    }
}

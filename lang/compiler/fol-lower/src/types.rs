use crate::ids::LoweredTypeId;
use fol_resolver::{PackageIdentity, SymbolId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoweredBuiltinType {
    Int,
    Float,
    Bool,
    Char,
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
                Some(LoweredType::Owned { .. })
                | Some(LoweredType::Pointer { shared: false, .. })
                | Some(LoweredType::Eventual { .. })
                | Some(LoweredType::Channel { .. }) => true,
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
                Some(LoweredType::Record { fields }) => fields
                    .values()
                    .any(|field| moves(table, *field, visiting)),
                Some(LoweredType::Entry { variants }) => variants
                    .values()
                    .flatten()
                    .any(|variant| moves(table, *variant, visiting)),
                Some(LoweredType::Builtin(_))
                | Some(LoweredType::GenericParameter { .. })
                | Some(LoweredType::Named { .. })
                | Some(LoweredType::Borrowed { .. })
                | Some(LoweredType::Pointer { shared: true, .. })
                | Some(LoweredType::ChannelSender { .. })
                | Some(LoweredType::Routine(_))
                | None => false,
            };
            visiting.remove(&id);
            result
        }

        moves(self, id, &mut BTreeSet::new())
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
            LoweredType::Record { fields } => fields
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
                    || error_type.is_some_and(|error_type| {
                        self.contains_generic_structural_type(error_type)
                    })
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
            LoweredType::Record { fields } => {
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

#[cfg(test)]
mod tests {
    use super::{LoweredBuiltinType, LoweredRoutineType, LoweredType, LoweredTypeTable};
    use crate::ids::LoweredTypeId;
    use std::collections::BTreeMap;

    #[test]
    fn lowered_type_table_interns_builtin_shapes_canonically() {
        let mut table = LoweredTypeTable::new();

        let first = table.intern_builtin(LoweredBuiltinType::Int);
        let second = table.intern_builtin(LoweredBuiltinType::Int);
        let third = table.intern_builtin(LoweredBuiltinType::Str);

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn lowered_type_table_canonicalizes_structural_shapes() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);

        let mut fields = BTreeMap::new();
        fields.insert("x".to_string(), int_id);
        fields.insert("y".to_string(), int_id);

        let record_first = table.intern(LoweredType::Record {
            fields: fields.clone(),
        });
        let record_second = table.intern(LoweredType::Record { fields });
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
            })
        );
    }

    #[test]
    fn aggregate_transfer_is_move_only_when_a_field_is_unique() {
        let mut table = LoweredTypeTable::new();
        let int_id = table.intern_builtin(LoweredBuiltinType::Int);
        let unique = table.intern(LoweredType::Pointer {
            target: int_id,
            shared: false,
        });
        let shared = table.intern(LoweredType::Pointer {
            target: int_id,
            shared: true,
        });
        let unique_record = table.intern(LoweredType::Record {
            fields: BTreeMap::from([("value".to_string(), unique)]),
        });
        let shared_record = table.intern(LoweredType::Record {
            fields: BTreeMap::from([("value".to_string(), shared)]),
        });
        let unique_array = table.intern(LoweredType::Array {
            element_type: unique_record,
            size: Some(1),
        });

        assert!(table.moves_on_transfer(unique_record));
        assert!(table.moves_on_transfer(unique_array));
        assert!(!table.moves_on_transfer(shared_record));
    }
}

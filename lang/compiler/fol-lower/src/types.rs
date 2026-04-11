use crate::ids::LoweredTypeId;
use std::collections::BTreeMap;

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoweredType {
    Builtin(LoweredBuiltinType),
    GenericParameter {
        name: String,
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
}

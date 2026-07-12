use fol_parser::ast::AstNode;
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    hash::{Hash, Hasher},
};

use fol_resolver::SymbolId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedTypeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BuiltinType {
    Int,
    Float,
    Bool,
    Char,
    Str,
    Never,
}

impl BuiltinType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "flt",
            Self::Bool => "bol",
            Self::Char => "chr",
            Self::Str => "str",
            Self::Never => "never",
        }
    }

    pub const ALL_NAMES: &[&str] = &["int", "flt", "bol", "chr", "str", "never"];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclaredTypeKind {
    Type,
    Alias,
    GenericParameter,
}

#[derive(Debug, Clone)]
pub struct RoutineType {
    pub generic_params: Vec<SymbolId>,
    pub generic_constraints: BTreeMap<SymbolId, Vec<SymbolId>>,
    pub param_names: Vec<String>,
    pub param_defaults: Vec<Option<AstNode>>,
    pub variadic_index: Option<usize>,
    pub params: Vec<CheckedTypeId>,
    pub return_type: Option<CheckedTypeId>,
    pub error_type: Option<CheckedTypeId>,
}

impl RoutineType {
    /// Which parameters carry a default. Part of the routine's callable
    /// identity together with `param_names`: named-argument binding and
    /// default filling both read the interned signature, so two routines
    /// that differ only in parameter names or defaultedness must not
    /// collapse to one interned type. The default *expressions* are not
    /// hashable (`AstNode` is `PartialEq`-only), but the defaultedness
    /// pattern is what call-arity binding depends on.
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
            && self.default_flags() == other.default_flags()
            && self.param_names == other.param_names
            && self.params == other.params
            && self.return_type == other.return_type
            && self.error_type == other.error_type
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
            self.default_flags(),
            &self.param_names,
            &self.params,
            self.return_type,
            self.error_type,
        )
            .cmp(&(
                &other.generic_params,
                &other.generic_constraints,
                other.variadic_index,
                other.default_flags(),
                &other.param_names,
                &other.params,
                other.return_type,
                other.error_type,
            ))
    }
}

impl Hash for RoutineType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.generic_params.hash(state);
        self.generic_constraints.hash(state);
        self.variadic_index.hash(state);
        self.default_flags().hash(state);
        self.param_names.hash(state);
        self.params.hash(state);
        self.return_type.hash(state);
        self.error_type.hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckedType {
    Builtin(BuiltinType),
    Declared {
        symbol: SymbolId,
        name: String,
        kind: DeclaredTypeKind,
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
    Optional {
        inner: CheckedTypeId,
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
            Some(CheckedType::Declared { name, .. }) => name.clone(),
            Some(CheckedType::Optional { inner }) => {
                format!("opt[{}]", self.render_type(*inner))
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
                    Some(err) => format!(
                        "fun({params}): {returns} / {}",
                        self.render_type(err)
                    ),
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

        let first = table.intern_builtin(BuiltinType::Int);
        let second = table.intern_builtin(BuiltinType::Int);
        let third = table.intern_builtin(BuiltinType::Str);

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(table.len(), 2);
        assert_eq!(
            table.get(first),
            Some(&CheckedType::Builtin(BuiltinType::Int))
        );
        assert_eq!(
            table.get(third),
            Some(&CheckedType::Builtin(BuiltinType::Str))
        );
    }

    #[test]
    fn type_table_canonicalizes_declared_and_structural_shapes() {
        let mut table = TypeTable::new();
        let int_id = table.intern_builtin(BuiltinType::Int);
        let declared = table.intern(CheckedType::Declared {
            symbol: SymbolId(4),
            name: "Point".to_string(),
            kind: DeclaredTypeKind::Type,
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
            params: vec![declared, int_id],
            return_type: Some(declared),
            error_type: None,
        }));

        assert_eq!(record_first, record_second);
        assert_ne!(declared, routine);
        assert_eq!(
            table.get(declared),
            Some(&CheckedType::Declared {
                symbol: SymbolId(4),
                name: "Point".to_string(),
                kind: DeclaredTypeKind::Type,
            })
        );
    }

    #[test]
    fn builtin_type_as_str_matches_language_spelling() {
        assert_eq!(BuiltinType::Int.as_str(), "int");
        assert_eq!(BuiltinType::Float.as_str(), "flt");
        assert_eq!(BuiltinType::Bool.as_str(), "bol");
        assert_eq!(BuiltinType::Char.as_str(), "chr");
        assert_eq!(BuiltinType::Str.as_str(), "str");
        assert_eq!(BuiltinType::Never.as_str(), "never");
    }

    #[test]
    fn builtin_type_all_names_covers_every_variant() {
        assert_eq!(BuiltinType::ALL_NAMES.len(), 6);
        for name in BuiltinType::ALL_NAMES {
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn render_type_handles_builtins_and_containers() {
        let mut table = TypeTable::new();
        let int_id = table.intern_builtin(BuiltinType::Int);
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
        let int_id = table.intern_builtin(BuiltinType::Int);
        let str_id = table.intern_builtin(BuiltinType::Str);
        let routine_id = table.intern(CheckedType::Routine(RoutineType {
            generic_params: Vec::new(),
            generic_constraints: BTreeMap::new(),
            param_names: vec!["left".to_string(), "right".to_string()],
            param_defaults: vec![None, None],
            variadic_index: None,
            params: vec![int_id, str_id],
            return_type: Some(int_id),
            error_type: None,
        }));
        assert_eq!(table.render_type(routine_id), "fun(int, str): int");
    }
}

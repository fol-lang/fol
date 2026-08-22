use crate::types::{BuiltinType, CheckedTypeId, TypeTable};
use fol_types::{FloatWidth, IntWidth};

/// The interned ids for the scalar types. `int` and `flt` are the default
/// widths rather than separate types, so `int` and `i64` share one id and are
/// the same type — see `plan/V4_SCALAR_WIDTHS.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTypeIds {
    pub int: CheckedTypeId,
    pub float: CheckedTypeId,
    pub bool_: CheckedTypeId,
    pub char_: CheckedTypeId,
    pub str_: CheckedTypeId,
    pub never: CheckedTypeId,
}

impl BuiltinTypeIds {
    pub fn install(table: &mut TypeTable) -> Self {
        // Every width is interned up front so a later lookup never has to
        // mutate the table just to name a type that already exists.
        for width in IntWidth::ALL {
            table.intern_builtin(BuiltinType::Int(*width));
        }
        for width in FloatWidth::ALL {
            table.intern_builtin(BuiltinType::Float(*width));
        }
        Self {
            int: table.intern_builtin(BuiltinType::Int(IntWidth::DEFAULT)),
            float: table.intern_builtin(BuiltinType::Float(FloatWidth::DEFAULT)),
            bool_: table.intern_builtin(BuiltinType::Bool),
            char_: table.intern_builtin(BuiltinType::Char),
            str_: table.intern_builtin(BuiltinType::Str),
            never: table.intern_builtin(BuiltinType::Never),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::BuiltinTypeIds;
    use crate::types::{BuiltinType, CheckedType, TypeTable};

    #[test]
    fn builtin_type_ids_install_all_v1_scalar_types_once() {
        let mut table = TypeTable::new();
        let builtins = BuiltinTypeIds::install(&mut table);

        assert_eq!(table.len(), 6);
        assert_eq!(
            table.get(builtins.int),
            Some(&CheckedType::Builtin(BuiltinType::Int(
                fol_types::IntWidth::DEFAULT
            )))
        );
        assert_eq!(
            table.get(builtins.float),
            Some(&CheckedType::Builtin(BuiltinType::Float(
                fol_types::FloatWidth::DEFAULT
            )))
        );
        assert_eq!(
            table.get(builtins.bool_),
            Some(&CheckedType::Builtin(BuiltinType::Bool))
        );
        assert_eq!(
            table.get(builtins.char_),
            Some(&CheckedType::Builtin(BuiltinType::Char))
        );
        assert_eq!(
            table.get(builtins.str_),
            Some(&CheckedType::Builtin(BuiltinType::Str))
        );
        assert_eq!(
            table.get(builtins.never),
            Some(&CheckedType::Builtin(BuiltinType::Never))
        );
    }

    #[test]
    fn builtin_type_ids_reuse_existing_builtin_slots() {
        let mut table = TypeTable::new();
        let first = BuiltinTypeIds::install(&mut table);
        let second = BuiltinTypeIds::install(&mut table);

        assert_eq!(first, second);
        assert_eq!(table.len(), 6);
    }
}

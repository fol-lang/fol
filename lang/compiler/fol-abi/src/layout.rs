//! Target layout for the ABI's aggregate types.
//!
//! FOL computes size, alignment, and every field offset itself, and the
//! generated header asserts those numbers with `_Static_assert`. That is the
//! point: the C compiler recomputes them from its own rules and refuses to
//! compile if it disagrees, so the two are checked against each other in the
//! consumer's own translation unit rather than trusted to match.
//!
//! The rules implemented are the System V AMD64 ones, which the certified
//! Linux targets use: a scalar is aligned to its own size, a struct's
//! alignment is the largest of its fields', each field sits at the next
//! offset satisfying its alignment, and the struct's size is rounded up to its
//! alignment so that an array of them stays aligned.

use crate::types::{AbiScalar, AbiType, AbiTypeId, AbiTypeTable};

/// Where one field sits inside its record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPlacement {
    pub name: String,
    pub offset: u64,
    pub size: u64,
    pub align: u64,
}

/// One record's computed layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordLayout {
    pub name: String,
    pub size: u64,
    pub align: u64,
    pub fields: Vec<FieldPlacement>,
    /// Padding bytes the layout inserted, for a reader asking where the size
    /// went. Not asserted -- it is implied by the offsets and the size.
    pub padding: u64,
}

/// Why a type has no computable layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// A type the table does not define.
    UnknownType,
    /// A shape with no fixed size on the certified targets.
    Unsized { kind: &'static str },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownType => f.write_str("the ABI table does not define this type"),
            Self::Unsized { kind } => {
                write!(f, "a {kind} has no fixed size on the certified targets")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

/// The size and alignment of one ABI type.
pub fn size_and_align(table: &AbiTypeTable, id: AbiTypeId) -> Result<(u64, u64), LayoutError> {
    match table.get(id).ok_or(LayoutError::UnknownType)? {
        AbiType::Scalar(scalar) => {
            let bytes = scalar_bytes(*scalar);
            // Every scalar the boundary carries is naturally aligned: none is
            // wider than 8 bytes, and none is over- or under-aligned.
            Ok((bytes, bytes))
        }
        // A pointer is 8 bytes on both certified targets, which are LP64.
        AbiType::Pointer { .. } | AbiType::OpaqueHandle { .. } => Ok((8, 8)),
        AbiType::Record { .. } => {
            let layout = record_layout(table, id)?;
            Ok((layout.size, layout.align))
        }
        AbiType::Void => Err(LayoutError::Unsized { kind: "void" }),
        other => Err(LayoutError::Unsized {
            kind: other.kind_name(),
        }),
    }
}

fn scalar_bytes(scalar: AbiScalar) -> u64 {
    match scalar {
        AbiScalar::Int(width) => {
            u64::from(width.bits().expect("arch widths never reach the ABI")) / 8
        }
        AbiScalar::Float(fol_types::FloatWidth::F32) => 4,
        AbiScalar::Float(_) => 8,
        // `fol_bool_t` is `uint8_t`, `fol_char_t` is `uint32_t`.
        AbiScalar::Bool => 1,
        AbiScalar::Char => 4,
    }
}

/// Lay out one record in source field order.
pub fn record_layout(table: &AbiTypeTable, id: AbiTypeId) -> Result<RecordLayout, LayoutError> {
    let AbiType::Record { name, fields } = table.get(id).ok_or(LayoutError::UnknownType)? else {
        return Err(LayoutError::Unsized { kind: "non-record" });
    };

    let mut offset = 0u64;
    let mut align = 1u64;
    let mut placements = Vec::with_capacity(fields.len());
    let mut used = 0u64;

    for field in fields {
        let (size, field_align) = size_and_align(table, field.type_id)?;
        // Advance to the next offset this field's alignment allows. The gap is
        // padding, which is what makes a struct larger than its fields.
        offset = align_up(offset, field_align);
        placements.push(FieldPlacement {
            name: field.name.clone(),
            offset,
            size,
            align: field_align,
        });
        offset += size;
        used += size;
        align = align.max(field_align);
    }

    // Round the total up so an array of these stays aligned.
    let size = align_up(offset, align);
    Ok(RecordLayout {
        name: name.clone(),
        size,
        align,
        fields: placements,
        padding: size - used,
    })
}

fn align_up(offset: u64, align: u64) -> u64 {
    if align <= 1 {
        return offset;
    }
    offset.div_ceil(align) * align
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AbiField;

    fn table_with(fields: &[(&str, AbiScalar)]) -> (AbiTypeTable, AbiTypeId) {
        let mut table = AbiTypeTable::new();
        let entries = fields
            .iter()
            .map(|(name, scalar)| AbiField {
                name: (*name).to_string(),
                type_id: table_intern(&mut table, *scalar),
            })
            .collect();
        let id = table.intern(AbiType::Record {
            name: "Demo".to_string(),
            fields: entries,
        });
        (table, id)
    }

    fn table_intern(table: &mut AbiTypeTable, scalar: AbiScalar) -> AbiTypeId {
        table.intern(AbiType::Scalar(scalar))
    }

    #[test]
    fn a_record_of_equal_widths_has_no_padding() {
        let (table, id) = table_with(&[
            ("a", AbiScalar::Int(fol_types::IntWidth::I32)),
            ("b", AbiScalar::Int(fol_types::IntWidth::I32)),
        ]);
        let layout = record_layout(&table, id).expect("layout");

        assert_eq!(layout.size, 8);
        assert_eq!(layout.align, 4);
        assert_eq!(layout.padding, 0);
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[1].offset, 4);
    }

    #[test]
    fn a_narrow_field_before_a_wide_one_is_padded_to_its_alignment() {
        // `{i8, i32}` is 8 bytes, not 5: the i32 cannot start at offset 1.
        let (table, id) = table_with(&[
            ("small", AbiScalar::Int(fol_types::IntWidth::I8)),
            ("wide", AbiScalar::Int(fol_types::IntWidth::I32)),
        ]);
        let layout = record_layout(&table, id).expect("layout");

        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(
            layout.fields[1].offset, 4,
            "three bytes of padding precede it"
        );
        assert_eq!(layout.size, 8);
        assert_eq!(layout.align, 4);
        assert_eq!(layout.padding, 3);
    }

    #[test]
    fn trailing_padding_rounds_the_size_up_to_the_alignment() {
        // `{i32, i8}` is 8 bytes so an array of them keeps each i32 aligned.
        let (table, id) = table_with(&[
            ("wide", AbiScalar::Int(fol_types::IntWidth::I32)),
            ("small", AbiScalar::Int(fol_types::IntWidth::I8)),
        ]);
        let layout = record_layout(&table, id).expect("layout");

        assert_eq!(layout.fields[1].offset, 4);
        assert_eq!(layout.size, 8, "rounded up from 5");
        assert_eq!(layout.padding, 3);
    }

    #[test]
    fn field_order_changes_the_layout() {
        // The whole reason declaration order is carried through: these two
        // records have the same fields and different layouts.
        let (compact, compact_id) = table_with(&[
            ("wide", AbiScalar::Int(fol_types::IntWidth::I32)),
            ("a", AbiScalar::Int(fol_types::IntWidth::I8)),
            ("b", AbiScalar::Int(fol_types::IntWidth::I8)),
        ]);
        let (loose, loose_id) = table_with(&[
            ("a", AbiScalar::Int(fol_types::IntWidth::I8)),
            ("wide", AbiScalar::Int(fol_types::IntWidth::I32)),
            ("b", AbiScalar::Int(fol_types::IntWidth::I8)),
        ]);

        assert_eq!(record_layout(&compact, compact_id).expect("layout").size, 8);
        assert_eq!(
            record_layout(&loose, loose_id).expect("layout").size,
            12,
            "the same three fields in a different order are a different struct"
        );
    }

    #[test]
    fn a_nested_record_contributes_its_own_alignment() {
        let mut table = AbiTypeTable::new();
        let i64_id = table.intern_int(fol_types::IntWidth::I64);
        let i8_id = table.intern_int(fol_types::IntWidth::I8);
        let inner = table.intern(AbiType::Record {
            name: "Inner".to_string(),
            fields: vec![AbiField {
                name: "value".to_string(),
                type_id: i64_id,
            }],
        });
        let outer = table.intern(AbiType::Record {
            name: "Outer".to_string(),
            fields: vec![
                AbiField {
                    name: "flag".to_string(),
                    type_id: i8_id,
                },
                AbiField {
                    name: "inner".to_string(),
                    type_id: inner,
                },
            ],
        });

        let layout = record_layout(&table, outer).expect("layout");
        assert_eq!(layout.align, 8, "the nested record raises the alignment");
        assert_eq!(layout.fields[1].offset, 8);
        assert_eq!(layout.size, 16);
    }

    #[test]
    fn the_boundary_scalars_have_their_c_sizes() {
        let mut table = AbiTypeTable::new();
        for (scalar, bytes) in [
            (AbiScalar::Bool, 1),
            (AbiScalar::Char, 4),
            (AbiScalar::Int(fol_types::IntWidth::U8), 1),
            (AbiScalar::Int(fol_types::IntWidth::I16), 2),
            (AbiScalar::Int(fol_types::IntWidth::I64), 8),
            (AbiScalar::Float(fol_types::FloatWidth::F32), 4),
            (AbiScalar::Float(fol_types::FloatWidth::F64), 8),
        ] {
            let id = table.intern(AbiType::Scalar(scalar));
            assert_eq!(
                size_and_align(&table, id).expect("scalar layout"),
                (bytes, bytes),
                "{scalar:?}"
            );
        }
    }

    #[test]
    fn void_has_no_layout() {
        let mut table = AbiTypeTable::new();
        let id = table.intern(AbiType::Void);
        assert_eq!(
            size_and_align(&table, id),
            Err(LayoutError::Unsized { kind: "void" })
        );
    }
}

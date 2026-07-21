use crate::IntrinsicEntry;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ComparisonOperandContract {
    EqualityScalar,
    OrderedScalar,
}

impl ComparisonOperandContract {
    pub const fn expected_operands(self) -> &'static str {
        match self {
            Self::EqualityScalar => "two comparable scalar operands",
            Self::OrderedScalar => "two ordered scalar operands",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BooleanOperandContract {
    BoolScalar,
}

impl BooleanOperandContract {
    pub const fn expected_operands(self) -> &'static str {
        match self {
            Self::BoolScalar => "one boolean operand",
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum QueryOperandContract {
    LengthQueryable,
}

impl QueryOperandContract {
    pub const fn expected_operands(self) -> &'static str {
        match self {
            Self::LengthQueryable => "one string, array, vector, sequence, set, or map operand",
        }
    }
}

pub const fn comparison_operand_contract(
    entry: &IntrinsicEntry,
) -> Option<ComparisonOperandContract> {
    match entry.id.index() {
        0 | 1 => Some(ComparisonOperandContract::EqualityScalar),
        2..=5 => Some(ComparisonOperandContract::OrderedScalar),
        _ => None,
    }
}

pub const fn boolean_operand_contract(entry: &IntrinsicEntry) -> Option<BooleanOperandContract> {
    match entry.id.index() {
        6 => Some(BooleanOperandContract::BoolScalar),
        _ => None,
    }
}

pub const fn query_operand_contract(entry: &IntrinsicEntry) -> Option<QueryOperandContract> {
    match entry.id.index() {
        7 => Some(QueryOperandContract::LengthQueryable),
        _ => None,
    }
}

//! Runtime-owned builtin and intrinsic hook support.

use crate::{
    memo::{FolMap, FolSeq, FolSet, FolStr, FolVec},
    containers::FolArray,
    value::FolInt,
};

pub trait FolLength {
    fn fol_length(&self) -> FolInt;
}

pub fn len<T: FolLength + ?Sized>(value: &T) -> FolInt {
    value.fol_length()
}

impl FolLength for FolStr {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T, const N: usize> FolLength for FolArray<T, N> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T> FolLength for FolVec<T> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T> FolLength for FolSeq<T> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<T: Ord> FolLength for FolSet<T> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

impl<K: Ord, V> FolLength for FolMap<K, V> {
    fn fol_length(&self) -> FolInt {
        self.len() as FolInt
    }
}

pub fn pow(base: FolInt, exponent: FolInt) -> FolInt {
    base.pow(exponent as u32)
}

pub fn pow_float(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
}

pub fn module_name() -> &'static str {
    "builtins"
}

#[cfg(test)]
mod tests {
    use super::{len, FolLength};
    use crate::{
        memo::{FolMap, FolSeq, FolSet, FolStr, FolVec},
        containers::FolArray,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn runtime_length_trait_covers_current_v1_families() {
        let text = FolStr::from("Ada");
        let array: FolArray<i64, 3> = [1, 2, 3];
        let vector = FolVec::from_items(vec![1, 2]);
        let sequence = FolSeq::from_items(vec![1, 2, 3, 4]);
        let set = FolSet::new(BTreeSet::from([1, 2, 3]));
        let map = FolMap::new(BTreeMap::from([("ada", 1), ("lin", 2)]));

        assert_eq!(text.fol_length(), 3);
        assert_eq!(array.fol_length(), 3);
        assert_eq!(vector.fol_length(), 2);
        assert_eq!(sequence.fol_length(), 4);
        assert_eq!(set.fol_length(), 3);
        assert_eq!(map.fol_length(), 2);
    }

    #[test]
    fn runtime_len_helper_covers_current_v1_supported_families() {
        let text = FolStr::from("Ada");
        let array: FolArray<i64, 3> = [1, 2, 3];
        let vector = FolVec::from_items(vec![1, 2]);
        let sequence = FolSeq::from_items(vec![1, 2, 3, 4]);
        let set = FolSet::from_items(vec![3, 1, 2]);
        let map = FolMap::from_pairs(vec![(FolStr::from("ada"), 1), (FolStr::from("lin"), 2)]);

        assert_eq!(len(&text), 3);
        assert_eq!(len(&array), 3);
        assert_eq!(len(&vector), 2);
        assert_eq!(len(&sequence), 4);
        assert_eq!(len(&set), 3);
        assert_eq!(len(&map), 2);
    }
}

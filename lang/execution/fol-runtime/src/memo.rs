//! Source/API surface for the public heap-enabled `memo` tier, without hosted
//! source services. A host-compatible memo binary may still be launched by
//! the frontend; declaring bundled `standard` separately selects the effective
//! hosted runtime tier and its source APIs.

pub use crate::abi::{check_recoverable, recoverable_succeeded, FolRecover};
pub use crate::aggregate::{
    render_echo, render_entry, render_entry_debug, render_record, render_record_debug,
    FolEchoFormat, FolEntry, FolNamedValue, FolRecord,
};
pub use crate::builtins::{len, pow, pow_float, FolLength};
pub use crate::containers::{
    index_array, index_seq, index_set, index_vec, lookup_map, render_array, render_map, render_seq,
    render_set, render_vec, slice_seq, slice_vec, FolArray,
};
pub use crate::error::require;
pub use crate::shell::{
    unwrap_error_shell, unwrap_error_shell_ref, unwrap_optional_shell, unwrap_optional_shell_ref,
    FolError, FolOption,
};
pub use crate::value::{impossible, FolBool, FolChar, FolFloat, FolInt, FolNever};
pub use crate::{crate_name, CRATE_NAME};

use crate::core::{self, RuntimeTier};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::{Add, Deref},
};

pub const HAS_HEAP: bool = true;
pub const HAS_OS: bool = false;
pub const TIER: RuntimeTier = RuntimeTier::new("memo", HAS_HEAP, HAS_OS);

pub fn module_name() -> &'static str {
    "memo"
}

pub fn tier_name() -> &'static str {
    TIER.name
}

pub fn base_core_tier() -> RuntimeTier {
    core::capabilities()
}

pub fn capabilities() -> RuntimeTier {
    TIER
}

#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FolStr(String);

impl FolStr {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_inner(self) -> String {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&str> for FolStr {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for FolStr {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<FolStr> for String {
    fn from(value: FolStr) -> Self {
        value.0
    }
}

impl AsRef<str> for FolStr {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for FolStr {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Deref for FolStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl Add for FolStr {
    type Output = FolStr;

    fn add(self, rhs: FolStr) -> FolStr {
        FolStr(self.0 + &rhs.0)
    }
}

impl fmt::Display for FolStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for FolStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct FolVec<T>(Vec<T>);

impl<T> FolVec<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self(values)
    }

    pub fn from_items(values: Vec<T>) -> Self {
        Self(values)
    }

    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> From<Vec<T>> for FolVec<T> {
    fn from(values: Vec<T>) -> Self {
        Self::new(values)
    }
}

impl<T> From<FolVec<T>> for Vec<T> {
    fn from(values: FolVec<T>) -> Self {
        values.into_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct FolSeq<T>(Vec<T>);

impl<T> FolSeq<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self(values)
    }

    pub fn from_items(values: Vec<T>) -> Self {
        Self(values)
    }

    pub fn as_slice(&self) -> &[T] {
        self.0.as_slice()
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T> From<Vec<T>> for FolSeq<T> {
    fn from(values: Vec<T>) -> Self {
        Self::new(values)
    }
}

impl<T> From<FolSeq<T>> for Vec<T> {
    fn from(values: FolSeq<T>) -> Self {
        values.into_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct FolSet<T>(BTreeSet<T>);

impl<T: Ord> FolSet<T> {
    pub fn new(values: BTreeSet<T>) -> Self {
        Self(values)
    }

    pub fn from_items(values: Vec<T>) -> Self {
        Self(values.into_iter().collect())
    }

    pub fn as_set(&self) -> &BTreeSet<T> {
        &self.0
    }

    pub fn into_set(self) -> BTreeSet<T> {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }
}

impl<T: Ord> From<BTreeSet<T>> for FolSet<T> {
    fn from(values: BTreeSet<T>) -> Self {
        Self::new(values)
    }
}

impl<T: Ord> From<FolSet<T>> for BTreeSet<T> {
    fn from(values: FolSet<T>) -> Self {
        values.into_set()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct FolMap<K, V>(BTreeMap<K, V>);

impl<K: Ord, V> FolMap<K, V> {
    pub fn new(values: BTreeMap<K, V>) -> Self {
        Self(values)
    }

    pub fn from_pairs(values: Vec<(K, V)>) -> Self {
        Self(values.into_iter().collect())
    }

    pub fn as_map(&self) -> &BTreeMap<K, V> {
        &self.0
    }

    pub fn into_map(self) -> BTreeMap<K, V> {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }
}

impl<K: Ord, V> From<BTreeMap<K, V>> for FolMap<K, V> {
    fn from(values: BTreeMap<K, V>) -> Self {
        Self::new(values)
    }
}

impl<K: Ord, V> From<FolMap<K, V>> for BTreeMap<K, V> {
    fn from(values: FolMap<K, V>) -> Self {
        values.into_map()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memo_tier_marks_heap_without_os() {
        assert_eq!(module_name(), "memo");
        assert_eq!(tier_name(), "memo");
        assert!(HAS_HEAP);
        assert!(!HAS_OS);
        assert_eq!(capabilities(), TIER);
    }

    #[test]
    fn memo_tier_builds_on_core_tier() {
        assert_eq!(base_core_tier(), core::TIER);
        assert!(!base_core_tier().has_heap);
        assert!(capabilities().has_heap);
        assert_eq!(capabilities().has_os, base_core_tier().has_os);
    }

    #[test]
    fn fol_str_supports_literal_and_owned_conversions() {
        let borrowed = FolStr::from("Ada");
        let owned = FolStr::new(String::from("Lin"));

        assert_eq!(borrowed.as_str(), "Ada");
        assert_eq!(owned.as_str(), "Lin");
        assert_eq!(String::from(borrowed.clone()), "Ada");
        assert_eq!(borrowed.len(), 3);
        assert!(!owned.is_empty());
    }

    #[test]
    fn fol_str_freezes_equality_order_display_and_debug_behavior() {
        let ada = FolStr::from("Ada");
        let lin = FolStr::from("Lin");

        assert_eq!(ada, FolStr::from("Ada"));
        assert!(ada < lin);
        assert_eq!(format!("{ada}"), "Ada");
        assert_eq!(format!("{ada:?}"), "\"Ada\"");
    }

    #[test]
    fn fol_str_add_concatenates_strings() {
        let hello = FolStr::from("hello ");
        let world = FolStr::from("world");

        assert_eq!((hello + world).as_str(), "hello world");
    }

    #[test]
    fn fol_vec_wraps_owned_vector_storage() {
        let values = FolVec::new(vec![1, 2, 3]);

        assert_eq!(values.as_slice(), &[1, 2, 3]);
        assert_eq!(values.len(), 3);
        assert!(!values.is_empty());
        assert_eq!(Vec::from(values), vec![1, 2, 3]);
    }

    #[test]
    fn fol_vec_deterministic_constructor_keeps_input_order() {
        let values = FolVec::from_items(vec![3, 1, 2]);

        assert_eq!(values.as_slice(), &[3, 1, 2]);
    }

    #[test]
    fn fol_seq_wraps_owned_sequence_storage() {
        let values = FolSeq::new(vec![1, 2, 3]);

        assert_eq!(values.as_slice(), &[1, 2, 3]);
        assert_eq!(values.len(), 3);
        assert!(!values.is_empty());
        assert_eq!(Vec::from(values), vec![1, 2, 3]);
    }

    #[test]
    fn fol_seq_deterministic_constructor_keeps_input_order() {
        let values = FolSeq::from_items(vec![3, 1, 2]);

        assert_eq!(values.as_slice(), &[3, 1, 2]);
    }

    #[test]
    fn fol_set_wraps_deterministic_ordered_storage() {
        let values = FolSet::new(BTreeSet::from([3, 1, 2]));

        assert_eq!(values.len(), 3);
        assert!(values.contains(&2));
        assert_eq!(
            values.as_set().iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            BTreeSet::from(values).into_iter().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn fol_set_deterministic_constructor_sorts_and_dedupes() {
        let values = FolSet::from_items(vec![3, 1, 2, 2]);

        assert_eq!(
            values.as_set().iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn fol_map_wraps_deterministic_key_ordered_storage() {
        let values = FolMap::new(BTreeMap::from([("lin", 2), ("ada", 1)]));

        assert_eq!(values.len(), 2);
        assert!(!values.is_empty());
        assert_eq!(values.get(&"ada"), Some(&1));
        assert_eq!(
            values.as_map().keys().copied().collect::<Vec<_>>(),
            vec!["ada", "lin"]
        );
        assert_eq!(BTreeMap::from(values).get("lin"), Some(&2));
    }

    #[test]
    fn fol_map_deterministic_constructor_orders_keys_and_keeps_last_value() {
        let values = FolMap::from_pairs(vec![("lin", 2), ("ada", 1), ("lin", 4)]);

        assert_eq!(
            values.as_map().keys().copied().collect::<Vec<_>>(),
            vec!["ada", "lin"]
        );
        assert_eq!(values.get(&"lin"), Some(&4));
    }
}

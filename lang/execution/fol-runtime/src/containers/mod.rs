//! Runtime container helpers used by executable FOL programs.

use crate::{
    error::{RuntimeError, RuntimeErrorKind},
    memo::{FolMap, FolSeq, FolSet, FolVec},
    value::FolInt,
};
use std::fmt::Display;

/// Fixed-size array strategy for FOL `arr[...]`.
///
/// Arrays stay native Rust arrays in the runtime layer so generated backends can
/// rely on stable fixed-size layout without an extra wrapper type.
pub type FolArray<T, const N: usize> = [T; N];

fn normalize_slice_bound(bound: FolInt, len: usize) -> usize {
    if bound < 0 {
        let adjusted = len as FolInt + bound;
        if adjusted < 0 {
            0
        } else {
            adjusted as usize
        }
    } else {
        let b = bound as usize;
        if b > len {
            len
        } else {
            b
        }
    }
}

fn normalize_index(index: FolInt, len: usize) -> Result<usize, RuntimeError> {
    if index < 0 {
        return Err(RuntimeError::new(
            RuntimeErrorKind::InvalidInput,
            format!("index out of bounds: the len is {len} but the index is {index}"),
        ));
    }

    let index = index as usize;
    if index >= len {
        return Err(RuntimeError::new(
            RuntimeErrorKind::InvalidInput,
            format!("index out of bounds: the len is {len} but the index is {index}"),
        ));
    }

    Ok(index)
}

pub fn index_array<T, const N: usize>(
    values: &FolArray<T, N>,
    index: FolInt,
) -> Result<&T, RuntimeError> {
    let index = normalize_index(index, values.len())?;
    Ok(&values[index])
}

pub fn index_vec<T>(values: &FolVec<T>, index: FolInt) -> Result<&T, RuntimeError> {
    let index = normalize_index(index, values.len())?;
    Ok(&values.as_slice()[index])
}

pub fn index_seq<T>(values: &FolSeq<T>, index: FolInt) -> Result<&T, RuntimeError> {
    let index = normalize_index(index, values.len())?;
    Ok(&values.as_slice()[index])
}

pub fn index_set<T: Ord>(values: &FolSet<T>, index: FolInt) -> Result<&T, RuntimeError> {
    let index = normalize_index(index, values.len())?;
    values.as_set().iter().nth(index).ok_or_else(|| {
        RuntimeError::new(
            RuntimeErrorKind::InvalidInput,
            format!(
                "index out of bounds: the len is {} but the index is {index}",
                values.len()
            ),
        )
    })
}

pub fn slice_vec<T: Clone>(
    values: &FolVec<T>,
    start: FolInt,
    end: FolInt,
) -> Result<FolVec<T>, RuntimeError> {
    let len = values.len();
    let start = normalize_slice_bound(start, len);
    let end = normalize_slice_bound(end, len);
    let end = if end < start { start } else { end };
    Ok(FolVec::from_items(values.as_slice()[start..end].to_vec()))
}

pub fn slice_seq<T: Clone>(
    values: &FolSeq<T>,
    start: FolInt,
    end: FolInt,
) -> Result<FolSeq<T>, RuntimeError> {
    let len = values.len();
    let start = normalize_slice_bound(start, len);
    let end = normalize_slice_bound(end, len);
    let end = if end < start { start } else { end };
    Ok(FolSeq::from_items(values.as_slice()[start..end].to_vec()))
}

pub fn lookup_map<'a, K: Ord, V>(values: &'a FolMap<K, V>, key: &K) -> Result<&'a V, RuntimeError> {
    values
        .get(key)
        .ok_or_else(|| RuntimeError::new(RuntimeErrorKind::InvalidInput, "missing map key"))
}

fn join_rendered<I>(items: I) -> String
where
    I: IntoIterator<Item = String>,
{
    items.into_iter().collect::<Vec<_>>().join(", ")
}

pub fn render_array<T: Display, const N: usize>(values: &FolArray<T, N>) -> String {
    format!(
        "arr[{}]",
        join_rendered(values.iter().map(|value| value.to_string()))
    )
}

pub fn render_vec<T: Display>(values: &FolVec<T>) -> String {
    format!(
        "vec[{}]",
        join_rendered(values.as_slice().iter().map(|value| value.to_string()))
    )
}

pub fn render_seq<T: Display>(values: &FolSeq<T>) -> String {
    format!(
        "seq[{}]",
        join_rendered(values.as_slice().iter().map(|value| value.to_string()))
    )
}

pub fn render_set<T: Display + Ord>(values: &FolSet<T>) -> String {
    format!(
        "set{{{}}}",
        join_rendered(values.as_set().iter().map(|value| value.to_string()))
    )
}

pub fn render_map<K: Display + Ord, V: Display>(values: &FolMap<K, V>) -> String {
    format!(
        "map{{{}}}",
        join_rendered(
            values
                .as_map()
                .iter()
                .map(|(key, value)| format!("{key}: {value}"))
        )
    )
}

pub fn module_name() -> &'static str {
    "containers"
}

#[cfg(test)]
mod tests {
    use super::{
        index_array, index_seq, index_set, index_vec, lookup_map, render_array, render_map,
        render_seq, render_set, render_vec, slice_seq, slice_vec, FolArray,
    };
    use crate::error::RuntimeErrorKind;
    use crate::memo::{FolMap, FolSeq, FolSet, FolVec};

    #[test]
    fn fol_array_keeps_native_fixed_size_behavior() {
        let values: FolArray<i64, 3> = [1, 2, 3];

        assert_eq!(values.len(), 3);
        assert_eq!(values[0], 1);
        assert_eq!(values[2], 3);
        assert_eq!(values.iter().copied().sum::<i64>(), 6);
    }

    #[test]
    fn linear_heap_vectors_stay_available_through_memo() {
        let values = FolVec::new(vec![1, 2, 3]);

        assert_eq!(values.len(), 3);
    }

    #[test]
    fn linear_heap_sequences_stay_available_through_alloc() {
        let values = FolSeq::new(vec![1, 2, 3]);

        assert_eq!(values.len(), 3);
    }

    #[test]
    fn ordered_heap_sets_stay_available_through_alloc() {
        let values = FolSet::new(std::collections::BTreeSet::from([1, 2, 3]));

        assert_eq!(values.len(), 3);
    }

    #[test]
    fn ordered_heap_maps_stay_available_through_alloc() {
        let values = FolMap::new(std::collections::BTreeMap::from([("ada", 1), ("lin", 2)]));

        assert_eq!(values.len(), 2);
    }

    #[test]
    fn deterministic_container_constructors_are_available_from_public_types() {
        let vec = FolVec::from_items(vec![1, 2]);
        let seq = FolSeq::from_items(vec![1, 2]);
        let set = FolSet::from_items(vec![2, 1, 2]);
        let map = FolMap::from_pairs(vec![("lin", 2), ("ada", 1)]);

        assert_eq!(vec.as_slice(), &[1, 2]);
        assert_eq!(seq.as_slice(), &[1, 2]);
        assert_eq!(set.len(), 2);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn runtime_index_helpers_cover_container_families() {
        let array: FolArray<i64, 3> = [10, 20, 30];
        let vector = FolVec::from_items(vec![10, 20, 30]);
        let sequence = FolSeq::from_items(vec![10, 20, 30]);
        let set = FolSet::from_items(vec![30, 10, 20]);
        let map = FolMap::from_pairs(vec![("ada", 1), ("lin", 2)]);

        assert_eq!(index_array(&array, 1), Ok(&20));
        assert_eq!(index_vec(&vector, 2), Ok(&30));
        assert_eq!(index_seq(&sequence, 0), Ok(&10));
        assert_eq!(index_set(&set, 1), Ok(&20));
        assert_eq!(lookup_map(&map, &"lin"), Ok(&2));

        let failure = index_vec(&vector, -1).expect_err("negative index should fail");
        assert_eq!(failure.kind(), RuntimeErrorKind::InvalidInput);
        assert_eq!(
            failure.message(),
            "index out of bounds: the len is 3 but the index is -1"
        );
    }

    #[test]
    fn container_render_helpers_cover_all_current_v1_families() {
        let array: FolArray<i64, 3> = [1, 2, 3];
        let vector = FolVec::from_items(vec![1, 2, 3]);
        let sequence = FolSeq::from_items(vec![1, 2, 3]);
        let set = FolSet::from_items(vec![3, 1, 2]);
        let map = FolMap::from_pairs(vec![("lin", 2), ("ada", 1)]);

        assert_eq!(render_array(&array), "arr[1, 2, 3]");
        assert_eq!(render_vec(&vector), "vec[1, 2, 3]");
        assert_eq!(render_seq(&sequence), "seq[1, 2, 3]");
        assert_eq!(render_set(&set), "set{1, 2, 3}");
        assert_eq!(render_map(&map), "map{ada: 1, lin: 2}");
    }

    #[test]
    fn empty_container_invariants_stay_stable_across_v1_families() {
        let array: FolArray<i64, 0> = [];
        let vector = FolVec::<i64>::from_items(vec![]);
        let sequence = FolSeq::<i64>::from_items(vec![]);
        let set = FolSet::<i64>::from_items(vec![]);
        let map = FolMap::<&str, i64>::from_pairs(vec![]);

        assert_eq!(array.len(), 0);
        assert!(vector.is_empty());
        assert!(sequence.is_empty());
        assert!(set.is_empty());
        assert!(map.is_empty());

        assert_eq!(render_array(&array), "arr[]");
        assert_eq!(render_vec(&vector), "vec[]");
        assert_eq!(render_seq(&sequence), "seq[]");
        assert_eq!(render_set(&set), "set{}");
        assert_eq!(render_map(&map), "map{}");

        let failure = index_seq(&sequence, 0).expect_err("empty sequence access should fail");
        assert_eq!(failure.kind(), RuntimeErrorKind::InvalidInput);
        assert_eq!(
            failure.message(),
            "index out of bounds: the len is 0 but the index is 0"
        );
    }

    #[test]
    fn ordered_set_and_map_families_keep_deterministic_behavior_independent_of_insertion_order() {
        let left_set = FolSet::from_items(vec![3, 1, 2, 2]);
        let right_set = FolSet::from_items(vec![2, 3, 1]);

        let left_map = FolMap::from_pairs(vec![("lin", 2), ("ada", 1), ("lin", 4)]);
        let right_map = FolMap::from_pairs(vec![("ada", 1), ("lin", 4)]);

        assert_eq!(
            left_set.as_set().iter().copied().collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(left_set.as_set(), right_set.as_set());
        assert_eq!(render_set(&left_set), "set{1, 2, 3}");
        assert_eq!(render_set(&left_set), render_set(&right_set));

        assert_eq!(
            left_map
                .as_map()
                .iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            vec![("ada", 1), ("lin", 4)]
        );
        assert_eq!(left_map.as_map(), right_map.as_map());
        assert_eq!(render_map(&left_map), "map{ada: 1, lin: 4}");
        assert_eq!(render_map(&left_map), render_map(&right_map));
    }

    #[test]
    fn runtime_slice_helpers_cover_vec_and_seq_families() {
        let vector = FolVec::from_items(vec![10, 20, 30, 40, 50]);
        let sequence = FolSeq::from_items(vec![10, 20, 30, 40, 50]);

        assert_eq!(slice_vec(&vector, 1, 4).unwrap().as_slice(), &[20, 30, 40]);
        assert_eq!(
            slice_seq(&sequence, 1, 4).unwrap().as_slice(),
            &[20, 30, 40]
        );

        assert_eq!(
            slice_vec(&vector, 0, 5).unwrap().as_slice(),
            &[10, 20, 30, 40, 50]
        );
        assert_eq!(slice_vec(&vector, 0, 2).unwrap().as_slice(), &[10, 20]);
        assert_eq!(slice_vec(&vector, 3, 5).unwrap().as_slice(), &[40, 50]);

        // clamping: end beyond length clamps to length
        assert_eq!(
            slice_vec(&vector, 0, 100).unwrap().as_slice(),
            &[10, 20, 30, 40, 50]
        );

        // empty slice: start == end
        assert_eq!(slice_vec(&vector, 2, 2).unwrap().as_slice(), &[] as &[i64]);

        // inverted bounds: produces empty
        assert_eq!(slice_vec(&vector, 4, 2).unwrap().as_slice(), &[] as &[i64]);

        // negative bounds count from end (Python-style clamping)
        assert_eq!(slice_vec(&vector, -3, 5).unwrap().as_slice(), &[30, 40, 50]);
    }
}

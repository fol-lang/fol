use std::marker::PhantomData;

pub trait LoweringId: Copy + Eq + Ord {
    fn from_index(index: usize) -> Self;
    fn index(self) -> usize;
}

macro_rules! define_lowering_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub usize);

        impl LoweringId for $name {
            fn from_index(index: usize) -> Self {
                Self(index)
            }

            fn index(self) -> usize {
                self.0
            }
        }
    };
}

define_lowering_id!(LoweredPackageId);
define_lowering_id!(LoweredGlobalId);
define_lowering_id!(LoweredRoutineId);
define_lowering_id!(LoweredBlockId);
define_lowering_id!(LoweredLocalId);
define_lowering_id!(LoweredInstrId);
define_lowering_id!(LoweredTypeId);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdTable<I, T> {
    entries: Vec<T>,
    _marker: PhantomData<I>,
}

impl<I, T> Default for IdTable<I, T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            _marker: PhantomData,
        }
    }
}

impl<I: LoweringId, T> IdTable<I, T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, value: T) -> I {
        let id = I::from_index(self.entries.len());
        self.entries.push(value);
        id
    }

    pub fn get(&self, id: I) -> Option<&T> {
        self.entries.get(id.index())
    }

    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.entries.get_mut(id.index())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.entries.iter_mut()
    }

    pub fn iter_with_ids(&self) -> impl Iterator<Item = (I, &T)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, value)| (I::from_index(index), value))
    }
}

impl<'table, I: LoweringId, T> IntoIterator for &'table IdTable<I, T> {
    type Item = &'table T;
    type IntoIter = std::slice::Iter<'table, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        IdTable, LoweredBlockId, LoweredGlobalId, LoweredInstrId, LoweredLocalId, LoweredPackageId,
        LoweredRoutineId, LoweredTypeId, LoweringId,
    };

    #[test]
    fn lowering_ids_round_trip_indexes() {
        let ids = [
            LoweredPackageId::from_index(0).0,
            LoweredGlobalId::from_index(1).0,
            LoweredRoutineId::from_index(2).0,
            LoweredBlockId::from_index(3).0,
            LoweredLocalId::from_index(4).0,
            LoweredInstrId::from_index(5).0,
            LoweredTypeId::from_index(6).0,
        ];

        assert_eq!(ids, [0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn id_table_pushes_values_in_stable_order() {
        let mut table = IdTable::<LoweredRoutineId, &str>::new();

        let alpha = table.push("alpha");
        let beta = table.push("beta");

        assert_eq!(alpha.0, 0);
        assert_eq!(beta.0, 1);
        assert_eq!(table.get(alpha), Some(&"alpha"));
        assert_eq!(table.get(beta), Some(&"beta"));
    }

    #[test]
    fn id_table_iter_with_ids_matches_insert_order() {
        let mut table = IdTable::<LoweredBlockId, &str>::new();
        table.push("entry");
        table.push("exit");

        let collected = table
            .iter_with_ids()
            .map(|(id, value)| (id.0, *value))
            .collect::<Vec<_>>();

        assert_eq!(collected, vec![(0, "entry"), (1, "exit")]);
    }
}

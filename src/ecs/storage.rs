use rayon::prelude::*;
use std::{
    fmt::Debug,
    hash::Hash,
    ops::{Deref, DerefMut},
};

use super::{entity::EntityId, world::ComponentPtr};

pub trait Index: Copy + Clone + PartialEq + Eq + Hash + Debug + Send + Sync + 'static {
    fn index(&self) -> usize;
    fn from_usize(index: usize) -> Self;
}

macro_rules! impl_index {
    ($($name:ident),*) => {
        $(
            impl Index for $name {
                fn index(&self) -> usize {
                    *self as usize
                }

                fn from_usize(index: usize) -> Self {
                    index as Self
                }
            }
        )*
    };
}

impl_index!(u8, u16, u32, u64, usize);

#[derive(Default)]
pub struct SparseArray<I: Index, V> {
    pub values: Vec<Option<V>>,
    _phantom: std::marker::PhantomData<I>,
}

impl<I: Index, V> SparseArray<I, V> {
    pub fn new() -> Self {
        Self {
            values: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn insert(&mut self, index: I, value: V) {
        let index = index.index();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index] = Some(value);
    }

    pub fn get(&self, index: usize) -> Option<&V> {
        self.values.get(index).and_then(|v| v.as_ref())
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut V> {
        self.values.get_mut(index).and_then(|v| v.as_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = &V> {
        self.values.iter().filter_map(|v| v.as_ref())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.values.iter_mut().filter_map(|v| v.as_mut())
    }

    pub fn remove(&mut self, index: usize) -> Option<V> {
        self.values.get_mut(index).and_then(|v| v.take())
    }

    pub fn par_iter(&self) -> impl ParallelIterator<Item = &V>
    where
        V: Sync,
    {
        self.values.par_iter().filter_map(|v| v.as_ref())
    }

    pub fn par_iter_mut(&mut self) -> impl ParallelIterator<Item = &mut V>
    where
        V: Send + Sync,
    {
        self.values.par_iter_mut().filter_map(|v| v.as_mut())
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SparseSet<T, I: Index> {
    pub(crate) dense: Vec<T>,
    pub(crate) indices: Vec<I>,
    pub(crate) sparse: SparseArray<I, usize>,
}

impl<T, I> SparseSet<T, I>
where
    I: Index,
{
    pub fn new() -> Self {
        Self {
            dense: Vec::new(),
            indices: Vec::new(),
            sparse: SparseArray::new(),
        }
    }

    pub fn insert(&mut self, index: I, value: T) {
        if let Some(dense_index) = self.sparse.get(index.index()) {
            self.dense[*dense_index] = value;
        } else {
            let dense_index = self.dense.len();
            self.dense.push(value);
            self.indices.push(index);
            self.sparse.insert(index, dense_index);
        }
    }

    pub fn get_or_insert_with<F>(&mut self, index: I, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        if let Some(dense_index) = self.sparse.get(index.index()) {
            &mut self.dense[*dense_index]
        } else {
            let dense_index = self.dense.len();
            self.dense.push(f());
            self.indices.push(index);
            self.sparse.insert(index, dense_index);
            &mut self.dense[dense_index]
        }
    }

    pub fn contains(&self, index: I) -> bool {
        self.sparse.get(index.index()).is_some()
    }

    pub fn indices(&self) -> impl Iterator<Item = I> + '_ {
        self.indices.iter().copied()
    }

    pub fn values(&self) -> impl Iterator<Item = &T> + '_ {
        self.dense.iter()
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut T> + '_ {
        self.dense.iter_mut()
    }

    pub fn par_values(&self) -> impl ParallelIterator<Item = &T> + '_
    where
        T: Sync,
    {
        self.dense.par_iter()
    }

    pub fn par_values_mut(&mut self) -> impl ParallelIterator<Item = &mut T> + '_
    where
        T: Send + Sync,
    {
        self.dense.par_iter_mut()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&I, &T)> + '_ {
        self.indices.iter().zip(self.dense.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&I, &mut T)> + '_ {
        self.indices.iter().zip(self.dense.iter_mut())
    }

    pub fn par_iter(&self) -> impl ParallelIterator<Item = (&I, &T)> + '_
    where
        I: Sync,
        T: Sync,
    {
        self.indices.par_iter().zip(self.dense.par_iter())
    }

    pub fn par_iter_mut(&mut self) -> impl ParallelIterator<Item = (&I, &mut T)> + '_
    where
        I: Send + Sync,
        T: Send + Sync,
    {
        self.indices.par_iter().zip(self.dense.par_iter_mut())
    }

    #[inline(never)]
    pub fn remove(&mut self, index: I) -> Option<T> {
        if let Some(dense_index) = self.sparse.remove(index.index()) {
            let is_last = dense_index == self.dense.len() - 1;
            let value = self.dense.swap_remove(dense_index);
            self.indices.swap_remove(dense_index);
            if !is_last {
                let swapped_index = self.indices[dense_index];
                self.sparse.insert(swapped_index, dense_index);
            }
            Some(value)
        } else {
            None
        }
    }

    pub fn get(&self, index: I) -> Option<&T> {
        self.sparse
            .get(index.index())
            .and_then(|dense_index| self.dense.get(*dense_index))
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut T> {
        self.sparse
            .get(index.index())
            .and_then(|dense_index| self.dense.get_mut(*dense_index))
    }
}

impl<T, I> FromIterator<(I, T)> for SparseSet<T, I>
where
    I: Index,
{
    fn from_iter<I2>(iter: I2) -> Self
    where
        I2: IntoIterator<Item = (I, T)>,
    {
        let iter = iter.into_iter();
        let (indices, dense): (Vec<_>, Vec<_>) = iter.unzip();

        let mut sparse = SparseArray::new();
        for (dense_index, index) in indices.iter().enumerate() {
            sparse.insert(*index, dense_index);
        }

        Self {
            dense,
            indices,
            sparse,
        }
    }
}

impl<T, I> FromParallelIterator<(I, T)> for SparseSet<T, I>
where
    I: Index + Send + Sync,
    T: Send + Sync,
{
    fn from_par_iter<I2>(par_iter: I2) -> Self
    where
        I2: IntoParallelIterator<Item = (I, T)>,
    {
        let iter = par_iter.into_par_iter();
        let (indices, dense): (Vec<_>, Vec<_>) = iter.unzip();

        let mut sparse = SparseArray::new();
        for (dense_index, index) in indices.iter().enumerate() {
            sparse.insert(*index, dense_index);
        }

        Self {
            dense,
            indices,
            sparse,
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityComponents {
    pub(crate) components: SparseSet<ComponentPtr, usize>,
}

impl Deref for EntityComponents {
    type Target = SparseSet<ComponentPtr, usize>;

    fn deref(&self) -> &Self::Target {
        &self.components
    }
}

impl DerefMut for EntityComponents {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.components
    }
}

impl Default for EntityComponents {
    fn default() -> Self {
        Self {
            components: SparseSet::new(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Components {
    pub(crate) entity_components: SparseSet<EntityComponents, EntityId>,
}

impl Deref for Components {
    type Target = SparseSet<EntityComponents, EntityId>;

    fn deref(&self) -> &Self::Target {
        &self.entity_components
    }
}

impl DerefMut for Components {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entity_components
    }
}

impl Default for Components {
    fn default() -> Self {
        Self {
            entity_components: SparseSet::new(),
        }
    }
}

#[cfg(feature = "serde")]
pub(crate) mod _serde {
    use super::*;
    use rustc_hash::FxHashMap;
    use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};

    impl<I, V> Serialize for SparseArray<I, V>
    where
        I: Index + Serialize,
        V: Serialize,
    {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut s = serializer.serialize_map(Some(self.values.len()))?;
            for (i, v) in self.iter().enumerate() {
                s.serialize_entry(&i, v)?;
            }

            s.end()
        }
    }

    impl<'de, I, V> Deserialize<'de> for SparseArray<I, V>
    where
        I: Index + Deserialize<'de>,
        V: Deserialize<'de>,
    {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let mut values = SparseArray::new();
            let mut map: FxHashMap<I, V> = FxHashMap::deserialize(deserializer)?;
            for (i, v) in map.drain() {
                values.insert(i, v);
            }
            Ok(values)
        }
    }
}

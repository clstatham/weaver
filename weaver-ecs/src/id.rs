use std::{any::TypeId, collections::HashMap, hash::BuildHasherDefault, sync::atomic::AtomicU32};

use atomic_refcell::AtomicRefCell;
use rustc_hash::FxHasher;

pub type DynamicId = u32;

pub struct Registry {
    next_id: AtomicU32,
    static_ids: AtomicRefCell<TypeIdMap<DynamicId>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            static_ids: AtomicRefCell::new(HashMap::default()),
        }
    }

    #[inline]
    pub fn create(&self) -> DynamicId {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_static<T: 'static>(&self) -> DynamicId {
        if let Some(id) = self.static_ids.borrow().get(&TypeId::of::<T>()) {
            return *id;
        }

        let static_id = TypeId::of::<T>();
        let id = self.create();

        self.static_ids.borrow_mut().insert(static_id, id);
        id
    }

    pub fn split(&self) -> Self {
        let next_id = self.next_id.load(std::sync::atomic::Ordering::Relaxed);
        let static_ids = self.static_ids.borrow().clone();

        Self {
            next_id: AtomicU32::new(next_id),
            static_ids: AtomicRefCell::new(static_ids),
        }
    }

    pub fn merge(&self, other: &Self) {
        self.next_id.store(
            other.next_id.load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        self.static_ids
            .borrow_mut()
            .extend(other.static_ids.borrow().iter());
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct TypeIdHasher(u64);

impl std::hash::Hasher for TypeIdHasher {
    fn write_u64(&mut self, i: u64) {
        debug_assert_eq!(self.0, 0);
        self.0 = i;
    }

    fn write_u128(&mut self, i: u128) {
        debug_assert_eq!(self.0, 0);
        self.0 = i as u64;
    }

    fn write(&mut self, bytes: &[u8]) {
        debug_assert_eq!(self.0, 0);

        let mut hasher = FxHasher::default();
        hasher.write(bytes);
        self.0 = hasher.finish();
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub struct SortedMap<K: Ord + Copy, V>(Box<[(K, V)]>);

impl<K: Ord + Copy, V> SortedMap<K, V> {
    pub fn new(map: impl IntoIterator<Item = (K, V)>) -> Self {
        let mut vec: Vec<_> = map.into_iter().collect();
        vec.sort_unstable_by_key(|(key, _)| *key);
        Self(vec.into_boxed_slice())
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.0
            .binary_search_by_key(key, |(key, _)| *key)
            .ok()
            .map(|index| &self.0[index].1)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.0
            .binary_search_by_key(key, |(key, _)| *key)
            .ok()
            .map(|index| &mut self.0[index].1)
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.0.iter().map(|(key, value)| (*key, value))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.0.iter_mut().map(|(key, value)| (*key, value))
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.0.binary_search_by_key(key, |(key, _)| *key).is_ok()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub type TypeIdMap<T> = HashMap<TypeId, T, BuildHasherDefault<TypeIdHasher>>;

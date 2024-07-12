use std::{
    any::TypeId,
    hash::{BuildHasherDefault, Hasher},
};

#[derive(Default)]
pub struct TypeIdHasher {
    state: u64,
}

impl Hasher for TypeIdHasher {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write_u128(&mut self, i: u128) {
        self.state = i as u64;
    }

    fn write_u64(&mut self, i: u64) {
        self.state = i;
    }

    fn write(&mut self, _bytes: &[u8]) {
        unimplemented!("TypeIdHasher should not be used with anything other than TypeId")
    }
}

pub type TypeIdMap<T> = hashbrown::HashMap<TypeId, T, BuildHasherDefault<TypeIdHasher>>;

pub type FxHashMap<K, V> = hashbrown::HashMap<K, V, rustc_hash::FxBuildHasher>;
pub type FxHashSet<T> = hashbrown::HashSet<T, rustc_hash::FxBuildHasher>;

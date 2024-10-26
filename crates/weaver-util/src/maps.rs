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
pub type TypeIdSet = hashbrown::HashSet<TypeId, BuildHasherDefault<TypeIdHasher>>;

pub type FxHashMap<K, V> = hashbrown::HashMap<K, V, rustc_hash::FxBuildHasher>;
pub type FxHashSet<T> = hashbrown::HashSet<T, rustc_hash::FxBuildHasher>;

/// A const non-cryptographically secure hash function for a `u128`.
/// Uses the FNV-1a algorithm with a 64-bit seed.
///
/// This can be used to hash a `u128` (such as a UUID) into a `u64` with a good enough distribution.
pub const fn fast_hash_u128_const(a: u128) -> u64 {
    #[cfg(target_pointer_width = "64")]
    const K: usize = 0xf1357aea2e62a9c5;
    #[cfg(target_pointer_width = "32")]
    const K: usize = 0x93d765dd;

    let mut state = 0u64;
    state = state.wrapping_add(a as u64).wrapping_mul(K as u64);
    state = state.wrapping_add((a >> 64) as u64).wrapping_mul(K as u64);
    state
}

use std::{
    any::TypeId,
    hash::{BuildHasherDefault, Hasher},
};

pub mod lock;

pub mod prelude {
    pub use crate::lock::*;
    pub use crate::TypeIdMap;
    pub use anyhow::{anyhow, bail, ensure, Error, Result};
    pub use downcast_rs::{impl_downcast, Downcast, DowncastSync};
}

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

pub type TypeIdMap<T> =
    std::collections::hash_map::HashMap<TypeId, T, BuildHasherDefault<TypeIdHasher>>;

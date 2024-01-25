use std::{
    any::TypeId,
    collections::HashMap,
    hash::BuildHasherDefault,
    sync::{atomic::AtomicU32, Arc},
};

use atomic_refcell::AtomicRefCell;
use rustc_hash::{FxHashMap, FxHasher};

use crate::{component::MethodWrapper, component_impl::register_all, prelude::Component};

pub type DynamicId = u32;

pub struct Registry {
    next_id: AtomicU32,
    static_ids: AtomicRefCell<TypeIdMap<DynamicId>>,
    named_ids: AtomicRefCell<FxHashMap<String, DynamicId>>,
    id_names: AtomicRefCell<FxHashMap<DynamicId, String>>,
    methods: AtomicRefCell<FxHashMap<DynamicId, FxHashMap<String, MethodWrapper>>>,
}

impl Registry {
    pub fn new() -> Arc<Self> {
        let registry = Self {
            next_id: AtomicU32::new(1),
            static_ids: AtomicRefCell::new(HashMap::default()),
            named_ids: AtomicRefCell::new(FxHashMap::default()),
            id_names: AtomicRefCell::new(FxHashMap::default()),
            methods: AtomicRefCell::new(FxHashMap::default()),
        };

        let registry = Arc::new(registry);

        // register builtin types
        register_all(&registry);

        registry
    }

    #[inline]
    pub fn create(&self) -> DynamicId {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get_static<T: Component>(&self) -> DynamicId {
        if let Some(id) = self.static_ids.borrow().get(&TypeId::of::<T>()) {
            return *id;
        }

        let static_id = TypeId::of::<T>();
        let id = self.create();

        self.static_ids.borrow_mut().insert(static_id, id);

        let name = self.static_name::<T>();
        self.named_ids.borrow_mut().insert(name.to_string(), id);
        self.id_names.borrow_mut().insert(id, name.to_string());

        id
    }

    pub fn get_named(&self, name: &str) -> DynamicId {
        if let Some(id) = self.named_ids.borrow().get(name) {
            return *id;
        }

        let id = self.create();

        self.named_ids.borrow_mut().insert(name.to_string(), id);
        self.id_names.borrow_mut().insert(id, name.to_string());
        id
    }

    pub fn method_by_id(&self, id: DynamicId, name: &str) -> Option<MethodWrapper> {
        self.methods
            .borrow()
            .get(&id)
            .and_then(|methods| methods.get(name))
            .cloned()
    }

    pub fn method_by_name(&self, ty: &str, name: &str) -> Option<MethodWrapper> {
        let id = self.get_named(ty);
        self.methods
            .borrow()
            .get(&id)
            .and_then(|methods| methods.get(name))
            .cloned()
    }

    pub fn methods_registered(&self, id: DynamicId) -> bool {
        self.methods.borrow().contains_key(&id)
    }

    pub fn register_methods(
        &self,
        id: DynamicId,
        methods: impl IntoIterator<Item = MethodWrapper>,
    ) {
        if self.methods.borrow().contains_key(&id) {
            #[cfg(debug_assertions)]
            {
                for method in methods.into_iter() {
                    if !self
                        .methods
                        .borrow()
                        .get(&id)
                        .unwrap()
                        .contains_key(method.name())
                    {
                        log::warn!(
                            "Method {} will not be registered for id {}",
                            method.name(),
                            id
                        );
                    }
                }
            }
            return;
        }
        for method in methods {
            self.methods
                .borrow_mut()
                .entry(id)
                .or_default()
                .insert(method.name().to_string(), method);
        }
    }

    pub fn split(&self) -> Self {
        let next_id = self.next_id.load(std::sync::atomic::Ordering::Relaxed);
        let static_ids = self.static_ids.borrow().clone();
        let named_ids = self.named_ids.borrow().clone();
        let id_names = self.id_names.borrow().clone();
        let methods = self.methods.borrow().clone();

        Self {
            next_id: AtomicU32::new(next_id),
            static_ids: AtomicRefCell::new(static_ids),
            named_ids: AtomicRefCell::new(named_ids),
            id_names: AtomicRefCell::new(id_names),
            methods: AtomicRefCell::new(methods),
        }
    }

    pub fn merge(&self, other: &Self) {
        self.next_id.store(
            other.next_id.load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        self.static_ids
            .borrow_mut()
            .extend(other.static_ids.borrow().clone());
        self.named_ids
            .borrow_mut()
            .extend(other.named_ids.borrow().clone());
        self.id_names
            .borrow_mut()
            .extend(other.id_names.borrow().clone());
        self.methods
            .borrow_mut()
            .extend(other.methods.borrow().clone());
    }

    pub fn static_name<T: Component>(&self) -> &'static str {
        T::type_name()
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

#[derive(Debug)]
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

use std::sync::{Arc, RwLock};

use rustc_hash::FxHashMap;

use crate::{
    query::Write,
    resource::{Res, ResMut},
    Bundle, Component, Entity, Read, Resource, System,
};

#[derive(Default)]
pub struct World {
    pub(crate) entities_components: FxHashMap<Entity, Vec<Arc<RwLock<dyn Component>>>>,
    pub(crate) systems: Vec<Box<dyn System>>,
    pub(crate) resources: FxHashMap<u64, Arc<RwLock<dyn crate::resource::Resource>>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_entity(&mut self) -> Entity {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Entity::new(id, 0)
    }

    pub fn build<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(self)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let component = Arc::new(RwLock::new(component));
        self.entities_components
            .entry(entity)
            .or_default()
            .push(component);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.entities_components.get_mut(&entity) {
            components.retain(|component| !component.read().unwrap().as_any().is::<T>());
        }
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) {
        let resource = Arc::new(RwLock::new(resource));
        self.resources.insert(T::resource_id(), resource);
    }

    pub fn read_resource<T: Resource>(&self) -> Res<T> {
        let resource = self.resources.get(&T::resource_id()).unwrap();
        Res::new(resource.read().unwrap())
    }

    pub fn write_resource<T: Resource>(&self) -> ResMut<T> {
        let resource = self.resources.get(&T::resource_id()).unwrap();
        ResMut::new(resource.write().unwrap())
    }

    pub fn read<T: Component>(&self) -> Read<'_, T> {
        Read::new(self)
    }

    pub fn write<T: Component>(&self) -> Write<'_, T> {
        Write::new(self)
    }

    pub fn query<'w, 'q, Q: crate::query::Queryable<'w, 'q>>(&'w self) -> Q
    where
        'w: 'q,
    {
        Q::create(self)
    }

    pub fn add_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    pub fn update(&self) {
        for system in &self.systems {
            system.run(self);
        }
    }
}

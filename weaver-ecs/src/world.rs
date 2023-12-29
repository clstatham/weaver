use std::sync::{Arc, RwLock};

use rustc_hash::FxHashMap;

use crate::{
    resource::{Res, ResMut},
    Bundle, Component, Entity, Resource, System,
};

#[derive(Default)]
pub struct World {
    next_entity_id: u32,
    pub(crate) entities_components: FxHashMap<Entity, FxHashMap<u64, Arc<RwLock<dyn Component>>>>,
    pub(crate) systems: Vec<Box<dyn System>>,
    pub(crate) resources: FxHashMap<u64, Arc<RwLock<dyn crate::resource::Resource>>>,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_entity(&mut self) -> Entity {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        let entity = Entity::new(id, 0);
        self.entities_components
            .insert(entity, FxHashMap::default());
        entity
    }

    pub fn spawn<T: Bundle>(&mut self, bundle: T) -> Entity {
        bundle.build(self)
    }

    pub fn add_component<T: Component>(&mut self, entity: Entity, component: T) {
        let component = Arc::new(RwLock::new(component));
        self.entities_components
            .entry(entity)
            .or_default()
            .insert(T::component_id(), component);
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.entities_components.get_mut(&entity) {
            components.remove(&T::component_id());
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

    pub fn query<'w, 'q, 'i, Q: crate::query::Queryable<'w, 'q, 'i>>(&'w self) -> Q
    where
        'w: 'q,
        'q: 'i,
    {
        Q::create(self)
    }

    pub fn add_system<S: System + 'static>(&mut self, system: S) {
        self.systems.push(Box::new(system));
    }

    pub fn update(&mut self) {
        for system in &self.systems {
            system.run(self);
        }
    }
}

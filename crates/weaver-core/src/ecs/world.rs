use std::sync::atomic::{AtomicU32, Ordering};

use weaver_util::lock::Lock;

use super::{
    component::Component,
    entity::Entity,
    query::{Query, QueryResults},
    storage::{Mut, Ref, Storage},
};

pub struct World {
    resource_entity: Entity,
    next_entity: AtomicU32,
    free_entities: Lock<Vec<Entity>>,
    storage: Lock<Storage>,
}

impl Default for World {
    fn default() -> Self {
        let mut world = Self {
            resource_entity: Entity::new(0, 0),
            next_entity: AtomicU32::new(0),
            free_entities: Lock::new(Vec::new()),
            storage: Lock::new(Storage::new()),
        };

        world.resource_entity = world.create_entity(); // reserve entity 0 for the world itself

        world
    }
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn storage(&self) -> &Lock<Storage> {
        &self.storage
    }

    pub fn create_entity(&self) -> Entity {
        if let Some(entity) = self.free_entities.write().pop() {
            Entity::new(entity.id(), entity.generation() + 1)
        } else {
            let id = self.next_entity.fetch_add(1, Ordering::Relaxed);
            Entity::new(id, 0)
        }
    }

    pub fn destroy_entity(&self, entity: Entity) {
        self.storage.write().remove_entity(entity);
        self.free_entities.write().push(entity);
    }

    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.storage.write().insert_component(entity, component)
    }

    pub fn remove_component<T: Component>(&self, entity: Entity) -> Option<T> {
        self.storage.write().remove_component::<T>(entity)
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<Ref<T>> {
        self.storage.read().get_component::<T>(entity)
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<Mut<T>> {
        self.storage.read().get_component_mut::<T>(entity)
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        self.storage.read().has_component::<T>(entity)
    }

    pub fn query<'a>(&'a self, query: &Query) -> QueryResults<'a> {
        query.get(self)
    }

    pub const fn resource_entity(&self) -> Entity {
        self.resource_entity
    }

    pub fn get_resource<T: Component>(&self) -> Option<Ref<T>> {
        self.get_component::<T>(self.resource_entity())
    }

    pub fn get_resource_mut<T: Component>(&self) -> Option<Mut<T>> {
        self.get_component_mut::<T>(self.resource_entity())
    }

    pub fn has_resource<T: Component>(&self) -> bool {
        self.has_component::<T>(self.resource_entity())
    }

    pub fn insert_resource<T: Component>(&self, component: T) {
        self.insert_component(self.resource_entity(), component)
    }

    pub fn remove_resource<T: Component>(&self) -> Option<T> {
        self.remove_component::<T>(self.resource_entity())
    }
}

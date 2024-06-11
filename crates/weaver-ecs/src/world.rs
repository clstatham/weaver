use std::{
    rc::Rc,
    sync::atomic::{AtomicU32, Ordering},
};

use weaver_util::lock::Lock;

use crate::prelude::{Bundle, Query, QueryFilter, Scene};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref, Storage},
};

pub struct World {
    resource_entity: Entity,
    root_scene_entity: Entity,
    next_entity: AtomicU32,
    free_entities: Lock<Vec<Entity>>,
    storage: Lock<Storage>,
}

impl World {
    pub fn new() -> Rc<Self> {
        let mut world = Self {
            resource_entity: Entity::new(0, 0),
            root_scene_entity: Entity::new(1, 0),
            next_entity: AtomicU32::new(0),
            free_entities: Lock::new(Vec::new()),
            storage: Lock::new(Storage::new()),
        };

        world.resource_entity = world.create_entity(); // reserve entity 0 for the world itself
        world.root_scene_entity = world.create_entity(); // reserve entity 1 for the root scene

        let world = Rc::new(world);

        let root_scene = Scene::new(world.clone());
        world.insert_component(world.root_scene_entity(), root_scene);

        world
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

    pub fn spawn<T: Bundle>(&self, bundle: T) -> Entity {
        let entity = self.create_entity();
        self.storage().write().insert_components(entity, bundle);
        entity
    }

    pub fn destroy_entity(&self, entity: Entity) {
        self.storage.write().remove_entity(entity);
        self.free_entities.write().push(entity);
    }

    pub fn insert_component<T: Component>(&self, entity: Entity, component: T) {
        self.storage.write().insert_component(entity, component)
    }

    pub fn insert_components<T: Bundle>(&self, entity: Entity, bundle: T) {
        self.storage.write().insert_components(entity, bundle)
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

    pub fn query<Q: QueryFilter>(&self) -> Query<Q> {
        Query::new(self)
    }

    pub const fn resource_entity(&self) -> Entity {
        self.resource_entity
    }

    pub const fn root_scene_entity(&self) -> Entity {
        self.root_scene_entity
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

    pub fn root_scene(&self) -> Ref<Scene> {
        self.get_component::<Scene>(self.root_scene_entity())
            .unwrap()
    }
}

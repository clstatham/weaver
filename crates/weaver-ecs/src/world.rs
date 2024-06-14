use std::sync::{
    atomic::{AtomicU32, AtomicU64, Ordering},
    Arc,
};

use weaver_util::lock::Lock;

use crate::prelude::{
    Bundle, Query, QueryFetch, QueryFilter, Res, ResMut, Resource, Resources, Scene, Tick,
};

use super::{
    component::Component,
    entity::Entity,
    storage::{Mut, Ref, Storage},
};

pub struct World {
    root_scene_entity: Entity,
    next_entity: AtomicU32,
    free_entities: Lock<Vec<Entity>>,
    storage: Lock<Storage>,
    resources: Lock<Resources>,
    update_tick: AtomicU64,
}

impl World {
    pub fn new() -> Arc<Self> {
        let mut world = Self {
            root_scene_entity: Entity::new(1, 0),
            next_entity: AtomicU32::new(0),
            free_entities: Lock::new(Vec::new()),
            storage: Lock::new(Storage::new()),
            resources: Lock::new(Resources::default()),
            update_tick: AtomicU64::new(0),
        };

        world.root_scene_entity = world.create_entity(); // reserve entity 0 for the root scene

        let world = Arc::new(world);

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

    pub fn query<'a, Q: QueryFetch<'a>>(self: &Arc<Self>) -> Query<'a, Q, ()> {
        Query::new(self)
    }

    pub fn query_filtered<'a, Q: QueryFetch<'a>, F: QueryFilter>(
        self: &Arc<Self>,
    ) -> Query<'a, Q, F> {
        Query::new(self)
    }

    pub const fn root_scene_entity(&self) -> Entity {
        self.root_scene_entity
    }

    pub fn get_resource<T: Resource>(&self) -> Option<Res<T>> {
        self.resources.read().get::<T>()
    }

    pub fn get_resource_mut<T: Resource>(&self) -> Option<ResMut<T>> {
        self.resources.read().get_mut::<T>()
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.read().contains::<T>()
    }

    pub fn insert_resource<T: Resource>(&self, component: T) {
        self.resources.write().insert(component)
    }

    pub fn remove_resource<T: Resource>(&self) -> Option<T> {
        self.resources.write().remove::<T>()
    }

    pub fn root_scene(&self) -> Ref<Scene> {
        self.get_component::<Scene>(self.root_scene_entity())
            .unwrap()
    }

    pub fn update_tick(&self) -> Tick {
        Tick::new(self.update_tick.load(Ordering::Acquire))
    }

    pub fn update(&self) {
        self.update_tick.fetch_add(1, Ordering::AcqRel);
    }
}

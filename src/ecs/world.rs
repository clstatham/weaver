use std::sync::{atomic::AtomicU32, Arc};

#[cfg(feature = "serde")]
use std::io::{Read, Write};

use parking_lot::RwLock;
use rustc_hash::FxHashMap;

use super::{
    resource::{Res, ResMut, Resource},
    storage::{Components, EntityComponents},
    system::{SystemGraph, SystemId, SystemStage},
    Bundle, Component, Entity, System,
};

#[derive(Clone)]
pub struct ComponentPtr {
    pub component_id: usize,
    pub component: Arc<RwLock<Box<dyn Component>>>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for ComponentPtr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("ComponentPtr", 2)?;
        state.serialize_field("component_id", &self.component_id)?;
        state.serialize_field("component", &*self.component.read())?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ComponentPtr {
    fn deserialize<D>(deserializer: D) -> Result<ComponentPtr, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct ComponentPtrHelper {
            component_id: usize,
            component: Box<dyn Component>,
        }

        let helper = ComponentPtrHelper::deserialize(deserializer)?;

        Ok(ComponentPtr {
            component_id: helper.component_id,
            component: Arc::new(RwLock::new(helper.component)),
        })
    }
}

pub struct World {
    next_entity_id: AtomicU32,
    pub(crate) components: Arc<RwLock<Components>>,
    pub(crate) systems: FxHashMap<SystemStage, RwLock<SystemGraph>>,
    pub(crate) resources: FxHashMap<usize, Arc<RwLock<dyn Resource>>>,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_entity_id: AtomicU32::new(0),
            components: Arc::new(RwLock::new(Components::new())),
            systems: FxHashMap::default(),
            resources: FxHashMap::default(),
        }
    }

    pub fn create_entity(&self) -> Entity {
        let id = self
            .next_entity_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let entity = Entity::new(id, 0);
        self.components.write().insert(id, EntityComponents::new());
        entity
    }

    pub fn spawn<T: Bundle>(&self, bundle: T) -> anyhow::Result<Entity> {
        bundle.build(self)
    }

    pub fn add_component<T: Component>(
        &self,
        entity: Entity,
        component: Box<dyn Component>,
    ) -> anyhow::Result<()> {
        if self.has_component::<T>(entity) {
            return Err(anyhow::anyhow!("Component already exists"));
        }
        let component = Arc::new(RwLock::new(component));
        let mut components = self.components.write();
        if let Some(entity_components) = components.get_mut(entity.id()) {
            entity_components.insert(
                T::static_id(),
                ComponentPtr {
                    component_id: T::static_id(),
                    component,
                },
            );
        } else {
            return Err(anyhow::anyhow!("Entity does not exist"));
        }
        Ok(())
    }

    pub fn remove_component<T: Component>(&mut self, entity: Entity) {
        if let Some(components) = self.components.write().get_mut(entity.id()) {
            components.remove(T::static_id());
        }
    }

    pub fn has_component<T: Component>(&self, entity: Entity) -> bool {
        if let Some(components) = self.components.read().get(entity.id()) {
            components.contains(T::static_id())
        } else {
            false
        }
    }

    pub fn remove_entity(&self, entity: Entity) {
        self.components.write().remove(entity.id());
    }

    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> anyhow::Result<()> {
        if self.has_resource::<T>() {
            return Err(anyhow::anyhow!("Resource already exists"));
        }
        let resource = Arc::new(RwLock::new(resource));
        self.resources.insert(T::static_id(), resource);
        Ok(())
    }

    pub fn read_resource<T: Resource>(&self) -> anyhow::Result<Res<T>> {
        let resource = self
            .resources
            .get(&T::static_id())
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;
        Ok(Res::new(resource.read()))
    }

    pub fn write_resource<T: Resource>(&self) -> anyhow::Result<ResMut<T>> {
        let resource = self
            .resources
            .get(&T::static_id())
            .ok_or(anyhow::anyhow!("Resource does not exist"))?;

        Ok(ResMut::new(resource.write()))
    }

    pub fn has_resource<T: Resource>(&self) -> bool {
        self.resources.contains_key(&T::static_id())
    }

    pub fn add_system<T: System + 'static>(&mut self, system: T) -> SystemId {
        self.add_system_to_stage(system, SystemStage::default())
    }

    pub fn add_system_to_stage<T: System + 'static>(
        &mut self,
        system: T,
        stage: SystemStage,
    ) -> SystemId {
        let system = Arc::new(system);

        self.systems
            .entry(stage)
            .or_default()
            .write()
            .add_system(system)
    }

    pub fn run_stage(world: &Arc<RwLock<Self>>, stage: SystemStage) -> anyhow::Result<()> {
        if let Some(systems) = world.read().systems.get(&stage) {
            systems.write().autodetect_dependencies()?;
            systems.read().run_parallel(world)?;
        }
        Ok(())
    }

    #[cfg(feature = "serde")]
    pub fn to_file(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        let bytes = postcard::to_allocvec(self).unwrap();
        writer.write_all(&bytes)?;
        Ok(())
    }

    #[cfg(feature = "serde")]
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(path)?;
        let mut bytes = Vec::new();
        let mut reader = std::io::BufReader::new(file);
        reader.read_to_end(&mut bytes)?;
        let world = postcard::from_bytes(&bytes).unwrap();
        Ok(world)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for World {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("World", 2)?;

        state.serialize_field(
            "next_entity_id",
            &self
                .next_entity_id
                .load(std::sync::atomic::Ordering::Relaxed),
        )?;

        state.serialize_field("components", &*self.components.read())?;

        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for World {
    fn deserialize<D>(deserializer: D) -> Result<World, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct WorldHelper {
            next_entity_id: u32,
            components: Components,
        }

        let helper = WorldHelper::deserialize(deserializer)?;

        let mut world = World::new();
        world.next_entity_id = AtomicU32::new(helper.next_entity_id);
        *world.components.write() = helper.components;

        Ok(world)
    }
}

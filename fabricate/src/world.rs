use std::sync::OnceLock;

use anyhow::Result;

use crate::{
    commands::Commands,
    component::Component,
    lock::SharedLock,
    prelude::{Bundle, Mut},
    query::{Query, QueryBuilder},
    registry::{Entity, Id},
    relationship::Relationship,
    script::{interp::BuildOnWorld, Script},
    storage::{Data, Ref, SortedMap, StaticMut, StaticRef, Storage},
    system::{DynamicSystem, SystemGraph, SystemStage},
};

static WORLD: OnceLock<LockedWorldHandle> = OnceLock::new();

pub fn get_world() -> &'static LockedWorldHandle {
    WORLD.get_or_init(World::new_handle)
}

/// A container for all the data in the game world.
/// Contains a [`Storage`] for all the entities and components.
pub struct World {
    storage: Storage,
    systems: SortedMap<Entity, DynamicSystem>,
    system_graphs: SortedMap<SystemStage, SystemGraph>,
    root: Entity,
}

impl World {
    /// Creates a new [`World`] with the global registry and a default [`Storage`], wrapped in a [`LockedWorldHandle`].
    pub fn new_handle() -> LockedWorldHandle {
        let mut storage = Storage::new();
        let root = storage.create_entity();
        let world = Self {
            storage,
            systems: SortedMap::default(),
            system_graphs: SortedMap::default(),
            root,
        };
        LockedWorldHandle::new(world)
    }

    /// Returns a reference to the component [`Storage`] in the [`World`].
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    /// Returns the root entity of the [`World`], representing the [`World`] itself.
    pub fn root(&self) -> Entity {
        self.root
    }

    pub fn add_resource<T: Component>(&mut self, resource: T) -> Result<()> {
        self.add(self.root, resource)
    }

    pub fn get_resource(&self, id: Entity) -> Option<Ref<'_>> {
        self.get(self.root, id)
    }

    pub fn get_resource_mut(&self, id: Entity) -> Option<Mut<'_>> {
        self.get_mut(self.root, id)
    }

    pub fn read_resource<T: Component>(&self) -> Option<StaticRef<'_, T>> {
        self.get_component::<T>(self.root)
    }

    pub fn write_resource<T: Component>(&self) -> Option<StaticMut<'_, T>> {
        self.get_component_mut::<T>(self.root)
    }

    pub fn insert_entity(&mut self, entity: Entity) -> Result<()> {
        self.storage.insert_entity(entity)?;
        // self.add_relative(entity, BelongsToWorld, self.root)?;
        Ok(())
    }

    /// Creates a new entity in the [`World`].
    pub fn create_entity(&mut self) -> Result<Entity> {
        let e = self.storage.create_entity();
        // self.add_relative(e, BelongsToWorld, self.root)?;
        Ok(e)
    }

    pub fn all_entities(&self) -> Vec<Entity> {
        self.storage
            .archetypes()
            .flat_map(|a| a.entity_iter())
            .collect()
    }

    pub fn add_relative<R: Relationship>(
        &mut self,
        add_to: Entity,
        relationship: R,
        relative: Entity,
    ) -> Result<()> {
        let relationship_data = relationship.into_relationship_data(relative);
        self.add_data(add_to, [relationship_data])?;
        Ok(())
    }

    /// Removes an entity from the [`World`].
    pub fn despawn(&mut self, entity: Entity) -> Option<Vec<Data>> {
        self.storage.destroy_entity(entity)
    }

    /// Creates a new entity in the [`World`] with the given [`Bundle`] of components.
    pub fn spawn<B: Bundle>(&mut self, components: B) -> Result<Entity> {
        let entity = self.create_entity()?;
        let data = components.into_data_vec();
        self.add_data(entity, data)?;
        Ok(entity)
    }

    pub fn get_relatives_id(&self, entity: Entity, relationship_type: Id) -> Option<Vec<Entity>> {
        let archetype = self.storage().entity_archetype(entity)?;
        let relationships =
            archetype.row_type_filtered(entity, |ty| ty.id() == relationship_type)?;
        let mut out = Vec::new();
        for relationship in relationships {
            let relationship_type = relationship.type_id();
            out.push(relationship_type.relationship_second()?);
        }
        Some(out)
    }

    pub fn get_relatives<R: Relationship>(&self, entity: Entity) -> Option<Vec<Entity>> {
        let relationship_type = R::type_id();
        self.get_relatives_id(entity, relationship_type.id())
    }

    pub fn all_relatives(&self, entity: Entity) -> Option<Vec<(Id, Entity)>> {
        let archetype = self.storage().entity_archetype(entity)?;
        let relationships = archetype.row_type_filtered(entity, |ty| ty.is_relative())?;
        let mut out = Vec::new();
        for relationship in relationships {
            let relationship_type = relationship.type_id();
            out.push((
                relationship_type.id(),
                relationship_type.relationship_second()?,
            ));
        }
        Some(out)
    }

    pub fn get_component<T: Component>(&self, entity: Entity) -> Option<StaticRef<'_, T>> {
        self.storage.get_component::<T>(entity)
    }

    pub fn get_component_mut<T: Component>(&self, entity: Entity) -> Option<StaticMut<'_, T>> {
        self.storage.get_component_mut::<T>(entity)
    }

    pub fn add_data(&mut self, entity: Entity, data: impl IntoIterator<Item = Data>) -> Result<()> {
        let data = data.into_iter().collect::<Vec<_>>();
        self.storage.insert(entity, data)?;
        Ok(())
    }

    pub fn add<T: Bundle>(&mut self, entity: Entity, component: T) -> Result<()> {
        let data = component.into_data_vec();
        self.add_data(entity, data)?;
        Ok(())
    }

    pub fn has<T: Component>(&self, entity: Entity) -> bool {
        self.storage.has::<T>(entity)
    }

    pub fn get(&self, entity: Entity, component_type: Entity) -> Option<Ref<'_>> {
        self.storage.get(component_type, entity)
    }

    pub fn get_mut(&self, entity: Entity, component_type: Entity) -> Option<Mut<'_>> {
        self.storage.get_mut(component_type, entity)
    }

    /// Queries the [`World`] for entities with certain components.
    pub fn query(&self) -> QueryBuilder {
        QueryBuilder::new(self)
    }

    pub fn garbage_collect(&mut self) {
        self.storage.garbage_collect();
    }

    pub fn get_system(&self, system: Entity) -> Option<&DynamicSystem> {
        self.systems.get(&system)
    }

    pub fn add_system(
        &mut self,
        stage: SystemStage,
        system: impl Fn(LockedWorldHandle) + Send + Sync + 'static,
    ) {
        let id = Entity::allocate(None);
        let system = DynamicSystem::new(move |world, _| {
            system(world);
            Ok(Vec::new())
        });
        self.systems.insert(id, system);
        if let Some(graph) = self.system_graphs.get_mut(&stage) {
            graph.add_system(id);
        } else {
            let mut graph = SystemGraph::default();
            graph.add_system(id);
            self.system_graphs.insert(stage, graph);
        }
    }
}

/// A shared handle to a [`World`] that can be locked for reading or writing.
#[derive(Clone)]
pub struct LockedWorldHandle(SharedLock<World>, SharedLock<Commands>);

impl LockedWorldHandle {
    pub fn new(world: World) -> Self {
        Self(SharedLock::new(world), SharedLock::new(Commands::new()))
    }

    pub fn defer<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&World, &mut Commands) -> R,
    {
        let world = self
            .0
            .try_read()
            .ok_or_else(|| anyhow::anyhow!("Defer failed: World already locked"))?;
        let mut commands = Commands::new();
        let result = f(&world, &mut commands);
        drop(world);

        self.1
            .write()
            .queue
            .write()
            .extend(commands.queue.write().drain(..));

        if let Some(mut world) = self.0.try_write() {
            self.1.write().finalize(&mut world)?;
        }
        Ok(result)
    }

    pub fn run_systems(&self, stage: SystemStage) {
        let world = self.0.read();
        if let Some(graph) = world.system_graphs.get(&stage).cloned() {
            drop(world);
            graph.run(self.clone());
        }
    }

    pub fn add_script(&self, script: Script) {
        script.build_on_world(self.clone()).unwrap();
    }

    pub fn add_system(
        &self,
        stage: SystemStage,
        system: impl Fn(LockedWorldHandle) + Send + Sync + 'static,
    ) -> Result<()> {
        self.defer(|_, commands| {
            commands.add_system(stage, system);
        })
    }

    pub fn add_resource<T: Component>(&self, resource: T) -> Result<()> {
        let data = resource.into_data_vec();
        self.defer(|world, commands| {
            commands.add(world.root, data);
        })
    }

    pub fn with_resource_id<F, R>(&self, id: Entity, f: F) -> Option<R>
    where
        F: FnOnce(Ref<'_>) -> R,
    {
        let world = self.0.read();
        let res = world.get_resource(id)?;
        Some(f(res))
    }

    pub fn with_resource_id_mut<F, R>(&self, id: Entity, f: F) -> Option<R>
    where
        F: FnOnce(Mut<'_>) -> R,
    {
        let world = self.0.read();
        let res = world.get_resource_mut(id)?;
        Some(f(res))
    }

    pub fn with_resource<T: Component, F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(StaticRef<'_, T>) -> R,
    {
        self.defer(|world, _| {
            let res = world.read_resource::<T>()?;
            Some(f(res))
        })?
        .ok_or_else(|| anyhow::anyhow!("Resource not found"))
    }

    pub fn with_resource_mut<T: Component, F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(StaticMut<'_, T>) -> R,
    {
        self.defer(|world, _| {
            let res = world.write_resource::<T>()?;
            Some(f(res))
        })?
        .ok_or_else(|| anyhow::anyhow!("Resource not found"))
    }

    pub fn create_entity(&self) -> Result<Entity> {
        self.defer(|_, commands| commands.create_entity())
    }

    pub fn despawn(&self, entity: Entity) -> Result<()> {
        self.defer(|_, commands| {
            commands.despawn(entity);
        })
    }

    pub fn spawn<B: Bundle>(&self, components: B) -> Result<Entity> {
        self.defer(|_, commands| {
            let entity = commands.create_entity();
            commands.add(entity, components.into_data_vec());
            Ok(entity)
        })?
    }

    pub fn get_relatives_id(&self, entity: Entity, relationship_type: Id) -> Option<Vec<Entity>> {
        let world = self.0.read();
        world.get_relatives_id(entity, relationship_type)
    }

    pub fn get_relatives<R: Relationship>(&self, entity: Entity) -> Option<Vec<Entity>> {
        let world = self.0.read();
        world.get_relatives::<R>(entity)
    }

    pub fn all_relatives(&self, entity: Entity) -> Option<Vec<(Id, Entity)>> {
        let world = self.0.read();
        world.all_relatives(entity)
    }

    pub fn with_component_ref<T: Component, F, R>(&self, entity: Entity, f: F) -> Option<R>
    where
        F: FnOnce(StaticRef<'_, T>) -> R,
    {
        let world = self.0.read();
        let c = world.get_component::<T>(entity)?;
        Some(f(c))
    }

    pub fn with_component_mut<T: Component, F, R>(&self, entity: Entity, f: F) -> Option<R>
    where
        F: FnOnce(StaticMut<'_, T>) -> R,
    {
        let world = self.0.read();
        let c = world.get_component_mut::<T>(entity)?;
        Some(f(c))
    }

    pub fn with_component_id_ref<F, R>(
        &self,
        entity: Entity,
        component_type: Entity,
        f: F,
    ) -> Option<R>
    where
        F: FnOnce(Ref<'_>) -> R,
    {
        let world = self.0.read();
        let c = world.get(entity, component_type)?;
        Some(f(c))
    }

    pub fn with_component_id_mut<F, R>(
        &self,
        entity: Entity,
        component_type: Entity,
        f: F,
    ) -> Option<R>
    where
        F: FnOnce(Mut<'_>) -> R,
    {
        let world = self.0.read();
        let c = world.get_mut(entity, component_type)?;
        Some(f(c))
    }

    pub fn add_data(&self, entity: Entity, data: impl IntoIterator<Item = Data>) -> Result<()> {
        self.defer(|_, commands| {
            commands.add(entity, data.into_iter().collect());
        })
    }

    pub fn add<T: Bundle>(&self, entity: Entity, component: T) -> Result<()> {
        self.defer(|_, commands| {
            commands.add(entity, component.into_data_vec());
        })
    }

    pub fn query<F, G, R>(&self, build_query: F, with_query: G) -> R
    where
        F: FnOnce(QueryBuilder) -> QueryBuilder,
        G: FnOnce(Query) -> R,
    {
        let world = self.0.read();
        let query = build_query(QueryBuilder::new(&world)).build();
        with_query(query)
    }

    pub fn garbage_collect(&self) -> Result<()> {
        self.defer(|_, commands| {
            commands.garbage_collect();
        })
    }

    pub fn add_relative<R: Relationship>(
        &self,
        add_to: Entity,
        relationship: R,
        relative: Entity,
    ) -> Result<()> {
        let data = relationship.into_relationship_data(relative);
        self.defer(|_, commands| {
            commands.add(add_to, vec![data]);
        })
    }

    pub fn all_entities(&self) -> Vec<Entity> {
        let world = self.0.read();
        world.all_entities()
    }

    pub fn with_system<F, R>(&self, system: Entity, f: F) -> Option<R>
    where
        F: FnOnce(&DynamicSystem) -> R,
    {
        let world = self.0.read();
        let system = world.get_system(system)?;
        Some(f(system))
    }

    pub fn has<T: Component>(&self, entity: Entity) -> bool {
        let world = self.0.read();
        world.has::<T>(entity)
    }
}

impl From<World> for LockedWorldHandle {
    fn from(world: World) -> Self {
        Self::new(world)
    }
}

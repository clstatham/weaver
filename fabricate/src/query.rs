use std::fmt::Debug;

use anyhow::{ensure, Result};

use crate::{
    component::Atom,
    registry::{Entity, StaticId},
    storage::{Mut, Ref},
    world::World,
};

#[derive(Clone, Debug, PartialEq)]
pub enum QueryBuilderAccess {
    Entity,
    Read(Entity),
    Write(Entity),
    With(Entity),
    Without(Entity),
}

/// A builder that can be used to create a [`Query`].
/// See [`World::query`] for more information.
pub struct QueryBuilder<'a> {
    pub(crate) world: &'a World,
    pub(crate) access: Vec<QueryBuilderAccess>,
}

impl<'a> QueryBuilder<'a> {
    /// Creates a new [`QueryBuilder`] for the specified [`World`].
    pub fn new(world: &'a World) -> Self {
        Self {
            world,
            access: Vec::new(),
        }
    }

    /// Requests access to the entity for the query.
    #[must_use]
    pub fn entity(mut self) -> Self {
        self.access.push(QueryBuilderAccess::Entity);
        self
    }

    /// Requests read access to a component for the query.
    pub fn read<T: Atom>(mut self) -> Result<Self> {
        let id = T::type_uid();
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Write(id)),
            "cannot read and write the same component in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Read(id)),
            "cannot read the same component twice in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::Read(id));
        Ok(self)
    }

    /// Requests write access to a component for the query.
    pub fn write<T: Atom>(mut self) -> Result<Self> {
        let id = T::type_uid();
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Read(id)),
            "cannot read and write the same component in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Write(id)),
            "cannot write the same component twice in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::Write(id));
        Ok(self)
    }

    /// Requests that the query only returns entities that have a component of the specified type, without requiring access to the component.
    pub fn with<T: Atom>(mut self) -> Result<Self> {
        let id = T::type_uid();
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Without(id)),
            "cannot include the same component twice in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::With(id)),
            "cannot include and exclude the same component in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::With(id));
        Ok(self)
    }

    /// Requests that the query only returns entities that do not have a component of the specified type.
    pub fn without<T: Atom>(mut self) -> Result<Self> {
        let id = T::type_uid();
        ensure!(
            !self.access.contains(&QueryBuilderAccess::With(id)),
            "cannot exclude the same component twice in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Without(id)),
            "cannot include and exclude the same component in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::Without(id));
        Ok(self)
    }

    /// Requests read access to a component for the query based on its type [`Uid`].
    pub fn read_dynamic(mut self, id: Entity) -> Result<Self> {
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Write(id)),
            "cannot read and write the same component in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Read(id)),
            "cannot read the same component twice in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::Read(id));
        Ok(self)
    }

    /// Requests write access to a component for the query based on its type [`Uid`].
    pub fn write_dynamic(mut self, id: Entity) -> Result<Self> {
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Read(id)),
            "cannot read and write the same component in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Write(id)),
            "cannot write the same component twice in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::Write(id));
        Ok(self)
    }

    /// Requests that the query only returns entities that have a component with the specified type id, without requiring access to the component.
    pub fn with_dynamic(mut self, id: Entity) -> Result<Self> {
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Without(id)),
            "cannot include the same component twice in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::With(id)),
            "cannot include and exclude the same component in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::With(id));
        Ok(self)
    }

    /// Requests that the query only returns entities that do not have a component with the specified type id.
    pub fn without_dynamic(mut self, id: Entity) -> Result<Self> {
        ensure!(
            !self.access.contains(&QueryBuilderAccess::With(id)),
            "cannot exclude the same component twice in a query: {:?}",
            id
        );
        ensure!(
            !self.access.contains(&QueryBuilderAccess::Without(id)),
            "cannot include and exclude the same component in a query: {:?}",
            id
        );
        self.access.push(QueryBuilderAccess::Without(id));
        Ok(self)
    }

    /// Consumes the [`QueryBuilder`], returning a [`Query`] that can be used to iterate over the query results.
    pub fn build(self) -> Query<'a> {
        let mut entities = Vec::new();

        let mut reads = Vec::new();
        let mut writes = Vec::new();
        let mut with = Vec::new();
        let mut without = Vec::new();

        for access in self.access.iter() {
            match access {
                QueryBuilderAccess::Read(id) => reads.push(*id),
                QueryBuilderAccess::Write(id) => writes.push(*id),
                QueryBuilderAccess::With(id) => with.push(*id),
                QueryBuilderAccess::Without(id) => without.push(*id),
                QueryBuilderAccess::Entity => {}
            }
        }

        // find the archetypes that match the query
        for archetype in self.world.storage().archetypes() {
            let mut matches = true;
            if !reads.is_empty() && !archetype.contains_all_types(&reads) {
                matches = false;
            }
            if !writes.is_empty() && !archetype.contains_all_types(&writes) {
                matches = false;
            }
            if !with.is_empty() && !archetype.contains_all_types(&with) {
                matches = false;
            }
            if !without.is_empty() && archetype.contains_any_type(&without) {
                matches = false;
            }
            if matches {
                entities.extend(archetype.entity_iter());
            }
        }
        Query {
            world: self.world,
            access: self.access,
            entities,
        }
    }
}

/// A query that can be used to iterate over entities that match the query.
/// The query will only iterate over entities that match all of the components that were added to the [`QueryBuilder`].
pub struct Query<'a> {
    pub(crate) world: &'a World,
    pub(crate) access: Vec<QueryBuilderAccess>,
    pub(crate) entities: Vec<Entity>,
}

impl<'a> Query<'a> {
    pub fn iter(&self) -> QueryIter {
        QueryIter {
            world: self.world,
            access: &self.access,
            entities: &self.entities,
            index: 0,
        }
    }

    pub fn get(&self, entity: Entity) -> Option<QueryResults> {
        let archetype = self.world.storage().entity_archetype(entity)?;

        let mut data = Vec::new();

        for access in self.access.iter() {
            match access {
                QueryBuilderAccess::Read(id) => {
                    let component = archetype.get(*id, entity)?;
                    let proxy = Proxy {
                        component_type: *id,
                        component,
                        entity,
                    };
                    data.push(QueryItem::Proxy(proxy));
                }
                QueryBuilderAccess::Write(id) => {
                    let component = archetype.get_mut(*id, entity)?;
                    let proxy = ProxyMut {
                        component_type: *id,
                        component,
                        entity,
                    };
                    data.push(QueryItem::ProxyMut(proxy));
                }
                QueryBuilderAccess::Entity => {
                    data.push(QueryItem::Entity(entity));
                }
                QueryBuilderAccess::With(_) => {}
                QueryBuilderAccess::Without(_) => {}
            }
        }

        Some(QueryResults(data))
    }
}

/// A proxy that can be used to get a component and its fields for an entity.

pub struct Proxy<'a> {
    pub(crate) component_type: Entity,
    pub(crate) component: Ref<'a>,
    pub(crate) entity: Entity,
}

impl<'a> Proxy<'a> {
    pub fn component_type_name(&self) -> Option<String> {
        self.component_type.type_name()
    }

    pub fn get<T: Atom>(&self) -> Option<&T> {
        if self.component_type != T::static_type_uid() {
            return None;
        }

        self.component.as_dynamic()?.as_ref::<T>()
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }
}

/// A proxy that can be used to get a mutable component and its fields for an entity.

pub struct ProxyMut<'a> {
    pub(crate) component_type: Entity,
    pub(crate) component: Mut<'a>,
    pub(crate) entity: Entity,
}

impl<'a> ProxyMut<'a> {
    pub fn component_type_name(&self) -> Option<String> {
        self.component_type.type_name()
    }

    pub fn get<T: Atom>(&self) -> Option<&T> {
        if self.component_type != T::static_type_uid() {
            return None;
        }

        self.component.as_dynamic()?.as_ref::<T>()
    }

    pub fn get_mut<T: Atom>(&mut self) -> Option<&mut T> {
        if self.component_type != T::static_type_uid() {
            return None;
        }

        self.component.as_dynamic_mut()?.as_mut::<T>()
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }
}

/// An item in a query result.
/// Can be used to get the entity or a component from the query result.
/// See [`Query::iter`] and [`Query::get`] for more information.
pub enum QueryItem<'a> {
    Entity(Entity),
    Proxy(Proxy<'a>),
    ProxyMut(ProxyMut<'a>),
}

impl<'a> QueryItem<'a> {
    /// Returns the entity for the query item.
    pub fn entity(&self) -> Entity {
        match self {
            QueryItem::Entity(entity) => *entity,
            QueryItem::Proxy(data) => data.entity,
            QueryItem::ProxyMut(data) => data.entity,
        }
    }

    /// Returns a component for the query item.
    /// Returns [`None`] if the query item does not contain a component of the specified type.
    pub fn get<T: Atom>(&self) -> Option<&T> {
        match self {
            QueryItem::Proxy(data) => data.get::<T>(),
            QueryItem::ProxyMut(data) => data.get::<T>(),
            _ => None,
        }
    }

    /// Returns a mutable component for the query item.
    /// Returns [`None`] if the query item does not contain a mutable component of the specified type.
    pub fn get_mut<T: Atom>(&mut self) -> Option<&mut T> {
        match self {
            QueryItem::ProxyMut(data) => data.get_mut::<T>(),
            _ => None,
        }
    }

    /// Returns a [`Ref`] allowing access to the component for the query item.
    /// Returns [`None`] if the query item does not contain a component.
    pub fn get_data(&self) -> Option<&Ref<'a>> {
        match self {
            QueryItem::Proxy(data) => Some(&data.component),
            _ => None,
        }
    }

    /// Returns a [`Mut`] allowing mutable access to the component for the query item.
    /// Returns [`None`] if the query item does not contain a mutable component.
    pub fn get_data_mut(&mut self) -> Option<&mut Mut<'a>> {
        match self {
            QueryItem::ProxyMut(data) => Some(&mut data.component),
            _ => None,
        }
    }

    /// Returns the [`Entity`] representing the type of the component in the query item.
    /// Returns [`None`] if the query item does not contain a component.
    pub fn get_type(&self) -> Option<Entity> {
        match self {
            QueryItem::Proxy(data) => Some(data.component_type),
            QueryItem::ProxyMut(data) => Some(data.component_type),
            _ => None,
        }
    }
}

impl<'a> Debug for QueryItem<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryItem::Entity(entity) => write!(f, "Entity({})", entity),
            QueryItem::Proxy(data) => write!(f, "Proxy({:?})", data.component_type_name()),
            QueryItem::ProxyMut(data) => write!(f, "ProxyMut({:?})", data.component_type_name()),
        }
    }
}

pub struct QueryResults<'a>(Vec<QueryItem<'a>>);

impl<'a> QueryResults<'a> {
    pub fn entity(&self) -> Option<Entity> {
        self.0.first().map(|item| item.entity())
    }

    pub fn get<T: Atom>(&self) -> Option<&T> {
        self.0.iter().find_map(|item| item.get::<T>())
    }

    pub fn get_mut<T: Atom>(&mut self) -> Option<&'_ mut T> {
        self.0.iter_mut().find_map(|item| item.get_mut::<T>())
    }

    pub fn get_data(&'a self) -> Option<&Ref<'a>> {
        self.0.iter().find_map(|item| item.get_data())
    }

    pub fn get_data_mut(&'a mut self) -> Option<&mut Mut<'a>> {
        self.0.iter_mut().find_map(|item| item.get_data_mut())
    }

    pub fn into_inner(self) -> Vec<QueryItem<'a>> {
        self.0
    }
}

/// An iterator over the query results.
/// See [`Query::iter`] for more information.
pub struct QueryIter<'a> {
    world: &'a World,
    access: &'a Vec<QueryBuilderAccess>,
    entities: &'a Vec<Entity>,
    index: usize,
}

impl<'a> Iterator for QueryIter<'a> {
    type Item = QueryResults<'a>;

    /// Returns the next query result.
    /// Returns [`None`] if there are no more query results.
    /// See [`Query::iter`] for more information.
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.entities.len() {
            return None;
        }

        let entity = self.entities[self.index];

        if entity.is_dead() {
            self.index += 1;
            return self.next();
        }

        let archetype = self.world.storage().entity_archetype(entity).unwrap();

        let mut data = Vec::new();

        for access in self.access.iter() {
            match access {
                QueryBuilderAccess::Read(id) => {
                    let component = archetype.get(*id, entity).unwrap();
                    data.push(QueryItem::Proxy(Proxy {
                        component_type: *id,
                        component,
                        entity,
                    }))
                }
                QueryBuilderAccess::Write(id) => {
                    let component = archetype.get_mut(*id, entity).unwrap();
                    data.push(QueryItem::ProxyMut(ProxyMut {
                        component_type: *id,
                        component,
                        entity,
                    }))
                }
                QueryBuilderAccess::Entity => {
                    data.push(QueryItem::Entity(entity));
                }
                QueryBuilderAccess::With(_) => {}
                QueryBuilderAccess::Without(_) => {}
            }
        }

        self.index += 1;

        Some(QueryResults(data))
    }
}

#[cfg(test)]
mod tests {
    use crate as fabricate;
    use crate::prelude::*;

    #[derive(Atom, Clone, Debug, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Atom, Clone, Debug, PartialEq)]
    struct Velocity {
        x: f32,
        y: f32,
    }

    #[test]
    fn test_query() {
        let world_handle = get_world();

        let mut world = world_handle.write();

        let entity1 = world.spawn((Position { x: 0.0, y: 0.0 },)).unwrap();
        world.add(entity1, Velocity { x: 1.0, y: 1.0 }).unwrap();

        let entity2 = world.spawn((Position { x: 0.0, y: 0.0 },)).unwrap();

        let query = world.query().read::<Position>().unwrap().build();

        let iter = query.iter();

        assert_eq!(iter.count(), 2);

        let e1 = query.get(entity1).unwrap().into_inner();
        let e2 = query.get(entity2).unwrap().into_inner();

        assert_eq!(e1.len(), 1);
        assert_eq!(e2.len(), 1);

        let e1 = e1[0].get_data().unwrap();
        let e2 = e2[0].get_data().unwrap();

        let e1_ty = e1.type_uid();
        let e2_ty = e2.type_uid();

        assert_eq!(e1_ty, Position::type_uid());
        assert_eq!(e2_ty, Position::type_uid());
    }

    #[test]
    fn test_query_write() {
        let world_handle = get_world();

        let mut world = world_handle.write();

        let entity1 = world.spawn((Position { x: 0.0, y: 0.0 },)).unwrap();
        world.add(entity1, Velocity { x: 1.0, y: 1.0 }).unwrap();

        let entity2 = world.spawn((Position { x: 0.0, y: 0.0 },)).unwrap();

        let query = world.query().write::<Position>().unwrap().build();

        let iter = query.iter();

        assert_eq!(iter.count(), 2);

        let mut e1 = query.get(entity1).unwrap().into_inner();
        let mut e2 = query.get(entity2).unwrap().into_inner();

        assert_eq!(e1.len(), 1);
        assert_eq!(e2.len(), 1);

        let e1 = e1[0].get_data_mut().unwrap();
        let e2 = e2[0].get_data_mut().unwrap();

        let e1_ty = e1.type_uid();
        let e2_ty = e2.type_uid();

        assert_eq!(e1_ty, Position::type_uid());
        assert_eq!(e2_ty, Position::type_uid());
    }
}

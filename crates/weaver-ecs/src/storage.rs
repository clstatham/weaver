use std::any::{Any, TypeId};

use any_vec::AnyVec;
use weaver_util::SharedLock;

use crate::{
    bundle::ComponentBundle,
    entity::{Entity, EntityMap},
    loan::LoanStorage,
    prelude::Bundle,
};

#[derive(Default)]
pub struct Archetype {
    data_types: Vec<TypeId>,
    columns: Vec<SharedLock<LoanStorage<AnyVec<dyn Send + Sync>>>>,
    entity_id_lookup: Vec<Entity>,
}

impl Archetype {
    pub fn new_for_bundle<T: Bundle>() -> Self {
        let mut vecs = T::empty_vecs();
        vecs.sort_unstable_by_key(|vec| vec.element_typeid());
        let mut data_types = T::component_type_ids();
        data_types.sort_unstable();
        let columns = vecs
            .into_iter()
            .map(LoanStorage::new)
            .map(SharedLock::new)
            .collect();

        Self {
            data_types,
            columns,
            entity_id_lookup: Vec::new(),
        }
    }

    pub fn columns(&self) -> &[SharedLock<LoanStorage<AnyVec<dyn Send + Sync>>>] {
        &self.columns
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.entity_id_lookup.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.entity_id_lookup.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entity_id_lookup.is_empty()
    }

    pub fn index_of(&self, ty: TypeId) -> Option<usize> {
        self.data_types.iter().position(|&id| id == ty)
    }

    pub fn entity_index(&self, entity: Entity) -> Option<usize> {
        self.entity_id_lookup.iter().position(|&id| id == entity)
    }

    pub fn has<T: Any + Send + Sync>(&self) -> bool {
        self.index_of(TypeId::of::<T>()).is_some()
    }

    pub fn exactly_matches_bundle<T: Bundle>(&self) -> bool {
        let mut sorted = self.data_types.clone();
        sorted.sort_unstable();
        let mut bundle = T::component_type_ids();
        bundle.sort_unstable();
        sorted == bundle
    }

    pub fn partially_matches_bundle<T: Bundle>(&self) -> bool {
        let bundle = T::component_type_ids();
        self.data_types.iter().all(|id| bundle.contains(id))
    }

    pub fn exactly_matches_type_ids(&self, data_types: impl IntoIterator<Item = TypeId>) -> bool {
        let mut sorted = self.data_types.clone();
        sorted.sort_unstable();
        let mut data_types = data_types.into_iter().collect::<Vec<_>>();
        data_types.sort_unstable();
        sorted == data_types
    }

    pub fn partially_matches_type_ids(&self, data_types: impl IntoIterator<Item = TypeId>) -> bool {
        data_types
            .into_iter()
            .all(|id| self.data_types.contains(&id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchetypeId(u64);

impl ArchetypeId {
    pub fn from_u64(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }

    pub fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

#[derive(Default)]
pub struct Components {
    // Note: This Vec never shrinks. This is intentional to avoid changing the ArchetypeId of existing archetypes. Empty archetypes are kept initialized in memory for potential reuse later.
    archetypes: Vec<Archetype>,
    entity_archetype: EntityMap<ArchetypeId>,
}

impl Components {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn get_archetype_id_for_type_ids(
        &self,
        data_types: impl IntoIterator<Item = TypeId>,
    ) -> Option<ArchetypeId> {
        let data_types = data_types.into_iter().collect::<Vec<_>>();
        self.archetypes
            .iter()
            .enumerate()
            .find_map(|(i, archetype)| {
                if archetype.exactly_matches_type_ids(data_types.iter().copied()) {
                    Some(ArchetypeId::from_u64(i as u64))
                } else {
                    None
                }
            })
    }

    pub(crate) fn remove_entity(&mut self, entity: Entity) -> ComponentBundle {
        assert!(
            self.entity_archetype.contains_key(&entity),
            "Entity does not exist"
        );
        let archetype_id = self.entity_archetype.remove(&entity).unwrap();
        let archetype = &mut self.archetypes[archetype_id.as_usize()];
        let entity_index = archetype.entity_index(entity).unwrap();
        let mut components = Vec::new();
        for column in archetype.columns() {
            let mut column = column.write();
            let mut tmp = column.loan().unwrap().clone_empty();
            let mut column = column.loan_mut().unwrap();
            let component = column.swap_remove(entity_index);
            tmp.push(component);
            components.push(tmp);
        }
        archetype.entity_id_lookup.swap_remove(entity_index);
        ComponentBundle {
            types: archetype.data_types.clone(),
            components,
        }
    }

    pub(crate) fn insert_entity(&mut self, entity: Entity, components: ComponentBundle) {
        assert!(
            !self.entity_archetype.contains_key(&entity),
            "Entity already exists"
        );

        let maybe_archetype_id =
            self.get_archetype_id_for_type_ids(components.types.iter().copied());
        let archetype_id = match maybe_archetype_id {
            Some(id) => id, // archetype already exists
            None => {
                // create a new archetype for this bundle
                let archetype = Archetype {
                    data_types: components.types.clone(),
                    columns: components
                        .empty_vecs()
                        .into_iter()
                        .map(LoanStorage::new)
                        .map(SharedLock::new)
                        .collect(),
                    entity_id_lookup: Vec::new(),
                };
                let id = ArchetypeId::from_u64(self.archetypes.len() as u64);
                self.archetypes.push(archetype);
                id
            }
        };

        let archetype = &mut self.archetypes[archetype_id.as_usize()];
        archetype.entity_id_lookup.push(entity);

        for (column, mut component) in archetype.columns.iter_mut().zip(components.components) {
            let mut column = column.write();
            column.loan_mut().unwrap().push(component.pop().unwrap());
        }

        self.entity_archetype.insert(entity, archetype_id);
    }

    pub fn insert_bundle<T: Bundle>(&mut self, entity: Entity, bundle: T) {
        let mut components = ComponentBundle::from_tuple(bundle);
        if self.entity_archetype.contains_key(&entity) {
            let old_comps = self.remove_entity(entity);
            components.union(old_comps);
        }
        self.insert_entity(entity, components);
    }

    pub fn remove_component<T: Any + Send + Sync>(&mut self, entity: Entity) -> Option<T> {
        let mut components = self.remove_entity(entity);
        let removed = components.remove::<T>();
        self.insert_entity(entity, components);
        removed
    }

    pub fn has_component<T: Any + Send + Sync>(&self, entity: Entity) -> bool {
        let archetype_id = self.entity_archetype.get(&entity).unwrap();
        let archetype = &self.archetypes[archetype_id.as_usize()];
        archetype.entity_index(entity).is_some() && archetype.has::<T>()
    }

    pub fn archetype_iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
    }
}

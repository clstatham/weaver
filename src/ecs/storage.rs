use bit_set::BitSet;
use fixedbitset::FixedBitSet;
use roaring::RoaringBitmap;
use rustc_hash::FxHashSet;
use std::ops::{Deref, DerefMut};

use super::{entity::EntityId, world::ComponentPtr};

pub type EntitySet = FixedBitSet;
pub type ComponentSet = FixedBitSet;
pub type ComponentMap = SparseMap<usize, ComponentPtr, ComponentSet>;
pub type EntityComponentsMap = SparseMap<EntityId, ComponentMap, EntitySet>;
pub type QueryMap = SparseMap<EntityId, Vec<ComponentPtr>, EntitySet>;

pub trait Index:
    Default + Clone + Copy + PartialEq + Eq + std::hash::Hash + bit_vec::BitBlock
{
    fn from_usize(i: usize) -> Self;
    fn to_usize(self) -> usize;
    fn to_u32(self) -> u32 {
        self.to_usize() as u32
    }
    fn from_u32(i: u32) -> Self {
        Self::from_usize(i as usize)
    }
}

impl Index for usize {
    fn from_usize(i: usize) -> Self {
        i
    }
    fn to_usize(self) -> usize {
        self
    }
}

impl Index for u32 {
    fn from_usize(i: usize) -> Self {
        i as u32
    }
    fn to_usize(self) -> usize {
        self as usize
    }
}

impl Index for u64 {
    fn from_usize(i: usize) -> Self {
        i as u64
    }
    fn to_usize(self) -> usize {
        self as usize
    }
}

pub trait Set<I: Index>: Default {
    fn set_insert(&mut self, t: I);
    fn set_remove(&mut self, t: I);
    fn set_contains(&self, t: I) -> bool;
    fn set_len(&self) -> usize;
    fn set_is_empty(&self) -> bool;
    fn set_clear(&mut self);
    fn set_iter(&self) -> impl Iterator<Item = I>;
    fn set_from_iter<Iter: IntoIterator<Item = I>>(iter: Iter) -> Self {
        let mut this = Self::default();
        for i in iter {
            this.set_insert(i);
        }
        this
    }

    fn set_union_with(&mut self, other: &Self);
    fn set_intersection_with(&mut self, other: &Self);
    fn set_difference_with(&mut self, other: &Self);

    fn set_eq(&self, other: &Self) -> bool {
        self.set_len() == other.set_len()
            && self
                .set_iter()
                .all(|x| other.set_contains(x) && self.set_contains(x))
    }
}

impl<I: Index> Set<I> for FxHashSet<I> {
    fn set_insert(&mut self, t: I) {
        self.insert(t);
    }
    fn set_remove(&mut self, t: I) {
        self.remove(&t);
    }
    fn set_contains(&self, t: I) -> bool {
        self.contains(&t)
    }
    fn set_len(&self) -> usize {
        self.len()
    }
    fn set_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn set_clear(&mut self) {
        self.clear();
    }
    fn set_iter(&self) -> impl Iterator<Item = I> {
        self.iter().copied()
    }
    fn set_union_with(&mut self, other: &Self) {
        self.extend(other.iter().copied());
    }
    fn set_intersection_with(&mut self, other: &Self) {
        self.retain(|x| other.contains(x));
    }
    fn set_difference_with(&mut self, other: &Self) {
        self.retain(|x| !other.contains(x));
    }
}

impl<I: Index> Set<I> for FixedBitSet {
    fn set_insert(&mut self, t: I) {
        self.grow(t.to_usize() + 1);
        self.insert(t.to_usize());
    }
    fn set_remove(&mut self, t: I) {
        if t.to_usize() < self.len() {
            self.set(t.to_usize(), false);
        }
    }
    fn set_contains(&self, t: I) -> bool {
        self.contains(t.to_usize())
    }
    fn set_len(&self) -> usize {
        self.count_ones(..)
    }
    fn set_is_empty(&self) -> bool {
        self.is_clear()
    }
    fn set_clear(&mut self) {
        self.clear();
    }
    fn set_iter(&self) -> impl Iterator<Item = I> {
        self.ones().map(I::from_usize)
    }
    fn set_union_with(&mut self, other: &Self) {
        self.union_with(other);
    }
    fn set_intersection_with(&mut self, other: &Self) {
        self.intersect_with(other);
    }
    fn set_difference_with(&mut self, other: &Self) {
        self.difference_with(other);
    }
}

impl<I: Index> Set<I> for BitSet<I> {
    fn set_insert(&mut self, t: I) {
        self.insert(t.to_usize());
    }
    fn set_remove(&mut self, t: I) {
        self.remove(t.to_usize());
    }
    fn set_contains(&self, t: I) -> bool {
        self.contains(t.to_usize())
    }
    fn set_len(&self) -> usize {
        self.len()
    }
    fn set_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn set_clear(&mut self) {
        self.clear();
    }
    fn set_iter(&self) -> impl Iterator<Item = I> {
        self.iter().map(I::from_usize)
    }
    fn set_union_with(&mut self, other: &Self) {
        self.union_with(other);
    }
    fn set_intersection_with(&mut self, other: &Self) {
        self.intersect_with(other);
    }
    fn set_difference_with(&mut self, other: &Self) {
        self.difference_with(other);
    }
}

impl<I: Index> Set<I> for RoaringBitmap {
    fn set_insert(&mut self, t: I) {
        self.insert(t.to_u32());
    }
    fn set_remove(&mut self, t: I) {
        self.remove(t.to_u32());
    }
    fn set_contains(&self, t: I) -> bool {
        self.contains(t.to_u32())
    }
    fn set_len(&self) -> usize {
        self.len() as usize
    }
    fn set_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn set_clear(&mut self) {
        self.clear();
    }
    fn set_iter(&self) -> impl Iterator<Item = I> {
        self.iter().map(I::from_u32)
    }
    fn set_union_with(&mut self, other: &Self) {
        *self |= other;
    }
    fn set_intersection_with(&mut self, other: &Self) {
        *self &= other;
    }
    fn set_difference_with(&mut self, other: &Self) {
        *self -= other;
    }
}

#[derive(Clone)]
pub struct SparseMap<I: Index, T: Clone, S: Set<I>> {
    data: Vec<Option<T>>,
    indices: S,
    _phantom: std::marker::PhantomData<I>,
}

impl<I: Index, T: Clone, S: Set<I>> SparseMap<I, T, S> {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            indices: S::default(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn insert(&mut self, index: I, value: T) {
        if index.to_usize() >= self.data.len() {
            self.data.resize(index.to_usize() + 1, None);
        }
        self.data[index.to_usize()] = Some(value);
        self.indices.set_insert(index);
    }

    pub fn get(&self, index: I) -> Option<&T> {
        self.data.get(index.to_usize()).and_then(|x| x.as_ref())
    }

    pub fn get_mut(&mut self, index: I) -> Option<&mut T> {
        self.data.get_mut(index.to_usize()).and_then(|x| x.as_mut())
    }

    pub fn remove(&mut self, index: I) -> Option<T> {
        if index.to_usize() >= self.data.len() {
            return None;
        }
        self.indices.set_remove(index);
        self.data[index.to_usize()].take()
    }

    pub fn contains(&self, index: I) -> bool {
        self.indices.set_contains(index)
    }

    pub fn keys(&self) -> impl Iterator<Item = I> + '_ {
        self.indices.set_iter()
    }

    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.indices
            .set_iter()
            .map(|i| (i, self.data[i.to_usize()].as_ref()))
            .filter_map(|(i, v)| v.map(|v| (i, v)))
    }

    pub fn len(&self) -> usize {
        self.indices.set_len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.set_is_empty()
    }

    pub fn clear(&mut self) {
        self.data.clear();
        self.indices.set_clear();
    }
}

impl<I: Index, T: Clone, S: Set<I>> Default for SparseMap<I, T, S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I: Index, T: Clone, S: Set<I>> FromIterator<(I, T)> for SparseMap<I, T, S> {
    fn from_iter<Iter: IntoIterator<Item = (I, T)>>(iter: Iter) -> Self {
        let mut map = Self::new();
        for (i, v) in iter {
            map.insert(i, v);
        }
        map
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EntityComponents {
    pub(crate) components: ComponentMap,
}

impl Deref for EntityComponents {
    type Target = ComponentMap;

    fn deref(&self) -> &Self::Target {
        &self.components
    }
}

impl DerefMut for EntityComponents {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.components
    }
}

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Components {
    pub entity_components: EntityComponentsMap,
    pub archetypes: Vec<(ComponentSet, EntitySet)>,
}

impl Components {
    pub fn add_component(&mut self, entity: EntityId, component: ComponentPtr) {
        let component_id = component.component_id;

        if let Some(components) = self.entity_components.get_mut(entity) {
            components.insert(component_id, component.clone());
        } else {
            self.entity_components.insert(entity, SparseMap::default());
            self.entity_components
                .get_mut(entity)
                .unwrap()
                .insert(component_id, component.clone());
        }

        self.recalculate_archetype(entity);
    }

    pub fn remove_component(&mut self, entity: EntityId, component_id: usize) {
        self.entity_components
            .get_mut(entity)
            .and_then(|components| components.remove(component_id));

        self.recalculate_archetype(entity);
    }

    pub fn despawn(&mut self, entity: EntityId) {
        self.entity_components.remove(entity);
        self.recalculate_archetype(entity);
    }

    pub fn generate_archetype(&self, entity: EntityId) -> Option<ComponentSet> {
        self.entity_components
            .get(entity)
            .map(|components| ComponentSet::set_from_iter(components.keys()))
    }

    pub fn recalculate_archetype(&mut self, entity: EntityId) {
        let archetype = self.generate_archetype(entity);

        // Remove the entity from all archetypes
        self.archetypes.iter_mut().for_each(|(_, entities)| {
            entities.set_remove(entity);
        });

        if let Some(archetype) = archetype {
            let mut found = false;
            for (archetype_components, entities) in self.archetypes.iter_mut() {
                if Set::<usize>::set_eq(archetype_components, &archetype) {
                    entities.set_insert(entity);
                    found = true;
                    break;
                }
            }
            if !found {
                self.archetypes
                    .push((archetype, EntitySet::set_from_iter(vec![entity.to_usize()])));
            }
        }

        self.archetypes.retain(|(_, entities)| !entities.is_empty());
    }
}

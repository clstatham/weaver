use std::fmt::Debug;

use anyhow::{bail, Result};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    component::Atom,
    lock::Lock,
    prelude::{Entity, MapRead, MapWrite, Read, Uid, Write},
    registry::StaticId,
    relationship::Relationship,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SortedVec<K: Ord>(Vec<K>);

impl<K: Ord> SortedVec<K> {
    pub fn sort(&mut self) {
        self.0.sort_unstable();
    }

    pub fn get(&self, index: usize) -> Option<&K> {
        self.0.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut K> {
        self.0.get_mut(index)
    }

    pub fn push(&mut self, value: K) {
        self.0.push(value);
        self.sort();
    }

    pub fn pop(&mut self) -> Option<K> {
        // we shouldn't need to re-sort since the elements should have been sorted to begin with
        self.0.pop()
    }

    pub fn remove(&mut self, index: usize) -> Option<K> {
        if index >= self.0.len() {
            return None;
        }
        let v = self.0.remove(index);
        self.sort();
        Some(v)
    }

    pub fn search(&self, value: &K) -> Result<usize, usize> {
        self.0.binary_search(value)
    }

    pub fn index_of(&self, value: &K) -> Option<usize> {
        match self.0.binary_search(value) {
            Ok(index) => Some(index),
            Err(_) => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &K> {
        self.0.iter()
    }

    pub fn contains(&self, value: &K) -> bool {
        self.0.binary_search(value).is_ok()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn into_inner(self) -> Vec<K> {
        self.0
    }
}

impl<K: Ord> From<Vec<K>> for SortedVec<K> {
    fn from(v: Vec<K>) -> Self {
        let mut s = Self(v);
        s.sort();
        s
    }
}

impl Default for SortedVec<Entity> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

#[derive(Clone)]
pub struct SortedMap<K: Ord, V>(Vec<(K, V)>);

impl<K: Ord, V> Default for SortedMap<K, V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<K: Ord + Clone, V> SortedMap<K, V> {
    pub fn sort(&mut self) {
        self.0.sort_unstable_by(|(k, _), (k2, _)| k.cmp(k2));
    }

    pub fn binary_search(&self, key: &K) -> Result<usize, usize> {
        self.0.binary_search_by(|(k, _)| k.cmp(key))
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        match self.binary_search(key) {
            Ok(index) => Some(&self.0[index].1),
            Err(_) => None,
        }
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        match self.binary_search(key) {
            Ok(index) => Some(&mut self.0[index].1),
            Err(_) => None,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        match self.binary_search(&key) {
            Ok(index) => self.0[index].1 = value,
            Err(index) => self.0.insert(index, (key, value)),
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        match self.binary_search(key) {
            Ok(index) => Some(self.0.remove(index).1),
            Err(_) => None,
        }
    }

    #[allow(clippy::map_identity)] // false positive
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(k, v)| (k, v))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.0.iter_mut().map(|(k, v)| (&*k, v))
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.0.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0.iter().map(|(_, v)| v)
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.0.iter_mut().map(|(_, v)| v)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn drain(&mut self) -> impl Iterator<Item = (K, V)> + '_ {
        self.0.drain(..)
    }

    pub fn contains(&self, key: &K) -> bool {
        self.binary_search(key).is_ok()
    }

    pub fn into_inner(self) -> Vec<(K, V)> {
        self.0
    }
}

impl<K: Debug + Ord, V: Debug> Debug for SortedMap<K, V> {
    #[allow(clippy::map_identity)] // false positive
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map()
            .entries(self.0.iter().map(|(k, v)| (k, v)))
            .finish()
    }
}

impl<K: Ord + Copy, V> From<Vec<(K, V)>> for SortedMap<K, V> {
    fn from(v: Vec<(K, V)>) -> Self {
        let mut s = Self(v);
        s.sort();
        s
    }
}

pub struct DynamicData {
    type_uid: Entity,
    value_uid: Entity,
    data: Box<dyn Atom>,
}

impl DynamicData {
    pub fn new<T: Atom>(data: T) -> Self {
        let type_uid = T::static_type_uid();
        let value_uid = Entity::allocate(Some(&type_uid));
        let data = Box::new(data);
        Self {
            type_uid,
            value_uid,
            data,
        }
    }

    pub fn new_relation<R: Relationship>(relation: R, relative: &Entity) -> Self {
        let type_uid = Entity::new_relationship(&R::static_type_uid(), relative);
        let value_uid = Entity::allocate(Some(&type_uid));
        let data = Box::new(relation);
        Self {
            type_uid,
            value_uid,
            data,
        }
    }

    pub fn type_uid(&self) -> &Entity {
        &self.type_uid
    }

    pub fn value_uid(&self) -> &Entity {
        &self.value_uid
    }

    pub fn as_ref<T: Atom>(&self) -> Option<&T> {
        if self.type_uid == T::static_type_uid() {
            Some((*self.data).as_any().downcast_ref().unwrap())
        } else {
            None
        }
    }

    pub fn as_mut<T: Atom>(&mut self) -> Option<&mut T> {
        if self.type_uid == T::static_type_uid() {
            Some((*self.data).as_any_mut().downcast_mut().unwrap())
        } else {
            None
        }
    }

    pub fn into<T: Atom>(self) -> Result<Box<T>, Self> {
        if self.type_uid == T::static_type_uid() {
            Ok(self.data.as_any_box().downcast().unwrap())
        } else {
            Err(self)
        }
    }

    pub fn display(&self) -> String {
        if let Some(data) = self.as_ref::<String>() {
            data.clone()
        } else {
            format!("{:?}", self)
        }
    }

    pub fn data(&self) -> &dyn Atom {
        self.data.as_ref()
    }

    pub fn data_mut(&mut self) -> &mut dyn Atom {
        self.data.as_mut()
    }
}

impl Debug for DynamicData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicData")
            .field("type_uid", &self.type_uid)
            .field("value_uid", &self.value_uid)
            .finish()
    }
}

impl PartialEq for DynamicData {
    fn eq(&self, other: &Self) -> bool {
        self.type_uid == other.type_uid && self.value_uid == other.value_uid
    }
}

impl Clone for DynamicData {
    fn clone(&self) -> Self {
        Self {
            type_uid: self.type_uid.clone(),
            value_uid: self.value_uid.clone(),
            data: self.data.clone_box(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Data {
    Dynamic(DynamicData),
    Pointer(Pointer),
}

impl Data {
    pub fn new_dynamic<T: Atom>(data: T) -> Self {
        Self::Dynamic(DynamicData::new(data))
    }

    pub fn new_pointer(target_type_uid: &Entity, target_value_uid: &Entity) -> Self {
        Self::Pointer(Pointer::new(target_type_uid, target_value_uid))
    }

    pub fn new_relation<R: Relationship>(relation: R, relative: &Entity) -> Self {
        Self::Dynamic(DynamicData::new_relation(relation, relative))
    }

    pub fn type_uid(&self) -> &Entity {
        match self {
            Self::Dynamic(data) => data.type_uid(),
            Self::Pointer(pointer) => pointer.target_type_uid(),
        }
    }

    pub fn value_uid(&self) -> &Entity {
        match self {
            Self::Dynamic(data) => data.value_uid(),
            Self::Pointer(pointer) => pointer.target_value_uid(),
        }
    }

    pub fn as_ref<T: Atom>(&self) -> Option<&T> {
        match self {
            Self::Dynamic(data) => data.as_ref(),
            Self::Pointer(_) => None,
        }
    }

    pub fn as_mut<T: Atom>(&mut self) -> Option<&mut T> {
        match self {
            Self::Dynamic(data) => data.as_mut(),
            Self::Pointer(_) => None,
        }
    }

    pub fn as_pointer(&self) -> Option<&Pointer> {
        match self {
            Self::Dynamic(_) => None,
            Self::Pointer(pointer) => Some(pointer),
        }
    }

    pub fn as_dynamic(&self) -> Option<&DynamicData> {
        match self {
            Self::Dynamic(data) => Some(data),
            Self::Pointer(_) => None,
        }
    }

    pub fn into<T: Atom>(self) -> Result<Box<T>, Self> {
        match self {
            Self::Dynamic(data) => data.into::<T>().map_err(Self::Dynamic),
            Self::Pointer(_) => Err(self),
        }
    }

    pub fn into_pointer(self) -> Result<Pointer, Self> {
        match self {
            Self::Dynamic(_) => Err(self),
            Self::Pointer(pointer) => Ok(pointer),
        }
    }

    pub fn into_dynamic_data(self) -> Result<DynamicData, Self> {
        match self {
            Self::Dynamic(data) => Ok(data),
            Self::Pointer(_) => Err(self),
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Dynamic(data) => data.display(),
            Self::Pointer(pointer) => format!(
                "<pointer to {:?} ({:?})>",
                pointer.target_value_uid(),
                pointer.target_type_uid()
            ),
        }
    }

    pub fn to_owned(&self) -> Self {
        match self {
            Self::Dynamic(data) => Self::Dynamic(data.clone()),
            Self::Pointer(pointer) => Self::Pointer(pointer.clone()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pointer {
    target_type_uid: Entity,
    target_value_uid: Entity,
}

impl Pointer {
    pub fn new(target_type_uid: &Entity, target_value_uid: &Entity) -> Self {
        Self {
            target_type_uid: target_type_uid.clone(),
            target_value_uid: target_value_uid.clone(),
        }
    }

    pub fn target_type_uid(&self) -> &Entity {
        &self.target_type_uid
    }

    pub fn target_value_uid(&self) -> &Entity {
        &self.target_value_uid
    }

    pub fn into_data(self) -> Data {
        Data::Pointer(self)
    }
}

impl PartialEq for Pointer {
    fn eq(&self, other: &Self) -> bool {
        self.target_type_uid == other.target_type_uid
            && self.target_value_uid == other.target_value_uid
    }
}

pub struct PointerRef<'a> {
    target_type_uid: Entity,
    target_value_uid: Entity,
    _column: Read<'a, Column>,
}

impl PartialEq for PointerRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.target_type_uid == other.target_type_uid
            && self.target_value_uid == other.target_value_uid
    }
}

impl<'a> PointerRef<'a> {
    pub fn new(
        target_type_uid: &Entity,
        target_value_uid: &Entity,
        _column: Read<'a, Column>,
    ) -> Self {
        Self {
            target_type_uid: target_type_uid.clone(),
            target_value_uid: target_value_uid.clone(),
            _column,
        }
    }

    pub fn target_type_uid(&self) -> &Entity {
        &self.target_type_uid
    }

    pub fn target_value_uid(&self) -> &Entity {
        &self.target_value_uid
    }

    pub fn deref(&self, storage: &'a Storage) -> Option<Ref<'_>> {
        storage.deref_pointer_ref(self)
    }

    pub fn to_owned(&self) -> Pointer {
        Pointer::new(&self.target_type_uid, &self.target_value_uid)
    }
}

impl<'a> std::fmt::Debug for PointerRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointerRef")
            .field("target_type_uid", &self.target_type_uid)
            .field("target_value_uid", &self.target_value_uid)
            .finish()
    }
}

impl<'a> std::fmt::Display for PointerRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PointerRef")
    }
}

#[derive(Debug)]
pub struct PointerMut<'a> {
    target_type_uid: Entity,
    target_value_uid: Entity,
    _column: Write<'a, Column>,
}

impl PartialEq for PointerMut<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.target_type_uid == other.target_type_uid
            && self.target_value_uid == other.target_value_uid
    }
}

impl<'a> PointerMut<'a> {
    pub fn new(
        target_type_uid: &Entity,
        target_value_uid: &Entity,
        _column: Write<'a, Column>,
    ) -> Self {
        Self {
            target_type_uid: target_type_uid.clone(),
            target_value_uid: target_value_uid.clone(),
            _column,
        }
    }

    pub fn target_type_uid(&self) -> &Entity {
        &self.target_type_uid
    }

    pub fn target_value_uid(&self) -> &Entity {
        &self.target_value_uid
    }

    pub fn deref_mut(&mut self, storage: &'a mut Storage) -> Option<Mut<'_>> {
        storage.deref_pointer_mut(self)
    }

    pub fn to_owned(&self) -> Pointer {
        Pointer::new(&self.target_type_uid, &self.target_value_uid)
    }
}

pub struct DynamicDataRef<'a> {
    type_uid: Entity,
    value_uid: Entity,
    column: Read<'a, Column>,
}

impl PartialEq for DynamicDataRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.type_uid == other.type_uid && self.value_uid == other.value_uid
    }
}

impl<'a> DynamicDataRef<'a> {
    pub fn new(type_uid: &Entity, value_uid: &Entity, column: Read<'a, Column>) -> Self {
        Self {
            type_uid: type_uid.clone(),
            value_uid: value_uid.clone(),
            column,
        }
    }

    pub fn type_uid(&self) -> &Entity {
        &self.type_uid
    }

    pub fn value_uid(&self) -> &Entity {
        &self.data().value_uid
    }

    pub fn data(&self) -> &DynamicData {
        self.column
            .as_dynamic()
            .unwrap()
            .get(&self.value_uid)
            .unwrap()
    }

    pub fn as_ref<T: Atom>(&self) -> Option<&T> {
        self.column.as_dynamic()?.get(&self.value_uid)?.as_ref()
    }

    pub fn to_owned(&self) -> Option<DynamicData> {
        Some(self.column.as_dynamic()?.get(&self.value_uid)?.to_owned())
    }
}

impl<'a> Debug for DynamicDataRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicDataRef")
            .field("type_uid", &self.type_uid)
            .field("value_uid", &self.value_uid)
            .finish()
    }
}

impl<'a> std::fmt::Display for DynamicDataRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DynamicDataRef")
    }
}

pub struct DynamicDataMut<'a> {
    type_uid: Entity,
    value_uid: Entity,
    column: Write<'a, Column>,
}

impl PartialEq for DynamicDataMut<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.type_uid == other.type_uid && self.value_uid == other.value_uid
    }
}

impl<'a> DynamicDataMut<'a> {
    pub fn new(type_uid: &Entity, value_uid: &Entity, column: Write<'a, Column>) -> Self {
        Self {
            type_uid: type_uid.clone(),
            value_uid: value_uid.clone(),
            column,
        }
    }

    pub fn type_uid(&self) -> &Entity {
        &self.type_uid
    }

    pub fn value_uid(&self) -> &Entity {
        &self.data().value_uid
    }

    pub fn as_ref<T: Atom>(&self) -> Option<&T> {
        self.column.as_dynamic()?.get(&self.value_uid)?.as_ref()
    }

    pub fn data(&self) -> &DynamicData {
        self.column
            .as_dynamic()
            .unwrap()
            .get(&self.value_uid)
            .unwrap()
    }

    pub fn data_mut(&mut self) -> &mut DynamicData {
        self.column
            .as_dynamic_mut()
            .unwrap()
            .get_mut(&self.value_uid)
            .unwrap()
    }

    pub fn as_mut<T: Atom>(&mut self) -> Option<&mut T> {
        self.column
            .as_dynamic_mut()?
            .get_mut(&self.value_uid)?
            .as_mut()
    }
}

impl<'a> Debug for DynamicDataMut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicDataMut")
            .field("type_uid", &self.type_uid)
            .field("value_uid", &self.value_uid)
            .finish()
    }
}

impl<'a> std::fmt::Display for DynamicDataMut<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DynamicDataMut")
    }
}

#[derive(Debug, PartialEq)]
pub enum Ref<'a> {
    Pointer(PointerRef<'a>),
    Dynamic(DynamicDataRef<'a>),
}

impl<'a> Ref<'a> {
    pub fn type_uid(&self) -> &Entity {
        match self {
            Self::Pointer(pointer) => pointer.target_type_uid(),
            Self::Dynamic(data) => data.type_uid(),
        }
    }

    pub fn value_uid(&self) -> &Entity {
        match self {
            Self::Pointer(pointer) => pointer.target_value_uid(),
            Self::Dynamic(data) => data.value_uid(),
        }
    }

    pub fn as_pointer(&self) -> Option<&PointerRef<'a>> {
        match self {
            Self::Pointer(pointer) => Some(pointer),
            Self::Dynamic(_) => None,
        }
    }

    pub fn as_dynamic(&self) -> Option<&DynamicDataRef<'a>> {
        match self {
            Self::Pointer(_) => None,
            Self::Dynamic(data) => Some(data),
        }
    }

    pub fn as_ref<T: Atom>(&self) -> Option<&T> {
        match self {
            Self::Pointer(_) => None,
            Self::Dynamic(data) => data.as_ref(),
        }
    }

    pub fn to_owned(&self) -> Option<Data> {
        match self {
            Self::Pointer(pointer) => Some(Data::Pointer(pointer.to_owned())),
            Self::Dynamic(data) => Some(Data::Dynamic(data.to_owned()?)),
        }
    }
}

#[derive(Debug)]
pub enum Mut<'a> {
    Pointer(PointerMut<'a>),
    Dynamic(DynamicDataMut<'a>),
}

impl<'a> Mut<'a> {
    pub fn type_uid(&self) -> &Entity {
        match self {
            Self::Pointer(pointer) => pointer.target_type_uid(),
            Self::Dynamic(data) => data.type_uid(),
        }
    }

    pub fn value_uid(&self) -> &Entity {
        match self {
            Self::Pointer(pointer) => pointer.target_value_uid(),
            Self::Dynamic(data) => data.value_uid(),
        }
    }

    pub fn as_pointer_mut(&self) -> Option<&PointerMut<'a>> {
        match self {
            Self::Pointer(pointer) => Some(pointer),
            Self::Dynamic(_) => None,
        }
    }

    pub fn as_dynamic(&self) -> Option<&DynamicDataMut<'a>> {
        match self {
            Self::Pointer(_) => None,
            Self::Dynamic(data) => Some(data),
        }
    }

    pub fn as_dynamic_mut(&mut self) -> Option<&mut DynamicDataMut<'a>> {
        match self {
            Self::Pointer(_) => None,
            Self::Dynamic(data) => Some(data),
        }
    }

    pub fn as_ref<T: Atom>(&self) -> Option<&T> {
        match self {
            Self::Pointer(_) => None,
            Self::Dynamic(data) => data.as_ref(),
        }
    }

    pub fn as_mut<T: Atom>(&mut self) -> Option<&mut T> {
        match self {
            Self::Pointer(_) => None,
            Self::Dynamic(data) => data.as_mut(),
        }
    }

    pub fn to_owned(&self) -> Data {
        match self {
            Self::Pointer(pointer) => Data::Pointer(pointer.to_owned()),
            Self::Dynamic(_) => todo!(),
        }
    }
}

pub struct DynamicColumn {
    type_uid: Entity,
    entity_ids: Vec<Entity>,
    dense: Vec<DynamicData>,
    sparse: Vec<Option<usize>>,
}

impl DynamicColumn {
    pub fn new(type_uid: Entity) -> Self {
        Self {
            type_uid,
            entity_ids: vec![],
            dense: Vec::new(),
            sparse: Vec::new(),
        }
    }

    pub fn type_uid(&self) -> &Entity {
        &self.type_uid
    }

    pub fn data_uids(&self) -> Vec<&Entity> {
        self.dense.iter().map(|data| &data.value_uid).collect()
    }

    pub fn dense_index_of(&self, entity: &Entity) -> Option<usize> {
        if entity.as_usize() >= self.sparse.len() {
            return None;
        }

        self.sparse[entity.as_usize()]
    }

    pub fn insert(&mut self, entity: &Entity, data: DynamicData) -> Result<()> {
        if data.type_uid() != &self.type_uid {
            bail!(
                "attempted to insert data of type {:?} into column of type {:?}",
                data.type_uid(),
                self.type_uid
            );
        }
        match self.dense_index_of(entity) {
            Some(_) => {
                bail!("attempted to insert data into column that already contains data for entity {:?}", entity);
            }
            None => {
                let dense_index = self.dense.len();

                if entity.as_usize() >= self.sparse.len() {
                    self.sparse.resize(entity.as_usize() + 1, None);
                }
                self.sparse[entity.as_usize()] = Some(dense_index);
                self.entity_ids.push(entity.clone());
                self.dense.push(data);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, entity: &Entity) -> Option<DynamicData> {
        if entity.as_usize() >= self.sparse.len() {
            return None;
        }
        let dense_index = self.sparse[entity.as_usize()].take()?;

        let value = self.dense.swap_remove(dense_index);
        let _index = self.entity_ids.swap_remove(dense_index);

        if dense_index != self.dense.len() {
            let swapped_index = &self.entity_ids[dense_index];
            self.sparse[swapped_index.as_usize()] = Some(dense_index);
        }

        Some(value)
    }

    pub fn get(&self, entity: &Entity) -> Option<&DynamicData> {
        if entity.as_usize() >= self.sparse.len() {
            return None;
        }
        let dense_index = self.sparse[entity.as_usize()]?;
        let value = self.dense.get(dense_index)?;
        Some(value)
    }

    pub fn get_mut(&mut self, entity: &Entity) -> Option<&mut DynamicData> {
        if entity.as_usize() >= self.sparse.len() {
            return None;
        }
        let dense_index = self.sparse[entity.as_usize()]?;
        let value = self.dense.get_mut(dense_index)?;
        Some(value)
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = &Entity> {
        self.entity_ids
            .iter()
            .filter(|entity| !entity.is_wildcard())
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.sparse
            .get(entity.as_usize())
            .copied()
            .flatten()
            .is_some()
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &DynamicData> {
        self.dense.iter()
    }

    pub fn find_entity_with(&self, value_id: &Entity) -> Option<&Entity> {
        self.entity_ids
            .iter()
            .find(|entity| self.get(entity).unwrap().value_uid() == value_id)
    }

    pub fn garbage_collect(&mut self) {
        let mut i = 0;
        while i < self.dense.len() {
            if self.dense[i].value_uid.is_dead() {
                self.remove(&self.entity_ids[i].clone());
            } else {
                i += 1;
            }
        }
    }
}

impl Debug for DynamicColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynamicColumn")
            .field("type_uid", &self.type_uid)
            .field("len", &self.len())
            .finish()
    }
}

#[derive(Clone)]
pub struct PointerColumn {
    target_type_uid: Entity,
    entity_ids: Vec<Entity>,
    dense: Vec<Pointer>,
    sparse: Vec<Option<usize>>,
}

impl PointerColumn {
    pub fn new(target_type_uid: Entity) -> Self {
        Self {
            target_type_uid,
            entity_ids: vec![],
            dense: Vec::new(),
            sparse: Vec::new(),
        }
    }

    pub fn target_type_uid(&self) -> &Entity {
        &self.target_type_uid
    }

    pub fn dense_index_of(&self, entity: &Entity) -> Option<usize> {
        if entity.as_usize() >= self.sparse.len() {
            return None;
        }

        self.sparse[entity.as_usize()]
    }

    pub fn insert(&mut self, entity: &Entity, pointer: Pointer) -> Result<()> {
        if pointer.target_type_uid() != &self.target_type_uid {
            bail!(
                "attempted to insert pointer to type {:?} into column of type {:?}",
                pointer.target_type_uid(),
                self.target_type_uid
            );
        }
        match self.dense_index_of(entity) {
            Some(_) => {
                bail!("attempted to insert data into column that already contains data for entity {:?}", entity);
            }
            None => {
                let dense_index = self.dense.len();

                if entity.as_usize() >= self.sparse.len() {
                    self.sparse.resize(entity.as_usize() + 1, None);
                }
                self.sparse[entity.as_usize()] = Some(dense_index);
                self.entity_ids.push(entity.clone());
                self.dense.push(pointer);
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, entity: &Entity) -> Option<Pointer> {
        if entity.as_usize() >= self.sparse.len() {
            return None;
        }
        let dense_index = self.sparse[entity.as_usize()].take()?;

        let value = self.dense.swap_remove(dense_index);
        let _index = self.entity_ids.swap_remove(dense_index);

        if dense_index != self.dense.len() {
            let swapped_index = &self.entity_ids[dense_index];
            self.sparse[swapped_index.as_usize()] = Some(dense_index);
        }

        Some(value)
    }

    pub fn get(&self, entity: &Entity) -> Option<&Pointer> {
        if entity.as_usize() >= self.sparse.len() {
            return None;
        }
        let dense_index = self.sparse[entity.as_usize()]?;
        let value = self.dense.get(dense_index)?;
        Some(value)
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = &Entity> {
        self.entity_ids.iter()
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.sparse
            .get(entity.as_usize())
            .copied()
            .flatten()
            .is_some()
    }

    pub fn len(&self) -> usize {
        self.dense.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dense.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pointer> {
        self.dense.iter()
    }

    pub fn find_entity_with_pointer_to(&self, target_value_uid: &Entity) -> Option<&Entity> {
        self.entity_ids
            .iter()
            .find(|entity| *self.get(entity).unwrap().target_value_uid() == *target_value_uid)
    }

    pub fn garbage_collect(&mut self) {
        let mut i = 0;
        while i < self.dense.len() {
            if self.dense[i].target_value_uid.is_dead() {
                self.remove(&self.entity_ids[i].clone());
            } else {
                i += 1;
            }
        }
    }
}

impl Debug for PointerColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointerColumn")
            .field("target_type_uid", &self.target_type_uid)
            .field("len", &self.len())
            .finish()
    }
}

#[derive(Debug)]
pub enum Column {
    Dynamic(DynamicColumn),
    Pointer(PointerColumn),
}

impl Column {
    pub fn type_uid(&self) -> &Entity {
        match self {
            Self::Dynamic(col) => col.type_uid(),
            Self::Pointer(col) => col.target_type_uid(),
        }
    }

    pub fn dense_index_of(&self, entity: &Entity) -> Option<usize> {
        match self {
            Self::Dynamic(col) => col.dense_index_of(entity),
            Self::Pointer(col) => col.dense_index_of(entity),
        }
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        match self {
            Self::Dynamic(col) => col.contains(entity),
            Self::Pointer(col) => col.contains(entity),
        }
    }

    pub fn as_dynamic(&self) -> Option<&DynamicColumn> {
        match self {
            Self::Dynamic(col) => Some(col),
            Self::Pointer(_) => None,
        }
    }

    pub fn as_dynamic_mut(&mut self) -> Option<&mut DynamicColumn> {
        match self {
            Self::Dynamic(col) => Some(col),
            Self::Pointer(_) => None,
        }
    }

    pub fn as_pointer(&self) -> Option<&PointerColumn> {
        match self {
            Self::Dynamic(_) => None,
            Self::Pointer(col) => Some(col),
        }
    }

    pub fn as_pointer_mut(&mut self) -> Option<&mut PointerColumn> {
        match self {
            Self::Dynamic(_) => None,
            Self::Pointer(col) => Some(col),
        }
    }

    pub fn entity_ids(&self) -> Vec<&Entity> {
        match self {
            Self::Dynamic(col) => col.entity_iter().collect(),
            Self::Pointer(col) => col.entity_iter().collect(),
        }
    }

    pub fn garbage_collect(&mut self) {
        match self {
            Self::Dynamic(col) => col.garbage_collect(),
            Self::Pointer(col) => col.garbage_collect(),
        }
    }
}

#[derive(Debug)]
pub struct LockedColumn(Lock<Column>);

impl LockedColumn {
    pub fn read(&self) -> Read<'_, Column> {
        self.0.read()
    }

    pub fn write(&self) -> Write<'_, Column> {
        self.0.write()
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.0.read().contains(entity)
    }

    pub fn entity_ids(&self) -> Vec<Entity> {
        self.0.read().entity_ids().into_iter().cloned().collect()
    }

    pub fn get<T: Atom>(&self, entity: &Entity) -> Option<MapRead<'_, T>> {
        if entity.is_dead() {
            return None;
        }
        let index = entity.as_usize();

        let col_lock = self.0.read();
        let col = col_lock.as_dynamic()?;

        if !col.contains(entity) {
            return None;
        }

        let component_id = T::static_type_uid();
        if col.type_uid() != &component_id {
            return None;
        }

        drop(col_lock);

        Some(self.0.map_read(|col| {
            let col = col.as_dynamic().unwrap();
            let dense_index = col.sparse.get(index).copied().flatten().unwrap();
            let value = col.dense.get(dense_index).unwrap();
            value.as_ref().unwrap()
        }))
    }

    pub fn get_mut<T: Atom>(&self, entity: &Entity) -> Option<MapWrite<'_, T>> {
        if entity.is_dead() {
            return None;
        }
        let index = entity.as_usize();

        let col_lock = self.0.read();
        let col = col_lock.as_dynamic()?;

        if !col.contains(entity) {
            return None;
        }

        let component_id = T::static_type_uid();
        if col.type_uid() != &component_id {
            return None;
        }

        drop(col_lock);

        Some(self.0.map_write(|col| {
            let col = col.as_dynamic_mut().unwrap();
            let dense_index = col.sparse.get(index).copied().flatten().unwrap();
            let value = col.dense.get_mut(dense_index).unwrap();
            value.as_mut().unwrap()
        }))
    }

    pub fn get_dynamic(&self, entity: &Entity) -> Option<DynamicDataRef<'_>> {
        let col_lock = self.0.read();
        let col = col_lock.as_dynamic()?;

        if !col.contains(entity) {
            return None;
        }

        let data = DynamicDataRef::new(&col.type_uid().clone(), entity, col_lock);
        Some(data)
    }

    pub fn get_dynamic_mut(&self, entity: &Entity) -> Option<DynamicDataMut<'_>> {
        let col_lock = self.0.read();

        if !col_lock.contains(entity) {
            return None;
        }

        drop(col_lock);
        let col_lock = self.0.write();
        let data = DynamicDataMut::new(&col_lock.type_uid().clone(), entity, col_lock);
        Some(data)
    }

    pub fn get_pointer(&self, entity: &Entity) -> Option<PointerRef<'_>> {
        let index = entity.as_usize();

        let col_lock = self.0.read();
        let col = col_lock.as_pointer()?;

        if !col.contains(entity) {
            return None;
        }

        let dense_index = col.sparse.get(index).copied().flatten()?;

        let pointer = col.dense.get(dense_index)?;
        let target_type_uid = pointer.target_type_uid().clone();
        let target_value_uid = pointer.target_value_uid().clone();
        let pointer = PointerRef::new(&target_type_uid, &target_value_uid, col_lock);
        Some(pointer)
    }

    pub fn get_pointer_mut(&self, entity: &Entity) -> Option<PointerMut<'_>> {
        let index = entity.as_usize();

        let col_lock = self.0.read();
        let col = col_lock.as_pointer()?;

        if !col.contains(entity) {
            return None;
        }

        let dense_index = col.sparse.get(index).copied().flatten()?;

        let pointer = col.dense.get(dense_index)?;
        let target_type_uid = pointer.target_type_uid().clone();
        let target_value_uid = pointer.target_value_uid().clone();
        drop(col_lock);
        let col_lock = self.0.write();
        let pointer = PointerMut::new(&target_type_uid, &target_value_uid, col_lock);
        Some(pointer)
    }
}

/// Storage for a sparse set of indices and a dense array of contiguous arrays of instances of a (single) storable type.
#[derive(Debug, Default)]
pub struct Archetype {
    archetype_id: Uid,
    entity_ids: FxHashSet<Entity>,
    type_columns: FxHashMap<Entity, LockedColumn>,
}

impl Archetype {
    pub fn archetype_id(&self) -> Uid {
        self.archetype_id
    }

    #[allow(clippy::map_identity)] // false positive
    pub fn type_columns(&self) -> impl Iterator<Item = (&Entity, &LockedColumn)> {
        self.type_columns.iter().map(|(k, v)| (k, v))
    }

    pub fn column(&self, type_id: &Entity) -> Option<&LockedColumn> {
        self.type_columns.get(type_id)
    }

    pub fn row(&self, entity: &Entity) -> Option<Vec<Ref<'_>>> {
        let mut row = Vec::new();
        for (_, column) in self.type_columns() {
            let data = column.get_dynamic(entity)?;
            row.push(Ref::Dynamic(data));
        }
        Some(row)
    }

    pub fn row_mut(&self, entity: &Entity) -> Option<Vec<Mut<'_>>> {
        let mut row = Vec::new();
        for (_, column) in self.type_columns() {
            let data = column.get_dynamic_mut(entity)?;
            row.push(Mut::Dynamic(data));
        }
        Some(row)
    }

    pub fn row_type_filtered<F>(&self, entity: &Entity, filter_types: F) -> Option<Vec<Ref<'_>>>
    where
        F: Fn(&Entity) -> bool,
    {
        let mut row = Vec::new();
        for (type_id, column) in self.type_columns() {
            if filter_types(type_id) {
                let data = column.get_dynamic(entity)?;
                row.push(Ref::Dynamic(data));
            }
        }
        Some(row)
    }

    pub fn entity_iter(&self) -> impl Iterator<Item = &Entity> + '_ {
        self.entity_ids.iter()
    }

    pub fn contains_entity(&self, entity: &Entity) -> bool {
        self.entity_ids.contains(entity)
    }

    pub fn contains_all_entities(&self, entities: &[Entity]) -> bool {
        entities.iter().all(|i| self.contains_entity(i))
    }

    pub fn contains_type(&self, type_id: &Entity) -> bool {
        self.type_columns.contains_key(type_id)
            || if type_id.is_wildcard() {
                self.type_columns
                    .iter()
                    .any(|(k, _)| k.id() == type_id.id())
            } else {
                false
            }
    }

    pub fn contains_all_types(&self, type_ids: &[Entity]) -> bool {
        type_ids.iter().all(|i| self.contains_type(i))
    }

    pub fn contains_any_type(&self, type_ids: &[Entity]) -> bool {
        type_ids.iter().any(|i| self.contains_type(i))
    }

    pub fn exclusively_contains_types(&self, type_id: &[Entity]) -> bool {
        self.contains_all_types(type_id) && self.type_columns.len() == type_id.len()
    }

    pub fn has_no_entities(&self) -> bool {
        self.entity_ids.is_empty()
    }

    pub fn has_no_types(&self) -> bool {
        self.type_columns.is_empty()
    }

    pub fn clear(&mut self) {
        self.type_columns.clear();
    }

    pub fn get(&self, type_id: &Entity, entity: &Entity) -> Option<Ref<'_>> {
        let column = self.type_columns.get(type_id)?;
        let is_dynamic = matches!(&*column.read(), Column::Dynamic(_));
        if is_dynamic {
            let data = column.get_dynamic(entity)?;
            Some(Ref::Dynamic(data))
        } else {
            let pointer = column.get_pointer(entity)?;
            Some(Ref::Pointer(pointer))
        }
    }

    pub fn get_mut(&self, type_id: &Entity, entity: &Entity) -> Option<Mut<'_>> {
        let column = self.type_columns.get(type_id)?;
        let is_dynamic = matches!(&*column.read(), Column::Dynamic(_));
        if is_dynamic {
            let data = column.get_dynamic_mut(entity)?;
            Some(Mut::Dynamic(data))
        } else {
            let pointer = column.get_pointer_mut(entity)?;
            Some(Mut::Pointer(pointer))
        }
    }

    pub fn find(&self, type_id: &Entity, value_id: &Entity) -> Option<Ref<'_>> {
        let column = self.type_columns.get(type_id)?;

        let is_dynamic = matches!(&*column.read(), Column::Dynamic(_));

        if is_dynamic {
            let entity = column
                .read()
                .as_dynamic()
                .unwrap()
                .find_entity_with(value_id)?
                .clone();
            let data = column.get_dynamic(&entity)?;
            Some(Ref::Dynamic(data))
        } else {
            let entity = column
                .read()
                .as_pointer()
                .unwrap()
                .find_entity_with_pointer_to(value_id)?
                .clone();
            let pointer = column.get_pointer(&entity)?;
            Some(Ref::Pointer(pointer))
        }
    }

    pub fn find_mut(&self, type_id: &Entity, value_id: &Entity) -> Option<Mut<'_>> {
        let column = self.type_columns.get(type_id)?;

        let is_dynamic = matches!(&*column.read(), Column::Dynamic(_));

        if is_dynamic {
            let entity = column
                .read()
                .as_dynamic()
                .unwrap()
                .find_entity_with(value_id)?
                .clone();
            let data = column.get_dynamic_mut(&entity)?;
            Some(Mut::Dynamic(data))
        } else {
            let entity = column
                .read()
                .as_pointer()
                .unwrap()
                .find_entity_with_pointer_to(value_id)?
                .clone();
            let pointer = column.get_pointer_mut(&entity)?;
            Some(Mut::Pointer(pointer))
        }
    }

    pub fn remove_entity(&mut self, entity: &Entity) -> Option<Vec<Data>> {
        if !self.entity_ids.remove(entity) {
            return None;
        }
        let mut data = Vec::new();
        for column in self.type_columns.values_mut() {
            match &mut *column.write() {
                Column::Dynamic(col) => {
                    if let Some(d) = col.remove(entity) {
                        data.push(Data::Dynamic(d));
                    }
                }
                Column::Pointer(col) => {
                    if let Some(p) = col.remove(entity) {
                        data.push(Data::Pointer(p));
                    }
                }
            }
        }
        Some(data)
    }

    pub fn garbage_collect(&mut self) {
        let mut entities = self.entity_ids.clone().into_iter().collect::<Vec<_>>();
        while let Some(entity) = entities.pop() {
            if entity.is_dead() {
                self.remove_entity(&entity);
            }
        }
        for column in self.type_columns.values_mut() {
            column.write().garbage_collect();
        }
    }
}

#[derive(Debug)]
pub struct Storage {
    archetypes: SortedMap<Uid, Archetype>,
    entity_archetypes: SortedMap<Entity, Uid>,
}

impl Storage {
    pub fn new() -> Self {
        Self {
            archetypes: SortedMap::default(),
            entity_archetypes: SortedMap::default(),
        }
    }

    pub fn archetype(&self, archetype_id: &Uid) -> Option<&Archetype> {
        self.archetypes.get(archetype_id)
    }

    pub fn archetypes(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.values()
    }

    pub fn entity_archetype(&self, entity: &Entity) -> Option<&Archetype> {
        let archetype_id = self.entity_archetypes.get(entity)?;
        self.archetypes.get(archetype_id)
    }

    pub fn entity_archetypes(&self) -> impl Iterator<Item = &Archetype> {
        self.entity_archetypes
            .iter()
            .filter_map(|(_, a)| self.archetypes.get(a))
    }

    pub fn contains_entity(&self, entity: &Entity) -> bool {
        self.entity_archetypes.contains(entity)
    }

    pub fn num_entities(&self) -> usize {
        self.entity_archetypes.len()
    }

    pub fn num_archetypes(&self) -> usize {
        self.archetypes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entity_archetypes.is_empty() && self.archetypes.is_empty()
    }

    pub fn clear(&mut self) {
        self.entity_archetypes.clear();
        self.archetypes.clear();
    }

    pub fn has<T: Atom>(&self, entity: &Entity) -> bool {
        self.entity_archetype(entity)
            .map(|a| a.contains_type(&T::static_type_uid()))
            .unwrap_or(false)
    }

    pub fn get(&self, type_id: &Entity, entity: &Entity) -> Option<Ref<'_>> {
        let archetype = self.entity_archetype(entity)?;
        archetype.get(type_id, entity)
    }

    pub fn get_mut(&self, type_id: &Entity, entity: &Entity) -> Option<Mut<'_>> {
        let archetype = self.entity_archetype(entity)?;
        archetype.get_mut(type_id, entity)
    }

    pub fn get_component<T: Atom>(&self, entity: &Entity) -> Option<Ref<'_>> {
        self.get(&T::static_type_uid(), entity)
    }

    pub fn get_component_mut<T: Atom>(&self, entity: &Entity) -> Option<Mut<'_>> {
        self.get_mut(&T::static_type_uid(), entity)
    }

    pub fn find(&self, type_id: &Entity, value_id: &Entity) -> Option<Ref<'_>> {
        self.archetypes
            .iter()
            .filter(|(_, a)| a.contains_type(type_id))
            .find_map(|(_, a)| a.find(type_id, value_id))
    }

    pub fn find_untyped(&self, value_id: &Entity) -> Option<Ref<'_>> {
        if let Some(type_id) = value_id.type_id() {
            // short path
            self.find(&type_id, value_id)
        } else {
            // give it our best effort
            self.archetypes().find_map(|a| {
                a.type_columns().find_map(|(_, c)| {
                    let c_lock = c.read();
                    if let Some(c_lock) = c_lock.as_dynamic() {
                        if c_lock.data_uids().contains(&value_id) {
                            let e = c_lock.find_entity_with(value_id)?;

                            let d = c.get_dynamic(e)?;
                            Some(Ref::Dynamic(d))
                        } else {
                            None
                        }
                    } else if let Some(c_lock) = c_lock.as_pointer() {
                        if c_lock
                            .dense
                            .iter()
                            .any(|p| p.target_value_uid() == value_id)
                        {
                            let e = c_lock.find_entity_with_pointer_to(value_id)?;

                            let p = c.get_pointer(e)?;
                            Some(Ref::Pointer(p))
                        } else {
                            None
                        }
                    } else {
                        unreachable!()
                    }
                })
            })
        }
    }

    pub fn find_mut(&self, type_id: &Entity, value_id: &Entity) -> Option<Mut<'_>> {
        self.archetypes
            .iter()
            .filter(|(_, a)| a.contains_type(type_id))
            .find_map(|(_, a)| a.find_mut(type_id, value_id))
    }

    pub fn find_untyped_mut(&self, value_id: &Entity) -> Option<Mut<'_>> {
        if let Some(type_id) = value_id.type_id() {
            // short path
            self.find_mut(&type_id, value_id)
        } else {
            // give it our best effort
            self.archetypes().find_map(|a| {
                a.type_columns().find_map(|(_, c)| {
                    let c_lock = c.read();
                    if let Some(c_lock) = c_lock.as_dynamic() {
                        if c_lock.data_uids().contains(&value_id) {
                            let e = c_lock.find_entity_with(value_id)?;

                            let d = c.get_dynamic_mut(e)?;
                            Some(Mut::Dynamic(d))
                        } else {
                            None
                        }
                    } else {
                        todo!()
                    }
                })
            })
        }
    }

    pub fn deref_pointer_ref(&self, pointer: &PointerRef<'_>) -> Option<Ref<'_>> {
        let archetype = self
            .archetypes()
            .find(|a| a.contains_entity(pointer.target_value_uid()))?;
        archetype.get(pointer.target_type_uid(), pointer.target_value_uid())
    }

    pub fn deref_pointer_mut(&self, pointer: &PointerMut<'_>) -> Option<Mut<'_>> {
        let archetype = self
            .archetypes()
            .filter(|a| a.contains_type(pointer.target_type_uid()))
            .find(|a| a.contains_entity(pointer.target_value_uid()))?;
        archetype.get_mut(pointer.target_type_uid(), pointer.target_value_uid())
    }

    pub fn insert_entity(&mut self, entity: Entity) -> Result<()> {
        if self.entity_archetypes.contains(&entity) {
            bail!("entity already exists in storage: {:?}", entity);
        }
        self.entity_archetypes.insert(entity, Uid::default());
        Ok(())
    }

    pub fn create_entity(&mut self) -> Entity {
        let entity = Entity::allocate(None);
        self.entity_archetypes
            .insert(entity.clone(), Uid::default());
        entity
    }

    pub fn create_entity_with(&mut self, data: impl IntoIterator<Item = Data>) -> Result<Entity> {
        let entity = Entity::allocate(None);
        self.insert(&entity, data)?;
        Ok(entity)
    }

    pub fn destroy_entity(&mut self, entity: &Entity) -> Option<Vec<Data>> {
        self.entity_archetypes.remove(entity)?;
        let mut data = Vec::new();
        for archetype in self.archetypes.values_mut() {
            if archetype.contains_entity(entity) {
                data.extend(archetype.remove_entity(entity)?);
            }
        }
        Some(data)
    }

    pub fn garbage_collect(&mut self) {
        let entities = self.entity_archetypes.keys().cloned().collect::<Vec<_>>();
        for entity in entities {
            if entity.is_dead() {
                self.destroy_entity(&entity);
            }
        }

        // remove empty archetypes
        let archetypes = self.archetypes.keys().copied().collect::<Vec<_>>();
        for archetype in archetypes {
            if self.archetype(&archetype).unwrap().has_no_entities() {
                self.archetypes.remove(&archetype);
            }
        }
    }

    pub fn insert(
        &mut self,
        entity: &Entity,
        new_data: impl IntoIterator<Item = Data>,
    ) -> Result<()> {
        let old_archetype_id = self.entity_archetypes.remove(entity);
        let old_archetype = old_archetype_id.and_then(|id| self.archetypes.get_mut(&id));

        let new_data = new_data.into_iter().collect::<Vec<_>>();

        let data = if let Some(old_archetype) = old_archetype {
            // remove entity from old archetype
            let mut data = old_archetype.remove_entity(entity).unwrap();
            // check for duplicate types
            for new in &new_data {
                if data
                    .iter()
                    .any(|d| d.type_uid() == new.type_uid() && !d.type_uid().is_wildcard())
                {
                    bail!(
                        "attempted to insert duplicate type into entity: {:?}",
                        new.type_uid()
                    );
                }
            }
            data.extend(new_data);

            data
        } else {
            new_data
        };

        // check if we already have an archetype with the same types
        let existing_archetype = self.archetypes.iter_mut().find_map(|(_, a)| {
            if a.exclusively_contains_types(
                &data
                    .iter()
                    .map(|d| d.type_uid().clone())
                    .collect::<Vec<_>>(),
            ) {
                Some(a)
            } else {
                None
            }
        });

        let archetype = if let Some(existing_archetype) = existing_archetype {
            existing_archetype
        } else {
            let mut type_columns = FxHashMap::default();
            for d in &data {
                let type_id = d.type_uid();
                let column = match d {
                    Data::Dynamic(d) => {
                        let column = DynamicColumn::new(d.type_uid().clone());
                        LockedColumn(Lock::new(Column::Dynamic(column)))
                    }
                    Data::Pointer(d) => {
                        let column = PointerColumn::new(d.target_type_uid().clone());
                        LockedColumn(Lock::new(Column::Pointer(column)))
                    }
                };
                type_columns.insert(type_id.clone(), column);
            }

            let archetype_id = Uid::allocate();
            let archetype = Archetype {
                archetype_id,
                type_columns,
                entity_ids: FxHashSet::default(),
            };
            self.archetypes.insert(archetype_id, archetype);
            self.archetypes.get_mut(&archetype_id).unwrap()
        };

        // insert entity into archetype
        archetype.entity_ids.insert(entity.clone());
        for d in data {
            let type_id = d.type_uid();
            let col = archetype.column(type_id).unwrap();
            // col.write().insert(entity, d);
            match &mut *col.write() {
                Column::Dynamic(col) => {
                    col.insert(entity, d.into_dynamic_data().unwrap())?;
                }
                Column::Pointer(col) => {
                    col.insert(entity, d.into_pointer().unwrap())?;
                }
            }
        }

        // insert entity into entity_archetypes
        self.entity_archetypes
            .insert(entity.clone(), archetype.archetype_id);

        Ok(())
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

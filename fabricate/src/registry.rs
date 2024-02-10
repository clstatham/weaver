use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::{BuildHasherDefault, Hash},
    ops::{Deref, Range},
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use anyhow::Result;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    bundle::Bundle,
    commands::Commands,
    component::Atom,
    lock::Lock,
    relationship::Relationship,
    storage::{Mut, Ref, SortedMap},
    world::{get_world, World},
};

/// A unique identifier.
/// This is a wrapper around a [`u32`] that is used to uniquely identify entities, components, and other resources.

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    bytemuck::Pod,
    bytemuck::Zeroable,
    derive_more::Add,
    derive_more::Sub,
    derive_more::Display,
    derive_more::From,
    derive_more::Into,
    derive_more::Mul,
    derive_more::BitAnd,
    derive_more::BitOr,
    derive_more::BitXor,
    derive_more::Not,
    derive_more::Sum,
)]
#[repr(transparent)]
pub struct Id(u32);

impl Id {
    pub const PLACEHOLDER: Self = Self(u32::MAX);
    pub const WILDCARD: Self = Self(VALUE_INFO_MASK);

    const fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn allocate() -> Self {
        global_registry().allocate()
    }

    pub const fn as_u32(self) -> u32 {
        self.0
    }

    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    pub fn is_placeholder(self) -> bool {
        self == Self::PLACEHOLDER
    }

    pub fn is_wildcard(self) -> bool {
        self == Self::WILDCARD
    }

    pub fn check_placeholder(self) -> Option<Self> {
        if self.is_placeholder() {
            None
        } else {
            Some(self)
        }
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::PLACEHOLDER
    }
}

pub const METADATA_SHIFT: u32 = 28;
pub const METADATA_BITS: Range<u32> = METADATA_SHIFT..32;
pub const GENERATION_BITS: u32 = 1 << METADATA_SHIFT;
pub const RELATIVE_BITS: u32 = 2 << METADATA_SHIFT;
pub const TYPE_BITS: u32 = 3 << METADATA_SHIFT;
pub const VALUE_METADATA_MASK: u32 = !((1 << METADATA_SHIFT) - 1);
pub const VALUE_INFO_MASK: u32 = !VALUE_METADATA_MASK;
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct EntityMeta(Id);

impl EntityMeta {
    pub const PLACEHOLDER: Self = Self(Id::PLACEHOLDER);
    pub const WILDCARD: Self = Self(Id(VALUE_INFO_MASK));

    pub const fn new_relative(id: Id) -> Self {
        Self(Id(id.as_u32() | RELATIVE_BITS))
    }

    pub const fn new_generation(generation: Id) -> Self {
        Self(Id(generation.as_u32() | GENERATION_BITS))
    }

    pub const fn new_type(id: Id) -> Self {
        Self(Id(id.as_u32() | TYPE_BITS))
    }

    pub const fn value(&self) -> Id {
        Id(self.0.as_u32() & VALUE_INFO_MASK)
    }

    pub const fn is_relative(&self) -> bool {
        self.0.as_u32() & VALUE_METADATA_MASK == RELATIVE_BITS
    }

    pub const fn is_generation(&self) -> bool {
        self.0.as_u32() & VALUE_METADATA_MASK == GENERATION_BITS
    }

    pub const fn is_type(&self) -> bool {
        self.0.as_u32() & VALUE_METADATA_MASK == TYPE_BITS
    }

    pub const fn is_wildcard(&self) -> bool {
        self.0.as_u32() == Self::WILDCARD.0.as_u32()
    }
}

impl Default for EntityMeta {
    fn default() -> Self {
        Self::PLACEHOLDER
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for EntityMeta {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.value().partial_cmp(&other.value())
    }
}

impl Ord for EntityMeta {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value().cmp(&other.value())
    }
}

impl Debug for EntityMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_relative() {
            write!(f, "relative: {}", self.value())
        } else if self.is_generation() {
            write!(f, "generation: {}", self.value())
        } else if self.is_type() {
            write!(f, "type: {}", self.value())
        } else {
            write!(f, "unknown: {}", self.value())
        }
    }
}

impl Display for EntityMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_relative() {
            write!(f, "id: {}", self.value())
        } else if self.is_generation() {
            write!(f, "generation: {}", self.value())
        } else if self.is_type() {
            write!(f, "type: {}", self.value())
        } else {
            write!(f, "unknown: {}", self.value())
        }
    }
}

/// A unique identifier for a value.
/// This can be either a primitive value (e.g. `42`), or a dynamic/composite value (e.g. a struct instance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct Entity(Id, EntityMeta);

impl Entity {
    pub const fn placeholder() -> Self {
        Self(Id::PLACEHOLDER, EntityMeta::PLACEHOLDER)
    }

    pub const fn new_generational(id: Id, generation: Id) -> Self {
        Self(id, EntityMeta::new_generation(generation))
    }

    pub fn new_with_current_generation(id: Id) -> Option<Self> {
        global_registry().current_generation(id)
    }

    pub const fn new_relationship(relation: Id, relative: Id) -> Self {
        Self(relation, EntityMeta::new_relative(relative))
    }

    pub const fn new_type(entity: Id) -> Self {
        Self(entity, EntityMeta::new_type(entity))
    }

    pub const fn new_wildcard_id(entity: Id) -> Self {
        Self(entity, EntityMeta::WILDCARD)
    }

    pub fn new_wildcard<T: StaticId + ?Sized>() -> Self {
        Self::new_wildcard_id(T::static_type_id().id())
    }

    pub fn allocate(ty: Option<Entity>) -> Self {
        let this = if let Some(ty) = ty {
            global_registry().allocate_entity_with_type(ty)
        } else {
            global_registry().allocate_generational_entity()
        };
        log::trace!("Allocated value: {}", this);
        this
    }

    pub fn allocate_type(name: Option<&str>) -> Self {
        let this = global_registry().allocate_type(name);
        log::trace!("Allocated type: {}", this);
        this
    }

    pub fn type_from_name(name: &str) -> Option<Self> {
        global_registry().get_named_type(name)
    }

    pub const fn as_usize(self) -> usize {
        self.id().as_usize()
    }

    pub fn is_placeholder(self) -> bool {
        self.id() == Id::PLACEHOLDER
    }

    pub fn is_wildcard(self) -> bool {
        self.meta().is_wildcard()
    }

    pub const fn id(self) -> Id {
        self.0
    }

    pub const fn meta(self) -> EntityMeta {
        self.1
    }

    pub const fn is_relative(self) -> bool {
        self.meta().is_relative()
    }

    pub const fn is_generational(self) -> bool {
        self.meta().is_generation()
    }

    pub const fn is_type(self) -> bool {
        self.meta().is_type()
    }

    pub fn generation(self) -> Option<Id> {
        if self.meta().is_generation() {
            Some(self.meta().value())
        } else {
            None
        }
    }

    pub fn get_relative(self) -> Option<Id> {
        if self.meta().is_relative() {
            Some(self.meta().value())
        } else {
            None
        }
    }

    pub fn is_alive(self) -> bool {
        self.is_generational()
            && global_registry().entity_generations.read().get(&self.id())
                == Some(&self.meta().value())
    }

    pub fn is_dead(self) -> bool {
        !self.is_alive()
    }

    pub fn kill(self) {
        if self.is_generational() {
            log::trace!("Killing value: {}", self);
            global_registry().delete_entity(self);
        }
    }

    pub fn check_placeholder(self) -> Option<Self> {
        if self.is_placeholder() {
            None
        } else {
            Some(self)
        }
    }

    pub fn type_id(self) -> Option<Entity> {
        if self.is_type() {
            Some(self)
        } else {
            global_registry().get_value_type(self)
        }
    }

    pub fn register_as_type(self, typ: Entity) {
        debug_assert!(!self.is_type());
        debug_assert!(typ.is_type());
        global_registry().register_entity_as_type(self, typ);
    }

    pub fn type_name(self) -> Option<String> {
        self.type_id()
            .and_then(|entity| global_registry().get_type_name(entity))
    }

    pub fn register_type_name(self, name: &str) {
        if let Some(ty) = self.type_id() {
            global_registry().register_type_name(ty, name);
        }
    }

    pub fn with_world<F, R>(self, f: F) -> R
    where
        F: FnOnce(&World) -> R,
    {
        let world = get_world().read();
        f(&world)
    }

    pub fn defer<F, R>(self, f: F) -> Result<R>
    where
        F: FnOnce(&World, &mut Commands) -> R,
    {
        get_world().defer(f)
    }

    pub fn with_value_ref<F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce(Ref<'_>) -> R,
    {
        self.with_world(|world| {
            let storage = world.storage();
            let r = storage.find(self.type_id()?, self)?;
            Some(f(r))
        })
    }

    pub fn with_value_mut<F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce(Mut<'_>) -> R,
    {
        self.with_world(|world| {
            let storage = world.storage();
            let r = storage.find_mut(self.type_id()?, self)?;
            Some(f(r))
        })
    }

    pub fn with_self_as_ref<T: Atom, F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        self.with_value_ref(|r| {
            let r = r.as_ref::<T>()?;
            Some(f(r))
        })?
    }

    pub fn with_self_as_mut<T: Atom, F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.with_value_mut(|mut r| {
            let r = r.as_mut::<T>()?;
            Some(f(r))
        })?
    }

    pub fn is<T: Atom>(self) -> bool {
        self.type_id() == Some(T::type_id())
    }

    pub fn has<T: Atom>(self) -> bool {
        self.with_world(|world| world.has::<T>(self))
    }

    pub fn with_component_ref<T: Atom, R>(self, f: impl FnOnce(&T) -> R) -> Option<R> {
        self.with_world(|world| {
            let r = world.get(self, T::type_id())?;
            let r = r.as_ref::<T>()?;
            Some(f(r))
        })
    }

    pub fn with_component_mut<T: Atom, R>(self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        self.with_world(|world| {
            let mut r = world.get_mut(self, T::type_id())?;
            let r = r.as_mut::<T>()?;
            Some(f(r))
        })
    }

    pub fn with_relatives<F, R>(self, relationship_type: Id, f: F) -> Option<R>
    where
        F: FnOnce(&[Entity]) -> R,
    {
        let rels = self.with_world(|world| world.get_relatives_id(self, relationship_type))?;
        Some(f(&rels))
    }

    pub fn with_all_relatives<F, R>(self, f: F) -> Option<R>
    where
        F: FnOnce(&[(Id, Entity)]) -> R,
    {
        let rels = self.with_world(|world| world.all_relatives(self))?;
        Some(f(&rels))
    }

    pub fn add_relative<R: Relationship>(self, relationship: R, relative: Entity) -> Result<()> {
        self.defer(|_, commands| {
            commands.add_components(self, vec![relationship.into_relationship_data(relative)?]);
            Ok::<(), anyhow::Error>(())
        })??;
        Ok(())
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add<T: Bundle>(self, components: T) -> Result<()> {
        let data = components.into_data_vec();
        self.defer(|_, commands| {
            commands.add_components(self, data);
            Ok::<(), anyhow::Error>(())
        })??;
        Ok(())
    }
}

impl Display for Entity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = self.type_name() {
            write!(
                f,
                "Entity({}, {}, type: {}, type_id: {})",
                self.id(),
                self.meta(),
                name,
                self.type_id().unwrap().id()
            )
        } else if let Some(ty) = self.type_id() {
            write!(
                f,
                "Entity({}, {}, type_id: {})",
                self.id(),
                self.meta(),
                ty.id()
            )
        } else {
            write!(f, "Entity({}, {})", self.id(), self.meta())
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct TypeIdHasher(u64);

impl std::hash::Hasher for TypeIdHasher {
    fn write(&mut self, bytes: &[u8]) {
        self.0 = u64::from_le_bytes(bytes.try_into().unwrap());
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = i;
    }

    fn write_u128(&mut self, i: u128) {
        self.0 = i as u64;
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

pub trait StaticId: 'static {
    fn static_id() -> u64 {
        let mut hasher = TypeIdHasher::default();
        std::any::TypeId::of::<Self>().hash(&mut hasher);
        hasher.0
    }

    fn static_type_id() -> Entity {
        global_registry().get_or_register_static_type::<Self>()
    }

    fn register_static_name(name: &str) {
        Self::static_type_id().register_type_name(name);
    }

    fn static_type_name() -> Option<String> {
        Self::static_type_id().type_name()
    }
}

impl<T: std::any::Any> StaticId for T {}

/// A registry for storing and retrieving unique identifiers, and type-related information.
pub struct Registry {
    next_id: AtomicU32,

    static_types: Lock<HashMap<u64, Entity, BuildHasherDefault<TypeIdHasher>>>,
    storable_types: Lock<FxHashSet<Entity>>,
    named_types: Lock<FxHashMap<String, Entity>>,
    type_names: Lock<FxHashMap<Entity, String>>,

    entity_types: Lock<FxHashMap<Entity, Entity>>,
    dead: Lock<SortedMap<Id, ()>>,
    entity_generations: Lock<FxHashMap<Id, Id>>,
}

impl Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("next_id", &self.next_id)
            .finish()
    }
}

impl Registry {
    pub fn new() -> Self {
        let this = Self {
            next_id: AtomicU32::new(0),
            static_types: Lock::new(HashMap::default()),
            storable_types: Lock::new(FxHashSet::default()),
            named_types: Lock::new(FxHashMap::default()),
            type_names: Lock::new(FxHashMap::default()),

            entity_types: Lock::new(FxHashMap::default()),
            dead: Lock::new(SortedMap::default()),
            entity_generations: Lock::new(FxHashMap::default()),
        };

        let storable_types = vec![
            this.register_static_name::<World>("World"),
            this.register_static_name::<Entity>("Entity"),
            this.register_static_name::<bool>("bool"),
            this.register_static_name::<u8>("u8"),
            this.register_static_name::<u16>("u16"),
            this.register_static_name::<u32>("u32"),
            this.register_static_name::<u64>("u64"),
            this.register_static_name::<u128>("u128"),
            this.register_static_name::<usize>("usize"),
            this.register_static_name::<i8>("i8"),
            this.register_static_name::<i16>("i16"),
            this.register_static_name::<i32>("i32"),
            this.register_static_name::<i64>("i64"),
            this.register_static_name::<i128>("i128"),
            this.register_static_name::<isize>("isize"),
            this.register_static_name::<f32>("f32"),
            this.register_static_name::<f64>("f64"),
            this.register_static_name::<char>("char"),
            this.register_static_name::<String>("String"),
        ];

        *this.storable_types.write() = FxHashSet::from_iter(storable_types);

        this
    }

    /// Gets the unique identifier based on the given name, if it exists.
    fn get_named_type(&self, name: &str) -> Option<Entity> {
        self.named_types.read().get(name).cloned()
    }

    /// Gets the name of the type with the given unique identifier, if it exists.
    fn get_type_name(&self, entity: Entity) -> Option<String> {
        self.type_names.read().get(&entity).map(|x| x.to_owned())
    }

    /// Allocates a new unique identifier for a type, and optionally associates it with the given name.
    fn allocate_type(&self, name: Option<&str>) -> Entity {
        if let Some(name) = name {
            if let Some(entity) = self.named_types.read().get(name).cloned() {
                return entity;
            }
        }
        let entity = self.next_id.fetch_add(1, Ordering::Relaxed);
        let entity = Entity::new_type(Id(entity));
        if let Some(name) = name {
            self.register_type_name(entity, name);
        }
        entity
    }

    /// Registers the given value as the given type.
    fn register_entity_as_type(&self, entity: Entity, entity_type: Entity) {
        self.entity_types.write().insert(entity, entity_type);
    }

    /// Associates the given unique identifier with the given name.
    ///
    /// WARNING: This will overwrite any existing unique identifier for the given name, and vice versa.
    fn register_type_name(&self, typ: Entity, name: &str) {
        self.named_types.write().insert(name.to_owned(), typ);
        self.type_names.write().insert(typ, name.to_owned());
    }

    /// Registers the given type as a static type with the given name.
    fn register_static_name<T: StaticId + ?Sized>(&self, name: &str) -> Entity {
        let id = T::static_id();
        if let Some(entity) = self.static_types.read().get(&id).cloned() {
            return entity;
        }
        if let Some(entity) = self.named_types.read().get(name).cloned() {
            return entity;
        }
        let entity = self.allocate_type(Some(name));
        self.static_types.write().insert(id, entity);
        entity
    }

    fn get_or_register_static_type<T: StaticId + ?Sized>(&self) -> Entity {
        let id = T::static_id();
        if let Some(entity) = self.static_types.read().get(&id).cloned() {
            return entity;
        }
        let entity = self.allocate_type(None);
        self.static_types.write().insert(id, entity);
        entity
    }

    fn current_generation(&self, entity: Id) -> Option<Entity> {
        if let Some(gen) = self.entity_generations.read().get(&entity) {
            return Some(Entity::new_generational(entity, *gen));
        }
        None
    }

    fn allocate_generational_entity(&self) -> Entity {
        if let Some((entity, ())) = self.dead.write().drain().next() {
            // vacancy found, return the entity with the next generation
            let gen = *self.entity_generations.read().get(&entity).unwrap();
            return Entity::new_generational(entity, gen);
        }
        // if no vacancy is found, allocate a new value
        let entity = self.next_id.fetch_add(1, Ordering::Relaxed);
        if entity == u32::MAX {
            // we panic here to prevent weird bugs from happening where the entity id rolls over or equals EntityId::PLACEHOLDER (which is u32::MAX)
            panic!("Entity allocation overflow");
        }
        if entity % 10000 == 0 && entity != 0 {
            log::warn!(
                "Entity allocation: {}/{} ({:.4}%)",
                entity,
                u32::MAX,
                entity as f64 / u32::MAX as f64 * 100.0
            );
        }
        let id = Id(entity);
        let gen = Id(0);
        self.entity_generations.write().insert(id, gen);
        Entity::new_generational(id, gen)
    }

    fn allocate_entity_with_type(&self, value_type: Entity) -> Entity {
        let entity = self.allocate_generational_entity();
        self.register_entity_as_type(entity, value_type);
        entity
    }

    fn delete_entity(&self, entity: Entity) {
        if !self.dead.write().contains(&entity.id()) {
            self.dead.write().insert(entity.id(), ());
            if let Some(gen) = self.entity_generations.write().get_mut(&entity.id()) {
                if gen.0.checked_add(1).is_none() {
                    log::warn!("Generation overflow for value: {}", entity);
                }
                gen.0 = gen.0.wrapping_add(1);
            }
        }
    }

    /// Returns the [`Entity`] of the type that the given [`Entity`] is associated with, if it exists.
    fn get_value_type(&self, entity: Entity) -> Option<Entity> {
        self.entity_types.read().get(&entity).cloned()
    }

    fn allocate(&self) -> Id {
        let entity = self.next_id.fetch_add(1, Ordering::Relaxed);
        Id::new(entity)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// A shared handle to a [`Registry`].
#[derive(Debug, Clone)]
pub struct RegistryHandle(Arc<Registry>);

impl Deref for RegistryHandle {
    type Target = Registry;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Registry> for RegistryHandle {
    fn from(registry: Registry) -> Self {
        Self(Arc::new(registry))
    }
}

static REGISTRY: std::sync::OnceLock<RegistryHandle> = std::sync::OnceLock::new();

pub fn global_registry() -> &'static RegistryHandle {
    REGISTRY.get_or_init(|| RegistryHandle::from(Registry::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        struct Foo;

        let registry = Registry::new();

        let foo = registry.get_or_register_static_type::<Foo>();
        assert_eq!(foo, registry.get_or_register_static_type::<Foo>());
        assert_ne!(foo, registry.get_or_register_static_type::<u32>());
    }

    #[test]
    fn test_registry_handle() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        struct Foo;

        let registry = Registry::new();
        let handle = RegistryHandle::from(registry);

        let foo = handle.get_or_register_static_type::<Foo>();
        assert_eq!(foo, handle.get_or_register_static_type::<Foo>());
        assert_ne!(foo, handle.get_or_register_static_type::<u32>());
    }

    #[test]
    fn test_registry_value_types() {
        let registry = Registry::new();

        let foo = registry.get_or_register_static_type::<u32>();
        let bar = registry.get_or_register_static_type::<u64>();
        let baz = registry.get_or_register_static_type::<u128>();

        let foo_value = registry.allocate_entity_with_type(foo);
        let bar_value = registry.allocate_entity_with_type(bar);
        let baz_value = registry.allocate_entity_with_type(baz);

        assert_eq!(registry.get_value_type(foo_value), Some(foo));
        assert_eq!(registry.get_value_type(bar_value), Some(bar));
        assert_eq!(registry.get_value_type(baz_value), Some(baz));
    }
}

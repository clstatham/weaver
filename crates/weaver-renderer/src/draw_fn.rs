use std::{any::TypeId, fmt::Debug, hash::Hash, ops::Range};

use weaver_app::{App, SubApp};
use weaver_ecs::{
    entity::Entity,
    prelude::Resource,
    query::{QueryFetch, QueryFilter},
    world::World,
};
use weaver_util::{
    impl_downcast,
    lock::{Read, SharedLock, Write},
    DowncastSync, Result, TypeIdMap,
};

use crate::render_phase::GetBatchData;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DrawFnId(u32);

impl DrawFnId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    pub fn id(&self) -> u32 {
        self.0
    }
}

pub trait DrawItem: Send + Sync + 'static {
    type QueryFetch: QueryFetch + 'static;
    type QueryFilter: QueryFilter + 'static;

    fn entity(&self) -> Entity;
    fn draw_fn(&self) -> DrawFnId;
}

pub trait FromDrawItemQuery<T: DrawItem> {
    fn from_draw_item_query(
        query: <T::QueryFetch as QueryFetch>::Item<'_>,
        draw_fn_id: DrawFnId,
    ) -> Self;
}

pub trait BinnedDrawItem: DrawItem + Sized {
    type Key: Debug + Clone + Eq + Ord + Hash + Send + Sync + FromDrawItemQuery<Self>;
    type Instances: GetBatchData + Resource;

    fn new(key: Self::Key, entity: Entity, batch_range: Range<u32>) -> Self;
}

pub trait DrawFn<T: DrawItem>: DowncastSync {
    #[allow(unused_variables)]
    fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        Ok(())
    }

    fn draw<'w>(
        &mut self,
        render_world: &'w World,
        render_pass: &mut wgpu::RenderPass<'w>,
        view_entity: Entity,
        item: T,
    ) -> Result<()>;
}
impl_downcast!(DrawFn<T> where T: DrawItem);

pub struct DrawFunctionsInner<T: DrawItem> {
    functions: Vec<Box<dyn DrawFn<T>>>,
    indices: TypeIdMap<DrawFnId>,
}

impl<T: DrawItem> DrawFunctionsInner<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            indices: TypeIdMap::default(),
        }
    }

    pub fn prepare(&mut self, render_world: &mut World) -> Result<()> {
        for function in self.functions.iter_mut() {
            function.prepare(render_world)?;
        }
        Ok(())
    }

    pub fn add<F: DrawFn<T> + 'static>(&mut self, function: F) -> DrawFnId {
        let id = DrawFnId::new(self.functions.len() as u32);
        self.functions.push(Box::new(function));
        self.indices.insert(TypeId::of::<F>(), id);
        id
    }

    pub fn get(&self, id: DrawFnId) -> Option<&'_ dyn DrawFn<T>> {
        self.functions.get(id.id() as usize).map(|f| f.as_ref())
    }

    pub fn get_mut(&mut self, id: DrawFnId) -> Option<&'_ mut dyn DrawFn<T>> {
        self.functions.get_mut(id.id() as usize).map(|f| f.as_mut())
    }

    pub fn get_id<F: DrawFn<T>>(&self) -> Option<DrawFnId> {
        self.indices.get(&TypeId::of::<F>()).copied()
    }
}

#[derive(Resource)]
pub struct DrawFunctions<T: DrawItem> {
    inner: SharedLock<DrawFunctionsInner<T>>,
}

impl<T: DrawItem> DrawFunctions<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inner: SharedLock::new(DrawFunctionsInner::new()),
        }
    }

    pub fn read(&self) -> Read<'_, DrawFunctionsInner<T>> {
        self.inner.read()
    }

    pub fn write(&self) -> Write<'_, DrawFunctionsInner<T>> {
        self.inner.write()
    }
}

pub trait DrawFnsApp {
    fn add_draw_fn<F: DrawFn<T> + 'static, T: DrawItem>(&mut self, function: F) -> &mut Self;
}

impl DrawFnsApp for SubApp {
    fn add_draw_fn<F: DrawFn<T> + 'static, T: DrawItem>(&mut self, function: F) -> &mut Self {
        if !self.has_resource::<DrawFunctions<T>>() {
            self.insert_resource(DrawFunctions::<T>::new());
        }
        self.get_resource_mut::<DrawFunctions<T>>()
            .unwrap()
            .write()
            .add(function);
        self
    }
}

impl DrawFnsApp for App {
    fn add_draw_fn<F: DrawFn<T> + 'static, T: DrawItem>(&mut self, function: F) -> &mut Self {
        self.main_app_mut().add_draw_fn::<F, T>(function);
        self
    }
}

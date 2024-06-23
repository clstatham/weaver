use std::{any::TypeId, fmt::Debug, hash::Hash, ops::Range};

use weaver_app::{App, SubApp};
use weaver_ecs::{
    entity::Entity,
    prelude::Resource,
    query::{QueryFetch, QueryFilter},
    world::WorldLock,
};
use weaver_util::{
    lock::{ArcRead, ArcWrite, Read, SharedLock, Write},
    prelude::Result,
    TypeIdMap,
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
        query: <T::QueryFetch as QueryFetch>::Item,
        draw_fn_id: DrawFnId,
    ) -> Self;
}

pub trait BinnedDrawItem: DrawItem + Sized {
    type Key: Debug + Clone + Send + Sync + Eq + Ord + Hash + FromDrawItemQuery<Self>;
    type Instances: GetBatchData + Resource;

    fn new(key: Self::Key, entity: Entity, batch_range: Range<u32>) -> Self;
}

pub trait DrawFn<T: DrawItem>: 'static + Send + Sync {
    #[allow(unused_variables)]
    fn prepare(&mut self, render_world: &WorldLock) -> Result<()> {
        Ok(())
    }

    fn draw(
        &mut self,
        render_world: &WorldLock,
        encoder: &mut wgpu::CommandEncoder,
        view_entity: Entity,
        item: &T,
    ) -> Result<()>;
}

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

    pub fn prepare(&mut self, render_world: &WorldLock) -> Result<()> {
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

    pub fn get(&self, id: DrawFnId) -> Option<&dyn DrawFn<T>> {
        self.functions.get(id.id() as usize).map(|f| f.as_ref())
    }

    pub fn get_mut(&mut self, id: DrawFnId) -> Option<&mut dyn DrawFn<T>> {
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

    pub fn read_arc(&self) -> ArcRead<DrawFunctionsInner<T>> {
        self.inner.read_arc()
    }

    pub fn write_arc(&self) -> ArcWrite<DrawFunctionsInner<T>> {
        self.inner.write_arc()
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

use std::any::TypeId;

use weaver_app::{plugin::Plugin, App, SubApp};
use weaver_ecs::{
    entity::Entity,
    prelude::Resource,
    query::{QueryFetch, QueryFilter},
    system::SystemParam,
    world::World,
};
use weaver_util::{
    lock::{Lock, Read, Write},
    prelude::Result,
    TypeIdMap,
};

use crate::pipeline::CreateRenderPipeline;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    type Param: SystemParam;
    type ViewQueryFetch: QueryFetch;
    type ViewQueryFilter: QueryFilter;
    type ItemQueryFetch: QueryFetch;
    type ItemQueryFilter: QueryFilter;

    fn entity(&self) -> Entity;
    fn draw_fn(&self) -> DrawFnId;
}

pub trait RenderPipelinedDrawItem: CreateRenderPipeline + DrawItem {}
impl<T: CreateRenderPipeline + DrawItem> RenderPipelinedDrawItem for T {}

pub trait DrawFn<T: DrawItem>: 'static + Send + Sync {
    #[allow(unused_variables)]
    fn prepare(&mut self, render_world: &World) -> Result<()> {
        Ok(())
    }

    fn draw<'w>(
        &'w mut self,
        render_world: &World,
        render_pass: &mut wgpu::RenderPass<'w>,
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
    inner: Lock<DrawFunctionsInner<T>>,
}

impl<T: DrawItem> DrawFunctions<T> {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            inner: Lock::new(DrawFunctionsInner::new()),
        }
    }

    pub fn read(&self) -> Read<'_, DrawFunctionsInner<T>> {
        self.inner.read()
    }

    pub fn write(&self) -> Write<'_, DrawFunctionsInner<T>> {
        self.inner.write()
    }
}

pub struct DrawFunctionsPlugin<T: DrawItem>(std::marker::PhantomData<T>);
impl<T: DrawItem> Default for DrawFunctionsPlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: DrawItem> Plugin for DrawFunctionsPlugin<T> {
    fn build(&self, app: &mut App) -> Result<()> {
        app.main_app()
            .world()
            .insert_resource(DrawFunctions::<T>::new());

        Ok(())
    }
}

pub trait AddDrawFn {
    fn add_draw_fn<F: DrawFn<T> + 'static, T: DrawItem>(&mut self, function: F) -> &mut Self;
}

impl AddDrawFn for SubApp {
    fn add_draw_fn<F: DrawFn<T> + 'static, T: DrawItem>(&mut self, function: F) -> &mut Self {
        if !self.world().has_resource::<DrawFunctions<T>>() {
            self.world().insert_resource(DrawFunctions::<T>::new());
        }
        self.world()
            .get_resource_mut::<DrawFunctions<T>>()
            .unwrap()
            .write()
            .add(function);
        self
    }
}

impl AddDrawFn for App {
    fn add_draw_fn<F: DrawFn<T> + 'static, T: DrawItem>(&mut self, function: F) -> &mut Self {
        self.main_app_mut().add_draw_fn::<F, T>(function);
        self
    }
}

use std::{any::TypeId, ops::Deref, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{prelude::Resource, world::ReadWorld};
use weaver_util::{
    prelude::{DowncastSync, Result},
    TypeIdMap,
};

use crate::{bind_group::BindGroupLayoutCache, Extract, WgpuDevice};

#[derive(Resource, Default)]
pub struct RenderPipelineCache {
    layout_cache: TypeIdMap<RenderPipelineLayout>,
    pipeline_cache: TypeIdMap<RenderPipeline>,
}

impl RenderPipelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_layout<T>(&self) -> Option<&RenderPipelineLayout>
    where
        T: CreateRenderPipeline,
    {
        self.layout_cache.get(&TypeId::of::<T>())
    }

    pub fn get_pipeline<T>(&self) -> Option<&RenderPipeline>
    where
        T: CreateRenderPipeline,
    {
        self.pipeline_cache.get(&TypeId::of::<T>())
    }

    pub fn get_or_create_layout<T>(
        &mut self,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        T: CreateRenderPipeline,
    {
        if let Some(cached_layout) = self.layout_cache.get(&TypeId::of::<T>()) {
            cached_layout.clone()
        } else {
            let layout = T::create_render_pipeline_layout(device, bind_group_layout_cache);
            self.layout_cache.insert(TypeId::of::<T>(), layout.clone());
            self.layout_cache.get(&TypeId::of::<T>()).unwrap().clone()
        }
    }

    pub fn get_or_create_pipeline<T>(
        &mut self,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipeline
    where
        T: CreateRenderPipeline,
    {
        if let Some(cached_pipeline) = self.pipeline_cache.get(&TypeId::of::<T>()) {
            cached_pipeline.clone()
        } else {
            let layout = self.get_or_create_layout::<T>(device, bind_group_layout_cache);
            let pipeline = T::create_render_pipeline(device, &layout);
            self.pipeline_cache.insert(TypeId::of::<T>(), pipeline);
            self.pipeline_cache.get(&TypeId::of::<T>()).unwrap().clone()
        }
    }
}

#[derive(Clone)]
pub struct RenderPipelineLayout {
    layout: Arc<wgpu::PipelineLayout>,
}

impl RenderPipelineLayout {
    pub fn new(layout: wgpu::PipelineLayout) -> Self {
        Self {
            layout: Arc::new(layout),
        }
    }

    pub fn get_or_create<T>(
        device: &wgpu::Device,
        pipeline_cache: &mut RenderPipelineCache,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> Self
    where
        T: CreateRenderPipeline,
    {
        pipeline_cache.get_or_create_layout::<T>(device, bind_group_layout_cache)
    }
}

impl Deref for RenderPipelineLayout {
    type Target = wgpu::PipelineLayout;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

#[derive(Clone)]
pub struct RenderPipeline {
    pipeline: Arc<wgpu::RenderPipeline>,
}

impl RenderPipeline {
    pub fn new(pipeline: wgpu::RenderPipeline) -> Self {
        Self {
            pipeline: Arc::new(pipeline),
        }
    }

    pub fn get_or_create<T>(
        device: &wgpu::Device,
        pipeline_cache: &mut RenderPipelineCache,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> Self
    where
        T: CreateRenderPipeline,
    {
        pipeline_cache.get_or_create_pipeline::<T>(device, bind_group_layout_cache)
    }
}

impl Deref for RenderPipeline {
    type Target = wgpu::RenderPipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

pub trait CreateRenderPipeline: DowncastSync {
    fn create_render_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> RenderPipelineLayout
    where
        Self: Sized;
    fn create_render_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> RenderPipeline
    where
        Self: Sized;
}

pub struct RenderPipelinePlugin<T: CreateRenderPipeline>(std::marker::PhantomData<T>);

impl<T: CreateRenderPipeline> Default for RenderPipelinePlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateRenderPipeline> Plugin for RenderPipelinePlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_system(extract_render_pipeline::<T>, Extract);
        Ok(())
    }
}

fn extract_render_pipeline<T: CreateRenderPipeline>(render_world: ReadWorld) -> Result<()> {
    let device = render_world.get_resource::<WgpuDevice>().unwrap();
    let mut pipeline_cache = render_world
        .get_resource_mut::<RenderPipelineCache>()
        .unwrap();
    let mut bind_group_layout_cache = render_world
        .get_resource_mut::<BindGroupLayoutCache>()
        .unwrap();

    pipeline_cache.get_or_create_pipeline::<T>(&device, &mut bind_group_layout_cache);

    Ok(())
}

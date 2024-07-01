use std::{any::TypeId, collections::HashMap, ops::Deref, sync::Arc};

use weaver_app::{plugin::Plugin, App};
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::Resource,
};
use weaver_util::{
    define_atomic_id,
    prelude::{DowncastSync, Result},
    TypeIdMap,
};

use crate::{bind_group::BindGroupLayoutCache, ExtractPipelineStage, WgpuDevice};

define_atomic_id!(PipelineId);

#[derive(Resource, Default)]
pub struct RenderPipelineCache {
    layout_cache: HashMap<PipelineId, RenderPipelineLayout>,
    pipeline_cache: HashMap<PipelineId, RenderPipeline>,
    ids: TypeIdMap<PipelineId>,
}

impl RenderPipelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_id_for<T>(&self) -> Option<PipelineId>
    where
        T: CreateRenderPipeline,
    {
        self.ids.get(&TypeId::of::<T>()).copied()
    }

    pub fn get_layout(&self, id: PipelineId) -> Option<&RenderPipelineLayout> {
        self.layout_cache.get(&id)
    }

    pub fn get_pipeline(&self, id: PipelineId) -> Option<&RenderPipeline> {
        self.pipeline_cache.get(&id)
    }

    pub fn get_layout_for<T>(&self) -> Option<&RenderPipelineLayout>
    where
        T: CreateRenderPipeline,
    {
        self.ids
            .get(&TypeId::of::<T>())
            .and_then(|id| self.layout_cache.get(id))
    }

    pub fn get_pipeline_for<T>(&self) -> Option<&RenderPipeline>
    where
        T: CreateRenderPipeline,
    {
        self.ids
            .get(&TypeId::of::<T>())
            .and_then(|id| self.pipeline_cache.get(id))
    }

    pub fn create_for<T>(
        &mut self,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> Result<PipelineId>
    where
        T: CreateRenderPipeline,
    {
        if let Some(id) = self.ids.get(&TypeId::of::<T>()) {
            return Ok(*id);
        }

        let id = PipelineId::new();

        let layout = T::create_render_pipeline_layout(device, bind_group_layout_cache);
        let pipeline = T::create_render_pipeline(device, &layout);
        self.layout_cache.insert(id, layout);
        self.pipeline_cache.insert(id, pipeline);
        self.ids.insert(TypeId::of::<T>(), id);

        Ok(id)
    }

    pub fn insert(&mut self, layout: RenderPipelineLayout, pipeline: RenderPipeline) -> PipelineId {
        let id = PipelineId::new();
        self.layout_cache.insert(id, layout);
        self.pipeline_cache.insert(id, pipeline);
        id
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

    pub fn get_mut(&mut self) -> &mut wgpu::RenderPipeline {
        Arc::get_mut(&mut self.pipeline).unwrap()
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
        render_app.add_system(extract_render_pipeline::<T>, ExtractPipelineStage);
        Ok(())
    }
}

fn extract_render_pipeline<T: CreateRenderPipeline>(
    device: Res<WgpuDevice>,
    mut pipeline_cache: ResMut<RenderPipelineCache>,
    mut bind_group_layout_cache: ResMut<BindGroupLayoutCache>,
) -> Result<()> {
    pipeline_cache.create_for::<T>(&device, &mut bind_group_layout_cache)?;

    Ok(())
}

#[derive(Resource, Default)]
pub struct ComputePipelineCache {
    layout_cache: TypeIdMap<ComputePipelineLayout>,
    pipeline_cache: TypeIdMap<ComputePipeline>,
}

impl ComputePipelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_layout<T>(&self) -> Option<&ComputePipelineLayout>
    where
        T: CreateComputePipeline,
    {
        self.layout_cache.get(&TypeId::of::<T>())
    }

    pub fn get_pipeline<T>(&self) -> Option<&ComputePipeline>
    where
        T: CreateComputePipeline,
    {
        self.pipeline_cache.get(&TypeId::of::<T>())
    }

    pub fn get_or_create_layout<T>(
        &mut self,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> ComputePipelineLayout
    where
        T: CreateComputePipeline,
    {
        if let Some(cached_layout) = self.layout_cache.get(&TypeId::of::<T>()) {
            cached_layout.clone()
        } else {
            let layout = T::create_compute_pipeline_layout(device, bind_group_layout_cache);
            self.layout_cache.insert(TypeId::of::<T>(), layout.clone());
            self.layout_cache.get(&TypeId::of::<T>()).unwrap().clone()
        }
    }

    pub fn get_or_create_pipeline<T>(
        &mut self,
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> ComputePipeline
    where
        T: CreateComputePipeline,
    {
        if let Some(cached_pipeline) = self.pipeline_cache.get(&TypeId::of::<T>()) {
            cached_pipeline.clone()
        } else {
            let layout = self.get_or_create_layout::<T>(device, bind_group_layout_cache);
            let pipeline = T::create_compute_pipeline(device, &layout);
            self.pipeline_cache.insert(TypeId::of::<T>(), pipeline);
            self.pipeline_cache.get(&TypeId::of::<T>()).unwrap().clone()
        }
    }
}

#[derive(Clone)]
pub struct ComputePipelineLayout {
    layout: Arc<wgpu::PipelineLayout>,
}

impl ComputePipelineLayout {
    pub fn new(layout: wgpu::PipelineLayout) -> Self {
        Self {
            layout: Arc::new(layout),
        }
    }

    pub fn get_or_create<T>(
        device: &wgpu::Device,
        pipeline_cache: &mut ComputePipelineCache,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> Self
    where
        T: CreateComputePipeline,
    {
        pipeline_cache.get_or_create_layout::<T>(device, bind_group_layout_cache)
    }
}

impl Deref for ComputePipelineLayout {
    type Target = wgpu::PipelineLayout;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

#[derive(Clone)]
pub struct ComputePipeline {
    pipeline: Arc<wgpu::ComputePipeline>,
}

impl ComputePipeline {
    pub fn new(pipeline: wgpu::ComputePipeline) -> Self {
        Self {
            pipeline: Arc::new(pipeline),
        }
    }

    pub fn get_or_create<T>(
        device: &wgpu::Device,
        pipeline_cache: &mut ComputePipelineCache,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> Self
    where
        T: CreateComputePipeline,
    {
        pipeline_cache.get_or_create_pipeline::<T>(device, bind_group_layout_cache)
    }
}

impl Deref for ComputePipeline {
    type Target = wgpu::ComputePipeline;

    fn deref(&self) -> &Self::Target {
        &self.pipeline
    }
}

pub trait CreateComputePipeline: DowncastSync {
    fn create_compute_pipeline_layout(
        device: &wgpu::Device,
        bind_group_layout_cache: &mut BindGroupLayoutCache,
    ) -> ComputePipelineLayout
    where
        Self: Sized;
    fn create_compute_pipeline(
        device: &wgpu::Device,
        cached_layout: &wgpu::PipelineLayout,
    ) -> ComputePipeline
    where
        Self: Sized;
}

pub struct ComputePipelinePlugin<T: CreateComputePipeline>(std::marker::PhantomData<T>);
impl<T: CreateComputePipeline> Default for ComputePipelinePlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: CreateComputePipeline> Plugin for ComputePipelinePlugin<T> {
    fn build(&self, render_app: &mut App) -> Result<()> {
        if !render_app.has_resource::<ComputePipelineCache>() {
            render_app.insert_resource(ComputePipelineCache::new());
        }
        render_app.add_system(extract_compute_pipeline::<T>, ExtractPipelineStage);
        Ok(())
    }
}

fn extract_compute_pipeline<T: CreateComputePipeline>(
    device: Res<WgpuDevice>,
    mut pipeline_cache: ResMut<ComputePipelineCache>,
    mut bind_group_layout_cache: ResMut<BindGroupLayoutCache>,
) -> Result<()> {
    pipeline_cache.get_or_create_pipeline::<T>(&device, &mut bind_group_layout_cache);

    Ok(())
}

use std::{
    collections::HashMap,
    ops::{Deref, DerefMut, Range},
};

use weaver_app::plugin::Plugin;
use weaver_ecs::{
    entity::Entity,
    prelude::Resource,
    query::QueryFetch,
    system::SystemParam,
    world::{World, WorldLock},
};
use weaver_util::{lock::Write, prelude::Result};

use crate::{
    bind_group::{BindGroupLayout, CreateBindGroup, ResourceBindGroupPlugin},
    buffer::{GpuArrayBuffer, GpuArrayBufferIndex, GpuArrayBufferable},
    draw_fn::{BinnedDrawItem, DrawFunctionsInner, FromDrawItemQuery},
    extract::{RenderResource, RenderResourcePlugin},
    prelude::DrawFunctions,
    render_command::{RenderCommand, RenderCommandState},
    WgpuDevice, WgpuQueue,
};

pub struct BinnedRenderPhase<T: BinnedDrawItem> {
    pub batch_keys: Vec<T::Key>,
    pub batch_values: HashMap<T::Key, Vec<Entity>>,
    pub batch_sets: Vec<Vec<BinnedBatch>>,
}

pub struct BinnedBatch {
    pub representative_entity: Entity,
    pub batch_range: Range<u32>,
}

impl<T: BinnedDrawItem> Default for BinnedRenderPhase<T> {
    fn default() -> Self {
        Self {
            batch_keys: Vec::new(),
            batch_values: HashMap::new(),
            batch_sets: Vec::new(),
        }
    }
}

impl<T: BinnedDrawItem> BinnedRenderPhase<T> {
    pub fn add(&mut self, key: T::Key, entity: Entity) {
        if let Some(batch) = self.batch_values.get_mut(&key) {
            batch.push(entity);
        } else {
            self.batch_keys.push(key.clone());
            self.batch_values.insert(key, vec![entity]);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.batch_keys.is_empty()
    }

    pub fn clear(&mut self) {
        self.batch_keys.clear();
        self.batch_values.clear();
        self.batch_sets.clear();
    }

    pub fn render<'w>(
        &self,
        render_world: &'w WorldLock,
        encoder: &mut wgpu::CommandEncoder,
        view_entity: Entity,
        draw_functions: &mut Write<'w, DrawFunctionsInner<T>>,
    ) -> Result<()> {
        debug_assert_eq!(self.batch_keys.len(), self.batch_sets.len());
        for (key, batch_set) in self.batch_keys.iter().zip(&self.batch_sets) {
            for batch in batch_set {
                let item = T::new(
                    key.clone(),
                    batch.representative_entity,
                    batch.batch_range.clone(),
                );
                if let Some(draw_fn) = draw_functions.get_mut(item.draw_fn()) {
                    draw_fn.draw(render_world, encoder, view_entity, &item)?;
                } else {
                    log::warn!("Draw function not found for key: {:?}", key);
                }
            }
        }

        Ok(())
    }
}

#[derive(Resource)]
pub struct BinnedRenderPhases<T: BinnedDrawItem> {
    pub phases: HashMap<Entity, BinnedRenderPhase<T>>,
}

impl<T: BinnedDrawItem> BinnedRenderPhases<T> {
    pub fn insert_or_clear(&mut self, entity: Entity) {
        if let Some(phase) = self.phases.get_mut(&entity) {
            phase.clear();
        } else {
            self.phases.insert(entity, BinnedRenderPhase::default());
        }
    }
}

impl<T: BinnedDrawItem> Default for BinnedRenderPhases<T> {
    fn default() -> Self {
        Self {
            phases: HashMap::new(),
        }
    }
}

impl<T: BinnedDrawItem> Deref for BinnedRenderPhases<T> {
    type Target = HashMap<Entity, BinnedRenderPhase<T>>;

    fn deref(&self) -> &Self::Target {
        &self.phases
    }
}

impl<T: BinnedDrawItem> DerefMut for BinnedRenderPhases<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.phases
    }
}

pub struct BinnedRenderPhasePlugin<T: BinnedDrawItem>(std::marker::PhantomData<T>);
impl<T: BinnedDrawItem> Default for BinnedRenderPhasePlugin<T> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<T: BinnedDrawItem> Plugin for BinnedRenderPhasePlugin<T> {
    fn build(&self, app: &mut weaver_app::App) -> Result<()> {
        app.insert_resource(BinnedRenderPhases::<T>::default());
        Ok(())
    }
}

pub trait GetBatchData: Send + Sync + 'static {
    type Param: SystemParam + 'static;
    type BufferData: GpuArrayBufferable + Send + Sync + 'static;
    type UpdateQuery: QueryFetch + 'static;

    fn update_from_world(&mut self, render_world: &World);

    fn get_batch_data(
        param: &<Self::Param as SystemParam>::Fetch<'_, '_>,
        query_item: Entity,
    ) -> Option<Self::BufferData>;
}

pub type BinnedInstances<I> = <I as BinnedDrawItem>::Instances;

pub type BinnedBatchData<I> = <BinnedInstances<I> as GetBatchData>::BufferData;

pub type BinnedBatchDataParam<'w, 's, I> =
    <<BinnedInstances<I> as GetBatchData>::Param as SystemParam>::Fetch<'w, 's>;

#[derive(Resource)]
pub struct BatchedInstanceBuffer<I: BinnedDrawItem, C: RenderCommand<I>> {
    pub buffer: GpuArrayBuffer<BinnedBatchData<I>>,
    _phantom: std::marker::PhantomData<C>,
}

impl<I: BinnedDrawItem, C: RenderCommand<I>> Default for BatchedInstanceBuffer<I, C> {
    fn default() -> Self {
        Self {
            buffer: GpuArrayBuffer::default(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I: BinnedDrawItem, C: RenderCommand<I>> BatchedInstanceBuffer<I, C> {
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn push(
        &mut self,
        param: &BinnedBatchDataParam<I>,
        query_item: Entity,
    ) -> Option<GpuArrayBufferIndex<BinnedBatchData<I>>> {
        let data = BinnedInstances::<I>::get_batch_data(param, query_item)?;
        let index = self.buffer.push(data);
        Some(index)
    }

    pub fn enqueue_update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.buffer.enqueue_update(device, queue);
    }
}

impl<I: BinnedDrawItem, C: RenderCommand<I>> CreateBindGroup for BatchedInstanceBuffer<I, C> {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Batched Instace Buffer Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: GpuArrayBuffer::<BinnedBatchData<I>>::binding_type(),
                count: None,
            }],
        })
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: cached_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.binding().unwrap(),
            }],
            label: Some("Batched Instace Buffer Bind Group"),
        })
    }
}

impl<I: BinnedDrawItem, C: RenderCommand<I>> RenderResource for BatchedInstanceBuffer<I, C> {
    type UpdateQuery = <<I as BinnedDrawItem>::Instances as GetBatchData>::UpdateQuery;

    fn extract_render_resource(_main_world: &mut World, render_world: &mut World) -> Option<Self>
    where
        Self: Sized,
    {
        let mut this = Self::default();
        let device = render_world.get_resource::<WgpuDevice>()?;

        this.buffer.reserve(1, &device);

        Some(this)
    }

    fn update_render_resource(
        &mut self,
        _main_world: &mut World,
        render_world: &mut World,
    ) -> Result<()> {
        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let queue = render_world.get_resource::<WgpuQueue>().unwrap();
        self.enqueue_update(&device, &queue);
        Ok(())
    }
}

pub struct BatchedInstanceBufferPlugin<I: BinnedDrawItem, C: RenderCommand<I>>(
    std::marker::PhantomData<(I, C)>,
);
impl<I: BinnedDrawItem, C: RenderCommand<I>> Default for BatchedInstanceBufferPlugin<I, C> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<I: BinnedDrawItem, C: RenderCommand<I>> Plugin for BatchedInstanceBufferPlugin<I, C> {
    fn build(&self, app: &mut weaver_app::App) -> Result<()> {
        app.add_plugin(RenderResourcePlugin::<BatchedInstanceBuffer<I, C>>::default())?;
        app.add_plugin(ResourceBindGroupPlugin::<BatchedInstanceBuffer<I, C>>::default())?;
        Ok(())
    }
}

pub fn batch_and_prepare<I: BinnedDrawItem, C: RenderCommand<I>>(
    render_world: WorldLock,
) -> Result<()> {
    let mut binned_phases = render_world
        .read()
        .get_resource_mut::<BinnedRenderPhases<I>>()
        .unwrap();

    let draw_fns = render_world
        .read()
        .get_resource::<DrawFunctions<I>>()
        .unwrap();

    let draw_fn_id = draw_fns
        .read()
        .get_id::<RenderCommandState<I, C>>()
        .unwrap();

    for phase in binned_phases.values_mut() {
        phase.clear();
    }

    let pbrs_query = render_world.query::<I::QueryFetch>();

    for (entity, query_fetch) in pbrs_query.iter() {
        let key = <I::Key as FromDrawItemQuery<I>>::from_draw_item_query(query_fetch, draw_fn_id);

        for phase in binned_phases.values_mut() {
            phase.add(key.clone(), entity);
        }
    }

    let mut pbr_mesh_instances = render_world
        .read()
        .get_resource_mut::<I::Instances>()
        .unwrap();
    pbr_mesh_instances.update_from_world(&render_world.read());
    drop(pbr_mesh_instances);

    let mut batched_instance_buffer = render_world
        .read()
        .get_resource_mut::<BatchedInstanceBuffer<I, C>>()
        .unwrap();

    batched_instance_buffer.clear();

    let mut state = <I::Instances as GetBatchData>::Param::init_state(&render_world);
    let param = <I::Instances as GetBatchData>::Param::fetch(&mut state, &render_world);

    for phase in binned_phases.values_mut() {
        for key in &phase.batch_keys {
            let mut batch_set = Vec::new();
            for entity in &phase.batch_values[key] {
                let instance = batched_instance_buffer.push(&param, *entity).unwrap();

                if !batch_set
                    .last()
                    .is_some_and(|batch: &BinnedBatch| batch.batch_range.end == instance.index())
                {
                    batch_set.push(BinnedBatch {
                        representative_entity: *entity,
                        batch_range: instance.index()..instance.index(),
                    });
                }

                if let Some(batch) = batch_set.last_mut() {
                    batch.batch_range.end = instance.index() + 1;
                }
            }
            phase.batch_sets.push(batch_set);
        }
    }

    Ok(())
}

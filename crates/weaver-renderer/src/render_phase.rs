use std::ops::{Deref, DerefMut, Range};

use weaver_app::plugin::Plugin;
use weaver_ecs::{
    component::{Res, ResMut},
    entity::{Entity, EntityMap},
    prelude::{Resource, WorldView},
    query::{Query, QueryFetch, QueryFilter},
    world::{FromWorld, World},
};
use weaver_util::{
    lock::Write,
    {FxHashMap, Result},
};

use crate::{
    bind_group::{BindGroupLayout, CreateBindGroup, ResourceBindGroupPlugin},
    buffer::{GpuArrayBuffer, GpuArrayBufferIndex, GpuArrayBufferable},
    draw_fn::{BinnedDrawItem, DrawFunctionsInner, FromDrawItemQuery},
    prelude::DrawFunctions,
    render_command::{RenderCommand, RenderCommandState},
    WgpuDevice, WgpuQueue,
};

pub struct BinnedRenderPhase<T: BinnedDrawItem> {
    pub batch_keys: Vec<T::Key>,
    pub batch_values: FxHashMap<T::Key, Vec<Entity>>,
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
            batch_values: FxHashMap::default(),
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
        &'w self,
        render_world: &'w World,
        render_pass: &mut wgpu::RenderPass<'w>,
        view_entity: Entity,
        draw_functions: &mut Write<DrawFunctionsInner<T>>,
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
                    log::trace!(
                        "Running DrawFn {:?} for range {:?} for key: {:?}",
                        item.draw_fn(),
                        &batch.batch_range,
                        key
                    );
                    draw_fn.draw(render_world, render_pass, view_entity, item)?;
                } else {
                    log::debug!("Draw function not found for key: {:?}", key);
                }
            }
        }

        Ok(())
    }
}

#[derive(Resource)]
pub struct BinnedRenderPhases<T: BinnedDrawItem> {
    pub phases: EntityMap<BinnedRenderPhase<T>>,
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
            phases: EntityMap::default(),
        }
    }
}

impl<T: BinnedDrawItem> Deref for BinnedRenderPhases<T> {
    type Target = EntityMap<BinnedRenderPhase<T>>;

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

pub trait GetBatchData: 'static {
    type BufferData: GpuArrayBufferable + Send + Sync + 'static;
    type UpdateQueryFetch: QueryFetch + 'static;
    type UpdateQueryFilter: QueryFilter + 'static;

    fn update(&mut self, query: WorldView<Self::UpdateQueryFetch, Self::UpdateQueryFilter>);

    fn get_batch_data(&self, query_item: Entity) -> Option<Self::BufferData>;
}

pub type BinnedInstances<I> = <I as BinnedDrawItem>::Instances;

pub type BinnedBatchData<I> = <BinnedInstances<I> as GetBatchData>::BufferData;

#[derive(Resource)]
pub struct BatchedInstanceBuffer<I: BinnedDrawItem, C: RenderCommand<I>> {
    pub buffer: GpuArrayBuffer<BinnedBatchData<I>>,
    _phantom: std::marker::PhantomData<C>,
    bind_group_stale: bool,
}

impl<I: BinnedDrawItem, C: RenderCommand<I>> BatchedInstanceBuffer<I, C> {
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    pub fn push(
        &mut self,
        instances: &BinnedInstances<I>,
        query_item: Entity,
    ) -> Option<GpuArrayBufferIndex<BinnedBatchData<I>>> {
        let data = instances.get_batch_data(query_item)?;
        let index = self.buffer.push(data);
        Some(index)
    }

    pub fn enqueue_update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.bind_group_stale = self.buffer.enqueue_update(device, queue);
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

    fn bind_group_stale(&self) -> bool {
        self.bind_group_stale
    }

    fn set_bind_group_stale(&mut self, stale: bool) {
        self.bind_group_stale = stale;
    }
}

impl<I: BinnedDrawItem, C: RenderCommand<I>> FromWorld for BatchedInstanceBuffer<I, C> {
    fn from_world(world: &mut World) -> Self {
        let mut buffer = GpuArrayBuffer::new();
        let device = world.get_resource::<WgpuDevice>().unwrap();
        buffer.reserve(1, &device);
        Self {
            buffer,
            _phantom: std::marker::PhantomData,
            bind_group_stale: true,
        }
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
        app.add_plugin(ResourceBindGroupPlugin::<BatchedInstanceBuffer<I, C>>::default())?;
        Ok(())
    }

    fn finish(&self, app: &mut weaver_app::App) -> Result<()> {
        app.init_resource::<BatchedInstanceBuffer<I, C>>();
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
pub fn batch_and_prepare<I: BinnedDrawItem, C: RenderCommand<I>>(
    mut binned_phases: ResMut<BinnedRenderPhases<I>>,
    draw_fns: Res<DrawFunctions<I>>,
    mut batched_instance_buffer: ResMut<BatchedInstanceBuffer<I, C>>,
    mut instances: ResMut<I::Instances>,
    item_query: WorldView<I::QueryFetch, I::QueryFilter>,
    instance_update_query: WorldView<
        <I::Instances as GetBatchData>::UpdateQueryFetch,
        <I::Instances as GetBatchData>::UpdateQueryFilter,
    >,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    let draw_fn_id = draw_fns
        .read()
        .get_id::<RenderCommandState<I, C>>()
        .unwrap();

    for phase in binned_phases.values_mut() {
        phase.clear();
    }

    for (entity, query_fetch) in item_query.iter() {
        let key = <I::Key as FromDrawItemQuery<I>>::from_draw_item_query(query_fetch, draw_fn_id);

        for phase in binned_phases.values_mut() {
            phase.add(key.clone(), entity);
        }
    }

    instances.update(instance_update_query);

    batched_instance_buffer.clear();

    for phase in binned_phases.values_mut() {
        for key in &phase.batch_keys {
            let mut batch_set = Vec::new();
            for entity in &phase.batch_values[key] {
                let instance = batched_instance_buffer.push(&instances, *entity).unwrap();

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

    batched_instance_buffer.enqueue_update(&device, &queue);
    device.poll(wgpu::Maintain::Wait);
}

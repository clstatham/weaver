//! Instanced drawing of opaque meshes.
//!
//! After a view's [`Opaque3d`] phase is sorted, [`batch_opaque_meshes`] writes the model matrix of
//! every queued mesh into the view's [`ViewMeshInstances`] buffer, in draw order, and merges
//! neighboring items with the same [`Opaque3dKey`](crate::phase::Opaque3dKey) (same mesh, same
//! material, same draw function) into one item whose `batch_range` covers all of them. A draw
//! function then issues one instanced draw per item, and the shader reads its model matrix with
//! `instances[instance_index]`.

use weaver_ecs::{
    commands::Commands,
    component::Res,
    entity::Entity,
    query::{Query, With},
};
use weaver_util::prelude::*;

use crate::{
    WgpuDevice, WgpuQueue,
    bind_group::{AsBindGroup, BindGroupLayoutBuilder, BindingsBuilder},
    buffer::GpuBufferVec,
    phase::{Opaque3d, RenderPhase},
    transform::GpuTransform,
    view::ExtractedView,
};

/// A view's mesh instance data: model matrices in the order its opaque meshes are drawn.
pub struct ViewMeshInstances {
    transforms: GpuBufferVec<GpuTransform>,
}

impl Default for ViewMeshInstances {
    fn default() -> Self {
        Self {
            transforms: GpuBufferVec::new(
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            ),
        }
    }
}

impl AsBindGroup for ViewMeshInstances {
    const LABEL: &'static str = "View Mesh Instances";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        layout.storage_buffer(wgpu::ShaderStages::VERTEX, true);
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings.buffer_vec(&self.transforms);
    }
}

/// Merges sorted items into instanced batches; returns the batched items and, for each instance in
/// draw order, the entity whose transform it uses.
pub fn batch_items(items: Vec<Opaque3d>) -> (Vec<Opaque3d>, Vec<Entity>) {
    let mut batched: Vec<Opaque3d> = Vec::with_capacity(items.len());
    let mut instances = Vec::with_capacity(items.len());
    for mut item in items {
        let index = instances.len() as u32;
        instances.push(item.entity);
        match batched.last_mut() {
            Some(last) if last.key == item.key => last.batch_range.end = index + 1,
            _ => {
                item.batch_range = index..index + 1;
                batched.push(item);
            }
        }
    }
    (batched, instances)
}

/// Batches each view's sorted [`Opaque3d`] items and uploads their transforms.
#[allow(clippy::type_complexity)]
pub fn batch_opaque_meshes(
    commands: Commands,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
    mut views: Query<(
        Entity,
        &mut RenderPhase<Opaque3d>,
        Option<&mut ViewMeshInstances>,
        With<ExtractedView>,
    )>,
    mut transforms: Query<&GpuTransform>,
) {
    for (view, mut phase, instances, _) in views.iter() {
        // items whose entity lost its transform since queueing can't be drawn
        let items = std::mem::take(&mut phase.items)
            .into_iter()
            .filter(|item| transforms.get(item.entity).is_some())
            .collect();
        let (batched, entities) = batch_items(items);
        log::trace!(
            "view {view:?}: {} opaque instances in {} draws",
            entities.len(),
            batched.len()
        );
        phase.items = batched;

        let mut upload = |instances: &mut ViewMeshInstances| {
            instances.transforms.clear();
            for &entity in &entities {
                instances.transforms.push(*transforms.get(entity).unwrap());
            }
            if instances.transforms.is_empty() {
                // a storage buffer can't be bound empty; nothing reads this one
                instances.transforms.push(GpuTransform::IDENTITY);
            }
            instances.transforms.enqueue_update(&device, &queue);
        };
        match instances {
            Some(mut instances) => upload(&mut instances),
            None => {
                let mut instances = ViewMeshInstances::default();
                upload(&mut instances);
                commands.insert_component(view, instances);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use weaver_asset::AssetId;

    use super::*;
    use crate::phase::{DrawFunctionId, Opaque3dKey};

    fn key(material: u64, mesh: u64) -> Opaque3dKey {
        Opaque3dKey {
            draw_function: DrawFunctionId::for_tests(0),
            material: AssetId::from_u64(material),
            mesh: AssetId::from_u64(mesh),
        }
    }

    fn item(id: u32, key: Opaque3dKey) -> Opaque3d {
        Opaque3d::new(Entity::from_u64(((1u64) << 32) | id as u64), key)
    }

    #[test]
    fn neighbors_with_equal_keys_merge_into_one_batch() {
        let (a, b) = (key(1, 1), key(1, 2));
        let items = vec![item(0, a), item(1, a), item(2, a), item(3, b), item(4, a)];

        let (batched, instances) = batch_items(items);

        let ranges: Vec<_> = batched
            .iter()
            .map(|item| (item.key, item.batch_range.clone()))
            .collect();
        assert_eq!(ranges, vec![(a, 0..3), (b, 3..4), (a, 4..5)]);
        assert_eq!(instances.len(), 5);
        assert_eq!(batched[0].entity, instances[0]);
    }
}

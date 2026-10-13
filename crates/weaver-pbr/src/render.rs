use weaver_asset::Handle;
use weaver_core::mesh::Mesh;
use weaver_ecs::{
    entity::Entity,
    prelude::{Res, ResMut, World},
    query::Query,
    system::SystemParamItem,
    world::ConstructFromWorld,
};
use weaver_renderer::{
    asset::RenderAssets,
    bind_group::{AsBindGroup, BindGroupLayoutBuilder, BindGroupLayouts, BindingsBuilder},
    camera::CameraBindGroup,
    mesh::{GpuMesh, instances::ViewMeshInstances, vertex_buffer_layout},
    prelude::*,
    transform::GpuTransform,
};

use weaver_util::prelude::*;

use crate::{
    light::GpuPointLightArray,
    prelude::{GpuMaterial, Material, MaterialBindings, Skybox, irradiance::GpuSkyboxIrradiance},
};

/// Point lights and the environment map, bound as group 3 of the PBR pipeline.
pub struct PbrLightingInformation {
    pub point_lights: GpuPointLightArray,
    pub env_map: GpuSkyboxIrradiance,
}

/// Starts out with the black fallback environment map; `update_environment_map` swaps in the
/// skybox's maps.
impl ConstructFromWorld for PbrLightingInformation {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let queue = world.get_resource::<WgpuQueue>().unwrap();
        Self {
            point_lights: GpuPointLightArray::from_world(world),
            env_map: GpuSkyboxIrradiance::fallback(&device, &queue),
        }
    }
}

/// Loads the skybox's image-based lighting maps whenever the skybox changes.
pub(crate) fn update_environment_map(
    skybox: Res<Skybox>,
    mut lighting: ResMut<PbrLightingInformation>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    match GpuSkyboxIrradiance::load(&skybox, &device, &queue) {
        Ok(env_map) => lighting.env_map = env_map,
        Err(e) => log::error!(
            "Failed to load the environment maps for skybox {}: {e}",
            skybox.path.display()
        ),
    }
}

impl AsBindGroup for PbrLightingInformation {
    const LABEL: &'static str = "PBR Lighting Information";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        let fragment = wgpu::ShaderStages::FRAGMENT;
        layout
            .storage_buffer(fragment, true) // point lights
            .texture_cube(fragment) // environment map diffuse
            .texture_cube(fragment) // environment map specular
            .texture_2d(fragment) // BRDF LUT
            .filtering_sampler(fragment); // environment map sampler
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings
            .buffer_vec(&self.point_lights.buffer)
            .texture(&self.env_map.diffuse)
            .texture(&self.env_map.specular)
            .texture(&self.env_map.brdf_lut)
            .sampler(&self.env_map.sampler);
    }
}

pub struct PbrPipeline {
    pub pipeline: RenderPipeline,
}

impl ConstructFromWorld for PbrPipeline {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let layouts = world.get_resource::<BindGroupLayouts>().unwrap();
        let shaders = world.get_resource::<ShaderLibrary>().unwrap();
        let shader = shaders.load(&device, "pbr.wesl");

        let pipeline = RenderPipelineDescriptor::new("PBR Pipeline", &shader)
            .bind_group_layouts([
                layouts.get::<MaterialBindings>(&device),
                layouts.get::<CameraBindGroup>(&device),
                layouts.get::<ViewMeshInstances>(&device),
                layouts.get::<PbrLightingInformation>(&device),
            ])
            .vertex_buffer(vertex_buffer_layout())
            .cull_mode(Some(wgpu::Face::Back))
            .depth(DepthTest::default())
            .build(&device);

        Self { pipeline }
    }
}

/// Queues every mesh with a material that a view may see into that view's [`Opaque3d`] phase.
#[allow(clippy::type_complexity)]
pub(crate) fn queue_pbr_meshes(
    meshes: Res<RenderAssets<GpuMesh>>,
    materials: Res<RenderAssets<GpuMaterial>>,
    draw_functions: Res<DrawFunctions<Opaque3d>>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<Opaque3d>)>,
    mut items: Query<(Entity, &Handle<Mesh>, &Handle<Material>, &GpuTransform)>,
) {
    let draw_function = draw_functions.id::<DrawPbr>();
    for (view, mut phase) in views.iter() {
        for (entity, mesh_handle, material_handle, transform) in items.iter() {
            // either may still be loading
            let (Some(mesh), true) = (
                meshes.get(*mesh_handle),
                materials.get(*material_handle).is_some(),
            ) else {
                continue;
            };
            if !view.may_see(&mesh.aabb, transform.matrix) {
                continue;
            }
            phase.add(Opaque3d::new(
                entity,
                Opaque3dKey {
                    draw_function,
                    material: material_handle.id(),
                    mesh: mesh_handle.id(),
                },
            ));
        }
    }
}

/// Draws [`Opaque3d`] items with the PBR pipeline: one instanced draw per batch.
pub(crate) struct DrawPbr;

impl DrawFunction<Opaque3d> for DrawPbr {
    type Param = (
        Res<PbrPipeline>,
        Res<RenderAssets<GpuMesh>>,
        Res<RenderAssets<GpuMaterial>>,
        Res<BindGroup<PbrLightingInformation>>,
        Query<(
            &'static BindGroup<CameraBindGroup>,
            &'static BindGroup<ViewMeshInstances>,
        )>,
    );

    fn draw(
        (pipeline, meshes, materials, lights, views): &mut SystemParamItem<Self::Param>,
        pass: &mut TrackedRenderPass,
        view: Entity,
        items: &[Opaque3d],
    ) {
        let Some((camera, instances)) = views.get(view) else {
            return;
        };
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(1, &camera);
        pass.set_bind_group(2, &instances);
        pass.set_bind_group(3, lights);

        for item in items {
            let (Some(mesh), Some(material)) = (
                meshes.get_by_id(item.key.mesh),
                materials.get_by_id(item.key.material),
            ) else {
                continue;
            };
            pass.set_bind_group(0, &material.bind_group);
            pass.set_vertex_buffer(0, &mesh.vertex_buffer);
            pass.set_index_buffer(&mesh.index_buffer, wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, item.batch_range.clone());
        }
    }
}

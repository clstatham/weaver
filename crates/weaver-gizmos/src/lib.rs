//! Immediate-mode debug shapes.
//!
//! Any system can call methods on [`Gizmos`] (for example [`Gizmos::wire_cube`]) to request a shape
//! for the current frame. [`GizmoPlugin`] clears the list at the start of each frame, and the
//! render world copies what's left into instance buffers, one batch per [`GizmoKey`]. The batches
//! are drawn in the `Overlay3d` phase, after the scene, so they show up on top of every view.
//! Only cubes exist so far.

use weaver_app::{App, AppStage, plugin::Plugin};
use weaver_core::{color::Color, prelude::Mat4, transform::Transform};
use weaver_ecs::{
    component::Res,
    entity::Entity,
    prelude::ResMut,
    query::Query,
    system::SystemParamItem,
    world::{ConstructFromWorld, World},
};
use weaver_renderer::{
    RenderApp, RenderStage, WgpuDevice, WgpuQueue,
    bind_group::{
        AsBindGroup, BindGroup, BindGroupLayoutBuilder, BindGroupLayouts, BindingsBuilder,
    },
    buffer::{GpuBuffer, GpuBufferVec},
    camera::CameraBindGroup,
    mesh::{
        primitive::{CubePrimitive, Primitive},
        vertex_buffer_layout,
    },
    phase::{AddDrawFunction, DrawFunction, DrawFunctions, Overlay3d, RenderPhase},
    prelude::*,
    tracked_pass::TrackedRenderPass,
};
use weaver_util::prelude::*;

use wgpu::util::DeviceExt;

/// The types needed to draw gizmos.
pub mod prelude {
    pub use super::{Gizmo, GizmoMode, GizmoPlugin, Gizmos};
}

/// A shape a gizmo can take.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Gizmo {
    /// A unit cube, scaled and placed by the instance's transform.
    Cube,
}

/// A mesh drawn for every instance of a gizmo.
pub struct GizmoMesh {
    /// Vertices in the engine's standard vertex layout.
    pub vertex_buffer: GpuBuffer,
    /// `u32` indices: triangles for solid meshes, line pairs for wireframe ones.
    pub index_buffer: GpuBuffer,
    /// Number of indices to draw.
    pub num_indices: u32,
}

impl GizmoMesh {
    fn cube(device: &wgpu::Device, wireframe: bool) -> Self {
        let mesh = CubePrimitive::new(1.0, wireframe).generate_mesh();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Gizmo Cube Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Gizmo Cube Index Buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer: GpuBuffer::from(vertex_buffer),
            index_buffer: GpuBuffer::from(index_buffer),
            num_indices: mesh.indices.len() as u32,
        }
    }
}

/// The cube meshes for solid and wireframe gizmos.
pub struct GizmoMeshes {
    /// Filled cube.
    pub solid_cube: GizmoMesh,
    /// Cube edges only.
    pub wire_cube: GizmoMesh,
}

impl GizmoMeshes {
    fn get(&self, mode: GizmoMode) -> &GizmoMesh {
        match mode {
            GizmoMode::Solid => &self.solid_cube,
            GizmoMode::Wireframe => &self.wire_cube,
        }
    }
}

impl ConstructFromWorld for GizmoMeshes {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        Self {
            solid_cube: GizmoMesh::cube(&device, false),
            wire_cube: GizmoMesh::cube(&device, true),
        }
    }
}

/// How a gizmo is rasterized.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GizmoMode {
    /// Filled triangles.
    Solid,
    /// Edges only.
    Wireframe,
}

/// Groups gizmos that can be drawn with the same pipeline and instanced together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GizmoKey {
    /// Solid or wireframe.
    pub mode: GizmoMode,
    /// Whether the scene can hide the gizmo. If false it's drawn over everything.
    pub depth_test: bool,
}

/// One gizmo to draw this frame.
pub struct GizmoInstance {
    /// The shape.
    pub gizmo: Gizmo,
    /// Flat color.
    pub color: Color,
    /// Placement and size.
    pub transform: Transform,
}

impl GizmoInstance {
    /// Creates an instance of `gizmo`.
    pub fn new(gizmo: Gizmo, color: Color, transform: Transform) -> Self {
        Self {
            gizmo,
            color,
            transform,
        }
    }
}

/// The gizmos requested this frame, shared between the main and render worlds.
///
/// Systems add shapes through `Res<Gizmos>` (all methods take `&self`); [`GizmoPlugin`] empties it
/// at the start of every frame, so a gizmo has to be re-added each frame it should be visible.
/// Clones share the same list.
///
/// # Examples
///
/// ```no_run
/// use weaver_core::{color::Color, transform::Transform};
/// use weaver_ecs::component::Res;
/// use weaver_gizmos::prelude::*;
///
/// fn mark_origin(gizmos: Res<Gizmos>) {
///     gizmos.wire_cube(Transform::default(), Color::RED);
/// }
/// ```
#[derive(Clone)]
pub struct Gizmos {
    pub(crate) gizmos: SharedLock<FxHashMap<GizmoKey, Vec<GizmoInstance>>>,
}

impl Default for Gizmos {
    fn default() -> Self {
        Self {
            gizmos: SharedLock::new(FxHashMap::default()),
        }
    }
}

impl Gizmos {
    /// Adds `gizmo` to the batch for `key`.
    pub fn add_gizmo(&self, key: GizmoKey, gizmo: GizmoInstance) {
        self.gizmos.write().entry(key).or_default().push(gizmo);
    }

    /// Removes every gizmo.
    pub fn clear(&self) {
        self.gizmos.write().clear();
    }

    /// Draws a filled, depth-tested unit cube with the given transform.
    pub fn solid_cube(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Solid,
                depth_test: true,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }

    /// Draws a depth-tested wireframe unit cube with the given transform.
    pub fn wire_cube(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Wireframe,
                depth_test: true,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }

    /// Draws a filled unit cube over the scene, ignoring depth.
    pub fn solid_cube_no_depth(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Solid,
                depth_test: false,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }

    /// Draws a wireframe unit cube over the scene, ignoring depth.
    pub fn wire_cube_no_depth(&self, transform: Transform, color: Color) {
        self.add_gizmo(
            GizmoKey {
                mode: GizmoMode::Wireframe,
                depth_test: false,
            },
            GizmoInstance::new(Gizmo::Cube, color, transform),
        );
    }
}

/// One batch of gizmo instances on the GPU, bound as group 0 of the gizmo pipeline.
pub struct GizmoInstances {
    transforms: GpuBufferVec<Mat4>,
    colors: GpuBufferVec<Color>,
}

impl Default for GizmoInstances {
    fn default() -> Self {
        Self {
            transforms: GpuBufferVec::new(
                wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            ),
            colors: GpuBufferVec::new(wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST),
        }
    }
}

impl AsBindGroup for GizmoInstances {
    const LABEL: &'static str = "Gizmo Instances";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        let stages = wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT;
        layout
            .storage_buffer(stages, true) // transforms
            .storage_buffer(stages, true); // colors
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings
            .buffer_vec(&self.transforms)
            .buffer_vec(&self.colors);
    }
}

/// A gizmo batch ready to draw this frame.
struct PreparedGizmos {
    instances: GizmoInstances,
    bind_group: Option<BindGroup<GizmoInstances>>,
    count: u32,
}

/// One pipeline per [`GizmoKey`]: solid or wireframe, depth-tested against the scene or drawn
/// over it.
pub struct GizmoPipelines {
    pipelines: FxHashMap<GizmoKey, RenderPipeline>,
}

impl GizmoPipelines {
    fn get(&self, key: GizmoKey) -> &RenderPipeline {
        &self.pipelines[&key]
    }
}

impl ConstructFromWorld for GizmoPipelines {
    fn from_world(world: &World) -> Self {
        let device = world.get_resource::<WgpuDevice>().unwrap();
        let layouts = world.get_resource::<BindGroupLayouts>().unwrap();
        let shaders = world.get_resource::<ShaderLibrary>().unwrap();
        let shader = shaders.load(&device, "gizmos.wesl");

        let mut pipelines = FxHashMap::default();
        for mode in [GizmoMode::Solid, GizmoMode::Wireframe] {
            for depth_test in [true, false] {
                let topology = match mode {
                    GizmoMode::Solid => wgpu::PrimitiveTopology::TriangleList,
                    GizmoMode::Wireframe => wgpu::PrimitiveTopology::LineList,
                };
                let depth = if depth_test {
                    DepthTest {
                        compare: wgpu::CompareFunction::Less,
                        write: true,
                    }
                } else {
                    // drawn over the scene, in queue order
                    DepthTest {
                        compare: wgpu::CompareFunction::Always,
                        write: false,
                    }
                };
                let pipeline = RenderPipelineDescriptor::new("Gizmo Pipeline", &shader)
                    .bind_group_layouts([
                        layouts.get::<GizmoInstances>(&device),
                        layouts.get::<CameraBindGroup>(&device),
                    ])
                    .vertex_buffer(vertex_buffer_layout())
                    .topology(topology)
                    .depth(depth)
                    .build(&device);
                pipelines.insert(GizmoKey { mode, depth_test }, pipeline);
            }
        }

        Self { pipelines }
    }
}

/// The render world's copy of the gizmos to draw this frame.
#[derive(Default)]
pub struct GizmoRenderable {
    batches: FxHashMap<GizmoKey, PreparedGizmos>,
}

/// Copies the shared [`Gizmos`] into per-key GPU buffers, reusing last frame's allocations.
pub fn prepare_gizmos(
    gizmos: Res<Gizmos>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
    layouts: Res<BindGroupLayouts>,
    mut gizmo_renderable: ResMut<GizmoRenderable>,
) {
    // snapshot the shared gizmo list now: the main thread may change it while this frame renders
    let gizmos = gizmos.gizmos.read();
    let batches = &mut gizmo_renderable.batches;
    for batch in batches.values_mut() {
        batch.count = 0;
    }
    for (key, instances) in gizmos.iter() {
        let batch = batches.entry(*key).or_insert_with(|| PreparedGizmos {
            instances: GizmoInstances::default(),
            bind_group: None,
            count: 0,
        });

        batch.instances.transforms.clear();
        batch.instances.colors.clear();
        for gizmo in instances {
            batch.instances.transforms.push(gizmo.transform.matrix());
            batch.instances.colors.push(gizmo.color);
        }
        batch.count = instances.len() as u32;
        if batch.count == 0 {
            continue;
        }
        batch.instances.transforms.enqueue_update(&device, &queue);
        batch.instances.colors.enqueue_update(&device, &queue);

        let outdated = batch
            .bind_group
            .as_ref()
            .is_none_or(|bind_group| bind_group.is_outdated(&batch.instances));
        if outdated {
            let layout = layouts.get::<GizmoInstances>(&device);
            batch.bind_group = Some(BindGroup::new(&device, &layout, &batch.instances));
        }
    }
}

/// Puts the gizmos over every view, if there are any.
fn queue_gizmos(
    gizmo_renderable: Res<GizmoRenderable>,
    draw_functions: Res<DrawFunctions<Overlay3d>>,
    mut views: Query<(Entity, &mut RenderPhase<Overlay3d>)>,
) {
    if gizmo_renderable
        .batches
        .values()
        .all(|batch| batch.count == 0)
    {
        return;
    }
    let draw_function = draw_functions.id::<DrawGizmos>();
    for (view, mut phase) in views.iter() {
        phase.add(Overlay3d {
            entity: view,
            draw_function,
        });
    }
}

/// Draws every gizmo batch, depth-tested ones first.
struct DrawGizmos;

impl DrawFunction<Overlay3d> for DrawGizmos {
    type Param = (
        Res<GizmoRenderable>,
        Res<GizmoPipelines>,
        Res<GizmoMeshes>,
        Query<&'static BindGroup<CameraBindGroup>>,
    );

    fn draw(
        (gizmos, pipelines, meshes, cameras): &mut SystemParamItem<Self::Param>,
        pass: &mut TrackedRenderPass,
        view: Entity,
        _items: &[Overlay3d],
    ) {
        let Some(camera) = cameras.get(view) else {
            return;
        };
        let mut batches = gizmos
            .batches
            .iter()
            .filter(|(_, batch)| batch.count > 0)
            .collect::<Vec<_>>();
        batches.sort_by_key(|(key, _)| !key.depth_test);

        pass.set_bind_group(1, &camera);
        for (key, batch) in batches {
            let Some(bind_group) = &batch.bind_group else {
                continue;
            };
            let mesh = meshes.get(key.mode);
            pass.set_pipeline(pipelines.get(*key));
            pass.set_bind_group(0, bind_group);
            pass.set_vertex_buffer(0, &mesh.vertex_buffer);
            pass.set_index_buffer(&mesh.index_buffer, wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_indices, 0, 0..batch.count);
        }
    }
}

/// Adds gizmo drawing: the [`Gizmos`] resource, and an overlay drawn over every view.
///
/// Not part of `DefaultPlugins`. Needs [`RenderApp`] to exist, so add it after the renderer.
///
/// # Panics
///
/// Panics in `build` if there's no [`RenderApp`] sub-app.
pub struct GizmoPlugin;

impl Plugin for GizmoPlugin {
    fn build(&self, app: &mut App) -> Result<()> {
        let gizmos = Gizmos::default();
        app.insert_resource(gizmos.clone());
        app.add_system(clear_gizmos, AppStage::PrepareFrame);

        let render_app = app.get_sub_app_mut::<RenderApp>().unwrap();
        render_app.add_plugin(GizmoRenderAppPlugin { gizmos })?;

        Ok(())
    }
}

/// The render-world half of [`GizmoPlugin`]: hands the shared [`Gizmos`] to the render app and
/// registers the prepare/queue systems and the draw function.
pub struct GizmoRenderAppPlugin {
    gizmos: Gizmos,
}

impl Plugin for GizmoRenderAppPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.insert_resource(self.gizmos.clone());
        render_app.add_plugin(InitRenderResourcePlugin::<GizmoMeshes>::default())?;
        render_app.add_plugin(InitRenderResourcePlugin::<GizmoPipelines>::default())?;
        render_app.add_plugin(InitRenderResourcePlugin::<GizmoRenderable>::default())?;

        render_app.add_draw_function::<Overlay3d, DrawGizmos>();
        render_app.add_system(prepare_gizmos, RenderStage::Prepare);
        render_app.add_system(queue_gizmos, RenderStage::Queue);
        Ok(())
    }
}

// FIXME: can clear the list before the render thread has prepared the previous frame from it
fn clear_gizmos(gizmos: Res<Gizmos>) {
    gizmos.clear();
}

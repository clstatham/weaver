use weaver_app::{plugin::Plugin, App};
use weaver_core::color::Color;
use weaver_ecs::{
    entity::Entity,
    prelude::{Component, Reflect},
    world::World,
};
use weaver_renderer::{
    bind_group::BindGroup,
    camera::{Camera, CameraRenderGraph, GpuCamera},
    clear_color::{ClearColorLabel, ClearColorNode},
    extract::{RenderComponent, RenderComponentPlugin},
    graph::{GraphInputLabel, RenderLabel, RenderNode, Slot, ViewNodeRunner},
    PreRender, Render,
};
use weaver_util::prelude::Result;

use crate::render::{PbrNode, PbrNodeLabel};

#[derive(Component, Reflect)]
pub struct CameraRenderComponent {
    pub camera: Camera,
    pub pbr_camera: PbrCamera,
}

impl RenderComponent for CameraRenderComponent {
    type ExtractQuery<'a> = (&'a Camera, &'a PbrCamera);

    fn extract_render_component(
        entity: Entity,
        main_world: &mut World,
        _render_world: &mut World,
    ) -> Option<Self> {
        let camera = main_world.get_component::<Camera>(entity).unwrap();
        let pbr_camera = main_world.get_component::<PbrCamera>(entity).unwrap();
        Some(Self {
            camera: *camera,
            pbr_camera: *pbr_camera,
        })
    }

    fn update_render_component(
        &mut self,
        entity: Entity,
        main_world: &mut World,
        _render_world: &mut World,
    ) -> Result<()> {
        let camera = main_world.get_component::<Camera>(entity).unwrap();
        let pbr_camera = main_world.get_component::<PbrCamera>(entity).unwrap();
        self.camera = *camera;
        self.pbr_camera = *pbr_camera;
        Ok(())
    }
}

pub struct PbrCameraBindGroupNode {
    camera_entity: Entity,
}

impl RenderNode for PbrCameraBindGroupNode {
    fn run(
        &self,
        render_world: &World,
        graph_ctx: &mut weaver_renderer::graph::RenderGraphCtx,
        _render_ctx: &mut weaver_renderer::graph::RenderCtx,
    ) -> Result<()> {
        let bind_group = render_world
            .get_component::<BindGroup<GpuCamera>>(self.camera_entity)
            .unwrap();
        let bind_group = bind_group.bind_group().clone();
        graph_ctx.set_output(0, Slot::BindGroup(bind_group));
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PbrCameraBindGroupLabel;
impl RenderLabel for PbrCameraBindGroupLabel {}

#[derive(Component, Reflect, Clone, Copy)]
pub struct PbrCamera {
    clear_color: Color,
}

impl PbrCamera {
    pub fn new(clear_color: Color) -> Self {
        Self { clear_color }
    }
}

pub struct PbrCameraPlugin;

impl Plugin for PbrCameraPlugin {
    fn build(&self, render_app: &mut App) -> Result<()> {
        render_app.add_plugin(RenderComponentPlugin::<CameraRenderComponent>::default())?;
        render_app.add_system(setup_pbr_render_graphs, PreRender);
        render_app.add_system(render_pbr_cameras, Render);
        Ok(())
    }
}

fn setup_pbr_render_graphs(render_world: &mut World) -> Result<()> {
    let camera_query = render_world.query::<(&CameraRenderComponent, &mut CameraRenderGraph)>();

    for (camera_entity, (camera_render_component, mut graph)) in camera_query.iter() {
        let base_camera = camera_render_component.camera;
        let pbr_camera = camera_render_component.pbr_camera;
        if base_camera.active() {
            let graph = graph.render_graph_mut();
            if !graph.has_node(PbrNodeLabel) {
                graph
                    .add_node(
                        PbrNodeLabel,
                        ViewNodeRunner::new(PbrNode::new(camera_entity), render_world),
                    )
                    .unwrap();
                graph
                    .add_node(
                        ClearColorLabel,
                        ViewNodeRunner::new(
                            ClearColorNode::new(pbr_camera.clear_color),
                            render_world,
                        ),
                    )
                    .unwrap();

                graph
                    .try_add_node_edge(GraphInputLabel, ClearColorLabel)
                    .unwrap();

                graph
                    .try_add_node_edge(ClearColorLabel, PbrNodeLabel)
                    .unwrap();
            }
        }
    }

    Ok(())
}

fn render_pbr_cameras(render_world: &mut World) -> Result<()> {
    let camera_query = render_world.query::<(&CameraRenderComponent, &mut CameraRenderGraph)>();

    for (entity, (camera, mut graph)) in camera_query.iter() {
        if camera.camera.active() {
            let mut renderer = render_world
                .get_resource_mut::<weaver_renderer::Renderer>()
                .unwrap();
            let device = render_world
                .get_resource::<weaver_renderer::WgpuDevice>()
                .unwrap();
            let queue = render_world
                .get_resource::<weaver_renderer::WgpuQueue>()
                .unwrap();
            graph.render_graph_mut().prepare(render_world)?;
            graph
                .render_graph()
                .run(&device, &queue, &mut renderer, render_world, entity)?;
        }
    }

    Ok(())
}

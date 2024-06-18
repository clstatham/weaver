use weaver_app::{plugin::Plugin, App};
use weaver_core::color::Color;
use weaver_ecs::{
    entity::Entity,
    prelude::{Component, Reflect},
    world::World,
};
use weaver_renderer::{
    bind_group::BindGroup,
    camera::{Camera, GpuCamera},
    clear_color::ClearColor,
    extract::{RenderComponent, RenderComponentPlugin},
    graph::{RenderGraph, RenderNode, Slot, StartNode},
    CurrentFrame, PreRender, Render,
};
use weaver_util::prelude::Result;

use crate::{light::PointLightArrayNode, render::PbrNode};

#[derive(Component, Reflect)]
pub struct CameraRenderComponent {
    pub camera: Camera,
    pub pbr_camera: PbrCamera,
    #[reflect(ignore)]
    pub graph: RenderGraph,
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
        let graph = RenderGraph::new();
        Some(Self {
            camera: *camera,
            pbr_camera: *pbr_camera,
            graph,
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

impl weaver_renderer::graph::Render for PbrCameraBindGroupNode {
    fn prepare(&mut self, _render_world: &mut World) -> Result<()> {
        Ok(())
    }

    fn render(&mut self, render_world: &mut World, _input_slots: &[Slot]) -> Result<Vec<Slot>> {
        let bind_group = render_world
            .get_component::<BindGroup<GpuCamera>>(self.camera_entity)
            .unwrap();
        Ok(vec![Slot::BindGroup(bind_group.bind_group().clone())])
    }
}

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
        render_app.add_system(prepare_pbr_cameras, PreRender);
        render_app.add_system(render_pbr_cameras, Render);
        Ok(())
    }
}

fn prepare_pbr_cameras(render_world: &mut World) -> Result<()> {
    let camera_query = render_world.query::<&mut CameraRenderComponent>();

    for (camera_entity, mut camera_render_component) in camera_query.iter() {
        let base_camera = camera_render_component.camera;
        let pbr_camera = camera_render_component.pbr_camera;
        if base_camera.active() {
            let graph = &mut camera_render_component.graph;

            if graph.node_index::<PbrNode>().is_none() {
                let camera_bind_group_node = graph.add_node(RenderNode::new(
                    "PbrCameraBindGroupNode",
                    PbrCameraBindGroupNode { camera_entity },
                ));
                let pbr_node =
                    graph.add_node(RenderNode::new("PbrNode", PbrNode::new(camera_entity)));
                let clear_color_node = graph.add_node(RenderNode::new(
                    "ClearColor",
                    ClearColor::new(pbr_camera.clear_color),
                ));
                let start_node = graph.node_index::<StartNode>().unwrap();

                // start:color -> clear:color
                graph.add_edge(start_node, 0, clear_color_node, 0);
                // start:depth -> clear:depth
                graph.add_edge(start_node, 1, clear_color_node, 1);

                // clear:color -> pbr:color
                graph.add_edge(clear_color_node, 0, pbr_node, 0);
                // clear:depth -> pbr:depth
                graph.add_edge(clear_color_node, 1, pbr_node, 1);

                // camera:bind_group -> pbr:camera_bind_group
                graph.add_edge(camera_bind_group_node, 0, pbr_node, 2);
            }

            drop(camera_render_component);
            let camera_render_component = render_world
                .get_component::<CameraRenderComponent>(camera_entity)
                .unwrap();
            camera_render_component.graph.prepare(render_world)?;
        }
    }

    Ok(())
}

fn render_pbr_cameras(render_world: &mut World) -> Result<()> {
    let camera_query = render_world.query::<&CameraRenderComponent>();

    for (_entity, camera) in camera_query.iter() {
        if camera.camera.active() && render_world.has_resource::<CurrentFrame>() {
            camera.graph.render(render_world)?;
        }
    }

    Ok(())
}

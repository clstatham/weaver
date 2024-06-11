use std::rc::Rc;

use weaver_app::{plugin::Plugin, App};
use weaver_core::color::Color;
use weaver_ecs::{entity::Entity, prelude::Component, system::SystemStage, world::World};
use weaver_renderer::{
    bind_group::BindGroup,
    camera::{Camera, GpuCamera},
    clear_color::ClearColor,
    graph::{EndNode, Render, RenderNode, Slot, StartNode},
    Renderer,
};
use weaver_util::prelude::Result;

use crate::{light::PointLightArrayNode, render::PbrNode};

struct PbrCameraBindGroupNode {
    camera_entity: Entity,
}

impl Render for PbrCameraBindGroupNode {
    fn prepare(&self, _world: Rc<World>, _renderer: &Renderer) -> Result<()> {
        Ok(())
    }

    fn render(
        &self,
        world: Rc<World>,
        _renderer: &Renderer,
        _input_slots: &[Slot],
    ) -> Result<Vec<Slot>> {
        let bind_group = world
            .get_component::<BindGroup<GpuCamera>>(self.camera_entity)
            .unwrap();
        Ok(vec![Slot::BindGroup(bind_group.bind_group().clone())])
    }
}

#[derive(Component)]
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
    fn build(&self, app: &mut App) -> Result<()> {
        app.add_system(prepare_pbr_cameras, SystemStage::PreRender)?;
        app.add_system(render_pbr_cameras, SystemStage::Render)?;
        Ok(())
    }
}

fn prepare_pbr_cameras(world: Rc<World>) -> Result<()> {
    let camera_query = world.clone().query::<(&mut Camera, &PbrCamera)>();

    for (camera_entity, (mut base_camera, pbr_camera)) in camera_query.iter() {
        if base_camera.active() {
            let graph = base_camera.render_graph_mut();
            let renderer = world.get_resource::<Renderer>().unwrap();

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
                let point_light_array_node =
                    graph.add_node(RenderNode::new("PointLightArrayNode", PointLightArrayNode));
                let start_node = graph.node_index::<StartNode>().unwrap();
                let end_node = graph.node_index::<EndNode>().unwrap();

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

                // point_light_array -> pbr:point_light_array
                graph.add_edge(point_light_array_node, 0, pbr_node, 3);

                // pbr:color -> end:color
                graph.add_edge(pbr_node, 0, end_node, 0);
                // pbr:depth -> end:depth
                graph.add_edge(pbr_node, 1, end_node, 1);
            }

            drop(base_camera);
            let base_camera = world.get_component::<Camera>(camera_entity).unwrap();
            base_camera.graph.prepare(world.clone(), &renderer)?;
        }
    }

    Ok(())
}

fn render_pbr_cameras(world: Rc<World>) -> Result<()> {
    let camera_query = world.query::<&mut Camera>();

    for (_entity, mut camera) in camera_query.iter() {
        if camera.active() {
            let graph = camera.render_graph_mut();
            let renderer = world.get_resource::<Renderer>().unwrap();
            graph.render(world.clone(), &renderer)?;
        }
    }

    Ok(())
}

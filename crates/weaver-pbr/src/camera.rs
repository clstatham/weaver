use weaver_app::{plugin::Plugin, App};
use weaver_core::color::Color;
use weaver_ecs::{entity::Entity, query::Query, system::SystemStage, world::World};
use weaver_renderer::{
    bind_group::BindGroup,
    camera::{Camera, GpuCamera},
    clear_color::ClearColor,
    graph::{EndNode, Render, RenderNode, Slot, StartNode},
    Renderer,
};
use weaver_util::prelude::{bail, Result};

use crate::{light::PointLightArrayNode, render::PbrNode};

struct PbrCameraBindGroupNode {
    camera_entity: Option<Entity>,
}

impl Render for PbrCameraBindGroupNode {
    fn prepare(&mut self, _world: &World, _renderer: &Renderer, entity: Entity) -> Result<()> {
        self.camera_entity = Some(entity);
        Ok(())
    }

    fn render(
        &self,
        world: &World,
        _renderer: &Renderer,
        _input_slots: &[Slot],
    ) -> Result<Vec<Slot>> {
        let Some(camera_entity) = self.camera_entity else {
            bail!("PbrCameraBindGroupNode expected a camera entity");
        };
        let bind_group = world
            .get_component::<BindGroup<GpuCamera>>(camera_entity)
            .unwrap();
        Ok(vec![Slot::BindGroup(bind_group.bind_group().clone())])
    }
}

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

fn prepare_pbr_cameras(world: &World) -> Result<()> {
    let camera_query = world.query(&Query::new().write::<Camera>().read::<PbrCamera>());

    for camera_entity in camera_query.iter() {
        let pbr_camera = camera_query.get::<PbrCamera>(camera_entity).unwrap();
        let mut base_camera = camera_query.get_mut::<Camera>(camera_entity).unwrap();
        if base_camera.active() {
            let graph = base_camera.render_graph_mut();
            let renderer = world.get_resource::<Renderer>().unwrap();

            if graph.node_index::<PbrNode>().is_none() {
                let camera_bind_group_node = graph.add_node(RenderNode::new(
                    "PbrCameraBindGroupNode",
                    PbrCameraBindGroupNode {
                        camera_entity: None,
                    },
                ));
                let pbr_node = graph.add_node(RenderNode::new("PbrNode", PbrNode::default()));
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

            graph.prepare(world, &renderer, camera_entity)?;
        }
    }

    Ok(())
}

fn render_pbr_cameras(world: &World) -> Result<()> {
    let camera_query = world.query(&Query::new().write::<Camera>());

    for camera_entity in camera_query.iter() {
        let mut camera = camera_query.get_mut::<Camera>(camera_entity).unwrap();
        if camera.active() {
            let graph = camera.render_graph_mut();
            let renderer = world.get_resource::<Renderer>().unwrap();
            graph.prepare(world, &renderer, camera_entity)?;
            graph.render(world, &renderer)?;
        }
    }

    Ok(())
}

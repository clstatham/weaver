use std::fmt::Debug;

use weaver_app::plugin::Plugin;
use weaver_core::color::Color;
use weaver_ecs::{query::Query, system::SystemStage, world::World};

use crate::{
    clear_color::ClearColor,
    graph::{RenderGraph, RenderNode},
    target::RenderTarget,
    Renderer,
};

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut weaver_app::App) -> anyhow::Result<()> {
        app.add_system(prepare_cameras, SystemStage::PreRender)?;
        app.add_system(render_cameras, SystemStage::Render)?;
        Ok(())
    }
}

fn prepare_cameras(world: &World) -> anyhow::Result<()> {
    let camera_query = world.query(&Query::new().read::<Camera>());

    for camera_entity in camera_query.iter() {
        let camera = camera_query.get::<Camera>(camera_entity).unwrap();
        if camera.active() {
            let graph = &camera.graph;
            let renderer = world.get_resource::<Renderer>().unwrap();
            graph.prepare(world, &renderer)?;
        }
    }

    Ok(())
}

fn render_cameras(world: &World) -> anyhow::Result<()> {
    let camera_query = world.query(&Query::new().read::<Camera>());

    for camera_entity in camera_query.iter() {
        let camera = camera_query.get::<Camera>(camera_entity).unwrap();
        if camera.active() {
            let graph = &camera.graph;
            let renderer = world.get_resource::<Renderer>().unwrap();
            graph.render(world, &renderer)?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub inv_view: glam::Mat4,
    pub inv_proj: glam::Mat4,
    pub camera_position: glam::Vec3,
    pub _padding: u32,
}

impl From<&Camera> for CameraUniform {
    fn from(camera: &Camera) -> Self {
        let view = camera.view_matrix;
        let proj = camera.projection_matrix;
        let inv_view = view.inverse();
        let inv_proj = proj.inverse();
        let camera_position = inv_view.col(3).truncate();

        Self {
            view,
            proj,
            inv_view,
            inv_proj,
            camera_position,
            _padding: 0,
        }
    }
}

pub struct Camera {
    pub active: bool,
    pub graph: RenderGraph,
    pub view_matrix: glam::Mat4,
    pub projection_matrix: glam::Mat4,
}

impl Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("view_matrix", &self.view_matrix)
            .field("projection_matrix", &self.projection_matrix)
            .finish()
    }
}

impl Camera {
    pub fn new(view_matrix: glam::Mat4, projection_matrix: glam::Mat4) -> Self {
        let mut graph = RenderGraph::new();
        graph.add_node(RenderNode::new(
            "clear_color",
            ClearColor::new(Color::RED),
            RenderTarget::PrimaryScreen,
        ));

        Self {
            active: true,
            graph,
            view_matrix,
            projection_matrix,
        }
    }

    pub fn perspective_lookat(
        eye: glam::Vec3,
        center: glam::Vec3,
        up: glam::Vec3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self::new(
            glam::Mat4::look_at_rh(eye, center, up),
            glam::Mat4::perspective_rh(fov, aspect, near, far),
        )
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn activate(&mut self) {
        self.set_active(true);
    }

    pub fn deactivate(&mut self) {
        self.set_active(false);
    }

    pub fn render_graph(&self) -> &RenderGraph {
        &self.graph
    }

    pub fn render_graph_mut(&mut self) -> &mut RenderGraph {
        &mut self.graph
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(glam::Mat4::IDENTITY, glam::Mat4::IDENTITY)
    }
}

use std::{fmt::Debug, sync::Arc};

use encase::ShaderType;
use weaver_core::geometry::Ray;
use weaver_util::prelude::Result;

use weaver_app::plugin::Plugin;
use weaver_ecs::prelude::*;

use crate::{
    begin_render,
    bind_group::{BindGroupLayout, ComponentBindGroupPlugin, CreateBindGroup},
    buffer::GpuBufferVec,
    end_render,
    extract::{RenderComponent, RenderComponentPlugin},
    CurrentFrame, PostRender, PreRender, WgpuDevice, WgpuQueue,
};

#[derive(Component, Reflect, Clone, Copy)]
pub struct PrimaryCamera;

impl RenderComponent for PrimaryCamera {
    type ExtractQuery<'a> = &'a Self;

    fn extract_render_component(
        entity: Entity,
        main_world: &mut World,
        _render_world: &mut World,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let camera = main_world.get_component::<PrimaryCamera>(entity)?;
        Some(*camera)
    }

    fn update_render_component(
        &mut self,
        _entity: Entity,
        _main_world: &mut World,
        _render_world: &mut World,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Component, Reflect)]
pub struct ViewTarget {
    #[reflect(ignore)]
    pub color_target: Arc<wgpu::TextureView>,
    #[reflect(ignore)]
    pub depth_target: Arc<wgpu::TextureView>,
}

impl From<&CurrentFrame> for ViewTarget {
    fn from(current_frame: &CurrentFrame) -> Self {
        Self {
            color_target: current_frame.color_view.clone(),
            depth_target: current_frame.depth_view.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, ShaderType)]
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

#[derive(Component, Reflect, Clone, Copy)]
pub struct Camera {
    pub active: bool,
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
        Self {
            active: true,
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

    pub fn screen_to_ray(&self, screen_pos: glam::Vec2, screen_size: glam::Vec2) -> Ray {
        let ndc = glam::Vec2::new(
            (2.0 * screen_pos.x / screen_size.x) - 1.0,
            1.0 - (2.0 * screen_pos.y / screen_size.y),
        );

        let inv_proj = self.projection_matrix.inverse();
        let inv_view = self.view_matrix.inverse();

        let clip = glam::Vec4::new(ndc.x, ndc.y, -1.0, 1.0);
        let eye = inv_proj * clip;
        let eye = glam::Vec4::new(eye.x, eye.y, -1.0, 0.0);
        let world = inv_view * eye;

        Ray::new(inv_view.col(3).truncate(), world.truncate().normalize())
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new(glam::Mat4::IDENTITY, glam::Mat4::IDENTITY)
    }
}

#[derive(Component, Reflect)]
pub struct GpuCamera {
    pub camera: Camera,
    #[reflect(ignore)]
    pub uniform_buffer: GpuBufferVec<CameraUniform>,
}

impl RenderComponent for GpuCamera {
    type ExtractQuery<'a> = &'a Camera;

    fn extract_render_component(
        entity: Entity,
        main_world: &mut World,
        render_world: &mut World,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let camera = main_world.get_component::<Camera>(entity)?;

        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let queue = render_world.get_resource::<WgpuQueue>().unwrap();

        let mut uniform_buffer =
            GpuBufferVec::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
        uniform_buffer.push(CameraUniform::from(&*camera));

        uniform_buffer.enqueue_update(&device, &queue);

        Some(Self {
            camera: *camera,
            uniform_buffer,
        })
    }

    fn update_render_component(
        &mut self,
        entity: Entity,
        main_world: &mut World,
        render_world: &mut World,
    ) -> Result<()> {
        let camera = main_world.get_component::<Camera>(entity).unwrap();

        let device = render_world.get_resource::<WgpuDevice>().unwrap();
        let queue = render_world.get_resource::<WgpuQueue>().unwrap();

        self.camera = *camera;

        self.uniform_buffer.clear();
        self.uniform_buffer.push(CameraUniform::from(&*camera));

        self.uniform_buffer.enqueue_update(&device, &queue);

        Ok(())
    }
}

impl CreateBindGroup for GpuCamera {
    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: cached_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: self.uniform_buffer.buffer().unwrap(),
                    offset: 0,
                    size: None,
                }),
            }],
            label: Some("Camera Bind Group"),
        })
    }

    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut weaver_app::App) -> Result<()> {
        app.add_plugin(RenderComponentPlugin::<GpuCamera>::default())?;
        app.add_plugin(RenderComponentPlugin::<PrimaryCamera>::default())?;
        app.add_plugin(ComponentBindGroupPlugin::<GpuCamera>::default())?;

        app.add_system_after(insert_view_target, begin_render, PreRender);
        app.add_system_before(remove_view_target, end_render, PostRender);

        Ok(())
    }
}

fn insert_view_target(mut render_world: WriteWorld) -> Result<()> {
    if let Some(current_frame) = render_world.get_resource::<CurrentFrame>() {
        let query = render_world.query::<&PrimaryCamera>();
        for (gpu_camera, primary_camera) in query.iter() {
            let view_target = ViewTarget::from(&*current_frame);
            drop(primary_camera);
            render_world.insert_component(gpu_camera, view_target);
        }
    } else {
        log::warn!("CurrentFrame resource not found");
    }

    Ok(())
}

fn remove_view_target(mut render_world: WriteWorld) -> Result<()> {
    let query = render_world.query_filtered::<&PrimaryCamera, With<ViewTarget>>();
    for (gpu_camera, primary_camera) in query.iter() {
        drop(primary_camera);
        render_world.remove_component::<ViewTarget>(gpu_camera);
    }

    Ok(())
}

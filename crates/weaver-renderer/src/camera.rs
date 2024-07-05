use std::{fmt::Debug, sync::Arc};

use encase::ShaderType;
use weaver_core::geometry::{Aabb, Intersection, Plane, Ray};
use weaver_util::prelude::Result;

use weaver_app::plugin::Plugin;
use weaver_ecs::prelude::*;

use crate::{
    begin_render,
    bind_group::{BindGroupLayout, ComponentBindGroupPlugin, CreateBindGroup},
    buffer::GpuBufferVec,
    end_render,
    extract::{ExtractComponent, ExtractComponentPlugin},
    hdr::HdrRenderTarget,
    CurrentFrame, ExtractBindGroupStage, PostRender, PreRender, WgpuDevice, WgpuQueue,
};

#[derive(Component, Clone, Copy)]
pub struct PrimaryCamera;

impl ExtractComponent for PrimaryCamera {
    type ExtractQueryFetch = &'static Self;
    type ExtractQueryFilter = ();
    type Out = Self;

    fn extract_render_component(
        item: QueryFetchItem<'_, Self::ExtractQueryFetch>,
    ) -> Option<Self::Out> {
        Some(*item)
    }
}

#[derive(Component, Clone)]
pub struct ViewTarget {
    pub color_target: Arc<wgpu::TextureView>,
    pub depth_target: Arc<wgpu::TextureView>,
}

impl From<(&CurrentFrame, &HdrRenderTarget)> for ViewTarget {
    fn from((current_frame, hdr_target): (&CurrentFrame, &HdrRenderTarget)) -> Self {
        Self {
            color_target: hdr_target.color_target().clone(),
            depth_target: current_frame.inner.as_ref().unwrap().depth_view.clone(),
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
    pub fn perspective_lookat(
        eye: glam::Vec3,
        center: glam::Vec3,
        up: glam::Vec3,
        fov: f32,
        aspect: f32,
        near: f32,
        far: f32,
    ) -> Self {
        let view = glam::Mat4::look_at_rh(eye, center, up);
        let proj = glam::Mat4::perspective_rh_gl(fov, aspect, near, far);
        Self {
            active: true,
            view_matrix: view,
            projection_matrix: proj,
        }
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

    pub fn world_to_screen(
        &self,
        world_pos: glam::Vec3,
        screen_size: glam::Vec2,
    ) -> Option<glam::Vec2> {
        let clip_from_world = self.projection_matrix * self.view_matrix;
        let ndc = clip_from_world.project_point3(world_pos);
        let mut screen = (ndc.truncate() + glam::Vec2::ONE) / 2.0 * screen_size;
        screen.y = screen_size.y - screen.y;
        Some(screen)
    }

    pub fn intersect_frustum_with_aabb(
        &self,
        aabb: &Aabb,
        intersect_near: bool,
        intersect_far: bool,
    ) -> Intersection {
        let planes = self.frustum_planes();

        for (i, plane) in planes.into_iter().enumerate() {
            if i == 4 && !intersect_near {
                continue;
            }
            if i == 5 && !intersect_far {
                continue;
            }
            let center = aabb.center().extend(1.0);
            let rel_rad = aabb.relative_radius(plane.normal);
            let normal_d = plane.to_coefficients();
            let distance = center.dot(normal_d) + rel_rad;
            if distance <= 0.0 {
                return Intersection::Outside;
            }
        }

        Intersection::Inside
    }

    /// Returns the frustum planes in the following order:
    /// - Left
    /// - Right
    /// - Bottom
    /// - Top
    /// - Near
    /// - Far
    pub fn frustum_planes(&self) -> [Plane; 6] {
        let clip_from_view = self.projection_matrix * self.view_matrix;

        let row1 = clip_from_view.row(0);
        let row2 = clip_from_view.row(1);
        let row3 = clip_from_view.row(2);
        let row4 = clip_from_view.row(3);

        [
            // Left
            Plane::from_coefficient_vec4(row4 + row1, true),
            // Right
            Plane::from_coefficient_vec4(row4 - row1, true),
            // Bottom
            Plane::from_coefficient_vec4(row4 + row2, true),
            // Top
            Plane::from_coefficient_vec4(row4 - row2, true),
            // Near
            Plane::from_coefficient_vec4(row4 + row3, true),
            // Far
            Plane::from_coefficient_vec4(row4 - row3, true),
        ]
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            active: true,
            view_matrix: glam::Mat4::IDENTITY,
            projection_matrix: glam::Mat4::IDENTITY,
        }
    }
}

#[derive(Component)]
pub struct GpuCamera {
    pub camera: Camera,
}

impl ExtractComponent for GpuCamera {
    type ExtractQueryFetch = &'static Camera;
    type ExtractQueryFilter = ();
    type Out = Self;
    fn extract_render_component(camera: QueryFetchItem<Self::ExtractQueryFetch>) -> Option<Self>
    where
        Self: Sized,
    {
        Some(Self { camera: *camera })
    }
}

#[derive(Component)]
pub struct CameraBindGroup {
    pub buffer: GpuBufferVec<CameraUniform>,
}

impl CreateBindGroup for CameraBindGroup {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
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

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &BindGroupLayout,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    self.buffer.buffer().unwrap().as_entire_buffer_binding(),
                ),
            }],
        })
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut weaver_app::App) -> Result<()> {
        app.add_plugin(ExtractComponentPlugin::<GpuCamera>::default())?;
        app.add_plugin(ComponentBindGroupPlugin::<CameraBindGroup>::default())?;
        app.add_plugin(ExtractComponentPlugin::<PrimaryCamera>::default())?;

        app.add_system(extract_camera_bind_groups, ExtractBindGroupStage);
        app.add_system_after(insert_view_target, begin_render, PreRender);
        app.add_system_before(remove_view_target, end_render, PostRender);

        Ok(())
    }
}

pub fn extract_camera_bind_groups(
    commands: Commands,
    query: Query<(&GpuCamera, Option<&mut CameraBindGroup>)>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) -> Result<()> {
    for (entity, (gpu_camera, mut bind_group)) in query.iter() {
        let camera_uniform = CameraUniform::from(&gpu_camera.camera);
        if let Some(bind_group) = bind_group.as_mut() {
            bind_group.buffer.clear();
            bind_group.buffer.push(camera_uniform);
            bind_group.buffer.enqueue_update(&device, &queue);
        } else {
            let mut buffer =
                GpuBufferVec::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
            buffer.push(camera_uniform);
            buffer.enqueue_update(&device, &queue);
            let bind_group = CameraBindGroup { buffer };
            commands.insert_component(entity, bind_group);
        }
    }

    Ok(())
}

pub fn insert_view_target(
    commands: Commands,
    current_frame: Res<CurrentFrame>,
    hdr_target: Res<HdrRenderTarget>,
    query: Query<&GpuCamera>,
) -> Result<()> {
    for gpu_camera in query.entity_iter() {
        let view_target = ViewTarget::from((&*current_frame, &*hdr_target));
        commands.insert_component(gpu_camera, view_target);
    }

    Ok(())
}

pub fn remove_view_target(commands: Commands, query: Query<&GpuCamera>) -> Result<()> {
    for gpu_camera in query.entity_iter() {
        commands.remove_component::<ViewTarget>(gpu_camera);
    }

    Ok(())
}

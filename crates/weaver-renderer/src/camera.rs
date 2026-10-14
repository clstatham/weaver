//! Cameras: view and projection matrices, frustum tests, and the per-view camera uniform.
//!
//! A [`Camera`] lives on a main-world entity. Extraction (see [`crate::view`]) turns an active one
//! into an [`ExtractedView`] on the render-world mirror, and `prepare_camera_uniforms` uploads
//! its matrices every frame into a [`CameraBindGroup`] that pipelines bind as the camera group.

use std::fmt::Debug;

use encase::ShaderType;
use weaver_core::geometry::{Aabb, Frustum, Intersection, Ray, Sphere};
use weaver_util::prelude::*;

use weaver_app::plugin::Plugin;
use weaver_ecs::prelude::*;

use crate::{
    RenderStage, WgpuDevice, WgpuQueue,
    bind_group::{AsBindGroup, BindGroupLayoutBuilder, BindingsBuilder, ComponentBindGroupPlugin},
    buffer::GpuBufferVec,
    extract::{ExtractComponent, ExtractComponentPlugin},
    view::ExtractedView,
};

/// Marks the camera whose view is tonemapped onto the window. Other active cameras still render,
/// but only into their own [`ViewTarget`](crate::view::ViewTarget).
#[derive(Clone, Copy)]
pub struct PrimaryCamera;

impl ExtractComponent for PrimaryCamera {
    type Data = ();
    type Filter = Changed<&'static Self>;
    type Out = Self;

    fn extract(_: ()) -> Option<Self::Out> {
        Some(PrimaryCamera)
    }
}

/// The camera data shaders see. Mirrors `CameraUniform` in `assets/shaders/utils.wesl`.
#[derive(Debug, Clone, Copy, ShaderType)]
#[repr(C)]
pub struct CameraUniform {
    /// World to view space.
    pub view: glam::Mat4,
    /// View to clip space.
    pub proj: glam::Mat4,
    /// Inverse of [`view`](Self::view).
    pub inv_view: glam::Mat4,
    /// Inverse of [`proj`](Self::proj).
    pub inv_proj: glam::Mat4,
    /// The camera's world-space position (translation of `inv_view`).
    pub camera_position: glam::Vec3,
    /// Pads the struct out to a 16-byte multiple.
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

/// A view matrix, a projection matrix and the frustum derived from them.
///
/// The frustum is recomputed by every setter that changes a matrix, so it never goes stale. Only
/// active cameras are extracted into [`ExtractedView`]s.
#[derive(Clone, Copy)]
pub struct Camera {
    /// Whether the camera is extracted and rendered.
    pub active: bool,
    view_matrix: glam::Mat4,
    projection_matrix: glam::Mat4,
    frustum: Frustum,
    frustum_bounding_sphere: Sphere,
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
    /// Creates an active camera at `eye` looking at `center`, with a right-handed perspective
    /// projection.
    ///
    /// `fov` is the vertical field of view in radians.
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
        // FIXME: perspective_rh_gl maps depth to [-1, 1] but wgpu clips to [0, 1], so the near half
        // of the range is lost.
        let proj = glam::Mat4::perspective_rh_gl(fov, aspect, near, far);
        let frustum = Frustum::from_view_proj(proj * view);
        let frustum_bounding_sphere = frustum.bounding_sphere();
        Self {
            active: true,
            view_matrix: view,
            projection_matrix: proj,
            frustum,
            frustum_bounding_sphere,
        }
    }

    /// Returns whether the camera is active.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Sets whether the camera is active.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    /// Makes the camera active.
    pub fn activate(&mut self) {
        self.set_active(true);
    }

    /// Makes the camera inactive. Its [`ExtractedView`] is removed on the next extract.
    pub fn deactivate(&mut self) {
        self.set_active(false);
    }

    /// Returns the world-to-view matrix.
    pub fn view_matrix(&self) -> glam::Mat4 {
        self.view_matrix
    }

    /// Sets the world-to-view matrix and recomputes the frustum.
    pub fn set_view_matrix(&mut self, view_matrix: glam::Mat4) {
        self.view_matrix = view_matrix;
        self.frustum = Frustum::from_view_proj(self.projection_matrix * self.view_matrix);
        self.frustum_bounding_sphere = self.frustum.bounding_sphere();
    }

    /// Returns the view-to-clip matrix.
    pub fn projection_matrix(&self) -> glam::Mat4 {
        self.projection_matrix
    }

    /// Sets the view-to-clip matrix and recomputes the frustum.
    pub fn set_projection_matrix(&mut self, projection_matrix: glam::Mat4) {
        self.projection_matrix = projection_matrix;
        self.frustum = Frustum::from_view_proj(self.projection_matrix * self.view_matrix);
        self.frustum_bounding_sphere = self.frustum.bounding_sphere();
    }

    /// Returns `projection * view`, the world-to-clip matrix.
    pub fn view_projection_matrix(&self) -> glam::Mat4 {
        self.projection_matrix * self.view_matrix
    }

    /// Sets both matrices at once, recomputing the frustum only once.
    pub fn set_view_projection_matrix(
        &mut self,
        view_matrix: glam::Mat4,
        projection_matrix: glam::Mat4,
    ) {
        self.view_matrix = view_matrix;
        self.projection_matrix = projection_matrix;
        self.frustum = Frustum::from_view_proj(self.projection_matrix * self.view_matrix);
        self.frustum_bounding_sphere = self.frustum.bounding_sphere();
    }

    /// Returns the world-space ray from the camera through a pixel.
    ///
    /// `screen_pos` is in pixels with the origin at the top left, and `screen_size` is the size of
    /// the window (or viewport) in pixels.
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

    /// Projects a world-space point to pixel coordinates (origin at the top left).
    ///
    /// Returns `None` if the point can't be shown on screen.
    pub fn world_to_screen(
        &self,
        world_pos: glam::Vec3,
        screen_size: glam::Vec2,
    ) -> Option<glam::Vec2> {
        let clip_from_world = self.projection_matrix * self.view_matrix;
        let ndc = clip_from_world.project_point3(world_pos);
        // FIXME: never returns None, so points behind the camera project to a bogus screen
        // position.
        let mut screen = (ndc.truncate() + glam::Vec2::ONE) / 2.0 * screen_size;
        screen.y = screen_size.y - screen.y;
        Some(screen)
    }

    /// Tests a world-space sphere against the frustum.
    ///
    /// The near and far planes are only checked if `intersect_near` / `intersect_far` are set.
    /// Returns [`Intersection::Outside`] or [`Intersection::Inside`] (spheres that straddle a plane
    /// count as inside).
    pub fn intersect_frustum_with_sphere(
        &self,
        sphere: Sphere,
        intersect_near: bool,
        intersect_far: bool,
    ) -> Intersection {
        for (i, plane) in self.frustum.planes().iter().enumerate() {
            if i == 4 && !intersect_near {
                continue;
            }
            if i == 5 && !intersect_far {
                continue;
            }
            let distance = plane.normal.dot(sphere.center) + plane.distance;
            if distance < -sphere.radius {
                return Intersection::Outside;
            }
        }

        Intersection::Inside
    }

    /// Tests a world-space box against the frustum, using its bounding sphere as a quick reject
    /// first.
    ///
    /// `intersect_near` and `intersect_far` work as in [`Camera::intersect_frustum_with_sphere`].
    /// Boxes that straddle a plane are reported as [`Intersection::Inside`].
    pub fn intersect_frustum_with_aabb(
        &self,
        aabb: &Aabb,
        intersect_near: bool,
        intersect_far: bool,
    ) -> Intersection {
        if self.intersect_frustum_with_sphere(aabb.bounding_sphere(), intersect_near, intersect_far)
            == Intersection::Outside
        {
            return Intersection::Outside;
        }

        for (i, plane) in self.frustum.planes().iter().enumerate() {
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
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            active: true,
            view_matrix: glam::Mat4::IDENTITY,
            projection_matrix: glam::Mat4::IDENTITY,
            frustum: Frustum::default(),
            frustum_bounding_sphere: Sphere::default(),
        }
    }
}

/// A view's camera uniform, bound as the camera bind group of most pipelines.
pub struct CameraBindGroup {
    /// Holds exactly one [`CameraUniform`].
    pub buffer: GpuBufferVec<CameraUniform>,
}

impl AsBindGroup for CameraBindGroup {
    const LABEL: &'static str = "Camera";

    fn layout(layout: &mut BindGroupLayoutBuilder) {
        layout.uniform_buffer(
            wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
        );
    }

    fn bindings<'a>(&'a self, bindings: &mut BindingsBuilder<'a>) {
        bindings.buffer_vec(&self.buffer);
    }
}

/// Registers camera extraction and the per-frame camera uniform upload.
///
/// [`Camera`] extraction itself is registered by [`crate::view::ViewPlugin`].
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut weaver_app::App) -> Result<()> {
        app.add_plugin(ComponentBindGroupPlugin::<CameraBindGroup>::default())?;
        app.add_plugin(ExtractComponentPlugin::<PrimaryCamera>::default())?;

        app.add_system(prepare_camera_uniforms, RenderStage::Prepare);

        Ok(())
    }
}

/// Uploads each view's camera matrices.
fn prepare_camera_uniforms(
    commands: Commands,
    mut query: Query<(Entity, &ExtractedView, Option<&mut CameraBindGroup>)>,
    device: Res<WgpuDevice>,
    queue: Res<WgpuQueue>,
) {
    for (entity, view, bind_group) in query.iter() {
        let camera_uniform = CameraUniform::from(&view.camera);
        if let Some(mut bind_group) = bind_group {
            bind_group.buffer.clear();
            bind_group.buffer.push(camera_uniform);
            bind_group.buffer.enqueue_update(&device, &queue);
        } else {
            let mut buffer =
                GpuBufferVec::new(wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST);
            buffer.push(camera_uniform);
            buffer.enqueue_update(&device, &queue);
            commands.insert_component(entity, CameraBindGroup { buffer });
        }
    }
}

use weaver_proc_macro::Resource;

use crate::core::camera::FlyCamera;

use super::Renderer;

#[derive(Debug, Clone, Copy, Default)]
pub struct PickResult {
    pub position: glam::Vec3,
}

#[derive(Resource)]
pub struct ScreenPicker {
    buffer: wgpu::Buffer,
}

impl ScreenPicker {
    pub fn new(renderer: &Renderer) -> Self {
        let depth_texture_size =
            renderer.depth_texture.texture().width() * renderer.depth_texture.texture().height();
        let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screen picker buffer"),
            size: depth_texture_size as u64 * std::mem::size_of::<f32>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self { buffer }
    }

    pub fn pick(
        &self,
        screen_pos: glam::Vec2,
        renderer: &Renderer,
        camera: &FlyCamera,
    ) -> anyhow::Result<Option<PickResult>> {
        let width = renderer.config.width as f32;
        let height = renderer.config.height as f32;

        if screen_pos.x < 0.0 || screen_pos.x > width || screen_pos.y < 0.0 || screen_pos.y > height
        {
            return Ok(None);
        }

        let mut encoder = renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        encoder.copy_texture_to_buffer(
            renderer.depth_texture.texture().as_image_copy(),
            wgpu::ImageCopyBuffer {
                buffer: &self.buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(
                        std::mem::size_of::<f32>() as u32
                            * renderer.depth_texture.texture().width(),
                    ),
                    rows_per_image: Some(renderer.depth_texture.texture().height()),
                },
            },
            wgpu::Extent3d {
                width: renderer.depth_texture.texture().width(),
                height: renderer.depth_texture.texture().height(),
                depth_or_array_layers: 1,
            },
        );

        renderer.queue.submit(std::iter::once(encoder.finish()));

        let (tx, rx) = futures_channel::oneshot::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| {
                tx.send(result).unwrap();
            });
        renderer.device.poll(wgpu::Maintain::Wait);
        pollster::block_on(rx)??;

        let pick_result = {
            let data: &[u8] = &self.buffer.slice(..).get_mapped_range();
            let depth_buffer: &[f32] = bytemuck::cast_slice(data);

            let mut depth = depth_buffer
                [screen_pos.y as usize * renderer.config.width as usize + screen_pos.x as usize]
                as f32;

            let x = screen_pos.x / width * 2.0 - 1.0;
            let y = -(screen_pos.y / height * 2.0 - 1.0);

            let inv_view_proj = (camera.projection_matrix() * camera.view_matrix()).inverse();

            depth = depth * 2.0 - 1.0;

            let ndc = glam::Vec4::new(x, y, depth, 1.0);

            let mut position = inv_view_proj * ndc;
            position /= position.w;

            Some(PickResult {
                position: position.truncate(),
            })
        };

        self.buffer.unmap();

        Ok(pick_result)
    }
}

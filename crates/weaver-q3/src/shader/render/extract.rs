use weaver_asset::{prelude::Asset, Assets, Handle};
use weaver_core::texture::Texture;
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::SystemParamItem,
};
use weaver_renderer::{
    asset::RenderAsset,
    bind_group::{BindGroup, BindGroupLayoutCache, CreateBindGroup},
    extract::Extract,
    prelude::wgpu,
    texture::{texture_format, GpuTexture},
};
use weaver_util::prelude::Result;

use crate::shader::loader::LoadedShader;

use super::{KeyedShaderStage, ShaderStagePipelineKey};

#[derive(Asset)]
pub struct ExtractedShader {
    pub name: String,
    pub shader: LoadedShader,
    pub stages: Vec<KeyedShaderStage>,
}

impl RenderAsset for ExtractedShader {
    type Param = (
        Extract<'static, 'static, Res<'static, Assets<Texture>>>,
        ResMut<'static, BindGroupLayoutCache>,
        ResMut<'static, Assets<BindGroup<KeyedShaderStage>>>,
    );
    type Source = LoadedShader;

    fn extract_render_asset(
        source: &Self::Source,
        (textures, bind_group_layout_cache, bind_group_assets): &mut SystemParamItem<Self::Param>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let mut stages = Vec::new();

        for stage in source.shader.stages.iter() {
            let key = ShaderStagePipelineKey::new(&source.shader, stage, source.topology);

            let texture = if let Some(texture) = stage.texture_map() {
                let texture = source.textures.get(&texture).unwrap();
                let texture = textures.get(*texture).unwrap();
                texture.clone()
            } else {
                Texture::from_rgba8(&[255, 0, 0, 255], 1, 1)
            };

            let texture =
                GpuTexture::from_image(device, queue, &texture, texture_format::SDR_FORMAT)
                    .unwrap();

            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });

            let mut extracted_stage = KeyedShaderStage {
                key,
                texture,
                sampler,
                bind_group: Handle::INVALID,
            };

            let bind_group = BindGroup::new(device, &extracted_stage, bind_group_layout_cache);
            let bind_group = bind_group_assets.insert(bind_group);
            extracted_stage.bind_group = bind_group;

            stages.push(extracted_stage);
        }

        Some(Self {
            name: source.shader.name.clone(),
            shader: source.clone(),
            stages,
        })
    }

    fn update_render_asset(
        &mut self,
        _source: &Self::Source,
        _param: &mut SystemParamItem<Self::Param>,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
    ) -> Result<()>
    where
        Self: Sized,
    {
        // todo
        Ok(())
    }
}

impl CreateBindGroup for KeyedShaderStage {
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout
    where
        Self: Sized,
    {
        let texture_binding = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let sampler_binding = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let layout = wgpu::BindGroupLayoutDescriptor {
            entries: &[texture_binding, sampler_binding],
            label: Some("shader_stage_bind_group_layout"),
        };

        device.create_bind_group_layout(&layout)
    }

    fn create_bind_group(
        &self,
        device: &wgpu::Device,
        cached_layout: &weaver_renderer::bind_group::BindGroupLayout,
    ) -> wgpu::BindGroup {
        let texture_binding = wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&self.texture.view),
        };

        let sampler_binding = wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&self.sampler),
        };

        let entries = [texture_binding, sampler_binding];

        let descriptor = wgpu::BindGroupDescriptor {
            layout: cached_layout,
            entries: &entries,
            label: Some("shader_stage_bind_group"),
        };

        device.create_bind_group(&descriptor)
    }
}

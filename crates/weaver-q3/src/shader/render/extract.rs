use std::num::NonZeroU32;

use weaver_asset::{prelude::Asset, Assets};
use weaver_core::texture::Texture;
use weaver_ecs::{
    component::{Res, ResMut},
    prelude::SystemParamItem,
};
use weaver_renderer::{
    asset::RenderAsset,
    bind_group::BindGroupLayoutCache,
    extract::Extract,
    prelude::wgpu,
    texture::{texture_format, GpuTexture},
};
use weaver_util::prelude::Result;

use crate::shader::loader::LoadedShader;

use super::{KeyedShaderStage, KeyedShaderStagePipelineCache, ShaderStagePipelineKey};

pub const SHADER_TEXTURE_ARRAY_SIZE: u32 = 5;

#[derive(Asset)]
pub struct ExtractedShader {
    pub name: String,
    pub shader: LoadedShader,
    pub stages: Vec<KeyedShaderStage>,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
    pub dummy_texture: GpuTexture,
}

impl RenderAsset for ExtractedShader {
    type Param = (
        Extract<'static, 'static, Res<'static, Assets<Texture>>>,
        ResMut<'static, KeyedShaderStagePipelineCache>,
        ResMut<'static, BindGroupLayoutCache>,
    );
    type Source = LoadedShader;

    fn extract_render_asset(
        source: &Self::Source,
        (textures, pipeline_cache, bind_group_layout_cache): &mut SystemParamItem<Self::Param>,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Option<Self>
    where
        Self: Sized,
    {
        let mut stages = Vec::new();

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let dummy_texture = Texture::from_rgba8(&[255, 0, 255, 255], 1, 1);

        let dummy_texture =
            GpuTexture::from_image(device, queue, &dummy_texture, texture_format::SDR_FORMAT)
                .unwrap();

        for stage in source.shader.stages.iter() {
            let key = ShaderStagePipelineKey::new(&source.shader, stage);

            let texture = if let Some(texture) = stage.texture_map() {
                let texture = source.textures.get(&texture).unwrap();
                let texture = textures.get(*texture).unwrap();
                GpuTexture::from_image(device, queue, &texture, texture_format::SDR_FORMAT).unwrap()
            } else {
                dummy_texture.clone()
            };

            let extracted_stage = KeyedShaderStage { key, texture };

            stages.push(extracted_stage);
        }

        let texture_binding = wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: NonZeroU32::new(SHADER_TEXTURE_ARRAY_SIZE),
        };

        let sampler_binding = wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let layout = wgpu::BindGroupLayoutDescriptor {
            entries: &[texture_binding, sampler_binding],
            label: Some("BSP Shader Bind Group Layout"),
        };

        let bind_group_layout = device.create_bind_group_layout(&layout);

        let mut views = stages
            .iter()
            .map(|stage| &*stage.texture.view)
            .collect::<Vec<_>>();
        if views.len() > SHADER_TEXTURE_ARRAY_SIZE as usize {
            panic!(
                "Too many textures in shader {}: {}",
                source.shader.name,
                views.len()
            );
        }
        if views.is_empty() {
            views.push(&dummy_texture.view);
        }
        if views.len() < SHADER_TEXTURE_ARRAY_SIZE as usize {
            for _ in views.len()..SHADER_TEXTURE_ARRAY_SIZE as usize {
                views.push(&dummy_texture.view);
            }
        }
        let texture_binding = wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureViewArray(&views),
        };

        let sampler_binding = wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Sampler(&sampler),
        };

        let entries = [texture_binding, sampler_binding];

        let descriptor = wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &entries,
            label: Some("shader_stage_bind_group"),
        };

        let bind_group = device.create_bind_group(&descriptor);

        for stage in &stages {
            pipeline_cache.get_or_init(
                stage.key,
                device,
                bind_group_layout_cache,
                &bind_group_layout,
            );
        }

        Some(Self {
            name: source.shader.name.clone(),
            shader: source.clone(),
            stages,
            sampler,
            bind_group_layout,
            bind_group,
            dummy_texture,
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

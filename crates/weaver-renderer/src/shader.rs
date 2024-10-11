use std::{
    borrow::Cow,
    io::Read,
    path::{Path, PathBuf},
};

use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};
use weaver_asset::{
    prelude::{Asset, Loader},
    AssetLoadQueues, Filesystem, LoadSource,
};
use weaver_ecs::prelude::Resource;
use weaver_util::{bail, Result};

#[derive(Debug, Clone, Asset)]
pub struct Shader {
    pub path: PathBuf,
    pub module: wgpu::ShaderSource<'static>,
}

impl Shader {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let module = preprocess_shader(path.to_str().unwrap(), "assets/shaders/");
        Self {
            path: path.into(),
            module,
        }
    }

    pub fn create_shader_module(&self, device: &wgpu::Device) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(self.path.to_str().unwrap()),
            source: self.module.clone(),
        })
    }
}

#[derive(Resource, Default)]
pub struct ShaderLoader;

impl Loader<Shader> for ShaderLoader {
    fn load(
        &self,
        source: LoadSource,
        _fs: &Filesystem,
        _load_queues: &AssetLoadQueues<'_>,
    ) -> Result<Shader> {
        Ok(Shader::new(source.as_url().unwrap().path()))
    }
}

fn try_every_shader_file(
    composer: &mut Composer,
    for_shader: &str,
    shader_dir: &str,
    max_iters: usize,
) -> Result<()> {
    let mut try_again = true;
    let mut iters = 0;
    while try_again {
        try_again = false;
        let shader_dir = std::fs::read_dir(shader_dir)?;
        for entry in shader_dir {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if path.extension().unwrap() != "wgsl" {
                    continue;
                }
                if path.to_str().unwrap() == for_shader {
                    continue;
                }

                let mut file = std::fs::File::open(&path)?;
                let mut shader = String::new();

                file.read_to_string(&mut shader)?;

                if composer
                    .add_composable_module(ComposableModuleDescriptor {
                        file_path: path.to_str().unwrap(),
                        source: shader.as_str(),
                        ..Default::default()
                    })
                    .is_err()
                {
                    try_again = true;
                }
            } else if path.is_dir() {
                try_every_shader_file(composer, for_shader, path.to_str().unwrap(), max_iters)?;
            }
        }

        iters += 1;

        if iters > max_iters {
            bail!("Max iterations reached");
        }
    }

    Ok(())
}

pub fn preprocess_shader(
    file_path: &str,
    base_include_path: &'static str,
) -> wgpu::ShaderSource<'static> {
    let mut composer = Composer::non_validating();

    let shader = std::fs::read_to_string(file_path).unwrap();

    try_every_shader_file(&mut composer, file_path, base_include_path, 100).unwrap();

    let module = composer
        .make_naga_module(NagaModuleDescriptor {
            file_path,
            source: shader.as_str(),
            ..Default::default()
        })
        .unwrap_or_else(|e| {
            log::error!("Failed to compile shader {}: {}", file_path, e.inner);
            panic!("{}", e.inner);
        });

    wgpu::ShaderSource::Naga(Cow::Owned(module))
}

use weaver_asset::{prelude::Asset, AssetLoadQueues, Filesystem, Handle, LoadSource, Loader};
use weaver_core::texture::{Texture, TextureLoader};
use weaver_ecs::prelude::Resource;
use weaver_pbr::material::ERROR_TEXTURE;
use weaver_util::{anyhow, FxHashMap, Result};

use crate::shader::parser::parse_shaders_manual;

use super::lexer::{LexedShader, LexedShaderStage, Map, ShaderStageParam};

pub fn make_error_shader(name: &str) -> LoadedShader {
    LoadedShader {
        shader: LexedShader {
            name: name.to_string(),
            global_params: vec![],
            stages: vec![LexedShaderStage {
                params: vec![ShaderStageParam::Map(Map::Path(
                    "textures/error".to_string(),
                ))],
            }],
        },
        textures: FxHashMap::from_iter([(Map::Path("textures/error".to_string()), ERROR_TEXTURE)]),
    }
}

pub const ERROR_SHADER_HANDLE: Handle<LoadedShader> =
    Handle::from_uuid(330543239317182820506064093680982255445);

#[inline]
pub fn strip_extension(path: &str) -> &str {
    let mut path = path;
    if let Some(pos) = path.rfind('.') {
        path = &path[..pos];
    }
    path
}

#[derive(Resource, Default)]
pub struct TextureCache(pub FxHashMap<String, Handle<Texture>>);

impl TextureCache {
    pub fn get(&self, name: &str) -> Option<Handle<Texture>> {
        self.0.get(name).copied()
    }

    pub fn insert(&mut self, name: String, handle: Handle<Texture>) {
        self.0.insert(name, handle);
    }
}

#[derive(Resource, Default)]
pub struct LexedShaderCache(pub FxHashMap<String, LexedShader>);

impl LexedShaderCache {
    pub fn get(&self, name: &str) -> Option<&LexedShader> {
        self.0.get(name)
    }

    pub fn insert(&mut self, name: String, shader: LexedShader) {
        self.0.insert(name, shader);
    }

    pub fn shader_names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn load_all(&mut self, dirname: &str, fs: &Filesystem) -> Result<()> {
        for path in fs.read_dir(dirname)? {
            if path.is_dir() {
                continue;
            }
            if path.extension().map_or(true, |ext| ext != "shader") {
                continue;
            }

            let shader = fs.read_sub_path(&path)?;
            let shader = String::from_utf8(shader).unwrap();
            let parsed = parse_shaders_manual(&shader);

            for shader in parsed {
                let shader_name = shader.name.clone();
                let shader = shader.lex();
                self.insert(shader_name, shader);
            }
        }

        Ok(())
    }
}

#[derive(Resource, Default)]
pub struct LoadedShaderCache(pub FxHashMap<String, Handle<LoadedShader>>);

impl LoadedShaderCache {
    pub fn get(&self, name: &str) -> Option<Handle<LoadedShader>> {
        self.0.get(name).copied()
    }

    pub fn insert(&mut self, name: String, handle: Handle<LoadedShader>) {
        self.0.insert(name, handle);
    }

    pub fn shader_names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

#[derive(Asset, Clone)]
pub struct LoadedShader {
    pub shader: LexedShader,
    pub textures: FxHashMap<Map, Handle<Texture>>,
}

impl LoadedShader {
    pub fn make_simple_textured(texture: Handle<Texture>, texture_name: &str) -> Self {
        let shader = LexedShader {
            name: texture_name.to_string(),
            global_params: vec![],
            stages: vec![LexedShaderStage {
                params: vec![ShaderStageParam::Map(Map::Path(texture_name.to_string()))],
            }],
        };

        let mut textures = FxHashMap::default();
        textures.insert(Map::Path(texture_name.to_string()), texture);

        Self { shader, textures }
    }
}

#[derive(Resource, Default)]
pub struct TryEverythingTextureLoader;

impl Loader<Texture> for TryEverythingTextureLoader {
    fn load(
        &self,
        source: LoadSource,
        fs: &Filesystem,
        load_queues: &AssetLoadQueues<'_>,
    ) -> Result<Texture> {
        let path = source.as_path().unwrap();

        let extensions = ["png", "tga", "jpg", "jpeg", "pcx", "bmp"];
        for ext in &extensions {
            let path = path.with_extension(ext);
            if let Ok(texture) = TextureLoader.load(LoadSource::Path(path), fs, load_queues) {
                return Ok(texture);
            }
        }

        Err(anyhow!("Failed to load texture: {:?}", path))
    }
}

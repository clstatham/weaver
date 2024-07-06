use weaver_asset::{
    loading::{Filesystem, LoadCtx, Loader},
    prelude::Asset,
    Assets, Handle,
};
use weaver_core::texture::{Texture, TextureLoader};
use weaver_ecs::prelude::Resource;
use weaver_pbr::material::{ERROR_TEXTURE, WHITE_TEXTURE};
use weaver_util::{
    prelude::{anyhow, FxHashMap, Result},
    warn_once,
};

use crate::shader::{lexer::LexedShaderGlobalParam, parser::parse_shaders_manual};

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

    pub fn load_all(&mut self, dirname: &str, fs: &mut Filesystem) -> Result<()> {
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

    pub fn load_from_lexed(shader: LexedShader, load_ctx: &mut LoadCtx) -> Self {
        let mut textures = FxHashMap::default();

        let mut texture_cache = load_ctx.get_resource_mut::<TextureCache>().unwrap();

        for param in &shader.global_params {
            if let LexedShaderGlobalParam::EditorImage(path) = param {
                let stripped = strip_extension(path);
                let map = Map::Path(stripped.to_string());
                if textures.contains_key(&map) {
                    continue;
                }
                if let Some(handle) = texture_cache.get(stripped) {
                    textures.insert(map, handle);
                    continue;
                }
                let handle = match load_ctx.load_asset::<_, TryEverythingTextureLoader>(stripped) {
                    Ok(texture) => {
                        log::debug!("Loaded texture: {}", stripped);
                        let mut texture_assets =
                            load_ctx.get_resource_mut::<Assets<Texture>>().unwrap();
                        let handle = texture_assets.insert(texture);
                        load_ctx.drop_resource_mut(texture_assets);
                        handle
                    }
                    Err(e) => {
                        warn_once!("Failed to load texture: {}", e);
                        ERROR_TEXTURE
                    }
                };
                texture_cache.insert(stripped.to_string(), handle);
                textures.insert(map, handle);
            }
        }

        for stage in &shader.stages {
            for directive in &stage.params {
                if let ShaderStageParam::Map(map) = directive {
                    match map {
                        Map::Path(path) => {
                            let stripped = strip_extension(path);
                            let map = Map::Path(stripped.to_string());
                            if textures.contains_key(&map) {
                                continue;
                            }
                            if let Some(handle) = texture_cache.get(stripped) {
                                textures.insert(map, handle);
                                continue;
                            }
                            let handle = match load_ctx
                                .load_asset::<_, TryEverythingTextureLoader>(stripped)
                            {
                                Ok(texture) => {
                                    log::debug!("Loaded texture: {}", stripped);
                                    let mut texture_assets =
                                        load_ctx.get_resource_mut::<Assets<Texture>>().unwrap();
                                    let handle = texture_assets.insert(texture);
                                    load_ctx.drop_resource_mut(texture_assets);
                                    handle
                                }
                                Err(e) => {
                                    warn_once!("Failed to load texture: {}", e);
                                    ERROR_TEXTURE
                                }
                            };
                            texture_cache.insert(stripped.to_string(), handle);
                            textures.insert(map, handle);
                        }
                        Map::WhiteImage => {
                            textures.insert(Map::WhiteImage, WHITE_TEXTURE);
                        }
                        Map::Lightmap => {
                            textures.insert(Map::Lightmap, WHITE_TEXTURE);
                        }
                    }
                }
            }
        }

        load_ctx.drop_resource_mut(texture_cache);

        Self { shader, textures }
    }
}

#[derive(Resource, Default)]
pub struct TryEverythingTextureLoader;

impl Loader<Texture> for TryEverythingTextureLoader {
    fn load(&self, ctx: &mut LoadCtx<'_, '_>) -> Result<Texture> {
        let path = ctx.original_path().to_path_buf();

        let extensions = ["png", "tga", "jpg", "jpeg", "pcx", "bmp"];
        for ext in &extensions {
            let path = path.with_extension(ext);
            if let Ok(texture) = ctx.load_asset::<_, TextureLoader>(&path) {
                return Ok(texture);
            }
        }

        Err(anyhow!("Failed to load texture: {:?}", path))
    }
}

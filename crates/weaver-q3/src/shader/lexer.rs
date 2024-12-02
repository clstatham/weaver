use std::fmt::Display;

use weaver_asset::prelude::Asset;
use weaver_core::prelude::Vec3;
use weaver_util::debug_once;

use super::{loader::strip_extension, parser::*};

#[derive(Debug, Clone, PartialEq)]
pub enum LexedShaderGlobalParam {
    SurfaceParm(SurfaceParm),
    SkyParms(SkyParms),
    Cull(Cull),
    DeformVertexes(DeformVertexes),
    FogParms(FogParms),
    NoPicMip,
    NoMipMaps,
    PolygonOffset,
    Portal,
    Sort(Sort),
    Light(f32),
    // qer specific params
    TessSize(f32),
    EditorImage(String),
    Trans(f32),
    // q3map specific params
    SurfaceLight(f32),
    LightImage(Map),
    LightSubdivide(f32),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SurfaceParm {
    AlphaShadow,
    AreaPortal,
    ClusterPortal,
    DoNotEnter,
    Flesh,
    Fog,
    Lava,
    MetalSteps,
    NoDamage,
    NoDLight,
    NoDraw,
    NoDrop,
    NoImpact,
    NoMarks,
    NoLightMap,
    NoSteps,
    NonSolid,
    Origin,
    PlayerClip,
    Slick,
    Slime,
    Structural,
    Trans,
    Water,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Cull {
    Front,
    Back,
    Disable,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeformVertexes {
    Wave {
        div: f32,
        func: WaveFunc,
        base: f32,
        amp: f32,
        phase: f32,
        freq: f32,
    },
    Normal {
        amp: f32,
        freq: f32,
    },
    Bulge {
        width: f32,
        height: f32,
        speed: f32,
    },
    Move {
        x: f32,
        y: f32,
        z: f32,
        func: WaveFunc,
        base: f32,
        amp: f32,
        phase: f32,
        freq: f32,
    },
    AutoSprite,
    AutoSprite2,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WaveFunc {
    Sin,
    Triangle,
    Square,
    Sawtooth,
    InverseSawtooth,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkyParms {
    pub farbox: Option<String>,
    pub cloudheight: u8,
    pub nearbox: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FogParms {
    pub color: [f32; 3],
    pub distance_to_opaque: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
#[non_exhaustive]
pub enum Sort {
    Portal = 1,
    Sky = 2,
    Opaque = 3,
    Banner = 6,
    Underwater = 8,
    Additive = 9,
    Nearest = 16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderStageParam {
    Map(Map),
    ClampMap(String),
    AnimMap(AnimMap),
    BlendFunc(BlendFunc),
    RgbGen(RgbGen),
    AlphaGen(AlphaGen),
    TcGen(TcGen),
    TcMod(TcMod),
    DepthFunc(DepthFunc),
    DepthWrite,
    AlphaFunc(AlphaFunc),
    Detail,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Map {
    Path(String),
    Lightmap,
    WhiteImage,
}

impl Map {
    pub fn strip_extension(&self) -> Self {
        match self {
            Map::Path(path) => Map::Path(strip_extension(path).to_string()),
            _ => self.clone(),
        }
    }
}

impl Display for Map {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Map::Path(path) => write!(f, "{}", path),
            Map::Lightmap => write!(f, "$lightmap"),
            Map::WhiteImage => write!(f, "$whiteimage"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimMap {
    pub freq: f32,
    pub maps: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum BlendFunc {
    #[default]
    Add,
    Filter,
    Blend,
    Explicit(BlendFuncExplicit),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlendFuncExplicit {
    pub src: BlendFuncExplicitParam,
    pub dst: BlendFuncExplicitParam,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendFuncExplicitParam {
    One,
    Zero,
    SrcColor,
    DstColor,
    OneMinusSrcColor,
    OneMinusDstColor,
    SrcAlpha,
    DstAlpha,
    OneMinusSrcAlpha,
    OneMinusDstAlpha,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RgbGen {
    Identity,
    IdentityLighting,
    Wave {
        func: WaveFunc,
        base: f32,
        amp: f32,
        phase: f32,
        freq: f32,
    },
    Entity,
    OneMinusEntity,
    Vertex,
    OneMinusVertex,
    LightingDiffuse,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlphaGen {
    Identity,
    Wave {
        func: WaveFunc,
        base: f32,
        amp: f32,
        phase: f32,
        freq: f32,
    },
    Entity,
    OneMinusEntity,
    Vertex,
    OneMinusVertex,
    LightingDiffuse,
    Portal,
    LightingSpecular,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TcGen {
    Base,
    Environment,
    Lightmap,
    LightmapEnvironment,
    Vector { s: Vec3, t: Vec3 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TcMod {
    Rotate {
        degrees_per_second: f32,
    },
    Scale {
        s: f32,
        t: f32,
    },
    Scroll {
        s_speed: f32,
        t_speed: f32,
    },
    Stretch {
        func: WaveFunc,
        base: f32,
        amp: f32,
        phase: f32,
        freq: f32,
    },
    Transform {
        m00: f32,
        m01: f32,
        m10: f32,
        m11: f32,
        t0: f32,
        t1: f32,
    },
    Turb {
        base: f32,
        amp: f32,
        phase: f32,
        freq: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum DepthFunc {
    Equal,
    Lequal,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlphaFunc {
    Gt0,
    Lt128,
    Ge128,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sun {
    pub color: [f32; 3],
    pub intensity: f32,
    pub degrees: f32,
    pub elevation: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexedShaderStage {
    pub params: Vec<ShaderStageParam>,
}

impl LexedShaderStage {
    pub fn blend_func(&self) -> Option<&BlendFunc> {
        for param in self.params.iter() {
            if let ShaderStageParam::BlendFunc(blend_func) = param {
                return Some(blend_func);
            }
        }
        None
    }

    pub fn texture_map(&self) -> Option<Map> {
        for param in self.params.iter() {
            if let ShaderStageParam::Map(map) = param {
                return Some(map.strip_extension());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Asset, PartialEq)]
pub struct LexedShader {
    pub name: String,
    pub global_params: Vec<LexedShaderGlobalParam>,
    pub stages: Vec<LexedShaderStage>,
}

impl LexedShader {
    pub fn cull(&self) -> Cull {
        for param in self.global_params.iter() {
            if let LexedShaderGlobalParam::Cull(cull) = param {
                return *cull;
            }
        }
        Cull::Back
    }
}

impl ParsedDirectiveArg {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            ParsedDirectiveArg::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_path(&self) -> Option<&str> {
        match self {
            ParsedDirectiveArg::Path(path) => Some(path),
            _ => None,
        }
    }

    pub fn as_ident(&self) -> Option<&str> {
        match self {
            ParsedDirectiveArg::Ident(ident) => Some(ident),
            _ => None,
        }
    }

    pub fn as_surface_parm(&self) -> Option<SurfaceParm> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "alphashadow" => Some(SurfaceParm::AlphaShadow),
                "areaportal" => Some(SurfaceParm::AreaPortal),
                "clusterportal" => Some(SurfaceParm::ClusterPortal),
                "flesh" => Some(SurfaceParm::Flesh),
                "fog" => Some(SurfaceParm::Fog),
                "lava" => Some(SurfaceParm::Lava),
                "metalsteps" => Some(SurfaceParm::MetalSteps),
                "nodamage" => Some(SurfaceParm::NoDamage),
                "nodlight" => Some(SurfaceParm::NoDLight),
                "nodraw" => Some(SurfaceParm::NoDraw),
                "nodrop" => Some(SurfaceParm::NoDrop),
                "noimpact" => Some(SurfaceParm::NoImpact),
                "nomarks" => Some(SurfaceParm::NoMarks),
                "nolightmap" => Some(SurfaceParm::NoLightMap),
                "nosteps" => Some(SurfaceParm::NoSteps),
                "origin" => Some(SurfaceParm::Origin),
                "playerclip" => Some(SurfaceParm::PlayerClip),
                "slick" => Some(SurfaceParm::Slick),
                "slime" => Some(SurfaceParm::Slime),
                "structural" => Some(SurfaceParm::Structural),
                "trans" => Some(SurfaceParm::Trans),
                "water" => Some(SurfaceParm::Water),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<Map> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "$lightmap" => Some(Map::Lightmap),
                "$whiteimage" => Some(Map::WhiteImage),
                _ => None,
            },
            ParsedDirectiveArg::Path(path) => Some(Map::Path(path.to_owned())),
            _ => None,
        }
    }

    pub fn as_blend_func_explicit_param(&self) -> Option<BlendFuncExplicitParam> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "gl_one" => Some(BlendFuncExplicitParam::One),
                "gl_zero" => Some(BlendFuncExplicitParam::Zero),
                "gl_src_color" => Some(BlendFuncExplicitParam::SrcColor),
                "gl_dst_color" => Some(BlendFuncExplicitParam::DstColor),
                "gl_one_minus_src_color" => Some(BlendFuncExplicitParam::OneMinusSrcColor),
                "gl_one_minus_dst_color" => Some(BlendFuncExplicitParam::OneMinusDstColor),
                "gl_src_alpha" => Some(BlendFuncExplicitParam::SrcAlpha),
                "gl_dst_alpha" => Some(BlendFuncExplicitParam::DstAlpha),
                "gl_one_minus_src_alpha" => Some(BlendFuncExplicitParam::OneMinusSrcAlpha),
                "gl_one_minus_dst_alpha" => Some(BlendFuncExplicitParam::OneMinusDstAlpha),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_wave_func(&self) -> Option<WaveFunc> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "sin" => Some(WaveFunc::Sin),
                "triangle" => Some(WaveFunc::Triangle),
                "square" => Some(WaveFunc::Square),
                "sawtooth" => Some(WaveFunc::Sawtooth),
                "inversesawtooth" => Some(WaveFunc::InverseSawtooth),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_sort(&self) -> Option<Sort> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "portal" => Some(Sort::Portal),
                "sky" => Some(Sort::Sky),
                "opaque" => Some(Sort::Opaque),
                "banner" => Some(Sort::Banner),
                "underwater" => Some(Sort::Underwater),
                "additive" => Some(Sort::Additive),
                "nearest" => Some(Sort::Nearest),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_depth_func(&self) -> Option<DepthFunc> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "equal" => Some(DepthFunc::Equal),
                "lequal" => Some(DepthFunc::Lequal),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_alpha_func(&self) -> Option<AlphaFunc> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "gt0" => Some(AlphaFunc::Gt0),
                "lt128" => Some(AlphaFunc::Lt128),
                "ge128" => Some(AlphaFunc::Ge128),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_cull(&self) -> Option<Cull> {
        match self {
            ParsedDirectiveArg::Ident(ident) => match ident.to_ascii_lowercase().as_str() {
                "front" => Some(Cull::Front),
                "back" => Some(Cull::Back),
                _ => None,
            },
            _ => None,
        }
    }
}

impl ParsedDirective {
    pub fn as_global_param(&self) -> Option<LexedShaderGlobalParam> {
        match self.name.to_ascii_lowercase().as_str() {
            "nopicmip" => Some(LexedShaderGlobalParam::NoPicMip),
            "nomipmaps" => Some(LexedShaderGlobalParam::NoMipMaps),
            "polygonoffset" => Some(LexedShaderGlobalParam::PolygonOffset),
            "portal" => Some(LexedShaderGlobalParam::Portal),
            "surfaceparm" => {
                let arg = self.args.first()?;
                arg.as_surface_parm()
                    .map(LexedShaderGlobalParam::SurfaceParm)
            }
            "cull" => {
                let arg = self.args.first()?;
                arg.as_cull().map(LexedShaderGlobalParam::Cull)
            }
            "qer_editorimage" => {
                let arg = self.args.first()?;
                arg.as_path()
                    .map(|path| LexedShaderGlobalParam::EditorImage(path.to_owned()))
            }
            _ => None,
        }
    }

    pub fn as_stage_param(&self) -> Option<ShaderStageParam> {
        match self.name.to_ascii_lowercase().as_str() {
            "detail" => Some(ShaderStageParam::Detail),
            "depthwrite" => Some(ShaderStageParam::DepthWrite),
            "map" => {
                if let Some(arg) = self.args.first() {
                    arg.as_map().map(ShaderStageParam::Map)
                } else {
                    None
                }
            }
            "blendfunc" => {
                if let Some(arg1) = self.args.first() {
                    if let Some(arg2) = self.args.get(1) {
                        let arg1 = arg1.as_blend_func_explicit_param()?;
                        let arg2 = arg2.as_blend_func_explicit_param()?;
                        Some(ShaderStageParam::BlendFunc(BlendFunc::Explicit(
                            BlendFuncExplicit {
                                src: arg1,
                                dst: arg2,
                            },
                        )))
                    } else {
                        let arg1 = arg1.as_ident()?;
                        match arg1.to_ascii_lowercase().as_str() {
                            "add" => Some(ShaderStageParam::BlendFunc(BlendFunc::Add)),
                            "filter" => Some(ShaderStageParam::BlendFunc(BlendFunc::Filter)),
                            "blend" => Some(ShaderStageParam::BlendFunc(BlendFunc::Blend)),
                            _ => None,
                        }
                    }
                } else {
                    None
                }
            }
            "depthfunc" => {
                if let Some(arg) = self.args.first() {
                    arg.as_depth_func().map(ShaderStageParam::DepthFunc)
                } else {
                    None
                }
            }
            "alphafunc" => {
                if let Some(arg) = self.args.first() {
                    arg.as_alpha_func().map(ShaderStageParam::AlphaFunc)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl ParsedShader {
    pub fn lex(&self) -> LexedShader {
        let mut global_params = Vec::new();
        let mut stages = Vec::new();

        for global in self.globals.iter() {
            if let Some(param) = global.as_global_param() {
                global_params.push(param);
            } else {
                debug_once!("Unknown global directive: `{}`", global.name);
            }
        }

        for stage in self.stages.iter() {
            let mut shader_stage = LexedShaderStage { params: Vec::new() };
            for param in stage.directives.iter() {
                if let Some(param) = param.as_stage_param() {
                    shader_stage.params.push(param);
                } else {
                    debug_once!("Unknown stage directive: `{}`", param.name);
                }
            }
            stages.push(shader_stage);
        }

        LexedShader {
            name: self.name.clone(),
            global_params,
            stages,
        }
    }
}

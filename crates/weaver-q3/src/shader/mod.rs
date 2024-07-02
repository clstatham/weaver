use std::fmt::Display;

use weaver_core::prelude::Vec3;

pub mod lexer;
pub mod parser;

#[derive(Debug, Clone, PartialEq)]
pub enum ShaderGlobalParam {
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum Map {
    Path(String),
    Lightmap,
    WhiteImage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnimMap {
    pub freq: f32,
    pub maps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlendFunc {
    Add,
    Filter,
    Blend,
    Explicit(BlendFuncExplicit),
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlendFuncExplicit {
    pub src: BlendFuncExplicitParam,
    pub dest: BlendFuncExplicitParam,
}

#[derive(Debug, Clone, PartialEq)]
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
pub struct ShaderStage {
    pub params: Vec<ShaderStageParam>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Shader {
    pub name: String,
    pub global_params: Vec<ShaderGlobalParam>,
    pub stages: Vec<ShaderStage>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParsedDirectiveArg {
    Float(f32),
    Ident(String),
    Path(String),
    Parens(Vec<ParsedDirectiveArg>),
}

impl Display for ParsedDirectiveArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParsedDirectiveArg::Float(x) => write!(f, "{}", x),
            ParsedDirectiveArg::Ident(x) => write!(f, "{}", x),
            ParsedDirectiveArg::Path(x) => write!(f, "{}", x),
            ParsedDirectiveArg::Parens(x) => {
                write!(f, "(")?;
                for (i, arg) in x.iter().enumerate() {
                    write!(f, "{}", arg)?;
                    if i < x.len() - 1 {
                        write!(f, ", ")?;
                    }
                }
                write!(f, ")")?;
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDirective {
    pub name: String,
    pub args: Vec<ParsedDirectiveArg>,
}

impl Display for ParsedDirective {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        for arg in &self.args {
            write!(f, " {}", arg)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedStage {
    pub directives: Vec<ParsedDirective>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedShader {
    pub name: String,
    pub globals: Vec<ParsedDirective>,
    pub stages: Vec<ParsedStage>,
}

#[cfg(test)]
mod tests {
    use crate::shader::parser::parse_shaders;

    #[test]
    #[rustfmt::skip]
    fn test_parse_shader_file() {
        let input = include_str!("../../../../assets/maps/test.shader");
        parse_shaders(input).unwrap();
    }
}

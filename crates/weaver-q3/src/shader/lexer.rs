use super::*;

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
    pub fn as_global_param(&self) -> Option<ShaderGlobalParam> {
        match self.name.to_ascii_lowercase().as_str() {
            "nopicmip" => Some(ShaderGlobalParam::NoPicMip),
            "nomipmaps" => Some(ShaderGlobalParam::NoMipMaps),
            "polygonoffset" => Some(ShaderGlobalParam::PolygonOffset),
            "portal" => Some(ShaderGlobalParam::Portal),
            "surfaceparm" => {
                let arg = self.args.first()?;
                arg.as_surface_parm().map(ShaderGlobalParam::SurfaceParm)
            }
            "cull" => {
                let arg = self.args.first()?;
                arg.as_cull().map(ShaderGlobalParam::Cull)
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
                                dest: arg2,
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
    pub fn lex(&self) -> Shader {
        let mut global_params = Vec::new();
        let mut stages = Vec::new();

        for global in self.globals.iter() {
            if let Some(param) = global.as_global_param() {
                global_params.push(param);
            } else {
                log::debug!("Unknown global directive: `{}`", global);
            }
        }

        for stage in self.stages.iter() {
            let mut shader_stage = ShaderStage { params: Vec::new() };
            for param in stage.directives.iter() {
                if let Some(param) = param.as_stage_param() {
                    shader_stage.params.push(param);
                } else {
                    log::debug!("Unknown stage directive: `{}`", param);
                }
            }
            stages.push(shader_stage);
        }

        Shader {
            name: self.name.clone(),
            global_params,
            stages,
        }
    }
}

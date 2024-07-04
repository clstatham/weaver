use std::fmt::Display;

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

pub type Span<'a> = &'a str;

pub fn take(input: Span, n: usize) -> Option<(Span, Span)> {
    if input.len() < n {
        return None;
    }
    Some((&input[n..], &input[..n]))
}

pub fn tag<'a>(input: Span<'a>, tag: &str) -> Option<Span<'a>> {
    if input
        .to_ascii_lowercase()
        .starts_with(&tag.to_ascii_lowercase())
    {
        Some(&input[tag.len()..])
    } else {
        None
    }
}

pub fn take_whitespace(mut input: Span) -> Span {
    while let Some(c) = input.chars().next() {
        if input.starts_with("//") {
            input = &input[input.find('\n').unwrap_or(input.len())..];
            continue;
        }
        if !c.is_whitespace() {
            break;
        }
        input = &input[1..];
    }
    input
}

pub fn parse_shaders_manual(mut input: Span) -> Vec<ParsedShader> {
    let mut shaders = vec![];
    loop {
        input = take_whitespace(input);
        if input.is_empty() {
            break;
        }
        let (rest, shader) = parse_shader_manual(input);
        if let Some(shader) = shader {
            shaders.push(shader);
        }
        input = rest;
    }
    shaders
}

pub fn parse_directive_manual(input: Span) -> Option<ParsedDirective> {
    let input = take_whitespace(input);
    let name = input.split_whitespace().next()?.to_string();
    let input = &input[name.len()..];
    let input = take_whitespace(input);
    let mut args = vec![];
    for raw_arg in input.split_whitespace() {
        if raw_arg.contains('/') {
            args.push(ParsedDirectiveArg::Path(raw_arg.to_string()));
            continue;
        } else {
            let arg = match raw_arg.parse::<f32>() {
                Ok(f) => ParsedDirectiveArg::Float(f),
                Err(_) => ParsedDirectiveArg::Ident(raw_arg.to_string()),
            };
            args.push(arg);
        }
    }
    Some(ParsedDirective { name, args })
}

pub fn parse_shader_manual(input: Span) -> (Span, Option<ParsedShader>) {
    let input = take_whitespace(input);
    let name = input.lines().next();
    let Some(name) = name else {
        return (input, None);
    };
    let name = name.trim();
    let name = name.split_whitespace().next();
    let Some(name) = name else {
        return (input, None);
    };
    let input = &input[name.len()..];
    let input = take_whitespace(input);
    let Some(input) = tag(input, "{") else {
        return (input, None);
    };
    let input = take_whitespace(input);
    let mut globals = vec![];
    let mut stages = vec![];
    let mut stage_directives = vec![];
    let mut in_stage = false;
    let mut length_read = 0;

    for line in input.split('\n') {
        length_read += line.len() + 1; // +1 for newline
        let line = line.trim();
        if line.is_empty() {
            // Empty line
        } else if line.starts_with("//") {
            // Comment
        } else if line.starts_with('}') {
            if in_stage {
                // End of stage
                stages.push(ParsedStage {
                    directives: std::mem::take(&mut stage_directives),
                });
                in_stage = false;
            } else {
                // End of shader
                break;
            }
        } else if line.starts_with('{') {
            // Start of stage
            in_stage = true;
        } else if in_stage {
            // Parse stage directive
            if let Some(directive) = parse_directive_manual(line) {
                stage_directives.push(directive);
            }
        } else if let Some(directive) = parse_directive_manual(line) {
            // Parse global directive
            globals.push(directive);
        }
    }

    // Skip the lines we've read
    length_read = length_read.saturating_sub(1); // Remove the last newline
    let input = &input[length_read..];

    (
        input,
        Some(ParsedShader {
            name: name.to_string(),
            globals,
            stages,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test shaders copied from quake 3 arena data files
    // Copyright id software, licensed under GNU GPL2

    #[test]
    #[rustfmt::skip]
    fn test_parse_shader() {
        let input = r#"
textures/dont_use/openwindow
{	
	
	surfaceparm nolightmap
	cull none
	
	{
	map textures/dont_use/openwindow.tga
	alphaMap openwindow.tga
	blendFunc GL_ONE_MINUS_SRC_ALPHA GL_SRC_ALPHA
	depthWrite
	}
}
        "#;
        let shaders = parse_shaders_manual(input);
        assert_eq!(shaders.len(), 1);
        let shader = &shaders[0];
        assert_eq!(shader.name, "textures/dont_use/openwindow");
        assert_eq!(shader.globals.len(), 2);
        assert_eq!(shader.stages.len(), 1);
        let stage = &shader.stages[0];
        assert_eq!(stage.directives.len(), 4);
    }

    #[test]
    fn test_parse_shaders() {
        let input = r#"
textures/common/nolightmap
{
	surfaceparm nolightmap
}

textures/common/nodrawnonsolid
{
	surfaceparm	nonsolid
	surfaceparm	nodraw
}

textures/common/invisible
{
	surfaceparm nolightmap			
        {
                map textures/common/invisible.tga
                alphaFunc GE128
		depthWrite
		rgbGen vertex
        }
}

textures/common/teleporter
{
	surfaceparm nolightmap
	surfaceparm noimpact
	q3map_lightimage textures/sfx/powerupshit.tga	
	q3map_surfacelight 800
	{
		map textures/sfx/powerupshit.tga
		tcGen environment
//		tcMod scale 5 5
		tcMod turb 0 0.015 0 0.3
	}
}
        "#;
        let shaders = parse_shaders_manual(input);
        assert_eq!(shaders.len(), 4);

        let shader = &shaders[0];
        assert_eq!(shader.name, "textures/common/nolightmap");
        assert_eq!(shader.globals.len(), 1);
        assert_eq!(shader.stages.len(), 0);

        let shader = &shaders[1];
        assert_eq!(shader.name, "textures/common/nodrawnonsolid");
        assert_eq!(shader.globals.len(), 2);
        assert_eq!(shader.stages.len(), 0);

        let shader = &shaders[2];
        assert_eq!(shader.name, "textures/common/invisible");
        assert_eq!(shader.globals.len(), 1);
        assert_eq!(shader.stages.len(), 1);
        let stage = &shader.stages[0];
        assert_eq!(stage.directives.len(), 4);

        let shader = &shaders[3];
        assert_eq!(shader.name, "textures/common/teleporter");
        assert_eq!(shader.globals.len(), 4);
        assert_eq!(shader.stages.len(), 1);
        let stage = &shader.stages[0];
        assert_eq!(stage.directives.len(), 3);
    }
}

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag_no_case},
    character::complete::{alphanumeric1, char, one_of},
    combinator::{all_consuming, map, opt, recognize, value},
    error::{context, convert_error, VerboseError},
    multi::{many0, many1},
    number::complete::float,
    sequence::{delimited, pair, preceded, tuple},
    Finish,
};

use weaver_util::prelude::{anyhow, Result};

use super::*;

pub type Span<'a> = &'a str;
pub type IResult<'a, I, O> = nom::IResult<I, O, VerboseError<Span<'a>>>;

pub fn hspace0(input: Span) -> IResult<Span, Span> {
    let (input, space) = recognize(many0(one_of(" \t")))(input)?;
    Ok((input, space))
}

pub fn hspace1(input: Span) -> IResult<Span, Span> {
    let (input, space) = recognize(many1(one_of(" \t")))(input)?;
    Ok((input, space))
}

pub fn newline(input: Span) -> IResult<Span, Span> {
    recognize(alt((
        tag_no_case("\r\n"),
        tag_no_case("\r"),
        tag_no_case("\n"),
    )))(input)
}

pub fn full_line<'a, O, F>(f: F) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, O>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
{
    delimited(hspace0, f, preceded(hspace0, newline))
}

pub fn many0_lines<'a, O, F>(f: F) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, Vec<O>>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
    O: Clone,
{
    map(
        many0(alt((
            map(full_line(f), Some),
            value(None, preceded(hspace0, newline)),
            value(None, parse_comment),
        ))),
        |o| o.into_iter().flatten().collect::<Vec<_>>(),
    )
}

pub fn many1_lines<'a, O, F>(f: F) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, Vec<O>>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
    O: Clone,
{
    map(
        many1(alt((
            map(full_line(f), Some),
            value(None, preceded(hspace0, newline)),
            value(None, parse_comment),
        ))),
        |o| o.into_iter().flatten().collect::<Vec<_>>(),
    )
}

pub fn parse_comment(input: Span) -> IResult<Span, ()> {
    let (input, _) = full_line(preceded(tag_no_case("//"), opt(is_not("\r\n"))))(input)?;
    Ok((input, ()))
}

pub fn hspace_or_comments(input: Span) -> IResult<Span, ()> {
    let (input, _) = many0(alt((parse_comment, value((), hspace1))))(input)?;
    Ok((input, ()))
}

pub fn space0(input: Span) -> IResult<Span, Span> {
    recognize(many0(one_of("\t \r\n")))(input)
}

pub fn space1(input: Span) -> IResult<Span, Span> {
    recognize(many1(one_of("\t \r\n")))(input)
}

pub fn space_or_comments(input: Span) -> IResult<Span, ()> {
    let (input, _) = many0(alt((parse_comment, value((), space1))))(input)?;
    Ok((input, ()))
}

pub fn hspace_delimited<'a, F, O>(f: F) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, O>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
{
    delimited(hspace_or_comments, f, hspace_or_comments)
}

pub fn space_delimited<'a, F, O>(f: F) -> impl FnMut(Span<'a>) -> IResult<Span<'a>, O>
where
    F: FnMut(Span<'a>) -> IResult<Span<'a>, O>,
{
    delimited(space_or_comments, f, space_or_comments)
}

pub fn make_anyhow_error(input: Span, e: VerboseError<Span>) -> weaver_util::prelude::Error {
    anyhow!("Failed to parse shaders:\n{}", convert_error(input, e))
}

pub fn parse_shaders(input: &str) -> Result<Vec<ParsedShader>> {
    let (_, shaders) = all_consuming(many1(space_delimited(parse_shader)))(input)
        .finish()
        .map_err(|e| make_anyhow_error(input, e))?;

    Ok(shaders)
}

pub fn parse_shader(input: Span) -> IResult<Span, ParsedShader> {
    map(
        pair(
            space_delimited(full_line(parse_path)),
            delimited(
                space_delimited(char('{')),
                tuple((
                    many0(space_delimited(full_line(parse_directive))),
                    many0(space_delimited(parse_stage)),
                    many0(space_delimited(full_line(parse_directive))),
                )),
                space_delimited(char('}')),
            ),
        ),
        |(name, (globals, stages, more_globals))| ParsedShader {
            name,
            globals: [globals, more_globals].concat(),
            stages,
        },
    )(input)
}

pub fn parse_path(input: Span) -> IResult<Span, String> {
    let (input, name_first) = alt((tag_no_case("_"), tag_no_case("-"), alphanumeric1))(input)?;
    let (input, name) = many1(pair(
        tag_no_case("/"),
        many1(alt((
            tag_no_case("_"),
            tag_no_case("."),
            tag_no_case("-"),
            alphanumeric1,
        ))),
    ))(input)?;
    let name = name
        .into_iter()
        .fold(String::from(name_first), |mut acc, (slash, item)| {
            acc.push_str(slash);
            acc.push_str(item.concat().as_str());
            acc
        });
    Ok((input, name))
}

pub fn parse_identifier(input: Span) -> IResult<Span, String> {
    let (input, name) = recognize(many1(alt((
        tag_no_case("_"),
        tag_no_case("$"),
        tag_no_case("."),
        tag_no_case("-"),
        alphanumeric1,
    ))))(input)?;
    Ok((input, name.to_string()))
}

pub fn parse_directive_arg(input: Span) -> IResult<Span, ParsedDirectiveArg> {
    let (input, arg) = alt((
        map(float, ParsedDirectiveArg::Float),
        map(parse_path, ParsedDirectiveArg::Path),
        map(parse_identifier, ParsedDirectiveArg::Ident),
        map(
            delimited(
                tag_no_case("("),
                many1(hspace_delimited(parse_directive_arg)),
                tag_no_case(")"),
            ),
            ParsedDirectiveArg::Parens,
        ),
    ))(input)?;
    Ok((input, arg))
}

pub fn parse_directive(input: Span) -> IResult<Span, ParsedDirective> {
    let (input, name) =
        context("directive identifier", preceded(hspace0, parse_identifier))(input)?;
    let (input, args) = many0(hspace_delimited(parse_directive_arg))(input)?;
    Ok((input, ParsedDirective { name, args }))
}

pub fn parse_stage(input: Span) -> IResult<Span, ParsedStage> {
    map(
        delimited(
            hspace_delimited(char('{')),
            many1_lines(parse_directive),
            hspace_delimited(char('}')),
        ),
        |directives| ParsedStage { directives },
    )(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_path() {
        assert_eq!(
            parse_path("textures/common/caulk"),
            Ok(("", "textures/common/caulk".to_string()))
        );
    }

    #[test]
    fn test_parse_identifier() {
        assert_eq!(parse_identifier("$foo"), Ok(("", "$foo".to_string())));
        assert_eq!(parse_identifier("foo"), Ok(("", "foo".to_string())));
        assert_eq!(parse_identifier("foo_bar"), Ok(("", "foo_bar".to_string())));
    }

    #[test]
    fn test_parse_directive() {
        assert_eq!(
            parse_directive("foo"),
            Ok((
                "",
                ParsedDirective {
                    name: "foo".to_string(),
                    args: vec![]
                }
            ))
        );
        assert_eq!(
            parse_directive("foo bar"),
            Ok((
                "",
                ParsedDirective {
                    name: "foo".to_string(),
                    args: vec![ParsedDirectiveArg::Ident("bar".to_string())],
                }
            ))
        );
        assert_eq!(
            parse_directive("foo bar 4 baz/qux.tga"),
            Ok((
                "",
                ParsedDirective {
                    name: "foo".to_string(),
                    args: vec![
                        ParsedDirectiveArg::Ident("bar".to_string()),
                        ParsedDirectiveArg::Float(4.0),
                        ParsedDirectiveArg::Path("baz/qux.tga".to_string())
                    ]
                }
            ))
        );
        assert_eq!(
            parse_directive("map $lightmap"),
            Ok((
                "",
                ParsedDirective {
                    name: "map".to_string(),
                    args: vec![ParsedDirectiveArg::Ident("$lightmap".to_string())],
                }
            ))
        );
        assert_eq!(
            parse_directive("rgbGen identity"),
            Ok((
                "",
                ParsedDirective {
                    name: "rgbGen".to_string(),
                    args: vec![ParsedDirectiveArg::Ident("identity".to_string())],
                }
            ))
        );
    }

    #[test]
    fn test_parse_stage() {
        let input = r#"
            {
                foo
            }
            "#;
        assert_eq!(
            space_delimited(parse_stage)(input)
                .finish()
                .map_err(|e| make_anyhow_error(input, e))
                .unwrap(),
            (
                "",
                ParsedStage {
                    directives: vec![ParsedDirective {
                        name: "foo".to_string(),
                        args: vec![]
                    },]
                }
            )
        );

        let input = r#"
            {
                foo
                bar
            }
            "#;
        assert_eq!(
            space_delimited(parse_stage)(input)
                .finish()
                .map_err(|e| make_anyhow_error(input, e))
                .unwrap(),
            (
                "",
                ParsedStage {
                    directives: vec![
                        ParsedDirective {
                            name: "foo".to_string(),
                            args: vec![]
                        },
                        ParsedDirective {
                            name: "bar".to_string(),
                            args: vec![]
                        },
                    ]
                }
            )
        );
    }

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
        parse_shaders(input).unwrap();
    }
}

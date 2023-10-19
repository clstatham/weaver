use raqote::{DrawOptions, DrawTarget, PathBuilder, SolidSource, StrokeStyle};

pub struct Renderer;

impl Renderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn render(&self, draw_target: &mut DrawTarget) -> anyhow::Result<()> {
        let mut path = PathBuilder::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 200.0);
        let path = path.finish();
        draw_target.stroke(
            &path,
            &SolidSource {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            }
            .into(),
            &StrokeStyle::default(),
            &DrawOptions::new(),
        );

        Ok(())
    }
}

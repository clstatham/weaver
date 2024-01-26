//! Syntax highlighting for Loom files.
//! Mostly copied from egui_extras::syntax_highlighting (Apache 2.0 | MIT licensed).

use egui::text::LayoutJob;

pub fn highlight(ctx: &egui::Context, text: &str) -> LayoutJob {
    impl egui::util::cache::ComputerMut<&str, LayoutJob> for Highlighter {
        fn compute(&mut self, text: &str) -> LayoutJob {
            self.highlight(text)
        }
    }

    type Cache = egui::util::cache::FrameCache<LayoutJob, Highlighter>;
    ctx.memory_mut(|mem| mem.caches.cache::<Cache>().get(text))
}

pub struct Highlighter {
    pub theme: syntect::highlighting::Theme,
    pub syntax_set: syntect::parsing::SyntaxSet,
}

impl Default for Highlighter {
    fn default() -> Self {
        let mut set = syntect::parsing::SyntaxSetBuilder::new();
        set.add_from_folder("assets/syntaxes", true).unwrap();
        let mut theme_set = syntect::highlighting::ThemeSet::load_defaults();
        theme_set.add_from_folder("assets/syntaxes").unwrap();
        let theme = theme_set.themes["One Dark"].clone();
        Self {
            theme,
            syntax_set: set.build(),
        }
    }
}

impl Highlighter {
    pub fn highlight(&self, text: &str) -> LayoutJob {
        self.highlight_impl(text).unwrap_or_else(|| {
            LayoutJob::simple(
                text.into(),
                egui::FontId::monospace(12.0),
                egui::Color32::from_rgb(128, 128, 128),
                f32::INFINITY,
            )
        })
    }

    fn highlight_impl(&self, text: &str) -> Option<LayoutJob> {
        use syntect::easy::HighlightLines;
        use syntect::highlighting::FontStyle;
        use syntect::util::LinesWithEndings;

        let syntax = self.syntax_set.find_syntax_by_extension("loom").unwrap();
        let mut h = HighlightLines::new(syntax, &self.theme);

        use egui::text::{LayoutSection, TextFormat};

        let mut job = LayoutJob {
            text: text.into(),
            ..Default::default()
        };

        for line in LinesWithEndings::from(text) {
            for (style, range) in h.highlight_line(line, &self.syntax_set).ok()? {
                let fg = style.foreground;
                let text_color = egui::Color32::from_rgb(fg.r, fg.g, fg.b);
                let italics = style.font_style.contains(FontStyle::ITALIC);
                let underline = style.font_style.contains(FontStyle::ITALIC);
                let underline = if underline {
                    egui::Stroke::new(1.0, text_color)
                } else {
                    egui::Stroke::NONE
                };
                job.sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: as_byte_range(text, range),
                    format: TextFormat {
                        font_id: egui::FontId::monospace(12.0),
                        color: text_color,
                        italics,
                        underline,
                        ..Default::default()
                    },
                });
            }
        }

        Some(job)
    }
}

fn as_byte_range(whole: &str, range: &str) -> std::ops::Range<usize> {
    let whole_start = whole.as_ptr() as usize;
    let range_start = range.as_ptr() as usize;
    assert!(whole_start <= range_start);
    assert!(range_start + range.len() <= whole_start + whole.len());
    let offset = range_start - whole_start;
    offset..(offset + range.len())
}

use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use vello::Glyph;
use vello::Scene;
use vello::kurbo::Affine;
use vello::peniko::{Color, Fill};

use crate::resources::fonts::FontResource;

#[derive(Debug, Clone, Copy)]
pub struct TextLayoutMetrics {
    pub width: f32,
    pub height: f32,
}

pub fn draw_simple_line(
    scene: &mut Scene,
    font: &FontResource,
    text: &str,
    font_size: f32,
    origin: (f32, f32),
    color: Color,
) -> Option<TextLayoutMetrics> {
    let font_data = font.font_data();
    let font_ref = skrifa::FontRef::from_index(font_data.data.as_ref(), font_data.index).ok()?;
    let metrics = font_ref.metrics(Size::new(font_size), LocationRef::default());
    let glyph_metrics = font_ref.glyph_metrics(Size::new(font_size), LocationRef::default());
    let charmap = font_ref.charmap();

    let fallback_advance = font_size * 0.35;
    let mut pen_x = 0.0f32;
    let mut glyphs = Vec::with_capacity(text.chars().count());

    for ch in text.chars() {
        if ch == ' ' {
            pen_x += fallback_advance;
            continue;
        }

        let glyph_id = charmap.map(ch)?;
        glyphs.push(Glyph {
            id: glyph_id.to_u32(),
            x: pen_x,
            y: 0.0,
        });
        pen_x += glyph_metrics
            .advance_width(glyph_id)
            .unwrap_or(font_size * 0.5);
    }

    scene
        .draw_glyphs(font_data)
        .font_size(font_size)
        .transform(
            Affine::translate((origin.0 as f64, (origin.1 + metrics.ascent) as f64))
                * Affine::scale_non_uniform(1.0, -1.0),
        )
        .brush(color)
        .draw(Fill::NonZero, glyphs.into_iter());

    Some(TextLayoutMetrics {
        width: pen_x,
        height: metrics.ascent - metrics.descent + metrics.leading,
    })
}

use anyhow::Result;
use skia_safe::{
    Canvas, Color4f, FontMgr,
    textlayout::{FontCollection, ParagraphBuilder, ParagraphStyle, TextAlign, TextDirection, TextStyle},
};
use winit::dpi::PhysicalSize;

use crate::scene::{SceneRect, TextRole};

pub struct TextSystem {
    font_collection: FontCollection,
    viewport: PhysicalSize<u32>,
}

impl TextSystem {
    pub fn new(viewport: PhysicalSize<u32>) -> Result<Self> {
        let mut font_collection = FontCollection::new();
        font_collection.set_default_font_manager_and_family_names(
            FontMgr::default(),
            BODY_FAMILIES,
        );

        Ok(Self {
            font_collection,
            viewport,
        })
    }

    pub fn resize(&mut self, viewport: PhysicalSize<u32>) {
        self.viewport = viewport;
    }

    pub fn draw_text(
        &self,
        canvas: &Canvas,
        text: &str,
        rect: SceneRect,
        role: TextRole,
        color: Color4f,
    ) {
        if text.is_empty() || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }

        let spec = typography_spec(role);
        let mut text_style = TextStyle::new();
        text_style.set_font_families(spec.families);
        text_style.set_font_size(spec.font_size);
        text_style.set_color(color.to_color());
        text_style.set_height(spec.line_height / spec.font_size);
        text_style.set_height_override(true);
        text_style.set_letter_spacing(spec.letter_spacing);
        text_style.set_locale("zh-CN");

        let mut paragraph_style = ParagraphStyle::new();
        paragraph_style.set_text_style(&text_style);
        paragraph_style.set_text_direction(TextDirection::LTR);
        paragraph_style.set_text_align(TextAlign::Left);
        paragraph_style.set_max_lines(spec.max_lines);
        paragraph_style.set_ellipsis("...");

        let mut builder = ParagraphBuilder::new(&paragraph_style, self.font_collection.clone());
        builder.push_style(&text_style);
        builder.add_text(text);
        builder.pop();

        let mut paragraph = builder.build();
        paragraph.layout(rect.width.max(1.0));
        paragraph.paint(canvas, (rect.x, rect.y));
    }
}

struct TypographySpec {
    families: &'static [&'static str],
    font_size: f32,
    line_height: f32,
    letter_spacing: f32,
    max_lines: Option<usize>,
}

const DISPLAY_FAMILIES: &[&str] = &[
    "Segoe UI Variable Display",
    "SF Pro Display",
    "PingFang SC",
    "Microsoft YaHei UI",
    "Noto Sans CJK SC",
    "Source Han Sans SC",
    "sans-serif",
];

const BODY_FAMILIES: &[&str] = &[
    "Segoe UI Variable Text",
    "SF Pro Text",
    "PingFang SC",
    "Microsoft YaHei UI",
    "Noto Sans CJK SC",
    "Source Han Sans SC",
    "sans-serif",
];

const MONO_FAMILIES: &[&str] = &[
    "Cascadia Mono",
    "JetBrains Mono",
    "Sarasa Mono SC",
    "Consolas",
    "Menlo",
    "monospace",
];

fn typography_spec(role: TextRole) -> TypographySpec {
    match role {
        TextRole::Hero => TypographySpec {
            families: DISPLAY_FAMILIES,
            font_size: 34.0,
            line_height: 40.0,
            letter_spacing: -0.02,
            max_lines: Some(2),
        },
        TextRole::Title => TypographySpec {
            families: DISPLAY_FAMILIES,
            font_size: 20.0,
            line_height: 26.0,
            letter_spacing: -0.01,
            max_lines: Some(1),
        },
        TextRole::Body => TypographySpec {
            families: BODY_FAMILIES,
            font_size: 16.0,
            line_height: 22.0,
            letter_spacing: 0.0,
            max_lines: Some(3),
        },
        TextRole::Status => TypographySpec {
            families: BODY_FAMILIES,
            font_size: 15.0,
            line_height: 20.0,
            letter_spacing: 0.0,
            max_lines: Some(2),
        },
        TextRole::Debug => TypographySpec {
            families: MONO_FAMILIES,
            font_size: 14.0,
            line_height: 18.0,
            letter_spacing: 0.0,
            max_lines: Some(8),
        },
    }
}

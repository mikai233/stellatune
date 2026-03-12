use anyhow::{Result, anyhow};
use skia_safe::{
    AlphaType, Canvas, Color, Color4f, ColorType, ImageInfo, Paint, PaintStyle, Point, Rect, RRect,
    surfaces,
};
use winit::dpi::PhysicalSize;

use crate::scene::{
    ButtonNode, ButtonVariant, ImageKind, ListNode, PanelNode, PanelStyle, SceneNode, SceneRect, SceneState,
    TextNode, TextRole,
};
use crate::text::TextSystem;

use super::frame::UiFrame;

pub struct SkiaRenderer {
    clear_color: Color,
}

const SHELL_MARGIN: f32 = 0.0;
const SHELL_RADIUS: f32 = 0.0;

impl SkiaRenderer {
    pub fn new() -> Self {
        Self {
            clear_color: Color::TRANSPARENT,
        }
    }

    pub fn render(
        &mut self,
        scene: &SceneState,
        text_system: &mut TextSystem,
        viewport: PhysicalSize<u32>,
    ) -> Result<UiFrame> {
        let width = viewport.width.max(1);
        let height = viewport.height.max(1);
        let mut surface = surfaces::raster_n32_premul((width as i32, height as i32))
            .ok_or_else(|| anyhow!("create Skia raster surface"))?;
        {
            let canvas = surface.canvas();
            canvas.clear(self.clear_color);
            let shell = shell_rect(viewport);
            canvas.save();
            self.draw_shell_tint(canvas, shell);
            self.draw_background(canvas, scene, viewport);

            for layer in &scene.graph.layers {
                for node in &layer.nodes {
                    self.draw_node(canvas, scene, text_system, node);
                }
            }
            canvas.restore();
        }

        let row_bytes = width as usize * 4;
        let mut pixels = vec![0_u8; row_bytes * height as usize];
        let info = ImageInfo::new(
            (width as i32, height as i32),
            ColorType::BGRA8888,
            AlphaType::Premul,
            None,
        );
        if !surface.read_pixels(&info, pixels.as_mut_slice(), row_bytes, (0, 0)) {
            return Err(anyhow!("read pixels from Skia surface"));
        }

        Ok(UiFrame {
            label: format!(
                "skia-ui:{}x{}:{}:{}",
                width,
                height,
                scene.status,
                scene.graph.layer_label_summary()
            ),
            width,
            height,
            row_bytes,
            pixels,
        })
    }

    fn draw_node(
        &self,
        canvas: &Canvas,
        scene: &SceneState,
        text_system: &TextSystem,
        node: &SceneNode,
    ) {
        match node {
            SceneNode::Panel(panel) => self.draw_panel(canvas, panel),
            SceneNode::Text(text) => self.draw_text(canvas, text_system, text),
            SceneNode::Button(button) => self.draw_button(canvas, scene, text_system, button),
            SceneNode::Image(image) => {
                let (outer_fill, outer_stroke, inner_fill, inner_stroke) = match image.kind {
                    ImageKind::AlbumCover => (
                        Color4f::new(0.17, 0.21, 0.30, 0.94),
                        Color4f::new(0.29, 0.36, 0.48, 0.95),
                        Color4f::new(0.10, 0.13, 0.19, 0.96),
                        Color4f::new(0.36, 0.54, 0.85, 0.55),
                    ),
                };
                self.draw_shadow(
                    canvas,
                    image.rect,
                    26.0,
                    Color4f::new(0.02, 0.04, 0.08, 0.28),
                    (0.0, 18.0),
                );
                self.draw_card(
                    canvas,
                    image.rect,
                    outer_fill,
                    Some(outer_stroke),
                    24.0,
                );
                let inner = image.rect.inset(14.0, 14.0);
                self.draw_card(
                    canvas,
                    inner,
                    inner_fill,
                    Some(inner_stroke),
                    18.0,
                );
                self.draw_album_placeholder(canvas, inner);
                self.draw_label(
                    canvas,
                    text_system,
                    &image.caption,
                    image.rect.inset(18.0, image.rect.height - 46.0),
                    TextRole::Body,
                    Color4f::new(0.94, 0.95, 0.98, 0.92),
                );
            },
            SceneNode::List(list) => self.draw_list(canvas, text_system, list),
            SceneNode::Effect(_) => {},
        }
    }

    fn draw_panel(&self, canvas: &Canvas, panel: &PanelNode) {
        let (fill, stroke, radius) = match panel.style {
            PanelStyle::Titlebar => (
                Color4f::new(0.13, 0.17, 0.25, 0.035),
                Some(Color4f::new(0.88, 0.93, 1.0, 0.11)),
                30.0,
            ),
            PanelStyle::Sidebar => (
                Color4f::new(0.10, 0.12, 0.17, 0.09),
                Some(Color4f::new(0.64, 0.74, 0.90, 0.22)),
                32.0,
            ),
            PanelStyle::Main => (
                Color4f::new(0.09, 0.11, 0.16, 0.065),
                Some(Color4f::new(0.62, 0.72, 0.90, 0.20)),
                36.0,
            ),
            PanelStyle::Queue => (
                Color4f::new(0.10, 0.12, 0.17, 0.08),
                Some(Color4f::new(0.62, 0.72, 0.88, 0.19)),
                28.0,
            ),
            PanelStyle::Status => (
                Color4f::new(0.08, 0.10, 0.14, 0.055),
                Some(Color4f::new(0.60, 0.69, 0.84, 0.17)),
                24.0,
            ),
            PanelStyle::Overlay => (
                Color4f::new(0.18, 0.08, 0.10, 0.96),
                Some(Color4f::new(0.42, 0.20, 0.22, 0.90)),
                18.0,
            ),
        };
        if !matches!(panel.style, PanelStyle::Titlebar) {
            self.draw_shadow(
                canvas,
                panel.rect,
                radius,
                Color4f::new(0.02, 0.03, 0.06, 0.025),
                (0.0, 8.0),
            );
        }
        self.draw_card(canvas, panel.rect, fill, stroke, radius);
        self.draw_top_accent(
            canvas,
            panel.rect,
            radius,
            match panel.style {
                PanelStyle::Titlebar => Color4f::new(0.92, 0.96, 1.0, 0.10),
                PanelStyle::Sidebar => Color4f::new(0.74, 0.84, 1.0, 0.11),
                PanelStyle::Main => Color4f::new(0.72, 0.82, 1.0, 0.10),
                PanelStyle::Queue => Color4f::new(0.90, 0.76, 0.58, 0.10),
                PanelStyle::Status => Color4f::new(0.70, 0.80, 1.0, 0.09),
                PanelStyle::Overlay => Color4f::new(0.92, 0.46, 0.40, 0.16),
            },
        );
    }

    fn draw_button(
        &self,
        canvas: &Canvas,
        scene: &SceneState,
        text_system: &TextSystem,
        button: &ButtonNode,
    ) {
        let hovered = scene.hovered_node_id.as_deref() == Some(button.id.as_str());
        let (fill, stroke) = match (button.variant, button.selected, hovered) {
            (ButtonVariant::Primary, true, _) => (
                Color4f::new(0.35, 0.56, 0.94, 1.0),
                Some(Color4f::new(0.79, 0.88, 1.0, 0.88)),
            ),
            (ButtonVariant::Primary, false, true) => (
                Color4f::new(0.25, 0.42, 0.76, 0.98),
                Some(Color4f::new(0.61, 0.74, 0.95, 0.82)),
            ),
            (ButtonVariant::Primary, false, false) => (
                Color4f::new(0.20, 0.34, 0.61, 0.94),
                Some(Color4f::new(0.35, 0.48, 0.72, 0.70)),
            ),
            (ButtonVariant::Secondary, true, _) => (
                Color4f::new(0.33, 0.29, 0.58, 0.95),
                Some(Color4f::new(0.65, 0.58, 0.92, 0.70)),
            ),
            (ButtonVariant::Secondary, false, true) => (
                Color4f::new(0.22, 0.24, 0.36, 0.96),
                Some(Color4f::new(0.45, 0.48, 0.72, 0.70)),
            ),
            (ButtonVariant::Secondary, false, false) => (
                Color4f::new(0.17, 0.18, 0.27, 0.92),
                Some(Color4f::new(0.29, 0.31, 0.42, 0.64)),
            ),
            (ButtonVariant::Nav, true, _) => (
                Color4f::new(0.83, 0.44, 0.28, 0.98),
                Some(Color4f::new(0.99, 0.82, 0.72, 0.86)),
            ),
            (ButtonVariant::Nav, false, true) => (
                Color4f::new(0.20, 0.21, 0.30, 0.94),
                Some(Color4f::new(0.41, 0.45, 0.57, 0.74)),
            ),
            (ButtonVariant::Nav, false, false) => (
                Color4f::new(0.14, 0.15, 0.22, 0.90),
                Some(Color4f::new(0.23, 0.26, 0.34, 0.62)),
            ),
            (ButtonVariant::Ghost, _, true) => (
                Color4f::new(0.16, 0.18, 0.24, 0.88),
                Some(Color4f::new(0.36, 0.40, 0.51, 0.62)),
            ),
            (ButtonVariant::Ghost, _, false) => (
                Color4f::new(0.11, 0.13, 0.18, 0.80),
                Some(Color4f::new(0.20, 0.22, 0.28, 0.56)),
            ),
        };
        self.draw_shadow(
            canvas,
            button.rect,
            16.0,
            Color4f::new(0.01, 0.02, 0.05, if hovered || button.selected { 0.22 } else { 0.12 }),
            (0.0, if hovered { 10.0 } else { 6.0 }),
        );
        self.draw_card(canvas, button.rect, fill, stroke, 16.0);
        self.draw_top_accent(
            canvas,
            button.rect,
            16.0,
            if button.selected {
                Color4f::new(1.0, 1.0, 1.0, 0.12)
            } else if hovered {
                Color4f::new(1.0, 1.0, 1.0, 0.08)
            } else {
                Color4f::new(1.0, 1.0, 1.0, 0.04)
            },
        );
        self.draw_label(
            canvas,
            text_system,
            &button.label,
            button.rect.inset(16.0, 8.0),
            TextRole::Body,
            Color4f::new(0.95, 0.96, 0.99, 0.96),
        );
    }

    fn draw_list(&self, canvas: &Canvas, text_system: &TextSystem, list: &ListNode) {
        let fill = if list.compact {
            Color4f::new(0.12, 0.14, 0.19, 0.10)
        } else {
            Color4f::new(0.12, 0.14, 0.18, 0.09)
        };
        self.draw_shadow(
            canvas,
            list.rect,
            20.0,
            Color4f::new(0.02, 0.03, 0.06, 0.022),
            (0.0, 10.0),
        );
        self.draw_card(
            canvas,
            list.rect,
            fill,
            Some(Color4f::new(0.74, 0.84, 0.98, 0.20)),
            20.0,
        );
        self.draw_top_accent(
            canvas,
            list.rect,
            20.0,
            Color4f::new(0.88, 0.94, 1.0, 0.10),
        );
        self.draw_label(
            canvas,
            text_system,
            &list.title,
            SceneRect::new(list.rect.x + 18.0, list.rect.y + 18.0, list.rect.width - 36.0, 28.0),
            TextRole::Title,
            Color4f::new(0.97, 0.98, 0.99, 0.96),
        );

        let item_height = if list.compact { 28.0 } else { 34.0 };
        for (index, item) in list.items.iter().enumerate() {
            let item_rect = SceneRect::new(
                list.rect.x + 16.0,
                list.rect.y + 56.0 + index as f32 * (item_height + 8.0),
                list.rect.width - 32.0,
                item_height,
            );
            if item_rect.y + item_rect.height > list.rect.y + list.rect.height - 12.0 {
                break;
            }
            let selected = list.selected_index == Some(index);
            if selected {
                self.draw_card(
                    canvas,
                    item_rect,
                    Color4f::new(0.21, 0.29, 0.45, 0.18),
                    Some(Color4f::new(0.82, 0.90, 1.0, 0.30)),
                    12.0,
                );
            } else {
                self.draw_divider(
                    canvas,
                    item_rect.x,
                    item_rect.y + item_rect.height + 4.0,
                    item_rect.width,
                    Color4f::new(0.36, 0.42, 0.52, 0.18),
                );
            }
            self.draw_label(
                canvas,
                text_system,
                &format!("{:02}  {}", index + 1, item),
                item_rect.inset(12.0, 4.0),
                TextRole::Body,
                if selected {
                    Color4f::new(0.98, 0.99, 1.0, 0.98)
                } else {
                    Color4f::new(0.84, 0.87, 0.92, 0.90)
                },
            );
        }
    }

    fn draw_text(&self, canvas: &Canvas, text_system: &TextSystem, text: &TextNode) {
        let color = match text.role {
            TextRole::Hero => Color4f::new(0.97, 0.98, 1.0, 0.98),
            TextRole::Title => Color4f::new(0.95, 0.97, 1.0, 0.94),
            TextRole::Body => Color4f::new(0.82, 0.86, 0.92, 0.90),
            TextRole::Status => Color4f::new(0.80, 0.84, 0.90, 0.84),
            TextRole::Debug => Color4f::new(1.0, 0.92, 0.92, 0.96),
        };
        if matches!(text.role, TextRole::Hero) {
            self.draw_label(
                canvas,
                text_system,
                &text.content,
                SceneRect::new(text.rect.x, text.rect.y + 2.0, text.rect.width, text.rect.height),
                text.role,
                Color4f::new(0.09, 0.14, 0.24, 0.42),
            );
        }
        self.draw_label(canvas, text_system, &text.content, text.rect, text.role, color);
    }

    fn draw_background(&self, canvas: &Canvas, scene: &SceneState, viewport: PhysicalSize<u32>) {
        let bounds = SceneRect::new(0.0, 0.0, viewport.width as f32, viewport.height as f32);
        self.draw_background_orb(
            canvas,
            scene.pointer
                .map(|(x, y)| Point::new(x as f32, y as f32))
                .unwrap_or_else(|| Point::new(bounds.width * 0.72, bounds.height * 0.28)),
            bounds.width * 0.38,
            match scene.visual_mode_label.as_str() {
                "Immersive" => Color4f::new(0.18, 0.32, 0.82, 0.035),
                "Pulse" => Color4f::new(0.42, 0.18, 0.74, 0.03),
                _ => Color4f::new(0.21, 0.39, 0.72, 0.026),
            },
        );
        self.draw_background_orb(
            canvas,
            Point::new(bounds.width * 0.18, bounds.height * 0.78),
            bounds.width * 0.32,
            Color4f::new(0.89, 0.46, 0.23, 0.020),
        );
        self.draw_background_grid(canvas, bounds);
    }

    fn draw_shell_tint(&self, canvas: &Canvas, shell: SceneRect) {
        self.draw_card(
            canvas,
            shell,
            Color4f::new(0.05, 0.08, 0.12, 0.04),
            Some(Color4f::new(0.94, 0.97, 1.0, 0.08)),
            SHELL_RADIUS,
        );
        self.draw_top_accent(
            canvas,
            shell,
            SHELL_RADIUS,
            Color4f::new(1.0, 1.0, 1.0, 0.05),
        );
    }

    fn draw_card(
        &self,
        canvas: &Canvas,
        rect: SceneRect,
        fill_color: Color4f,
        stroke_color: Option<Color4f>,
        radius: f32,
    ) {
        let round_rect = RRect::new_rect_xy(as_skia_rect(rect), radius, radius);
        let mut fill = Paint::new(fill_color, None);
        fill.set_anti_alias(true);
        fill.set_style(PaintStyle::Fill);
        canvas.draw_rrect(round_rect, &fill);

        if let Some(stroke_color) = stroke_color {
            let mut stroke = Paint::new(stroke_color, None);
            stroke.set_anti_alias(true);
            stroke.set_style(PaintStyle::Stroke);
            stroke.set_stroke_width(1.5);
            canvas.draw_rrect(round_rect, &stroke);
        }
    }

    fn draw_shadow(
        &self,
        canvas: &Canvas,
        rect: SceneRect,
        radius: f32,
        color: Color4f,
        offset: (f32, f32),
    ) {
        let shadow_rect = SceneRect::new(
            rect.x + offset.0,
            rect.y + offset.1,
            rect.width,
            rect.height,
        );
        let mut shadow = Paint::new(color, None);
        shadow.set_anti_alias(true);
        shadow.set_style(PaintStyle::Fill);
        canvas.draw_rrect(
            RRect::new_rect_xy(as_skia_rect(shadow_rect), radius, radius),
            &shadow,
        );
    }

    fn draw_top_accent(&self, canvas: &Canvas, rect: SceneRect, radius: f32, color: Color4f) {
        let accent_rect = SceneRect::new(rect.x + 1.0, rect.y + 1.0, rect.width - 2.0, 18.0);
        let mut paint = Paint::new(color, None);
        paint.set_anti_alias(true);
        paint.set_style(PaintStyle::Fill);
        canvas.draw_rrect(
            RRect::new_rect_xy(as_skia_rect(accent_rect), radius.min(16.0), radius.min(16.0)),
            &paint,
        );
    }

    fn draw_divider(&self, canvas: &Canvas, x: f32, y: f32, width: f32, color: Color4f) {
        let mut paint = Paint::new(color, None);
        paint.set_anti_alias(true);
        paint.set_stroke_width(1.0);
        canvas.draw_line((x, y), (x + width, y), &paint);
    }

    fn draw_background_orb(&self, canvas: &Canvas, center: Point, radius: f32, color: Color4f) {
        let mut paint = Paint::new(color, None);
        paint.set_anti_alias(true);
        paint.set_style(PaintStyle::Fill);
        canvas.draw_circle(center, radius, &paint);
    }

    fn draw_background_grid(&self, canvas: &Canvas, bounds: SceneRect) {
        let mut paint = Paint::new(Color4f::new(0.82, 0.88, 1.0, 0.008), None);
        paint.set_anti_alias(true);
        paint.set_stroke_width(1.0);

        let step = 48.0;
        let mut x = 24.0;
        while x < bounds.width {
            canvas.draw_line((x, 0.0), (x, bounds.height), &paint);
            x += step;
        }
        let mut y = 24.0;
        while y < bounds.height {
            canvas.draw_line((0.0, y), (bounds.width, y), &paint);
            y += step;
        }
    }

    fn draw_album_placeholder(&self, canvas: &Canvas, rect: SceneRect) {
        let rings = [
            (0.45_f32, Color4f::new(0.29, 0.56, 0.96, 0.30)),
            (0.32_f32, Color4f::new(0.84, 0.48, 0.30, 0.20)),
            (0.18_f32, Color4f::new(0.92, 0.94, 0.98, 0.18)),
        ];
        let center = Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.42);
        for (scale, color) in rings {
            let mut paint = Paint::new(color, None);
            paint.set_anti_alias(true);
            paint.set_style(PaintStyle::Stroke);
            paint.set_stroke_width(8.0);
            canvas.draw_circle(center, rect.width.min(rect.height) * scale, &paint);
        }
        self.draw_divider(
            canvas,
            rect.x + 26.0,
            rect.y + rect.height * 0.74,
            rect.width - 52.0,
            Color4f::new(0.90, 0.93, 0.99, 0.20),
        );
    }

    fn draw_label(
        &self,
        canvas: &Canvas,
        text_system: &TextSystem,
        text: &str,
        rect: SceneRect,
        role: TextRole,
        color: Color4f,
    ) {
        text_system.draw_text(canvas, text, rect, role, color);
    }
}

fn as_skia_rect(rect: SceneRect) -> Rect {
    Rect::from_xywh(rect.x, rect.y, rect.width, rect.height)
}

fn shell_rect(viewport: PhysicalSize<u32>) -> SceneRect {
    let margin = SHELL_MARGIN;
    SceneRect::new(
        margin,
        margin,
        (viewport.width as f32 - margin * 2.0).max(320.0),
        (viewport.height as f32 - margin * 2.0).max(240.0),
    )
}

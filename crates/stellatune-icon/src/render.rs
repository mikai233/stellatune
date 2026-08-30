use std::{fs, path::Path};

use anyhow::{Context, Result, anyhow, ensure};
use clap::ValueEnum;
use skia_safe::{
    Canvas, ClipOp, Color, Color4f, Data, EncodedImageFormat, Paint, PaintStyle, Path as SkPath,
    PathBuilder, Point, RRect, Rect, TileMode, gradient, paint, path_utils, surfaces,
};

use crate::document::{BackgroundPreset, IconDocument};

pub const DEFAULT_EXPORT_SIZE: i32 = 2048;
const DESIGN_VIEWBOX: f32 = 1024.0;
const FOREGROUND_SCALE: f32 = 1.24;
const FOREGROUND_OFFSET_X: f32 = 22.0;
const ROUNDED_MASK_RADIUS_RATIO: f32 = 0.223;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum IconLayer {
    Background,
    Foreground,
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ExportMask {
    Square,
    Rounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSize {
    pub width: i32,
    pub height: i32,
}

impl PixelSize {
    pub fn new(width: i32, height: i32) -> Result<Self> {
        ensure!(width > 0, "width must be positive");
        ensure!(height > 0, "height must be positive");
        Ok(Self { width, height })
    }

    pub fn square(size: i32) -> Result<Self> {
        Self::new(size, size)
    }
}

#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub document: IconDocument,
    pub layer: IconLayer,
    pub size: PixelSize,
    pub mask: ExportMask,
}

impl RenderRequest {
    pub fn new(layer: IconLayer, size: PixelSize) -> Self {
        Self::with_style(layer, size, BackgroundPreset::Teal, ExportMask::Square)
    }

    pub fn with_background_preset(
        layer: IconLayer,
        size: PixelSize,
        preset: BackgroundPreset,
    ) -> Self {
        Self::with_style(layer, size, preset, ExportMask::Square)
    }

    pub fn with_style(
        layer: IconLayer,
        size: PixelSize,
        preset: BackgroundPreset,
        mask: ExportMask,
    ) -> Self {
        Self {
            document: IconDocument::with_background_preset(preset),
            layer,
            size,
            mask,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderContext {
    pub canvas_rect: Rect,
    pub icon_rect: Rect,
}

impl RenderContext {
    pub fn new(size: PixelSize) -> Self {
        let width = size.width as f32;
        let height = size.height as f32;
        let side = width.min(height);
        let left = (width - side) * 0.5;
        let top = (height - side) * 0.5;

        Self {
            canvas_rect: Rect::from_xywh(0.0, 0.0, width, height),
            icon_rect: Rect::from_xywh(left, top, side, side),
        }
    }

    pub fn x(self, value: f32) -> f32 {
        self.icon_rect.left + (value / DESIGN_VIEWBOX) * self.icon_rect.width()
    }

    pub fn y(self, value: f32) -> f32 {
        self.icon_rect.top + (value / DESIGN_VIEWBOX) * self.icon_rect.height()
    }

    pub fn scalar(self, value: f32) -> f32 {
        (value / DESIGN_VIEWBOX) * self.icon_rect.width()
    }

    pub fn rect(self, left: f32, top: f32, width: f32, height: f32) -> Rect {
        Rect::from_xywh(
            self.x(left),
            self.y(top),
            self.scalar(width),
            self.scalar(height),
        )
    }
}

pub fn render_png(request: &RenderRequest) -> Result<Data> {
    let mut surface = surfaces::raster_n32_premul((request.size.width, request.size.height))
        .ok_or_else(|| anyhow!("failed to create raster surface"))?;

    {
        let canvas = surface.canvas();
        canvas.clear(skia_safe::Color::TRANSPARENT);
        render_canvas(canvas, request);
    }

    #[allow(deprecated)]
    {
        surface
            .image_snapshot()
            .encode_to_data(EncodedImageFormat::PNG)
            .ok_or_else(|| anyhow!("failed to encode image as png"))
    }
}

pub fn write_png(request: &RenderRequest, output: impl AsRef<Path>) -> Result<()> {
    let output = output.as_ref();
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create parent directory for {}", output.display())
        })?;
    }

    let data = render_png(request)?;
    fs::write(output, data.as_bytes())
        .with_context(|| format!("failed to write png to {}", output.display()))?;
    Ok(())
}

fn render_canvas(canvas: &Canvas, request: &RenderRequest) {
    let ctx = RenderContext::new(request.size);
    let clip_guard = apply_export_mask(canvas, ctx, request.mask);

    if matches!(request.layer, IconLayer::Background | IconLayer::Composite) {
        draw_background(canvas, ctx, &request.document);
    }

    if matches!(request.layer, IconLayer::Foreground | IconLayer::Composite) {
        draw_foreground(canvas, ctx, &request.document);
    }

    if clip_guard {
        canvas.restore();
    }
}

fn apply_export_mask(canvas: &Canvas, ctx: RenderContext, mask: ExportMask) -> bool {
    if !matches!(mask, ExportMask::Rounded) {
        return false;
    }

    let radius = ctx.icon_rect.width().min(ctx.icon_rect.height()) * ROUNDED_MASK_RADIUS_RATIO;
    let rrect = RRect::new_rect_xy(ctx.canvas_rect, radius, radius);
    canvas.save();
    canvas.clip_rrect(rrect, ClipOp::Intersect, true);
    true
}

fn draw_background(canvas: &Canvas, ctx: RenderContext, document: &IconDocument) {
    let palette = document.palette;
    let background_rect = ctx.canvas_rect;

    let mut base = Paint::default();
    base.set_anti_alias(false);
    let base_gradient_colors = [
        Color4f::from(palette.background_card_top),
        Color4f::from(palette.background_card_bottom),
    ];
    base.set_shader(make_linear_gradient(
        (background_rect.left, background_rect.top),
        (background_rect.right, background_rect.bottom),
        &base_gradient_colors,
        None,
    ));
    canvas.draw_rect(background_rect, &base);

    let mut inner_cool = Paint::default();
    inner_cool.set_anti_alias(false);
    let inner_cool_colors = [
        Color4f::from(with_alpha(palette.background_accent_top, 0.06)),
        Color4f::from(with_alpha(palette.background_accent_top, 0.015)),
        Color4f::from(Color::TRANSPARENT),
    ];
    inner_cool.set_shader(make_radial_gradient(
        Point::new(ctx.x(590.0), ctx.y(360.0)),
        ctx.scalar(360.0),
        &inner_cool_colors,
        Some(&[0.0, 0.45, 1.0]),
    ));
    canvas.draw_rect(background_rect, &inner_cool);

    let mut inner_warm = Paint::default();
    inner_warm.set_anti_alias(false);
    let inner_warm_colors = [
        Color4f::from(with_alpha(palette.background_accent_bottom, 0.035)),
        Color4f::from(with_alpha(palette.background_accent_bottom, 0.01)),
        Color4f::from(Color::TRANSPARENT),
    ];
    inner_warm.set_shader(make_radial_gradient(
        Point::new(ctx.x(360.0), ctx.y(760.0)),
        ctx.scalar(260.0),
        &inner_warm_colors,
        Some(&[0.0, 0.38, 1.0]),
    ));
    canvas.draw_rect(background_rect, &inner_warm);
}

fn draw_foreground(canvas: &Canvas, ctx: RenderContext, document: &IconDocument) {
    let palette = document.palette;
    let triangle_path = triangle_path(ctx);
    let star_center = scaled_point(ctx, Point::new(ctx.x(476.0), ctx.y(515.0)));
    let star_points = star_points(star_center, ctx.scalar(180.0) * FOREGROUND_SCALE, 0.44);
    let star_path = star_path(&star_points);

    let triangle_stroke_width = ctx.scalar(50.0) * FOREGROUND_SCALE;

    let mut triangle_paint = make_stroke_paint(palette.foreground_triangle, triangle_stroke_width);
    triangle_paint.set_stroke_join(paint::Join::Round);

    let star_paint = make_fill_paint(palette.foreground_star);

    canvas.save();
    for notch_tip_index in [0_usize, 4_usize] {
        let notch_path = star_corner_notch_mask(
            &star_points,
            notch_tip_index,
            ctx,
            ctx.scalar(104.0) * FOREGROUND_SCALE,
            ctx.scalar(88.0) * FOREGROUND_SCALE,
        );
        canvas.clip_path(&notch_path, ClipOp::Difference, true);
    }
    canvas.draw_path(&triangle_path, &triangle_paint);
    canvas.restore();

    canvas.draw_path(&star_path, &star_paint);
}

fn make_stroke_paint(color: skia_safe::Color, stroke_width: f32) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Stroke);
    paint.set_stroke_width(stroke_width);
    paint.set_stroke_cap(paint::Cap::Round);
    paint.set_stroke_join(paint::Join::Round);
    paint.set_color(color);
    paint
}

fn make_fill_paint(color: skia_safe::Color) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Fill);
    paint.set_color(color);
    paint
}

fn triangle_path(ctx: RenderContext) -> SkPath {
    let vertices = triangle_vertices(ctx);
    SkPath::polygon(&vertices, true, None, None)
}

fn triangle_vertices(ctx: RenderContext) -> [Point; 3] {
    [
        scaled_point(ctx, Point::new(ctx.x(284.0), ctx.y(218.0))),
        scaled_point(ctx, Point::new(ctx.x(284.0), ctx.y(806.0))),
        scaled_point(ctx, Point::new(ctx.x(790.0), ctx.y(512.0))),
    ]
}

fn star_points(center: Point, outer_radius: f32, inner_ratio: f32) -> Vec<Point> {
    let inner_radius = outer_radius * inner_ratio;
    let mut points = Vec::with_capacity(10);

    for index in 0..10 {
        let angle = (-90.0 + index as f32 * 36.0).to_radians();
        let radius = if index % 2 == 0 {
            outer_radius
        } else {
            inner_radius
        };
        points.push(Point::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        ));
    }

    points
}

fn star_path(points: &[Point]) -> SkPath {
    SkPath::polygon(points, true, None, None)
}

fn star_corner_notch_mask(
    star_points: &[Point],
    tip_index: usize,
    ctx: RenderContext,
    arm_length: f32,
    stroke_width: f32,
) -> SkPath {
    let point_count = star_points.len();
    let tip = star_points[tip_index];
    let prev = star_points[(tip_index + point_count - 1) % point_count];
    let next = star_points[(tip_index + 1) % point_count];

    let notch_points = [
        move_towards(tip, prev, arm_length),
        tip,
        move_towards(tip, next, arm_length),
    ];
    let notch_path = SkPath::polygon(&notch_points, false, None, None);

    let mut stroke_paint = make_stroke_paint(skia_safe::Color::WHITE, stroke_width);
    stroke_paint.set_stroke_join(paint::Join::Miter);
    stroke_paint.set_stroke_miter(4.0);

    let mut mask = PathBuilder::new();
    let cull_rect = ctx.canvas_rect;
    let _ = path_utils::fill_path_with_paint(
        &notch_path,
        &stroke_paint,
        &mut mask,
        Some(&cull_rect),
        None,
    );
    mask.detach()
}

fn move_towards(from: Point, to: Point, distance: f32) -> Point {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let length = (dx * dx + dy * dy).sqrt();

    if length <= f32::EPSILON {
        return from;
    }

    let scale = (distance / length).min(1.0);
    Point::new(from.x + dx * scale, from.y + dy * scale)
}

fn scaled_point(ctx: RenderContext, point: Point) -> Point {
    let center = Point::new(ctx.x(512.0), ctx.y(512.0));
    Point::new(
        center.x + (point.x - center.x) * FOREGROUND_SCALE + ctx.scalar(FOREGROUND_OFFSET_X),
        center.y + (point.y - center.y) * FOREGROUND_SCALE,
    )
}

fn make_linear_gradient(
    start: (f32, f32),
    end: (f32, f32),
    colors: &[Color4f],
    positions: Option<&[f32]>,
) -> skia_safe::Shader {
    let gradient_colors = gradient::Colors::new(colors, positions, TileMode::Clamp, None);
    let gradient = gradient::Gradient::new(gradient_colors, gradient::Interpolation::default());
    gradient::shaders::linear_gradient(
        (Point::new(start.0, start.1), Point::new(end.0, end.1)),
        &gradient,
        None,
    )
    .expect("linear gradient shader should be created")
}

fn make_radial_gradient(
    center: Point,
    radius: f32,
    colors: &[Color4f],
    positions: Option<&[f32]>,
) -> skia_safe::Shader {
    let gradient_colors = gradient::Colors::new(colors, positions, TileMode::Clamp, None);
    let gradient = gradient::Gradient::new(gradient_colors, gradient::Interpolation::default());
    gradient::shaders::radial_gradient((center, radius), &gradient, None)
        .expect("radial gradient shader should be created")
}

fn with_alpha(color: Color, opacity: f32) -> Color {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_argb(alpha, color.r(), color.g(), color.b())
}

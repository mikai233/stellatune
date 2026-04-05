mod text;

use std::f64::consts::TAU;

use vello::Scene;
use vello::kurbo::{Affine, Circle, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::resources::fonts::FontResource;
use crate::scene::text::draw_simple_line;
use crate::view::home::HomeViewModel;

#[derive(Clone, Copy)]
pub struct DemoSceneFrame<'a> {
    pub scene: &'a Scene,
    pub cover_rect: Rect,
}

#[derive(Default)]
pub struct DemoScene {
    scene: Scene,
}

impl DemoScene {
    pub fn new() -> Self {
        Self {
            scene: Scene::new(),
        }
    }

    pub fn rebuild(
        &mut self,
        view_model: &HomeViewModel,
        ui_font: &FontResource,
    ) -> DemoSceneFrame<'_> {
        self.scene.reset();

        let width = view_model.viewport.width.max(1) as f64;
        let height = view_model.viewport.height.max(1) as f64;
        let cover_rect = Rect::new(
            view_model.cover.rect[0],
            view_model.cover.rect[1],
            view_model.cover.rect[2],
            view_model.cover.rect[3],
        );

        self.draw_shell(width, height, view_model);
        self.draw_cover(width, height, view_model, cover_rect);
        self.draw_progress(width, height, view_model);
        self.draw_header(width, height, view_model, ui_font);
        self.draw_lyrics(width, height, view_model);
        self.draw_diagnostics(width, height, view_model);

        DemoSceneFrame {
            scene: &self.scene,
            cover_rect,
        }
    }

    fn draw_shell(&mut self, width: f64, height: f64, view_model: &HomeViewModel) {
        let shell = RoundedRect::from_rect(
            Rect::new(
                view_model.shell.inset[0],
                view_model.shell.inset[1],
                width - view_model.shell.inset[2],
                height - view_model.shell.inset[3],
            ),
            36.0,
        );
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(8, 14, 18, 214),
            None,
            &shell,
        );
        self.scene.stroke(
            &Stroke::new(2.0),
            Affine::IDENTITY,
            Color::from_rgba8(255, 255, 255, 26),
            None,
            &shell,
        );
    }

    fn draw_cover(
        &mut self,
        width: f64,
        height: f64,
        view_model: &HomeViewModel,
        cover_rect: Rect,
    ) {
        let hero = RoundedRect::from_rect(cover_rect, 28.0);
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(255, 255, 255, 20),
            None,
            &hero,
        );
        self.scene.stroke(
            &Stroke::new(2.0),
            Affine::IDENTITY,
            Color::from_rgba8(255, 250, 244, 38),
            None,
            &hero,
        );

        let hero_center = (
            (view_model.cover.rect[0] + view_model.cover.rect[2]) * 0.5,
            (view_model.cover.rect[1] + view_model.cover.rect[3]) * 0.5,
        );
        let album_disc = Circle::new(
            hero_center,
            cover_rect.width().min(cover_rect.height()) * 0.32,
        );
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(16, 20, 24, 230),
            None,
            &album_disc,
        );
        self.scene.stroke(
            &Stroke::new(10.0),
            Affine::IDENTITY,
            Color::from_rgba8(255, 244, 228, 50),
            None,
            &album_disc,
        );

        for index in 0..view_model.cover.orbit_count {
            let angle = view_model.cover.orbit_phase + (index as f64 * (TAU / 3.0));
            let orbit = Circle::new(
                (
                    hero_center.0 + angle.cos() * width * 0.05,
                    hero_center.1 + angle.sin() * height * 0.04,
                ),
                9.0 + index as f64 * 3.0,
            );
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgba8(255, 235, 206, 168),
                None,
                &orbit,
            );
        }
    }

    fn draw_progress(&mut self, width: f64, height: f64, view_model: &HomeViewModel) {
        let rail = RoundedRect::from_rect(
            Rect::new(width * 0.56, height * 0.28, width * 0.88, height * 0.31),
            999.0,
        );
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(255, 255, 255, 26),
            None,
            &rail,
        );

        let progress_bar = RoundedRect::from_rect(
            Rect::new(
                width * 0.56,
                height * 0.28,
                width * (0.56 + 0.32 * view_model.progress.value),
                height * 0.31,
            ),
            999.0,
        );
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(246, 196, 72, 255),
            None,
            &progress_bar,
        );
    }

    fn draw_header(
        &mut self,
        width: f64,
        height: f64,
        view_model: &HomeViewModel,
        ui_font: &FontResource,
    ) {
        let title_origin = (width as f32 * 0.56, height as f32 * 0.17);
        let subtitle_origin = (width as f32 * 0.56, height as f32 * 0.22);
        let caption_origin = (width as f32 * 0.56, height as f32 * 0.325);

        let title_metrics = draw_simple_line(
            &mut self.scene,
            ui_font,
            &view_model.header.title,
            40.0,
            title_origin,
            Color::from_rgba8(255, 248, 239, 255),
        );
        let subtitle_metrics = draw_simple_line(
            &mut self.scene,
            ui_font,
            &view_model.header.subtitle,
            18.0,
            subtitle_origin,
            Color::from_rgba8(255, 248, 239, 160),
        );
        let caption_metrics = draw_simple_line(
            &mut self.scene,
            ui_font,
            &view_model.header.caption,
            14.0,
            caption_origin,
            Color::from_rgba8(246, 196, 72, 220),
        );

        if let Some(title_metrics) = title_metrics {
            let glow = RoundedRect::from_rect(
                Rect::new(
                    width * 0.56,
                    height * 0.255,
                    width * 0.56 + title_metrics.width as f64 * 0.42,
                    height * 0.261,
                ),
                999.0,
            );
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgba8(255, 248, 239, 34),
                None,
                &glow,
            );
        }

        if let Some(subtitle_metrics) = subtitle_metrics {
            let underline = RoundedRect::from_rect(
                Rect::new(
                    width * 0.56,
                    height * 0.245 + subtitle_metrics.height as f64 * 0.2,
                    width * 0.56 + subtitle_metrics.width as f64,
                    height * 0.245 + subtitle_metrics.height as f64 * 0.28,
                ),
                999.0,
            );
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgba8(255, 248, 239, 18),
                None,
                &underline,
            );
        }

        if let Some(caption_metrics) = caption_metrics {
            let tag = RoundedRect::from_rect(
                Rect::new(
                    width * 0.56 - 14.0,
                    height * 0.325 - 8.0,
                    width * 0.56 + caption_metrics.width as f64 + 14.0,
                    height * 0.325 + caption_metrics.height as f64 + 6.0,
                ),
                999.0,
            );
            self.scene.stroke(
                &Stroke::new(1.0),
                Affine::IDENTITY,
                Color::from_rgba8(246, 196, 72, 60),
                None,
                &tag,
            );
        }
    }

    fn draw_lyrics(&mut self, width: f64, height: f64, view_model: &HomeViewModel) {
        for (row, scale) in view_model.lyrics.line_scales.iter().enumerate() {
            let y = height * (0.38 + row as f64 * 0.065);
            let line = RoundedRect::from_rect(
                Rect::new(
                    width * 0.56,
                    y,
                    width * (0.56 + 0.32 * *scale),
                    y + height * 0.022,
                ),
                12.0,
            );
            let alpha = 90u8.saturating_sub((row as u8) * 6);
            self.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                Color::from_rgba8(255, 248, 239, alpha),
                None,
                &line,
            );
        }
    }

    fn draw_diagnostics(&mut self, width: f64, height: f64, view_model: &HomeViewModel) {
        let badge = RoundedRect::from_rect(
            Rect::new(
                width * 0.79,
                height * 0.16,
                width * (0.79 + 0.09 * view_model.diagnostics.fps_fill.max(0.35)),
                height * 0.195,
            ),
            999.0,
        );
        self.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            Color::from_rgba8(16, 210, 184, 220),
            None,
            &badge,
        );
    }
}

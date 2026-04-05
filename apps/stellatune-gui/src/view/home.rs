use winit::dpi::PhysicalSize;

use crate::app::FrameState;

#[derive(Debug, Clone)]
pub struct HomeShellModel {
    pub inset: [f64; 4],
}

#[derive(Debug, Clone)]
pub struct HomeCoverModel {
    pub rect: [f64; 4],
    pub orbit_phase: f64,
    pub orbit_count: usize,
}

#[derive(Debug, Clone)]
pub struct HomeHeaderModel {
    pub title: String,
    pub subtitle: String,
    pub caption: String,
}

#[derive(Debug, Clone)]
pub struct HomeProgressModel {
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct HomeLyricsModel {
    pub line_scales: [f64; 7],
}

#[derive(Debug, Clone)]
pub struct HomeDiagnosticsModel {
    pub fps_fill: f64,
}

#[derive(Debug, Clone)]
pub struct HomeViewModel {
    pub viewport: PhysicalSize<u32>,
    pub shell: HomeShellModel,
    pub cover: HomeCoverModel,
    pub header: HomeHeaderModel,
    pub progress: HomeProgressModel,
    pub lyrics: HomeLyricsModel,
    pub diagnostics: HomeDiagnosticsModel,
}

impl HomeViewModel {
    pub fn demo(
        frame: &FrameState,
        cover_aspect_ratio: f64,
        texture_count: usize,
        ui_font_family: &str,
    ) -> Self {
        let width = frame.physical_size.width.max(1) as f64;
        let height = frame.physical_size.height.max(1) as f64;
        let cover_aspect_ratio = cover_aspect_ratio.clamp(0.75, 1.5);

        let cover_left = width * 0.11;
        let cover_top = height * 0.16;
        let cover_width = width * 0.30;
        let cover_height = (cover_width / cover_aspect_ratio).min(height * 0.52);

        Self {
            viewport: frame.physical_size,
            shell: HomeShellModel {
                inset: [width * 0.07, height * 0.08, width * 0.07, height * 0.08],
            },
            cover: HomeCoverModel {
                rect: [
                    cover_left,
                    cover_top,
                    cover_left + cover_width,
                    cover_top + cover_height,
                ],
                orbit_phase: frame.elapsed_seconds as f64 * 0.45,
                orbit_count: 3 + texture_count.min(2),
            },
            header: HomeHeaderModel {
                title: "Neon Aurora Mix".to_owned(),
                subtitle: "StellaTune Experimental Render Shell".to_owned(),
                caption: ui_font_family.to_owned(),
            },
            progress: HomeProgressModel {
                value: 0.35 + 0.25 * ((frame.elapsed_seconds as f64 * 0.8).sin() * 0.5 + 0.5),
            },
            lyrics: HomeLyricsModel {
                line_scales: [1.0, 0.965, 0.93, 1.0, 0.965, 0.93, 1.0],
            },
            diagnostics: HomeDiagnosticsModel {
                fps_fill: (frame.smoothed_fps.clamp(24.0, 144.0) / 144.0) as f64,
            },
        }
    }
}

use std::time::Instant;

use winit::dpi::PhysicalSize;

use crate::scene::SceneState;

use super::frame::EffectFrame;
use super::gpu::GpuContext;

pub struct EffectRenderer {
    started_at: Instant,
}

impl EffectRenderer {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }

    pub fn render(
        &mut self,
        scene: &SceneState,
        viewport: PhysicalSize<u32>,
        _gpu: &GpuContext,
    ) -> EffectFrame {
        let effect_summary = scene
            .graph
            .effect_nodes()
            .map(|effect| {
                format!(
                    "{}:{:.2}:{}:{:.0}x{:.0}",
                    effect.id,
                    effect.intensity,
                    if effect.animated { "anim" } else { "still" },
                    effect.rect.width,
                    effect.rect.height
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        let (clear_color, accent_color, glow_color) = palette_for_scene(scene);
        let intensity = scene
            .graph
            .effect_nodes()
            .map(|effect| effect.intensity)
            .fold(0.0_f32, f32::max)
            .max(0.18);
        let pointer = scene
            .pointer
            .map(|(x, y)| {
                [
                    (x as f32 / viewport.width.max(1) as f32).clamp(0.0, 1.0),
                    (y as f32 / viewport.height.max(1) as f32).clamp(0.0, 1.0),
                ]
            })
            .unwrap_or([0.72, 0.26]);
        EffectFrame {
            label: format!(
                "effect-pass:{}x{}:{}:{}:{}",
                viewport.width,
                viewport.height,
                scene.app_title,
                scene.graph.layer_label_summary(),
                effect_summary
            ),
            clear_color,
            accent_color,
            glow_color,
            pointer,
            intensity,
            time: self.started_at.elapsed().as_secs_f32(),
        }
    }
}

fn palette_for_scene(scene: &SceneState) -> ([f32; 4], [f32; 4], [f32; 4]) {
    match scene.visual_mode_label.as_str() {
        "Immersive" => (
            [0.03, 0.05, 0.09, 0.0],
            [0.22, 0.44, 0.96, 1.0],
            [0.95, 0.38, 0.28, 1.0],
        ),
        "Pulse" => (
            [0.07, 0.05, 0.10, 0.0],
            [0.58, 0.26, 0.90, 1.0],
            [0.97, 0.54, 0.30, 1.0],
        ),
        _ => (
            [0.03, 0.05, 0.08, 0.0],
            [0.23, 0.44, 0.78, 1.0],
            [0.88, 0.50, 0.26, 1.0],
        ),
    }
}

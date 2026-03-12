use anyhow::Result;
use tracing::info;
use winit::dpi::PhysicalSize;

use crate::platform::host::SurfaceHandles;
use crate::scene::SceneState;
use crate::text::TextSystem;

use super::compositor::Compositor;
use super::effect_renderer::EffectRenderer;
use super::frame::{EffectFrame, UiFrame};
use super::gpu::GpuContext;
use super::skia_renderer::SkiaRenderer;

pub struct Renderer {
    gpu: GpuContext,
    effects: EffectRenderer,
    skia: SkiaRenderer,
    compositor: Compositor,
    viewport: PhysicalSize<u32>,
}

impl Renderer {
    pub fn new(surface_handles: SurfaceHandles, viewport: PhysicalSize<u32>) -> Result<Self> {
        let gpu = GpuContext::new(surface_handles, viewport)?;
        let effects = EffectRenderer::new();
        let skia = SkiaRenderer::new();
        let compositor = Compositor::new(gpu.device(), gpu.surface_format());
        let adapter = gpu.adapter_info();
        info!(
            backend = ?adapter.backend,
            adapter = %adapter.name,
            format = ?gpu.surface_format(),
            "renderer bootstrap complete"
        );
        Ok(Self {
            gpu,
            effects,
            skia,
            compositor,
            viewport,
        })
    }

    pub fn resize(&mut self, viewport: PhysicalSize<u32>) {
        self.viewport = viewport;
        self.gpu.resize(viewport);
    }

    pub fn draw(&mut self, scene: &SceneState, text_system: &mut TextSystem) -> Result<()> {
        let effect_frame = self.build_effect_frame(scene);
        let ui_frame = self.skia.render(scene, text_system, self.viewport)?;
        self.compositor
            .compose(&mut self.gpu, effect_frame, ui_frame, self.viewport)?;
        Ok(())
    }

    pub fn build_effect_frame(&mut self, scene: &SceneState) -> EffectFrame {
        self.effects.render(scene, self.viewport, &self.gpu)
    }

    pub fn build_ui_frame(
        &mut self,
        scene: &SceneState,
        text_system: &mut TextSystem,
    ) -> Result<UiFrame> {
        self.skia.render(scene, text_system, self.viewport)
    }
}

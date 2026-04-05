use anyhow::Result;
use vello::peniko::Color;

pub struct VelloRenderer {
    renderer: vello::Renderer,
}

impl VelloRenderer {
    pub fn new(device: &wgpu::Device) -> Result<Self> {
        let renderer = vello::Renderer::new(
            device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: std::num::NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )?;
        Ok(Self { renderer })
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &vello::Scene,
        target: &wgpu::TextureView,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> Result<()> {
        self.renderer.render_to_texture(
            device,
            queue,
            scene,
            target,
            &vello::RenderParams {
                base_color: Color::from_rgba8(0, 0, 0, 0),
                width: size.width.max(1),
                height: size.height.max(1),
                antialiasing_method: vello::AaConfig::Msaa16,
            },
        )?;
        Ok(())
    }
}

use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use winit::dpi::PhysicalSize;
use winit::window::Window;

#[derive(Debug)]
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    size: PhysicalSize<u32>,
    scale_factor: f64,
}

impl GpuContext {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(Arc::clone(&window))
            .context("create wgpu surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("request adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("stellatune-gui-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .context("request device")?;

        let size = window.inner_size();
        let scale_factor = window.scale_factor();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or_else(|| anyhow!("surface is not supported by the selected adapter"))?;
        let capabilities = surface.get_capabilities(&adapter);
        if let Some(format) = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
        {
            config.format = format;
        }
        config.present_mode = if capabilities
            .present_modes
            .contains(&wgpu::PresentMode::AutoVsync)
        {
            wgpu::PresentMode::AutoVsync
        } else {
            config.present_mode
        };
        config.alpha_mode = capabilities
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::PreMultiplied)
            .unwrap_or(config.alpha_mode);

        surface.configure(&device, &config);

        Ok(Self {
            device,
            queue,
            surface,
            config,
            size,
            scale_factor,
        })
    }

    pub fn is_zero_sized(&self) -> bool {
        self.size.width == 0 || self.size.height == 0
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.size = new_size;
        if self.is_zero_sized() {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn set_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor;
    }

    pub fn surface_size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }
}

use anyhow::{Context, Result, anyhow};

use crate::platform::host::SurfaceHandles;

pub struct GpuContext {
    #[allow(dead_code)]
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl GpuContext {
    pub fn new(surface_handles: SurfaceHandles, viewport: winit::dpi::PhysicalSize<u32>) -> Result<Self> {
        pollster::block_on(Self::new_async(surface_handles, viewport))
    }

    async fn new_async(
        surface_handles: SurfaceHandles,
        viewport: winit::dpi::PhysicalSize<u32>,
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(&instance_descriptor());
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: surface_handles.raw_display_handle,
                raw_window_handle: surface_handles.raw_window_handle,
            })
        }
        .context("create wgpu surface")?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("request wgpu adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("stellatune-gui-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::default(),
                experimental_features: Default::default(),
            })
            .await
            .context("request wgpu device")?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| anyhow!("surface reported no supported formats"))?;
        let present_mode = caps
            .present_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::PresentMode::AutoVsync)
            .or_else(|| caps.present_modes.first().copied())
            .ok_or_else(|| anyhow!("surface reported no present modes"))?;
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
            .or_else(|| caps.alpha_modes.first().copied())
            .ok_or_else(|| anyhow!("surface reported no alpha modes"))?;

        tracing::info!(
            ?format,
            ?present_mode,
            ?alpha_mode,
            alpha_modes = ?caps.alpha_modes,
            "selected wgpu surface configuration"
        );

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: viewport.width.max(1),
            height: viewport.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self {
            instance,
            surface,
            adapter,
            device,
            queue,
            config,
        })
    }

    pub fn resize(&mut self, viewport: winit::dpi::PhysicalSize<u32>) {
        if viewport.width == 0 || viewport.height == 0 {
            return;
        }
        self.config.width = viewport.width;
        self.config.height = viewport.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render<F>(&mut self, clear_color: wgpu::Color, draw: F) -> Result<()>
    where
        F: FnOnce(&wgpu::Device, &mut wgpu::CommandEncoder, &wgpu::TextureView) -> Result<()>,
    {
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            },
            Err(wgpu::SurfaceError::Timeout) => {
                return Ok(());
            },
            Err(wgpu::SurfaceError::OutOfMemory) => {
                return Err(anyhow!("wgpu surface out of memory"));
            },
            Err(wgpu::SurfaceError::Other) => {
                return Ok(());
            },
        };

        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("stellatune-gui-frame"),
            });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("stellatune-gui-clear-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        draw(&self.device, &mut encoder, &view)?;
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    #[allow(dead_code)]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    #[allow(dead_code)]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn adapter_info(&self) -> wgpu::AdapterInfo {
        self.adapter.get_info()
    }
}

#[cfg(target_os = "windows")]
fn instance_descriptor() -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..Default::default()
    }
}

#[cfg(not(target_os = "windows"))]
fn instance_descriptor() -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor::default()
}

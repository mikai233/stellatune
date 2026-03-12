use anyhow::Result;
use winit::dpi::PhysicalSize;

use super::effect_pass::EffectPass;
use super::frame::{EffectFrame, UiFrame};
use super::gpu::GpuContext;
use super::texture_renderer::TextureRenderer;

pub struct Compositor {
    effects: EffectPass,
    textured_ui: TextureRenderer,
    ui_texture: Option<UiTexture>,
}

impl Compositor {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self {
            effects: EffectPass::new(device, surface_format),
            textured_ui: TextureRenderer::new(device, surface_format),
            ui_texture: None,
        }
    }

    pub fn compose(
        &mut self,
        gpu: &mut GpuContext,
        effects: EffectFrame,
        ui: UiFrame,
        viewport: PhysicalSize<u32>,
    ) -> Result<()> {
        let _frame_label = format!(
            "viewport={}x{} :: {} -> {}",
            viewport.width, viewport.height, effects.label, ui.label
        );
        let color = wgpu::Color {
            r: effects.clear_color[0] as f64,
            g: effects.clear_color[1] as f64,
            b: effects.clear_color[2] as f64,
            a: effects.clear_color[3] as f64,
        };
        self.effects.update(gpu.queue(), viewport, &effects);

        let bind_group = if ui.width == 0 || ui.height == 0 || ui.pixels.is_empty() {
            None
        } else {
            Some(self.upload_ui_frame(gpu.device(), gpu.queue(), ui))
        };

        gpu.render(color, |_device, encoder, view| {
            self.effects.draw(encoder, view)?;
            if let Some(bind_group) = bind_group.as_ref() {
                self.textured_ui.draw(encoder, view, bind_group)?;
            }
            Ok(())
        })
    }

    fn upload_ui_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        ui: UiFrame,
    ) -> wgpu::BindGroup {
        self.ensure_ui_texture(device, ui.width, ui.height);
        let texture = self.ui_texture.as_ref().expect("ui texture initialized");
        let upload = padded_upload(ui.pixels.as_slice(), ui.row_bytes, ui.height as usize);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            upload.bytes.as_slice(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(upload.row_bytes as u32),
                rows_per_image: Some(ui.height),
            },
            wgpu::Extent3d {
                width: ui.width,
                height: ui.height,
                depth_or_array_layers: 1,
            },
        );
        self.textured_ui.create_bind_group(device, &texture.view)
    }

    fn ensure_ui_texture(
        &mut self,
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) {
        let needs_recreate = self
            .ui_texture
            .as_ref()
            .map(|texture| texture.width != width || texture.height != height)
            .unwrap_or(true);

        if needs_recreate {
            self.ui_texture = Some(UiTexture::new(device, width, height));
        }
    }
}

struct UiTexture {
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl UiTexture {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stellatune-gui-ui-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            width,
            height,
            texture,
            view,
        }
    }
}

struct PaddedUpload {
    bytes: Vec<u8>,
    row_bytes: usize,
}

fn padded_upload(pixels: &[u8], row_bytes: usize, height: usize) -> PaddedUpload {
    let aligned_row_bytes = row_bytes
        .next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize)
        .max(row_bytes);
    if aligned_row_bytes == row_bytes {
        return PaddedUpload {
            bytes: pixels.to_vec(),
            row_bytes,
        };
    }

    let mut bytes = vec![0_u8; aligned_row_bytes * height];
    for row in 0..height {
        let src_start = row * row_bytes;
        let src_end = src_start + row_bytes;
        let dst_start = row * aligned_row_bytes;
        let dst_end = dst_start + row_bytes;
        bytes[dst_start..dst_end].copy_from_slice(&pixels[src_start..src_end]);
    }

    PaddedUpload {
        bytes,
        row_bytes: aligned_row_bytes,
    }
}

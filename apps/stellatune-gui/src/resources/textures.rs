use std::collections::HashMap;

use anyhow::{Result, ensure};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureHandle(u64);

#[derive(Debug, Clone, Copy)]
pub struct TextureMetadata {
    pub size: winit::dpi::PhysicalSize<u32>,
    pub format: wgpu::TextureFormat,
}

#[derive(Debug)]
pub struct TextureResource {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    metadata: TextureMetadata,
}

impl TextureResource {
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn sampler(&self) -> &wgpu::Sampler {
        &self.sampler
    }

    pub fn metadata(&self) -> TextureMetadata {
        self.metadata
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TextureCatalogStats {
    pub count: usize,
}

#[derive(Debug, Default)]
pub struct TextureCatalog {
    next_id: u64,
    textures: HashMap<TextureHandle, TextureResource>,
}

impl TextureCatalog {
    pub fn upload_rgba8(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Result<TextureHandle> {
        ensure!(
            width > 0 && height > 0,
            "texture dimensions must be non-zero"
        );

        let expected_len = width as usize * height as usize * 4;
        ensure!(
            data.len() == expected_len,
            "rgba8 upload expected {expected_len} bytes, got {}",
            data.len()
        );

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("stellatune-gui-resource-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let handle = self.next_handle();
        let metadata = TextureMetadata {
            size: winit::dpi::PhysicalSize::new(width, height),
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
        };

        self.textures.insert(
            handle,
            TextureResource {
                _texture: texture,
                view,
                sampler,
                metadata,
            },
        );

        Ok(handle)
    }

    pub fn create_demo_cover(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<TextureHandle> {
        let width = 4;
        let height = 4;
        let mut data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            for x in 0..width {
                let coral = [240, 116, 102, 255];
                let gold = [246, 196, 72, 255];
                let ink = [18, 24, 31, 255];
                let color = match (x + y) % 3 {
                    0 => coral,
                    1 => gold,
                    _ => ink,
                };
                data.extend_from_slice(&color);
            }
        }

        self.upload_rgba8(
            device,
            queue,
            "stellatune-gui-demo-cover",
            width,
            height,
            &data,
        )
    }

    pub fn get(&self, handle: TextureHandle) -> Option<&TextureResource> {
        self.textures.get(&handle)
    }

    pub fn stats(&self) -> TextureCatalogStats {
        TextureCatalogStats {
            count: self.textures.len(),
        }
    }

    fn next_handle(&mut self) -> TextureHandle {
        let handle = TextureHandle(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        handle
    }
}

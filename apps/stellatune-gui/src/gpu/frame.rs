use winit::dpi::PhysicalSize;

const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[derive(Debug)]
pub struct OffscreenTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl OffscreenTarget {
    fn new(
        device: &wgpu::Device,
        label: &str,
        size: PhysicalSize<u32>,
        usage: wgpu::TextureUsages,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            _texture: texture,
            view,
        }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

#[derive(Debug)]
pub struct FrameTargets {
    effect: OffscreenTarget,
    media: OffscreenTarget,
    source_vector: OffscreenTarget,
    destination_vector: OffscreenTarget,
}

impl FrameTargets {
    pub fn new(device: &wgpu::Device, size: PhysicalSize<u32>) -> Self {
        let effect = OffscreenTarget::new(
            device,
            "stellatune-gui-effect-target",
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let media = OffscreenTarget::new(
            device,
            "stellatune-gui-media-target",
            size,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let source_vector = OffscreenTarget::new(
            device,
            "stellatune-gui-source-vector-target",
            size,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        );
        let destination_vector = OffscreenTarget::new(
            device,
            "stellatune-gui-destination-vector-target",
            size,
            wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::STORAGE_BINDING,
        );
        Self {
            effect,
            media,
            source_vector,
            destination_vector,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, size: PhysicalSize<u32>) {
        *self = Self::new(device, size);
    }

    pub fn effect_view(&self) -> &wgpu::TextureView {
        self.effect.view()
    }

    pub fn source_vector_view(&self) -> &wgpu::TextureView {
        self.source_vector.view()
    }

    pub fn destination_vector_view(&self) -> &wgpu::TextureView {
        self.destination_vector.view()
    }

    pub fn media_view(&self) -> &wgpu::TextureView {
        self.media.view()
    }
}

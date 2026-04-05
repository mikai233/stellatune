#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OffscreenLayer {
    Effects,
    Media,
    VectorUi,
}

#[derive(Debug, Clone, Copy)]
pub struct CompositeLayers<'a> {
    pub background: &'a wgpu::TextureView,
    pub media: &'a wgpu::TextureView,
    pub foreground: &'a wgpu::TextureView,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameLayers<'a> {
    effect: &'a wgpu::TextureView,
    media: &'a wgpu::TextureView,
    vector: &'a wgpu::TextureView,
}

impl<'a> FrameLayers<'a> {
    pub fn new(
        effect: &'a wgpu::TextureView,
        media: &'a wgpu::TextureView,
        vector: &'a wgpu::TextureView,
    ) -> Self {
        Self {
            effect,
            media,
            vector,
        }
    }

    pub fn view(&self, layer: OffscreenLayer) -> &'a wgpu::TextureView {
        match layer {
            OffscreenLayer::Effects => self.effect,
            OffscreenLayer::Media => self.media,
            OffscreenLayer::VectorUi => self.vector,
        }
    }

    pub fn composite_layers(&self) -> CompositeLayers<'a> {
        CompositeLayers {
            background: self.view(OffscreenLayer::Effects),
            media: self.view(OffscreenLayer::Media),
            foreground: self.view(OffscreenLayer::VectorUi),
        }
    }
}

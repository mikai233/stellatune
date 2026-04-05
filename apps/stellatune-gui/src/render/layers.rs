#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OffscreenLayer {
    Effects,
    Media,
    SourceVectorUi,
    DestinationVectorUi,
}

#[derive(Debug, Clone, Copy)]
pub struct CompositeLayers<'a> {
    pub background: &'a wgpu::TextureView,
    pub media: &'a wgpu::TextureView,
    pub source_foreground: &'a wgpu::TextureView,
    pub destination_foreground: &'a wgpu::TextureView,
}

#[derive(Debug, Clone, Copy)]
pub struct FrameLayers<'a> {
    effect: &'a wgpu::TextureView,
    media: &'a wgpu::TextureView,
    source_vector: &'a wgpu::TextureView,
    destination_vector: &'a wgpu::TextureView,
}

impl<'a> FrameLayers<'a> {
    pub fn new(
        effect: &'a wgpu::TextureView,
        media: &'a wgpu::TextureView,
        source_vector: &'a wgpu::TextureView,
        destination_vector: &'a wgpu::TextureView,
    ) -> Self {
        Self {
            effect,
            media,
            source_vector,
            destination_vector,
        }
    }

    pub fn view(&self, layer: OffscreenLayer) -> &'a wgpu::TextureView {
        match layer {
            OffscreenLayer::Effects => self.effect,
            OffscreenLayer::Media => self.media,
            OffscreenLayer::SourceVectorUi => self.source_vector,
            OffscreenLayer::DestinationVectorUi => self.destination_vector,
        }
    }

    pub fn composite_layers(&self) -> CompositeLayers<'a> {
        CompositeLayers {
            background: self.view(OffscreenLayer::Effects),
            media: self.view(OffscreenLayer::Media),
            source_foreground: self.view(OffscreenLayer::SourceVectorUi),
            destination_foreground: self.view(OffscreenLayer::DestinationVectorUi),
        }
    }
}

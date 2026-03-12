#[derive(Debug, Clone)]
pub struct EffectFrame {
    pub label: String,
    pub clear_color: [f32; 4],
    pub accent_color: [f32; 4],
    pub glow_color: [f32; 4],
    pub pointer: [f32; 2],
    pub intensity: f32,
    pub time: f32,
}

#[derive(Debug, Clone)]
pub struct UiFrame {
    pub label: String,
    pub width: u32,
    pub height: u32,
    pub row_bytes: usize,
    pub pixels: Vec<u8>,
}

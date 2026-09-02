#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LfeMode {
    #[default]
    Mute,
    MixToFront,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResampleQuality {
    Fast,
    Balanced,
    #[default]
    High,
    Ultra,
}

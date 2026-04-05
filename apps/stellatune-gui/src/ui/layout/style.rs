#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct EdgeInsets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl EdgeInsets {
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Alignment {
    pub x: f32,
    pub y: f32,
}

impl Alignment {
    pub const TOP_LEFT: Self = Self { x: 0.0, y: 0.0 };
    pub const CENTER: Self = Self { x: 0.5, y: 0.5 };
    pub const TOP_CENTER: Self = Self { x: 0.5, y: 0.0 };
}

impl Default for Alignment {
    fn default() -> Self {
        Self::TOP_LEFT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LayoutLength {
    #[default]
    Shrink,
    Fill,
    Fixed(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutStyle {
    pub width: LayoutLength,
    pub height: LayoutLength,
    pub padding: EdgeInsets,
    pub gap: f32,
    pub alignment: Alignment,
}

impl Default for LayoutStyle {
    fn default() -> Self {
        Self {
            width: LayoutLength::Shrink,
            height: LayoutLength::Shrink,
            padding: EdgeInsets::default(),
            gap: 0.0,
            alignment: Alignment::TOP_LEFT,
        }
    }
}

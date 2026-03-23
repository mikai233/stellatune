use clap::ValueEnum;
use skia_safe::Color;

#[derive(Debug, Clone)]
pub struct IconDocument {
    pub palette: IconPalette,
}

impl Default for IconDocument {
    fn default() -> Self {
        Self::with_background_preset(BackgroundPreset::Teal)
    }
}

impl IconDocument {
    pub fn with_background_preset(preset: BackgroundPreset) -> Self {
        Self {
            palette: IconPalette::from_background_preset(preset),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BackgroundPreset {
    Indigo,
    Navy,
    Slate,
    Teal,
    Dusk,
}

#[derive(Debug, Clone, Copy)]
pub struct IconPalette {
    pub background_card_top: Color,
    pub background_card_bottom: Color,
    pub background_accent_top: Color,
    pub background_accent_bottom: Color,
    pub foreground_triangle: Color,
    pub foreground_star: Color,
}

impl Default for IconPalette {
    fn default() -> Self {
        Self::from_background_preset(BackgroundPreset::Teal)
    }
}

impl IconPalette {
    pub fn from_background_preset(preset: BackgroundPreset) -> Self {
        let (
            background_card_top,
            background_card_bottom,
            background_accent_top,
            background_accent_bottom,
        ) = match preset {
            BackgroundPreset::Indigo => (
                Color::from_argb(255, 32, 41, 74),
                Color::from_argb(255, 16, 24, 52),
                Color::from_argb(255, 74, 91, 150),
                Color::from_argb(255, 56, 47, 92),
            ),
            BackgroundPreset::Navy => (
                Color::from_argb(255, 34, 48, 74),
                Color::from_argb(255, 18, 29, 50),
                Color::from_argb(255, 76, 102, 146),
                Color::from_argb(255, 44, 67, 98),
            ),
            BackgroundPreset::Slate => (
                Color::from_argb(255, 42, 52, 78),
                Color::from_argb(255, 24, 33, 52),
                Color::from_argb(255, 88, 103, 146),
                Color::from_argb(255, 61, 72, 101),
            ),
            BackgroundPreset::Teal => (
                Color::from_argb(255, 40, 165, 255),
                Color::from_argb(255, 30, 112, 236),
                Color::from_argb(255, 168, 221, 255),
                Color::from_argb(255, 36, 148, 255),
            ),
            BackgroundPreset::Dusk => (
                Color::from_argb(255, 74, 62, 99),
                Color::from_argb(255, 43, 36, 58),
                Color::from_argb(255, 118, 101, 156),
                Color::from_argb(255, 84, 65, 120),
            ),
        };

        Self {
            background_card_top,
            background_card_bottom,
            background_accent_top,
            background_accent_bottom,
            foreground_triangle: Color::from_argb(255, 255, 255, 255),
            foreground_star: Color::from_argb(255, 255, 255, 255),
        }
    }
}

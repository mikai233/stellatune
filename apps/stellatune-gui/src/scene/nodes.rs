use winit::dpi::PhysicalSize;

#[derive(Debug, Clone)]
pub struct PanelNode {
    pub id: String,
    pub rect: SceneRect,
    pub style: PanelStyle,
}

#[derive(Debug, Clone, Copy)]
pub enum PanelStyle {
    Titlebar,
    Sidebar,
    Main,
    Queue,
    Status,
    Overlay,
}

#[derive(Debug, Clone)]
pub struct TextNode {
    pub id: String,
    pub rect: SceneRect,
    pub content: String,
    pub role: TextRole,
}

#[derive(Debug, Clone, Copy)]
pub enum TextRole {
    Hero,
    Title,
    Body,
    Status,
    Debug,
}

#[derive(Debug, Clone)]
pub struct ButtonNode {
    pub id: String,
    pub rect: SceneRect,
    pub label: String,
    pub variant: ButtonVariant,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Nav,
    Ghost,
}

#[derive(Debug, Clone)]
pub struct ImageNode {
    pub id: String,
    pub rect: SceneRect,
    pub kind: ImageKind,
    pub caption: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ImageKind {
    AlbumCover,
}

#[derive(Debug, Clone)]
pub struct ListNode {
    pub id: String,
    pub rect: SceneRect,
    pub title: String,
    pub items: Vec<String>,
    pub selected_index: Option<usize>,
    pub compact: bool,
}

#[derive(Debug, Clone)]
pub struct EffectNode {
    pub id: String,
    pub rect: SceneRect,
    pub intensity: f32,
    pub animated: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SceneRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl SceneRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn from_viewport(viewport: PhysicalSize<u32>) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: viewport.width as f32,
            height: viewport.height as f32,
        }
    }

    pub fn inset(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            width: (self.width - dx * 2.0).max(0.0),
            height: (self.height - dy * 2.0).max(0.0),
        }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.width && y <= self.y + self.height
    }
}

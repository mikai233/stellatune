use winit::dpi::{LogicalSize, Size};
use winit::window::WindowAttributes;
use winit::window::Window;

pub fn build_window_attributes() -> WindowAttributes {
    WindowAttributes::default()
        .with_title("Stellatune GUI")
        .with_inner_size(Size::Logical(LogicalSize::new(1360.0, 860.0)))
        .with_min_inner_size(Size::Logical(LogicalSize::new(960.0, 640.0)))
        .with_transparent(false)
        .with_decorations(false)
        .with_resizable(true)
}

pub fn configure_window(_window: &Window) {}

pub fn update_window_shape(_window: &Window) {}

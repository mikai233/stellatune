#![allow(dead_code)]

use winit::event::ElementState;
use winit::dpi::PhysicalSize;
use winit::event::{MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

#[derive(Debug, Clone, Copy)]
pub enum KeyCommand {
    ToggleDebugOverlay,
    TogglePlayback,
    ToggleSidebar,
    ToggleQueue,
    CycleVisualMode,
    NextRoute,
    RouteLibrary,
    RouteNowPlaying,
    RouteSettings,
}

#[derive(Debug, Clone, Copy)]
pub enum InputAction {
    CloseRequested,
    Resized(PhysicalSize<u32>),
    PointerMoved { x: f64, y: f64 },
    PointerLeft,
    PointerPrimaryPressed,
    KeyPressed(KeyCommand),
}

pub fn map_window_event(event: &WindowEvent) -> Option<InputAction> {
    match event {
        WindowEvent::CloseRequested => Some(InputAction::CloseRequested),
        WindowEvent::Resized(size) => Some(InputAction::Resized(*size)),
        WindowEvent::CursorMoved { position, .. } => Some(InputAction::PointerMoved {
            x: position.x,
            y: position.y,
        }),
        WindowEvent::CursorLeft { .. } => Some(InputAction::PointerLeft),
        WindowEvent::MouseInput { state, button, .. } => {
            if *state != ElementState::Pressed || *button != MouseButton::Left {
                return None;
            }
            Some(InputAction::PointerPrimaryPressed)
        },
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed || event.repeat {
                return None;
            }
            map_key_command(event.physical_key).map(InputAction::KeyPressed)
        },
        _ => None,
    }
}

fn map_key_command(key: PhysicalKey) -> Option<KeyCommand> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };

    match code {
        KeyCode::F5 => Some(KeyCommand::ToggleDebugOverlay),
        KeyCode::Space => Some(KeyCommand::TogglePlayback),
        KeyCode::Tab => Some(KeyCommand::NextRoute),
        KeyCode::Digit1 => Some(KeyCommand::RouteLibrary),
        KeyCode::Digit2 => Some(KeyCommand::RouteNowPlaying),
        KeyCode::Digit3 => Some(KeyCommand::RouteSettings),
        KeyCode::KeyB => Some(KeyCommand::ToggleSidebar),
        KeyCode::KeyQ => Some(KeyCommand::ToggleQueue),
        KeyCode::KeyV => Some(KeyCommand::CycleVisualMode),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
pub fn map_windows_virtual_key(vk: u32) -> Option<KeyCommand> {
    match vk {
        0x74 => Some(KeyCommand::ToggleDebugOverlay), // F5
        0x20 => Some(KeyCommand::TogglePlayback),     // Space
        0x09 => Some(KeyCommand::NextRoute),          // Tab
        0x31 => Some(KeyCommand::RouteLibrary),       // 1
        0x32 => Some(KeyCommand::RouteNowPlaying),    // 2
        0x33 => Some(KeyCommand::RouteSettings),      // 3
        0x42 => Some(KeyCommand::ToggleSidebar),      // B
        0x51 => Some(KeyCommand::ToggleQueue),        // Q
        0x56 => Some(KeyCommand::CycleVisualMode),    // V
        _ => None,
    }
}

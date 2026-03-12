use crate::platform::input::InputAction;
use crate::runtime::{RuntimeCommand, RuntimeEvent};

#[derive(Debug, Clone)]
pub enum AppAction {
    Bootstrap,
    FramePresented,
    Input(InputAction),
    Runtime(RuntimeEvent),
}

#[derive(Debug, Clone)]
pub enum AppEffect {
    RequestRedraw,
    SendRuntime(RuntimeCommand),
    StartWindowDrag,
    MinimizeWindow,
    ToggleMaximizeWindow,
    CloseWindow,
}

#[derive(Debug, Default)]
pub struct AppUpdate {
    pub effects: Vec<AppEffect>,
}

impl AppUpdate {
    pub fn request_redraw(&mut self) {
        self.effects.push(AppEffect::RequestRedraw);
    }

    pub fn send_runtime(&mut self, command: RuntimeCommand) {
        self.effects.push(AppEffect::SendRuntime(command));
    }

    pub fn start_window_drag(&mut self) {
        self.effects.push(AppEffect::StartWindowDrag);
    }

    pub fn minimize_window(&mut self) {
        self.effects.push(AppEffect::MinimizeWindow);
    }

    pub fn toggle_maximize_window(&mut self) {
        self.effects.push(AppEffect::ToggleMaximizeWindow);
    }

    pub fn close_window(&mut self) {
        self.effects.push(AppEffect::CloseWindow);
    }
}

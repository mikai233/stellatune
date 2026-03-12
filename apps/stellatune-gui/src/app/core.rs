use std::sync::Arc;

use anyhow::Result;
use winit::dpi::PhysicalSize;

use crate::platform::host::WindowHost;
use crate::platform::input::InputAction;
use crate::renderer::{EffectFrame, Renderer, UiFrame};
use crate::runtime::RuntimeServices;
use crate::text::TextSystem;

use super::state::{AppAction, AppEffect, AppState, AppUpdate};

pub struct GuiApp {
    host: Arc<dyn WindowHost>,
    runtime: RuntimeServices,
    renderer: Renderer,
    text_system: TextSystem,
    pub state: AppState,
    close_requested: bool,
}

impl GuiApp {
    pub fn new(host: Arc<dyn WindowHost>, runtime: RuntimeServices, size: PhysicalSize<u32>) -> Result<Self> {
        let renderer = Renderer::new(host.surface_handles(), size)?;
        let text_system = TextSystem::new(size)?;
        let mut state = AppState::new(size);

        runtime.spawn_heartbeat();
        let update = state.reduce(AppAction::Bootstrap);

        let mut app = Self {
            host,
            runtime,
            renderer,
            text_system,
            state,
            close_requested: false,
        };
        app.apply_update(update);
        Ok(app)
    }

    pub fn request_redraw(&self) {
        self.host.request_redraw();
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>) {
        let update = self.state.reduce(AppAction::Input(InputAction::Resized(size)));
        self.text_system.resize(size);
        self.renderer.resize(size);
        self.apply_update(update);
    }

    pub fn draw(&mut self) -> Result<()> {
        self.renderer.draw(&self.state.scene, &mut self.text_system)?;
        let update = self.state.reduce(AppAction::FramePresented);
        self.apply_update(update);
        Ok(())
    }

    pub fn handle_input(&mut self, action: InputAction) {
        if let InputAction::Resized(size) = action {
            self.text_system.resize(size);
            self.renderer.resize(size);
        }
        let update = self.state.reduce(AppAction::Input(action));
        self.apply_update(update);
    }

    pub fn handle_runtime_event(&mut self, event: crate::runtime::RuntimeEvent) {
        let update = self.state.reduce(AppAction::Runtime(event));
        self.apply_update(update);
    }

    pub fn try_recv_runtime_event(
        &mut self,
    ) -> Result<crate::runtime::RuntimeEvent, std::sync::mpsc::TryRecvError> {
        self.runtime.try_recv()
    }

    pub fn needs_continuous_redraw(&self) -> bool {
        self.state.scene.animation_active || self.state.has_pending_redraw
    }

    pub fn close_requested(&self) -> bool {
        self.close_requested
    }

    #[allow(dead_code)]
    pub fn composition_effect_frame(&mut self) -> EffectFrame {
        self.renderer.build_effect_frame(&self.state.scene)
    }

    #[allow(dead_code)]
    pub fn composition_ui_frame(&mut self) -> Result<UiFrame> {
        self.renderer
            .build_ui_frame(&self.state.scene, &mut self.text_system)
    }

    #[allow(dead_code)]
    pub fn frame_presented(&mut self) {
        let update = self.state.reduce(AppAction::FramePresented);
        self.apply_update(update);
    }

    fn apply_update(&mut self, update: AppUpdate) {
        for effect in update.effects {
            match effect {
                AppEffect::RequestRedraw => self.request_redraw(),
                AppEffect::SendRuntime(command) => self.runtime.send(command),
                AppEffect::StartWindowDrag => {
                    let _ = self.host.start_window_drag();
                },
                AppEffect::MinimizeWindow => self.host.minimize(),
                AppEffect::ToggleMaximizeWindow => self.host.toggle_maximize(),
                AppEffect::CloseWindow => {
                    self.close_requested = true;
                },
            }
        }
    }
}

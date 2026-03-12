use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use tracing::info;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowId};

use crate::app::GuiApp;
use crate::platform::host::{SurfaceHandles, WindowHost};
use crate::platform::input::{InputAction, map_window_event};
use crate::platform::window::{build_window_attributes, configure_window, update_window_shape};
use crate::runtime::RuntimeServices;

pub struct GuiShell {
    runtime: RuntimeServices,
    window: Option<Arc<Window>>,
    window_id: Option<WindowId>,
    app: Option<GuiApp>,
    last_frame_at: Instant,
}

impl GuiShell {
    pub fn new(runtime: RuntimeServices) -> Self {
        Self {
            runtime,
            window: None,
            window_id: None,
            app: None,
            last_frame_at: Instant::now(),
        }
    }

    fn initialize_if_needed(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        if self.window.is_some() {
            return Ok(());
        }

        let window = Arc::new(event_loop.create_window(build_window_attributes())?);
        configure_window(window.as_ref());
        let window_id = window.id();
        let host: Arc<dyn WindowHost> = Arc::new(WinitWindowHost {
            window: Arc::clone(&window),
        });
        let app = GuiApp::new(host, self.runtime.clone(), window.inner_size())?;
        app.request_redraw();

        self.window = Some(window);
        self.window_id = Some(window_id);
        self.app = Some(app);
        info!("stellatune-gui initialized");
        Ok(())
    }

    fn app_mut(&mut self, window_id: WindowId) -> Option<&mut GuiApp> {
        if self.window_id == Some(window_id) {
            self.app.as_mut()
        } else {
            None
        }
    }
}

impl ApplicationHandler<()> for GuiShell {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.initialize_if_needed(event_loop) {
            tracing::error!(error = %error, "failed to initialize gui shell");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let resize_from_scale_factor = matches!(event, WindowEvent::ScaleFactorChanged { .. });
        let scaled_size = if resize_from_scale_factor {
            self.window.as_ref().map(|window| window.inner_size())
        } else {
            None
        };
        let Some(app) = self.app_mut(window_id) else {
            return;
        };

        if let Some(action) = map_window_event(&event) {
            if matches!(action, InputAction::CloseRequested) {
                event_loop.exit();
                return;
            }
            app.handle_input(action);
            if app.close_requested() {
                event_loop.exit();
                return;
            }
        }

        match event {
            WindowEvent::RedrawRequested => {
                if let Err(error) = app.draw() {
                    tracing::error!(error = %error, "renderer draw failed");
                    event_loop.exit();
                    return;
                }
                self.last_frame_at = Instant::now();
            },
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(size) = scaled_size {
                    app.resize(size);
                }
            },
            _ => {},
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let Some(app) = self.app.as_mut() else {
            return;
        };

        while let Ok(event) = app.try_recv_runtime_event() {
            app.handle_runtime_event(event);
        }

        if app.close_requested() {
            return;
        }

        if app.needs_continuous_redraw()
            || self.last_frame_at.elapsed() > Duration::from_millis(250)
        {
            app.request_redraw();
        }
    }
}

struct WinitWindowHost {
    window: Arc<Window>,
}

impl WindowHost for WinitWindowHost {
    fn size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    fn request_redraw(&self) {
        self.window.request_redraw();
    }

    fn start_window_drag(&self) -> Result<()> {
        self.window.drag_window()?;
        Ok(())
    }

    fn minimize(&self) {
        self.window.set_minimized(true);
    }

    fn toggle_maximize(&self) {
        self.window.set_maximized(!self.window.is_maximized());
        update_window_shape(self.window.as_ref());
    }

    fn is_maximized(&self) -> bool {
        self.window.is_maximized()
    }

    fn surface_handles(&self) -> SurfaceHandles {
        SurfaceHandles {
            raw_display_handle: self.window.display_handle().expect("winit display handle").as_raw(),
            raw_window_handle: self.window.window_handle().expect("winit window handle").as_raw(),
        }
    }
}

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

use crate::debug::overlay::DebugOverlay;
use crate::gpu::context::GpuContext;
use crate::gpu::frame::FrameTargets;
use crate::navigation::{NavigationState, RouteId};
use crate::page_transition::ResolvedPageTransition;
use crate::render::composer::FrameComposer;
use crate::resources::fonts::{FontCatalog, FontHandle};
use crate::resources::textures::{TextureCatalog, TextureHandle};
use crate::scene::DemoScene;
use crate::ui::transition::UiTransitionResolver;
use crate::view::build_demo_routes;

#[derive(Debug, Clone, Copy)]
pub struct FrameState {
    pub frame_index: u64,
    pub elapsed_seconds: f32,
    pub delta_seconds: f32,
    pub smoothed_fps: f32,
    pub physical_size: PhysicalSize<u32>,
    pub scale_factor: f64,
}

#[derive(Debug)]
pub enum RenderFrameError {
    Surface(wgpu::SurfaceError),
    Fatal(anyhow::Error),
}

impl From<wgpu::SurfaceError> for RenderFrameError {
    fn from(value: wgpu::SurfaceError) -> Self {
        Self::Surface(value)
    }
}

impl From<anyhow::Error> for RenderFrameError {
    fn from(value: anyhow::Error) -> Self {
        Self::Fatal(value)
    }
}

struct FrameClock {
    started_at: Instant,
    last_frame_at: Instant,
    frame_index: u64,
}

impl FrameClock {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            started_at: now,
            last_frame_at: now,
            frame_index: 0,
        }
    }

    fn tick(
        &mut self,
        smoothed_fps: f32,
        physical_size: PhysicalSize<u32>,
        scale_factor: f64,
    ) -> FrameState {
        let now = Instant::now();
        let delta_seconds = now
            .saturating_duration_since(self.last_frame_at)
            .as_secs_f32()
            .max(1.0 / 10_000.0);
        let elapsed_seconds = now.saturating_duration_since(self.started_at).as_secs_f32();
        self.last_frame_at = now;
        self.frame_index = self.frame_index.saturating_add(1);
        FrameState {
            frame_index: self.frame_index,
            elapsed_seconds,
            delta_seconds,
            smoothed_fps,
            physical_size,
            scale_factor,
        }
    }
}

pub struct RenderApp {
    window: Arc<Window>,
    gpu: GpuContext,
    targets: FrameTargets,
    composer: FrameComposer,
    fonts: FontCatalog,
    ui_font: FontHandle,
    textures: TextureCatalog,
    demo_cover_texture: TextureHandle,
    scene: DemoScene,
    navigation: NavigationState,
    transitions: UiTransitionResolver,
    overlay: DebugOverlay,
    clock: FrameClock,
}

impl RenderApp {
    async fn new(window: Arc<Window>) -> Result<Self> {
        let gpu = GpuContext::new(Arc::clone(&window))
            .await
            .context("initialize GPU context")?;
        let targets = FrameTargets::new(&gpu.device, gpu.surface_size());
        let composer = FrameComposer::new(&gpu.device, gpu.surface_format())
            .context("create frame composer")?;
        let mut fonts = FontCatalog::default();
        let ui_font = fonts.load_ui_font().context("load UI font")?;
        let mut textures = TextureCatalog::default();
        let demo_cover_texture = textures
            .create_demo_cover(&gpu.device, &gpu.queue)
            .context("create demo cover texture")?;
        Ok(Self {
            window,
            gpu,
            targets,
            composer,
            fonts,
            ui_font,
            textures,
            demo_cover_texture,
            scene: DemoScene::new(),
            navigation: NavigationState::default(),
            transitions: UiTransitionResolver::default(),
            overlay: DebugOverlay::default(),
            clock: FrameClock::new(),
        })
    }

    fn window(&self) -> &Arc<Window> {
        &self.window
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.gpu.resize(new_size);
        self.targets
            .resize(&self.gpu.device, self.gpu.surface_size());
    }

    fn update_scale_factor(&mut self, scale_factor: f64) {
        self.gpu.set_scale_factor(scale_factor);
    }

    fn navigate_to(&mut self, route: RouteId) {
        let elapsed = self.clock.started_at.elapsed().as_secs_f32();
        self.navigation.navigate_to(route, elapsed);
    }

    fn pop_route(&mut self) {
        let elapsed = self.clock.started_at.elapsed().as_secs_f32();
        let _ = self.navigation.pop(elapsed);
    }

    fn toggle_route(&mut self) {
        let elapsed = self.clock.started_at.elapsed().as_secs_f32();
        self.navigation.toggle_demo_route(elapsed);
    }

    fn render(&mut self) -> std::result::Result<(), RenderFrameError> {
        if self.gpu.is_zero_sized() {
            return Ok(());
        }

        let smoothed_fps = self.overlay.observe_frame(Instant::now());
        let frame = self.clock.tick(
            smoothed_fps,
            self.gpu.surface_size(),
            self.gpu.scale_factor(),
        );
        self.navigation.update_demo_timeline(frame.elapsed_seconds);
        let demo_cover = self
            .textures
            .get(self.demo_cover_texture)
            .expect("demo cover texture should be available");
        let metadata = demo_cover.metadata();
        debug_assert_eq!(metadata.format, wgpu::TextureFormat::Rgba8UnormSrgb);
        let ui_font = self
            .fonts
            .get(self.ui_font)
            .expect("UI font should be available");
        let active_transition = self.navigation.active_transition(frame.elapsed_seconds);
        let page_transition = active_transition
            .map(|transition| {
                transition.preset().resolve(
                    Some(transition),
                    frame.physical_size,
                    frame.elapsed_seconds,
                )
            })
            .unwrap_or_else(ResolvedPageTransition::identity);
        let route_views = build_demo_routes(
            self.navigation.top_route(),
            active_transition,
            &frame,
            ui_font.family_name(),
        );
        let resolved_ui = self.transitions.resolve_navigation(
            route_views.source.as_ref(),
            &route_views.destination,
            active_transition,
            frame.elapsed_seconds,
        );
        let scene_frame = self.scene.rebuild(
            resolved_ui.source.as_ref(),
            &resolved_ui.destination,
            &resolved_ui.plan,
            ui_font,
        );
        self.composer
            .render(
                &self.gpu,
                &self.targets,
                scene_frame,
                demo_cover,
                &frame,
                page_transition,
            )
            .map_err(RenderFrameError::from)
    }
}

#[derive(Default)]
pub struct StellatuneGuiApp {
    render_app: Option<RenderApp>,
    window_id: Option<WindowId>,
}

impl StellatuneGuiApp {
    fn bootstrap_window(event_loop: &ActiveEventLoop) -> Result<Arc<Window>> {
        let attributes: WindowAttributes = Window::default_attributes()
            .with_title("StellaTune GUI Experimental")
            .with_inner_size(LogicalSize::new(1440.0, 920.0))
            .with_min_inner_size(LogicalSize::new(960.0, 640.0));
        let window = event_loop
            .create_window(attributes)
            .context("create GUI window")?;
        Ok(Arc::new(window))
    }

    fn handle_render_error(&mut self, event_loop: &ActiveEventLoop, error: RenderFrameError) {
        match error {
            RenderFrameError::Surface(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                if let Some(app) = self.render_app.as_mut() {
                    let size = app.window().inner_size();
                    app.resize(size);
                }
            },
            RenderFrameError::Surface(wgpu::SurfaceError::OutOfMemory) => {
                tracing::error!("surface out of memory; exiting event loop");
                event_loop.exit();
            },
            RenderFrameError::Surface(wgpu::SurfaceError::Timeout) => {
                tracing::warn!("surface acquisition timed out");
            },
            RenderFrameError::Surface(wgpu::SurfaceError::Other) => {
                tracing::warn!("surface reported an unknown error");
            },
            RenderFrameError::Fatal(error) => {
                tracing::error!(error = %error, "fatal render failure");
                event_loop.exit();
            },
        }
    }
}

impl ApplicationHandler for StellatuneGuiApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.render_app.is_some() {
            return;
        }

        match Self::bootstrap_window(event_loop).and_then(|window| {
            let id = window.id();
            let render_app = pollster::block_on(RenderApp::new(window))?;
            Ok((id, render_app))
        }) {
            Ok((window_id, render_app)) => {
                render_app.window().request_redraw();
                self.window_id = Some(window_id);
                self.render_app = Some(render_app);
            },
            Err(error) => {
                tracing::error!(error = %error, "failed to initialize experimental GUI app");
                event_loop.exit();
            },
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if self.window_id != Some(window_id) {
            return;
        }

        let Some(app) = self.render_app.as_mut() else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                app.resize(new_size);
                app.window().request_redraw();
            },
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                app.update_scale_factor(scale_factor);
                app.resize(app.window().inner_size());
                app.window().request_redraw();
            },
            WindowEvent::KeyboardInput { event, .. }
                if matches!(event.logical_key, Key::Named(NamedKey::Escape)) =>
            {
                event_loop.exit();
            },
            WindowEvent::KeyboardInput { event, .. }
                if matches!(event.logical_key, Key::Named(NamedKey::Space)) =>
            {
                app.toggle_route();
                app.window().request_redraw();
            },
            WindowEvent::KeyboardInput { event, .. }
                if matches!(event.logical_key, Key::Named(NamedKey::Backspace)) =>
            {
                app.pop_route();
                app.window().request_redraw();
            },
            WindowEvent::KeyboardInput { event, .. } => match &event.logical_key {
                Key::Character(value) if value == "1" => {
                    app.navigate_to(RouteId::Library);
                    app.window().request_redraw();
                },
                Key::Character(value) if value == "2" => {
                    app.navigate_to(RouteId::HeroDemo);
                    app.window().request_redraw();
                },
                _ => {},
            },
            WindowEvent::RedrawRequested => {
                let redraw_window = Arc::clone(app.window());
                let render_result = app.render();
                redraw_window.request_redraw();
                if let Err(error) = render_result {
                    self.handle_render_error(event_loop, error);
                }
            },
            _ => {},
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(app) = self.render_app.as_ref() {
            app.window().request_redraw();
        }
    }
}

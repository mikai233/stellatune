mod app;
mod debug;
mod gpu;
mod render;
mod resources;
mod scene;
mod view;

use anyhow::Result;
use tracing_subscriber::EnvFilter;
use winit::event_loop::{ControlFlow, EventLoop};

fn main() -> Result<()> {
    init_tracing();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = app::StellatuneGuiApp::default();
    event_loop.run_app(&mut app)?;
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,wgpu=warn,naga=warn"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

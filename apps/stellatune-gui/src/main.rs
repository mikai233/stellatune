mod app;
mod platform;
mod renderer;
mod runtime;
mod scene;
mod text;

use anyhow::Result;
use tracing_subscriber::EnvFilter;
#[cfg(not(target_os = "windows"))]
use app::GuiShell;

fn main() -> Result<()> {
    init_tracing();

    let runtime = runtime::RuntimeServices::new()?;

    #[cfg(target_os = "windows")]
    {
        return platform::windows::run(runtime);
    }

    #[cfg(not(target_os = "windows"))]
    {
    let event_loop = winit::event_loop::EventLoop::new()?;
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Wait);

    let mut shell = GuiShell::new(runtime);
    event_loop.run_app(&mut shell)?;
    Ok(())
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

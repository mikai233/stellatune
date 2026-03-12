#![allow(dead_code)]

use anyhow::Result;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use winit::dpi::PhysicalSize;

#[derive(Debug, Clone, Copy)]
pub struct SurfaceHandles {
    pub raw_display_handle: RawDisplayHandle,
    pub raw_window_handle: RawWindowHandle,
}

pub trait WindowHost {
    fn size(&self) -> PhysicalSize<u32>;
    fn request_redraw(&self);
    fn start_window_drag(&self) -> Result<()>;
    fn minimize(&self);
    fn toggle_maximize(&self);
    fn is_maximized(&self) -> bool;
    fn surface_handles(&self) -> SurfaceHandles;
}

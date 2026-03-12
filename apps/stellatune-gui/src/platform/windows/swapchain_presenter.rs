#![allow(dead_code)]

use super::presenter::{WindowsPresenterKind, WindowsPresenterProfile};

#[derive(Debug, Clone)]
pub struct SwapchainPresenterConfig {
    pub opaque_surface: bool,
    pub composition_clip_in_ui: bool,
    pub enables_heavy_gpu_effects: bool,
}

impl Default for SwapchainPresenterConfig {
    fn default() -> Self {
        Self {
            opaque_surface: true,
            composition_clip_in_ui: true,
            enables_heavy_gpu_effects: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SwapchainPresenterPlan {
    pub profile: WindowsPresenterProfile,
    pub notes: &'static [&'static str],
}

impl SwapchainPresenterPlan {
    pub fn current() -> Self {
        Self {
            profile: WindowsPresenterProfile::swapchain_surface(),
            notes: &[
                "current Windows runtime path",
                "true heavy GPU effects are allowed",
                "window stays rectangular; rounded shell is drawn by UI",
            ],
        }
    }
}

pub fn presenter_kind() -> WindowsPresenterKind {
    WindowsPresenterKind::SwapchainSurface
}

#![allow(dead_code)]

use super::composition::{WindowsCompositionConfig, WindowsCompositionHost, WindowsCompositionPlan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsPresenterKind {
    SwapchainSurface,
    LayeredWindow,
    CompositionVisualTree,
}

#[derive(Debug, Clone)]
pub struct WindowsPresenterProfile {
    pub kind: WindowsPresenterKind,
    pub supports_per_pixel_alpha: bool,
    pub supports_true_window_rounding: bool,
    pub supports_heavy_gpu_effects: bool,
}

impl WindowsPresenterProfile {
    pub fn swapchain_surface() -> Self {
        Self {
            kind: WindowsPresenterKind::SwapchainSurface,
            supports_per_pixel_alpha: false,
            supports_true_window_rounding: false,
            supports_heavy_gpu_effects: true,
        }
    }

    pub fn layered_window() -> Self {
        Self {
            kind: WindowsPresenterKind::LayeredWindow,
            supports_per_pixel_alpha: true,
            supports_true_window_rounding: true,
            supports_heavy_gpu_effects: false,
        }
    }

    pub fn composition_visual_tree() -> Self {
        Self {
            kind: WindowsPresenterKind::CompositionVisualTree,
            supports_per_pixel_alpha: true,
            supports_true_window_rounding: true,
            supports_heavy_gpu_effects: true,
        }
    }
}

#[derive(Debug)]
pub struct WindowsPresenterSkeleton {
    pub active: WindowsPresenterProfile,
    pub target: WindowsPresenterProfile,
}

impl WindowsPresenterSkeleton {
    pub fn new() -> Self {
        Self {
            active: WindowsPresenterProfile::swapchain_surface(),
            target: WindowsPresenterProfile::composition_visual_tree(),
        }
    }

    pub fn composition_plan(
        &self,
        host: &WindowsCompositionHost,
        config: &WindowsCompositionConfig,
    ) -> WindowsCompositionPlan {
        host.plan(config)
    }
}

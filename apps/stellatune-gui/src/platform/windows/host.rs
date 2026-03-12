#![allow(dead_code)]

use anyhow::Result;
use tracing::{info, warn};

use crate::runtime::RuntimeServices;

use super::composition::{WindowsCompositionConfig, WindowsCompositionHost, WindowsCompositionPlan};
use super::composition_presenter::{CompositionPresenterConfig, CompositionPresenterRuntime};
use super::presenter::{WindowsPresenterKind, WindowsPresenterProfile, WindowsPresenterSkeleton};

pub fn run(runtime: RuntimeServices) -> Result<()> {
    super::super::windows_host::run(runtime)
}

pub fn active_presenter_profile() -> WindowsPresenterProfile {
    WindowsPresenterProfile::swapchain_surface()
}

pub fn target_presenter_profile() -> WindowsPresenterProfile {
    WindowsPresenterProfile::composition_visual_tree()
}

pub fn presenter_skeleton() -> WindowsPresenterSkeleton {
    WindowsPresenterSkeleton::new()
}

pub fn composition_bootstrap_plan() -> WindowsCompositionPlan {
    let host = WindowsCompositionHost::new();
    let config = WindowsCompositionConfig::default();
    host.plan(&config)
}

pub fn active_presenter_kind() -> WindowsPresenterKind {
    active_presenter_profile().kind
}

pub fn bootstrap_composition_runtime(hwnd: isize) -> Option<CompositionPresenterRuntime> {
    match CompositionPresenterRuntime::bootstrap(hwnd, &CompositionPresenterConfig::default()) {
        Ok(runtime) => {
            info!(
                ?runtime.backdrop_type,
                ?runtime.corner_preference,
                "composition bootstrap initialized"
            );
            Some(runtime)
        },
        Err(error) => {
            warn!(error = %error, "composition bootstrap failed; keeping swapchain presenter active");
            None
        },
    }
}

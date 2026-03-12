#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicIsize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionVisualKind {
    Root,
    Backdrop,
    Effects,
    Ui,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsCompositionPhase {
    Uninitialized,
    DeviceBootstrap,
    VisualTreeBootstrap,
    Running,
}

#[derive(Debug, Clone)]
pub struct WindowsCompositionConfig {
    pub rounded_clip_radius: f32,
    pub backdrop_enabled: bool,
    pub acrylic_requested: bool,
    pub effect_layer_enabled: bool,
}

impl Default for WindowsCompositionConfig {
    fn default() -> Self {
        Self {
            rounded_clip_radius: 18.0,
            backdrop_enabled: true,
            acrylic_requested: true,
            effect_layer_enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct WindowsCompositionPlan {
    pub phase: WindowsCompositionPhase,
    pub visuals: Vec<CompositionVisualKind>,
    pub uses_gpu_presenter: bool,
    pub notes: &'static [&'static str],
}

impl WindowsCompositionPlan {
    pub fn bootstrap() -> Self {
        Self {
            phase: WindowsCompositionPhase::Uninitialized,
            visuals: vec![
                CompositionVisualKind::Root,
                CompositionVisualKind::Backdrop,
                CompositionVisualKind::Effects,
                CompositionVisualKind::Ui,
                CompositionVisualKind::Overlay,
            ],
            uses_gpu_presenter: true,
            notes: &[
                "target architecture for Windows is composition-first",
                "rounded clip belongs to the visual tree, not SetWindowRgn",
                "heavy shader effects should render into a GPU composition layer",
                "UI and FX should converge before final composition present",
            ],
        }
    }
}

#[derive(Debug, Default)]
pub struct WindowsCompositionHost {
    hwnd: AtomicIsize,
    dcomp_ready: AtomicBool,
}

impl WindowsCompositionHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach_hwnd(&self, hwnd: isize) {
        self.hwnd.store(hwnd, std::sync::atomic::Ordering::Release);
    }

    pub fn mark_ready(&self, ready: bool) {
        self.dcomp_ready
            .store(ready, std::sync::atomic::Ordering::Release);
    }

    pub fn is_ready(&self) -> bool {
        self.dcomp_ready
            .load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd.load(std::sync::atomic::Ordering::Acquire)
    }

    pub fn plan(&self, config: &WindowsCompositionConfig) -> WindowsCompositionPlan {
        let mut plan = WindowsCompositionPlan::bootstrap();
        plan.phase = if self.is_ready() {
            WindowsCompositionPhase::Running
        } else {
            WindowsCompositionPhase::DeviceBootstrap
        };
        if !config.backdrop_enabled {
            plan.visuals
                .retain(|kind| *kind != CompositionVisualKind::Backdrop);
        }
        if !config.effect_layer_enabled {
            plan.visuals
                .retain(|kind| *kind != CompositionVisualKind::Effects);
        }
        plan
    }
}

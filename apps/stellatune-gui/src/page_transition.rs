use winit::dpi::PhysicalSize;

use crate::navigation::{NavigationOperation, RouteTransition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageTransitionPreset {
    OpaqueCover,
    Crossfade,
    FadeThrough,
    SharedAxisZ,
}

#[derive(Debug, Clone, Copy)]
pub struct PageTransitionContext {
    pub operation: NavigationOperation,
    pub progress: f32,
    pub viewport: PhysicalSize<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct PageSurfacePlan {
    pub opacity: f32,
    pub translation_uv: [f32; 2],
    pub scale: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct ResolvedPageTransition {
    pub source: PageSurfacePlan,
    pub destination: PageSurfacePlan,
    pub media_on_top: bool,
}

pub trait PageTransitionSpec {
    fn resolve(&self, ctx: &PageTransitionContext) -> ResolvedPageTransition;
}

#[derive(Debug, Clone, Copy)]
pub struct OpaqueCoverTransition;

#[derive(Debug, Clone, Copy)]
pub struct CrossfadeTransition;

#[derive(Debug, Clone, Copy)]
pub struct FadeThroughTransition;

#[derive(Debug, Clone, Copy)]
pub struct SharedAxisZTransition;

impl PageTransitionPreset {
    pub fn resolve(
        self,
        transition: Option<RouteTransition>,
        viewport: PhysicalSize<u32>,
        now_seconds: f32,
    ) -> ResolvedPageTransition {
        let Some(transition) = transition else {
            return ResolvedPageTransition::identity();
        };

        let ctx = PageTransitionContext {
            operation: transition.operation(),
            progress: ((now_seconds - transition.started_at) / transition.duration_seconds)
                .clamp(0.0, 1.0),
            viewport,
        };

        match self {
            Self::OpaqueCover => OpaqueCoverTransition.resolve(&ctx),
            Self::Crossfade => CrossfadeTransition.resolve(&ctx),
            Self::FadeThrough => FadeThroughTransition.resolve(&ctx),
            Self::SharedAxisZ => SharedAxisZTransition.resolve(&ctx),
        }
    }
}

impl ResolvedPageTransition {
    pub const fn identity() -> Self {
        Self {
            source: PageSurfacePlan {
                opacity: 0.0,
                translation_uv: [0.0, 0.0],
                scale: 1.0,
            },
            destination: PageSurfacePlan {
                opacity: 1.0,
                translation_uv: [0.0, 0.0],
                scale: 1.0,
            },
            media_on_top: false,
        }
    }
}

impl PageTransitionSpec for OpaqueCoverTransition {
    fn resolve(&self, ctx: &PageTransitionContext) -> ResolvedPageTransition {
        let progress = ctx.progress.clamp(0.0, 1.0);
        let slide = match ctx.operation {
            NavigationOperation::Push => 0.02 * (1.0 - progress),
            NavigationOperation::Pop => -0.02 * (1.0 - progress),
            NavigationOperation::Replace => 0.0,
        };

        ResolvedPageTransition {
            source: PageSurfacePlan {
                opacity: 0.0,
                translation_uv: [0.0, 0.0],
                scale: 1.0,
            },
            destination: PageSurfacePlan {
                opacity: 1.0,
                translation_uv: [slide, 0.0],
                scale: 1.0,
            },
            media_on_top: false,
        }
    }
}

impl PageTransitionSpec for CrossfadeTransition {
    fn resolve(&self, ctx: &PageTransitionContext) -> ResolvedPageTransition {
        let progress = ctx.progress.clamp(0.0, 1.0);
        let source_opacity = 1.0 - progress;
        let destination_opacity = progress;
        let vertical_hint =
            (ctx.viewport.height.max(1) as f32 / ctx.viewport.width.max(1) as f32).clamp(0.5, 1.5);

        ResolvedPageTransition {
            source: PageSurfacePlan {
                opacity: source_opacity,
                translation_uv: [0.0, 0.012 * progress / vertical_hint],
                scale: 1.0,
            },
            destination: PageSurfacePlan {
                opacity: destination_opacity,
                translation_uv: [0.0, -0.012 * (1.0 - progress) / vertical_hint],
                scale: 1.0,
            },
            media_on_top: false,
        }
    }
}

impl PageTransitionSpec for FadeThroughTransition {
    fn resolve(&self, ctx: &PageTransitionContext) -> ResolvedPageTransition {
        let progress = ctx.progress.clamp(0.0, 1.0);
        let fade_out = ((0.42 - progress) / 0.42).clamp(0.0, 1.0);
        let fade_in = ((progress - 0.28) / 0.72).clamp(0.0, 1.0);
        let destination_scale = 0.92 + 0.08 * ease_out_cubic(fade_in);

        ResolvedPageTransition {
            source: PageSurfacePlan {
                opacity: fade_out,
                translation_uv: [0.0, 0.0],
                scale: 1.0,
            },
            destination: PageSurfacePlan {
                opacity: fade_in,
                translation_uv: [0.0, 0.0],
                scale: destination_scale,
            },
            media_on_top: false,
        }
    }
}

impl PageTransitionSpec for SharedAxisZTransition {
    fn resolve(&self, ctx: &PageTransitionContext) -> ResolvedPageTransition {
        let progress = ease_in_out_cubic(ctx.progress.clamp(0.0, 1.0));
        let direction = match ctx.operation {
            NavigationOperation::Push => 1.0,
            NavigationOperation::Pop => -1.0,
            NavigationOperation::Replace => 1.0,
        };
        let depth_hint =
            (ctx.viewport.width.max(1) as f32 / ctx.viewport.height.max(1) as f32).clamp(0.7, 1.6);
        let source_scale = 1.0 - 0.08 * progress;
        let destination_scale = 1.08 - 0.08 * progress;
        let travel = 0.015 / depth_hint;

        ResolvedPageTransition {
            source: PageSurfacePlan {
                opacity: 1.0 - progress,
                translation_uv: [0.0, direction * travel * progress * 0.35],
                scale: source_scale,
            },
            destination: PageSurfacePlan {
                opacity: progress,
                translation_uv: [0.0, -direction * travel * (1.0 - progress)],
                scale: destination_scale,
            },
            media_on_top: false,
        }
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in_out_cubic(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) * 0.5
    }
}

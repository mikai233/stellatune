mod text;

use vello::Scene;
use vello::kurbo::{Affine, Rect, RoundedRect, Stroke};
use vello::peniko::{Color, Fill};

use crate::resources::fonts::FontResource;
use crate::scene::text::draw_simple_line;
use crate::ui::node::{NodeId, UiEffectHint, UiLayer, UiNode, UiNodeContent};
use crate::ui::transition::{
    ResolvedTransitionOverlayEntry, TransitionOverlayPhase, UiTransitionPlan,
};

#[derive(Clone, Copy)]
pub struct DemoSceneFrame<'a> {
    pub source_scene: &'a Scene,
    pub destination_scene: &'a Scene,
    pub cover_rect: Rect,
    pub cover_layer: UiLayer,
}

#[derive(Default)]
pub struct DemoScene {
    source_scene: Scene,
    destination_scene: Scene,
}

impl DemoScene {
    pub fn new() -> Self {
        Self {
            source_scene: Scene::new(),
            destination_scene: Scene::new(),
        }
    }

    pub fn rebuild(
        &mut self,
        source_root: Option<&UiNode>,
        destination_root: &UiNode,
        transition_plan: &UiTransitionPlan,
        ui_font: &FontResource,
    ) -> DemoSceneFrame<'_> {
        self.source_scene.reset();
        self.destination_scene.reset();

        if let Some(source_root) = source_root {
            render_tree_into(
                &mut self.source_scene,
                source_root,
                &UiTransitionPlan::default(),
                ui_font,
            );
        }
        render_tree_into(
            &mut self.destination_scene,
            destination_root,
            transition_plan,
            ui_font,
        );

        let (cover_rect, cover_layer) =
            if let Some(overlay) = transition_plan.primary_shared_media_overlay() {
                debug_assert!(
                    transition_plan
                        .shared_media_slot_for_overlay(overlay)
                        .is_some(),
                    "shared media overlay should resolve back to its source slot"
                );
                (overlay.rect(), UiLayer::Overlay)
            } else {
                transition_plan
                    .primary_shared_media_slot()
                    .map(|slot| (slot.rect(), slot.layer()))
                    .unwrap_or((Rect::new(0.0, 0.0, 1.0, 1.0), UiLayer::Content))
            };

        DemoSceneFrame {
            source_scene: &self.source_scene,
            destination_scene: &self.destination_scene,
            cover_rect,
            cover_layer,
        }
    }
}

fn render_tree_into(
    scene: &mut Scene,
    root: &UiNode,
    transition_plan: &UiTransitionPlan,
    ui_font: &FontResource,
) {
    for layer in [UiLayer::Background, UiLayer::Content, UiLayer::Overlay] {
        render_layer(
            scene,
            root,
            transition_plan,
            ui_font,
            layer,
            Affine::IDENTITY,
            1.0,
        );
    }
    render_transition_overlays(scene, root, transition_plan, ui_font);
}

fn render_layer(
    scene: &mut Scene,
    node: &UiNode,
    transition_plan: &UiTransitionPlan,
    ui_font: &FontResource,
    layer: UiLayer,
    parent_transform: Affine,
    parent_opacity: f32,
) {
    let transform = parent_transform * node.transform;
    let opacity = parent_opacity * node.opacity;

    if node.layer == layer && !transition_plan.is_promoted_node(node.id) {
        render_node(scene, node, ui_font, transform, opacity);
    }

    for child in &node.children {
        render_layer(
            scene,
            child,
            transition_plan,
            ui_font,
            layer,
            transform,
            opacity,
        );
    }
}

fn render_transition_overlays(
    scene: &mut Scene,
    root: &UiNode,
    transition_plan: &UiTransitionPlan,
    ui_font: &FontResource,
) {
    for overlay in transition_plan.promoted_node_overlays() {
        debug_assert!(
            render_promoted_node(
                scene,
                root,
                overlay.source_id(),
                ui_font,
                Affine::IDENTITY,
                1.0
            ),
            "promoted overlay should resolve back to a source node"
        );
        render_overlay_decoration(scene, overlay);
    }
}

fn render_overlay_decoration(scene: &mut Scene, overlay: &ResolvedTransitionOverlayEntry) {
    let stroke_width = 1.0 + overlay.progress() as f64 * 1.2;
    let stroke_color = match overlay.phase() {
        TransitionOverlayPhase::Promoting => Color::from_rgba8(255, 248, 239, 120),
        TransitionOverlayPhase::Settling => Color::from_rgba8(246, 196, 72, 82),
    };
    let glow_color = match overlay.phase() {
        TransitionOverlayPhase::Promoting => Color::from_rgba8(255, 245, 232, 34),
        TransitionOverlayPhase::Settling => Color::from_rgba8(246, 196, 72, 22),
    };
    let glow = RoundedRect::from_rect(overlay.rect().inset(10.0), 999.0);
    let outline = RoundedRect::from_rect(overlay.rect(), 999.0);
    scene.fill(Fill::NonZero, Affine::IDENTITY, glow_color, None, &glow);
    scene.stroke(
        &Stroke::new(stroke_width),
        Affine::IDENTITY,
        stroke_color,
        None,
        &outline,
    );
}

fn render_promoted_node(
    scene: &mut Scene,
    node: &UiNode,
    target_id: NodeId,
    ui_font: &FontResource,
    parent_transform: Affine,
    parent_opacity: f32,
) -> bool {
    let transform = parent_transform * node.transform;
    let opacity = parent_opacity * node.opacity;

    if node.id == target_id {
        render_node(scene, node, ui_font, transform, opacity);
        return true;
    }

    for child in &node.children {
        if render_promoted_node(scene, child, target_id, ui_font, transform, opacity) {
            return true;
        }
    }

    false
}

fn render_node(
    scene: &mut Scene,
    node: &UiNode,
    ui_font: &FontResource,
    transform: Affine,
    opacity: f32,
) {
    debug_assert!(!node.id.0.is_empty());
    match &node.content {
        UiNodeContent::Group | UiNodeContent::MediaSlot { .. } => {},
        UiNodeContent::RoundedRect { fill, stroke, .. } => {
            let Some(shape) = node.as_rounded_rect() else {
                return;
            };
            if let Some(fill) = fill {
                scene.fill(
                    Fill::NonZero,
                    transform,
                    fill.with_alpha(opacity),
                    None,
                    &shape,
                );
            }
            if let Some(stroke) = stroke {
                scene.stroke(
                    &Stroke::new(stroke.width),
                    transform,
                    stroke.color.with_alpha(opacity),
                    None,
                    &shape,
                );
            }
        },
        UiNodeContent::Circle { fill, stroke, .. } => {
            let Some(shape) = node.as_circle() else {
                return;
            };
            if let Some(fill) = fill {
                scene.fill(
                    Fill::NonZero,
                    transform,
                    fill.with_alpha(opacity),
                    None,
                    &shape,
                );
            }
            if let Some(stroke) = stroke {
                scene.stroke(
                    &Stroke::new(stroke.width),
                    transform,
                    stroke.color.with_alpha(opacity),
                    None,
                    &shape,
                );
            }
        },
        UiNodeContent::Text {
            origin,
            text,
            font_size,
            color,
        } => {
            let metrics = draw_simple_line(
                scene,
                ui_font,
                text,
                *font_size,
                *origin,
                color.with_alpha(opacity),
                transform,
            );
            if let Some(metrics) = metrics {
                render_text_effect(scene, node, *origin, metrics, transform, opacity);
            }
        },
    }
}

fn render_text_effect(
    scene: &mut Scene,
    node: &UiNode,
    origin: (f32, f32),
    metrics: text::TextLayoutMetrics,
    transform: Affine,
    opacity: f32,
) {
    match node.effect_hint {
        Some(UiEffectHint::Halo) => {
            let glow = RoundedRect::from_rect(
                Rect::new(
                    origin.0 as f64,
                    origin.1 as f64 + metrics.height as f64 + 12.0,
                    origin.0 as f64 + metrics.width as f64 * 0.42,
                    origin.1 as f64 + metrics.height as f64 + 18.0,
                ),
                999.0,
            );
            scene.fill(
                Fill::NonZero,
                transform,
                Color::from_rgba8(255, 248, 239, 34).with_alpha(opacity),
                None,
                &glow,
            );
        },
        Some(UiEffectHint::Underline) => {
            let underline = RoundedRect::from_rect(
                Rect::new(
                    origin.0 as f64,
                    origin.1 as f64 + metrics.height as f64 + 6.0,
                    origin.0 as f64 + metrics.width as f64 * 0.36,
                    origin.1 as f64 + metrics.height as f64 + 10.0,
                ),
                999.0,
            );
            scene.fill(
                Fill::NonZero,
                transform,
                Color::from_rgba8(246, 196, 72, 210).with_alpha(opacity),
                None,
                &underline,
            );
        },
        Some(UiEffectHint::OutlineTag) => {
            let chip = RoundedRect::from_rect(
                Rect::new(
                    origin.0 as f64 - 12.0,
                    origin.1 as f64 - metrics.height as f64 + 6.0,
                    origin.0 as f64 + metrics.width as f64 + 12.0,
                    origin.1 as f64 + 8.0,
                ),
                999.0,
            );
            scene.stroke(
                &Stroke::new(1.2),
                transform,
                Color::from_rgba8(255, 255, 255, 36).with_alpha(opacity),
                None,
                &chip,
            );
        },
        Some(UiEffectHint::PromoteSurface) | None => {},
    }
}

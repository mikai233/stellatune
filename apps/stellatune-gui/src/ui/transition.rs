use std::collections::HashMap;

use vello::kurbo::{Affine, Point, Rect};

use crate::navigation::RouteTransition;
use crate::ui::node::{NodeId, UiEffectHint, UiLayer, UiNode, UiNodeContent, UiTransitionHint};

const TRANSITION_DURATION_SECONDS: f32 = 0.28;
const LAYOUT_EPSILON: f64 = 0.5;
const OPACITY_EPSILON: f32 = 0.01;

#[derive(Debug, Clone, Copy)]
struct NodeSnapshot {
    bounds: Option<Rect>,
    opacity: f32,
    transition_hint: Option<UiTransitionHint>,
}

#[derive(Debug, Clone, Copy)]
struct ActiveTransition {
    from_bounds: Rect,
    to_bounds: Rect,
    from_opacity: f32,
    to_opacity: f32,
    started_at: f32,
    hint: UiTransitionHint,
}

#[derive(Debug, Default)]
pub struct UiTransitionResolver {
    previous: HashMap<NodeId, NodeSnapshot>,
    active: HashMap<NodeId, ActiveTransition>,
}

#[derive(Debug, Clone)]
pub struct ResolvedSharedMediaSlot {
    id: NodeId,
    rect: Rect,
    layer: UiLayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionOverlayKind {
    SharedMediaSurface,
    PromotedNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionOverlayPhase {
    Promoting,
    Settling,
}

#[derive(Debug, Clone)]
pub struct ResolvedTransitionOverlayEntry {
    kind: TransitionOverlayKind,
    source_id: NodeId,
    rect: Rect,
    progress: f32,
    phase: TransitionOverlayPhase,
}

#[derive(Debug, Default, Clone)]
pub struct UiTransitionPlan {
    pub shared_media_slots: Vec<ResolvedSharedMediaSlot>,
    pub overlay_entries: Vec<ResolvedTransitionOverlayEntry>,
}

impl UiTransitionPlan {
    pub fn primary_shared_media_slot(&self) -> Option<&ResolvedSharedMediaSlot> {
        self.shared_media_slots.first()
    }

    pub fn primary_shared_media_overlay(&self) -> Option<&ResolvedTransitionOverlayEntry> {
        self.overlay_entries
            .iter()
            .find(|entry| entry.kind == TransitionOverlayKind::SharedMediaSurface)
    }

    pub fn promoted_node_overlays(&self) -> impl Iterator<Item = &ResolvedTransitionOverlayEntry> {
        self.overlay_entries
            .iter()
            .filter(|entry| entry.kind == TransitionOverlayKind::PromotedNode)
    }

    pub fn shared_media_slot_for_overlay(
        &self,
        overlay: &ResolvedTransitionOverlayEntry,
    ) -> Option<&ResolvedSharedMediaSlot> {
        self.shared_media_slots
            .iter()
            .find(|slot| slot.id == overlay.source_id)
    }

    pub fn is_promoted_node(&self, id: NodeId) -> bool {
        self.overlay_entries
            .iter()
            .any(|entry| entry.kind == TransitionOverlayKind::PromotedNode && entry.source_id == id)
    }
}

impl ResolvedSharedMediaSlot {
    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn layer(&self) -> UiLayer {
        self.layer
    }
}

impl ResolvedTransitionOverlayEntry {
    pub fn source_id(&self) -> NodeId {
        self.source_id
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn progress(&self) -> f32 {
        self.progress
    }

    pub fn phase(&self) -> TransitionOverlayPhase {
        self.phase
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedUiTree {
    pub source: Option<UiNode>,
    pub destination: UiNode,
    pub plan: UiTransitionPlan,
}

impl UiTransitionResolver {
    pub fn resolve(&mut self, root: &UiNode, now_seconds: f32) -> ResolvedUiTree {
        let current = collect_snapshots(root, Affine::IDENTITY, 1.0);
        self.refresh_active(&current, now_seconds);

        let mut resolved = root.clone();
        apply_transitions(&mut resolved, &self.active, now_seconds);
        let plan = build_transition_plan(&resolved, &self.active, now_seconds);

        self.active.retain(|_, transition| {
            (now_seconds - transition.started_at) < TRANSITION_DURATION_SECONDS
        });
        self.previous = current;

        ResolvedUiTree {
            source: None,
            destination: resolved,
            plan,
        }
    }

    pub fn resolve_navigation(
        &mut self,
        source: Option<&UiNode>,
        destination: &UiNode,
        active_route_transition: Option<RouteTransition>,
        now_seconds: f32,
    ) -> ResolvedUiTree {
        if let (Some(source), Some(route_transition)) = (source, active_route_transition) {
            let source_snapshots = collect_snapshots(source, Affine::IDENTITY, 1.0);
            let destination_snapshots = collect_snapshots(destination, Affine::IDENTITY, 1.0);
            let active = build_route_active_transitions(
                &source_snapshots,
                &destination_snapshots,
                route_transition,
            );
            let mut resolved = destination.clone();
            apply_transitions(&mut resolved, &active, now_seconds);
            let plan = build_transition_plan(&resolved, &active, now_seconds);
            self.previous = destination_snapshots;
            return ResolvedUiTree {
                source: Some(source.clone()),
                destination: resolved,
                plan,
            };
        }

        self.resolve(destination, now_seconds)
    }

    fn refresh_active(&mut self, current: &HashMap<NodeId, NodeSnapshot>, now_seconds: f32) {
        for (id, snapshot) in current {
            let Some(hint) = snapshot.transition_hint else {
                continue;
            };
            let Some(current_bounds) = snapshot.bounds else {
                continue;
            };

            let Some(previous) = self.previous.get(id).copied() else {
                continue;
            };
            let Some(previous_bounds) = previous.bounds else {
                continue;
            };

            let layout_changed = !rect_approx_eq(previous_bounds, current_bounds);
            let opacity_changed = (previous.opacity - snapshot.opacity).abs() > OPACITY_EPSILON;

            if !(layout_changed || opacity_changed) {
                continue;
            }

            let transition = ActiveTransition {
                from_bounds: previous_bounds,
                to_bounds: current_bounds,
                from_opacity: previous.opacity,
                to_opacity: snapshot.opacity,
                started_at: now_seconds,
                hint,
            };
            self.active.insert(*id, transition);
        }
    }
}

fn collect_snapshots(
    node: &UiNode,
    parent_transform: Affine,
    parent_opacity: f32,
) -> HashMap<NodeId, NodeSnapshot> {
    let mut snapshots = HashMap::new();
    collect_snapshots_into(node, parent_transform, parent_opacity, &mut snapshots);
    snapshots
}

fn collect_snapshots_into(
    node: &UiNode,
    parent_transform: Affine,
    parent_opacity: f32,
    snapshots: &mut HashMap<NodeId, NodeSnapshot>,
) {
    let transform = parent_transform * node.transform;
    let opacity = parent_opacity * node.opacity;
    snapshots.insert(
        node.id,
        NodeSnapshot {
            bounds: node_bounds(node, transform),
            opacity,
            transition_hint: node.transition_hint,
        },
    );

    for child in &node.children {
        collect_snapshots_into(child, transform, opacity, snapshots);
    }
}

fn apply_transitions(
    node: &mut UiNode,
    active: &HashMap<NodeId, ActiveTransition>,
    now_seconds: f32,
) {
    if let Some(transition) = active.get(&node.id).copied() {
        let progress =
            ((now_seconds - transition.started_at) / TRANSITION_DURATION_SECONDS).clamp(0.0, 1.0);
        let current_transform = node.transform;
        let current_opacity = node.opacity;
        let transform = match transition.hint {
            UiTransitionHint::SharedElement => interpolate_hero_rect_transform(
                transition.from_bounds,
                transition.to_bounds,
                progress,
            ),
            UiTransitionHint::LayoutDriven => interpolate_rect_transform(
                transition.from_bounds,
                transition.to_bounds,
                ease_out_cubic(progress),
            ),
        };
        node.transform = current_transform * transform;
        node.opacity = lerp_f32(
            transition.from_opacity,
            transition.to_opacity,
            ease_out_cubic(progress),
        ) * current_opacity;
    }

    for child in &mut node.children {
        apply_transitions(child, active, now_seconds);
    }
}

fn build_transition_plan(
    root: &UiNode,
    active: &HashMap<NodeId, ActiveTransition>,
    now_seconds: f32,
) -> UiTransitionPlan {
    let mut shared_media_slots = Vec::new();
    let mut overlay_entries = Vec::new();
    collect_shared_media_slots(
        root,
        Affine::IDENTITY,
        1.0,
        active,
        now_seconds,
        &mut shared_media_slots,
        &mut overlay_entries,
    );
    UiTransitionPlan {
        shared_media_slots,
        overlay_entries,
    }
}

fn build_route_active_transitions(
    source: &HashMap<NodeId, NodeSnapshot>,
    destination: &HashMap<NodeId, NodeSnapshot>,
    route_transition: RouteTransition,
) -> HashMap<NodeId, ActiveTransition> {
    let mut active = HashMap::new();
    for (id, snapshot) in destination {
        let Some(hint) = snapshot.transition_hint else {
            continue;
        };
        if !matches!(hint, UiTransitionHint::SharedElement) {
            continue;
        }

        let Some(from_snapshot) = source.get(id).copied() else {
            continue;
        };
        let (Some(from_bounds), Some(to_bounds)) = (from_snapshot.bounds, snapshot.bounds) else {
            continue;
        };

        active.insert(
            *id,
            ActiveTransition {
                from_bounds,
                to_bounds,
                from_opacity: from_snapshot.opacity,
                to_opacity: snapshot.opacity,
                started_at: route_transition.started_at,
                hint,
            },
        );
    }
    active
}

fn collect_shared_media_slots(
    node: &UiNode,
    parent_transform: Affine,
    parent_opacity: f32,
    active: &HashMap<NodeId, ActiveTransition>,
    now_seconds: f32,
    slots: &mut Vec<ResolvedSharedMediaSlot>,
    overlays: &mut Vec<ResolvedTransitionOverlayEntry>,
) {
    let transform = parent_transform * node.transform;
    let opacity = parent_opacity * node.opacity;

    if matches!(node.transition_hint, Some(UiTransitionHint::SharedElement))
        && matches!(node.effect_hint, Some(UiEffectHint::PromoteSurface))
        && let UiNodeContent::MediaSlot { rect, .. } = node.content
    {
        let resolved_rect = transform_rect(transform, rect);
        slots.push(ResolvedSharedMediaSlot {
            id: node.id,
            rect: resolved_rect,
            layer: node.layer,
        });
        if matches!(
            active.get(&node.id).map(|transition| transition.hint),
            Some(UiTransitionHint::SharedElement)
        ) {
            let active_transition = active
                .get(&node.id)
                .copied()
                .expect("shared media overlay requires an active transition");
            overlays.push(ResolvedTransitionOverlayEntry {
                kind: TransitionOverlayKind::SharedMediaSurface,
                source_id: node.id,
                rect: resolved_rect,
                progress: transition_progress(active_transition, now_seconds),
                phase: transition_overlay_phase(active_transition),
            });
        }
    } else if matches!(
        active.get(&node.id).map(|transition| transition.hint),
        Some(UiTransitionHint::SharedElement)
    ) && supports_promoted_node_overlay(node)
    {
        let active_transition = active
            .get(&node.id)
            .copied()
            .expect("promoted node overlay requires an active transition");
        overlays.push(ResolvedTransitionOverlayEntry {
            kind: TransitionOverlayKind::PromotedNode,
            source_id: node.id,
            rect: node_bounds(node, transform).unwrap_or(Rect::ZERO),
            progress: transition_progress(active_transition, now_seconds),
            phase: transition_overlay_phase(active_transition),
        });
    }

    for child in &node.children {
        collect_shared_media_slots(
            child,
            transform,
            opacity,
            active,
            now_seconds,
            slots,
            overlays,
        );
    }
}

fn supports_promoted_node_overlay(node: &UiNode) -> bool {
    matches!(
        node.content,
        UiNodeContent::RoundedRect { .. }
            | UiNodeContent::Circle { .. }
            | UiNodeContent::Text { .. }
    )
}

fn node_bounds(node: &UiNode, transform: Affine) -> Option<Rect> {
    match &node.content {
        UiNodeContent::Group => None,
        UiNodeContent::RoundedRect { rect, .. } | UiNodeContent::MediaSlot { rect, .. } => {
            Some(transform_rect(transform, *rect))
        },
        UiNodeContent::Circle { center, radius, .. } => Some(transform_rect(
            transform,
            Rect::new(
                center.0 - *radius,
                center.1 - *radius,
                center.0 + *radius,
                center.1 + *radius,
            ),
        )),
        UiNodeContent::Text {
            origin,
            text,
            font_size,
            ..
        } => {
            let width = text.chars().count() as f64 * (*font_size as f64 * 0.58);
            let height = *font_size as f64 * 1.2;
            Some(transform_rect(
                transform,
                Rect::new(
                    origin.0 as f64,
                    origin.1 as f64,
                    origin.0 as f64 + width,
                    origin.1 as f64 + height,
                ),
            ))
        },
    }
}

fn transform_rect(transform: Affine, rect: Rect) -> Rect {
    let p0 = transform * Point::new(rect.x0, rect.y0);
    let p1 = transform * Point::new(rect.x1, rect.y0);
    let p2 = transform * Point::new(rect.x0, rect.y1);
    let p3 = transform * Point::new(rect.x1, rect.y1);

    let min_x = p0.x.min(p1.x).min(p2.x).min(p3.x);
    let max_x = p0.x.max(p1.x).max(p2.x).max(p3.x);
    let min_y = p0.y.min(p1.y).min(p2.y).min(p3.y);
    let max_y = p0.y.max(p1.y).max(p2.y).max(p3.y);

    Rect::new(min_x, min_y, max_x, max_y)
}

fn interpolate_rect_transform(from: Rect, to: Rect, progress: f32) -> Affine {
    let progress = progress as f64;
    let from_center = Point::new((from.x0 + from.x1) * 0.5, (from.y0 + from.y1) * 0.5);
    let to_center = Point::new((to.x0 + to.x1) * 0.5, (to.y0 + to.y1) * 0.5);
    let start_scale_x = safe_div(from.width(), to.width());
    let start_scale_y = safe_div(from.height(), to.height());
    let scale_x = lerp_f64(start_scale_x, 1.0, progress);
    let scale_y = lerp_f64(start_scale_y, 1.0, progress);
    let translate_x = lerp_f64(from_center.x - to_center.x, 0.0, progress);
    let translate_y = lerp_f64(from_center.y - to_center.y, 0.0, progress);

    Affine::translate((translate_x, translate_y))
        * Affine::translate((to_center.x, to_center.y))
        * Affine::scale_non_uniform(scale_x, scale_y)
        * Affine::translate((-to_center.x, -to_center.y))
}

fn interpolate_hero_rect_transform(from: Rect, to: Rect, progress: f32) -> Affine {
    let from_center = Point::new((from.x0 + from.x1) * 0.5, (from.y0 + from.y1) * 0.5);
    let to_center = Point::new((to.x0 + to.x1) * 0.5, (to.y0 + to.y1) * 0.5);
    let move_progress = ease_in_out_cubic((progress / 0.68).clamp(0.0, 1.0)) as f64;
    let scale_progress = ease_out_quart(((progress - 0.18) / 0.82).clamp(0.0, 1.0)) as f64;
    let arc_height = ((from_center.x - to_center.x).abs() * 0.18)
        .max((from_center.y - to_center.y).abs() * 0.22)
        .max(42.0);
    let center_x = lerp_f64(from_center.x, to_center.x, move_progress);
    let baseline_y = lerp_f64(from_center.y, to_center.y, move_progress);
    let center_y = baseline_y - arc_height * (std::f64::consts::PI * move_progress).sin();
    let start_scale_x = safe_div(from.width(), to.width());
    let start_scale_y = safe_div(from.height(), to.height());
    let scale_x = lerp_f64(start_scale_x, 1.0, scale_progress);
    let scale_y = lerp_f64(start_scale_y, 1.0, scale_progress);
    let to_center = Point::new((to.x0 + to.x1) * 0.5, (to.y0 + to.y1) * 0.5);

    Affine::translate((center_x - to_center.x, center_y - to_center.y))
        * Affine::translate((to_center.x, to_center.y))
        * Affine::scale_non_uniform(scale_x, scale_y)
        * Affine::translate((-to_center.x, -to_center.y))
}

fn rect_approx_eq(a: Rect, b: Rect) -> bool {
    (a.x0 - b.x0).abs() < LAYOUT_EPSILON
        && (a.y0 - b.y0).abs() < LAYOUT_EPSILON
        && (a.x1 - b.x1).abs() < LAYOUT_EPSILON
        && (a.y1 - b.y1).abs() < LAYOUT_EPSILON
}

fn safe_div(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() < f64::EPSILON {
        1.0
    } else {
        numerator / denominator
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

fn ease_out_quart(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(4)
}

fn lerp_f32(from: f32, to: f32, progress: f32) -> f32 {
    from + (to - from) * progress
}

fn lerp_f64(from: f64, to: f64, progress: f64) -> f64 {
    from + (to - from) * progress
}

fn transition_progress(transition: ActiveTransition, now_seconds: f32) -> f32 {
    ((now_seconds - transition.started_at) / TRANSITION_DURATION_SECONDS).clamp(0.0, 1.0)
}

fn transition_overlay_phase(transition: ActiveTransition) -> TransitionOverlayPhase {
    if transition.to_bounds.area() >= transition.from_bounds.area() {
        TransitionOverlayPhase::Promoting
    } else {
        TransitionOverlayPhase::Settling
    }
}

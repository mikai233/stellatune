use vello::kurbo::Rect;
use vello::peniko::Color;

use crate::ui::node::{NodeId, UiEffectHint, UiNode, UiStroke};

#[derive(Debug, Clone, Copy)]
pub enum ButtonTone {
    Accent,
    Ghost,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonVisualState {
    pub hovered: bool,
    pub pressed: bool,
}

pub struct PillButtonSpec {
    pub id: NodeId,
    pub rect: Rect,
    pub label_id: NodeId,
    pub label_rect: Rect,
    pub label: String,
    pub tone: ButtonTone,
    pub state: ButtonVisualState,
    pub label_color: Color,
}

pub fn page_background(id: NodeId, rect: Rect, color: Color) -> UiNode {
    UiNode::rounded_rect(id, rect, 0.0, Some(color), None)
}

pub fn surface(
    id: NodeId,
    rect: Rect,
    radius: f64,
    fill: Color,
    stroke: Option<UiStroke>,
) -> UiNode {
    UiNode::rounded_rect(id, rect, radius, Some(fill), stroke)
}

pub fn text_line(
    id: NodeId,
    rect: Rect,
    text: impl Into<String>,
    font_size: f32,
    color: Color,
) -> UiNode {
    let baseline_offset = (font_size * 0.84).round();
    UiNode::text(
        id,
        (rect.x0 as f32, rect.y0 as f32 + baseline_offset),
        text,
        font_size,
        color,
    )
}

pub fn tagged_caption(
    id: NodeId,
    rect: Rect,
    text: impl Into<String>,
    font_size: f32,
    color: Color,
) -> UiNode {
    text_line(id, rect, text, font_size, color).with_effect_hint(UiEffectHint::OutlineTag)
}

pub fn pill_button(spec: PillButtonSpec) -> UiNode {
    let (fill, stroke) = match spec.tone {
        ButtonTone::Accent => (
            if spec.state.pressed {
                Color::from_rgba8(249, 214, 138, 255)
            } else if spec.state.hovered {
                Color::from_rgba8(243, 204, 120, 255)
            } else {
                Color::from_rgba8(228, 188, 97, 255)
            },
            Some(UiStroke {
                width: if spec.state.hovered { 1.5 } else { 0.0 },
                color: Color::from_rgba8(255, 255, 255, 38),
            }),
        ),
        ButtonTone::Ghost => (
            if spec.state.pressed {
                Color::from_rgba8(255, 255, 255, 68)
            } else if spec.state.hovered {
                Color::from_rgba8(255, 255, 255, 48)
            } else {
                Color::from_rgba8(255, 255, 255, 28)
            },
            Some(UiStroke {
                width: if spec.state.hovered { 1.5 } else { 1.0 },
                color: if spec.state.hovered {
                    Color::from_rgba8(255, 255, 255, 76)
                } else {
                    Color::from_rgba8(255, 255, 255, 28)
                },
            }),
        ),
    };

    surface(spec.id, spec.rect, 999.0, fill, stroke).with_children(vec![text_line(
        spec.label_id,
        spec.label_rect,
        spec.label,
        17.0,
        spec.label_color,
    )])
}

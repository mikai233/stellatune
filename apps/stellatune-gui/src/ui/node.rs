#![allow(dead_code)]

use vello::kurbo::{Affine, Circle, Rect, RoundedRect};
use vello::peniko::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UiLayer {
    Background,
    Content,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEffectHint {
    Halo,
    Underline,
    OutlineTag,
    PromoteSurface,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiTransitionHint {
    SharedElement,
    LayoutDriven,
}

#[derive(Debug, Clone, Copy)]
pub struct UiStroke {
    pub width: f64,
    pub color: Color,
}

#[derive(Debug, Clone)]
pub enum UiNodeContent {
    Group,
    RoundedRect {
        rect: Rect,
        corner_radius: f64,
        fill: Option<Color>,
        stroke: Option<UiStroke>,
    },
    Circle {
        center: (f64, f64),
        radius: f64,
        fill: Option<Color>,
        stroke: Option<UiStroke>,
    },
    Text {
        origin: (f32, f32),
        text: String,
        font_size: f32,
        color: Color,
    },
    MediaSlot {
        rect: Rect,
        corner_radius: f64,
    },
}

#[derive(Debug, Clone)]
pub struct UiNode {
    pub id: NodeId,
    pub layer: UiLayer,
    pub opacity: f32,
    pub transform: Affine,
    pub effect_hint: Option<UiEffectHint>,
    pub transition_hint: Option<UiTransitionHint>,
    pub content: UiNodeContent,
    pub children: Vec<UiNode>,
}

impl UiNode {
    pub fn group(id: NodeId) -> Self {
        Self::new(id, UiNodeContent::Group)
    }

    pub fn rounded_rect(
        id: NodeId,
        rect: Rect,
        corner_radius: f64,
        fill: Option<Color>,
        stroke: Option<UiStroke>,
    ) -> Self {
        Self::new(
            id,
            UiNodeContent::RoundedRect {
                rect,
                corner_radius,
                fill,
                stroke,
            },
        )
    }

    pub fn circle(
        id: NodeId,
        center: (f64, f64),
        radius: f64,
        fill: Option<Color>,
        stroke: Option<UiStroke>,
    ) -> Self {
        Self::new(
            id,
            UiNodeContent::Circle {
                center,
                radius,
                fill,
                stroke,
            },
        )
    }

    pub fn text(
        id: NodeId,
        origin: (f32, f32),
        text: impl Into<String>,
        font_size: f32,
        color: Color,
    ) -> Self {
        Self::new(
            id,
            UiNodeContent::Text {
                origin,
                text: text.into(),
                font_size,
                color,
            },
        )
    }

    pub fn media_slot(id: NodeId, rect: Rect, corner_radius: f64) -> Self {
        Self::new(
            id,
            UiNodeContent::MediaSlot {
                rect,
                corner_radius,
            },
        )
    }

    pub fn with_layer(mut self, layer: UiLayer) -> Self {
        self.layer = layer;
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn with_transform(mut self, transform: Affine) -> Self {
        self.transform = transform;
        self
    }

    pub fn with_effect_hint(mut self, effect_hint: UiEffectHint) -> Self {
        self.effect_hint = Some(effect_hint);
        self
    }

    pub fn with_transition_hint(mut self, transition_hint: UiTransitionHint) -> Self {
        self.transition_hint = Some(transition_hint);
        self
    }

    pub fn with_children(mut self, children: Vec<UiNode>) -> Self {
        self.children = children;
        self
    }

    pub fn as_rounded_rect(&self) -> Option<RoundedRect> {
        match self.content {
            UiNodeContent::RoundedRect {
                rect,
                corner_radius,
                ..
            }
            | UiNodeContent::MediaSlot {
                rect,
                corner_radius,
            } => Some(RoundedRect::from_rect(rect, corner_radius)),
            _ => None,
        }
    }

    pub fn as_circle(&self) -> Option<Circle> {
        match self.content {
            UiNodeContent::Circle { center, radius, .. } => Some(Circle::new(center, radius)),
            _ => None,
        }
    }

    fn new(id: NodeId, content: UiNodeContent) -> Self {
        Self {
            id,
            layer: UiLayer::Content,
            opacity: 1.0,
            transform: Affine::IDENTITY,
            effect_hint: None,
            transition_hint: None,
            content,
            children: Vec::new(),
        }
    }
}

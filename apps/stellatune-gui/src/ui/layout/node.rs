#![allow(dead_code)]

use crate::ui::layout::geometry::{LayoutRect, LayoutSize};
use crate::ui::layout::kinds::LayoutKind;
use crate::ui::layout::style::LayoutStyle;
use crate::ui::node::NodeId;

#[derive(Debug, Clone)]
pub struct LayoutNode {
    pub id: NodeId,
    pub kind: LayoutKind,
    pub style: LayoutStyle,
    pub intrinsic_size: Option<LayoutSize>,
    pub children: Vec<LayoutNode>,
}

#[derive(Debug, Clone)]
pub struct LaidOutNode {
    pub id: NodeId,
    pub kind: LayoutKind,
    pub rect: LayoutRect,
    pub children: Vec<LaidOutNode>,
}

impl LayoutNode {
    pub fn stack(id: NodeId) -> Self {
        Self::new(id, LayoutKind::Stack)
    }

    pub fn align(id: NodeId) -> Self {
        Self::new(id, LayoutKind::Align)
    }

    pub fn row(id: NodeId) -> Self {
        Self::new(id, LayoutKind::Row)
    }

    pub fn column(id: NodeId) -> Self {
        Self::new(id, LayoutKind::Column)
    }

    pub fn sized_box(id: NodeId, size: LayoutSize) -> Self {
        Self {
            id,
            kind: LayoutKind::SizedBox,
            style: LayoutStyle::default(),
            intrinsic_size: Some(size),
            children: Vec::new(),
        }
    }

    pub fn leaf(id: NodeId, intrinsic_size: LayoutSize) -> Self {
        Self {
            id,
            kind: LayoutKind::Leaf,
            style: LayoutStyle::default(),
            intrinsic_size: Some(intrinsic_size),
            children: Vec::new(),
        }
    }

    pub fn with_style(mut self, style: LayoutStyle) -> Self {
        self.style = style;
        self
    }

    pub fn with_children(mut self, children: Vec<LayoutNode>) -> Self {
        self.children = children;
        self
    }

    fn new(id: NodeId, kind: LayoutKind) -> Self {
        Self {
            id,
            kind,
            style: LayoutStyle::default(),
            intrinsic_size: None,
            children: Vec::new(),
        }
    }
}

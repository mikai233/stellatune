use crate::ui::layout::constraints::LayoutConstraints;
use crate::ui::layout::geometry::{LayoutRect, LayoutSize};
use crate::ui::layout::kinds::LayoutKind;
use crate::ui::layout::node::{LaidOutNode, LayoutNode};
use crate::ui::layout::style::{Alignment, LayoutLength, LayoutStyle};

pub fn layout_tree(root: &LayoutNode, viewport: LayoutSize) -> LaidOutNode {
    layout_node(
        root,
        LayoutConstraints::tight(viewport),
        LayoutRect::new(0.0, 0.0, viewport.width, viewport.height),
    )
}

fn layout_node(
    node: &LayoutNode,
    constraints: LayoutConstraints,
    assigned_rect: LayoutRect,
) -> LaidOutNode {
    match node.kind {
        LayoutKind::Leaf => layout_leaf(node, constraints, assigned_rect),
        LayoutKind::Stack => layout_stack(node, constraints, assigned_rect),
        LayoutKind::Row => layout_row(node, constraints, assigned_rect),
        LayoutKind::Column => layout_column(node, constraints, assigned_rect),
        LayoutKind::Align => layout_align(node, constraints, assigned_rect),
        LayoutKind::SizedBox => layout_sized_box(node, constraints, assigned_rect),
    }
}

fn layout_leaf(
    node: &LayoutNode,
    constraints: LayoutConstraints,
    assigned_rect: LayoutRect,
) -> LaidOutNode {
    let mut size = node.intrinsic_size.unwrap_or(assigned_rect.size);
    size = apply_length(size, assigned_rect.size, node.style);
    size = constraints.clamp(size);
    LaidOutNode {
        id: node.id,
        kind: node.kind,
        rect: LayoutRect::new(
            assigned_rect.origin.x,
            assigned_rect.origin.y,
            size.width,
            size.height,
        ),
        children: Vec::new(),
    }
}

fn layout_align(
    node: &LayoutNode,
    constraints: LayoutConstraints,
    assigned_rect: LayoutRect,
) -> LaidOutNode {
    let rect = LayoutRect::new(
        assigned_rect.origin.x,
        assigned_rect.origin.y,
        constraints.max.width,
        constraints.max.height,
    );
    let content = rect.inset(node.style.padding);
    let children = node
        .children
        .iter()
        .map(|child| {
            let intrinsic = child.intrinsic_size.unwrap_or(content.size);
            let child_size =
                constraints
                    .loosen()
                    .clamp(apply_length(intrinsic, content.size, child.style));
            let child_rect = aligned_rect(content, child_size, node.style.alignment);
            layout_node(child, LayoutConstraints::tight(child_rect.size), child_rect)
        })
        .collect();

    LaidOutNode {
        id: node.id,
        kind: node.kind,
        rect,
        children,
    }
}

fn layout_stack(
    node: &LayoutNode,
    constraints: LayoutConstraints,
    assigned_rect: LayoutRect,
) -> LaidOutNode {
    let rect = LayoutRect::new(
        assigned_rect.origin.x,
        assigned_rect.origin.y,
        constraints.max.width,
        constraints.max.height,
    );
    let content = rect.inset(node.style.padding);
    let children = node
        .children
        .iter()
        .map(|child| layout_node(child, LayoutConstraints::tight(content.size), content))
        .collect();
    LaidOutNode {
        id: node.id,
        kind: node.kind,
        rect,
        children,
    }
}

fn layout_sized_box(
    node: &LayoutNode,
    constraints: LayoutConstraints,
    assigned_rect: LayoutRect,
) -> LaidOutNode {
    let intrinsic = node.intrinsic_size.unwrap_or(assigned_rect.size);
    let size = constraints.clamp(apply_length(intrinsic, assigned_rect.size, node.style));
    let rect = LayoutRect::new(
        assigned_rect.origin.x,
        assigned_rect.origin.y,
        size.width,
        size.height,
    );
    let content = rect.inset(node.style.padding);
    let children = node
        .children
        .iter()
        .map(|child| layout_node(child, LayoutConstraints::tight(content.size), content))
        .collect();

    LaidOutNode {
        id: node.id,
        kind: node.kind,
        rect,
        children,
    }
}

fn layout_row(
    node: &LayoutNode,
    constraints: LayoutConstraints,
    assigned_rect: LayoutRect,
) -> LaidOutNode {
    let rect = LayoutRect::new(
        assigned_rect.origin.x,
        assigned_rect.origin.y,
        constraints.max.width,
        constraints.max.height,
    );
    let content = rect.inset(node.style.padding);
    let gap = node.style.gap;
    let fill_count = node
        .children
        .iter()
        .filter(|child| matches!(child.style.width, LayoutLength::Fill))
        .count();
    let fixed_total = node
        .children
        .iter()
        .map(|child| match child.style.width {
            LayoutLength::Fixed(width) => width,
            _ => child.intrinsic_size.map(|size| size.width).unwrap_or(0.0),
        })
        .sum::<f32>();
    let total_gap = gap * node.children.len().saturating_sub(1) as f32;
    let free_width = (content.size.width - fixed_total - total_gap).max(0.0);
    let fill_width = if fill_count > 0 {
        free_width / fill_count as f32
    } else {
        0.0
    };

    let mut cursor_x = content.origin.x;
    let children = node
        .children
        .iter()
        .map(|child| {
            let child_width = match child.style.width {
                LayoutLength::Fixed(width) => width,
                LayoutLength::Fill => fill_width,
                LayoutLength::Shrink => child
                    .intrinsic_size
                    .map(|size| size.width)
                    .unwrap_or(fill_width),
            };
            let child_height = match child.style.height {
                LayoutLength::Fixed(height) => height,
                LayoutLength::Fill => content.size.height,
                LayoutLength::Shrink => child
                    .intrinsic_size
                    .map(|size| size.height)
                    .unwrap_or(content.size.height),
            };
            let child_rect = LayoutRect::new(cursor_x, content.origin.y, child_width, child_height);
            cursor_x += child_width + gap;
            layout_node(child, LayoutConstraints::tight(child_rect.size), child_rect)
        })
        .collect();

    LaidOutNode {
        id: node.id,
        kind: node.kind,
        rect,
        children,
    }
}

fn layout_column(
    node: &LayoutNode,
    constraints: LayoutConstraints,
    assigned_rect: LayoutRect,
) -> LaidOutNode {
    let rect = LayoutRect::new(
        assigned_rect.origin.x,
        assigned_rect.origin.y,
        constraints.max.width,
        constraints.max.height,
    );
    let content = rect.inset(node.style.padding);
    let gap = node.style.gap;
    let fill_count = node
        .children
        .iter()
        .filter(|child| matches!(child.style.height, LayoutLength::Fill))
        .count();
    let fixed_total = node
        .children
        .iter()
        .map(|child| match child.style.height {
            LayoutLength::Fixed(height) => height,
            _ => child.intrinsic_size.map(|size| size.height).unwrap_or(0.0),
        })
        .sum::<f32>();
    let total_gap = gap * node.children.len().saturating_sub(1) as f32;
    let free_height = (content.size.height - fixed_total - total_gap).max(0.0);
    let fill_height = if fill_count > 0 {
        free_height / fill_count as f32
    } else {
        0.0
    };

    let mut cursor_y = content.origin.y;
    let children = node
        .children
        .iter()
        .map(|child| {
            let child_width = match child.style.width {
                LayoutLength::Fixed(width) => width,
                LayoutLength::Fill => content.size.width,
                LayoutLength::Shrink => child
                    .intrinsic_size
                    .map(|size| size.width)
                    .unwrap_or(content.size.width),
            };
            let child_height = match child.style.height {
                LayoutLength::Fixed(height) => height,
                LayoutLength::Fill => fill_height,
                LayoutLength::Shrink => child
                    .intrinsic_size
                    .map(|size| size.height)
                    .unwrap_or(fill_height),
            };
            let child_rect = LayoutRect::new(content.origin.x, cursor_y, child_width, child_height);
            cursor_y += child_height + gap;
            layout_node(child, LayoutConstraints::tight(child_rect.size), child_rect)
        })
        .collect();

    LaidOutNode {
        id: node.id,
        kind: node.kind,
        rect,
        children,
    }
}

fn apply_length(size: LayoutSize, parent: LayoutSize, style: LayoutStyle) -> LayoutSize {
    LayoutSize::new(
        match style.width {
            LayoutLength::Fixed(width) => width,
            LayoutLength::Fill => parent.width,
            LayoutLength::Shrink => size.width,
        },
        match style.height {
            LayoutLength::Fixed(height) => height,
            LayoutLength::Fill => parent.height,
            LayoutLength::Shrink => size.height,
        },
    )
}

fn aligned_rect(parent: LayoutRect, child_size: LayoutSize, alignment: Alignment) -> LayoutRect {
    let free_x = (parent.size.width - child_size.width).max(0.0);
    let free_y = (parent.size.height - child_size.height).max(0.0);
    LayoutRect::new(
        parent.origin.x + free_x * alignment.x.clamp(0.0, 1.0),
        parent.origin.y + free_y * alignment.y.clamp(0.0, 1.0),
        child_size.width,
        child_size.height,
    )
}

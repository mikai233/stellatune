use crate::ui::layout::geometry::LayoutPoint;
use crate::ui::layout::node::LaidOutNode;
use crate::ui::node::NodeId;

pub fn hit_test(root: &LaidOutNode, point: LayoutPoint) -> Option<NodeId> {
    hit_test_node(root, point).map(|node| node.id)
}

pub fn hit_test_node(node: &LaidOutNode, point: LayoutPoint) -> Option<&LaidOutNode> {
    let x0 = node.rect.origin.x;
    let y0 = node.rect.origin.y;
    let x1 = x0 + node.rect.size.width;
    let y1 = y0 + node.rect.size.height;
    let contains = point.x >= x0 && point.x <= x1 && point.y >= y0 && point.y <= y1;

    if !contains {
        return None;
    }

    for child in node.children.iter().rev() {
        if let Some(hit) = hit_test_node(child, point) {
            return Some(hit);
        }
    }

    Some(node)
}

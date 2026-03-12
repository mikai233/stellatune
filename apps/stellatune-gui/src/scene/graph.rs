use super::{ButtonNode, EffectNode, ImageNode, ListNode, PanelNode, TextNode};

#[derive(Debug, Clone, Default)]
pub struct SceneGraph {
    pub layers: Vec<SceneLayer>,
}

impl SceneGraph {
    pub fn effect_nodes(&self) -> impl Iterator<Item = &EffectNode> {
        self.layers.iter().flat_map(|layer| {
            layer.nodes.iter().filter_map(|node| match node {
                SceneNode::Effect(effect) => Some(effect),
                _ => None,
            })
        })
    }

    pub fn layer_label_summary(&self) -> String {
        self.layers
            .iter()
            .map(|layer| layer.kind.label())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<SceneHit> {
        for layer in self.layers.iter().rev() {
            for node in layer.nodes.iter().rev() {
                let hit_id = match node {
                    SceneNode::Text(text) if text.rect.contains(x, y) => Some(text.id.clone()),
                    SceneNode::Panel(panel) if panel.rect.contains(x, y) => Some(panel.id.clone()),
                    SceneNode::Button(button) if button.rect.contains(x, y) => {
                        Some(button.id.clone())
                    },
                    SceneNode::Image(image) if image.rect.contains(x, y) => Some(image.id.clone()),
                    SceneNode::List(list) if list.rect.contains(x, y) => Some(list.id.clone()),
                    SceneNode::Effect(effect) if effect.rect.contains(x, y) => {
                        Some(effect.id.clone())
                    },
                    _ => None,
                };
                if let Some(node_id) = hit_id {
                    return Some(SceneHit {
                        node_id,
                        layer: layer.kind,
                    });
                }
            }
        }
        None
    }
}

#[derive(Debug, Clone)]
pub struct SceneLayer {
    pub kind: SceneLayerKind,
    pub nodes: Vec<SceneNode>,
}

#[derive(Debug, Clone, Copy)]
pub enum SceneLayerKind {
    Background,
    Panels,
    Content,
    Text,
    Overlay,
}

impl SceneLayerKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Panels => "panels",
            Self::Content => "content",
            Self::Text => "text",
            Self::Overlay => "overlay",
        }
    }
}

#[derive(Debug, Clone)]
pub enum SceneNode {
    Panel(PanelNode),
    Text(TextNode),
    Button(ButtonNode),
    Image(ImageNode),
    List(ListNode),
    Effect(EffectNode),
}

#[derive(Debug, Clone)]
pub struct SceneHit {
    pub node_id: String,
    pub layer: SceneLayerKind,
}

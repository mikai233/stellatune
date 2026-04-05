use vello::kurbo::Rect;
use vello::peniko::Color;

use crate::app::FrameState;
use crate::navigation::RouteTransition;
use crate::ui::layout::engine::layout_tree;
use crate::ui::layout::geometry::LayoutSize;
use crate::ui::layout::node::{LaidOutNode, LayoutNode};
use crate::ui::layout::style::{Alignment, EdgeInsets, LayoutLength, LayoutStyle};
use crate::ui::node::{NodeId, UiNode, UiStroke};
use crate::ui::primitives::{
    ButtonTone, ButtonVisualState, PillButtonSpec, page_background, pill_button, surface,
    tagged_caption, text_line,
};
use crate::view::{RouteAction, RouteActionBinding, RouteCursor, RouteInteractionState, RoutePage};

pub fn build_hero_demo_route(
    active_transition: Option<RouteTransition>,
    frame: &FrameState,
    ui_font_family: &str,
    interaction: RouteInteractionState,
) -> RoutePage {
    let width = frame.physical_size.width.max(1) as f64;
    let height = frame.physical_size.height.max(1) as f64;
    let caption = active_transition
        .map(|transition| {
            format!(
                "{} · {:?} · {:?} · {:?} -> {:?}",
                ui_font_family,
                transition.preset(),
                transition.operation(),
                transition.source_route(),
                transition.destination_route()
            )
        })
        .unwrap_or_else(|| format!("{ui_font_family} · HeroDemo"));
    let viewport = LayoutSize::new(width as f32, height as f32);
    let layout = layout_tree(&build_hero_layout_tree(viewport), viewport);
    let hero_panel = layout_rect(&layout, NodeId("hero.layout.panel"));
    let accent = layout_rect(&layout, NodeId("hero.layout.accent"));
    let title = layout_rect(&layout, NodeId("hero.layout.title"));
    let caption_rect = layout_rect(&layout, NodeId("hero.layout.caption"));
    let body_card = layout_rect(&layout, NodeId("hero.layout.body.card"));
    let body_title = layout_rect(&layout, NodeId("hero.layout.body.title"));
    let body_copy = layout_rect(&layout, NodeId("hero.layout.body.copy"));
    let primary = layout_rect(&layout, NodeId("hero.layout.action.primary"));
    let primary_label = layout_rect(&layout, NodeId("hero.layout.action.primary.label"));
    let secondary = layout_rect(&layout, NodeId("hero.layout.action.secondary"));
    let secondary_label = layout_rect(&layout, NodeId("hero.layout.action.secondary.label"));
    let primary_hovered = interaction.hovered == Some(NodeId("hero.layout.action.primary"));
    let secondary_hovered = interaction.hovered == Some(NodeId("hero.layout.action.secondary"));
    let primary_pressed = interaction.pressed == Some(NodeId("hero.layout.action.primary"));
    let secondary_pressed = interaction.pressed == Some(NodeId("hero.layout.action.secondary"));

    let node = UiNode::group(NodeId("hero_demo.root")).with_children(vec![
        page_background(
            NodeId("hero_demo.page.background"),
            Rect::new(0.0, 0.0, width, height),
            Color::from_rgba8(22, 16, 26, 255),
        ),
        surface(
            NodeId("hero_demo.page.panel"),
            hero_panel,
            40.0,
            Color::from_rgba8(38, 24, 48, 255),
            Some(UiStroke {
                width: 1.0,
                color: Color::from_rgba8(255, 255, 255, 20),
            }),
        ),
        surface(
            NodeId("hero_demo.accent"),
            accent,
            28.0,
            Color::from_rgba8(118, 72, 136, 255),
            None,
        ),
        text_line(
            NodeId("hero_demo.title"),
            title,
            "Hero Demo Page",
            42.0,
            Color::from_rgba8(250, 244, 255, 255),
        ),
        tagged_caption(
            NodeId("hero_demo.caption"),
            caption_rect,
            caption,
            15.0,
            Color::from_rgba8(216, 189, 228, 255),
        ),
        surface(
            NodeId("hero_demo.body.card"),
            body_card,
            28.0,
            Color::from_rgba8(49, 32, 61, 255),
            None,
        ),
        text_line(
            NodeId("hero_demo.body.title"),
            body_title,
            "Push / Pop Test Surface",
            30.0,
            Color::from_rgba8(250, 244, 255, 255),
        ),
        text_line(
            NodeId("hero_demo.body.copy"),
            body_copy,
            "This page is fully opaque.\nHero is disabled until layout and input are ready.",
            18.0,
            Color::from_rgba8(222, 208, 228, 255),
        ),
        pill_button(PillButtonSpec {
            id: NodeId("hero_demo.action.primary"),
            rect: primary,
            label_id: NodeId("hero_demo.action.primary.label"),
            label_rect: primary_label,
            label: "Backspace to Pop".to_string(),
            tone: ButtonTone::Accent,
            state: ButtonVisualState {
                hovered: primary_hovered,
                pressed: primary_pressed,
            },
            label_color: Color::from_rgba8(28, 18, 18, 255),
        }),
        pill_button(PillButtonSpec {
            id: NodeId("hero_demo.action.secondary"),
            rect: secondary,
            label_id: NodeId("hero_demo.action.secondary.label"),
            label_rect: secondary_label,
            label: "Press 1 for Library".to_string(),
            tone: ButtonTone::Ghost,
            state: ButtonVisualState {
                hovered: secondary_hovered,
                pressed: secondary_pressed,
            },
            label_color: Color::from_rgba8(243, 234, 247, 255),
        }),
    ]);

    RoutePage {
        node,
        layout,
        actions: vec![
            RouteActionBinding {
                target: NodeId("hero.layout.action.primary"),
                action: RouteAction::Pop,
                cursor: RouteCursor::Pointer,
            },
            RouteActionBinding {
                target: NodeId("hero.layout.action.secondary"),
                action: RouteAction::Navigate(crate::navigation::RouteId::Library),
                cursor: RouteCursor::Pointer,
            },
        ],
    }
}

fn build_hero_layout_tree(viewport: LayoutSize) -> LayoutNode {
    LayoutNode::stack(NodeId("hero.layout.root")).with_children(vec![
        LayoutNode::align(NodeId("hero.layout.shell"))
            .with_style(LayoutStyle {
                width: LayoutLength::Fill,
                height: LayoutLength::Fill,
                padding: EdgeInsets::symmetric(viewport.width * 0.10, viewport.height * 0.08),
                gap: 0.0,
                alignment: Alignment::CENTER,
            })
            .with_children(vec![
                LayoutNode::sized_box(
                    NodeId("hero.layout.panel"),
                    LayoutSize::new(viewport.width * 0.72, viewport.height * 0.70),
                )
                .with_children(vec![
                    LayoutNode::column(NodeId("hero.layout.panel.content"))
                        .with_style(LayoutStyle {
                            width: LayoutLength::Fill,
                            height: LayoutLength::Fill,
                            padding: EdgeInsets::all(36.0),
                            gap: 18.0,
                            alignment: Alignment::TOP_LEFT,
                        })
                        .with_children(vec![
                            LayoutNode::align(NodeId("hero.layout.accent.align"))
                                .with_style(LayoutStyle {
                                    width: LayoutLength::Fill,
                                    height: LayoutLength::Shrink,
                                    alignment: Alignment::TOP_CENTER,
                                    ..LayoutStyle::default()
                                })
                                .with_children(vec![LayoutNode::sized_box(
                                    NodeId("hero.layout.accent"),
                                    LayoutSize::new(viewport.width * 0.40, viewport.height * 0.12),
                                )]),
                            LayoutNode::sized_box(
                                NodeId("hero.layout.title"),
                                LayoutSize::new(360.0, 56.0),
                            ),
                            LayoutNode::sized_box(
                                NodeId("hero.layout.caption"),
                                LayoutSize::new(460.0, 28.0),
                            ),
                            LayoutNode::sized_box(
                                NodeId("hero.layout.body.card"),
                                LayoutSize::new(viewport.width * 0.52, viewport.height * 0.26),
                            )
                            .with_style(LayoutStyle {
                                width: LayoutLength::Fill,
                                ..LayoutStyle::default()
                            })
                            .with_children(vec![
                                LayoutNode::column(NodeId("hero.layout.body.card.content"))
                                    .with_style(LayoutStyle {
                                        width: LayoutLength::Fill,
                                        height: LayoutLength::Fill,
                                        padding: EdgeInsets::all(28.0),
                                        gap: 12.0,
                                        alignment: Alignment::TOP_LEFT,
                                    })
                                    .with_children(vec![
                                        LayoutNode::sized_box(
                                            NodeId("hero.layout.body.title"),
                                            LayoutSize::new(360.0, 38.0),
                                        ),
                                        LayoutNode::sized_box(
                                            NodeId("hero.layout.body.copy"),
                                            LayoutSize::new(460.0, 64.0),
                                        ),
                                    ]),
                            ]),
                            LayoutNode::row(NodeId("hero.layout.actions"))
                                .with_style(LayoutStyle {
                                    width: LayoutLength::Shrink,
                                    height: LayoutLength::Shrink,
                                    gap: 18.0,
                                    alignment: Alignment::TOP_LEFT,
                                    ..LayoutStyle::default()
                                })
                                .with_children(vec![
                                    LayoutNode::sized_box(
                                        NodeId("hero.layout.action.primary"),
                                        LayoutSize::new(220.0, 56.0),
                                    )
                                    .with_children(vec![
                                        LayoutNode::align(NodeId(
                                            "hero.layout.action.primary.center",
                                        ))
                                        .with_style(LayoutStyle {
                                            width: LayoutLength::Fill,
                                            height: LayoutLength::Fill,
                                            alignment: Alignment::CENTER,
                                            ..LayoutStyle::default()
                                        })
                                        .with_children(
                                            vec![LayoutNode::sized_box(
                                                NodeId("hero.layout.action.primary.label"),
                                                LayoutSize::new(180.0, 24.0),
                                            )],
                                        ),
                                    ]),
                                    LayoutNode::sized_box(
                                        NodeId("hero.layout.action.secondary"),
                                        LayoutSize::new(240.0, 56.0),
                                    )
                                    .with_children(vec![
                                        LayoutNode::align(NodeId(
                                            "hero.layout.action.secondary.center",
                                        ))
                                        .with_style(LayoutStyle {
                                            width: LayoutLength::Fill,
                                            height: LayoutLength::Fill,
                                            alignment: Alignment::CENTER,
                                            ..LayoutStyle::default()
                                        })
                                        .with_children(
                                            vec![LayoutNode::sized_box(
                                                NodeId("hero.layout.action.secondary.label"),
                                                LayoutSize::new(180.0, 24.0),
                                            )],
                                        ),
                                    ]),
                                ]),
                        ]),
                ]),
            ]),
    ])
}

fn layout_rect(layout: &LaidOutNode, id: NodeId) -> Rect {
    find_layout_node(layout, id)
        .map(|node| {
            Rect::new(
                node.rect.origin.x as f64,
                node.rect.origin.y as f64,
                (node.rect.origin.x + node.rect.size.width) as f64,
                (node.rect.origin.y + node.rect.size.height) as f64,
            )
        })
        .unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0))
}

fn find_layout_node(node: &LaidOutNode, id: NodeId) -> Option<&LaidOutNode> {
    if node.id == id {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_layout_node(child, id) {
            return Some(found);
        }
    }
    None
}

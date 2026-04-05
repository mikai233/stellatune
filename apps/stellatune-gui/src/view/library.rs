use vello::kurbo::Rect;
use vello::peniko::Color;

use crate::app::FrameState;
use crate::navigation::RouteTransition;
use crate::ui::layout::engine::layout_tree;
use crate::ui::layout::geometry::LayoutSize;
use crate::ui::layout::node::{LaidOutNode, LayoutNode};
use crate::ui::layout::style::{Alignment, EdgeInsets, LayoutLength, LayoutStyle};
use crate::ui::node::{NodeId, UiNode, UiStroke};
use crate::ui::primitives::{page_background, surface, tagged_caption, text_line};
use crate::view::{RouteAction, RouteActionBinding, RouteCursor, RouteInteractionState, RoutePage};

pub fn build_library_route(
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
        .unwrap_or_else(|| format!("{ui_font_family} · Library"));

    let viewport = LayoutSize::new(width as f32, height as f32);
    let layout = layout_tree(&build_library_layout_tree(viewport), viewport);
    let sidebar = layout_rect(&layout, NodeId("library.layout.sidebar"));
    let content = layout_rect(&layout, NodeId("library.layout.content"));
    let sidebar_card = layout_rect(&layout, NodeId("library.layout.sidebar.card"));
    let title = layout_rect(&layout, NodeId("library.layout.title"));
    let caption_rect = layout_rect(&layout, NodeId("library.layout.caption"));
    let sidebar_card_title = layout_rect(&layout, NodeId("library.layout.sidebar.card.title"));
    let sidebar_card_body = layout_rect(&layout, NodeId("library.layout.sidebar.card.body"));
    let rows_container = layout_rect(&layout, NodeId("library.layout.rows"));
    let sidebar_card_hovered = interaction.hovered == Some(NodeId("library.layout.sidebar.card"));
    let sidebar_card_pressed = interaction.pressed == Some(NodeId("library.layout.sidebar.card"));

    let mut rows = Vec::new();
    for index in 0..5 {
        let top = rows_container.y0 + index as f64 * (rows_container.height() / 5.0);
        let row_height = rows_container.height() * 0.80 / 5.0;
        let row_layout_id = match index {
            0 => NodeId("library.layout.rows.0"),
            1 => NodeId("library.layout.rows.1"),
            2 => NodeId("library.layout.rows.2"),
            3 => NodeId("library.layout.rows.3"),
            _ => NodeId("library.layout.rows.4"),
        };
        let row_hovered = interaction.hovered == Some(row_layout_id);
        rows.push(
            UiNode::rounded_rect(
                NodeId(match index {
                    0 => "library.row.0",
                    1 => "library.row.1",
                    2 => "library.row.2",
                    3 => "library.row.3",
                    _ => "library.row.4",
                }),
                Rect::new(content.x0 + 26.0, top, content.x1 - 24.0, top + row_height),
                22.0,
                Some(if row_hovered {
                    Color::from_rgba8(58, 82, 122, 255)
                } else {
                    Color::from_rgba8(35, 49, 74, 255)
                }),
                Some(UiStroke {
                    width: if row_hovered { 1.5 } else { 1.0 },
                    color: if row_hovered {
                        Color::from_rgba8(255, 255, 255, 52)
                    } else {
                        Color::from_rgba8(255, 255, 255, 22)
                    },
                }),
            )
            .with_children(vec![
                UiNode::text(
                    NodeId(match index {
                        0 => "library.row.0.title",
                        1 => "library.row.1.title",
                        2 => "library.row.2.title",
                        3 => "library.row.3.title",
                        _ => "library.row.4.title",
                    }),
                    ((content.x0 + 48.0) as f32, top as f32 + 26.0),
                    format!("Library Item {}", index + 1),
                    20.0,
                    Color::from_rgba8(244, 248, 255, 255),
                ),
                UiNode::text(
                    NodeId(match index {
                        0 => "library.row.0.meta",
                        1 => "library.row.1.meta",
                        2 => "library.row.2.meta",
                        3 => "library.row.3.meta",
                        _ => "library.row.4.meta",
                    }),
                    ((content.x0 + 48.0) as f32, top as f32 + 50.0),
                    "Opaque page for push/pop testing",
                    13.0,
                    Color::from_rgba8(188, 202, 228, 255),
                ),
            ]),
        );
    }

    let node = UiNode::group(NodeId("library.root")).with_children(vec![
        page_background(
            NodeId("library.page.background"),
            Rect::new(0.0, 0.0, width, height),
            Color::from_rgba8(15, 22, 34, 255),
        ),
        surface(
            NodeId("library.sidebar"),
            sidebar,
            34.0,
            Color::from_rgba8(24, 34, 52, 255),
            Some(UiStroke {
                width: 1.0,
                color: Color::from_rgba8(255, 255, 255, 18),
            }),
        ),
        surface(
            NodeId("library.content"),
            content,
            34.0,
            Color::from_rgba8(20, 29, 44, 255),
            Some(UiStroke {
                width: 1.0,
                color: Color::from_rgba8(255, 255, 255, 18),
            }),
        ),
        text_line(
            NodeId("library.title"),
            title,
            "Library Page",
            40.0,
            Color::from_rgba8(247, 250, 255, 255),
        ),
        tagged_caption(
            NodeId("library.caption"),
            caption_rect,
            caption,
            15.0,
            Color::from_rgba8(167, 183, 210, 255),
        ),
        surface(
            NodeId("library.sidebar.card"),
            sidebar_card,
            26.0,
            if sidebar_card_pressed {
                Color::from_rgba8(81, 116, 168, 255)
            } else if sidebar_card_hovered {
                Color::from_rgba8(72, 102, 148, 255)
            } else {
                Color::from_rgba8(53, 75, 112, 255)
            },
            Some(UiStroke {
                width: if sidebar_card_hovered { 1.5 } else { 0.0 },
                color: Color::from_rgba8(255, 255, 255, 48),
            }),
        ),
        text_line(
            NodeId("library.sidebar.card.title"),
            sidebar_card_title,
            "Stack Navigation",
            24.0,
            Color::from_rgba8(247, 250, 255, 255),
        ),
        text_line(
            NodeId("library.sidebar.card.body"),
            sidebar_card_body,
            "Press 2 to push\na new page,\nBackspace to pop.\nHero is disabled for now.",
            15.0,
            Color::from_rgba8(220, 229, 243, 255),
        ),
        UiNode::group(NodeId("library.rows")).with_children(rows),
    ]);

    RoutePage {
        node,
        layout,
        actions: vec![RouteActionBinding {
            target: NodeId("library.layout.sidebar.card"),
            action: RouteAction::Navigate(crate::navigation::RouteId::HeroDemo),
            cursor: RouteCursor::Pointer,
        }],
    }
}

fn build_library_layout_tree(viewport: LayoutSize) -> LayoutNode {
    LayoutNode::stack(NodeId("library.layout.root")).with_children(vec![
        LayoutNode::row(NodeId("library.layout.shell"))
            .with_style(LayoutStyle {
                width: LayoutLength::Fill,
                height: LayoutLength::Fill,
                padding: EdgeInsets::symmetric(viewport.width * 0.06, viewport.height * 0.08),
                gap: 28.0,
                alignment: Alignment::TOP_LEFT,
            })
            .with_children(vec![
                LayoutNode::column(NodeId("library.layout.sidebar"))
                    .with_style(LayoutStyle {
                        width: LayoutLength::Fixed(viewport.width * 0.24),
                        height: LayoutLength::Fill,
                        padding: EdgeInsets::all(22.0),
                        gap: 18.0,
                        alignment: Alignment::TOP_LEFT,
                    })
                    .with_children(vec![
                        LayoutNode::sized_box(
                            NodeId("library.layout.title"),
                            LayoutSize::new(280.0, 56.0),
                        ),
                        LayoutNode::sized_box(
                            NodeId("library.layout.caption"),
                            LayoutSize::new(280.0, 30.0),
                        ),
                        LayoutNode::align(NodeId("library.layout.sidebar.card.align"))
                            .with_style(LayoutStyle {
                                width: LayoutLength::Fill,
                                height: LayoutLength::Shrink,
                                alignment: Alignment::TOP_CENTER,
                                ..LayoutStyle::default()
                            })
                            .with_children(vec![LayoutNode::sized_box(
                                NodeId("library.layout.sidebar.card"),
                                LayoutSize::new(viewport.width * 0.20, viewport.height * 0.34),
                            )]),
                        LayoutNode::sized_box(
                            NodeId("library.layout.sidebar.card.title"),
                            LayoutSize::new(220.0, 40.0),
                        ),
                        LayoutNode::sized_box(
                            NodeId("library.layout.sidebar.card.body"),
                            LayoutSize::new(220.0, 120.0),
                        ),
                    ]),
                LayoutNode::column(NodeId("library.layout.content"))
                    .with_style(LayoutStyle {
                        width: LayoutLength::Fill,
                        height: LayoutLength::Fill,
                        padding: EdgeInsets::all(22.0),
                        gap: 16.0,
                        alignment: Alignment::TOP_LEFT,
                    })
                    .with_children(vec![
                        LayoutNode::sized_box(
                            NodeId("library.layout.rows.0"),
                            LayoutSize::new(viewport.width * 0.44, viewport.height * 0.09),
                        ),
                        LayoutNode::sized_box(
                            NodeId("library.layout.rows.1"),
                            LayoutSize::new(viewport.width * 0.44, viewport.height * 0.09),
                        ),
                        LayoutNode::sized_box(
                            NodeId("library.layout.rows.2"),
                            LayoutSize::new(viewport.width * 0.44, viewport.height * 0.09),
                        ),
                        LayoutNode::sized_box(
                            NodeId("library.layout.rows.3"),
                            LayoutSize::new(viewport.width * 0.44, viewport.height * 0.09),
                        ),
                        LayoutNode::sized_box(
                            NodeId("library.layout.rows.4"),
                            LayoutSize::new(viewport.width * 0.44, viewport.height * 0.09),
                        ),
                        LayoutNode::sized_box(
                            NodeId("library.layout.rows"),
                            LayoutSize::new(viewport.width * 0.44, viewport.height * 0.60),
                        )
                        .with_style(LayoutStyle {
                            width: LayoutLength::Fill,
                            height: LayoutLength::Fill,
                            ..LayoutStyle::default()
                        }),
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

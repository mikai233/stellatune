use vello::kurbo::Rect;
use vello::peniko::Color;

use crate::app::FrameState;
use crate::navigation::RouteTransition;
use crate::ui::node::{NodeId, UiEffectHint, UiNode, UiStroke};

pub fn build_library_route(
    active_transition: Option<RouteTransition>,
    frame: &FrameState,
    ui_font_family: &str,
) -> UiNode {
    let width = frame.physical_size.width.max(1) as f64;
    let height = frame.physical_size.height.max(1) as f64;
    let sidebar = Rect::new(width * 0.06, height * 0.08, width * 0.30, height * 0.92);
    let content = Rect::new(width * 0.34, height * 0.08, width * 0.94, height * 0.92);
    let caption = active_transition
        .map(|transition| {
            format!(
                "{} · {:?} · {:?} · {:?} -> {:?}",
                ui_font_family,
                transition.preset(),
                transition.operation(),
                transition.from_route(),
                transition.to_route()
            )
        })
        .unwrap_or_else(|| format!("{ui_font_family} · Library"));

    let mut rows = Vec::new();
    for index in 0..5 {
        let top = height * 0.24 + index as f64 * height * 0.105;
        rows.push(
            UiNode::rounded_rect(
                NodeId(match index {
                    0 => "library.row.0",
                    1 => "library.row.1",
                    2 => "library.row.2",
                    3 => "library.row.3",
                    _ => "library.row.4",
                }),
                Rect::new(width * 0.38, top, width * 0.90, top + height * 0.082),
                22.0,
                Some(Color::from_rgba8(35, 49, 74, 255)),
                Some(UiStroke {
                    width: 1.0,
                    color: Color::from_rgba8(255, 255, 255, 22),
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
                    (width as f32 * 0.405, top as f32 + 26.0),
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
                    (width as f32 * 0.405, top as f32 + 50.0),
                    "Opaque page for push/pop testing",
                    13.0,
                    Color::from_rgba8(188, 202, 228, 255),
                ),
            ]),
        );
    }

    UiNode::group(NodeId("library.root")).with_children(vec![
        UiNode::rounded_rect(
            NodeId("library.page.background"),
            Rect::new(0.0, 0.0, width, height),
            0.0,
            Some(Color::from_rgba8(15, 22, 34, 255)),
            None,
        ),
        UiNode::rounded_rect(
            NodeId("library.sidebar"),
            sidebar,
            34.0,
            Some(Color::from_rgba8(24, 34, 52, 255)),
            Some(UiStroke {
                width: 1.0,
                color: Color::from_rgba8(255, 255, 255, 18),
            }),
        ),
        UiNode::rounded_rect(
            NodeId("library.content"),
            content,
            34.0,
            Some(Color::from_rgba8(20, 29, 44, 255)),
            Some(UiStroke {
                width: 1.0,
                color: Color::from_rgba8(255, 255, 255, 18),
            }),
        ),
        UiNode::text(
            NodeId("library.title"),
            (width as f32 * 0.10, height as f32 * 0.18),
            "Library Page",
            40.0,
            Color::from_rgba8(247, 250, 255, 255),
        ),
        UiNode::text(
            NodeId("library.caption"),
            (width as f32 * 0.10, height as f32 * 0.23),
            caption,
            15.0,
            Color::from_rgba8(167, 183, 210, 255),
        )
        .with_effect_hint(UiEffectHint::OutlineTag),
        UiNode::rounded_rect(
            NodeId("library.sidebar.card"),
            Rect::new(width * 0.095, height * 0.34, width * 0.265, height * 0.56),
            26.0,
            Some(Color::from_rgba8(53, 75, 112, 255)),
            None,
        ),
        UiNode::text(
            NodeId("library.sidebar.card.title"),
            (width as f32 * 0.115, height as f32 * 0.41),
            "Stack Navigation",
            24.0,
            Color::from_rgba8(247, 250, 255, 255),
        ),
        UiNode::text(
            NodeId("library.sidebar.card.body"),
            (width as f32 * 0.115, height as f32 * 0.47),
            "Press 2 to push\na new page,\nBackspace to pop.\nHero is disabled for now.",
            15.0,
            Color::from_rgba8(220, 229, 243, 255),
        ),
        UiNode::group(NodeId("library.rows")).with_children(rows),
    ])
}

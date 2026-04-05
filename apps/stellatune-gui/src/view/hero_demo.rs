use vello::kurbo::Rect;
use vello::peniko::Color;

use crate::app::FrameState;
use crate::navigation::RouteTransition;
use crate::ui::node::{NodeId, UiEffectHint, UiNode, UiStroke};

pub fn build_hero_demo_route(
    active_transition: Option<RouteTransition>,
    frame: &FrameState,
    ui_font_family: &str,
) -> UiNode {
    let width = frame.physical_size.width.max(1) as f64;
    let height = frame.physical_size.height.max(1) as f64;
    let hero_panel = Rect::new(width * 0.14, height * 0.12, width * 0.86, height * 0.88);
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
        .unwrap_or_else(|| format!("{ui_font_family} · HeroDemo"));

    UiNode::group(NodeId("hero_demo.root")).with_children(vec![
        UiNode::rounded_rect(
            NodeId("hero_demo.page.background"),
            Rect::new(0.0, 0.0, width, height),
            0.0,
            Some(Color::from_rgba8(22, 16, 26, 255)),
            None,
        ),
        UiNode::rounded_rect(
            NodeId("hero_demo.page.panel"),
            hero_panel,
            40.0,
            Some(Color::from_rgba8(38, 24, 48, 255)),
            Some(UiStroke {
                width: 1.0,
                color: Color::from_rgba8(255, 255, 255, 20),
            }),
        ),
        UiNode::rounded_rect(
            NodeId("hero_demo.accent"),
            Rect::new(width * 0.18, height * 0.18, width * 0.82, height * 0.32),
            28.0,
            Some(Color::from_rgba8(118, 72, 136, 255)),
            None,
        ),
        UiNode::text(
            NodeId("hero_demo.title"),
            (width as f32 * 0.20, height as f32 * 0.25),
            "Hero Demo Page",
            42.0,
            Color::from_rgba8(250, 244, 255, 255),
        ),
        UiNode::text(
            NodeId("hero_demo.caption"),
            (width as f32 * 0.20, height as f32 * 0.31),
            caption,
            15.0,
            Color::from_rgba8(216, 189, 228, 255),
        )
        .with_effect_hint(UiEffectHint::OutlineTag),
        UiNode::rounded_rect(
            NodeId("hero_demo.body.card"),
            Rect::new(width * 0.20, height * 0.40, width * 0.80, height * 0.68),
            28.0,
            Some(Color::from_rgba8(49, 32, 61, 255)),
            None,
        ),
        UiNode::text(
            NodeId("hero_demo.body.title"),
            (width as f32 * 0.24, height as f32 * 0.49),
            "Push / Pop Test Surface",
            30.0,
            Color::from_rgba8(250, 244, 255, 255),
        ),
        UiNode::text(
            NodeId("hero_demo.body.copy"),
            (width as f32 * 0.24, height as f32 * 0.56),
            "This page is fully opaque.\nHero is disabled until layout and input are ready.",
            18.0,
            Color::from_rgba8(222, 208, 228, 255),
        ),
        UiNode::rounded_rect(
            NodeId("hero_demo.action.primary"),
            Rect::new(width * 0.24, height * 0.73, width * 0.40, height * 0.80),
            999.0,
            Some(Color::from_rgba8(228, 188, 97, 255)),
            None,
        ),
        UiNode::text(
            NodeId("hero_demo.action.primary.label"),
            (width as f32 * 0.275, height as f32 * 0.775),
            "Backspace to Pop",
            17.0,
            Color::from_rgba8(28, 18, 18, 255),
        ),
        UiNode::rounded_rect(
            NodeId("hero_demo.action.secondary"),
            Rect::new(width * 0.44, height * 0.73, width * 0.66, height * 0.80),
            999.0,
            Some(Color::from_rgba8(255, 255, 255, 28)),
            Some(UiStroke {
                width: 1.0,
                color: Color::from_rgba8(255, 255, 255, 28),
            }),
        ),
        UiNode::text(
            NodeId("hero_demo.action.secondary.label"),
            (width as f32 * 0.485, height as f32 * 0.775),
            "Press 1 for Library",
            17.0,
            Color::from_rgba8(243, 234, 247, 255),
        ),
    ])
}

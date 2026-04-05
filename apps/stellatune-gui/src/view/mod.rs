pub mod hero_demo;
pub mod home;
pub mod library;

use crate::app::FrameState;
use crate::navigation::{RouteId, RouteTransition};
use crate::ui::node::UiNode;

pub struct RouteViewSet {
    pub source: Option<UiNode>,
    pub destination: UiNode,
}

pub fn build_demo_routes(
    route: RouteId,
    active_transition: Option<RouteTransition>,
    frame: &FrameState,
    ui_font_family: &str,
) -> RouteViewSet {
    let destination_route = active_transition
        .map(|transition| transition.to_route())
        .unwrap_or(route);
    let destination = match destination_route {
        RouteId::Library => library::build_library_route(active_transition, frame, ui_font_family),
        RouteId::HeroDemo => {
            hero_demo::build_hero_demo_route(active_transition, frame, ui_font_family)
        },
    };

    let source = active_transition.map(|transition| match transition.from_route() {
        RouteId::Library => library::build_library_route(active_transition, frame, ui_font_family),
        RouteId::HeroDemo => {
            hero_demo::build_hero_demo_route(active_transition, frame, ui_font_family)
        },
    });

    RouteViewSet {
        source,
        destination,
    }
}

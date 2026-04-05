pub mod hero_demo;
pub mod home;
pub mod library;

use crate::app::FrameState;
use crate::navigation::{RouteId, RouteTransition};
use crate::ui::layout::node::LaidOutNode;
use crate::ui::node::NodeId;
use crate::ui::node::UiNode;

#[derive(Debug, Clone, Copy, Default)]
pub struct RouteInteractionState {
    pub hovered: Option<NodeId>,
    pub pressed: Option<NodeId>,
}

#[derive(Debug, Clone, Copy)]
pub enum RouteAction {
    Navigate(RouteId),
    Pop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteCursor {
    Default,
    Pointer,
}

#[derive(Debug, Clone, Copy)]
pub struct RouteActionBinding {
    pub target: NodeId,
    pub action: RouteAction,
    pub cursor: RouteCursor,
}

#[derive(Clone)]
pub struct RoutePage {
    pub node: UiNode,
    pub layout: LaidOutNode,
    pub actions: Vec<RouteActionBinding>,
}

pub struct RouteViewSet {
    pub source: Option<RoutePage>,
    pub destination: RoutePage,
}

pub fn build_demo_routes(
    route: RouteId,
    active_transition: Option<RouteTransition>,
    frame: &FrameState,
    ui_font_family: &str,
    interaction: RouteInteractionState,
) -> RouteViewSet {
    let destination_route = active_transition
        .map(|transition| transition.destination_route())
        .unwrap_or(route);
    let destination = match destination_route {
        RouteId::Library => {
            library::build_library_route(active_transition, frame, ui_font_family, interaction)
        },
        RouteId::HeroDemo => {
            hero_demo::build_hero_demo_route(active_transition, frame, ui_font_family, interaction)
        },
    };

    let source = active_transition.map(|transition| match transition.source_route() {
        RouteId::Library => library::build_library_route(
            active_transition,
            frame,
            ui_font_family,
            RouteInteractionState::default(),
        ),
        RouteId::HeroDemo => hero_demo::build_hero_demo_route(
            active_transition,
            frame,
            ui_font_family,
            RouteInteractionState::default(),
        ),
    });

    RouteViewSet {
        source,
        destination,
    }
}

use super::{
    ButtonNode, ButtonVariant, EffectNode, ImageKind, ImageNode, ListNode, PanelNode, PanelStyle,
    SceneGraph, SceneLayer, SceneLayerKind, SceneNode, SceneRect, SceneState, TextNode, TextRole,
};

pub fn build_scene_graph(state: &SceneState) -> SceneGraph {
    let viewport = SceneRect::from_viewport(state.viewport);
    let shell_margin = 6.0;
    let shell = SceneRect::new(
        shell_margin,
        shell_margin,
        (viewport.width - shell_margin * 2.0).max(320.0),
        (viewport.height - shell_margin * 2.0).max(240.0),
    );
    let titlebar_height = 56.0;
    let status_height = 64.0;
    let sidebar_width = if state.sidebar_open { 280.0 } else { 88.0 };
    let queue_width = if state.queue_open { 320.0 } else { 0.0 };
    let content_top = shell.y + titlebar_height + 16.0;
    let content_bottom = shell.y + shell.height - status_height - 18.0;
    let content_height = (content_bottom - content_top).max(120.0);

    let titlebar = SceneRect::new(shell.x + 8.0, shell.y + 8.0, shell.width - 16.0, titlebar_height);
    let sidebar = SceneRect::new(shell.x + 12.0, content_top, sidebar_width, content_height);
    let queue = SceneRect::new(
        shell.x + shell.width - queue_width - 12.0,
        content_top,
        queue_width.max(0.0),
        content_height,
    );
    let main_left = sidebar.x + sidebar.width + 20.0;
    let main_right = if state.queue_open {
        queue.x - 20.0
    } else {
        viewport.width - 24.0
    };
    let main = SceneRect::new(
        main_left,
        content_top,
        (main_right - main_left).max(240.0),
        content_height,
    );
    let status_bar = SceneRect::new(
        shell.x + 12.0,
        shell.y + shell.height - status_height,
        shell.width - 24.0,
        status_height - 12.0,
    );

    let mut layers = Vec::new();

    layers.push(SceneLayer {
        kind: SceneLayerKind::Background,
        nodes: vec![SceneNode::Effect(EffectNode {
            id: "background.fx".to_string(),
            rect: viewport,
            intensity: match state.visual_mode_label.as_str() {
                "Pulse" => 0.65,
                "Immersive" => 1.0,
                _ => 0.3,
            },
            animated: state.animation_active,
        })],
    });

    layers.push(SceneLayer {
        kind: SceneLayerKind::Panels,
        nodes: build_panel_nodes(state, shell, titlebar, sidebar, main, queue, status_bar),
    });

    layers.push(SceneLayer {
        kind: SceneLayerKind::Content,
        nodes: build_content_nodes(state, titlebar, sidebar, main, queue),
    });

    layers.push(SceneLayer {
        kind: SceneLayerKind::Text,
        nodes: build_text_nodes(state, titlebar, status_bar),
    });

    if state.debug_overlay {
        layers.push(SceneLayer {
            kind: SceneLayerKind::Overlay,
            nodes: build_debug_overlay(state, viewport),
        });
    }

    SceneGraph { layers }
}

fn build_panel_nodes(
    state: &SceneState,
    _shell: SceneRect,
    titlebar: SceneRect,
    sidebar: SceneRect,
    main: SceneRect,
    queue: SceneRect,
    status_bar: SceneRect,
) -> Vec<SceneNode> {
    let mut nodes = vec![
        SceneNode::Panel(PanelNode {
            id: "titlebar".to_string(),
            rect: titlebar,
            style: PanelStyle::Titlebar,
        }),
        SceneNode::Panel(PanelNode {
            id: "sidebar".to_string(),
            rect: sidebar,
            style: PanelStyle::Sidebar,
        }),
        SceneNode::Panel(PanelNode {
            id: "main".to_string(),
            rect: main,
            style: PanelStyle::Main,
        }),
        SceneNode::Panel(PanelNode {
            id: "status".to_string(),
            rect: status_bar,
            style: PanelStyle::Status,
        }),
    ];

    if state.queue_open {
        nodes.push(SceneNode::Panel(PanelNode {
            id: "queue".to_string(),
            rect: queue,
            style: PanelStyle::Queue,
        }));
    }

    nodes
}

fn build_content_nodes(
    state: &SceneState,
    titlebar: SceneRect,
    sidebar: SceneRect,
    main: SceneRect,
    queue: SceneRect,
) -> Vec<SceneNode> {
    let nav_x = sidebar.x + 18.0;
    let nav_w = sidebar.width - 36.0;
    let title_actions_y = titlebar.y + 12.0;
    let title_actions_x = titlebar.x + titlebar.width - 180.0;
    let cover_rect = SceneRect::new(main.x + 28.0, main.y + 24.0, 228.0, 228.0);
    let controls_x = cover_rect.x + cover_rect.width + 28.0;
    let list_rect = SceneRect::new(main.x + 28.0, main.y + 286.0, main.width - 56.0, 220.0);

    let mut nodes = vec![
        SceneNode::Button(ButtonNode {
            id: "window-minimize".to_string(),
            rect: SceneRect::new(title_actions_x, title_actions_y, 48.0, 30.0),
            label: "Min".to_string(),
            variant: ButtonVariant::Ghost,
            selected: false,
        }),
        SceneNode::Button(ButtonNode {
            id: "window-maximize".to_string(),
            rect: SceneRect::new(title_actions_x + 58.0, title_actions_y, 48.0, 30.0),
            label: "Max".to_string(),
            variant: ButtonVariant::Ghost,
            selected: false,
        }),
        SceneNode::Button(ButtonNode {
            id: "window-close".to_string(),
            rect: SceneRect::new(title_actions_x + 116.0, title_actions_y, 56.0, 30.0),
            label: "Close".to_string(),
            variant: ButtonVariant::Ghost,
            selected: false,
        }),
        SceneNode::Button(ButtonNode {
            id: "nav-library".to_string(),
            rect: SceneRect::new(nav_x, sidebar.y + 84.0, nav_w, 36.0),
            label: "Library".to_string(),
            variant: ButtonVariant::Nav,
            selected: state.route_label == "Library",
        }),
        SceneNode::Button(ButtonNode {
            id: "nav-now-playing".to_string(),
            rect: SceneRect::new(nav_x, sidebar.y + 128.0, nav_w, 36.0),
            label: "Now Playing".to_string(),
            variant: ButtonVariant::Nav,
            selected: state.route_label == "Now Playing",
        }),
        SceneNode::Button(ButtonNode {
            id: "nav-settings".to_string(),
            rect: SceneRect::new(nav_x, sidebar.y + 172.0, nav_w, 36.0),
            label: "Settings".to_string(),
            variant: ButtonVariant::Nav,
            selected: state.route_label == "Settings",
        }),
        SceneNode::Button(ButtonNode {
            id: "sidebar-toggle".to_string(),
            rect: SceneRect::new(nav_x, sidebar.y + sidebar.height - 52.0, nav_w, 32.0),
            label: if state.sidebar_open {
                "Collapse Sidebar".to_string()
            } else {
                "Expand Sidebar".to_string()
            },
            variant: ButtonVariant::Ghost,
            selected: false,
        }),
        SceneNode::Image(ImageNode {
            id: "cover-art".to_string(),
            rect: cover_rect,
            kind: ImageKind::AlbumCover,
            caption: state.track_title.clone(),
        }),
        SceneNode::Button(ButtonNode {
            id: "playback-state".to_string(),
            rect: SceneRect::new(controls_x, main.y + 126.0, 168.0, 36.0),
            label: format!("Playback: {}", state.playback_label),
            variant: ButtonVariant::Primary,
            selected: state.playback_label == "Playing",
        }),
        SceneNode::Button(ButtonNode {
            id: "visual-mode".to_string(),
            rect: SceneRect::new(controls_x + 184.0, main.y + 126.0, 168.0, 36.0),
            label: format!("Visual: {}", state.visual_mode_label),
            variant: ButtonVariant::Secondary,
            selected: state.visual_mode_label != "Calm",
        }),
        SceneNode::Button(ButtonNode {
            id: "queue-toggle".to_string(),
            rect: SceneRect::new(controls_x, main.y + 174.0, 168.0, 36.0),
            label: if state.queue_open {
                "Hide Queue".to_string()
            } else {
                "Open Queue".to_string()
            },
            variant: ButtonVariant::Secondary,
            selected: state.queue_open,
        }),
        SceneNode::List(ListNode {
            id: "content-list".to_string(),
            rect: list_rect,
            title: match state.route_label.as_str() {
                "Settings" => "Settings".to_string(),
                "Now Playing" => "Playback Queue".to_string(),
                _ => "Library Tracks".to_string(),
            },
            items: build_main_list_items(state),
            selected_index: Some(0),
            compact: false,
        }),
    ];

    if state.queue_open {
        nodes.push(SceneNode::List(ListNode {
            id: "queue-list".to_string(),
            rect: SceneRect::new(
                queue.x + 18.0,
                queue.y + 18.0,
                queue.width - 36.0,
                queue.height - 36.0,
            ),
            title: "Queue".to_string(),
            items: vec![
                "Current Track".to_string(),
                "Next Up Placeholder".to_string(),
                "Future Queue Item".to_string(),
            ],
            selected_index: Some(0),
            compact: true,
        }));
    }

    nodes
}

fn build_text_nodes(state: &SceneState, titlebar: SceneRect, status_bar: SceneRect) -> Vec<SceneNode> {
    vec![
        SceneNode::Text(TextNode {
            id: "app-title".to_string(),
            rect: SceneRect::new(titlebar.x + 18.0, titlebar.y + 10.0, 320.0, 22.0),
            content: state.app_title.clone(),
            role: TextRole::Title,
        }),
        SceneNode::Text(TextNode {
            id: "titlebar-route".to_string(),
            rect: SceneRect::new(titlebar.x + 18.0, titlebar.y + 30.0, 260.0, 18.0),
            content: format!("{}  |  {}", state.route_label, state.visual_mode_label),
            role: TextRole::Status,
        }),
        SceneNode::Text(TextNode {
            id: "track-title".to_string(),
            rect: SceneRect::new(titlebar.x + 300.0, titlebar.y + 34.0, 540.0, 42.0),
            content: state.track_title.clone(),
            role: TextRole::Hero,
        }),
        SceneNode::Text(TextNode {
            id: "track-subtitle".to_string(),
            rect: SceneRect::new(titlebar.x + 300.0, titlebar.y + 68.0, 560.0, 28.0),
            content: state.track_subtitle.clone(),
            role: TextRole::Body,
        }),
        SceneNode::Text(TextNode {
            id: "status-text".to_string(),
            rect: status_bar.inset(20.0, 16.0),
            content: state.status.clone(),
            role: TextRole::Status,
        }),
    ]
}

fn build_debug_overlay(state: &SceneState, viewport: SceneRect) -> Vec<SceneNode> {
    let pointer = state
        .pointer
        .map(|(x, y)| format!("{x:.0}, {y:.0}"))
        .unwrap_or_else(|| "none".to_string());
    let hovered = state.hovered_node_id.as_deref().unwrap_or("none");

    vec![
        SceneNode::Panel(PanelNode {
            id: "debug-overlay".to_string(),
            rect: SceneRect::new(viewport.width - 340.0, 24.0, 316.0, 148.0),
            style: PanelStyle::Overlay,
        }),
        SceneNode::Text(TextNode {
            id: "debug-content".to_string(),
            rect: SceneRect::new(viewport.width - 320.0, 40.0, 280.0, 116.0),
            content: format!(
                "debug: on\nviewport: {:.0}x{:.0}\npointer: {}\nhovered: {}\nanimated: {}\nlayers: {}",
                viewport.width,
                viewport.height,
                pointer,
                hovered,
                if state.animation_active { "yes" } else { "no" },
                state.graph.layers.len().max(4),
            ),
            role: TextRole::Debug,
        }),
    ]
}

fn build_main_list_items(state: &SceneState) -> Vec<String> {
    match state.route_label.as_str() {
        "Settings" => vec![
            "Output Device".to_string(),
            "DSP Chain".to_string(),
            "Theme".to_string(),
            "Renderer Diagnostics".to_string(),
        ],
        "Now Playing" => vec![
            "Current Track".to_string(),
            "Lyrics".to_string(),
            "Waveform".to_string(),
            "Visual Presets".to_string(),
        ],
        _ => vec![
            "Track 01".to_string(),
            "Track 02".to_string(),
            "Track 03".to_string(),
            "Track 04".to_string(),
        ],
    }
}

#![allow(dead_code, clippy::needless_range_loop)]

use winit::dpi::PhysicalSize;

use crate::ui::node::{NodeId, UiEffectHint, UiLayer, UiNode, UiStroke, UiTransitionHint};
use vello::kurbo::{Affine, Rect};
use vello::peniko::Color;

const LYRIC_NODE_IDS: [NodeId; 7] = [
    NodeId("home.lyrics.0"),
    NodeId("home.lyrics.1"),
    NodeId("home.lyrics.2"),
    NodeId("home.lyrics.3"),
    NodeId("home.lyrics.4"),
    NodeId("home.lyrics.5"),
    NodeId("home.lyrics.6"),
];

const ORBIT_NODE_IDS: [NodeId; 5] = [
    NodeId("home.cover.orbit.0"),
    NodeId("home.cover.orbit.1"),
    NodeId("home.cover.orbit.2"),
    NodeId("home.cover.orbit.3"),
    NodeId("home.cover.orbit.4"),
];

#[derive(Debug, Clone)]
pub struct HomeShellModel {
    pub inset: [f64; 4],
}

#[derive(Debug, Clone)]
pub struct HomeCoverModel {
    pub rect: [f64; 4],
    pub orbit_phase: f64,
    pub orbit_count: usize,
}

#[derive(Debug, Clone)]
pub struct HomeHeaderModel {
    pub title: String,
    pub subtitle: String,
    pub caption: String,
}

#[derive(Debug, Clone)]
pub struct HomeFocusChipModel {
    pub rect: [f64; 4],
    pub title: String,
    pub subtitle: String,
    pub subtitle_opacity: f32,
    pub accent_center: (f64, f64),
    pub accent_radius: f64,
    pub detail_rects: [[f64; 4]; 2],
    pub detail_opacity: f32,
    pub control_rects: [[f64; 4]; 2],
    pub control_labels: [String; 2],
    pub control_opacity: f32,
    pub scrim_opacity: f32,
    pub state: HomeFocusChipState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeFocusChipState {
    Compact,
    Promoted,
}

#[derive(Debug, Clone)]
pub struct HomeProgressModel {
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct HomeLyricsModel {
    pub line_scales: [f64; 7],
}

#[derive(Debug, Clone)]
pub struct HomeDiagnosticsModel {
    pub fps_fill: f64,
}

#[derive(Debug, Clone)]
pub struct HomeViewModel {
    pub viewport: PhysicalSize<u32>,
    pub shell: HomeShellModel,
    pub cover: HomeCoverModel,
    pub header: HomeHeaderModel,
    pub focus_chip: HomeFocusChipModel,
    pub progress: HomeProgressModel,
    pub lyrics: HomeLyricsModel,
    pub diagnostics: HomeDiagnosticsModel,
}

impl HomeViewModel {
    pub fn scene_graph(&self) -> UiNode {
        let width = self.viewport.width.max(1) as f64;
        let height = self.viewport.height.max(1) as f64;
        let cover_rect = Rect::new(
            self.cover.rect[0],
            self.cover.rect[1],
            self.cover.rect[2],
            self.cover.rect[3],
        );
        let title_origin = (width as f32 * 0.56, height as f32 * 0.17);
        let subtitle_origin = (width as f32 * 0.56, height as f32 * 0.22);
        let caption_origin = (width as f32 * 0.56, height as f32 * 0.325);
        let focus_chip_rect = Rect::new(
            self.focus_chip.rect[0],
            self.focus_chip.rect[1],
            self.focus_chip.rect[2],
            self.focus_chip.rect[3],
        );
        let focus_chip_label_origin = (
            (focus_chip_rect.x0 + 18.0) as f32,
            (focus_chip_rect.y0
                + if matches!(self.focus_chip.state, HomeFocusChipState::Promoted) {
                    20.0
                } else {
                    17.0
                }) as f32,
        );
        let focus_chip_subtitle_origin = (
            (focus_chip_rect.x0 + 18.0) as f32,
            (focus_chip_rect.y0
                + if matches!(self.focus_chip.state, HomeFocusChipState::Promoted) {
                    58.0
                } else {
                    45.0
                }) as f32,
        );
        let focus_chip_control_label_origins = [
            (
                (self.focus_chip.control_rects[0][0] + 14.0) as f32,
                (self.focus_chip.control_rects[0][1] + 18.0) as f32,
            ),
            (
                (self.focus_chip.control_rects[1][0] + 14.0) as f32,
                (self.focus_chip.control_rects[1][1] + 18.0) as f32,
            ),
        ];
        let hero_center = (
            (self.cover.rect[0] + self.cover.rect[2]) * 0.5,
            (self.cover.rect[1] + self.cover.rect[3]) * 0.5,
        );

        let shell_nodes = vec![
            UiNode::rounded_rect(
                NodeId("home.shell.scrim"),
                Rect::new(0.0, 0.0, width, height),
                0.0,
                Some(Color::from_rgba8(3, 5, 8, 186)),
                None,
            )
            .with_layer(UiLayer::Background)
            .with_opacity(self.focus_chip.scrim_opacity)
            .with_transition_hint(UiTransitionHint::LayoutDriven),
            UiNode::rounded_rect(
                NodeId("home.shell.fill"),
                Rect::new(
                    self.shell.inset[0],
                    self.shell.inset[1],
                    width - self.shell.inset[2],
                    height - self.shell.inset[3],
                ),
                36.0,
                Some(Color::from_rgba8(8, 14, 18, 214)),
                None,
            )
            .with_layer(UiLayer::Background),
            UiNode::rounded_rect(
                NodeId("home.shell.stroke"),
                Rect::new(
                    self.shell.inset[0],
                    self.shell.inset[1],
                    width - self.shell.inset[2],
                    height - self.shell.inset[3],
                ),
                36.0,
                None,
                Some(UiStroke {
                    width: 2.0,
                    color: Color::from_rgba8(255, 255, 255, 26),
                }),
            )
            .with_layer(UiLayer::Overlay),
        ];

        let mut cover_children = vec![
            UiNode::media_slot(NodeId("home.cover.media"), cover_rect, 28.0)
                .with_layer(UiLayer::Content)
                .with_effect_hint(UiEffectHint::PromoteSurface)
                .with_transition_hint(UiTransitionHint::SharedElement),
            UiNode::rounded_rect(
                NodeId("home.cover.frame"),
                cover_rect,
                28.0,
                Some(Color::from_rgba8(255, 255, 255, 20)),
                Some(UiStroke {
                    width: 2.0,
                    color: Color::from_rgba8(255, 250, 244, 38),
                }),
            )
            .with_layer(UiLayer::Overlay),
            UiNode::circle(
                NodeId("home.cover.disc"),
                hero_center,
                cover_rect.width().min(cover_rect.height()) * 0.32,
                Some(Color::from_rgba8(16, 20, 24, 230)),
                Some(UiStroke {
                    width: 10.0,
                    color: Color::from_rgba8(255, 244, 228, 50),
                }),
            ),
        ];

        for index in 0..self.cover.orbit_count.min(ORBIT_NODE_IDS.len()) {
            let angle = self.cover.orbit_phase + (index as f64 * (std::f64::consts::TAU / 3.0));
            cover_children.push(
                UiNode::circle(
                    ORBIT_NODE_IDS[index],
                    (
                        hero_center.0 + angle.cos() * width * 0.05,
                        hero_center.1 + angle.sin() * height * 0.04,
                    ),
                    9.0 + index as f64 * 3.0,
                    Some(Color::from_rgba8(255, 235, 206, 168)),
                    None,
                )
                .with_opacity(0.92),
            );
        }

        let progress_nodes = vec![
            UiNode::rounded_rect(
                NodeId("home.progress.rail"),
                Rect::new(width * 0.56, height * 0.28, width * 0.88, height * 0.31),
                999.0,
                Some(Color::from_rgba8(255, 255, 255, 26)),
                None,
            ),
            UiNode::rounded_rect(
                NodeId("home.progress.fill"),
                Rect::new(
                    width * 0.56,
                    height * 0.28,
                    width * (0.56 + 0.32 * self.progress.value),
                    height * 0.31,
                ),
                999.0,
                Some(Color::from_rgba8(246, 196, 72, 255)),
                None,
            )
            .with_transition_hint(UiTransitionHint::LayoutDriven),
        ];

        let header_nodes = vec![
            UiNode::text(
                NodeId("home.header.title"),
                title_origin,
                self.header.title.clone(),
                40.0,
                Color::from_rgba8(255, 248, 239, 255),
            )
            .with_effect_hint(UiEffectHint::Halo),
            UiNode::text(
                NodeId("home.header.subtitle"),
                subtitle_origin,
                self.header.subtitle.clone(),
                18.0,
                Color::from_rgba8(255, 248, 239, 160),
            )
            .with_effect_hint(UiEffectHint::Underline),
            UiNode::text(
                NodeId("home.header.caption"),
                caption_origin,
                self.header.caption.clone(),
                14.0,
                Color::from_rgba8(246, 196, 72, 220),
            )
            .with_effect_hint(UiEffectHint::OutlineTag),
        ];

        let focus_chip_nodes = vec![
            UiNode::rounded_rect(
                NodeId("home.focus_chip.body"),
                focus_chip_rect,
                999.0,
                Some(match self.focus_chip.state {
                    HomeFocusChipState::Compact => Color::from_rgba8(246, 196, 72, 228),
                    HomeFocusChipState::Promoted => Color::from_rgba8(255, 214, 104, 240),
                }),
                Some(UiStroke {
                    width: 1.0,
                    color: Color::from_rgba8(255, 249, 241, 68),
                }),
            )
            .with_transition_hint(UiTransitionHint::SharedElement),
            UiNode::circle(
                NodeId("home.focus_chip.accent"),
                self.focus_chip.accent_center,
                self.focus_chip.accent_radius,
                Some(Color::from_rgba8(18, 20, 24, 210)),
                Some(UiStroke {
                    width: 2.0,
                    color: Color::from_rgba8(255, 248, 239, 110),
                }),
            )
            .with_transition_hint(UiTransitionHint::SharedElement),
            UiNode::text(
                NodeId("home.focus_chip.title"),
                focus_chip_label_origin,
                self.focus_chip.title.clone(),
                14.0,
                Color::from_rgba8(14, 16, 18, 255),
            )
            .with_transition_hint(UiTransitionHint::SharedElement),
            UiNode::text(
                NodeId("home.focus_chip.subtitle"),
                focus_chip_subtitle_origin,
                self.focus_chip.subtitle.clone(),
                if matches!(self.focus_chip.state, HomeFocusChipState::Promoted) {
                    15.0
                } else {
                    13.0
                },
                Color::from_rgba8(14, 16, 18, 210),
            )
            .with_opacity(self.focus_chip.subtitle_opacity)
            .with_transition_hint(UiTransitionHint::SharedElement),
            UiNode::rounded_rect(
                NodeId("home.focus_chip.detail.0"),
                Rect::new(
                    self.focus_chip.detail_rects[0][0],
                    self.focus_chip.detail_rects[0][1],
                    self.focus_chip.detail_rects[0][2],
                    self.focus_chip.detail_rects[0][3],
                ),
                999.0,
                Some(Color::from_rgba8(18, 20, 24, 78)),
                None,
            )
            .with_opacity(self.focus_chip.detail_opacity)
            .with_transition_hint(UiTransitionHint::LayoutDriven),
            UiNode::rounded_rect(
                NodeId("home.focus_chip.detail.1"),
                Rect::new(
                    self.focus_chip.detail_rects[1][0],
                    self.focus_chip.detail_rects[1][1],
                    self.focus_chip.detail_rects[1][2],
                    self.focus_chip.detail_rects[1][3],
                ),
                999.0,
                Some(Color::from_rgba8(18, 20, 24, 54)),
                None,
            )
            .with_opacity(self.focus_chip.detail_opacity)
            .with_transition_hint(UiTransitionHint::LayoutDriven),
            UiNode::rounded_rect(
                NodeId("home.focus_chip.control.primary"),
                Rect::new(
                    self.focus_chip.control_rects[0][0],
                    self.focus_chip.control_rects[0][1],
                    self.focus_chip.control_rects[0][2],
                    self.focus_chip.control_rects[0][3],
                ),
                999.0,
                Some(Color::from_rgba8(18, 20, 24, 198)),
                Some(UiStroke {
                    width: 1.0,
                    color: Color::from_rgba8(255, 248, 239, 30),
                }),
            )
            .with_opacity(self.focus_chip.control_opacity)
            .with_transition_hint(UiTransitionHint::LayoutDriven),
            UiNode::text(
                NodeId("home.focus_chip.control.primary.label"),
                focus_chip_control_label_origins[0],
                self.focus_chip.control_labels[0].clone(),
                12.0,
                Color::from_rgba8(255, 248, 239, 235),
            )
            .with_opacity(self.focus_chip.control_opacity)
            .with_transition_hint(UiTransitionHint::LayoutDriven),
            UiNode::rounded_rect(
                NodeId("home.focus_chip.control.secondary"),
                Rect::new(
                    self.focus_chip.control_rects[1][0],
                    self.focus_chip.control_rects[1][1],
                    self.focus_chip.control_rects[1][2],
                    self.focus_chip.control_rects[1][3],
                ),
                999.0,
                Some(Color::from_rgba8(255, 255, 255, 28)),
                None,
            )
            .with_opacity(self.focus_chip.control_opacity)
            .with_transition_hint(UiTransitionHint::LayoutDriven),
            UiNode::text(
                NodeId("home.focus_chip.control.secondary.label"),
                focus_chip_control_label_origins[1],
                self.focus_chip.control_labels[1].clone(),
                12.0,
                Color::from_rgba8(18, 20, 24, 220),
            )
            .with_opacity(self.focus_chip.control_opacity)
            .with_transition_hint(UiTransitionHint::LayoutDriven),
        ];

        let lyric_nodes = self
            .lyrics
            .line_scales
            .iter()
            .enumerate()
            .map(|(row, scale)| {
                let y = height * (0.38 + row as f64 * 0.065);
                UiNode::rounded_rect(
                    LYRIC_NODE_IDS[row],
                    Rect::new(
                        width * 0.56,
                        y,
                        width * (0.56 + 0.32 * *scale),
                        y + height * 0.022,
                    ),
                    12.0,
                    Some(Color::from_rgba8(
                        255,
                        248,
                        239,
                        90u8.saturating_sub((row as u8) * 6),
                    )),
                    None,
                )
                .with_transition_hint(UiTransitionHint::LayoutDriven)
            })
            .collect();

        let diagnostics_nodes = vec![
            UiNode::rounded_rect(
                NodeId("home.diagnostics.fps"),
                Rect::new(
                    width * 0.79,
                    height * 0.16,
                    width * (0.79 + 0.09 * self.diagnostics.fps_fill.max(0.35)),
                    height * 0.195,
                ),
                999.0,
                Some(Color::from_rgba8(16, 210, 184, 220)),
                None,
            )
            .with_layer(UiLayer::Overlay),
        ];

        UiNode::group(NodeId("home.root"))
            .with_children(vec![
                UiNode::group(NodeId("home.shell")).with_children(shell_nodes),
                UiNode::group(NodeId("home.cover")).with_children(cover_children),
                UiNode::group(NodeId("home.focus_chip")).with_children(focus_chip_nodes),
                UiNode::group(NodeId("home.progress")).with_children(progress_nodes),
                UiNode::group(NodeId("home.header"))
                    .with_layer(UiLayer::Overlay)
                    .with_children(header_nodes),
                UiNode::group(NodeId("home.lyrics")).with_children(lyric_nodes),
                UiNode::group(NodeId("home.diagnostics"))
                    .with_layer(UiLayer::Overlay)
                    .with_children(diagnostics_nodes),
            ])
            .with_transform(Affine::IDENTITY)
    }
}

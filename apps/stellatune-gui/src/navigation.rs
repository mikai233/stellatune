use crate::page_transition::PageTransitionPreset;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RouteId {
    #[default]
    Library,
    HeroDemo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteEntry {
    id: RouteId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationOperation {
    Push,
    Pop,
    Replace,
}

#[derive(Debug, Clone, Copy)]
pub struct RouteTransition {
    pub operation: NavigationOperation,
    pub from: RouteEntry,
    pub to: RouteEntry,
    pub preset: PageTransitionPreset,
    pub started_at: f32,
    pub duration_seconds: f32,
}

#[derive(Debug)]
pub struct NavigationState {
    stack: Vec<RouteEntry>,
    active_transition: Option<RouteTransition>,
    last_demo_bucket: Option<i32>,
}

impl RouteEntry {
    pub const fn new(id: RouteId) -> Self {
        Self { id }
    }

    pub const fn id(self) -> RouteId {
        self.id
    }
}

impl NavigationState {
    pub fn new() -> Self {
        Self {
            stack: vec![RouteEntry::new(RouteId::Library)],
            active_transition: None,
            last_demo_bucket: None,
        }
    }

    pub fn top_entry(&self) -> RouteEntry {
        self.stack
            .last()
            .copied()
            .expect("navigation stack should never be empty")
    }

    pub fn top_route(&self) -> RouteId {
        self.top_entry().id()
    }

    pub fn can_pop(&self) -> bool {
        self.stack.len() > 1
    }

    pub fn active_transition(&self, now_seconds: f32) -> Option<RouteTransition> {
        self.active_transition
            .filter(|transition| !transition.is_finished(now_seconds))
    }

    pub fn update_demo_timeline(&mut self, now_seconds: f32) {
        let bucket = (now_seconds / 4.8).floor() as i32;
        if self.last_demo_bucket == Some(bucket) {
            return;
        }

        self.last_demo_bucket = Some(bucket);
        let target = if bucket % 2 == 0 {
            RouteId::Library
        } else {
            RouteId::HeroDemo
        };
        self.navigate_to(target, now_seconds);
    }

    pub fn toggle_demo_route(&mut self, now_seconds: f32) {
        match self.top_route() {
            RouteId::Library => {
                self.push(RouteId::HeroDemo, now_seconds);
            },
            RouteId::HeroDemo => {
                let _ = self.pop(now_seconds);
            },
        }
        self.last_demo_bucket = None;
    }

    pub fn navigate_to(&mut self, target: RouteId, now_seconds: f32) {
        self.finish_expired_transition(now_seconds);
        if self.top_route() == target {
            return;
        }

        if self
            .stack
            .iter()
            .rev()
            .nth(1)
            .is_some_and(|entry| entry.id() == target)
        {
            let _ = self.pop(now_seconds);
            return;
        }

        match target {
            RouteId::HeroDemo => self.push(target, now_seconds),
            RouteId::Library => self.replace_top(target, now_seconds),
        }
    }

    pub fn push(&mut self, target: RouteId, now_seconds: f32) {
        self.finish_expired_transition(now_seconds);
        let from = self.top_entry();
        let to = RouteEntry::new(target);
        if from == to {
            return;
        }

        self.stack.push(to);
        self.active_transition = Some(RouteTransition {
            operation: NavigationOperation::Push,
            from,
            to,
            preset: default_page_transition_preset(NavigationOperation::Push, from, to),
            started_at: now_seconds,
            duration_seconds: 0.9,
        });
    }

    pub fn pop(&mut self, now_seconds: f32) -> bool {
        self.finish_expired_transition(now_seconds);
        if !self.can_pop() {
            return false;
        }

        let from = self.top_entry();
        let _ = self.stack.pop();
        let to = self.top_entry();
        self.active_transition = Some(RouteTransition {
            operation: NavigationOperation::Pop,
            from,
            to,
            preset: default_page_transition_preset(NavigationOperation::Pop, from, to),
            started_at: now_seconds,
            duration_seconds: 0.9,
        });
        true
    }

    pub fn replace_top(&mut self, target: RouteId, now_seconds: f32) {
        self.finish_expired_transition(now_seconds);
        let from = self.top_entry();
        let to = RouteEntry::new(target);
        if from == to {
            return;
        }

        if let Some(top) = self.stack.last_mut() {
            *top = to;
        }
        self.active_transition = Some(RouteTransition {
            operation: NavigationOperation::Replace,
            from,
            to,
            preset: default_page_transition_preset(NavigationOperation::Replace, from, to),
            started_at: now_seconds,
            duration_seconds: 0.9,
        });
    }

    fn finish_expired_transition(&mut self, now_seconds: f32) {
        if self
            .active_transition
            .is_some_and(|transition| transition.is_finished(now_seconds))
        {
            self.active_transition = None;
        }
    }
}

impl Default for NavigationState {
    fn default() -> Self {
        Self::new()
    }
}

impl RouteTransition {
    pub fn operation(self) -> NavigationOperation {
        self.operation
    }

    pub fn source_route(self) -> RouteId {
        self.from.id()
    }

    pub fn destination_route(self) -> RouteId {
        self.to.id()
    }

    pub fn preset(self) -> PageTransitionPreset {
        self.preset
    }

    pub fn is_finished(self, now_seconds: f32) -> bool {
        (now_seconds - self.started_at) >= self.duration_seconds
    }
}

fn default_page_transition_preset(
    operation: NavigationOperation,
    from: RouteEntry,
    to: RouteEntry,
) -> PageTransitionPreset {
    match (from.id(), to.id(), operation) {
        (RouteId::Library, RouteId::HeroDemo, NavigationOperation::Push)
        | (RouteId::HeroDemo, RouteId::Library, NavigationOperation::Pop) => {
            PageTransitionPreset::SharedAxisZ
        },
        (RouteId::Library, RouteId::HeroDemo, NavigationOperation::Pop)
        | (RouteId::HeroDemo, RouteId::Library, NavigationOperation::Push)
        | (RouteId::Library, RouteId::HeroDemo, NavigationOperation::Replace)
        | (RouteId::HeroDemo, RouteId::Library, NavigationOperation::Replace) => {
            PageTransitionPreset::FadeThrough
        },
        _ if matches!(operation, NavigationOperation::Replace) => PageTransitionPreset::Crossfade,
        _ => PageTransitionPreset::OpaqueCover,
    }
}

#[derive(Default)]
pub(crate) struct OutputCallbackPlatformState;

impl OutputCallbackPlatformState {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn on_callback_start(&mut self, _format_label: &str) {}
}

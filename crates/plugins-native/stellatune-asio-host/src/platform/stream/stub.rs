#[derive(Default)]
pub(crate) struct OutputCallbackPlatformState;

impl OutputCallbackPlatformState {
    pub(crate) fn on_callback_start(&mut self, _format_label: &str) {}
}

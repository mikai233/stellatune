use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::{
    AVRT_PRIORITY_HIGH, AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority,
};
use windows::core::HSTRING;

pub(super) struct MmcssGuard(#[allow(dead_code)] HANDLE);

unsafe impl Send for MmcssGuard {}

#[derive(Default)]
pub(crate) struct OutputCallbackPlatformState {
    attempted: bool,
    guard: Option<MmcssGuard>,
}

impl OutputCallbackPlatformState {
    pub(crate) fn on_callback_start(&mut self, format_label: &str) {
        if self.attempted {
            return;
        }
        self.attempted = true;
        self.guard = enable_mmcss_pro_audio();
        if self.guard.is_some() {
            eprintln!("asio host mmcss: Pro Audio enabled ({format_label})");
        } else {
            eprintln!("asio host mmcss: failed to enable Pro Audio ({format_label})");
        }
    }
}

fn enable_mmcss_pro_audio() -> Option<MmcssGuard> {
    let mut task_index = 0u32;
    let task = HSTRING::from("Pro Audio");
    let handle = unsafe { AvSetMmThreadCharacteristicsW(&task, &mut task_index) }.ok()?;
    let _ = unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH) };
    Some(MmcssGuard(handle))
}

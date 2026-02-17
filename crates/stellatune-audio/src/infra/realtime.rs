#[cfg(windows)]
use windows::Win32::System::Threading::{
    AVRT_PRIORITY_HIGH, AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
    AvSetMmThreadPriority,
};

/// Best-effort realtime hint for audio-critical worker threads.
///
/// On Windows this enables MMCSS "Pro Audio" for the current thread and keeps
/// it active for the guard lifetime. On other platforms it is a no-op.
pub(crate) struct RealtimeThreadGuard {
    #[cfg(windows)]
    _mmcss: Option<MmcssGuard>,
}

pub(crate) fn enable_realtime_audio_thread() -> RealtimeThreadGuard {
    #[cfg(windows)]
    {
        RealtimeThreadGuard {
            _mmcss: enable_mmcss_pro_audio(),
        }
    }
    #[cfg(not(windows))]
    {
        RealtimeThreadGuard {}
    }
}

#[cfg(windows)]
struct MmcssGuard(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for MmcssGuard {
    fn drop(&mut self) {
        // Best-effort revert. Nothing we can do if it fails.
        let _ = unsafe { AvRevertMmThreadCharacteristics(self.0) };
    }
}

#[cfg(windows)]
fn enable_mmcss_pro_audio() -> Option<MmcssGuard> {
    let mut task_index = 0u32;
    let task = windows::core::HSTRING::from("Pro Audio");
    let handle = unsafe { AvSetMmThreadCharacteristicsW(&task, &mut task_index) }.ok()?;
    let _ = unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH) };
    Some(MmcssGuard(handle))
}

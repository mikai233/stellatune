#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
use windows::Win32::System::Threading::{
    AVRT_PRIORITY_HIGH, AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
    AvSetMmThreadPriority, GetCurrentProcess, GetCurrentThread,
    PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
    PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION, PROCESS_POWER_THROTTLING_STATE,
    ProcessPowerThrottling, SetProcessInformation, SetThreadInformation,
    THREAD_POWER_THROTTLING_CURRENT_VERSION, THREAD_POWER_THROTTLING_EXECUTION_SPEED,
    THREAD_POWER_THROTTLING_STATE, ThreadPowerThrottling,
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
        configure_audio_process_for_background_playback();
        disable_current_thread_power_throttling();
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

#[cfg(windows)]
fn configure_audio_process_for_background_playback() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED
                | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            StateMask: 0,
        };
        let _ = unsafe {
            SetProcessInformation(
                GetCurrentProcess(),
                ProcessPowerThrottling,
                (&state as *const PROCESS_POWER_THROTTLING_STATE).cast(),
                std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
            )
        };
    });
}

#[cfg(windows)]
fn disable_current_thread_power_throttling() {
    let state = THREAD_POWER_THROTTLING_STATE {
        Version: THREAD_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: THREAD_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    let _ = unsafe {
        SetThreadInformation(
            GetCurrentThread(),
            ThreadPowerThrottling,
            (&state as *const THREAD_POWER_THROTTLING_STATE).cast(),
            std::mem::size_of::<THREAD_POWER_THROTTLING_STATE>() as u32,
        )
    };
}

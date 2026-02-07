#[cfg(windows)]
use windows::Win32::System::Threading::{
    AVRT_PRIORITY_HIGH, AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
    AvSetMmThreadPriority,
};

#[cfg(windows)]
pub struct MmcssGuard(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for MmcssGuard {
    fn drop(&mut self) {
        // Best-effort revert. Nothing we can do if it fails.
        let _ = unsafe { AvRevertMmThreadCharacteristics(self.0) };
    }
}

#[cfg(windows)]
pub fn enable_mmcss_pro_audio() -> Option<MmcssGuard> {
    let mut task_index = 0u32;
    let task = windows::core::HSTRING::from("Pro Audio");
    let handle = unsafe { AvSetMmThreadCharacteristicsW(&task, &mut task_index) }.ok()?;
    let _ = unsafe { AvSetMmThreadPriority(handle, AVRT_PRIORITY_HIGH) };
    Some(MmcssGuard(handle))
}

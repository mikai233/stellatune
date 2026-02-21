use crate::common::DisableReason;
use crate::error::SdkResult;

pub trait PluginLifecycle {
    fn on_enable(&mut self) -> SdkResult<()> {
        Ok(())
    }

    fn on_disable(&mut self, _reason: DisableReason) -> SdkResult<()> {
        Ok(())
    }
}

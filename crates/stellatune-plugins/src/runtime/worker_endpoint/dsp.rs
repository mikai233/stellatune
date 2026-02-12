use anyhow::{Result, anyhow};
use stellatune_plugin_api::StDspInstanceRef;

use crate::capabilities::common::{status_to_result, ststr_from_str};
use crate::capabilities::dsp::DspInstance;
use crate::runtime::handle::PluginRuntimeHandle;
use crate::runtime::worker_controller::{WorkerConfigurableInstance, WorkerInstanceFactory};

use super::common::{
    acquire_active_lease, new_factory_state, new_instance_runtime_ctx, normalize_plugin_type_ids,
    subscribe_worker_control,
};
use super::{DspInstanceFactory, DspWorkerEndpoint};

impl DspInstanceFactory {
    pub fn create_instance(&self, config_json: &str) -> Result<DspInstance> {
        let lease = acquire_active_lease(&self.runtime, &self.plugin_id)?;
        let Some(create) = lease.loaded.module.create_dsp_instance else {
            return Err(anyhow!(
                "plugin `{}` does not provide dsp factory",
                self.plugin_id
            ));
        };

        let mut raw = StDspInstanceRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (create)(
            ststr_from_str(&self.type_id),
            self.sample_rate,
            self.channels,
            ststr_from_str(config_json),
            &mut raw,
        );
        let plugin_free = lease.loaded.module.plugin_free;
        status_to_result("create_dsp_instance", status, plugin_free)?;

        let ctx = new_instance_runtime_ctx(&self.instances, &self.updates, lease, plugin_free);
        match DspInstance::from_ffi(ctx, raw) {
            Ok(instance) => Ok(instance),
            Err(err) => {
                destroy_raw_dsp_instance(&mut raw);
                Err(err)
            }
        }
    }
}

impl PluginRuntimeHandle {
    pub fn bind_dsp_worker_endpoint(
        &self,
        plugin_id: &str,
        type_id: &str,
        sample_rate: u32,
        channels: u16,
    ) -> Result<DspWorkerEndpoint> {
        let (plugin_id, type_id) = normalize_plugin_type_ids(plugin_id, type_id)?;
        if sample_rate == 0 {
            return Err(anyhow!("sample_rate is zero"));
        }
        if channels == 0 {
            return Err(anyhow!("channels is zero"));
        }
        let control_rx = subscribe_worker_control(self, &plugin_id)?;
        let (instances, updates) = new_factory_state();
        let factory = DspInstanceFactory {
            runtime: self.clone(),
            plugin_id,
            type_id,
            sample_rate,
            channels,
            instances,
            updates,
        };
        Ok(DspWorkerEndpoint {
            factory,
            control_rx,
        })
    }
}

impl WorkerConfigurableInstance for DspInstance {
    fn apply_config_update_json(
        &mut self,
        new_config_json: &str,
    ) -> Result<crate::runtime::update::InstanceUpdateResult> {
        DspInstance::apply_config_update_json(self, new_config_json)
    }
}

impl WorkerInstanceFactory for DspInstanceFactory {
    type Instance = DspInstance;

    fn create_instance(&self, config_json: &str) -> Result<Self::Instance> {
        DspInstanceFactory::create_instance(self, config_json)
    }
}

fn destroy_raw_dsp_instance(raw: &mut StDspInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe { ((*raw.vtable).destroy)(raw.handle) };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

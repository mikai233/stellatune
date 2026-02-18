use anyhow::{Result, anyhow};
use stellatune_plugin_api::{StAsyncOpState, StCreateDspInstanceOpRef, StDspInstanceRef};

use crate::capabilities::common::{status_to_result, ststr_from_str};
use crate::capabilities::dsp::DspInstance;
use crate::runtime::handle::PluginRuntimeHandle;
use crate::runtime::update::InstanceUpdateResult;
use crate::runtime::worker_controller::{WorkerConfigurableInstance, WorkerInstanceFactory};

use super::common::{
    acquire_active_lease, new_factory_state, new_instance_runtime_ctx, normalize_plugin_type_ids,
    subscribe_worker_control,
};
use super::{DspInstanceFactory, DspWorkerEndpoint};

impl DspInstanceFactory {
    pub fn create_instance(&self, config_json: &str) -> Result<DspInstance> {
        let lease = acquire_active_lease(&self.runtime, &self.plugin_id)?;
        let module = lease.module();
        let Some(create) = module.begin_create_dsp_instance else {
            return Err(anyhow!(
                "plugin `{}` does not provide dsp factory",
                self.plugin_id
            ));
        };

        let plugin_free = module.plugin_free;
        let mut op = StCreateDspInstanceOpRef {
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
            &mut op,
        );
        status_to_result("begin_create_dsp_instance", status, plugin_free)?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "begin_create_dsp_instance returned null op handle/vtable"
            ));
        }

        let mut raw = StDspInstanceRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let create_res = (|| {
            loop {
                let mut state = StAsyncOpState::Pending;
                let status = unsafe { ((*op.vtable).wait)(op.handle, u32::MAX, &mut state) };
                status_to_result("create_dsp_instance op wait", status, plugin_free)?;
                match state {
                    StAsyncOpState::Pending => continue,
                    StAsyncOpState::Ready => {
                        let status = unsafe { ((*op.vtable).take_instance)(op.handle, &mut raw) };
                        status_to_result("create_dsp_instance take_instance", status, plugin_free)?;
                        return Ok(());
                    },
                    StAsyncOpState::Cancelled => {
                        return Err(anyhow!("create_dsp_instance operation cancelled"));
                    },
                    StAsyncOpState::Failed => {
                        let _ = status_to_result(
                            "create_dsp_instance op failed",
                            unsafe { ((*op.vtable).take_instance)(op.handle, &mut raw) },
                            plugin_free,
                        );
                        return Err(anyhow!("create_dsp_instance operation failed"));
                    },
                }
            }
        })();
        unsafe { ((*op.vtable).destroy)(op.handle) };
        create_res?;

        let ctx = new_instance_runtime_ctx(&self.instances, &self.updates, lease, plugin_free);
        match DspInstance::from_ffi(ctx, raw) {
            Ok(instance) => Ok(instance),
            Err(err) => {
                destroy_raw_dsp_instance(&mut raw);
                Err(err)
            },
        }
    }
}

impl PluginRuntimeHandle {
    pub async fn bind_dsp_worker_endpoint(
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
    fn apply_config_update_json(&mut self, new_config_json: &str) -> Result<InstanceUpdateResult> {
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

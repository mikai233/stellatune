use anyhow::{Result, anyhow};
use stellatune_plugin_api::{StAsyncOpState, StCreateDecoderInstanceOpRef, StDecoderInstanceRef};

use crate::capabilities::common::{status_to_result, ststr_from_str};
use crate::capabilities::decoder::DecoderInstance;
use crate::runtime::handle::PluginRuntimeHandle;
use crate::runtime::update::InstanceUpdateResult;
use crate::runtime::worker_controller::{WorkerConfigurableInstance, WorkerInstanceFactory};

use super::common::{
    acquire_active_lease, new_factory_state, new_instance_runtime_ctx, normalize_plugin_type_ids,
    subscribe_worker_control,
};
use super::{DecoderInstanceFactory, DecoderWorkerEndpoint};

impl DecoderInstanceFactory {
    pub fn create_instance(&self, config_json: &str) -> Result<DecoderInstance> {
        let lease = acquire_active_lease(&self.runtime, &self.plugin_id)?;
        let Some(create) = lease.loaded.module.begin_create_decoder_instance else {
            return Err(anyhow!(
                "plugin `{}` does not provide decoder factory",
                self.plugin_id
            ));
        };

        let plugin_free = lease.loaded.module.plugin_free;
        let mut op = StCreateDecoderInstanceOpRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let status = (create)(
            ststr_from_str(&self.type_id),
            ststr_from_str(config_json),
            &mut op,
        );
        status_to_result("begin_create_decoder_instance", status, plugin_free)?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "begin_create_decoder_instance returned null op handle/vtable"
            ));
        }

        let mut raw = StDecoderInstanceRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let create_res = (|| {
            loop {
                let mut state = StAsyncOpState::Pending;
                let status = unsafe { ((*op.vtable).wait)(op.handle, u32::MAX, &mut state) };
                status_to_result("create_decoder_instance op wait", status, plugin_free)?;
                match state {
                    StAsyncOpState::Pending => continue,
                    StAsyncOpState::Ready => {
                        let status = unsafe { ((*op.vtable).take_instance)(op.handle, &mut raw) };
                        status_to_result(
                            "create_decoder_instance take_instance",
                            status,
                            plugin_free,
                        )?;
                        return Ok(());
                    },
                    StAsyncOpState::Cancelled => {
                        return Err(anyhow!("create_decoder_instance operation cancelled"));
                    },
                    StAsyncOpState::Failed => {
                        let _ = status_to_result(
                            "create_decoder_instance op failed",
                            unsafe { ((*op.vtable).take_instance)(op.handle, &mut raw) },
                            plugin_free,
                        );
                        return Err(anyhow!("create_decoder_instance operation failed"));
                    },
                }
            }
        })();
        unsafe { ((*op.vtable).destroy)(op.handle) };
        create_res?;

        let ctx = new_instance_runtime_ctx(&self.instances, &self.updates, lease, plugin_free);
        match DecoderInstance::from_ffi(ctx, raw) {
            Ok(instance) => Ok(instance),
            Err(err) => {
                destroy_raw_decoder_instance(&mut raw);
                Err(err)
            },
        }
    }
}

impl PluginRuntimeHandle {
    pub async fn bind_decoder_worker_endpoint(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<DecoderWorkerEndpoint> {
        let (plugin_id, type_id) = normalize_plugin_type_ids(plugin_id, type_id)?;
        let control_rx = subscribe_worker_control(self, &plugin_id)?;
        let (instances, updates) = new_factory_state();
        let factory = DecoderInstanceFactory {
            runtime: self.clone(),
            plugin_id,
            type_id,
            instances,
            updates,
        };
        Ok(DecoderWorkerEndpoint {
            factory,
            control_rx,
        })
    }
}

impl WorkerConfigurableInstance for DecoderInstance {
    fn apply_config_update_json(&mut self, new_config_json: &str) -> Result<InstanceUpdateResult> {
        DecoderInstance::apply_config_update_json(self, new_config_json)
    }
}

impl WorkerInstanceFactory for DecoderInstanceFactory {
    type Instance = DecoderInstance;

    fn create_instance(&self, config_json: &str) -> Result<Self::Instance> {
        DecoderInstanceFactory::create_instance(self, config_json)
    }
}

fn destroy_raw_decoder_instance(raw: &mut StDecoderInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe { ((*raw.vtable).destroy)(raw.handle) };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

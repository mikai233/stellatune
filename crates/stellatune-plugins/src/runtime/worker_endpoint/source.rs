use anyhow::{Result, anyhow};
use stellatune_plugin_api::{
    StAsyncOpState, StCreateSourceCatalogInstanceOpRef, StSourceCatalogInstanceRef,
};

use crate::capabilities::common::{status_to_result, ststr_from_str};
use crate::capabilities::source::SourceCatalogInstance;
use crate::runtime::handle::PluginRuntimeHandle;
use crate::runtime::worker_controller::{WorkerConfigurableInstance, WorkerInstanceFactory};

use super::common::{
    acquire_active_lease, new_factory_state, new_instance_runtime_ctx, normalize_plugin_type_ids,
    subscribe_worker_control,
};
use super::{SourceCatalogInstanceFactory, SourceCatalogWorkerEndpoint};

impl SourceCatalogInstanceFactory {
    pub fn create_instance(&self, config_json: &str) -> Result<SourceCatalogInstance> {
        let lease = acquire_active_lease(&self.runtime, &self.plugin_id)?;
        let Some(create) = lease.loaded.module.begin_create_source_catalog_instance else {
            return Err(anyhow!(
                "plugin `{}` does not provide source catalog factory",
                self.plugin_id
            ));
        };

        let plugin_free = lease.loaded.module.plugin_free;
        let mut op = StCreateSourceCatalogInstanceOpRef {
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
        status_to_result("begin_create_source_catalog_instance", status, plugin_free)?;
        if op.handle.is_null() || op.vtable.is_null() {
            return Err(anyhow!(
                "begin_create_source_catalog_instance returned null op handle/vtable"
            ));
        }

        let mut raw = StSourceCatalogInstanceRef {
            handle: core::ptr::null_mut(),
            vtable: core::ptr::null(),
            reserved0: 0,
            reserved1: 0,
        };
        let create_res = (|| {
            loop {
                let mut state = StAsyncOpState::Pending;
                let status = unsafe { ((*op.vtable).wait)(op.handle, u32::MAX, &mut state) };
                status_to_result(
                    "create_source_catalog_instance op wait",
                    status,
                    plugin_free,
                )?;
                match state {
                    StAsyncOpState::Pending => continue,
                    StAsyncOpState::Ready => {
                        let status = unsafe { ((*op.vtable).take_instance)(op.handle, &mut raw) };
                        status_to_result(
                            "create_source_catalog_instance take_instance",
                            status,
                            plugin_free,
                        )?;
                        return Ok(());
                    }
                    StAsyncOpState::Cancelled => {
                        return Err(anyhow!(
                            "create_source_catalog_instance operation cancelled"
                        ));
                    }
                    StAsyncOpState::Failed => {
                        let _ = status_to_result(
                            "create_source_catalog_instance op failed",
                            unsafe { ((*op.vtable).take_instance)(op.handle, &mut raw) },
                            plugin_free,
                        );
                        return Err(anyhow!("create_source_catalog_instance operation failed"));
                    }
                }
            }
        })();
        unsafe { ((*op.vtable).destroy)(op.handle) };
        create_res?;

        let ctx = new_instance_runtime_ctx(&self.instances, &self.updates, lease, plugin_free);
        match SourceCatalogInstance::from_ffi(ctx, raw) {
            Ok(instance) => Ok(instance),
            Err(err) => {
                destroy_raw_source_instance(&mut raw);
                Err(err)
            }
        }
    }
}

impl PluginRuntimeHandle {
    pub async fn bind_source_catalog_worker_endpoint(
        &self,
        plugin_id: &str,
        type_id: &str,
    ) -> Result<SourceCatalogWorkerEndpoint> {
        let (plugin_id, type_id) = normalize_plugin_type_ids(plugin_id, type_id)?;
        let control_rx = subscribe_worker_control(self, &plugin_id)?;
        let (instances, updates) = new_factory_state();
        let factory = SourceCatalogInstanceFactory {
            runtime: self.clone(),
            plugin_id,
            type_id,
            instances,
            updates,
        };
        Ok(SourceCatalogWorkerEndpoint {
            factory,
            control_rx,
        })
    }
}

impl WorkerConfigurableInstance for SourceCatalogInstance {
    fn apply_config_update_json(
        &mut self,
        new_config_json: &str,
    ) -> Result<crate::runtime::update::InstanceUpdateResult> {
        SourceCatalogInstance::apply_config_update_json(self, new_config_json)
    }
}

impl WorkerInstanceFactory for SourceCatalogInstanceFactory {
    type Instance = SourceCatalogInstance;

    fn create_instance(&self, config_json: &str) -> Result<Self::Instance> {
        SourceCatalogInstanceFactory::create_instance(self, config_json)
    }
}

fn destroy_raw_source_instance(raw: &mut StSourceCatalogInstanceRef) {
    if raw.handle.is_null() || raw.vtable.is_null() {
        return;
    }
    unsafe { ((*raw.vtable).destroy)(raw.handle) };
    raw.handle = core::ptr::null_mut();
    raw.vtable = core::ptr::null();
}

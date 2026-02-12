use anyhow::Result;

use crate::runtime::actor::WorkerControlMessage;
use crate::runtime::update::InstanceUpdateResult;

pub trait WorkerConfigurableInstance {
    fn apply_config_update_json(&mut self, new_config_json: &str) -> Result<InstanceUpdateResult>;
}

pub trait WorkerInstanceFactory {
    type Instance: WorkerConfigurableInstance;

    fn create_instance(&self, config_json: &str) -> Result<Self::Instance>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerApplyPendingOutcome {
    Idle,
    Created,
    Recreated,
    Destroyed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerConfigUpdateOutcome {
    DeferredNoInstance,
    Applied {
        revision: u64,
    },
    RequiresRecreate {
        revision: u64,
        reason: Option<String>,
    },
    Rejected {
        revision: u64,
        reason: String,
    },
    Failed {
        revision: u64,
        error: String,
    },
}

pub struct WorkerInstanceController<F: WorkerInstanceFactory> {
    factory: F,
    instance: Option<F::Instance>,
    current_config_json: Option<String>,
    desired_config_json: String,
    pending_recreate: bool,
    pending_destroy: bool,
    last_control_seq: u64,
}

impl<F: WorkerInstanceFactory> WorkerInstanceController<F> {
    pub fn new(factory: F, initial_config_json: impl Into<String>) -> Self {
        Self {
            factory,
            instance: None,
            current_config_json: None,
            desired_config_json: initial_config_json.into(),
            pending_recreate: true,
            pending_destroy: false,
            last_control_seq: 0,
        }
    }

    pub fn instance(&self) -> Option<&F::Instance> {
        self.instance.as_ref()
    }

    pub fn instance_mut(&mut self) -> Option<&mut F::Instance> {
        self.instance.as_mut()
    }

    pub fn take_instance(&mut self) -> Option<F::Instance> {
        self.instance.take()
    }

    pub fn desired_config_json(&self) -> &str {
        &self.desired_config_json
    }

    pub fn current_config_json(&self) -> Option<&str> {
        self.current_config_json.as_deref()
    }

    pub fn has_pending_recreate(&self) -> bool {
        self.pending_recreate
    }

    pub fn has_pending_destroy(&self) -> bool {
        self.pending_destroy
    }

    pub fn request_recreate(&mut self) {
        self.pending_recreate = true;
    }

    pub fn request_destroy(&mut self) {
        self.pending_destroy = true;
        self.pending_recreate = false;
    }

    pub fn on_control_message(&mut self, message: WorkerControlMessage) {
        let (seq, is_destroy) = match &message {
            WorkerControlMessage::Recreate { seq, .. } => (*seq, false),
            WorkerControlMessage::Destroy { seq, .. } => (*seq, true),
        };
        if seq <= self.last_control_seq {
            return;
        }
        self.last_control_seq = seq;
        if is_destroy {
            self.request_destroy();
        } else {
            self.request_recreate();
        }
    }

    pub fn apply_config_update(
        &mut self,
        new_config_json: impl Into<String>,
    ) -> Result<WorkerConfigUpdateOutcome> {
        let new_config_json = new_config_json.into();
        self.desired_config_json = new_config_json.clone();

        let Some(instance) = self.instance.as_mut() else {
            self.pending_recreate = true;
            return Ok(WorkerConfigUpdateOutcome::DeferredNoInstance);
        };

        let result = instance.apply_config_update_json(&new_config_json)?;
        let outcome = match result {
            InstanceUpdateResult::Applied { revision, .. } => {
                self.current_config_json = Some(new_config_json);
                self.pending_recreate = false;
                WorkerConfigUpdateOutcome::Applied { revision }
            }
            InstanceUpdateResult::RequiresRecreate {
                revision, reason, ..
            } => {
                self.pending_recreate = true;
                WorkerConfigUpdateOutcome::RequiresRecreate { revision, reason }
            }
            InstanceUpdateResult::Rejected {
                revision, reason, ..
            } => WorkerConfigUpdateOutcome::Rejected { revision, reason },
            InstanceUpdateResult::Failed {
                revision, error, ..
            } => {
                self.pending_recreate = true;
                WorkerConfigUpdateOutcome::Failed { revision, error }
            }
        };
        Ok(outcome)
    }

    pub fn apply_pending(&mut self) -> Result<WorkerApplyPendingOutcome> {
        if self.pending_destroy {
            let had_instance = self.instance.take().is_some();
            self.pending_destroy = false;
            self.pending_recreate = false;
            self.current_config_json = None;
            return Ok(if had_instance {
                WorkerApplyPendingOutcome::Destroyed
            } else {
                WorkerApplyPendingOutcome::Idle
            });
        }

        if self.pending_recreate {
            let had_instance = self.instance.take().is_some();
            let instance = self.factory.create_instance(&self.desired_config_json)?;
            self.instance = Some(instance);
            self.current_config_json = Some(self.desired_config_json.clone());
            self.pending_recreate = false;
            return Ok(if had_instance {
                WorkerApplyPendingOutcome::Recreated
            } else {
                WorkerApplyPendingOutcome::Created
            });
        }

        Ok(WorkerApplyPendingOutcome::Idle)
    }
}

#[cfg(test)]
#[path = "tests/worker_controller_tests.rs"]
mod tests;

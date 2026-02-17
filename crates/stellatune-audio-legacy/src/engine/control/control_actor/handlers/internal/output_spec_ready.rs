use crate::output::OutputSpec;
use stellatune_runtime::thread_actor::{ActorContext, Handler, Message};
use tracing::debug;

use crate::engine::control::control_actor::ControlActor;

pub(crate) struct OutputSpecReadyInternalMessage {
    pub(crate) spec: OutputSpec,
    pub(crate) took_ms: u64,
    pub(crate) token: u64,
}

impl Message for OutputSpecReadyInternalMessage {
    type Response = ();
}

impl Handler<OutputSpecReadyInternalMessage> for ControlActor {
    fn handle(&mut self, message: OutputSpecReadyInternalMessage, _ctx: &mut ActorContext<Self>) {
        let OutputSpecReadyInternalMessage {
            spec,
            took_ms,
            token,
        } = message;
        if token != self.state.output_spec_token {
            return;
        }
        let sample_rate = spec.sample_rate;
        let channels = spec.channels;
        self.state.cached_output_spec = Some(spec);
        self.state.output_spec_prewarm_inflight = false;
        debug!(
            "output_spec prewarm ready in {}ms: {}Hz {}ch",
            took_ms, sample_rate, channels
        );
    }
}

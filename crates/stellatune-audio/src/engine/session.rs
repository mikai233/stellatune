use std::sync::Arc;
use std::time::Duration;
use std::{any::Any, panic::AssertUnwindSafe};

use stellatune_audio_core::pipeline::context::InputRef;
use stellatune_audio_core::pipeline::error::PipelineError;

use crate::config::engine::{EngineConfig, LfeMode, PauseBehavior, ResampleQuality, StopBehavior};
use crate::error::DecodeError;
use crate::pipeline::assembly::PipelineFactory;
use crate::pipeline::plan::PlaybackCheckpoint;
use crate::pipeline::runtime::dsp::control::SharedMasterGainHotControl;
use crate::workers::decode::DecodeWorkerEventCallback;
use crate::workers::decode::handlers;
use crate::workers::decode::state::DecodeWorkerState;
use crate::workers::decode::worker_loop::{compute_loop_timeout, drive_playback_once};

const PUMP_BLOCK_BUDGET: usize = 8;

fn catch_decode_panic<T>(
    operation: &'static str,
    f: impl FnOnce() -> Result<T, DecodeError>,
) -> Result<T, DecodeError> {
    match std::panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(payload) => Err(DecodeError::Pipeline(PipelineError::StageFailure(format!(
            "native playback stage panicked during {operation}: {}",
            panic_payload_message(payload.as_ref())
        )))),
    }
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("non-string panic payload")
}

pub(crate) struct PumpTurn {
    pub(crate) next_delay: Duration,
    pub(crate) recovering: bool,
}

/// Native playback resources owned by the playback actor.
///
/// This replaces the former long-running decode worker. Audio work is advanced
/// in bounded turns while the existing sink worker remains the only dedicated
/// data-plane thread.
pub(crate) struct PlaybackSession {
    factory: Arc<dyn PipelineFactory>,
    state: DecodeWorkerState,
    callback: DecodeWorkerEventCallback,
    config: EngineConfig,
    closed: bool,
}

impl PlaybackSession {
    pub(crate) fn new(
        factory: Arc<dyn PipelineFactory>,
        config: EngineConfig,
        callback: DecodeWorkerEventCallback,
        master_gain_hot_control: SharedMasterGainHotControl,
    ) -> Self {
        let state = DecodeWorkerState::new(
            config.sink_latency,
            config.sink_recovery,
            config.gain_transition,
            config.sink_control_timeout,
            master_gain_hot_control,
        );
        Self {
            factory,
            state,
            callback,
            config,
            closed: false,
        }
    }

    pub(crate) fn open(
        &mut self,
        input: String,
        start_playing: bool,
        _timeout: Duration,
    ) -> Result<(), DecodeError> {
        let result = catch_decode_panic("open", || {
            handlers::open::open_input(
                InputRef::TrackToken(input),
                start_playing,
                &self.factory,
                &self.callback,
                &mut self.state,
            )
        });
        if result.is_err() {
            // `open_input` assembles several independently fallible stages. A
            // failed replacement must not leave the old sink or pumping flag
            // attached to a session that no longer has an active runner.
            let _ = catch_decode_panic("failed-open cleanup", || {
                handlers::stop::handle(StopBehavior::Immediate, &self.callback, &mut self.state)
            });
        }
        result
    }

    pub(crate) fn queue_next(
        &mut self,
        input: String,
        _timeout: Duration,
    ) -> Result<(), DecodeError> {
        catch_decode_panic("queue-next prewarm", || {
            handlers::queue_next::handle(
                InputRef::TrackToken(input),
                &self.factory,
                &mut self.state,
            )
        })
    }

    pub(crate) fn play(&mut self, _timeout: Duration) -> Result<(), DecodeError> {
        catch_decode_panic("play", || {
            handlers::play::handle(&self.callback, &mut self.state)
        })
    }

    pub(crate) fn pause(
        &mut self,
        behavior: PauseBehavior,
        _timeout: Duration,
    ) -> Result<(), DecodeError> {
        catch_decode_panic("pause", || {
            handlers::pause::handle(behavior, &self.callback, &mut self.state)
        })
    }

    pub(crate) fn seek(&mut self, position_ms: i64, _timeout: Duration) -> Result<(), DecodeError> {
        catch_decode_panic("seek", || {
            handlers::seek::handle(position_ms, &self.callback, &mut self.state)
        })
    }

    pub(crate) fn stop(
        &mut self,
        behavior: StopBehavior,
        _timeout: Duration,
    ) -> Result<(), DecodeError> {
        catch_decode_panic("stop", || {
            handlers::stop::handle(behavior, &self.callback, &mut self.state)
        })
    }

    pub(crate) fn rebuild_pipeline(&mut self, _timeout: Duration) -> Result<(), DecodeError> {
        catch_decode_panic("pipeline rebuild", || {
            handlers::reconfigure_active::handle(&self.factory, &self.callback, &mut self.state)
        })
    }

    pub(crate) fn set_lfe_mode(
        &mut self,
        mode: LfeMode,
        _timeout: Duration,
    ) -> Result<(), DecodeError> {
        catch_decode_panic("LFE reconfiguration", || {
            handlers::set_lfe_mode::handle(mode, &self.factory, &self.callback, &mut self.state)
        })
    }

    pub(crate) fn set_resample_quality(
        &mut self,
        quality: ResampleQuality,
        _timeout: Duration,
    ) -> Result<(), DecodeError> {
        catch_decode_panic("resampler reconfiguration", || {
            handlers::set_resample_quality::handle(
                quality,
                &self.factory,
                &self.callback,
                &mut self.state,
            )
        })
    }

    /// Processes a bounded amount of audio and returns the delay before the
    /// next turn, or `None` when pumping should stop.
    pub(crate) fn pump_turn(&mut self) -> Option<PumpTurn> {
        for _ in 0..PUMP_BLOCK_BUDGET {
            let advanced = catch_decode_panic("audio pump", || {
                Ok(drive_playback_once(
                    &self.factory,
                    &self.config,
                    &self.callback,
                    &mut self.state,
                ))
            });
            match advanced {
                Ok(true) => {},
                Ok(false) => return None,
                Err(error) => {
                    let _ = catch_decode_panic("panic cleanup", || {
                        handlers::stop::handle(
                            StopBehavior::Immediate,
                            &self.callback,
                            &mut self.state,
                        )
                    });
                    (self.callback)(crate::workers::decode::DecodeWorkerEvent::Error(error));
                    return None;
                },
            }
        }
        Some(PumpTurn {
            next_delay: compute_loop_timeout(&self.state, &self.config),
            recovering: self.state.runner.is_none() || self.state.recovery_retry_at.is_some(),
        })
    }

    pub(crate) fn suspend_for_plugin_change(
        &mut self,
        resume_playing: bool,
    ) -> Result<Option<PlaybackCheckpoint>, DecodeError> {
        let checkpoint = self
            .state
            .active_input
            .clone()
            .map(|input| PlaybackCheckpoint {
                input,
                consumed_position_ms: self
                    .state
                    .sink_session
                    .consumed_position_ms()
                    .unwrap_or(self.state.ctx.position_ms)
                    .max(0),
                resume_playing,
            });
        self.stop(StopBehavior::Immediate, self.config.decode_command_timeout)?;
        Ok(checkpoint)
    }

    pub(crate) fn restore_after_plugin_change(
        &mut self,
        checkpoint: &PlaybackCheckpoint,
    ) -> Result<(), DecodeError> {
        let InputRef::TrackToken(track_token) = &checkpoint.input;
        self.open(
            track_token.clone(),
            false,
            self.config.decode_command_timeout,
        )?;
        if checkpoint.consumed_position_ms > 0 {
            self.seek(
                checkpoint.consumed_position_ms,
                self.config.decode_command_timeout,
            )?;
        }
        if checkpoint.resume_playing {
            self.play(self.config.decode_command_timeout)?;
        }
        Ok(())
    }

    pub(crate) fn shutdown(mut self, _timeout: Duration) -> Result<(), DecodeError> {
        handlers::shutdown::handle(&self.callback, &mut self.state);
        self.closed = true;
        Ok(())
    }
}

impl Drop for PlaybackSession {
    fn drop(&mut self) {
        if !self.closed {
            crate::workers::decode::worker_loop::shutdown_playback_state(&mut self.state);
            self.closed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use stellatune_audio_core::pipeline::context::{
        AudioBlock, InputRef, PipelineContext, SourceHandle, StreamSpec,
    };
    use stellatune_audio_core::pipeline::error::PipelineError;
    use stellatune_audio_core::pipeline::stages::decoder::DecoderStage;
    use stellatune_audio_core::pipeline::stages::sink::SinkStage;
    use stellatune_audio_core::pipeline::stages::source::SourceStage;
    use stellatune_audio_core::pipeline::stages::{Stage, StageFlow};

    use crate::config::engine::EngineConfig;
    use crate::pipeline::assembly::{AssembledPipeline, PipelineFactory};
    use crate::pipeline::runtime::dsp::control::MasterGainHotControl;

    use super::PlaybackSession;

    struct ConditionalFactory;

    impl PipelineFactory for ConditionalFactory {
        fn build_pipeline(&self, input: &InputRef) -> Result<AssembledPipeline, PipelineError> {
            let InputRef::TrackToken(token) = input;
            assert_ne!(token, "panic", "simulated decoder panic");
            if token == "unsupported" {
                return Err(PipelineError::StageFailure(
                    "unsupported test input".to_string(),
                ));
            }
            Ok(AssembledPipeline::from_static(
                Box::new(TestSource),
                Box::new(TestDecoder),
                Vec::new(),
                vec![Box::new(TestSink)],
            ))
        }
    }

    struct TestSource;

    impl Stage for TestSource {}

    impl SourceStage for TestSource {
        fn prepare(
            &mut self,
            _input: &InputRef,
            _ctx: &mut PipelineContext,
        ) -> Result<SourceHandle, PipelineError> {
            Ok(SourceHandle::Empty)
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    struct TestDecoder;

    impl Stage for TestDecoder {}

    impl DecoderStage for TestDecoder {
        fn prepare(
            &mut self,
            _source: &SourceHandle,
            _ctx: &mut PipelineContext,
        ) -> Result<StreamSpec, PipelineError> {
            Ok(StreamSpec {
                sample_rate: 48_000,
                channels: 2,
            })
        }

        fn next_block(
            &mut self,
            _out: &mut AudioBlock,
            _ctx: &mut PipelineContext,
        ) -> Result<StageFlow, PipelineError> {
            Ok(StageFlow::Eof)
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    struct TestSink;

    impl Stage for TestSink {}

    impl SinkStage for TestSink {
        fn prepare(
            &mut self,
            _spec: StreamSpec,
            _ctx: &mut PipelineContext,
        ) -> Result<(), PipelineError> {
            Ok(())
        }

        fn write(
            &mut self,
            _block: &AudioBlock,
            _ctx: &mut PipelineContext,
        ) -> Result<StageFlow, PipelineError> {
            Ok(StageFlow::Continue)
        }

        fn flush(&mut self, _ctx: &mut PipelineContext) -> Result<(), PipelineError> {
            Ok(())
        }

        fn stop(&mut self, _ctx: &mut PipelineContext) {}
    }

    #[test]
    fn failed_open_resets_session_before_the_next_track() {
        let callback = Arc::new(|_| {});
        let config = EngineConfig::default();
        let mut session = PlaybackSession::new(
            Arc::new(ConditionalFactory),
            config.clone(),
            callback,
            Arc::new(MasterGainHotControl::default()),
        );

        session
            .open("valid-before".to_string(), true, Duration::ZERO)
            .expect("first valid input should open");
        assert!(session.state.pumping);
        assert!(session.state.sink_session.consumed_position_ms().is_some());

        session
            .open("unsupported".to_string(), true, Duration::ZERO)
            .expect_err("unsupported input should fail");
        assert!(!session.state.pumping);
        assert!(session.state.runner.is_none());
        assert!(session.state.active_input.is_none());
        assert!(session.state.sink_session.consumed_position_ms().is_none());

        session
            .open("valid-after".to_string(), true, Duration::ZERO)
            .expect("a valid input must still open after the failure");

        session
            .queue_next("unsupported".to_string(), Duration::ZERO)
            .expect_err("failed prewarm should be reported");
        assert!(session.state.queued_next_input.is_none());
        assert!(session.state.prewarmed_next.is_none());
        assert!(session.state.runner.is_some());

        let panic_error = session
            .queue_next("panic".to_string(), Duration::ZERO)
            .expect_err("decoder panic should be isolated at the session boundary");
        assert!(panic_error.to_string().contains("panicked"));
        assert!(session.state.runner.is_some());

        session
            .shutdown(config.decode_command_timeout)
            .expect("session shutdown should succeed");
    }
}

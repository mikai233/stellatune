use std::{
    hint::black_box,
    sync::Arc,
    time::{Duration, Instant},
};

use lattice_actor::{
    context::HandlerContext,
    error::ActorError,
    mailbox::MailboxConfig,
    reply::ReplyTo,
    runtime::{ActorExecutionPolicy, ActorRuntime, ActorSpawnOptions},
    state_machine::Stateless,
    traits::{Actor, Handler, Responder, StopReason},
};
use tokio::sync::Semaphore;

const ASK_TIMEOUT: Duration = Duration::from_secs(5);
const SAMPLE_COUNT: usize = 2_048;
const BLOCKS_PER_TURN: usize = 8;
const BENCHMARK_TURNS: usize = 2_000;
const CONTROL_SAMPLES: usize = 200;

#[derive(Debug, lattice_actor::Message)]
struct StartPump {
    turns: usize,
    started: Arc<Semaphore>,
    completed: Arc<Semaphore>,
}

#[derive(Debug, lattice_actor::Message)]
struct PumpAudio;

#[derive(Debug, lattice_actor::Request)]
#[request(response = f32)]
struct ProbeControl;

struct AudioWorkActor {
    samples: Vec<f32>,
    remaining_turns: usize,
    checksum: f32,
    completed: Option<Arc<Semaphore>>,
}

impl AudioWorkActor {
    fn new() -> Self {
        let samples = (0..SAMPLE_COUNT)
            .map(|index| (index as f32 * 0.001).sin())
            .collect();
        Self {
            samples,
            remaining_turns: 0,
            checksum: 0.0,
            completed: None,
        }
    }

    fn process_audio_turn(&mut self) {
        for block_index in 0..BLOCKS_PER_TURN {
            let gain = 0.75 + block_index as f32 * 0.01;
            let mut left = 0.0_f32;
            let mut right = 0.0_f32;
            let (frames, remainder) = self.samples.as_chunks_mut::<2>();
            debug_assert!(remainder.is_empty());
            for frame in frames {
                let mixed = (frame[0] + frame[1]) * 0.5 * gain;
                frame[0] = mixed + frame[0] * 0.125;
                frame[1] = mixed + frame[1] * 0.125;
                left += frame[0];
                right += frame[1];
            }
            self.checksum = black_box(self.checksum + left * 0.000_001 + right * 0.000_002);
        }
    }
}

impl Actor for AudioWorkActor {
    type Error = ActorError;
    type Behavior = Stateless;
}

impl Handler<StartPump> for AudioWorkActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        message: StartPump,
    ) -> Result<(), ActorError> {
        self.remaining_turns = message.turns;
        self.completed = Some(message.completed);
        message.started.add_permits(1);
        ctx.self_handle()
            .try_tell(PumpAudio)
            .map_err(|error| ActorError::new(error.to_string()))?;
        Ok(())
    }
}

impl Handler<PumpAudio> for AudioWorkActor {
    async fn handle(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        _message: PumpAudio,
    ) -> Result<(), ActorError> {
        self.process_audio_turn();
        self.remaining_turns = self.remaining_turns.saturating_sub(1);
        if self.remaining_turns == 0 {
            if let Some(completed) = self.completed.take() {
                completed.add_permits(1);
            }
        } else {
            ctx.self_handle()
                .try_tell(PumpAudio)
                .map_err(|error| ActorError::new(error.to_string()))?;
        }
        Ok(())
    }
}

impl Responder<ProbeControl> for AudioWorkActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _request: ProbeControl,
        reply_to: ReplyTo<f32>,
    ) -> Result<(), ActorError> {
        let _ = reply_to.send(black_box(self.checksum));
        Ok(())
    }
}

#[derive(Debug)]
struct BenchmarkResult {
    policy: ActorExecutionPolicy,
    elapsed: Duration,
    control_p50: Duration,
    control_p99: Duration,
}

async fn run_policy(policy: ActorExecutionPolicy) -> BenchmarkResult {
    let handle = ActorRuntime::default()
        .spawn_actor(
            AudioWorkActor::new(),
            ActorSpawnOptions {
                mailbox: MailboxConfig::bounded(64).with_turn_budget(16),
                execution: Some(policy),
                ..ActorSpawnOptions::default()
            },
        )
        .expect("benchmark actor should spawn");
    let started = Arc::new(Semaphore::new(0));
    let completed = Arc::new(Semaphore::new(0));
    let start = Instant::now();
    handle
        .tell(StartPump {
            turns: BENCHMARK_TURNS,
            started: Arc::clone(&started),
            completed: Arc::clone(&completed),
        })
        .await
        .unwrap();
    started.acquire().await.expect("pump should start").forget();

    let mut control_latencies = Vec::with_capacity(CONTROL_SAMPLES);
    for _ in 0..CONTROL_SAMPLES {
        let control_start = Instant::now();
        let _ = handle.ask(ProbeControl, ASK_TIMEOUT).await.unwrap();
        control_latencies.push(control_start.elapsed());
    }

    tokio::time::timeout(ASK_TIMEOUT, completed.acquire())
        .await
        .expect("audio pump should complete")
        .expect("completion signal should remain open")
        .forget();
    let elapsed = start.elapsed();
    handle.stop(StopReason::Requested).unwrap();

    control_latencies.sort_unstable();
    let p50 = control_latencies[control_latencies.len() / 2];
    let p99_index = (control_latencies.len() * 99 / 100).min(control_latencies.len() - 1);
    let p99 = control_latencies[p99_index];
    BenchmarkResult {
        policy,
        elapsed,
        control_p50: p50,
        control_p99: p99,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "manual Phase 0 execution-policy benchmark"]
async fn compare_playback_actor_execution_policies() {
    let task_per_actor = run_policy(ActorExecutionPolicy::TaskPerActor).await;
    let dedicated = run_policy(ActorExecutionPolicy::DedicatedThreadPool { worker_count: 1 }).await;

    println!("{task_per_actor:#?}");
    println!("{dedicated:#?}");

    for result in [&task_per_actor, &dedicated] {
        println!(
            "policy={:?} elapsed={:?} control_p50={:?} control_p99={:?}",
            result.policy, result.elapsed, result.control_p50, result.control_p99
        );
        assert!(result.elapsed > Duration::ZERO);
        assert!(result.control_p50 <= result.control_p99);
        assert!(result.control_p99 < Duration::from_millis(100));
    }
}

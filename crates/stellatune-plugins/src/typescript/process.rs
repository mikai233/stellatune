use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use lattice_actor::context::{ActorContext, HandlerContext};
use lattice_actor::error::{ActorError, ActorStopError};
use lattice_actor::handle::ActorHandle;
use lattice_actor::mailbox::MailboxConfig;
use lattice_actor::reply::ReplyTo;
use lattice_actor::runtime::spawn_actor;
use lattice_actor::state_machine::Stateless;
use lattice_actor::traits::{Actor, Handler, Responder, StopReason};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use super::protocol::{
    CAPABILITY_RPC_PROTOCOL, DEFAULT_MAX_FRAME_BYTES, PluginError, RpcRequest, RpcResponse,
};

#[derive(Debug, Clone)]
pub struct PluginProcessConfig {
    pub plugin_id: String,
    pub entry_path: PathBuf,
    pub node_binary: PathBuf,
    pub runner_script: PathBuf,
    pub protocol: String,
    pub request_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_frame_bytes: usize,
}

impl PluginProcessConfig {
    pub fn new(
        plugin_id: impl Into<String>,
        entry_path: impl Into<PathBuf>,
        runner_script: impl Into<PathBuf>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            entry_path: entry_path.into(),
            node_binary: PathBuf::from("node"),
            runner_script: runner_script.into(),
            protocol: CAPABILITY_RPC_PROTOCOL.to_string(),
            request_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(60),
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InvocationResult {
    pub generation: u64,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginProcessSnapshot {
    pub running: bool,
    pub generation: u64,
    pub process_id: Option<u32>,
}

#[derive(Debug, Error)]
pub enum PluginRuntimeError {
    #[error("failed to spawn TypeScript plugin '{plugin_id}': {message}")]
    Spawn { plugin_id: String, message: String },
    #[error("plugin '{plugin_id}' process exited during {operation}")]
    ProcessExited {
        plugin_id: String,
        operation: String,
    },
    #[error("plugin '{plugin_id}' RPC {operation} timed out after {timeout_ms}ms")]
    Timeout {
        plugin_id: String,
        operation: String,
        timeout_ms: u128,
    },
    #[error("plugin '{plugin_id}' RPC protocol error during {operation}: {message}")]
    Protocol {
        plugin_id: String,
        operation: String,
        message: String,
    },
    #[error("plugin '{plugin_id}' capability '{capability_id}' {operation} failed: {error:?}")]
    Remote {
        plugin_id: String,
        capability_id: String,
        operation: String,
        generation: u64,
        error: Box<PluginError>,
    },
    #[error("plugin '{plugin_id}' generation mismatch: expected {expected}, current {current:?}")]
    GenerationMismatch {
        plugin_id: String,
        expected: u64,
        current: Option<u64>,
    },
    #[error("plugin actor call failed during {operation}: {message}")]
    ActorCall {
        operation: &'static str,
        message: String,
    },
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<InvocationResult, PluginRuntimeError>)]
struct InvokeCapability {
    capability_id: String,
    instance_id: Option<String>,
    operation: String,
    input: Value,
    expected_generation: Option<u64>,
}

#[derive(lattice_actor::Message)]
struct InvokeCompleted {
    result: Result<InvocationResult, PluginRuntimeError>,
    reply_to: ReplyTo<Result<InvocationResult, PluginRuntimeError>>,
}

#[derive(Debug, lattice_actor::Request)]
#[request(response = Result<(), PluginRuntimeError>)]
struct StopPluginProcess;

#[derive(Debug, lattice_actor::Request)]
#[request(response = PluginProcessSnapshot)]
struct GetPluginProcessSnapshot;

#[derive(Debug, lattice_actor::Message)]
struct ReapIdleProcess;

struct PluginProcessActor {
    cell: Arc<Mutex<ProcessCell>>,
}

impl Actor for PluginProcessActor {
    type Error = ActorError;
    type Behavior = Stateless;

    async fn stopping(
        &mut self,
        _ctx: &mut ActorContext<Self>,
        _reason: StopReason,
    ) -> Result<(), ActorStopError> {
        self.cell.lock().await.shutdown().await;
        Ok(())
    }
}

impl Responder<InvokeCapability> for PluginProcessActor {
    async fn respond(
        &mut self,
        ctx: &mut HandlerContext<'_, Self>,
        request: InvokeCapability,
        reply_to: ReplyTo<Result<InvocationResult, PluginRuntimeError>>,
    ) -> Result<(), ActorError> {
        let cell = Arc::clone(&self.cell);
        ctx.defer_reply(
            reply_to,
            async move { cell.lock().await.invoke(request).await },
            |result, reply_to| InvokeCompleted { result, reply_to },
        )?;
        Ok(())
    }
}

impl Handler<InvokeCompleted> for PluginProcessActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        message: InvokeCompleted,
    ) -> Result<(), ActorError> {
        let _ = message.reply_to.send(message.result);
        Ok(())
    }
}

impl Responder<StopPluginProcess> for PluginProcessActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: StopPluginProcess,
        reply_to: ReplyTo<Result<(), PluginRuntimeError>>,
    ) -> Result<(), ActorError> {
        self.cell.lock().await.shutdown().await;
        let _ = reply_to.send(Ok(()));
        Ok(())
    }
}

impl Responder<GetPluginProcessSnapshot> for PluginProcessActor {
    async fn respond(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: GetPluginProcessSnapshot,
        reply_to: ReplyTo<PluginProcessSnapshot>,
    ) -> Result<(), ActorError> {
        let snapshot = self.cell.lock().await.snapshot();
        let _ = reply_to.send(snapshot);
        Ok(())
    }
}

impl Handler<ReapIdleProcess> for PluginProcessActor {
    async fn handle(
        &mut self,
        _ctx: &mut HandlerContext<'_, Self>,
        _message: ReapIdleProcess,
    ) -> Result<(), ActorError> {
        self.cell.lock().await.reap_idle().await;
        Ok(())
    }
}

#[derive(Clone)]
pub struct PluginProcessHandle {
    actor: ActorHandle<PluginProcessActor>,
    timeout: Duration,
}

impl PluginProcessHandle {
    pub fn spawn(config: PluginProcessConfig) -> Self {
        let timeout = config.request_timeout;
        let idle_timeout = config.idle_timeout;
        let actor = spawn_actor(
            PluginProcessActor {
                cell: Arc::new(Mutex::new(ProcessCell::new(config))),
            },
            MailboxConfig::bounded(32).with_deferred_capacity(8),
        );
        spawn_idle_reaper(actor.clone(), idle_timeout);
        Self { actor, timeout }
    }

    pub async fn invoke(
        &self,
        capability_id: impl Into<String>,
        instance_id: Option<String>,
        operation: impl Into<String>,
        input: Value,
        expected_generation: Option<u64>,
    ) -> Result<InvocationResult, PluginRuntimeError> {
        self.actor
            .ask(
                InvokeCapability {
                    capability_id: capability_id.into(),
                    instance_id,
                    operation: operation.into(),
                    input,
                    expected_generation,
                },
                self.timeout + Duration::from_secs(1),
            )
            .await
            .map_err(|error| PluginRuntimeError::ActorCall {
                operation: "capability.invoke",
                message: error.to_string(),
            })?
    }

    pub async fn snapshot(&self) -> Result<PluginProcessSnapshot, PluginRuntimeError> {
        self.actor
            .ask(GetPluginProcessSnapshot, self.timeout)
            .await
            .map_err(|error| PluginRuntimeError::ActorCall {
                operation: "process.snapshot",
                message: error.to_string(),
            })
    }

    pub async fn shutdown(&self) -> Result<(), PluginRuntimeError> {
        self.actor
            .ask(StopPluginProcess, self.timeout)
            .await
            .map_err(|error| PluginRuntimeError::ActorCall {
                operation: "process.shutdown",
                message: error.to_string(),
            })??;
        self.actor
            .stop(StopReason::Requested)
            .map_err(|error| PluginRuntimeError::ActorCall {
                operation: "actor.stop",
                message: error.to_string(),
            })
    }
}

fn spawn_idle_reaper(actor: ActorHandle<PluginProcessActor>, idle_timeout: Duration) {
    tokio::spawn(async move {
        let mut terminated = actor.subscribe_terminated();
        let interval = idle_timeout
            .checked_div(2)
            .unwrap_or(Duration::from_millis(100))
            .max(Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    if actor.tell(ReapIdleProcess).await.is_err() {
                        break;
                    }
                }
                _ = terminated.recv() => break,
            }
        }
    });
}

struct ProcessCell {
    config: PluginProcessConfig,
    session: Option<NodeSession>,
    next_generation: u64,
    last_activity: Instant,
}

impl ProcessCell {
    fn new(config: PluginProcessConfig) -> Self {
        Self {
            config,
            session: None,
            next_generation: 1,
            last_activity: Instant::now(),
        }
    }

    async fn invoke(
        &mut self,
        request: InvokeCapability,
    ) -> Result<InvocationResult, PluginRuntimeError> {
        self.remove_exited_session();
        if let Some(expected) = request.expected_generation {
            let current = self.session.as_ref().map(|session| session.generation);
            if current != Some(expected) {
                return Err(PluginRuntimeError::GenerationMismatch {
                    plugin_id: self.config.plugin_id.clone(),
                    expected,
                    current,
                });
            }
        }
        self.ensure_started().await?;
        let session = self.session.as_mut().expect("session started");
        let generation = session.generation;
        let params = json!({
            "capabilityId": request.capability_id,
            "instanceId": request.instance_id,
            "operation": request.operation,
            "input": request.input,
        });
        let capability_id = params["capabilityId"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let operation = params["operation"].as_str().unwrap_or_default().to_string();
        let result = session
            .call("capability.invoke", params, self.config.request_timeout)
            .await;
        self.last_activity = Instant::now();
        match result {
            Ok(value) => Ok(InvocationResult { generation, value }),
            Err(SessionCallError::Remote(error)) => Err(PluginRuntimeError::Remote {
                plugin_id: self.config.plugin_id.clone(),
                capability_id,
                operation,
                generation,
                error: Box::new(error),
            }),
            Err(error) => {
                self.shutdown().await;
                Err(map_session_error(&self.config, "capability.invoke", error))
            },
        }
    }

    async fn ensure_started(&mut self) -> Result<(), PluginRuntimeError> {
        if self.session.is_some() {
            return Ok(());
        }
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        let mut session = NodeSession::spawn(&self.config, generation).await?;
        if let Err(error) = session
            .call(
                "plugin.handshake",
                json!({ "pluginId": self.config.plugin_id }),
                self.config.request_timeout,
            )
            .await
        {
            session.shutdown(Duration::from_millis(100)).await;
            return Err(map_session_error(&self.config, "plugin.handshake", error));
        }
        if let Err(error) = session
            .call(
                "plugin.initialize",
                json!({ "pluginId": self.config.plugin_id, "generation": generation }),
                self.config.request_timeout,
            )
            .await
        {
            session.shutdown(Duration::from_millis(100)).await;
            return Err(map_session_error(&self.config, "plugin.initialize", error));
        }
        debug!(plugin_id = %self.config.plugin_id, generation, "TypeScript plugin process started");
        self.session = Some(session);
        self.last_activity = Instant::now();
        Ok(())
    }

    fn remove_exited_session(&mut self) {
        let exited = self
            .session
            .as_mut()
            .and_then(|session| session.child.try_wait().ok().flatten())
            .is_some();
        if exited {
            self.session = None;
        }
    }

    async fn reap_idle(&mut self) {
        if self.session.is_some() && self.last_activity.elapsed() >= self.config.idle_timeout {
            debug!(plugin_id = %self.config.plugin_id, "stopping idle TypeScript plugin process");
            self.shutdown().await;
        }
    }

    async fn shutdown(&mut self) {
        if let Some(mut session) = self.session.take() {
            session.shutdown(Duration::from_millis(500)).await;
        }
    }

    fn snapshot(&self) -> PluginProcessSnapshot {
        PluginProcessSnapshot {
            running: self.session.is_some(),
            generation: self
                .session
                .as_ref()
                .map_or(self.next_generation.saturating_sub(1), |session| {
                    session.generation
                }),
            process_id: self.session.as_ref().and_then(|session| session.child.id()),
        }
    }
}

struct NodeSession {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr_task: tokio::task::JoinHandle<()>,
    protocol: String,
    generation: u64,
    next_request_id: u64,
    max_frame_bytes: usize,
}

impl NodeSession {
    async fn spawn(
        config: &PluginProcessConfig,
        generation: u64,
    ) -> Result<Self, PluginRuntimeError> {
        let mut child = Command::new(&config.node_binary)
            .arg(&config.runner_script)
            .arg(&config.entry_path)
            .arg(&config.plugin_id)
            .arg(&config.protocol)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| PluginRuntimeError::Spawn {
                plugin_id: config.plugin_id.clone(),
                message: error.to_string(),
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PluginRuntimeError::Spawn {
                plugin_id: config.plugin_id.clone(),
                message: "child stdin is unavailable".to_string(),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PluginRuntimeError::Spawn {
                plugin_id: config.plugin_id.clone(),
                message: "child stdout is unavailable".to_string(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| PluginRuntimeError::Spawn {
                plugin_id: config.plugin_id.clone(),
                message: "child stderr is unavailable".to_string(),
            })?;
        let plugin_id = config.plugin_id.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                warn!(plugin_id, line, "TypeScript plugin stderr");
            }
        });
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr_task,
            protocol: config.protocol.clone(),
            generation,
            next_request_id: 1,
            max_frame_bytes: config.max_frame_bytes,
        })
    }

    async fn call(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, SessionCallError> {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        let request = RpcRequest {
            jsonrpc: "2.0",
            id: request_id,
            protocol: &self.protocol,
            generation: self.generation,
            deadline_ms: unix_ms().saturating_add(timeout.as_millis() as u64),
            method,
            params,
        };
        let payload = serde_json::to_vec(&request)
            .map_err(|error| SessionCallError::Protocol(error.to_string()))?;
        let max_frame_bytes = self.max_frame_bytes;
        let result = tokio::time::timeout(timeout, async {
            write_frame(&mut self.stdin, &payload, max_frame_bytes).await?;
            let payload = read_frame(&mut self.stdout, max_frame_bytes).await?;
            let response: RpcResponse = serde_json::from_slice(&payload)
                .map_err(|error| SessionCallError::Protocol(error.to_string()))?;
            if response.jsonrpc != "2.0" {
                return Err(SessionCallError::Protocol(
                    "response jsonrpc must be 2.0".to_string(),
                ));
            }
            if response.id != request_id {
                return Err(SessionCallError::Protocol(format!(
                    "response id {} does not match {request_id}",
                    response.id
                )));
            }
            if response.generation != self.generation {
                return Err(SessionCallError::Protocol(format!(
                    "response generation {} does not match {}",
                    response.generation, self.generation
                )));
            }
            match (response.result, response.error) {
                (Some(value), None) => Ok(value),
                (None, None) => Ok(Value::Null),
                (_, Some(error)) => Err(SessionCallError::Remote(error)),
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(_) => Err(SessionCallError::Timeout),
        }
    }

    async fn shutdown(&mut self, timeout: Duration) {
        let _ = self.call("plugin.shutdown", Value::Null, timeout).await;
        if tokio::time::timeout(timeout, self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
        }
        self.stderr_task.abort();
    }
}

impl Drop for NodeSession {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        self.stderr_task.abort();
    }
}

#[derive(Debug)]
enum SessionCallError {
    Io(String),
    Timeout,
    Protocol(String),
    Remote(PluginError),
}

fn map_session_error(
    config: &PluginProcessConfig,
    operation: &str,
    error: SessionCallError,
) -> PluginRuntimeError {
    match error {
        SessionCallError::Timeout => PluginRuntimeError::Timeout {
            plugin_id: config.plugin_id.clone(),
            operation: operation.to_string(),
            timeout_ms: config.request_timeout.as_millis(),
        },
        SessionCallError::Io(message) if message.contains("early eof") => {
            PluginRuntimeError::ProcessExited {
                plugin_id: config.plugin_id.clone(),
                operation: operation.to_string(),
            }
        },
        SessionCallError::Io(message) | SessionCallError::Protocol(message) => {
            PluginRuntimeError::Protocol {
                plugin_id: config.plugin_id.clone(),
                operation: operation.to_string(),
                message,
            }
        },
        SessionCallError::Remote(error) => PluginRuntimeError::Remote {
            plugin_id: config.plugin_id.clone(),
            capability_id: "plugin".to_string(),
            operation: operation.to_string(),
            generation: 0,
            error: Box::new(error),
        },
    }
}

async fn write_frame(
    writer: &mut ChildStdin,
    payload: &[u8],
    max_frame_bytes: usize,
) -> Result<(), SessionCallError> {
    if payload.is_empty() || payload.len() > max_frame_bytes || payload.len() > u32::MAX as usize {
        return Err(SessionCallError::Protocol(format!(
            "outbound frame length {} is invalid",
            payload.len()
        )));
    }
    writer
        .write_u32(payload.len() as u32)
        .await
        .map_err(|error| SessionCallError::Io(error.to_string()))?;
    writer
        .write_all(payload)
        .await
        .map_err(|error| SessionCallError::Io(error.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|error| SessionCallError::Io(error.to_string()))
}

async fn read_frame(
    reader: &mut ChildStdout,
    max_frame_bytes: usize,
) -> Result<Vec<u8>, SessionCallError> {
    let length = reader
        .read_u32()
        .await
        .map_err(|error| SessionCallError::Io(error.to_string()))? as usize;
    if length == 0 || length > max_frame_bytes {
        return Err(SessionCallError::Protocol(format!(
            "inbound frame length {length} is invalid"
        )));
    }
    let mut payload = vec![0; length];
    reader
        .read_exact(&mut payload)
        .await
        .map_err(|error| SessionCallError::Io(error.to_string()))?;
    Ok(payload)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin},
    sync::{mpsc, oneshot, watch, Mutex},
    time::{interval, timeout, MissedTickBehavior},
};
use uuid::Uuid;

use super::{
    launcher::{ClaudeLauncher, LaunchError, LaunchOptions},
    protocol::{
        encode_interrupt, encode_permission_response, encode_user_input, parse_protocol_line,
        PermissionDecision, PermissionRequest, ProtocolEventKind, ProtocolMessage, UserInput,
        MAX_STDIN_MESSAGE_BYTES,
    },
};

const MAX_OUTPUT_LINE_BYTES: usize = 8 * 1024 * 1024;
const MAX_STDOUT_TOTAL_BYTES: usize = 10 * 1024 * 1024;
const MAX_STDERR_EVENT_BYTES: usize = 64 * 1024;
const MAX_STDERR_TOTAL_BYTES: usize = 2 * 1024 * 1024;
const READ_CHUNK_BYTES: usize = 16 * 1024;
const CRITICAL_EVENT_CHANNEL_CAPACITY: usize = 1024;
const TELEMETRY_EVENT_CHANNEL_CAPACITY: usize = 256;
const COMMAND_CHANNEL_CAPACITY: usize = 64;
const STDIN_WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const COMMAND_ACK_TIMEOUT: Duration = Duration::from_secs(15);
pub const PERMISSION_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);
const READER_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    pub active_turn: Duration,
    pub stop_grace: Duration,
    pub poll_interval: Duration,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            active_turn: Duration::from_secs(30 * 60),
            stop_grace: Duration::from_secs(5),
            poll_interval: Duration::from_millis(100),
        }
    }
}

impl WatchdogConfig {
    fn validate(&self) -> Result<(), SessionError> {
        if self.active_turn.is_zero() || self.stop_grace.is_zero() || self.poll_interval.is_zero() {
            return Err(SessionError::InvalidWatchdog);
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SessionStart {
    pub launch: LaunchOptions,
    pub initial_input: Option<UserInput>,
    pub watchdog: WatchdogConfig,
}

impl SessionStart {
    pub fn new(launch: LaunchOptions) -> Self {
        Self {
            launch,
            initial_input: None,
            watchdog: WatchdogConfig::default(),
        }
    }
}

impl std::fmt::Debug for SessionStart {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionStart")
            .field("launch", &self.launch)
            .field(
                "initial_input",
                &self.initial_input.as_ref().map(|_| "<present>"),
            )
            .field("watchdog", &self.watchdog)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEventPayload {
    Protocol {
        message: ProtocolMessage,
    },
    Stderr {
        text: String,
        truncated: bool,
    },
    Lifecycle {
        state: LifecycleState,
    },
    WatchdogTimeout,
    Exited {
        code: Option<i32>,
    },
    Error {
        code: SessionErrorCode,
        retryable: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Starting,
    Running,
    Interrupted,
    Stopping,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SessionErrorCode {
    SpawnFailed,
    OutputTooLarge,
    StdinClosed,
    ProtocolWriteFailed,
    PermissionNotPending,
    PermissionExpired,
    WatchdogTimeout,
    ProcessCheckFailed,
    ChildExited,
    EventConsumerGone,
    JobObjectFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionEvent {
    pub sequence: u64,
    pub run_id: Uuid,
    pub session_id: String,
    pub payload: SessionEventPayload,
}

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("another Claude session is already active")]
    AlreadyActive,
    #[error("the session watchdog configuration is invalid")]
    InvalidWatchdog,
    #[error("Claude CLI launch failed")]
    Launch(#[from] LaunchError),
    #[error("the session command channel is closed")]
    CommandChannelClosed,
    #[error("the session command did not acknowledge before the deadline")]
    CommandTimeout,
    #[error("the session event channel is closed")]
    EventChannelClosed,
    #[error("the session has already finished")]
    Finished,
    #[error("permission request is no longer pending")]
    PermissionNotPending,
    #[error("permission request has expired; retry it before responding")]
    PermissionExpired,
    #[error("permission request id does not match the pending request")]
    PermissionRequestMismatch,
    #[error("Claude did not report its canonical session id before startup timed out")]
    IdentityTimeout,
    #[error("Claude exited before reporting its canonical session id")]
    IdentityUnavailable,
    #[error("the session did not stop before the cleanup deadline")]
    StopTimeout,
    #[error("failed to write a protocol request to Claude")]
    ProtocolWriteFailed,
    #[error("failed to encode a protocol request")]
    ProtocolEncode,
}

#[derive(Debug)]
enum SessionCommand {
    User {
        input: UserInput,
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    Permission {
        request_id: String,
        decision: PermissionDecision,
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    RetryPermission {
        request_id: String,
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    Interrupt {
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    Stop {
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
    ForceStop {
        reply: oneshot::Sender<Result<(), SessionError>>,
    },
}

#[derive(Debug)]
struct EventLanes {
    critical: mpsc::Receiver<SessionEvent>,
    telemetry: mpsc::Receiver<SessionEvent>,
    critical_pending: Option<SessionEvent>,
    telemetry_pending: Option<SessionEvent>,
    critical_open: bool,
    telemetry_open: bool,
}

#[derive(Clone)]
struct EventLanesTx {
    critical: mpsc::Sender<SessionEvent>,
    telemetry: mpsc::Sender<SessionEvent>,
    consumer_gone: Arc<AtomicBool>,
}

impl EventLanes {
    async fn recv(&mut self) -> Option<SessionEvent> {
        loop {
            self.fill_pending();
            if let Some(event) = self.take_next_pending() {
                return Some(event);
            }
            if !self.critical_open && !self.telemetry_open {
                return None;
            }
            if !self.critical_open {
                match self.telemetry.recv().await {
                    Some(event) => self.telemetry_pending = Some(event),
                    None => self.telemetry_open = false,
                }
                continue;
            }
            if !self.telemetry_open {
                match self.critical.recv().await {
                    Some(event) => self.critical_pending = Some(event),
                    None => self.critical_open = false,
                }
                continue;
            }
            tokio::select! {
                event = self.critical.recv() => {
                    if let Some(event) = event {
                        self.critical_pending = Some(event);
                    } else {
                        self.critical_open = false;
                    }
                }
                event = self.telemetry.recv() => {
                    if let Some(event) = event {
                        self.telemetry_pending = Some(event);
                    } else {
                        self.telemetry_open = false;
                    }
                }
            }
        }
    }

    fn fill_pending(&mut self) {
        if self.critical_pending.is_none() && self.critical_open {
            match self.critical.try_recv() {
                Ok(event) => self.critical_pending = Some(event),
                Err(mpsc::error::TryRecvError::Disconnected) => self.critical_open = false,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
        }
        if self.telemetry_pending.is_none() && self.telemetry_open {
            match self.telemetry.try_recv() {
                Ok(event) => self.telemetry_pending = Some(event),
                Err(mpsc::error::TryRecvError::Disconnected) => self.telemetry_open = false,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
        }
    }

    fn take_next_pending(&mut self) -> Option<SessionEvent> {
        match (&self.critical_pending, &self.telemetry_pending) {
            (Some(critical), Some(telemetry)) if critical.sequence <= telemetry.sequence => {
                self.critical_pending.take()
            }
            (Some(_), Some(_)) => self.telemetry_pending.take(),
            (Some(_), None) => self.critical_pending.take(),
            (None, Some(_)) => self.telemetry_pending.take(),
            (None, None) => None,
        }
    }
}

#[derive(Clone)]
pub struct SessionHandle {
    run_id: Uuid,
    session_id: Arc<RwLock<String>>,
    identity: watch::Receiver<String>,
    commands: mpsc::Sender<SessionCommand>,
    events: Arc<Mutex<EventLanes>>,
}

impl std::fmt::Debug for SessionHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SessionHandle")
            .field("run_id", &self.run_id)
            .field("session_id", &self.session_id())
            .finish_non_exhaustive()
    }
}

impl SessionHandle {
    pub fn run_id(&self) -> Uuid {
        self.run_id
    }

    pub fn session_id(&self) -> String {
        self.session_id
            .read()
            .map(|id| id.clone())
            .unwrap_or_else(|_| "unknown".to_owned())
    }

    pub async fn recv(&self) -> Option<SessionEvent> {
        self.events.lock().await.recv().await
    }

    /// Waits for the CLI `system/init` record to publish its canonical session
    /// identity. New/resume/retry starts already know that identity and return
    /// immediately; continue/fork starts must not be exposed to callers until
    /// this method succeeds.
    pub async fn wait_for_session_id(&self, wait: Duration) -> Result<String, SessionError> {
        let mut identity = self.identity.clone();
        if !identity.borrow().is_empty() {
            return Ok(identity.borrow().clone());
        }
        timeout(wait, async {
            loop {
                identity
                    .changed()
                    .await
                    .map_err(|_| SessionError::IdentityUnavailable)?;
                if !identity.borrow().is_empty() {
                    return Ok(identity.borrow().clone());
                }
            }
        })
        .await
        .map_err(|_| SessionError::IdentityTimeout)?
    }

    pub async fn send_user(&self, input: UserInput) -> Result<(), SessionError> {
        self.send(|reply| SessionCommand::User { input, reply })
            .await
    }

    pub async fn allow(&self, request_id: impl Into<String>) -> Result<(), SessionError> {
        self.send(|reply| SessionCommand::Permission {
            request_id: request_id.into(),
            decision: PermissionDecision::Allow,
            reply,
        })
        .await
    }

    pub async fn retry_permission(
        &self,
        request_id: impl Into<String>,
    ) -> Result<(), SessionError> {
        self.send(|reply| SessionCommand::RetryPermission {
            request_id: request_id.into(),
            reply,
        })
        .await
    }

    pub async fn deny(
        &self,
        request_id: impl Into<String>,
        message: impl Into<String>,
        interrupt: bool,
    ) -> Result<(), SessionError> {
        self.send(|reply| SessionCommand::Permission {
            request_id: request_id.into(),
            decision: PermissionDecision::Deny {
                message: message.into(),
                interrupt,
            },
            reply,
        })
        .await
    }

    pub async fn interrupt(&self) -> Result<(), SessionError> {
        self.send(|reply| SessionCommand::Interrupt { reply }).await
    }

    pub async fn stop(&self) -> Result<(), SessionError> {
        self.send(|reply| SessionCommand::Stop { reply }).await
    }

    pub async fn force_stop(&self) -> Result<(), SessionError> {
        self.send(|reply| SessionCommand::ForceStop { reply }).await
    }

    async fn send<F>(&self, build: F) -> Result<(), SessionError>
    where
        F: FnOnce(oneshot::Sender<Result<(), SessionError>>) -> SessionCommand,
    {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(build(reply))
            .await
            .map_err(|_| SessionError::CommandChannelClosed)?;
        timeout(COMMAND_ACK_TIMEOUT, response)
            .await
            .map_err(|_| SessionError::CommandTimeout)?
            .map_err(|_| SessionError::CommandChannelClosed)?
    }
}

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<ManagerInner>,
}

struct ManagerInner {
    launcher: ClaudeLauncher,
    active: Mutex<Option<SessionHandle>>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new(ClaudeLauncher::default())
    }
}

impl SessionManager {
    pub fn new(launcher: ClaudeLauncher) -> Self {
        Self {
            inner: Arc::new(ManagerInner {
                launcher,
                active: Mutex::new(None),
            }),
        }
    }

    pub async fn is_active(&self) -> bool {
        self.inner.active.lock().await.is_some()
    }

    /// Returns a cloneable controller for the sole active child. All clones
    /// share the same serialized command channel and event receiver.
    pub async fn active_handle(&self) -> Option<SessionHandle> {
        self.inner.active.lock().await.clone()
    }

    pub async fn send_user(&self, input: UserInput) -> Result<(), SessionError> {
        self.active_handle()
            .await
            .ok_or(SessionError::Finished)?
            .send_user(input)
            .await
    }

    pub async fn allow(&self, request_id: impl Into<String>) -> Result<(), SessionError> {
        self.active_handle()
            .await
            .ok_or(SessionError::Finished)?
            .allow(request_id)
            .await
    }

    pub async fn retry_permission(
        &self,
        request_id: impl Into<String>,
    ) -> Result<(), SessionError> {
        self.active_handle()
            .await
            .ok_or(SessionError::Finished)?
            .retry_permission(request_id)
            .await
    }

    pub async fn deny(
        &self,
        request_id: impl Into<String>,
        message: impl Into<String>,
        interrupt: bool,
    ) -> Result<(), SessionError> {
        self.active_handle()
            .await
            .ok_or(SessionError::Finished)?
            .deny(request_id, message, interrupt)
            .await
    }

    pub async fn interrupt(&self) -> Result<(), SessionError> {
        self.active_handle()
            .await
            .ok_or(SessionError::Finished)?
            .interrupt()
            .await
    }

    pub async fn stop(&self) -> Result<(), SessionError> {
        self.active_handle()
            .await
            .ok_or(SessionError::Finished)?
            .stop()
            .await
    }

    pub async fn stop_and_wait(&self, wait: Duration) -> Result<(), SessionError> {
        let Some(handle) = self.active_handle().await else {
            return Ok(());
        };
        match handle.stop().await {
            Ok(()) | Err(SessionError::CommandChannelClosed) | Err(SessionError::Finished) => {}
            Err(error) => return Err(error),
        }
        timeout(wait, async {
            loop {
                if !self.is_active().await {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .map_err(|_| SessionError::StopTimeout)
    }

    /// Immediately terminates the active process tree and waits for manager
    /// cleanup. Intended for Tauri's exit/window-close path.
    pub async fn force_shutdown(&self) -> Result<(), SessionError> {
        let Some(handle) = self.active_handle().await else {
            return Ok(());
        };
        match handle.force_stop().await {
            Ok(()) | Err(SessionError::CommandChannelClosed) | Err(SessionError::Finished) => {}
            Err(error) => return Err(error),
        }
        timeout(Duration::from_secs(8), async {
            loop {
                if !self.is_active().await {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .map_err(|_| SessionError::StopTimeout)
    }

    pub async fn start(&self, mut start: SessionStart) -> Result<SessionHandle, SessionError> {
        start.watchdog.validate()?;
        let mut active = self.inner.active.lock().await;
        if active.is_some() {
            return Err(SessionError::AlreadyActive);
        }

        let run_id = Uuid::new_v4();
        let new_session_id = if matches!(start.launch.mode, super::SessionMode::New) {
            let id = start
                .launch
                .session_id
                .clone()
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            start.launch.session_id = Some(id.clone());
            Some(id)
        } else {
            None
        };
        let identity_pending = matches!(
            start.launch.mode,
            super::SessionMode::Continue | super::SessionMode::Fork { .. }
        );
        let initial_session_id = initial_session_id(&start.launch, run_id, new_session_id);
        let session_id = Arc::new(RwLock::new(initial_session_id.clone()));
        let initial_identity = if identity_pending {
            String::new()
        } else {
            initial_session_id.clone()
        };
        let (identity_tx, identity_rx) = watch::channel(initial_identity);
        let mut child = self.inner.launcher.spawn(&start.launch)?;
        let stdin = child.stdin.take().ok_or(SessionError::Finished)?;
        let stdout = child.stdout.take().ok_or(SessionError::Finished)?;
        let stderr = child.stderr.take().ok_or(SessionError::Finished)?;

        #[cfg(windows)]
        let job = {
            let pid = child.id().ok_or(SessionError::Finished)?;
            match super::windows_job::JobObject::for_pid(pid) {
                Ok(job) => Some(job),
                Err(error) => {
                    let _ = child.start_kill();
                    return Err(SessionError::Launch(LaunchError::JobObject(error)));
                }
            }
        };

        let (commands_tx, commands_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (critical_tx, critical_rx) = mpsc::channel(CRITICAL_EVENT_CHANNEL_CAPACITY);
        let (telemetry_tx, telemetry_rx) = mpsc::channel(TELEMETRY_EVENT_CHANNEL_CAPACITY);
        let consumer_gone = Arc::new(AtomicBool::new(false));
        let handle = SessionHandle {
            run_id,
            session_id: session_id.clone(),
            identity: identity_rx,
            commands: commands_tx,
            events: Arc::new(Mutex::new(EventLanes {
                critical: critical_rx,
                telemetry: telemetry_rx,
                critical_pending: None,
                telemetry_pending: None,
                critical_open: true,
                telemetry_open: true,
            })),
        };
        *active = Some(handle.clone());
        drop(active);
        let inner = Arc::clone(&self.inner);
        tokio::spawn(run_session(
            inner,
            run_id,
            session_id,
            identity_tx,
            initial_session_id,
            start,
            child,
            stdin,
            stdout,
            stderr,
            commands_rx,
            EventLanesTx {
                critical: critical_tx,
                telemetry: telemetry_tx,
                consumer_gone,
            },
            #[cfg(windows)]
            job,
        ));
        Ok(handle)
    }
}

fn initial_session_id(
    options: &LaunchOptions,
    _run_id: Uuid,
    new_session_id: Option<String>,
) -> String {
    match &options.mode {
        super::SessionMode::Resume { session_id } | super::SessionMode::Retry { session_id } => {
            session_id.clone()
        }
        super::SessionMode::New => new_session_id.expect("new sessions always receive an id"),
        // Continue resolves the most recent project session and fork creates a
        // new identity. Neither may temporarily masquerade as another session.
        super::SessionMode::Continue | super::SessionMode::Fork { .. } => String::new(),
    }
}

#[derive(Debug)]
enum ReaderMessage {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>, bool),
    Oversize { stderr: bool },
    IoFailure { stderr: bool },
    Closed { stderr: bool },
}

type QueuedInput = (UserInput, Option<oneshot::Sender<Result<(), SessionError>>>);

struct PendingPermission {
    request: PermissionRequest,
    deadline: Instant,
}

#[allow(clippy::too_many_arguments)]
async fn handle_protocol_line(
    line: &[u8],
    stdin: &mut ChildStdin,
    child: &mut Child,
    events: &EventLanesTx,
    sequence: &mut u64,
    run_id: Uuid,
    session_id: &Arc<RwLock<String>>,
    identity: &watch::Sender<String>,
    identity_pending: bool,
    queued_inputs: &mut Vec<QueuedInput>,
    pending_permissions: &mut HashMap<String, PendingPermission>,
    turn_deadline: &mut Option<Instant>,
    stopping: &mut bool,
    stop_deadline: &mut Option<Instant>,
    watchdog: &WatchdogConfig,
) {
    let parsed = parse_protocol_line(line);
    if let ProtocolMessage::Event(event) = &parsed {
        if let Some(incoming) = event.session_id.as_ref() {
            if matches!(event.kind, ProtocolEventKind::Init) {
                if let Ok(mut current) = session_id.write() {
                    *current = incoming.clone();
                }
                let _ = identity.send(incoming.clone());
                if identity_pending && !queued_inputs.is_empty() {
                    let mut queued = std::mem::take(queued_inputs).into_iter();
                    while let Some((input, reply)) = queued.next() {
                        if write_user(stdin, &input).await.is_err() {
                            if let Some(reply) = reply {
                                let _ = reply.send(Err(SessionError::ProtocolWriteFailed));
                            }
                            for (_, reply) in queued {
                                if let Some(reply) = reply {
                                    let _ = reply.send(Err(SessionError::Finished));
                                }
                            }
                            emit_error(
                                events,
                                sequence,
                                run_id,
                                session_id,
                                SessionErrorCode::ProtocolWriteFailed,
                                true,
                            )
                            .await;
                            begin_stop(child, stopping, stop_deadline, watchdog.stop_grace);
                            break;
                        }
                        if let Some(reply) = reply {
                            let _ = reply.send(Ok(()));
                        }
                        *turn_deadline = Some(Instant::now() + watchdog.active_turn);
                    }
                }
            }
        }
        match event.kind {
            ProtocolEventKind::PermissionRequest => {
                if let Some(request) = parsed.permission_request() {
                    pending_permissions.insert(
                        request.request_id.clone(),
                        PendingPermission {
                            request,
                            deadline: Instant::now() + PERMISSION_RESPONSE_TIMEOUT,
                        },
                    );
                }
            }
            ProtocolEventKind::Result => {
                pending_permissions.clear();
                *turn_deadline = None;
            }
            _ => {}
        }
    }
    emit(
        events,
        sequence,
        run_id,
        session_id,
        SessionEventPayload::Protocol { message: parsed },
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn run_session(
    inner: Arc<ManagerInner>,
    run_id: Uuid,
    session_id: Arc<RwLock<String>>,
    identity: watch::Sender<String>,
    initial_session_id: String,
    start: SessionStart,
    mut child: Child,
    mut stdin: ChildStdin,
    stdout: impl AsyncRead + Unpin + Send + 'static,
    stderr: impl AsyncRead + Unpin + Send + 'static,
    mut commands: mpsc::Receiver<SessionCommand>,
    events: EventLanesTx,
    #[cfg(windows)] job: Option<super::windows_job::JobObject>,
) {
    let (reader_tx, mut reader_rx) = mpsc::channel::<ReaderMessage>(128);
    let stdout_task = tokio::spawn(read_stdout(stdout, reader_tx.clone()));
    let stderr_task = tokio::spawn(read_stderr(stderr, reader_tx.clone()));
    drop(reader_tx);

    let mut sequence = 0_u64;
    let mut ticker = interval(start.watchdog.poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut turn_deadline: Option<Instant> = None;
    let mut pending_permissions = HashMap::<String, PendingPermission>::new();
    let mut stopping = false;
    let mut stop_deadline: Option<Instant> = None;
    let mut stdout_closed = false;
    let mut stderr_closed = false;
    let mut stderr_total = 0_usize;
    let identity_pending = initial_session_id.is_empty();
    let mut queued_inputs = Vec::<QueuedInput>::new();

    emit(
        &events,
        &mut sequence,
        run_id,
        &session_id,
        SessionEventPayload::Lifecycle {
            state: LifecycleState::Starting,
        },
    )
    .await;
    emit(
        &events,
        &mut sequence,
        run_id,
        &session_id,
        SessionEventPayload::Lifecycle {
            state: LifecycleState::Running,
        },
    )
    .await;

    if let Some(input) = start.initial_input {
        if identity_pending {
            queued_inputs.push((input, None));
        } else if write_user(&mut stdin, &input).await.is_err() {
            emit_error(
                &events,
                &mut sequence,
                run_id,
                &session_id,
                SessionErrorCode::ProtocolWriteFailed,
                true,
            )
            .await;
            stopping = true;
            stop_deadline = Some(Instant::now() + start.watchdog.stop_grace);
        } else {
            turn_deadline = Some(Instant::now() + start.watchdog.active_turn);
        }
    }

    loop {
        if stdout_closed && stderr_closed {
            if let Ok(Some(status)) = child.try_wait() {
                emit(
                    &events,
                    &mut sequence,
                    run_id,
                    &session_id,
                    SessionEventPayload::Exited {
                        code: status.code(),
                    },
                )
                .await;
                break;
            }
        }

        tokio::select! {
            Some(message) = reader_rx.recv() => {
                match message {
                    ReaderMessage::Stdout(line) => {
                        handle_protocol_line(
                            &line,
                            &mut stdin,
                            &mut child,
                            &events,
                            &mut sequence,
                            run_id,
                            &session_id,
                            &identity,
                            identity_pending,
                            &mut queued_inputs,
                            &mut pending_permissions,
                            &mut turn_deadline,
                            &mut stopping,
                            &mut stop_deadline,
                            &start.watchdog,
                        )
                        .await;
                    }
                    ReaderMessage::Stderr(bytes, truncated) => {
                        stderr_total = stderr_total.saturating_add(bytes.len());
                        let text = String::from_utf8_lossy(&bytes).into_owned();
                        emit(&events, &mut sequence, run_id, &session_id, SessionEventPayload::Stderr { text, truncated }).await;
                        if stderr_total >= MAX_STDERR_TOTAL_BYTES {
                            stderr_closed = true;
                        }
                    }
                    ReaderMessage::Oversize { stderr } => {
                        emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::OutputTooLarge, false).await;
                        if stderr { stderr_closed = true; } else { stdout_closed = true; }
                        if !stopping {
                            let _ = child.start_kill();
                            stopping = true;
                            stop_deadline = Some(Instant::now() + start.watchdog.stop_grace);
                        }
                    }
                    ReaderMessage::IoFailure { stderr } => {
                        if stderr { stderr_closed = true; } else { stdout_closed = true; }
                        emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::ProcessCheckFailed, true).await;
                    }
                    ReaderMessage::Closed { stderr } => {
                        if stderr { stderr_closed = true; } else { stdout_closed = true; }
                    }
                }
            }
            Some(command) = commands.recv() => {
                match command {
                    SessionCommand::User { input, reply } => {
                        if stopping {
                            let _ = reply.send(Err(SessionError::Finished));
                            continue;
                        }
                        if identity_pending && session_id.read().map(|id| id.is_empty()).unwrap_or(true) {
                            queued_inputs.push((input, Some(reply)));
                            continue;
                        }
                        match write_user(&mut stdin, &input).await {
                            Ok(()) => {
                                turn_deadline = Some(Instant::now() + start.watchdog.active_turn);
                                pending_permissions.clear();
                                let _ = reply.send(Ok(()));
                            }
                            Err(_) => {
                                let _ = reply.send(Err(SessionError::ProtocolWriteFailed));
                                emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::ProtocolWriteFailed, true).await;
                                begin_stop(&mut child, &mut stopping, &mut stop_deadline, start.watchdog.stop_grace);
                            }
                        }
                    }
                    SessionCommand::Permission { request_id, decision, reply } => {
                        let Some(pending) = pending_permissions.remove(&request_id) else {
                            let _ = reply.send(Err(SessionError::PermissionNotPending));
                            emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::PermissionNotPending, true).await;
                            continue;
                        };
                        if pending.deadline <= Instant::now() {
                            pending_permissions.insert(request_id, pending);
                            let _ = reply.send(Err(SessionError::PermissionExpired));
                            emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::PermissionExpired, true).await;
                            continue;
                        }
                        let interrupted = matches!(
                            &decision,
                            PermissionDecision::Deny {
                                interrupt: true,
                                ..
                            }
                        );
                        match encode_permission_response(&pending.request, &decision) {
                            Ok(bytes) if write_bytes(&mut stdin, &bytes).await.is_ok() => {
                                let _ = reply.send(Ok(()));
                                emit(
                                    &events,
                                    &mut sequence,
                                    run_id,
                                    &session_id,
                                    SessionEventPayload::Lifecycle {
                                        state: if interrupted {
                                            LifecycleState::Interrupted
                                        } else {
                                            LifecycleState::Running
                                        },
                                    },
                                )
                                .await;
                                // interrupt: true ends the current Claude turn,
                                // but it must not terminate the long-lived CLI
                                // process. The next user message can then resume
                                // the same conversation, matching terminal Claude.
                                turn_deadline = if interrupted {
                                    None
                                } else {
                                    Some(Instant::now() + start.watchdog.active_turn)
                                };
                            }
                            _ => {
                                pending_permissions.insert(request_id, pending);
                                let _ = reply.send(Err(SessionError::ProtocolWriteFailed));
                                emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::ProtocolWriteFailed, true).await;
                            }
                        }
                    }
                    SessionCommand::RetryPermission { request_id, reply } => {
                        match pending_permissions.get_mut(&request_id) {
                            Some(pending) => {
                                pending.deadline = Instant::now() + PERMISSION_RESPONSE_TIMEOUT;
                                let _ = reply.send(Ok(()));
                            }
                            None => {
                                let _ = reply.send(Err(SessionError::PermissionNotPending));
                                emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::PermissionNotPending, true).await;
                            }
                        }
                    }
                    SessionCommand::Interrupt { reply } | SessionCommand::Stop { reply } => {
                        if !stopping {
                            emit(
                                &events,
                                &mut sequence,
                                run_id,
                                &session_id,
                                SessionEventPayload::Lifecycle {
                                    state: LifecycleState::Stopping,
                                },
                            )
                            .await;
                            if let Ok(bytes) = encode_interrupt(&format!("interrupt-{run_id}")) {
                                let _ = write_bytes(&mut stdin, &bytes).await;
                            }
                            begin_stop(&mut child, &mut stopping, &mut stop_deadline, start.watchdog.stop_grace);
                        }
                        let _ = reply.send(Ok(()));
                    }
                    SessionCommand::ForceStop { reply } => {
                        if !stopping {
                            emit(
                                &events,
                                &mut sequence,
                                run_id,
                                &session_id,
                                SessionEventPayload::Lifecycle {
                                    state: LifecycleState::Stopping,
                                },
                            )
                            .await;
                        }
                        #[cfg(windows)]
                        if let Some(job) = &job {
                            let _ = job.terminate();
                        }
                        let _ = child.start_kill();
                        stopping = true;
                        stop_deadline = None;
                        let _ = reply.send(Ok(()));
                    }
                }
            }
            _ = ticker.tick() => {
                if events.consumer_gone.load(Ordering::Acquire) {
                    #[cfg(windows)]
                    if let Some(job) = &job {
                        let _ = job.terminate();
                    }
                    let _ = child.start_kill();
                    break;
                }
                if let Ok(Some(status)) = child.try_wait() {
                    if stdout_closed && stderr_closed {
                        emit(
                            &events,
                            &mut sequence,
                            run_id,
                            &session_id,
                            SessionEventPayload::Exited {
                                code: status.code(),
                            },
                        )
                        .await;
                        break;
                    }
                }
                let now = Instant::now();
                if !stopping {
                    if let Some(deadline) = turn_deadline {
                        if pending_permissions.is_empty() && now >= deadline {
                            emit(&events, &mut sequence, run_id, &session_id, SessionEventPayload::WatchdogTimeout).await;
                            if let Ok(bytes) = encode_interrupt(&format!("watchdog-{run_id}")) {
                                let _ = write_bytes(&mut stdin, &bytes).await;
                            }
                            begin_stop(&mut child, &mut stopping, &mut stop_deadline, start.watchdog.stop_grace);
                        }
                    }
                } else if stop_deadline.is_some_and(|deadline| now >= deadline) {
                    #[cfg(windows)]
                    if let Some(job) = &job {
                        let _ = job.terminate();
                    }
                    let _ = child.start_kill();
                    emit_error(&events, &mut sequence, run_id, &session_id, SessionErrorCode::ChildExited, true).await;
                    stop_deadline = None;
                }
            }
            else => break,
        }
    }

    let reader_drain = timeout(READER_DRAIN_TIMEOUT, async {
        while let Some(message) = reader_rx.recv().await {
            match message {
                ReaderMessage::Stdout(line) => {
                    handle_protocol_line(
                        &line,
                        &mut stdin,
                        &mut child,
                        &events,
                        &mut sequence,
                        run_id,
                        &session_id,
                        &identity,
                        identity_pending,
                        &mut queued_inputs,
                        &mut pending_permissions,
                        &mut turn_deadline,
                        &mut stopping,
                        &mut stop_deadline,
                        &start.watchdog,
                    )
                    .await;
                }
                ReaderMessage::Stderr(bytes, truncated) => {
                    let text = String::from_utf8_lossy(&bytes).into_owned();
                    emit(
                        &events,
                        &mut sequence,
                        run_id,
                        &session_id,
                        SessionEventPayload::Stderr { text, truncated },
                    )
                    .await;
                }
                ReaderMessage::Oversize { stderr } => {
                    emit_error(
                        &events,
                        &mut sequence,
                        run_id,
                        &session_id,
                        SessionErrorCode::OutputTooLarge,
                        false,
                    )
                    .await;
                    if stderr {
                        stderr_closed = true;
                    } else {
                        stdout_closed = true;
                    }
                }
                ReaderMessage::IoFailure { stderr } => {
                    emit_error(
                        &events,
                        &mut sequence,
                        run_id,
                        &session_id,
                        SessionErrorCode::ProcessCheckFailed,
                        true,
                    )
                    .await;
                    if stderr {
                        stderr_closed = true;
                    } else {
                        stdout_closed = true;
                    }
                }
                ReaderMessage::Closed { stderr } => {
                    if stderr {
                        stderr_closed = true;
                    } else {
                        stdout_closed = true;
                    }
                }
            }
        }
    })
    .await;
    if reader_drain.is_err() {
        stdout_task.abort();
        stderr_task.abort();
    } else {
        let _ = stdout_task.await;
        let _ = stderr_task.await;
    }
    for (_, reply) in queued_inputs {
        if let Some(reply) = reply {
            let _ = reply.send(Err(SessionError::IdentityUnavailable));
        }
    }
    let _ = child.start_kill();
    let _ = timeout(start.watchdog.stop_grace, child.wait()).await;
    release_run(&inner, run_id).await;
    emit(
        &events,
        &mut sequence,
        run_id,
        &session_id,
        SessionEventPayload::Lifecycle {
            state: LifecycleState::Finished,
        },
    )
    .await;
    let _ = initial_session_id;
}

async fn release_run(inner: &ManagerInner, run_id: Uuid) {
    let mut active = inner.active.lock().await;
    if active
        .as_ref()
        .is_some_and(|handle| handle.run_id() == run_id)
    {
        *active = None;
    }
}

fn begin_stop(
    child: &mut Child,
    stopping: &mut bool,
    stop_deadline: &mut Option<Instant>,
    grace: Duration,
) {
    *stopping = true;
    *stop_deadline = Some(Instant::now() + grace);
    let _ = child.try_wait();
}

async fn write_user(stdin: &mut ChildStdin, input: &UserInput) -> Result<(), ()> {
    let bytes = encode_user_input(input).map_err(|_| ())?;
    if bytes.len() > MAX_STDIN_MESSAGE_BYTES {
        return Err(());
    }
    write_bytes(stdin, &bytes).await
}

async fn write_bytes(stdin: &mut ChildStdin, bytes: &[u8]) -> Result<(), ()> {
    timeout(STDIN_WRITE_TIMEOUT, async {
        stdin.write_all(bytes).await?;
        stdin.flush().await
    })
    .await
    .map_err(|_| ())?
    .map_err(|_| ())
}

async fn emit(
    events: &EventLanesTx,
    sequence: &mut u64,
    run_id: Uuid,
    session_id: &Arc<RwLock<String>>,
    payload: SessionEventPayload,
) {
    *sequence = sequence.saturating_add(1);
    let critical = is_critical_payload(&payload);
    let event = SessionEvent {
        sequence: *sequence,
        run_id,
        session_id: session_id
            .read()
            .map(|id| id.clone())
            .unwrap_or_else(|_| "unknown".to_owned()),
        payload,
    };
    if critical {
        if events.critical.try_send(event).is_err() {
            // A closed or saturated critical lane means the consumer can no
            // longer receive a complete protocol history. Fail the run closed
            // instead of waiting forever or silently dropping the event.
            events.consumer_gone.store(true, Ordering::Release);
        }
    } else if events.telemetry.try_send(event).is_err() {
        // Telemetry is intentionally lossy and cannot consume critical capacity.
    }
}

fn is_critical_payload(payload: &SessionEventPayload) -> bool {
    match payload {
        SessionEventPayload::Lifecycle { .. }
        | SessionEventPayload::WatchdogTimeout
        | SessionEventPayload::Exited { .. }
        | SessionEventPayload::Error { .. } => true,
        SessionEventPayload::Stderr { .. } => false,
        SessionEventPayload::Protocol { message } => match message {
            ProtocolMessage::Event(event) => !matches!(
                event.kind,
                ProtocolEventKind::StreamEvent
                    | ProtocolEventKind::ToolProgress
                    | ProtocolEventKind::ToolUseSummary
                    | ProtocolEventKind::HookProgress
            ),
            ProtocolMessage::Malformed(_) => false,
        },
    }
}

async fn emit_error(
    events: &EventLanesTx,
    sequence: &mut u64,
    run_id: Uuid,
    session_id: &Arc<RwLock<String>>,
    code: SessionErrorCode,
    retryable: bool,
) {
    emit(
        events,
        sequence,
        run_id,
        session_id,
        SessionEventPayload::Error { code, retryable },
    )
    .await;
}

async fn read_stdout<R: AsyncRead + Unpin>(mut reader: R, sender: mpsc::Sender<ReaderMessage>) {
    let mut pending = Vec::new();
    let mut total = 0_usize;
    let mut chunk = [0_u8; READ_CHUNK_BYTES];
    loop {
        let count = match reader.read(&mut chunk).await {
            Ok(count) => count,
            Err(_) => {
                let _ = sender
                    .send(ReaderMessage::IoFailure { stderr: false })
                    .await;
                return;
            }
        };
        if count == 0 {
            if !pending.is_empty() {
                total = total.saturating_add(pending.len());
                if total > MAX_STDOUT_TOTAL_BYTES {
                    let _ = sender.send(ReaderMessage::Oversize { stderr: false }).await;
                    return;
                }
                let _ = sender
                    .send(ReaderMessage::Stdout(std::mem::take(&mut pending)))
                    .await;
            }
            let _ = sender.send(ReaderMessage::Closed { stderr: false }).await;
            return;
        }
        pending.extend_from_slice(&chunk[..count]);
        let mut start = 0;
        for index in 0..pending.len() {
            if pending[index] == b'\n' {
                let mut line = pending[start..index].to_vec();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                if line.len() > MAX_OUTPUT_LINE_BYTES {
                    let _ = sender.send(ReaderMessage::Oversize { stderr: false }).await;
                    return;
                }
                total = total.saturating_add(line.len());
                if total > MAX_STDOUT_TOTAL_BYTES {
                    let _ = sender.send(ReaderMessage::Oversize { stderr: false }).await;
                    return;
                }
                if sender.send(ReaderMessage::Stdout(line)).await.is_err() {
                    return;
                }
                start = index + 1;
            }
        }
        if start > 0 {
            pending.drain(..start);
        }
        if pending.len() > MAX_OUTPUT_LINE_BYTES {
            let _ = sender.send(ReaderMessage::Oversize { stderr: false }).await;
            return;
        }
    }
}

async fn read_stderr<R: AsyncRead + Unpin>(mut reader: R, sender: mpsc::Sender<ReaderMessage>) {
    let mut total = 0_usize;
    let mut chunk = [0_u8; MAX_STDERR_EVENT_BYTES];
    loop {
        let count = match reader.read(&mut chunk).await {
            Ok(count) => count,
            Err(_) => {
                let _ = sender.send(ReaderMessage::IoFailure { stderr: true }).await;
                return;
            }
        };
        if count == 0 {
            let _ = sender.send(ReaderMessage::Closed { stderr: true }).await;
            return;
        }
        let remaining = MAX_STDERR_TOTAL_BYTES.saturating_sub(total);
        if remaining == 0 {
            let _ = sender.send(ReaderMessage::Oversize { stderr: true }).await;
            return;
        }
        let take = count.min(remaining);
        total += take;
        let truncated = take < count || total == MAX_STDERR_TOTAL_BYTES;
        if sender
            .send(ReaderMessage::Stderr(chunk[..take].to_vec(), truncated))
            .await
            .is_err()
        {
            return;
        }
        if truncated {
            let _ = sender.send(ReaderMessage::Oversize { stderr: true }).await;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::{ClaudeLauncher, LaunchOptions, SessionMode};

    #[tokio::test]
    async fn manager_rejects_a_second_active_session() {
        // This only tests the manager gate without starting a process.
        let manager = SessionManager::new(ClaudeLauncher::new("definitely-not-a-real-cli"));
        let inner = Arc::clone(&manager.inner);
        let (commands, _) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (_, identity) = watch::channel("session".to_owned());
        let (_, critical) = mpsc::channel(1);
        let (_, telemetry) = mpsc::channel(1);
        *inner.active.lock().await = Some(SessionHandle {
            run_id: Uuid::nil(),
            session_id: Arc::new(RwLock::new("session".to_owned())),
            identity,
            commands,
            events: Arc::new(Mutex::new(test_lanes(critical, telemetry))),
        });
        let result = manager
            .start(SessionStart::new(LaunchOptions::new(SessionMode::New)))
            .await;
        assert!(matches!(result, Err(SessionError::AlreadyActive)));
        *inner.active.lock().await = None;
    }

    #[tokio::test]
    async fn command_send_waits_for_manager_acknowledgement() {
        let (commands, mut received) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (_, identity) = watch::channel("session".to_owned());
        let (_, critical) = mpsc::channel(1);
        let (_, telemetry) = mpsc::channel(1);
        let handle = SessionHandle {
            run_id: Uuid::nil(),
            session_id: Arc::new(RwLock::new("session".to_owned())),
            identity,
            commands,
            events: Arc::new(Mutex::new(test_lanes(critical, telemetry))),
        };

        let sending = tokio::spawn(async move { handle.send_user(UserInput::text("hello")).await });
        let command = received.recv().await.unwrap();
        assert!(!sending.is_finished());
        let SessionCommand::User { input, reply } = command else {
            panic!("expected user command")
        };
        assert_eq!(input.content(), &serde_json::json!("hello"));
        reply.send(Ok(())).unwrap();
        assert!(sending.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn final_unterminated_result_is_emitted_before_finish() {
        use std::process::Stdio;
        use tokio::io::{duplex, AsyncWriteExt};

        #[cfg(windows)]
        let mut process = {
            let mut command = tokio::process::Command::new("cmd.exe");
            command.args(["/C", "exit", "0"]);
            command
        };
        #[cfg(unix)]
        let mut process = {
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", "exit 0"]);
            command
        };
        process
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = process.spawn().unwrap();
        let stdin = child.stdin.take().unwrap();

        let (mut stdout_writer, stdout_reader) = duplex(4_096);
        stdout_writer
            .write_all(br#"{"type":"result","subtype":"success","session_id":"session-final"}"#)
            .await
            .unwrap();
        drop(stdout_writer);
        let (stderr_writer, stderr_reader) = duplex(64);
        drop(stderr_writer);

        let inner = Arc::new(ManagerInner {
            launcher: ClaudeLauncher::new("unused"),
            active: Mutex::new(None),
        });
        let session_id = Arc::new(RwLock::new("session-final".to_owned()));
        let (identity, _) = watch::channel("session-final".to_owned());
        let (_commands, commands_rx) = mpsc::channel(COMMAND_CHANNEL_CAPACITY);
        let (critical_tx, mut critical_rx) = mpsc::channel(32);
        let (telemetry_tx, _telemetry_rx) = mpsc::channel(8);
        let events = EventLanesTx {
            critical: critical_tx,
            telemetry: telemetry_tx,
            consumer_gone: Arc::new(AtomicBool::new(false)),
        };
        let mut launch = LaunchOptions::new(SessionMode::New);
        launch.session_id = Some("session-final".to_owned());

        run_session(
            inner,
            Uuid::nil(),
            session_id,
            identity,
            "session-final".to_owned(),
            SessionStart::new(launch),
            child,
            stdin,
            stdout_reader,
            stderr_reader,
            commands_rx,
            events,
            #[cfg(windows)]
            None,
        )
        .await;

        let mut payloads = Vec::new();
        while let Ok(event) = critical_rx.try_recv() {
            payloads.push(event.payload);
        }
        let result_index = payloads
            .iter()
            .position(|payload| {
                matches!(
                    payload,
                    SessionEventPayload::Protocol {
                        message: ProtocolMessage::Event(event)
                    } if event.kind == ProtocolEventKind::Result
                )
            })
            .expect("the final result record must be emitted");
        let finish_index = payloads
            .iter()
            .position(|payload| {
                matches!(
                    payload,
                    SessionEventPayload::Lifecycle {
                        state: LifecycleState::Finished
                    }
                )
            })
            .expect("the run must finish");
        assert!(result_index < finish_index);
    }

    #[tokio::test]
    async fn telemetry_saturation_does_not_block_critical_events() {
        let (critical_tx, critical_rx) = mpsc::channel(2);
        let (telemetry_tx, telemetry_rx) = mpsc::channel(1);
        let consumer_gone = Arc::new(AtomicBool::new(false));
        let tx = EventLanesTx {
            critical: critical_tx,
            telemetry: telemetry_tx,
            consumer_gone: Arc::clone(&consumer_gone),
        };
        let mut lanes = test_lanes(critical_rx, telemetry_rx);
        let run_id = Uuid::nil();
        let session_id = Arc::new(RwLock::new("session".into()));
        let mut sequence = 0;

        emit(
            &tx,
            &mut sequence,
            run_id,
            &session_id,
            SessionEventPayload::Stderr {
                text: "first".into(),
                truncated: false,
            },
        )
        .await;
        emit(
            &tx,
            &mut sequence,
            run_id,
            &session_id,
            SessionEventPayload::Stderr {
                text: "dropped".into(),
                truncated: false,
            },
        )
        .await;
        emit(
            &tx,
            &mut sequence,
            run_id,
            &session_id,
            SessionEventPayload::Lifecycle {
                state: LifecycleState::Starting,
            },
        )
        .await;

        let first = lanes.recv().await.unwrap();
        let second = lanes.recv().await.unwrap();
        assert_eq!((first.sequence, second.sequence), (1, 3));
        assert!(matches!(
            second.payload,
            SessionEventPayload::Lifecycle {
                state: LifecycleState::Starting
            }
        ));
        assert!(!consumer_gone.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn merged_lanes_preserve_sequence_for_buffered_events() {
        let (critical_tx, critical_rx) = mpsc::channel(2);
        let (telemetry_tx, telemetry_rx) = mpsc::channel(2);
        telemetry_tx.send(test_event(1, false)).await.unwrap();
        critical_tx.send(test_event(2, true)).await.unwrap();
        telemetry_tx.send(test_event(3, false)).await.unwrap();
        drop(critical_tx);
        drop(telemetry_tx);

        let mut lanes = test_lanes(critical_rx, telemetry_rx);
        let mut sequences = Vec::new();
        while let Some(event) = lanes.recv().await {
            sequences.push(event.sequence);
        }
        assert_eq!(sequences, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn saturated_critical_lane_marks_the_consumer_unusable() {
        let (critical_tx, _critical_rx) = mpsc::channel(1);
        let (telemetry_tx, _telemetry_rx) = mpsc::channel(1);
        let consumer_gone = Arc::new(AtomicBool::new(false));
        let tx = EventLanesTx {
            critical: critical_tx,
            telemetry: telemetry_tx,
            consumer_gone: Arc::clone(&consumer_gone),
        };
        let run_id = Uuid::nil();
        let session_id = Arc::new(RwLock::new("session".into()));
        let mut sequence = 0;

        for state in [LifecycleState::Starting, LifecycleState::Running] {
            emit(
                &tx,
                &mut sequence,
                run_id,
                &session_id,
                SessionEventPayload::Lifecycle { state },
            )
            .await;
        }

        assert!(consumer_gone.load(Ordering::Acquire));
    }

    fn test_lanes(
        critical: mpsc::Receiver<SessionEvent>,
        telemetry: mpsc::Receiver<SessionEvent>,
    ) -> EventLanes {
        EventLanes {
            critical,
            telemetry,
            critical_pending: None,
            telemetry_pending: None,
            critical_open: true,
            telemetry_open: true,
        }
    }

    fn test_event(sequence: u64, critical: bool) -> SessionEvent {
        SessionEvent {
            sequence,
            run_id: Uuid::nil(),
            session_id: "session".into(),
            payload: if critical {
                SessionEventPayload::Lifecycle {
                    state: LifecycleState::Running,
                }
            } else {
                SessionEventPayload::Stderr {
                    text: "telemetry".into(),
                    truncated: false,
                }
            },
        }
    }

    #[test]
    fn event_ids_and_error_payload_are_serializable() {
        let event = SessionEvent {
            sequence: 1,
            run_id: Uuid::nil(),
            session_id: "s".into(),
            payload: SessionEventPayload::Error {
                code: SessionErrorCode::OutputTooLarge,
                retryable: false,
            },
        };
        let value = serde_json::to_value(event).unwrap();
        assert_eq!(value["sequence"], 1);
        assert_eq!(value["payload"]["type"], "error");
        assert_eq!(value["payload"]["code"], "OUTPUT_TOO_LARGE");
    }
    #[tokio::test]
    async fn permission_requests_are_queued_before_responses_arrive() {
        use std::process::Stdio;

        #[cfg(windows)]
        let mut process = {
            let mut command = tokio::process::Command::new("cmd.exe");
            command.args(["/C", "exit", "0"]);
            command
        };
        #[cfg(unix)]
        let mut process = {
            let mut command = tokio::process::Command::new("sh");
            command.args(["-c", "exit 0"]);
            command
        };
        process
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = process.spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();

        let (critical_tx, mut critical_rx) = mpsc::channel(8);
        let (telemetry_tx, _telemetry_rx) = mpsc::channel(8);
        let events = EventLanesTx {
            critical: critical_tx,
            telemetry: telemetry_tx,
            consumer_gone: Arc::new(AtomicBool::new(false)),
        };
        let session_id = Arc::new(RwLock::new("session-permission".to_owned()));
        let (identity, _) = watch::channel("session-permission".to_owned());
        let mut sequence = 0;
        let mut queued_inputs = Vec::new();
        let mut pending_permissions = HashMap::new();
        let mut turn_deadline = None;
        let mut stopping = false;
        let mut stop_deadline = None;
        let watchdog = WatchdogConfig::default();

        for (request_id, command) in [("request-1", "first"), ("request-2", "second")] {
            let line = format!(
                r#"{{"type":"control_request","request_id":"{request_id}","request":{{"subtype":"can_use_tool","tool_name":"Bash","input":{{"command":"{command}"}}}}}}"#
            );
            handle_protocol_line(
                line.as_bytes(),
                &mut stdin,
                &mut child,
                &events,
                &mut sequence,
                Uuid::nil(),
                &session_id,
                &identity,
                false,
                &mut queued_inputs,
                &mut pending_permissions,
                &mut turn_deadline,
                &mut stopping,
                &mut stop_deadline,
                &watchdog,
            )
            .await;
        }

        assert_eq!(pending_permissions.len(), 2);
        assert!(pending_permissions.contains_key("request-1"));
        assert!(pending_permissions.contains_key("request-2"));
        assert_eq!(sequence, 2);
        assert!(matches!(
            critical_rx.try_recv().unwrap().payload,
            SessionEventPayload::Protocol {
                message: ProtocolMessage::Event(_)
            }
        ));
        child.start_kill().unwrap();
    }
}

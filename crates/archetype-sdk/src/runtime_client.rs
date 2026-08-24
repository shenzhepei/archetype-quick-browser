use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    thread,
    time::{Duration, Instant},
};

use arch_paint::DisplayList;
use archetype_protocol::{
    BrokeredResource, Capability, ClientHello, Codec, Envelope, Message, PROTOCOL_MINOR, Request,
    ResourceKind, Response,
};
use archetype_types::{ArchetypeUrl, NavigationId, PageId};
use thiserror::Error;

const LAUNCH_AUTH_MAGIC: [u8; 4] = *b"ARUN";
const LAUNCH_TOKEN_BYTES: usize = 32;
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_QUEUE_LIMIT: usize = 256;
const MAXIMUM_PENDING_REQUESTS: usize = 64;
const SUPERVISOR_TICK: Duration = Duration::from_millis(10);
const RESOURCE_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
const RESTART_BACKOFF: [Duration; 3] = [
    Duration::from_millis(100),
    Duration::from_millis(500),
    Duration::from_secs(2),
];

#[derive(Clone, Copy, Debug)]
pub struct RuntimeLimits {
    pub request_timeout: Duration,
    pub maximum_rss_bytes: u64,
    pub maximum_in_flight_bytes: usize,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(5),
            maximum_rss_bytes: 512 * 1024 * 1024,
            maximum_in_flight_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StaticDocument {
    pub page_id: PageId,
    pub navigation_id: NavigationId,
    pub url: ArchetypeUrl,
    pub html: String,
    pub viewport_width_px: u32,
    pub resources: Vec<BrokeredResource>,
    pub broker_diagnostics: Vec<String>,
}

impl StaticDocument {
    #[must_use]
    pub fn protocol_envelope(&self, request_id: u64) -> Envelope {
        Envelope::v4(
            request_id,
            Message::Request(Request::RenderDocument {
                page_id: self.page_id.clone(),
                navigation_id: self.navigation_id,
                url: self.url.clone(),
                html: self.html.clone(),
                viewport_width_px: self.viewport_width_px,
                resources: self.resources.clone(),
            }),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeRenderedPage {
    pub page_id: PageId,
    pub navigation_id: NavigationId,
    pub final_url: ArchetypeUrl,
    pub title: String,
    pub display_list: DisplayList,
    pub diagnostics: Vec<String>,
    pub image_resources: HashMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProcessError {
    #[error("could not start runtime supervisor: {0}")]
    SupervisorStart(String),
    #[error("could not start renderer runtime: {0}")]
    Spawn(String),
    #[error("renderer runtime handshake timed out")]
    HandshakeTimeout,
    #[error("renderer runtime rejected the handshake: {0}")]
    HandshakeRejected(String),
    #[error("renderer runtime does not provide the required capabilities")]
    MissingCapabilities,
    #[error("renderer runtime protocol failed: {0}")]
    Protocol(String),
    #[error("renderer runtime disconnected")]
    RuntimeDisconnected,
    #[error("renderer runtime request queue is full")]
    Backpressure,
    #[error("renderer runtime request timed out")]
    RequestTimedOut,
    #[error("renderer runtime returned an unexpected response")]
    UnexpectedResponse,
    #[error("renderer runtime failed with {code}: {message}")]
    RuntimeFailure { code: String, message: String },
    #[error("renderer runtime exceeded its {resource} limit")]
    ResourceLimit { resource: String },
}

pub type ReadyReceiver = Receiver<Result<(), RuntimeProcessError>>;
pub type RenderReceiver = Receiver<Result<RuntimeRenderedPage, RuntimeProcessError>>;
pub type ShutdownReceiver = Receiver<Result<(), RuntimeProcessError>>;

pub struct RuntimeSupervisor {
    commands: SyncSender<SupervisorCommand>,
    active: Arc<AtomicBool>,
    command_gate: Arc<Mutex<()>>,
    queued_bytes: Arc<AtomicUsize>,
    terminal_error: Arc<Mutex<Option<RuntimeProcessError>>>,
    limits: RuntimeLimits,
}

#[derive(Clone)]
struct SupervisorControl {
    active: Arc<AtomicBool>,
    command_gate: Arc<Mutex<()>>,
    queued_bytes: Arc<AtomicUsize>,
    terminal_error: Arc<Mutex<Option<RuntimeProcessError>>>,
}

impl RuntimeSupervisor {
    /// Starts a background supervisor for one renderer runtime executable.
    ///
    /// The returned readiness channel completes after the child handshake. No process operation
    /// blocks the calling thread.
    ///
    /// # Errors
    /// Returns an error only when the supervisor thread itself cannot be created.
    pub fn spawn(
        executable: impl AsRef<Path>,
    ) -> Result<(Self, ReadyReceiver), RuntimeProcessError> {
        Self::spawn_with_limits(executable, RuntimeLimits::default())
    }

    /// Starts a supervisor with explicit limits for automated probes.
    ///
    /// # Errors
    /// Returns an error only when the supervisor thread itself cannot be created.
    pub fn spawn_with_limits(
        executable: impl AsRef<Path>,
        limits: RuntimeLimits,
    ) -> Result<(Self, ReadyReceiver), RuntimeProcessError> {
        let executable = executable.as_ref().to_owned();
        let (command_sender, command_receiver) = mpsc::sync_channel(COMMAND_QUEUE_LIMIT);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let active = Arc::new(AtomicBool::new(true));
        let command_gate = Arc::new(Mutex::new(()));
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let terminal_error = Arc::new(Mutex::new(None));
        let supervisor_control = SupervisorControl {
            active: Arc::clone(&active),
            command_gate: Arc::clone(&command_gate),
            queued_bytes: Arc::clone(&queued_bytes),
            terminal_error: Arc::clone(&terminal_error),
        };
        thread::Builder::new()
            .name("archetype-runtime-supervisor".to_owned())
            .spawn(move || {
                supervise(
                    &executable,
                    &command_receiver,
                    &ready_sender,
                    &supervisor_control,
                    limits,
                );
            })
            .map_err(|error| RuntimeProcessError::SupervisorStart(error.to_string()))?;
        Ok((
            Self {
                commands: command_sender,
                active,
                command_gate,
                queued_bytes,
                terminal_error,
                limits,
            },
            ready_receiver,
        ))
    }

    #[must_use]
    pub fn render_document(&self, document: StaticDocument) -> RenderReceiver {
        let Ok(_command_gate) = self.command_gate.lock() else {
            return completed(RuntimeProcessError::RuntimeDisconnected);
        };
        if !self.active.load(Ordering::Acquire) {
            return completed(self.inactive_error());
        }
        let mut frame = Vec::new();
        if let Err(error) = Codec::default().encode(&mut frame, &document.protocol_envelope(1)) {
            return completed(RuntimeProcessError::Protocol(error.to_string()));
        }
        let reserved_bytes = frame.len().saturating_add(32);
        if !reserve_bytes(
            &self.queued_bytes,
            reserved_bytes,
            self.limits.maximum_in_flight_bytes,
        ) {
            return completed(RuntimeProcessError::Backpressure);
        }
        let (completion_sender, completion_receiver) = mpsc::sync_channel(1);
        let command = SupervisorCommand::Render {
            document,
            reserved_bytes,
            completion: completion_sender,
        };
        if let Err(error) = self.commands.try_send(command) {
            self.queued_bytes
                .fetch_sub(reserved_bytes, Ordering::AcqRel);
            let failure = match error {
                TrySendError::Full(_) => RuntimeProcessError::Backpressure,
                TrySendError::Disconnected(_) => RuntimeProcessError::RuntimeDisconnected,
            };
            return completed(failure);
        }
        completion_receiver
    }

    #[must_use]
    pub fn shutdown(&self) -> ShutdownReceiver {
        let Ok(_command_gate) = self.command_gate.lock() else {
            return completed(RuntimeProcessError::RuntimeDisconnected);
        };
        if !self.active.load(Ordering::Acquire) {
            return completed(self.inactive_error());
        }
        let (completion_sender, completion_receiver) = mpsc::sync_channel(1);
        if let Err(error) = self
            .commands
            .try_send(SupervisorCommand::Shutdown(completion_sender))
        {
            let failure = match error {
                TrySendError::Full(_) => RuntimeProcessError::Backpressure,
                TrySendError::Disconnected(_) => RuntimeProcessError::RuntimeDisconnected,
            };
            return completed(failure);
        }
        completion_receiver
    }

    /// Force-terminates the current child so the supervisor recovery path can restart it.
    #[must_use]
    pub fn force_restart(&self) -> ShutdownReceiver {
        let Ok(_command_gate) = self.command_gate.lock() else {
            return completed(RuntimeProcessError::RuntimeDisconnected);
        };
        if !self.active.load(Ordering::Acquire) {
            return completed(self.inactive_error());
        }
        let (completion_sender, completion_receiver) = mpsc::sync_channel(1);
        if let Err(error) = self
            .commands
            .try_send(SupervisorCommand::ForceRestart(completion_sender))
        {
            let failure = match error {
                TrySendError::Full(_) => RuntimeProcessError::Backpressure,
                TrySendError::Disconnected(_) => RuntimeProcessError::RuntimeDisconnected,
            };
            return completed(failure);
        }
        completion_receiver
    }

    fn inactive_error(&self) -> RuntimeProcessError {
        self.terminal_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
            .unwrap_or(RuntimeProcessError::RuntimeDisconnected)
    }
}

impl Drop for RuntimeSupervisor {
    fn drop(&mut self) {
        let Ok(_command_gate) = self.command_gate.lock() else {
            return;
        };
        let (completion, _) = mpsc::sync_channel(1);
        let _ = self
            .commands
            .try_send(SupervisorCommand::Shutdown(completion));
    }
}

enum SupervisorCommand {
    Render {
        document: StaticDocument,
        reserved_bytes: usize,
        completion: SyncSender<Result<RuntimeRenderedPage, RuntimeProcessError>>,
    },
    Shutdown(SyncSender<Result<(), RuntimeProcessError>>),
    ForceRestart(SyncSender<Result<(), RuntimeProcessError>>),
}

enum ReaderEvent {
    Authenticated,
    Envelope(Envelope),
    Failed(String),
}

struct PendingRender {
    deadline: Instant,
    page_id: PageId,
    navigation_id: NavigationId,
    broker_diagnostics: Vec<String>,
    image_resources: HashMap<String, Vec<u8>>,
    frame_bytes: usize,
    completion: SyncSender<Result<RuntimeRenderedPage, RuntimeProcessError>>,
}

struct RuntimeConnection {
    input: ChildStdin,
    pending: BTreeMap<u64, PendingRender>,
    next_request_id: u64,
    in_flight_bytes: usize,
    limits: RuntimeLimits,
}

impl RuntimeConnection {
    fn new(input: ChildStdin, limits: RuntimeLimits) -> Self {
        Self {
            input,
            pending: BTreeMap::new(),
            next_request_id: 2,
            in_flight_bytes: 0,
            limits,
        }
    }

    fn queue_render(
        &mut self,
        document: StaticDocument,
        completion: SyncSender<Result<RuntimeRenderedPage, RuntimeProcessError>>,
    ) -> Result<(), RuntimeProcessError> {
        if self.pending.len() >= MAXIMUM_PENDING_REQUESTS {
            let _ = completion.send(Err(RuntimeProcessError::Backpressure));
            return Ok(());
        }
        let request_id = self.next_request_id;
        let Some(following_request_id) = request_id.checked_add(1) else {
            let _ = completion.send(Err(RuntimeProcessError::Backpressure));
            return Ok(());
        };
        let mut frame = Vec::new();
        if let Err(error) =
            Codec::default().encode(&mut frame, &document.protocol_envelope(request_id))
        {
            let _ = completion.send(Err(RuntimeProcessError::Protocol(error.to_string())));
            return Ok(());
        }
        if self.in_flight_bytes.saturating_add(frame.len()) > self.limits.maximum_in_flight_bytes {
            let _ = completion.send(Err(RuntimeProcessError::Backpressure));
            return Ok(());
        }
        if let Err(error) = write_frame(&mut self.input, &frame) {
            let _ = completion.send(Err(error.clone()));
            return Err(error);
        }
        self.next_request_id = following_request_id;
        self.in_flight_bytes += frame.len();
        self.pending.insert(
            request_id,
            PendingRender {
                deadline: Instant::now() + self.limits.request_timeout,
                page_id: document.page_id,
                navigation_id: document.navigation_id,
                broker_diagnostics: document.broker_diagnostics,
                image_resources: document
                    .resources
                    .into_iter()
                    .filter(|resource| resource.kind == ResourceKind::Image)
                    .map(|resource| (resource.requested_url.to_string(), resource.body.into_vec()))
                    .collect(),
                frame_bytes: frame.len(),
                completion,
            },
        );
        Ok(())
    }

    fn drain_responses(
        &mut self,
        responses: &Receiver<ReaderEvent>,
    ) -> Result<(), RuntimeProcessError> {
        drain_responses(responses, &mut self.pending, &mut self.in_flight_bytes)
    }

    fn expire_requests(&mut self) -> bool {
        expire_requests(&mut self.pending, &mut self.in_flight_bytes)
    }

    fn fail_pending(&mut self, error: &RuntimeProcessError) {
        fail_pending(&mut self.pending, &mut self.in_flight_bytes, error);
    }
}

fn supervise(
    executable: &Path,
    commands: &Receiver<SupervisorCommand>,
    ready: &SyncSender<Result<(), RuntimeProcessError>>,
    control: &SupervisorControl,
    limits: RuntimeLimits,
) {
    let mut ready_sent = false;
    let mut restart_index = 0usize;
    loop {
        let (mut child, mut input, responses, reader) = match start_child(executable) {
            Ok(process) => process,
            Err(error) => {
                if !ready_sent {
                    let _ = ready.send(Err(error.clone()));
                }
                finish_supervision(control, commands, &error);
                return;
            }
        };
        if let Err(error) = perform_handshake(&mut input, &responses) {
            terminate(&mut child, reader);
            if !ready_sent {
                let _ = ready.send(Err(error.clone()));
                finish_supervision(control, commands, &error);
                return;
            }
            if !wait_to_restart(&mut restart_index) {
                finish_supervision(control, commands, &error);
                return;
            }
            continue;
        }
        if !ready_sent {
            let _ = ready.send(Ok(()));
            ready_sent = true;
        }

        let mut connection = RuntimeConnection::new(input, limits);
        let mut last_resource_sample = Instant::now();
        let mut terminal_error = RuntimeProcessError::RuntimeDisconnected;
        let mut shutdown = None;
        let mut forced_restart = None;
        let mut command_channel_closed = false;
        loop {
            if let Err(error) = connection.drain_responses(&responses) {
                terminal_error = error.clone();
                connection.fail_pending(&error);
                break;
            }
            if connection.expire_requests() {
                terminal_error = RuntimeProcessError::RequestTimedOut;
                connection.fail_pending(&terminal_error);
                break;
            }
            if last_resource_sample.elapsed() >= RESOURCE_SAMPLE_INTERVAL {
                last_resource_sample = Instant::now();
                if resident_bytes(child.id()).is_some_and(|rss| rss > limits.maximum_rss_bytes) {
                    terminal_error = RuntimeProcessError::ResourceLimit {
                        resource: "RSS".to_owned(),
                    };
                    connection.fail_pending(&terminal_error);
                    break;
                }
            }
            match commands.recv_timeout(SUPERVISOR_TICK) {
                Ok(SupervisorCommand::Render {
                    document,
                    reserved_bytes,
                    completion,
                }) => {
                    control
                        .queued_bytes
                        .fetch_sub(reserved_bytes, Ordering::AcqRel);
                    if let Err(error) = connection.queue_render(document, completion) {
                        terminal_error = error.clone();
                        connection.fail_pending(&error);
                        break;
                    }
                }
                Ok(SupervisorCommand::Shutdown(completion)) => {
                    connection.fail_pending(&RuntimeProcessError::RuntimeDisconnected);
                    shutdown = Some(completion);
                    break;
                }
                Ok(SupervisorCommand::ForceRestart(completion)) => {
                    connection.fail_pending(&RuntimeProcessError::RuntimeDisconnected);
                    forced_restart = Some(completion);
                    break;
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    connection.fail_pending(&RuntimeProcessError::RuntimeDisconnected);
                    command_channel_closed = true;
                    break;
                }
            }
        }
        terminate(&mut child, reader);
        if let Some(completion) = shutdown {
            control.active.store(false, Ordering::Release);
            let _ = completion.send(Ok(()));
            return;
        }
        if command_channel_closed {
            control.active.store(false, Ordering::Release);
            return;
        }
        complete_forced_restart(forced_restart);
        if !wait_to_restart(&mut restart_index) {
            finish_supervision(control, commands, &terminal_error);
            return;
        }
    }
}

fn complete_forced_restart(completion: Option<SyncSender<Result<(), RuntimeProcessError>>>) {
    if let Some(completion) = completion {
        let _ = completion.send(Ok(()));
    }
}

fn wait_to_restart(restart_index: &mut usize) -> bool {
    let Some(delay) = RESTART_BACKOFF.get(*restart_index) else {
        return false;
    };
    thread::sleep(*delay);
    *restart_index += 1;
    true
}

fn finish_supervision(
    control: &SupervisorControl,
    commands: &Receiver<SupervisorCommand>,
    error: &RuntimeProcessError,
) {
    record_terminal_error(&control.terminal_error, error.clone());
    deactivate(control, commands, error);
}

fn start_child(
    executable: &Path,
) -> Result<
    (
        Child,
        ChildStdin,
        Receiver<ReaderEvent>,
        thread::JoinHandle<()>,
    ),
    RuntimeProcessError,
> {
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| RuntimeProcessError::Spawn(error.to_string()))?;
    let mut input = child
        .stdin
        .take()
        .ok_or_else(|| RuntimeProcessError::Spawn("runtime stdin was not piped".to_owned()))?;
    let mut output = child
        .stdout
        .take()
        .ok_or_else(|| RuntimeProcessError::Spawn("runtime stdout was not piped".to_owned()))?;
    let mut launch_token = [0_u8; LAUNCH_TOKEN_BYTES];
    getrandom::fill(&mut launch_token)
        .map_err(|error| RuntimeProcessError::Spawn(error.to_string()))?;
    input
        .write_all(&LAUNCH_AUTH_MAGIC)
        .and_then(|()| input.write_all(&launch_token))
        .and_then(|()| input.flush())
        .map_err(|error| RuntimeProcessError::Protocol(error.to_string()))?;
    let (response_sender, response_receiver) = mpsc::channel();
    let reader = thread::Builder::new()
        .name("archetype-runtime-reader".to_owned())
        .spawn(move || {
            let mut authentication = [0_u8; LAUNCH_AUTH_MAGIC.len() + LAUNCH_TOKEN_BYTES];
            if let Err(error) = output.read_exact(&mut authentication) {
                let _ = response_sender.send(ReaderEvent::Failed(error.to_string()));
                return;
            }
            if authentication[..LAUNCH_AUTH_MAGIC.len()] != LAUNCH_AUTH_MAGIC
                || authentication[LAUNCH_AUTH_MAGIC.len()..] != launch_token
            {
                let _ = response_sender.send(ReaderEvent::Failed(
                    "runtime launch authentication mismatch".to_owned(),
                ));
                return;
            }
            if response_sender.send(ReaderEvent::Authenticated).is_err() {
                return;
            }
            let codec = Codec::default();
            loop {
                match codec.decode(&mut output) {
                    Ok(envelope) => {
                        if response_sender
                            .send(ReaderEvent::Envelope(envelope))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = response_sender.send(ReaderEvent::Failed(error.to_string()));
                        return;
                    }
                }
            }
        })
        .map_err(|error| RuntimeProcessError::SupervisorStart(error.to_string()))?;
    Ok((child, input, response_receiver, reader))
}

fn perform_handshake(
    input: &mut ChildStdin,
    responses: &Receiver<ReaderEvent>,
) -> Result<(), RuntimeProcessError> {
    match responses.recv_timeout(HANDSHAKE_TIMEOUT) {
        Ok(ReaderEvent::Authenticated) => {}
        Ok(ReaderEvent::Failed(error)) => return Err(RuntimeProcessError::Protocol(error)),
        Ok(ReaderEvent::Envelope(_)) => return Err(RuntimeProcessError::UnexpectedResponse),
        Err(RecvTimeoutError::Timeout) => return Err(RuntimeProcessError::HandshakeTimeout),
        Err(RecvTimeoutError::Disconnected) => {
            return Err(RuntimeProcessError::RuntimeDisconnected);
        }
    }
    let capabilities = BTreeSet::from([
        Capability::static_document(),
        Capability::display_list_v1(),
        Capability::cancellable_navigation(),
        Capability::resource_broker_v1(),
    ]);
    write_envelope(
        input,
        &Envelope::v4(
            1,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: PROTOCOL_MINOR,
                maximum_protocol_minor: PROTOCOL_MINOR,
                capabilities: capabilities.clone(),
            }),
        ),
    )?;
    let event = responses
        .recv_timeout(HANDSHAKE_TIMEOUT)
        .map_err(|error| match error {
            RecvTimeoutError::Timeout => RuntimeProcessError::HandshakeTimeout,
            RecvTimeoutError::Disconnected => RuntimeProcessError::RuntimeDisconnected,
        })?;
    match event {
        ReaderEvent::Envelope(envelope) if envelope.request_id() == 1 => match envelope.message() {
            Message::ServerHello(hello)
                if hello.capabilities.contains(&Capability::static_document())
                    && hello.capabilities.contains(&Capability::display_list_v1())
                    && hello
                        .capabilities
                        .contains(&Capability::resource_broker_v1()) =>
            {
                Ok(())
            }
            Message::ServerHello(_) => Err(RuntimeProcessError::MissingCapabilities),
            Message::Rejected(rejection) => Err(RuntimeProcessError::HandshakeRejected(
                rejection.message.clone(),
            )),
            _ => Err(RuntimeProcessError::UnexpectedResponse),
        },
        ReaderEvent::Authenticated | ReaderEvent::Envelope(_) => {
            Err(RuntimeProcessError::UnexpectedResponse)
        }
        ReaderEvent::Failed(error) => Err(RuntimeProcessError::Protocol(error)),
    }
}

fn write_envelope(input: &mut ChildStdin, envelope: &Envelope) -> Result<(), RuntimeProcessError> {
    let mut frame = Vec::new();
    Codec::default()
        .encode(&mut frame, envelope)
        .map_err(|error| RuntimeProcessError::Protocol(error.to_string()))?;
    write_frame(input, &frame)
}

fn write_frame(input: &mut ChildStdin, frame: &[u8]) -> Result<(), RuntimeProcessError> {
    input
        .write_all(frame)
        .and_then(|()| input.flush())
        .map_err(|error| RuntimeProcessError::Protocol(error.to_string()))
}

fn drain_responses(
    responses: &Receiver<ReaderEvent>,
    pending: &mut BTreeMap<u64, PendingRender>,
    in_flight_bytes: &mut usize,
) -> Result<(), RuntimeProcessError> {
    for event in responses.try_iter() {
        let envelope = match event {
            ReaderEvent::Envelope(envelope) => envelope,
            ReaderEvent::Authenticated => return Err(RuntimeProcessError::UnexpectedResponse),
            ReaderEvent::Failed(error) => return Err(RuntimeProcessError::Protocol(error)),
        };
        let Some(request) = pending.remove(&envelope.request_id()) else {
            continue;
        };
        *in_flight_bytes = in_flight_bytes.saturating_sub(request.frame_bytes);
        let result = match envelope.into_message() {
            Message::Response(Response::Rendered {
                page_id,
                navigation_id,
                final_url,
                title,
                display_list,
                mut diagnostics,
            }) if page_id == request.page_id && navigation_id == request.navigation_id => {
                diagnostics.splice(0..0, request.broker_diagnostics);
                Ok(RuntimeRenderedPage {
                    page_id,
                    navigation_id,
                    final_url,
                    title,
                    display_list,
                    diagnostics,
                    image_resources: request.image_resources,
                })
            }
            Message::Response(Response::Failed { code, message }) => {
                Err(RuntimeProcessError::RuntimeFailure { code, message })
            }
            _ => Err(RuntimeProcessError::UnexpectedResponse),
        };
        let _ = request.completion.send(result);
    }
    Ok(())
}

fn expire_requests(
    pending: &mut BTreeMap<u64, PendingRender>,
    in_flight_bytes: &mut usize,
) -> bool {
    let now = Instant::now();
    let expired: Vec<_> = pending
        .iter()
        .filter_map(|(request_id, request)| (now >= request.deadline).then_some(*request_id))
        .collect();
    for request_id in &expired {
        if let Some(request) = pending.remove(request_id) {
            *in_flight_bytes = in_flight_bytes.saturating_sub(request.frame_bytes);
            let _ = request
                .completion
                .send(Err(RuntimeProcessError::RequestTimedOut));
        }
    }
    !expired.is_empty()
}

fn fail_pending(
    pending: &mut BTreeMap<u64, PendingRender>,
    in_flight_bytes: &mut usize,
    error: &RuntimeProcessError,
) {
    for (_, request) in std::mem::take(pending) {
        let _ = request.completion.send(Err(error.clone()));
    }
    *in_flight_bytes = 0;
}

fn terminate(child: &mut Child, reader: thread::JoinHandle<()>) {
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
}

fn completed<T>(error: RuntimeProcessError) -> Receiver<Result<T, RuntimeProcessError>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let _ = sender.send(Err(error));
    receiver
}

fn record_terminal_error(
    terminal_error: &Mutex<Option<RuntimeProcessError>>,
    error: RuntimeProcessError,
) {
    if let Ok(mut terminal_error) = terminal_error.lock() {
        *terminal_error = Some(error);
    }
}

fn deactivate(
    control: &SupervisorControl,
    commands: &Receiver<SupervisorCommand>,
    error: &RuntimeProcessError,
) {
    let Ok(_command_gate) = control.command_gate.lock() else {
        control.active.store(false, Ordering::Release);
        return;
    };
    control.active.store(false, Ordering::Release);
    for command in commands.try_iter() {
        match command {
            SupervisorCommand::Render {
                reserved_bytes,
                completion,
                ..
            } => {
                control
                    .queued_bytes
                    .fetch_sub(reserved_bytes, Ordering::AcqRel);
                let _ = completion.send(Err(error.clone()));
            }
            SupervisorCommand::Shutdown(completion)
            | SupervisorCommand::ForceRestart(completion) => {
                let _ = completion.send(Err(error.clone()));
            }
        }
    }
}

fn reserve_bytes(queued: &AtomicUsize, bytes: usize, maximum: usize) -> bool {
    let mut current = queued.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(bytes) else {
            return false;
        };
        if next > maximum {
            return false;
        }
        match queued.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return true,
            Err(actual) => current = actual,
        }
    }
}

fn resident_bytes(process_id: u32) -> Option<u64> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &process_id.to_string()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let kibibytes = std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()?;
    kibibytes.checked_mul(1024)
}

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    io::{Read, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
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
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_QUEUE_LIMIT: usize = 256;
const MAXIMUM_PENDING_REQUESTS: usize = 64;
const SUPERVISOR_TICK: Duration = Duration::from_millis(10);

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
    pub(crate) fn protocol_envelope(&self, request_id: u64) -> Envelope {
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
}

pub type ReadyReceiver = Receiver<Result<(), RuntimeProcessError>>;
pub type RenderReceiver = Receiver<Result<RuntimeRenderedPage, RuntimeProcessError>>;
pub type ShutdownReceiver = Receiver<Result<(), RuntimeProcessError>>;

pub struct RuntimeSupervisor {
    commands: SyncSender<SupervisorCommand>,
    active: Arc<AtomicBool>,
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
        let executable = executable.as_ref().to_owned();
        let (command_sender, command_receiver) = mpsc::sync_channel(COMMAND_QUEUE_LIMIT);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let active = Arc::new(AtomicBool::new(true));
        let supervisor_active = Arc::clone(&active);
        thread::Builder::new()
            .name("archetype-runtime-supervisor".to_owned())
            .spawn(move || {
                supervise(
                    &executable,
                    &command_receiver,
                    &ready_sender,
                    &supervisor_active,
                );
            })
            .map_err(|error| RuntimeProcessError::SupervisorStart(error.to_string()))?;
        Ok((
            Self {
                commands: command_sender,
                active,
            },
            ready_receiver,
        ))
    }

    #[must_use]
    pub fn render_document(&self, document: StaticDocument) -> RenderReceiver {
        if !self.active.load(Ordering::Acquire) {
            return completed(RuntimeProcessError::RuntimeDisconnected);
        }
        let (completion_sender, completion_receiver) = mpsc::sync_channel(1);
        let command = SupervisorCommand::Render {
            document,
            completion: completion_sender,
        };
        if let Err(error) = self.commands.try_send(command) {
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
        if !self.active.load(Ordering::Acquire) {
            return completed(RuntimeProcessError::RuntimeDisconnected);
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
}

impl Drop for RuntimeSupervisor {
    fn drop(&mut self) {
        let (completion, _) = mpsc::sync_channel(1);
        let _ = self
            .commands
            .try_send(SupervisorCommand::Shutdown(completion));
    }
}

enum SupervisorCommand {
    Render {
        document: StaticDocument,
        completion: SyncSender<Result<RuntimeRenderedPage, RuntimeProcessError>>,
    },
    Shutdown(SyncSender<Result<(), RuntimeProcessError>>),
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
    completion: SyncSender<Result<RuntimeRenderedPage, RuntimeProcessError>>,
}

fn supervise(
    executable: &Path,
    commands: &Receiver<SupervisorCommand>,
    ready: &SyncSender<Result<(), RuntimeProcessError>>,
    active: &AtomicBool,
) {
    let (mut child, mut input, responses, reader) = match start_child(executable) {
        Ok(process) => process,
        Err(error) => {
            active.store(false, Ordering::Release);
            let _ = ready.send(Err(error));
            return;
        }
    };
    if let Err(error) = perform_handshake(&mut input, &responses) {
        active.store(false, Ordering::Release);
        let _ = ready.send(Err(error));
        terminate(&mut child, reader);
        return;
    }
    let _ = ready.send(Ok(()));

    let mut next_request_id = 2_u64;
    let mut pending = BTreeMap::new();
    loop {
        if !drain_responses(&responses, &mut pending) {
            fail_pending(&mut pending, &RuntimeProcessError::RuntimeDisconnected);
            break;
        }
        expire_requests(&mut pending);
        match commands.recv_timeout(SUPERVISOR_TICK) {
            Ok(SupervisorCommand::Render {
                document,
                completion,
            }) => {
                if pending.len() >= MAXIMUM_PENDING_REQUESTS {
                    let _ = completion.send(Err(RuntimeProcessError::Backpressure));
                    continue;
                }
                let request_id = next_request_id;
                let Some(following_request_id) = next_request_id.checked_add(1) else {
                    let _ = completion.send(Err(RuntimeProcessError::Backpressure));
                    continue;
                };
                let envelope = document.protocol_envelope(request_id);
                if let Err(error) = write_envelope(&mut input, &envelope) {
                    let _ = completion.send(Err(error.clone()));
                    fail_pending(&mut pending, &error);
                    break;
                }
                next_request_id = following_request_id;
                pending.insert(
                    request_id,
                    PendingRender {
                        deadline: Instant::now() + REQUEST_TIMEOUT,
                        page_id: document.page_id,
                        navigation_id: document.navigation_id,
                        broker_diagnostics: document.broker_diagnostics,
                        image_resources: document
                            .resources
                            .into_iter()
                            .filter(|resource| resource.kind == ResourceKind::Image)
                            .map(|resource| {
                                (resource.requested_url.to_string(), resource.body.into_vec())
                            })
                            .collect(),
                        completion,
                    },
                );
            }
            Ok(SupervisorCommand::Shutdown(completion)) => {
                active.store(false, Ordering::Release);
                fail_pending(&mut pending, &RuntimeProcessError::RuntimeDisconnected);
                terminate(&mut child, reader);
                let _ = completion.send(Ok(()));
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                active.store(false, Ordering::Release);
                fail_pending(&mut pending, &RuntimeProcessError::RuntimeDisconnected);
                terminate(&mut child, reader);
                return;
            }
        }
    }
    active.store(false, Ordering::Release);
    terminate(&mut child, reader);
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
    Codec::default()
        .encode(&mut *input, envelope)
        .map_err(|error| RuntimeProcessError::Protocol(error.to_string()))?;
    input
        .flush()
        .map_err(|error| RuntimeProcessError::Protocol(error.to_string()))
}

fn drain_responses(
    responses: &Receiver<ReaderEvent>,
    pending: &mut BTreeMap<u64, PendingRender>,
) -> bool {
    for event in responses.try_iter() {
        let envelope = match event {
            ReaderEvent::Envelope(envelope) => envelope,
            ReaderEvent::Authenticated | ReaderEvent::Failed(_) => return false,
        };
        let Some(request) = pending.remove(&envelope.request_id()) else {
            continue;
        };
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
    true
}

fn expire_requests(pending: &mut BTreeMap<u64, PendingRender>) {
    let now = Instant::now();
    let expired: Vec<_> = pending
        .iter()
        .filter_map(|(request_id, request)| (now >= request.deadline).then_some(*request_id))
        .collect();
    for request_id in expired {
        if let Some(request) = pending.remove(&request_id) {
            let _ = request
                .completion
                .send(Err(RuntimeProcessError::RequestTimedOut));
        }
    }
}

fn fail_pending(pending: &mut BTreeMap<u64, PendingRender>, error: &RuntimeProcessError) {
    for (_, request) in std::mem::take(pending) {
        let _ = request.completion.send(Err(error.clone()));
    }
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

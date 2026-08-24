use std::{
    collections::VecDeque,
    fs,
    io::Read as _,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use archetype_protocol::{BrokeredResource, ResourceBytes};
use archetype_raster::Rasterizer;
use archetype_types::{ArchetypeUrl, NavigationId, PageId};
use image::RgbaImage;
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

use crate::{
    SdkFuture,
    runtime_client::{
        RuntimeProcessError, RuntimeRenderedPage, RuntimeSupervisor,
        StaticDocument as RuntimeDocument,
    },
};

const DOCUMENT_BYTE_LIMIT: usize = 4 * 1024 * 1024;
const RESOURCE_BYTE_LIMIT: usize = 4 * 1024 * 1024;
const TOTAL_RESOURCE_BYTE_LIMIT: usize = 8 * 1024 * 1024;
const EVENT_QUEUE_LIMIT: usize = 64;
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(6);

/// Errors returned by the public SDK boundary.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum SdkError {
    #[error("invalid SDK configuration: {0}")]
    Configuration(String),
    #[error("Runtime integrity check failed: {0}")]
    Integrity(String),
    #[error("SDK input exceeds a resource limit: {0}")]
    Limit(String),
    #[error("Runtime protocol failed: {0}")]
    Protocol(String),
    #[error("Runtime failed: {0}")]
    Runtime(String),
    #[error("Runtime disconnected")]
    Disconnected,
    #[error("navigation result is stale")]
    StaleNavigation,
    #[error("frame operation failed: {0}")]
    Frame(String),
    #[error("I/O failed: {0}")]
    Io(String),
}

/// Runtime-backed rendering engine builder.
#[derive(Clone, Debug, Default)]
pub struct EngineBuilder {
    runtime_path: Option<PathBuf>,
    expected_sha256: Option<[u8; 32]>,
}

impl EngineBuilder {
    /// Uses an explicit Runtime executable instead of same-directory discovery.
    #[must_use]
    pub fn runtime_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.runtime_path = Some(path.into());
        self
    }

    /// Requires the Runtime executable to match a lowercase or uppercase SHA-256 digest.
    ///
    /// # Errors
    /// Returns [`SdkError::Configuration`] when `digest` is not 64 hexadecimal digits.
    pub fn expected_runtime_sha256(mut self, digest: &str) -> Result<Self, SdkError> {
        self.expected_sha256 = Some(parse_sha256(digest)?);
        Ok(self)
    }

    /// Starts and authenticates the configured Runtime on a worker thread.
    #[must_use]
    pub fn build(self) -> SdkFuture<Result<Engine, SdkError>> {
        SdkFuture::spawn(
            "archetype-sdk-build",
            move || build_engine(self),
            |error| Err(SdkError::Io(error.to_string())),
        )
    }
}

/// A connected Archetype Runtime instance.
pub struct Engine {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    runtime: Arc<RuntimeSupervisor>,
    stopped: AtomicBool,
}

impl Engine {
    /// Creates an SDK builder with same-directory Runtime discovery.
    #[must_use]
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    /// Creates a page with its own stable ID, viewport and event queue.
    #[must_use]
    pub fn create_page(&self, options: PageOptions) -> SdkFuture<Result<Page, SdkError>> {
        let result = options.validate().map(|()| Page {
            id: PageId::new(),
            options,
            engine: Arc::clone(&self.inner),
            navigation_id: Arc::new(Mutex::new(NavigationId::zero())),
            events: Arc::new(EventQueue::default()),
        });
        SdkFuture::ready(result)
    }

    /// Requests graceful Runtime shutdown on a worker thread.
    #[must_use]
    pub fn shutdown(&self) -> SdkFuture<Result<(), SdkError>> {
        if self.inner.stopped.swap(true, Ordering::AcqRel) {
            return SdkFuture::ready(Ok(()));
        }
        let runtime = Arc::clone(&self.inner.runtime);
        SdkFuture::spawn(
            "archetype-sdk-shutdown",
            move || {
                runtime
                    .shutdown()
                    .recv_timeout(COMPLETION_TIMEOUT)
                    .map_err(|_| SdkError::Disconnected)?
                    .map_err(map_runtime_error)
            },
            |error| Err(SdkError::Io(error.to_string())),
        )
    }

    /// Forces the child restart path for automated failure testing.
    #[doc(hidden)]
    #[must_use]
    pub fn force_restart_for_testing(&self) -> SdkFuture<Result<(), SdkError>> {
        let runtime = Arc::clone(&self.inner.runtime);
        SdkFuture::spawn(
            "archetype-sdk-force-restart",
            move || {
                runtime
                    .force_restart()
                    .recv_timeout(COMPLETION_TIMEOUT)
                    .map_err(|_| SdkError::Disconnected)?
                    .map_err(map_runtime_error)
            },
            |error| Err(SdkError::Io(error.to_string())),
        )
    }
}

/// Fixed page viewport for the V5 developer preview.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageOptions {
    width_px: u32,
    height_px: u32,
}

impl PageOptions {
    #[must_use]
    pub const fn new(width_px: u32, height_px: u32) -> Self {
        Self {
            width_px,
            height_px,
        }
    }

    #[must_use]
    pub const fn width_px(self) -> u32 {
        self.width_px
    }

    #[must_use]
    pub const fn height_px(self) -> u32 {
        self.height_px
    }

    fn validate(self) -> Result<(), SdkError> {
        if self.width_px == 0
            || self.height_px == 0
            || self.width_px > u32::from(u16::MAX)
            || self.height_px > u32::from(u16::MAX)
        {
            return Err(SdkError::Configuration(
                "viewport dimensions must be between 1 and 65535".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Resource type attached to a static document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ResourceKind {
    Stylesheet,
    Image,
}

/// Caller-supplied same-origin bytes for a static document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    url: ArchetypeUrl,
    kind: ResourceKind,
    bytes: Vec<u8>,
}

impl Resource {
    /// Creates a validated resource.
    ///
    /// # Errors
    /// Returns a structured error for invalid URLs or resources over 4 MiB.
    pub fn new(url: &str, kind: ResourceKind, bytes: Vec<u8>) -> Result<Self, SdkError> {
        if bytes.len() > RESOURCE_BYTE_LIMIT {
            return Err(SdkError::Limit("resource exceeds 4 MiB".to_owned()));
        }
        let url = url
            .parse()
            .map_err(|_| SdkError::Configuration("resource URL is invalid".to_owned()))?;
        Ok(Self { url, kind, bytes })
    }
}

/// UTF-8 HTML and caller-brokered same-origin resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticDocument {
    url: ArchetypeUrl,
    html: String,
    resources: Vec<Resource>,
}

impl StaticDocument {
    /// Creates a static HTTP(S) document under the V5 document-size limit.
    ///
    /// # Errors
    /// Returns a structured error for invalid schemes, URLs or HTML over 4 MiB.
    pub fn new(url: &str, html: impl Into<String>) -> Result<Self, SdkError> {
        let parsed = Url::parse(url)
            .map_err(|_| SdkError::Configuration("document URL is invalid".to_owned()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(SdkError::Configuration(
                "SDK documents require an HTTP(S) URL".to_owned(),
            ));
        }
        let html = html.into();
        if html.len() > DOCUMENT_BYTE_LIMIT {
            return Err(SdkError::Limit("document exceeds 4 MiB".to_owned()));
        }
        Ok(Self {
            url: url
                .parse()
                .map_err(|_| SdkError::Configuration("document URL is invalid".to_owned()))?,
            html,
            resources: Vec::new(),
        })
    }

    /// Adds one same-origin resource and checks the 8 MiB aggregate budget.
    ///
    /// # Errors
    /// Returns a structured error for cross-origin or over-budget resources.
    pub fn add_resource(&mut self, resource: Resource) -> Result<(), SdkError> {
        let document_url = Url::parse(self.url.as_str())
            .map_err(|_| SdkError::Configuration("document URL is invalid".to_owned()))?;
        let resource_url = Url::parse(resource.url.as_str())
            .map_err(|_| SdkError::Configuration("resource URL is invalid".to_owned()))?;
        if !same_origin(&document_url, &resource_url) {
            return Err(SdkError::Configuration(
                "resource must share the document origin".to_owned(),
            ));
        }
        let total = self
            .resources
            .iter()
            .map(|resource| resource.bytes.len())
            .sum::<usize>()
            .saturating_add(resource.bytes.len());
        if total > TOTAL_RESOURCE_BYTE_LIMIT {
            return Err(SdkError::Limit(
                "document resources exceed 8 MiB".to_owned(),
            ));
        }
        self.resources.push(resource);
        Ok(())
    }
}

/// A page with independent navigation identity and events.
#[derive(Clone)]
pub struct Page {
    id: PageId,
    options: PageOptions,
    engine: Arc<EngineInner>,
    navigation_id: Arc<Mutex<NavigationId>>,
    events: Arc<EventQueue>,
}

impl Page {
    #[must_use]
    pub fn id(&self) -> &PageId {
        &self.id
    }

    /// Renders one static document through Runtime and rasterizes an RGBA frame.
    #[must_use]
    pub fn render(&self, document: StaticDocument) -> SdkFuture<Result<Navigation, SdkError>> {
        if self.engine.stopped.load(Ordering::Acquire) {
            self.events.push(PageEvent::RuntimeDisconnected);
            return SdkFuture::ready(Err(SdkError::Disconnected));
        }
        let navigation_id = match self.navigation_id.lock() {
            Ok(mut current) => {
                *current = current.saturating_next();
                *current
            }
            Err(_) => {
                return SdkFuture::ready(Err(SdkError::Runtime("page state failed".to_owned())));
            }
        };
        self.events
            .push(PageEvent::NavigationStarted { navigation_id });
        let runtime = Arc::clone(&self.engine.runtime);
        let page_id = self.id.clone();
        let options = self.options;
        let current_navigation = Arc::clone(&self.navigation_id);
        let events = Arc::clone(&self.events);
        SdkFuture::spawn(
            "archetype-sdk-render",
            move || {
                let result = render_page(
                    &runtime,
                    page_id,
                    navigation_id,
                    options,
                    document,
                    &current_navigation,
                );
                match &result {
                    Ok(navigation) => events.push(PageEvent::FrameReady {
                        navigation_id,
                        frame: navigation.frame.clone(),
                    }),
                    Err(SdkError::Disconnected) => events.push(PageEvent::RuntimeDisconnected),
                    Err(error) => events.push(PageEvent::Failed {
                        navigation_id,
                        error: error.clone(),
                    }),
                }
                result
            },
            |error| Err(SdkError::Io(error.to_string())),
        )
    }

    /// Waits asynchronously for the next page event.
    #[must_use]
    pub fn next_event(&self) -> SdkFuture<Result<PageEvent, SdkError>> {
        let events = Arc::clone(&self.events);
        SdkFuture::spawn(
            "archetype-sdk-event",
            move || Ok(events.pop()),
            |error| Err(SdkError::Io(error.to_string())),
        )
    }
}

/// Completed static navigation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Navigation {
    id: NavigationId,
    final_url: ArchetypeUrl,
    title: String,
    frame: Frame,
    diagnostics: Vec<String>,
}

impl Navigation {
    #[must_use]
    pub const fn navigation_id(&self) -> NavigationId {
        self.id
    }

    #[must_use]
    pub fn final_url(&self) -> &ArchetypeUrl {
        &self.final_url
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

/// Owned, tightly packed RGBA8 frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    width_px: u32,
    height_px: u32,
    stride_bytes: u32,
    rgba: Vec<u8>,
}

impl Frame {
    #[must_use]
    pub const fn width_px(&self) -> u32 {
        self.width_px
    }

    #[must_use]
    pub const fn height_px(&self) -> u32 {
        self.height_px
    }

    #[must_use]
    pub const fn stride_bytes(&self) -> u32 {
        self.stride_bytes
    }

    #[must_use]
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Encodes the frame as PNG.
    ///
    /// # Errors
    /// Returns an error when the frame invariant or PNG write fails.
    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), SdkError> {
        let image = RgbaImage::from_raw(self.width_px, self.height_px, self.rgba.clone())
            .ok_or_else(|| SdkError::Frame("invalid RGBA frame length".to_owned()))?;
        image
            .save(path)
            .map_err(|error| SdkError::Frame(error.to_string()))
    }
}

/// Ordered page lifecycle event.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PageEvent {
    NavigationStarted {
        navigation_id: NavigationId,
    },
    FrameReady {
        navigation_id: NavigationId,
        frame: Frame,
    },
    RuntimeDisconnected,
    Failed {
        navigation_id: NavigationId,
        error: SdkError,
    },
}

#[derive(Default)]
struct EventQueue {
    queue: Mutex<VecDeque<PageEvent>>,
    ready: Condvar,
}

impl EventQueue {
    fn push(&self, event: PageEvent) {
        if let Ok(mut queue) = self.queue.lock() {
            if queue.len() >= EVENT_QUEUE_LIMIT {
                let removable = queue.iter().position(|event| {
                    matches!(
                        event,
                        PageEvent::NavigationStarted { .. } | PageEvent::FrameReady { .. }
                    )
                });
                if let Some(index) = removable {
                    queue.remove(index);
                }
            }
            queue.push_back(event);
            self.ready.notify_one();
        }
    }

    fn pop(&self) -> PageEvent {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if let Some(event) = queue.pop_front() {
                return event;
            }
            queue = self
                .ready
                .wait(queue)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

fn build_engine(builder: EngineBuilder) -> Result<Engine, SdkError> {
    let runtime_path = match builder.runtime_path {
        Some(path) => path,
        None => sibling_runtime(
            &std::env::current_exe().map_err(|error| SdkError::Io(error.to_string()))?,
        )?,
    };
    validate_runtime(&runtime_path, builder.expected_sha256)?;
    let (runtime, ready) = RuntimeSupervisor::spawn(&runtime_path).map_err(map_runtime_error)?;
    ready
        .recv_timeout(COMPLETION_TIMEOUT)
        .map_err(|_| SdkError::Disconnected)?
        .map_err(map_runtime_error)?;
    Ok(Engine {
        inner: Arc::new(EngineInner {
            runtime: Arc::new(runtime),
            stopped: AtomicBool::new(false),
        }),
    })
}

fn validate_runtime(path: &Path, expected_sha256: Option<[u8; 32]>) -> Result<(), SdkError> {
    let metadata = fs::metadata(path).map_err(|error| SdkError::Io(error.to_string()))?;
    if !metadata.is_file() {
        return Err(SdkError::Configuration(
            "Runtime path is not a regular file".to_owned(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(SdkError::Configuration(
                "Runtime file is not executable".to_owned(),
            ));
        }
    }
    if let Some(expected) = expected_sha256 {
        let mut file = fs::File::open(path).map_err(|error| SdkError::Io(error.to_string()))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
        loop {
            let length = file
                .read(&mut buffer)
                .map_err(|error| SdkError::Io(error.to_string()))?;
            if length == 0 {
                break;
            }
            hasher.update(&buffer[..length]);
        }
        let actual: [u8; 32] = hasher.finalize().into();
        if !constant_time_eq(&actual, &expected) {
            return Err(SdkError::Integrity(
                "Runtime SHA-256 does not match".to_owned(),
            ));
        }
    }
    Ok(())
}

fn sibling_runtime(current_executable: &Path) -> Result<PathBuf, SdkError> {
    current_executable
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(|parent| parent.join("archetype-runtime"))
        .ok_or_else(|| SdkError::Configuration("SDK executable has no parent directory".to_owned()))
}

fn render_page(
    runtime: &RuntimeSupervisor,
    page_id: PageId,
    navigation_id: NavigationId,
    options: PageOptions,
    document: StaticDocument,
    current_navigation: &Mutex<NavigationId>,
) -> Result<Navigation, SdkError> {
    let runtime_document = RuntimeDocument {
        page_id,
        navigation_id,
        url: document.url,
        html: document.html,
        viewport_width_px: options.width_px,
        resources: document
            .resources
            .into_iter()
            .map(|resource| BrokeredResource {
                requested_url: resource.url.clone(),
                final_url: resource.url,
                kind: match resource.kind {
                    ResourceKind::Stylesheet => archetype_protocol::ResourceKind::Stylesheet,
                    ResourceKind::Image => archetype_protocol::ResourceKind::Image,
                },
                body: ResourceBytes::new(resource.bytes),
            })
            .collect(),
        broker_diagnostics: Vec::new(),
    };
    let rendered = runtime
        .render_document(runtime_document)
        .recv_timeout(COMPLETION_TIMEOUT)
        .map_err(|_| SdkError::Disconnected)?
        .map_err(map_runtime_error)?;
    if current_navigation
        .lock()
        .map_or(true, |current| *current != navigation_id)
    {
        return Err(SdkError::StaleNavigation);
    }
    navigation_from_rendered(rendered, options)
}

fn navigation_from_rendered(
    rendered: RuntimeRenderedPage,
    options: PageOptions,
) -> Result<Navigation, SdkError> {
    let image = Rasterizer::default().render(
        options.width_px,
        options.height_px,
        &rendered.display_list,
        &rendered.image_resources,
    );
    let stride_bytes = options
        .width_px
        .checked_mul(4)
        .ok_or_else(|| SdkError::Frame("frame stride overflow".to_owned()))?;
    Ok(Navigation {
        id: rendered.navigation_id,
        final_url: rendered.final_url,
        title: rendered.title,
        frame: Frame {
            width_px: options.width_px,
            height_px: options.height_px,
            stride_bytes,
            rgba: image.into_raw(),
        },
        diagnostics: rendered.diagnostics,
    })
}

fn map_runtime_error(error: RuntimeProcessError) -> SdkError {
    match error {
        RuntimeProcessError::RuntimeDisconnected => SdkError::Disconnected,
        RuntimeProcessError::Protocol(message) => SdkError::Protocol(message),
        RuntimeProcessError::Backpressure => SdkError::Limit("Runtime backpressure".to_owned()),
        RuntimeProcessError::ResourceLimit { resource } => {
            SdkError::Limit(format!("Runtime exceeded {resource}"))
        }
        error => SdkError::Runtime(error.to_string()),
    }
}

fn parse_sha256(value: &str) -> Result<[u8; 32], SdkError> {
    if value.len() != 64 {
        return Err(SdkError::Configuration(
            "Runtime SHA-256 must contain 64 hexadecimal digits".to_owned(),
        ));
    }
    let mut output = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let chunk = std::str::from_utf8(chunk)
            .map_err(|_| SdkError::Configuration("Runtime SHA-256 is invalid".to_owned()))?;
        output[index] = u8::from_str_radix(chunk, 16)
            .map_err(|_| SdkError::Configuration("Runtime SHA-256 is invalid".to_owned()))?;
    }
    Ok(output)
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_documents_resources_and_viewports() {
        assert!(StaticDocument::new("file:///private/page.html", "<p>no</p>").is_err());
        assert!(PageOptions::new(0, 800).validate().is_err());
        let mut document = StaticDocument::new("https://example.test/page", "<p>ok</p>").unwrap();
        let cross_origin = Resource::new(
            "https://assets.example.test/style.css",
            ResourceKind::Stylesheet,
            b"p {}".to_vec(),
        )
        .unwrap();
        assert!(document.add_resource(cross_origin).is_err());
        assert!(
            Resource::new(
                "https://example.test/large.png",
                ResourceKind::Image,
                vec![0; RESOURCE_BYTE_LIMIT + 1]
            )
            .is_err()
        );
    }

    #[test]
    fn parses_and_compares_runtime_digests() {
        let digest = parse_sha256(&"ab".repeat(32)).unwrap();
        assert_eq!(digest, [0xab; 32]);
        assert!(constant_time_eq(&digest, &[0xab; 32]));
        assert!(!constant_time_eq(&digest, &[0xac; 32]));
        assert!(parse_sha256("not-a-digest").is_err());
    }

    #[test]
    fn event_queue_stays_bounded_and_preserves_disconnects() {
        let queue = EventQueue::default();
        for index in 0..EVENT_QUEUE_LIMIT + 8 {
            queue.push(PageEvent::NavigationStarted {
                navigation_id: (0..=index).fold(NavigationId::zero(), |id, _| id.saturating_next()),
            });
        }
        queue.push(PageEvent::RuntimeDisconnected);

        let events = queue.queue.lock().unwrap();
        assert_eq!(events.len(), EVENT_QUEUE_LIMIT);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, PageEvent::RuntimeDisconnected))
        );
    }

    #[test]
    fn discovers_only_a_same_directory_runtime() {
        assert_eq!(
            sibling_runtime(Path::new(
                "/Applications/Partner.app/Contents/MacOS/partner"
            ))
            .unwrap(),
            Path::new("/Applications/Partner.app/Contents/MacOS/archetype-runtime")
        );
        assert!(sibling_runtime(Path::new("partner")).is_err());
    }
}

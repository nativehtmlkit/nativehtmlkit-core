//! Host controller. The GUI thread owns every window and policy transition.
//! Workers can only send bounded messages back to this controller.
mod bitmap;
#[cfg(test)]
mod document_integration_tests;
mod document_native;
mod documents;
mod native;
use documents::HostedDocument;
mod placement;
mod requests;

use crate::{
    accessibility::{ActionEvent, NativeAccessibility},
    backend::Backend,
    config::{AppConfig, AppInfo, BackendConfig, HostConfig, InstancePolicy},
    lifecycle::{DecisionKind, DecisionTracker, Lifecycle, Phase},
    platform::{self, InstanceLock},
    presentation::Probe,
    protocol::{self, RpcError},
    transport::{self, Server, TransportEvent},
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use softbuffer::Surface;
use std::collections::{BTreeMap, HashMap};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};
use winit::{
    event_loop::{ActiveEventLoop, AsyncRequestSerial, EventLoop, OwnedDisplayHandle},
    keyboard::ModifiersState,
    window::{Window, WindowId},
};

type Wake = Arc<dyn Fn() + Send + Sync>;
type Reply = (u64, String);

/// Upper bound on requests whose trace start time is retained (trace level only). It is
/// above the pending-request limit; a request beyond it is still traced, without timing.
const RPC_TRACE_LIMIT: usize = 2 * protocol::MAX_PENDING;

struct NativeWindow {
    // Adapter must be removed before the underlying HWND is destroyed.
    accessibility: NativeAccessibility,
    surface: Surface<OwnedDisplayHandle, Arc<dyn Window>>,
    window: Arc<dyn Window>,
    modifiers: ModifiersState,
    ime_enabled: bool,
    drag_selecting: bool,
    /// Whether a frame has been presented (logged once for startup timing).
    presented: bool,
    /// NHK-CROSS-0006: until this deadline the new window is neither shown (Windows) nor given a
    /// first buffer (Wayland maps on the first commit), unless a document is loaded first. `None`
    /// once revealed. See [`reveal_due`].
    reveal_deadline: Option<Instant>,
    /// When the compositor last resized the surface and no frame of that
    /// size has been presented yet (debug-level resize-to-present timing).
    resized_at: Option<Instant>,
    pointer: (f64, f64),
    last_titlebar_press: Option<(Instant, f64, f64)>,
    document_epoch: Option<String>,
    composing: bool,
    ime_resets: u64,
    ime_quiet_until: Option<Instant>,
    preedit_cleared: bool,
    /// The IME state last sent while enabled. An unchanged state is not sent again: input
    /// methods answer every text-input update (for example by re-sending a pending dead-key
    /// preedit), and re-sending on each redraw turned that into an unbounded IME/redraw loop.
    ime_sent: Option<ImeState>,
    incoming_drop: Option<IncomingDrop>,
}

/// CORE-A-13: notifications the host drops (and counts in `host.diagnostics`
/// `dropped_notifications`) instead of failing the session while the backend does not read.
/// Replies, host requests and lifecycle notifications are never dropped.
const DROPPABLE_NOTIFICATIONS: [&str; 2] = ["window.event", "document.event"];

/// NHK-CROSS-0006: how long a new window waits for its first `document.load` before the
/// built-in placeholder page is presented instead.
pub(super) const FIRST_PRESENT_GRACE: Duration = Duration::from_millis(750);

/// Whether a window still waiting for its first presentation must be revealed now: its
/// document is loaded, the grace period has elapsed, or the application failed (the failure
/// page must be visible at once).
fn reveal_due(deadline: Option<Instant>, has_document: bool, failed: bool, now: Instant) -> bool {
    deadline.is_some_and(|deadline| has_document || failed || now >= deadline)
}

#[derive(Clone)]
struct IncomingDrop {
    transfer: winit::data_transfer::DataTransferId,
    document_id: Option<String>,
    target: Option<Value>,
    /// None means rejected, true selects local-file URIs, false selects text.
    files: Option<bool>,
    point: (f64, f64),
}

/// IME-visible state of the focused editor: surrounding text, cursor and anchor (bytes), the
/// cursor area in physical pixels and whether the field is a password.
#[derive(Clone, Debug, PartialEq)]
struct ImeState {
    text: String,
    cursor: usize,
    anchor: usize,
    area: (i64, i64, i64, i64),
    password: bool,
    hints: Value,
}

impl ImeState {
    fn new(
        text: &str,
        cursor: usize,
        anchor: usize,
        area: (f64, f64, f64, f64),
        password: bool,
    ) -> Self {
        // Sub-pixel jitter of the caret rectangle is not a change an input method can use.
        let px = |v: f64| v.round() as i64;
        Self {
            text: text.to_owned(),
            cursor,
            anchor,
            area: (px(area.0), px(area.1), px(area.2), px(area.3)),
            password,
            hints: Value::Null,
        }
    }
}

struct WindowRecord {
    title: String,
    width: u32,
    height: u32,
    visible: bool,
    resizable: bool,
    decorations: bool,
    limits: requests::WindowLimits,
    probe: Probe,
    document: Option<HostedDocument>,
    native: Option<NativeWindow>,
    closing: bool,
}

enum JobKind {
    Dialog,
    External,
    DocumentDialog {
        document_id: String,
        node: nativehtmlkit_renderer::NodeId,
    },
    /// A `document.load` whose file reads run on a worker (CORE-A-14).
    DocumentLoad {
        window_id: String,
    },
}
impl JobKind {
    fn is_dialog(&self) -> bool {
        matches!(self, Self::Dialog | Self::DocumentDialog { .. })
    }
}
struct PendingJob {
    reply: Option<Reply>,
    parent: Option<String>,
    kind: JobKind,
    cancelled: bool,
    dialog: Option<platform::dialogs::DialogHandle>,
}
struct JobResult {
    id: u64,
    value: Result<Value, RpcError>,
}

enum TransferTarget {
    Rpc(Reply),
    Paste(String),
    Drop(String),
    DropText(String),
    DocumentClipboard {
        window_id: String,
        document_id: String,
        request_id: u64,
    },
    DocumentDrop {
        window_id: String,
        document_id: String,
        target: Value,
        files: bool,
        x: f64,
        y: f64,
    },
}
struct PendingTransfer {
    target: TransferTarget,
    deadline: Instant,
}

pub struct Host {
    #[cfg(test)]
    observed_notifications: Vec<Value>,
    /// CORE-A-13: droppable notifications refused under output backpressure, and whether
    /// the current pressure episode was already logged.
    dropped_notifications: u64,
    notification_pressure: bool,
    config: AppConfig,
    icon: winit::icon::Icon,
    headless: bool,
    standalone: bool,
    lifecycle: Lifecycle,
    decisions: DecisionTracker,
    server: Option<Server>,
    backend: Option<Backend>,
    _instance: Option<InstanceLock>,
    client: Option<u64>,
    /// CORE-A-04: why the host dropped the authenticated backend (it could not receive bounded
    /// output); turned into the session failure on the next pump, where an event loop is at hand.
    send_failure: Option<String>,
    /// Connections the host closed itself: frames they had already queued are discarded.
    dropped_connections: std::collections::HashSet<u64>,
    connected: HashMap<u64, Instant>,
    token: String,
    wake: Wake,
    windows: BTreeMap<String, WindowRecord>,
    native_ids: HashMap<WindowId, String>,
    next_window: u64,
    started: Instant,
    quit_deadline: Option<Instant>,
    next_job: u64,
    jobs: HashMap<u64, PendingJob>,
    job_tx: mpsc::SyncSender<JobResult>,
    job_rx: mpsc::Receiver<JobResult>,
    action_tx: mpsc::SyncSender<ActionEvent>,
    action_rx: mpsc::Receiver<ActionEvent>,
    transfers: HashMap<AsyncRequestSerial, PendingTransfer>,
    /// Wayland selection id observed just before this host's last clipboard
    /// write, and when it was written (see read_clipboard_now).
    clipboard_written: Option<(Option<winit::data_transfer::DataTransferId>, Instant)>,
    /// Clipboard reads waiting for the compositor to announce our selection.
    deferred_clipboard_reads: Vec<TransferTarget>,
    exit_code: Arc<AtomicI32>,
    /// Set when a supervising launcher process has exited.
    launcher_gone: Option<Arc<AtomicBool>>,
    /// Set once this event loop has acted on `launcher_gone`; the launcher watchdog
    /// terminates the process when this does not happen in time (NHK-CROSS-0001).
    launcher_acknowledged: Arc<AtomicBool>,
    /// The backend process-group id shared with the launcher watchdog; zeroed when the
    /// backend is reaped (FR-R-13).
    backend_group: Arc<AtomicU32>,
    /// Method and receive time of in-flight requests, recorded only at trace level.
    rpc_started: HashMap<Reply, (String, Instant)>,
    /// Shared native presentation context, created with the first window.
    presentation: Option<softbuffer::Context<OwnedDisplayHandle>>,
    preferences: platform::preferences::PreferenceWatch,
    preference_revision: u64,
}

/// Trace-level record of a host notification (NHK-CROSS-0004): the method, and for
/// `window.event`/`document.event` only the event type name and window/document ids.
fn trace_notification(method: &str, params: &Value) {
    let (kind, window_id, document_id) = match method {
        "window.event" => (params["type"].as_str(), params["window_id"].as_str(), None),
        "document.event" => (
            params["event"]["type"].as_str(),
            params["window_id"].as_str(),
            params["document_id"].as_str(),
        ),
        _ => (None, None, None),
    };
    tracing::trace!(
        event = "notify.sent",
        method,
        "type" = kind,
        window_id,
        document_id,
    );
}

/// Launch a manifest, forwarding secondary launches to the existing instance.
pub fn run(config: AppConfig, headless: bool) -> Result<i32> {
    platform::initialize_application(&config.app.id)?;
    let instance = if config.host.instance == InstancePolicy::Single {
        Some(InstanceLock::acquire(&config.app.id)?)
    } else {
        None
    };
    if let Some(lock) = &instance {
        if !lock.primary {
            forward_activation(&lock.endpoint, &config)?;
            return Ok(0);
        }
    }
    let result = Arc::new(AtomicI32::new(0));
    if headless {
        let mut host = Host::new(
            config,
            true,
            false,
            instance,
            Arc::new(|| {}),
            result.clone(),
        )?;
        while host.lifecycle.phase() != Phase::Stopped {
            host.pump(None);
            std::thread::sleep(Duration::from_millis(5));
        }
    } else {
        let event_loop = EventLoop::new().context(
            "cannot create native event loop (Wayland or interactive Windows desktop required)",
        )?;
        let proxy = event_loop.create_proxy();
        let wake: Wake = Arc::new(move || proxy.wake_up());
        let host = Host::new(config, false, false, instance, wake, result.clone())?;
        event_loop.run_app(host)?;
    }
    Ok(result.load(Ordering::Relaxed))
}

/// Deliver this launch to the primary instance. A primary that is still
/// starting (for example two simultaneous launches) cannot yet deliver the
/// activation to a ready backend, so retry within the manifest's bounded
/// startup deadline instead of losing the user's launch.
fn forward_activation(endpoint: &str, config: &AppConfig) -> Result<()> {
    let request = json!({"jsonrpc":"2.0","id":"activation","method":"app.activate",
        "params":{"args":std::env::args().collect::<Vec<_>>(),"cwd":std::env::current_dir()?,
            "activation_token":std::env::var("XDG_ACTIVATION_TOKEN").ok()}});
    let deadline = Instant::now()
        + Duration::from_millis(config.host.startup_timeout_ms)
        + Duration::from_secs(5);
    loop {
        let timeout = deadline
            .saturating_duration_since(Instant::now())
            .clamp(Duration::from_millis(100), Duration::from_secs(5));
        let response = transport::request(endpoint, request.clone(), timeout)?;
        match protocol::parse_response(response)?.outcome {
            Ok(_) => return Ok(()),
            Err(error)
                if error.code == protocol::code::UNAVAILABLE && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error.into()),
        }
    }
}

/// The native probe works without a language runtime, so failures can be
/// isolated to the host/OS boundary before testing Python.
pub fn run_probe() -> Result<i32> {
    let config = AppConfig {
        app: AppInfo {
            id: "org.nativehtmlkit.HostProbe".into(),
            name: "NativeHTMLKit native host probe".into(),
            version: None,
            icon: None,
        },
        backend: BackendConfig {
            program: PathBuf::new(),
            args: Vec::new(),
            cwd: std::env::current_dir()?,
            env: BTreeMap::new(),
        },
        host: HostConfig {
            instance: InstancePolicy::Multiple,
            ..HostConfig::default()
        },
    };
    platform::initialize_application(&config.app.id)?;
    let event_loop = EventLoop::new()?;
    let proxy = event_loop.create_proxy();
    let result = Arc::new(AtomicI32::new(0));
    let host = Host::new(
        config,
        false,
        true,
        None,
        Arc::new(move || proxy.wake_up()),
        result.clone(),
    )?;
    event_loop.run_app(host)?;
    Ok(result.load(Ordering::Relaxed))
}

pub fn doctor() -> Value {
    let environment: serde_json::Map<String, Value> = [
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_ID",
        "SESSIONNAME",
        "UI4ME_SYSTEMD_UNIT",
    ]
    .into_iter()
    .filter_map(|key| {
        std::env::var(key)
            .ok()
            .map(|v| (key.to_owned(), Value::String(v)))
    })
    .collect();
    json!({"host_version":env!("CARGO_PKG_VERSION"),"protocol_version":protocol::PROTOCOL_VERSION,
        "os":std::env::consts::OS,"architecture":std::env::consts::ARCH,"pid":std::process::id(),
        "environment":environment,"containment":platform::containment_description(),
        "run_containment":platform::run_containment_description(),
        "paths":platform::standard_paths("org.nativehtmlkit.HostProbe").ok(),
        "native_tests":"not run by doctor","renderer":"CPU HTML/CSS desktop profile ui4me-desktop-1.0 (plus diagnostic probe)",
        "windowing_dependency":"winit 0.31.0-beta.4 (exactly pinned)",
        "build":crate::build_info::source()})
}

fn random_token() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("OS random source: {e}"))?;
    Ok(bytes.iter().map(|v| format!("{v:02x}")).collect())
}

/// The backend's launch token and the endpoint's instance key. The token is secret; the instance
/// key is public (socket path / pipe name).
fn session_credentials(instance: Option<&InstanceLock>) -> Result<(String, String)> {
    let token = random_token()?;
    // CORE-A-03: an independent draw from the same OS source, never a slice of the secret.
    let instance_key = match instance {
        Some(i) => i.instance_key.clone(),
        None => random_token()?[..24].to_owned(),
    };
    Ok((token, instance_key))
}

impl Host {
    fn new(
        config: AppConfig,
        headless: bool,
        standalone: bool,
        instance: Option<InstanceLock>,
        wake: Wake,
        exit_code: Arc<AtomicI32>,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        if !headless {
            accesskit_atspi_common::set_application_name(config.app.name.clone())
                .map_err(anyhow::Error::msg)?;
        }
        let icon = crate::icons::load(config.app.icon.as_deref())?;
        let (token, instance_key) = session_credentials(instance.as_ref())?;
        let server = if standalone {
            None
        } else {
            let swept = bitmap::sweep_stale_transports();
            if swept > 0 {
                tracing::info!(swept, "removed bitmap transport files of exited hosts");
            }
            Some(Server::bind(&config.app.id, &instance_key, wake.clone())?)
        };
        let backend = match &server {
            Some(server) => Some(Backend::spawn(
                &config.backend,
                &server.endpoint,
                &token,
                &config.app.id,
                wake.clone(),
            )?),
            None => None,
        };
        let (job_tx, job_rx) = mpsc::sync_channel(protocol::MAX_PENDING);
        let (action_tx, action_rx) = mpsc::sync_channel(128);
        let launcher_acknowledged = Arc::new(AtomicBool::new(false));
        let backend_group = Arc::new(AtomicU32::new(backend.as_ref().map_or(0, Backend::id)));
        let launcher_gone = platform::watch_launcher(
            wake.clone(),
            platform::LauncherWatchdog {
                acknowledged: launcher_acknowledged.clone(),
                shutdown_timeout: Duration::from_millis(config.host.shutdown_timeout_ms),
                backend_group: backend_group.clone(),
            },
        );
        let preferences = platform::preferences::PreferenceWatch::new(!headless, wake.clone());
        let mut lifecycle = Lifecycle::new();
        lifecycle.begin_connecting()?;
        if standalone {
            lifecycle.begin_initializing()?;
            lifecycle.ready()?;
        }
        tracing::info!(
            headless,
            standalone,
            containment = platform::containment_description(),
            "host started"
        );
        Ok(Self {
            config,
            icon,
            headless,
            standalone,
            lifecycle,
            decisions: DecisionTracker::default(),
            server,
            backend,
            _instance: instance,
            client: None,
            send_failure: None,
            dropped_connections: Default::default(),
            connected: HashMap::new(),
            token,
            wake,
            windows: BTreeMap::new(),
            native_ids: HashMap::new(),
            next_window: 1,
            started: Instant::now(),
            quit_deadline: None,
            next_job: 1,
            jobs: HashMap::new(),
            job_tx,
            job_rx,
            action_tx,
            action_rx,
            transfers: HashMap::new(),
            clipboard_written: None,
            deferred_clipboard_reads: Vec::new(),
            exit_code,
            launcher_gone,
            launcher_acknowledged,
            backend_group,
            rpc_started: HashMap::new(),
            presentation: None,
            preferences,
            preference_revision: 0,
            #[cfg(test)]
            observed_notifications: Vec::new(),
            dropped_notifications: 0,
            notification_pressure: false,
        })
    }

    fn send(&mut self, connection: u64, value: Value) {
        let Some(server) = &self.server else {
            return;
        };
        if let Err(error) = server.reply(connection, value) {
            if self.send_failed(connection, &error) {
                if let Some(server) = &self.server {
                    server.disconnect(connection);
                }
            }
        }
    }

    /// FR-R-14: a send to a connection that is already closed is not an output overflow: the
    /// connection's `Disconnected` event reports it (as "connection lost"), and nothing is
    /// recorded that no later event would remove. Returns whether the host must close the
    /// connection (a live peer that cannot receive bounded output).
    fn send_failed(&mut self, connection: u64, error: &anyhow::Error) -> bool {
        if error.is::<crate::transport::ConnectionClosed>() {
            tracing::debug!(connection, "reply to a closed connection discarded");
            return false;
        }
        tracing::warn!(connection, %error, "peer cannot receive bounded output");
        self.dropped_connections.insert(connection);
        if self.client == Some(connection) {
            self.client = None;
            self.send_failure.get_or_insert_with(|| {
                format!("Backend cannot receive bounded host output: {error}")
            });
        }
        true
    }
    fn respond(&mut self, reply: &Reply, value: Result<Value, RpcError>) {
        let mut error_code = value.as_ref().err().map(|error| error.code);
        let mut message = match value {
            Ok(value) => protocol::success(&reply.1, value),
            Err(error) => protocol::error(Some(&reply.1), &error),
        };
        if let Err(error) = protocol::encoded_size(&message) {
            tracing::warn!(%error, "response could not fit the protocol frame");
            let error = RpcError::unavailable("Result exceeds the encoded protocol frame limit");
            error_code = Some(error.code);
            message = protocol::error(Some(&reply.1), &error);
        }
        self.send(reply.0, message);
        if !self.rpc_started.is_empty() {
            if let Some((method, started)) = self.rpc_started.remove(reply) {
                // NHK-CROSS-0004: metadata only, never params or results.
                tracing::trace!(
                    event = "rpc.completed",
                    method = method.as_str(),
                    id = reply.1.as_str(),
                    connection = reply.0,
                    duration_us = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX),
                    error_code,
                );
            }
        }
    }

    /// Trace-level record of a client request or notification (NHK-CROSS-0004). The method
    /// name is envelope-validated (`[A-Za-z0-9._-]`, at most 128 bytes); params are never
    /// recorded. Nothing is computed when trace is disabled.
    fn trace_received(&mut self, connection: u64, method: &str, reply: Option<&Reply>) {
        if !tracing::enabled!(tracing::Level::TRACE) {
            return;
        }
        tracing::trace!(
            event = "rpc.received",
            method,
            id = reply.map(|reply| reply.1.as_str()),
            connection,
        );
        if let Some(reply) = reply {
            if self.rpc_started.len() < RPC_TRACE_LIMIT {
                self.rpc_started
                    .insert(reply.clone(), (method.to_owned(), Instant::now()));
            }
        }
    }
    fn notify(&mut self, method: &str, params: Value) {
        if let Some(client) = self.client {
            let droppable = DROPPABLE_NOTIFICATIONS.contains(&method);
            let message = protocol::notification(method, params);
            if let Err(error) = protocol::encoded_size(&message) {
                // A notification has no caller awaiting an error response.
                // Preserve the session and make this dropped event visible in
                // diagnostics rather than disconnecting a healthy backend.
                tracing::warn!(method, %error, "oversized host notification dropped");
                return;
            }
            if tracing::enabled!(tracing::Level::TRACE) {
                trace_notification(method, &message["params"]);
            }
            #[cfg(test)]
            self.observed_notifications.push(message.clone());
            if !droppable {
                self.send(client, message);
                return;
            }
            let Some(server) = &self.server else {
                return;
            };
            match server.notify_droppable(client, message) {
                Ok(()) => self.notification_pressure = false,
                Err(error) if error.is::<crate::transport::Backpressure>() => {
                    self.dropped_notifications += 1;
                    if !std::mem::replace(&mut self.notification_pressure, true) {
                        tracing::warn!(
                            method,
                            dropped_total = self.dropped_notifications,
                            "backend is not reading; dropping input notifications until it catches up"
                        );
                    }
                }
                Err(error) => {
                    if self.send_failed(client, &error) {
                        if let Some(server) = &self.server {
                            server.disconnect(client);
                        }
                    }
                }
            }
        }
    }
    fn window_event_message(&mut self, id: &str, kind: &str, data: Value) {
        self.notify(
            "window.event",
            json!({"window_id":id,"type":kind,"data":data}),
        );
    }

    fn pump(&mut self, event_loop: Option<&dyn ActiveEventLoop>) {
        self.apply_native_preferences(event_loop);
        // An input/log flood must not monopolize native event dispatch.
        for _ in 0..64 {
            let event = self
                .server
                .as_ref()
                .and_then(|s| s.incoming.try_recv().ok());
            let Some(event) = event else { break };
            match event {
                TransportEvent::Connected(id) => {
                    self.connected.insert(id, Instant::now());
                }
                TransportEvent::Message(id, value) => self.message(id, value, event_loop),
                TransportEvent::Disconnected(id, reason) => {
                    if !self.rpc_started.is_empty() {
                        self.rpc_started.retain(|reply, _| reply.0 != id);
                    }
                    self.connected.remove(&id);
                    self.dropped_connections.remove(&id);
                    if self.client == Some(id) {
                        self.client = None;
                        if !matches!(
                            self.lifecycle.phase(),
                            Phase::Quitting | Phase::Stopped | Phase::Failed
                        ) {
                            self.fail(&format!("Backend connection lost: {reason}"), event_loop);
                        }
                    }
                }
            }
            self.fail_on_send_failure(event_loop);
        }
        self.fail_on_send_failure(event_loop);
        for _ in 0..32 {
            let Ok(result) = self.job_rx.try_recv() else {
                break;
            };
            if let Some(job) = self.jobs.remove(&result.id) {
                if !job.cancelled {
                    if let JobKind::DocumentDialog {
                        ref document_id,
                        node,
                    } = job.kind
                    {
                        if let Some(parent) = &job.parent {
                            self.complete_document_dialog(
                                parent,
                                document_id,
                                node,
                                result.value,
                                event_loop,
                            );
                        }
                    } else if let JobKind::DocumentLoad { window_id } = &job.kind {
                        let window_id = window_id.clone();
                        let outcome = self.finish_document_load(&window_id, result.value);
                        self.prune_document_work(&window_id);
                        self.document_changed(&window_id, event_loop);
                        if let Some(reply) = &job.reply {
                            self.respond(reply, outcome);
                        }
                    } else if let Some(reply) = job.reply {
                        self.respond(&reply, result.value);
                    } else if let Some(parent) = &job.parent {
                        if let Some(window) = self.windows.get_mut(parent) {
                            window.probe.status = match result.value {
                                Ok(value) => format!("Result: {value}").chars().take(180).collect(),
                                Err(error) => error.to_string().chars().take(180).collect(),
                            };
                        }
                    }
                }
                if let Some(parent) = job.parent {
                    let close = self.windows.get(&parent).is_some_and(|w| w.closing)
                        && !self
                            .jobs
                            .values()
                            .any(|j| j.parent.as_ref() == Some(&parent));
                    if close {
                        self.remove_window(&parent);
                    } else {
                        self.refresh(&parent);
                    }
                }
            }
        }
        for _ in 0..32 {
            let Ok(action) = self.action_rx.try_recv() else {
                break;
            };
            self.accessibility_action(action, event_loop);
        }
        let now = Instant::now();
        let expired: Vec<_> = self
            .connected
            .iter()
            .filter(|(id, at)| {
                Some(**id) != self.client && now.duration_since(**at) > Duration::from_secs(5)
            })
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            if let Some(server) = &self.server {
                server.disconnect(id);
            }
            self.connected.remove(&id);
        }
        let expired: Vec<_> = self
            .transfers
            .iter()
            .filter(|(_, t)| now >= t.deadline)
            .map(|(id, _)| *id)
            .collect();
        for serial in expired {
            if let Some(transfer) = self.transfers.remove(&serial) {
                self.finish_transfer(
                    transfer.target,
                    Err(RpcError::timeout("Native data transfer timed out")),
                );
            }
        }
        if self
            .launcher_gone
            .as_ref()
            .is_some_and(|gone| gone.load(Ordering::SeqCst))
        {
            // The launcher was terminated (signal, closed terminal or a killed
            // test harness). This is an external termination, not an answer
            // to any pending decision: cancel those and shut down boundedly.
            self.launcher_gone = None;
            self.launcher_acknowledged.store(true, Ordering::SeqCst);
            tracing::debug!(
                event = "launcher.lost",
                "event loop acknowledged launcher loss"
            );
            if !matches!(self.lifecycle.phase(), Phase::Quitting | Phase::Stopped) {
                self.exit_code.store(1, Ordering::Relaxed);
                self.begin_shutdown();
            }
        }
        if let Some(event_loop) = event_loop {
            for target in std::mem::take(&mut self.deferred_clipboard_reads) {
                self.read_clipboard_now(event_loop, target);
            }
        }
        self.poll_documents(event_loop);
        self.reveal_windows();
        if let Some(backend) = &self.backend {
            crate::logging::backend_logs(backend.drain_logs());
        }
        let backend_exit = self.backend.as_mut().and_then(|b| match b.try_wait() {
            Ok(exit) => exit.map(Ok),
            Err(error) => Some(Err(error)),
        });
        if let Some(exit) = backend_exit {
            // try_wait reaped it: its process-group id may be reused from now on.
            self.backend_group.store(0, Ordering::SeqCst);
            self.backend.take();
            match exit {
                Ok(status) if self.lifecycle.phase() == Phase::Quitting => {
                    if !status.success() {
                        self.exit_code.store(1, Ordering::Relaxed);
                        tracing::warn!(%status, "backend exited unsuccessfully");
                    }
                    self.stop(event_loop);
                }
                Ok(status) => self.fail(&format!("Backend exited with {status}"), event_loop),
                Err(error) => self.fail(&format!("Cannot observe backend: {error}"), event_loop),
            }
        }
        if matches!(
            self.lifecycle.phase(),
            Phase::Connecting | Phase::Initializing
        ) && self.started.elapsed() >= Duration::from_millis(self.config.host.startup_timeout_ms)
        {
            self.fail(
                "Backend did not authenticate and become ready before the startup deadline",
                event_loop,
            );
        }
        if self.lifecycle.phase() == Phase::Quitting {
            if self.backend.is_none() {
                self.stop(event_loop);
            } else if self.quit_deadline.is_some_and(|deadline| now >= deadline) {
                if let Some(backend) = &mut self.backend {
                    let _ = backend.kill();
                }
                self.exit_code.store(1, Ordering::Relaxed);
                tracing::error!(
                    "backend exceeded shutdown deadline; managed workers are being terminated"
                );
                self.stop(event_loop);
            }
        }
        self.flush_document_refreshes();
    }

    /// Record the deferred CORE-A-04 failure with its real cause (output overflow), unless the
    /// session is already shutting down, exactly like a lost connection.
    fn fail_on_send_failure(&mut self, event_loop: Option<&dyn ActiveEventLoop>) {
        if let Some(reason) = self.send_failure.take() {
            if !matches!(
                self.lifecycle.phase(),
                Phase::Quitting | Phase::Stopped | Phase::Failed
            ) {
                self.fail(&reason, event_loop);
            }
        }
    }

    fn message(&mut self, connection: u64, value: Value, event_loop: Option<&dyn ActiveEventLoop>) {
        if self.dropped_connections.contains(&connection) {
            // Queued before the host closed this connection: never act on it (CORE-A-04).
            tracing::debug!(
                connection,
                "discarding a frame from a connection the host closed"
            );
            return;
        }
        if value.get("method").is_none() && self.client == Some(connection) {
            match protocol::parse_response(value) {
                Ok(response) => {
                    let allowed = response
                        .outcome
                        .as_ref()
                        .ok()
                        .and_then(|v| v.get("allow"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    if let Ok(resolved) = self.decisions.resolve(&response.id, allowed) {
                        match resolved.decision.kind {
                            DecisionKind::CloseWindow { window_id } if resolved.allowed => {
                                // A window awaiting cancellation of its native
                                // dialog is already committed to closing. It
                                // must not make the final usable window look
                                // like an ordinary non-final close.
                                if self
                                    .windows
                                    .values()
                                    .filter(|window| !window.closing)
                                    .count()
                                    <= 1
                                {
                                    let _ = self.request_quit("last_window");
                                } else {
                                    self.remove_window(&window_id);
                                }
                            }
                            DecisionKind::QuitApplication if resolved.allowed => {
                                self.begin_shutdown()
                            }
                            DecisionKind::QuitApplication => {
                                let _ = self.lifecycle.cancel_quit();
                                self.status_all("Quit cancelled. Application is still running.");
                            }
                            DecisionKind::CloseWindow { window_id } => {
                                if let Some(window) = self.windows.get_mut(&window_id) {
                                    window.probe.status =
                                        "Close cancelled. Window remains open.".into();
                                }
                                self.refresh(&window_id);
                            }
                        }
                    } else {
                        tracing::debug!("ignored stale decision response");
                    }
                }
                Err(error) => tracing::warn!(%error, "invalid backend response"),
            }
            return;
        }
        let request = match protocol::parse_request(value) {
            Ok(request) => request,
            Err(error) => {
                self.send(connection, protocol::error(None, &error));
                return;
            }
        };
        let reply = request.id.as_ref().map(|id| (connection, id.clone()));
        self.trace_received(connection, &request.method, reply.as_ref());
        if request.method == "host.hello" {
            let outcome = self.hello(connection, &request.params);
            if let Some(reply) = reply {
                self.respond(&reply, outcome);
            }
            return;
        }
        // app.activate is only reachable on the current user's private,
        // desktop-session-scoped endpoint, and never controls native resources.
        if request.method == "app.activate" && self.client != Some(connection) {
            let outcome = if matches!(
                self.lifecycle.phase(),
                Phase::Starting | Phase::Connecting | Phase::Initializing
            ) {
                // The backend has not called app.ready, so it may not have
                // registered for app.activated yet. The launcher retries.
                Err(RpcError::unavailable("Primary instance is still starting"))
            } else if matches!(
                self.lifecycle.phase(),
                Phase::Quitting | Phase::Stopped | Phase::Failed
            ) {
                Err(RpcError::closed("Primary instance is shutting down"))
            } else if request.params.to_string().len() > 32 * 1024 {
                Err(RpcError::invalid_params("Activation payload too large"))
            } else {
                for record in self.windows.values() {
                    if let Some(native) = &record.native {
                        native.window.focus_window();
                        break;
                    }
                }
                self.notify("app.activated", request.params);
                Ok(json!({"accepted":true}))
            };
            if let Some(reply) = reply {
                self.respond(&reply, outcome);
            }
            return;
        }
        if self.client != Some(connection) {
            if let Some(reply) = reply {
                self.respond(
                    &reply,
                    Err(RpcError::new(
                        protocol::code::UNAUTHENTICATED,
                        "host.hello required",
                    )),
                );
            }
            return;
        }
        if let Some(ref reply) = reply {
            let duplicate = self.jobs.values().any(|j| j.reply.as_ref() == Some(reply))
                || self
                    .transfers
                    .values()
                    .any(|t| matches!(&t.target, TransferTarget::Rpc(r) if r == reply));
            if duplicate {
                // Replying to the duplicate would consume the client's future
                // for the original request. Its eventual completion would
                // then produce a second, ambiguous response under the same
                // ID. End the invalid session and cancel all native work.
                if let Some(server) = &self.server {
                    server.disconnect(connection);
                }
                self.client = None;
                self.fail(
                    "Backend reused a request ID while its original request was pending",
                    event_loop,
                );
                return;
            }
        }
        match self.dispatch(&request.method, request.params, reply.clone(), event_loop) {
            Ok(Some(result)) => {
                if let Some(reply) = reply {
                    self.respond(&reply, Ok(result));
                }
            }
            Ok(None) => {}
            Err(error) => {
                if let Some(reply) = reply {
                    self.respond(&reply, Err(error));
                }
            }
        }
    }

    fn hello(&mut self, connection: u64, params: &Value) -> Result<Value, RpcError> {
        if self.client.is_some() {
            return Err(RpcError::busy("Backend is already connected"));
        }
        if self.lifecycle.phase() != Phase::Connecting {
            return Err(RpcError::invalid_state("Host no longer accepts a backend"));
        }
        if params.get("protocol_version").and_then(Value::as_u64)
            != Some(protocol::PROTOCOL_VERSION.into())
        {
            return Err(RpcError::new(
                protocol::code::VERSION_MISMATCH,
                "Protocol version 1 is required",
            ));
        }
        let supplied = params
            .get("token")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut different = supplied.len() ^ self.token.len();
        for (a, b) in supplied.as_bytes().iter().zip(self.token.as_bytes()) {
            different |= usize::from(a ^ b);
        }
        if different != 0 {
            return Err(RpcError::new(
                protocol::code::UNAUTHENTICATED,
                "Invalid backend credential",
            ));
        }
        self.client = Some(connection);
        self.lifecycle
            .begin_initializing()
            .map_err(|e| RpcError::invalid_state(e.to_string()))?;
        Ok(
            json!({"protocol_version":1,"host_version":env!("CARGO_PKG_VERSION"),
            "app_id":self.config.app.id,"capabilities":self.capabilities()}),
        )
    }

    fn capabilities(&self) -> Value {
        let native = !self.headless;
        let preferences = self.preferences.snapshot();
        let observed_service =
            |available: Option<bool>| if native { available } else { Some(false) };
        let feature = |implemented: bool, available: Option<bool>, scope: &str| {
            json!({
            "implemented":implemented,"available":available,
            "qualification":if implemented {"requires_current_native_evidence"}else{"out_of_scope"},"scope":scope})
        };
        let features = json!({
            "window_constraints":feature(true,Some(native),"Paired min/max CSS-pixel bounds; OS may impose additional limits"),
            "window_observation":feature(true,Some(native),"Native observed state; unavailable properties are null, separate from requested state"),
                "window_visibility":feature(cfg!(target_os="windows"),Some(native && cfg!(target_os="windows")),"Pinned Wayland backend cannot create hidden windows or show/hide existing top levels"),
                "window_focus":feature(cfg!(target_os="windows"),Some(native && cfg!(target_os="windows")),"Wayland focus is compositor-owned; use advisory attention"),
            "window_position":feature(cfg!(target_os="windows"),Some(native && cfg!(target_os="windows")),"Compositor owns Wayland top-level placement"),
            "window_restore":feature(cfg!(target_os="windows"),Some(native && cfg!(target_os="windows")),"Wayland has no un-minimize request"),
            "attention":feature(true,observed_service(if cfg!(target_os="windows") {Some(true)}else{None}),"Window-manager policy; Wayland requires xdg-activation and cannot cancel urgency"),
            "clipboard_text":feature(true,observed_service(None),"Text only; availability is established by the native request"),
            "file_dialogs":feature(true,observed_service(None),"Parented native picker; Linux requires the desktop portal"),
            "external_open":feature(true,observed_service(None),"Optional parent forwarded to OpenURI or Windows Shell; accepted is not proof of a presented app"),
            "reveal_file":feature(true,observed_service(None),"Local path selection via FileManager1.ShowItems or Windows Shell; these services have no parent argument"),
            "native_preferences":feature(true,observed_service(preferences.availability()),"Observed color scheme, reduced motion and high contrast; missing per-setting values stay null"),
            "ime":feature(true,observed_service(None),"Composition, surrounding text and candidate area depend on the active input method"),
            "ime_purpose_hints":feature(true,Some(native && cfg!(target_os="linux")),"Advisory Wayland input purpose/hints; pinned winit does not implement these hints on Windows"),
            "incoming_drop":feature(true,observed_service(None),"Bounded local file or text data, with document target policy"),
            "accessibility":feature(true,observed_service(None),"AccessKit AT-SPI or UI Automation; presence does not qualify screen-reader journeys"),
            "printing":feature(false,Some(false),"Outside the desktop core contract"),
            "tray":feature(false,Some(false),"Outside the current core contract")
        });
        let mut result = json!({"native":!self.headless,"headless":self.headless,"windows":true,"max_windows":self.config.host.max_windows,
            "clipboard_text":!self.headless,"file_dialogs":!self.headless,"external_open":!self.headless,
            "drag_drop":!self.headless,"ime":!self.headless,"accessibility":!self.headless,
            "probe":true,"html_css":true,"renderer":"cpu","css_profile":"ui4me-desktop-1.0","max_documents":documents::MAX_DOCUMENTS,"printing":false,"tray":false,
            "platform":std::env::consts::OS,"containment":platform::containment_description(),
            "limits":{"frame_bytes":protocol::MAX_FRAME_BYTES,"pending_requests":protocol::MAX_PENDING,"probe_text_bytes":crate::presentation::MAX_TEXT_BYTES},
            "window_positioning":native && cfg!(target_os="windows"),"native_qualification":"requires local evidence"});
        result["capability_schema"] = json!(2);
        result["qualification"] = json!({"status":"unqualified_candidate","evidence":"No native qualification is inferred from compiled code, a previous release, or an available service"});
        result["legacy_flags"] = json!("Top-level service booleans describe enabled API surfaces; feature availability below is authoritative and null means unobserved");
        result["features"] = features;
        result["native_preferences"] = preferences.value();
        result
    }

    fn request_close(&mut self, window_id: &str, reason: &str) -> Result<(), RpcError> {
        if !self.windows.contains_key(window_id) {
            return Err(RpcError::closed("Window does not exist"));
        }
        if self
            .windows
            .get(window_id)
            .is_some_and(|window| window.closing)
        {
            return Ok(());
        }
        if matches!(
            self.lifecycle.phase(),
            Phase::Starting | Phase::Connecting | Phase::Initializing
        ) {
            // The app is not ready to answer policy callbacks yet. A backend that
            // discards one of several windows it created (NHK-CROSS-0002) closes
            // just that window. Closing the last usable window, or any user close,
            // is a cancellation of startup, with the same bounded cooperative
            // shutdown used after an accepted quit.
            let others_remain = self
                .windows
                .iter()
                .any(|(id, window)| id != window_id && !window.closing);
            if reason == "backend_request" && others_remain {
                self.remove_window(window_id);
            } else {
                self.begin_shutdown();
            }
            return Ok(());
        }
        if self.standalone || self.lifecycle.phase() == Phase::Failed {
            self.remove_window(window_id);
            if self.windows.is_empty()
                && !matches!(self.lifecycle.phase(), Phase::Quitting | Phase::Stopped)
            {
                self.begin_shutdown();
            }
            return Ok(());
        }
        if self.lifecycle.phase() != Phase::Running {
            return Err(RpcError::busy(
                "Application is not ready for a close decision",
            ));
        }
        let decision = self
            .decisions
            .begin_close(window_id)
            .map_err(|e| RpcError::busy(e.to_string()))?;
        if decision.is_new {
            if let Some(window) = self.windows.get_mut(window_id) {
                window.probe.status =
                    "Waiting for application close decision. Ctrl+Shift+Q forces shutdown.".into();
            }
            self.refresh(window_id);
            self.window_event_message(window_id, "close_requested", json!({"reason":reason}));
            if let Some(client) = self.client {
                tracing::trace!(
                    event = "request.sent",
                    method = "window.before_close",
                    id = decision.decision.id.as_str(),
                    window_id,
                );
                self.send(
                    client,
                    json!({"jsonrpc":"2.0","id":decision.decision.id,"method":"window.before_close",
                    "params":{"window_id":window_id,"reason":reason}}),
                );
            }
        }
        Ok(())
    }

    fn request_quit(&mut self, reason: &str) -> Result<(), RpcError> {
        if self.standalone || self.lifecycle.phase() == Phase::Failed {
            self.begin_shutdown();
            return Ok(());
        }
        if matches!(
            self.lifecycle.phase(),
            Phase::Starting | Phase::Connecting | Phase::Initializing
        ) {
            self.begin_shutdown();
            return Ok(());
        }
        if matches!(
            self.lifecycle.phase(),
            Phase::QuitPending | Phase::Quitting | Phase::Stopped
        ) {
            return Ok(());
        }
        self.lifecycle
            .request_quit()
            .map_err(|e| RpcError::invalid_state(e.to_string()))?;
        let start = match self.decisions.begin_quit() {
            Ok(start) => start,
            Err(error) => {
                let _ = self.lifecycle.cancel_quit();
                return Err(RpcError::busy(error.to_string()));
            }
        };
        for stale in start.superseded {
            self.notify(
                "request.cancelled",
                json!({"id":stale,"reason":"application_quit"}),
            );
        }
        if let Some(client) = self.client {
            tracing::trace!(
                event = "request.sent",
                method = "app.before_quit",
                id = start.decision.id.as_str(),
            );
            self.send(client, json!({"jsonrpc":"2.0","id":start.decision.id,"method":"app.before_quit","params":{"reason":reason}}));
        }
        self.status_all("Waiting for application quit decision. Ctrl+Shift+Q forces shutdown.");
        Ok(())
    }

    fn begin_shutdown(&mut self) {
        if self.lifecycle.phase() == Phase::QuitPending {
            let _ = self.lifecycle.commit_quit();
        } else {
            let _ = self.lifecycle.force_quit();
        }
        self.decisions.cancel_all();
        self.cancel_native_jobs("Application is shutting down");
        self.notify(
            "app.shutdown",
            json!({"deadline_ms":self.config.host.shutdown_timeout_ms}),
        );
        self.quit_deadline =
            Some(Instant::now() + Duration::from_millis(self.config.host.shutdown_timeout_ms));
        self.status_all("Shutting down application...");
    }

    fn force_shutdown(&mut self) {
        self.begin_shutdown();
        if let Some(backend) = &mut self.backend {
            let _ = backend.kill();
        }
        self.quit_deadline = Some(Instant::now());
    }

    fn stop(&mut self, event_loop: Option<&dyn ActiveEventLoop>) {
        if self.lifecycle.phase() == Phase::Stopped {
            return;
        }
        let _ = self.lifecycle.stop();
        if let Some(server) = &mut self.server {
            server.close();
        }
        self.cancel_native_jobs("Host stopped");
        self.native_ids.clear();
        self.windows.clear();
        if let Some(event_loop) = event_loop {
            event_loop.exit();
        }
        tracing::info!(
            exit_code = self.exit_code.load(Ordering::Relaxed),
            "host stopped"
        );
    }

    fn fail(&mut self, reason: &str, event_loop: Option<&dyn ActiveEventLoop>) {
        if matches!(self.lifecycle.phase(), Phase::Stopped | Phase::Failed) {
            return;
        }
        tracing::error!(reason, "application failed");
        let _ = self.lifecycle.fail(reason);
        self.exit_code.store(1, Ordering::Relaxed);
        self.decisions.cancel_all();
        self.cancel_native_jobs("Backend failed");
        if let Some(backend) = &mut self.backend {
            let _ = backend.kill();
        }
        if self.headless {
            self.stop(event_loop);
            return;
        }
        if self.windows.is_empty() {
            if let Err(error) = self.create_window(
                json!({"title":format!("{} — application error",self.config.app.name),
                "width":820,"height":460,"visible":true}),
                event_loop,
            ) {
                // There is no native recovery/quit surface left. Keeping the
                // event loop alive would leave an unreachable failed host.
                tracing::error!(%error, "cannot present application failure");
                self.stop(event_loop);
                return;
            }
        }
        for record in self.windows.values_mut() {
            record.document = None;
            record.probe.failure = Some(reason.chars().take(160).collect());
        }
        self.reveal_windows();
        for id in self.windows.keys().cloned().collect::<Vec<_>>() {
            self.refresh(&id);
        }
    }

    fn status_all(&mut self, status: &str) {
        for window in self.windows.values_mut() {
            window.probe.status = status.into();
        }
        for id in self.windows.keys().cloned().collect::<Vec<_>>() {
            self.refresh(&id);
        }
    }

    fn remove_window(&mut self, id: &str) {
        self.decisions.cancel_for_window(id);
        // A late clipboard/drop completion must not modify a parent whose
        // close has already been accepted, even if a dialog defers destruction.
        self.transfers.retain(|_, pending| match &pending.target {
            TransferTarget::Paste(parent)
            | TransferTarget::Drop(parent)
            | TransferTarget::DropText(parent) => parent != id,
            TransferTarget::DocumentClipboard { window_id, .. }
            | TransferTarget::DocumentDrop { window_id, .. } => window_id != id,
            TransferTarget::Rpc(_) => true,
        });
        self.deferred_clipboard_reads.retain(|target| match target {
            TransferTarget::Paste(parent)
            | TransferTarget::Drop(parent)
            | TransferTarget::DropText(parent) => parent != id,
            TransferTarget::DocumentClipboard { window_id, .. }
            | TransferTarget::DocumentDrop { window_id, .. } => window_id != id,
            TransferTarget::Rpc(_) => true,
        });
        if self
            .jobs
            .values()
            .any(|j| j.parent.as_deref() == Some(id) && (j.kind.is_dialog() || j.dialog.is_some()))
        {
            if let Some(window) = self.windows.get_mut(id) {
                window.closing = true;
                window.probe.status = "Waiting for the native request to close.".into();
            }
            let replies: Vec<_> = self
                .jobs
                .values_mut()
                .filter(|j| j.parent.as_deref() == Some(id))
                .filter_map(|j| {
                    j.cancelled = true;
                    if let Some(dialog) = &j.dialog {
                        dialog.cancel();
                    }
                    j.reply.take()
                })
                .collect();
            for reply in replies {
                self.respond(&reply, Err(RpcError::closed("Dialog parent is closing")));
            }
            self.refresh(id);
            return;
        }
        if let Some(window) = self.windows.remove(id) {
            if let Some(native) = window.native {
                self.native_ids.remove(&native.window.id());
            }
            self.window_event_message(id, "closed", json!({}));
            if self.windows.is_empty()
                && (self.standalone || self.lifecycle.phase() == Phase::Failed)
                && !matches!(self.lifecycle.phase(), Phase::Quitting | Phase::Stopped)
            {
                // A parented dialog may have delayed this removal beyond the
                // original request_close call. Complete the lifecycle here.
                self.begin_shutdown();
            }
        }
    }

    /// Show every window whose first presentation is due (NHK-CROSS-0006): apply the
    /// requested visibility and ask for the first frame.
    fn reveal_windows(&mut self) {
        let now = Instant::now();
        let failed = self.lifecycle.phase() == Phase::Failed;
        for record in self.windows.values_mut() {
            let has_document = record.document.is_some();
            let Some(native) = &mut record.native else {
                continue;
            };
            if reveal_due(native.reveal_deadline, has_document, failed, now) {
                native.reveal_deadline = None;
                native.window.set_visible(record.visible);
                native.window.request_redraw();
            }
        }
    }

    /// Time until the earliest pending first-presentation deadline, if any.
    fn reveal_wait(&self) -> Option<Duration> {
        let now = Instant::now();
        self.windows
            .values()
            .filter_map(|w| w.native.as_ref()?.reveal_deadline)
            .min()
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    fn refresh(&mut self, id: &str) {
        if self.windows.get(id).is_some_and(|w| w.document.is_some()) {
            self.refresh_document(id);
            return;
        }
        if let Some(record) = self.windows.get_mut(id) {
            if let Some(native) = &mut record.native {
                if native.document_epoch.take().is_some() {
                    let _ = native
                        .window
                        .request_ime_update(winit::window::ImeRequest::Disable);
                    native.ime_enabled = false;
                    native.composing = false;
                    native.preedit_cleared = false;
                    native.ime_quiet_until = Some(Instant::now() + Duration::from_millis(250));
                }
                let size = native.window.surface_size();
                native.accessibility.update(record.probe.tree(
                    &record.title,
                    size.width,
                    size.height,
                    native.window.scale_factor(),
                ));
                native::update_ime(&record.probe, native);
                native.window.request_redraw();
            }
        }
    }

    fn cancel_native_jobs(&mut self, reason: &str) {
        let replies: Vec<_> = self
            .jobs
            .values_mut()
            .filter_map(|job| {
                job.cancelled = true;
                if let Some(dialog) = &job.dialog {
                    dialog.cancel();
                }
                job.reply.take()
            })
            .collect();
        for reply in replies {
            self.respond(&reply, Err(RpcError::cancelled(reason)));
        }
        let pending = std::mem::take(&mut self.transfers);
        for transfer in pending.into_values() {
            if let TransferTarget::Rpc(reply) = transfer.target {
                self.respond(&reply, Err(RpcError::cancelled(reason)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FR-R-14: a reply to an already closed connection neither records an output overflow nor
    /// leaves an entry in `dropped_connections`; a real overflow of the client does both.
    #[test]
    fn send_errors_distinguish_a_closed_connection_from_an_overflow() {
        let mut host = controller(true);
        host.client = Some(7);
        let closed = anyhow::Error::from(crate::transport::ConnectionClosed);
        assert!(!host.send_failed(7, &closed));
        assert!(!host.send_failed(9, &closed));
        assert!(host.dropped_connections.is_empty());
        assert_eq!(host.send_failure, None);
        assert_eq!(host.client, Some(7));
        let overflow = anyhow::anyhow!("connection outgoing byte limit exceeded");
        assert!(host.send_failed(7, &overflow));
        assert!(host.dropped_connections.contains(&7));
        assert_eq!(host.client, None);
        assert!(host
            .send_failure
            .as_deref()
            .is_some_and(|r| r.contains("byte limit")));
    }

    /// NHK-CROSS-0006: a new window is revealed as soon as its document is loaded, at the
    /// latest after the bounded grace period, and at once when the application failed;
    /// a revealed window (`None`) is never revealed again.
    #[test]
    fn first_presentation_waits_for_the_document_within_a_bounded_grace() {
        let created = Instant::now();
        let deadline = Some(created + FIRST_PRESENT_GRACE);
        assert!(!reveal_due(deadline, false, false, created));
        assert!(!reveal_due(
            deadline,
            false,
            false,
            created + FIRST_PRESENT_GRACE - Duration::from_millis(1)
        ));
        assert!(reveal_due(deadline, true, false, created));
        assert!(reveal_due(deadline, false, true, created));
        assert!(reveal_due(
            deadline,
            false,
            false,
            created + FIRST_PRESENT_GRACE
        ));
        assert!(!reveal_due(None, true, true, created + FIRST_PRESENT_GRACE));
        assert!(FIRST_PRESENT_GRACE <= Duration::from_secs(1));
    }

    /// Headless windows have no native surface, so there is nothing to reveal and no
    /// first-presentation wake-up is scheduled.
    #[test]
    fn headless_windows_schedule_no_first_presentation_wakeup() {
        let mut host = controller(true);
        host.create_window(json!({}), None).unwrap();
        assert_eq!(host.reveal_wait(), None);
        host.reveal_windows();
    }

    /// A controller with no transport/process/native resources. This exercises
    /// lifecycle policy independently of desktop or IPC availability.
    pub(super) fn controller(ready: bool) -> Host {
        let (job_tx, job_rx) = mpsc::sync_channel(protocol::MAX_PENDING);
        let (action_tx, action_rx) = mpsc::sync_channel(128);
        let mut lifecycle = Lifecycle::new();
        lifecycle.begin_connecting().unwrap();
        lifecycle.begin_initializing().unwrap();
        if ready {
            lifecycle.ready().unwrap();
        }
        Host {
            icon: crate::icons::load(None).unwrap(),
            config: AppConfig {
                app: AppInfo {
                    id: "org.nativehtmlkit.ControllerTest".into(),
                    name: "Controller".into(),
                    version: None,
                    icon: None,
                },
                backend: BackendConfig {
                    program: PathBuf::new(),
                    args: Vec::new(),
                    cwd: PathBuf::new(),
                    env: BTreeMap::new(),
                },
                host: HostConfig::default(),
            },
            headless: true,
            standalone: false,
            lifecycle,
            decisions: DecisionTracker::default(),
            server: None,
            backend: None,
            _instance: None,
            client: Some(7),
            send_failure: None,
            dropped_connections: Default::default(),
            connected: HashMap::new(),
            token: String::new(),
            wake: Arc::new(|| {}),
            windows: BTreeMap::new(),
            native_ids: HashMap::new(),
            next_window: 1,
            started: Instant::now(),
            quit_deadline: None,
            next_job: 1,
            jobs: HashMap::new(),
            job_tx,
            job_rx,
            action_tx,
            action_rx,
            transfers: HashMap::new(),
            clipboard_written: None,
            deferred_clipboard_reads: Vec::new(),
            exit_code: Arc::new(AtomicI32::new(0)),
            launcher_gone: None,
            launcher_acknowledged: Arc::new(AtomicBool::new(false)),
            backend_group: Arc::new(AtomicU32::new(0)),
            rpc_started: HashMap::new(),
            presentation: None,
            preferences: platform::preferences::PreferenceWatch::new(false, Arc::new(|| {})),
            preference_revision: 0,
            #[cfg(test)]
            observed_notifications: Vec::new(),
            dropped_notifications: 0,
            notification_pressure: false,
        }
    }

    /// CORE-A-03: with `instance = "multiple"` the public endpoint name must carry no token bits.
    #[test]
    fn instance_key_is_independent_of_the_token() {
        for _ in 0..8 {
            let (token, key) = session_credentials(None).unwrap();
            assert_eq!(token.len(), 64);
            assert_eq!(key.len(), 24);
            assert!(key.bytes().all(|b| b.is_ascii_hexdigit()), "{key}");
            assert!(
                !token.contains(&key[..8]),
                "endpoint name {key} shares material with the token"
            );
        }
    }

    pub(super) fn add_window(host: &mut Host, id: &str) {
        host.windows.insert(
            id.into(),
            WindowRecord {
                title: id.into(),
                width: 800,
                height: 500,
                visible: true,
                resizable: true,
                decorations: true,
                limits: requests::WindowLimits::default(),
                probe: Probe::default(),
                document: None,
                native: None,
                closing: false,
            },
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn wayland_unminimize_is_rejected_instead_of_reporting_success() {
        let mut host = controller(true);
        add_window(&mut host, "one");
        let error = host
            .set_window(json!({"window_id":"one","minimized":false,"title":"changed"}))
            .unwrap_err();
        assert_eq!(error.code, protocol::code::UNSUPPORTED);
        assert_eq!(
            host.windows["one"].title, "one",
            "rejected request changes nothing"
        );
        assert!(host
            .set_window(json!({"window_id":"one","minimized":true}))
            .is_ok());
    }

    /// CORE-A-04: once the host drops a backend that cannot receive its bounded output, requests
    /// already queued from that connection must not run, and the recorded failure must name the
    /// real cause (not a later "Backend exited" or "connection lost").
    #[test]
    fn output_overflow_fails_with_its_cause_and_drops_queued_requests() {
        let app = format!("org.nativehtmlkit.host.overflow.{}", std::process::id());
        let lock = InstanceLock::acquire(&app).unwrap();
        let server = Server::bind(&app, &lock.instance_key, Arc::new(|| {})).unwrap();
        let mut stream =
            crate::platform::NativeStream::connect(&server.endpoint, Duration::from_secs(5))
                .unwrap();
        let connection = match server
            .incoming
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
        {
            TransportEvent::Connected(id) => id,
            other => panic!("unexpected {other:?}"),
        };
        // A request that was already queued when the host gives up on the connection.
        protocol::write_frame(
            &mut stream,
            &json!({"jsonrpc":"2.0","id":"late","method":"window.create","params":{"title":"late"}}),
        )
        .unwrap();
        let queued = match server
            .incoming
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
        {
            TransportEvent::Message(id, value) => {
                assert_eq!(id, connection);
                value
            }
            other => panic!("unexpected {other:?}"),
        };
        let mut host = controller(true);
        host.server = Some(server);
        host.client = Some(connection);
        // The backend never reads: fill the bounded outbound queue until a send fails.
        let big = "x".repeat(1024 * 1024);
        for _ in 0..64 {
            host.notify("test.flood", json!({ "payload": big }));
            if host.client.is_none() {
                break;
            }
        }
        assert_eq!(
            host.client, None,
            "the overflowing backend is no longer the client"
        );
        let windows = host.windows.len();
        host.message(connection, queued, None);
        assert_eq!(
            host.windows.len(),
            windows,
            "a request from the dropped connection ran"
        );
        host.pump(None);
        let failure = host.lifecycle.failure().unwrap_or_default().to_owned();
        assert!(
            failure.contains("cannot receive"),
            "wrong failure cause: {failure:?}"
        );
        assert_eq!(host.exit_code.load(Ordering::Relaxed), 1);
        drop(stream);
    }

    #[test]
    fn launcher_exit_during_pending_quit_shuts_down_without_approving_it() {
        let mut host = controller(true);
        add_window(&mut host, "one");
        host.request_quit("native_close").unwrap();
        assert_eq!(host.lifecycle.phase(), Phase::QuitPending);
        assert_eq!(host.decisions.pending_count(), 1);
        let gone = Arc::new(AtomicBool::new(false));
        host.launcher_gone = Some(gone.clone());
        host.pump(None);
        assert_eq!(
            host.lifecycle.phase(),
            Phase::QuitPending,
            "launcher still alive"
        );
        gone.store(true, Ordering::SeqCst);
        host.pump(None);
        // No backend remains in this fixture, so the bounded shutdown completes.
        assert_eq!(host.lifecycle.phase(), Phase::Stopped);
        assert_eq!(host.decisions.pending_count(), 0);
        assert_eq!(host.exit_code.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn startup_window_close_cancels_startup_without_a_policy_callback() {
        let mut host = controller(false);
        add_window(&mut host, "one");
        host.request_close("one", "native_close").unwrap();
        assert_eq!(host.lifecycle.phase(), Phase::Quitting);
        assert_eq!(host.decisions.pending_count(), 0);
        assert!(host.quit_deadline.is_some());
    }

    /// NHK-CROSS-0002: before app.ready a backend may discard one of several windows; only
    /// its last window (or a user close of any window) cancels startup.
    #[test]
    fn backend_close_before_ready_cancels_startup_only_for_the_last_window() {
        let mut host = controller(false);
        add_window(&mut host, "one");
        add_window(&mut host, "two");
        host.request_close("two", "backend_request").unwrap();
        assert_eq!(host.lifecycle.phase(), Phase::Initializing);
        assert!(host.quit_deadline.is_none());
        assert_eq!(host.windows.keys().collect::<Vec<_>>(), ["one"]);
        host.request_close("one", "backend_request").unwrap();
        assert_eq!(host.lifecycle.phase(), Phase::Quitting);

        let mut host = controller(false);
        add_window(&mut host, "one");
        add_window(&mut host, "two");
        host.request_close("two", "native_close").unwrap();
        assert_eq!(
            host.lifecycle.phase(),
            Phase::Quitting,
            "a user close during startup still cancels it"
        );
    }

    #[test]
    fn deferred_dialog_close_cannot_hide_the_final_usable_window() {
        let mut host = controller(true);
        add_window(&mut host, "one");
        add_window(&mut host, "two");
        host.jobs.insert(
            1,
            PendingJob {
                reply: None,
                parent: Some("one".into()),
                kind: JobKind::Dialog,
                cancelled: false,
                dialog: None,
            },
        );
        let first = host.decisions.begin_close("one").unwrap().decision;
        host.message(7, protocol::success(&first.id, json!({"allow":true})), None);
        assert!(host.windows["one"].closing);
        assert!(host.jobs[&1].cancelled);
        let final_close = host.decisions.begin_close("two").unwrap().decision;
        host.message(
            7,
            protocol::success(&final_close.id, json!({"allow":true})),
            None,
        );
        assert_eq!(host.lifecycle.phase(), Phase::QuitPending);
        assert!(!host.windows["two"].closing);
        let quit = host.decisions.begin_quit().unwrap().decision;
        host.message(7, protocol::success(&quit.id, json!({"allow":false})), None);
        assert!(host
            .job_tx
            .send(JobResult {
                id: 1,
                value: Ok(json!(null))
            })
            .is_ok());
        host.pump(None);
        assert_eq!(host.lifecycle.phase(), Phase::Running);
        assert_eq!(host.windows.len(), 1);
        assert!(host.windows.contains_key("two"));
    }

    #[test]
    fn deferred_final_probe_close_completes_the_host_lifecycle() {
        let mut host = controller(true);
        host.standalone = true;
        add_window(&mut host, "one");
        host.jobs.insert(
            1,
            PendingJob {
                reply: None,
                parent: Some("one".into()),
                kind: JobKind::Dialog,
                cancelled: false,
                dialog: None,
            },
        );
        host.request_close("one", "native_close").unwrap();
        assert!(host.windows["one"].closing);
        assert!(host
            .job_tx
            .send(JobResult {
                id: 1,
                value: Ok(json!(null))
            })
            .is_ok());
        host.pump(None);
        assert_eq!(host.lifecycle.phase(), Phase::Stopped);
        assert!(host.windows.is_empty());
    }

    #[test]
    fn duplicate_pending_id_fails_the_session_and_cancels_original_work() {
        let mut host = controller(true);
        host.jobs.insert(
            1,
            PendingJob {
                reply: Some((7, "duplicate".into())),
                parent: None,
                kind: JobKind::External,
                cancelled: false,
                dialog: None,
            },
        );
        host.message(
            7,
            protocol::request("duplicate", "host.diagnostics", json!({})),
            None,
        );
        assert_eq!(host.lifecycle.phase(), Phase::Stopped);
        assert_eq!(host.exit_code.load(Ordering::Relaxed), 1);
        assert!(host.client.is_none());
        assert!(host.jobs[&1].cancelled);
        assert!(host.jobs[&1].reply.is_none());
    }

    #[test]
    fn native_failure_without_a_diagnostic_surface_stops_instead_of_hanging() {
        let mut host = controller(false);
        host.headless = false;
        host.fail("Backend did not start", None);
        assert_eq!(host.lifecycle.phase(), Phase::Stopped);
        assert_eq!(host.exit_code.load(Ordering::Relaxed), 1);
        assert!(host.windows.is_empty());
    }

    /// Redraws re-derive the IME state; only a real change may be sent to the input method
    /// (an unchanged resend made fcitx5 answer with its pending dead-key preedit forever).
    #[test]
    fn ime_state_ignores_subpixel_jitter_but_not_real_changes() {
        let base = ImeState::new("ab", 1, 1, (10.2, 20.0, 2.0, 26.0), false);
        assert_eq!(
            base,
            ImeState::new("ab", 1, 1, (10.4, 19.8, 2.0, 26.0), false)
        );
        assert_ne!(
            base,
            ImeState::new("ab", 2, 2, (10.2, 20.0, 2.0, 26.0), false)
        );
        assert_ne!(
            base,
            ImeState::new("abc", 1, 1, (10.2, 20.0, 2.0, 26.0), false)
        );
        assert_ne!(
            base,
            ImeState::new("ab", 1, 1, (30.0, 20.0, 2.0, 26.0), false)
        );
        assert_ne!(
            base,
            ImeState::new("ab", 1, 1, (10.2, 20.0, 2.0, 26.0), true)
        );
    }
}

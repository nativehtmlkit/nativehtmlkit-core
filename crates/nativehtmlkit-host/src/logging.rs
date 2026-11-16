//! Bounded logging for an event-loop process. Neither a verbose backend nor
//! a slow or unread redirected stderr pipe can block the GUI's log writes.

use crate::backend::LogLine;
use anyhow::Result;
use std::io::Write;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use tracing_appender::non_blocking::{ErrorCounter, NonBlocking, NonBlockingBuilder, WorkerGuard};

const BUFFERED_RECORDS: usize = 128;
const BACKEND_RECORDS_PER_PUMP: usize = 32;
static SINK_DROPPED: OnceLock<ErrorCounter> = OnceLock::new();
static BACKEND_OMITTED: AtomicU64 = AtomicU64::new(0);

fn writer<T: Write + Send + 'static>(output: T) -> (NonBlocking, WorkerGuard) {
    NonBlockingBuilder::default()
        .buffered_lines_limit(BUFFERED_RECORDS)
        .lossy(true)
        .thread_name("nativehtmlkit-log-writer")
        .finish(output)
}

/// Retain this until the native event loop has stopped. Flushing is bounded;
/// a broken sink is allowed to lose log records instead of retaining an app.
pub struct LogGuard(Option<WorkerGuard>);

impl Drop for LogGuard {
    fn drop(&mut self) {
        let Some(guard) = self.0.take() else {
            return;
        };
        // Upstream's WorkerGuard has bounded channel waits, but its timeout
        // diagnostic writes synchronously to stdout. Dispose it away from the
        // caller too, so even two blocked output pipes cannot stall shutdown.
        // ManuallyDrop intentionally leaks the guard if spawning this final
        // helper fails: the process is exiting and must not perform blocking IO.
        let guard = ManuallyDrop::new(guard);
        let (finished, wait) = std::sync::mpsc::sync_channel(1);
        if std::thread::Builder::new()
            .name("nativehtmlkit-log-shutdown".into())
            .spawn(move || {
                drop(ManuallyDrop::into_inner(guard));
                let _ = finished.send(());
            })
            .is_ok()
        {
            let _ = wait.recv_timeout(Duration::from_millis(1500));
        }
    }
}

/// Map user-facing diagnostics levels to an explicit, bounded set of filters.
/// Unknown input fails closed to normal output; user-provided filter expressions
/// are not interpreted here (RUST_LOG remains available for advanced developers).
///
/// NHK-CROSS-0007: the relayed backend output (the records emitted by
/// [`backend_logs`] under [`BACKEND_OUTPUT_TARGET`]) is application output, not a
/// host diagnostic, so every level keeps it; `off` silences only the host's own
/// non-error records.
fn diagnostic_filter(level: &str) -> &'static str {
    match level.trim().to_ascii_lowercase().as_str() {
        "off" => "nativehtmlkit_host=error,nativehtmlkit_host::logging=info",
        "debug" => "nativehtmlkit_host=debug,nativehtmlkit_host::logging=info",
        "trace" => "nativehtmlkit_host=trace,nativehtmlkit_host::logging=info",
        _ => "nativehtmlkit_host=info,nativehtmlkit_host::logging=info",
    }
}

/// Target of the relayed backend output records (unchanged from 1.0.2, so record
/// shapes stay the same). Only [`backend_logs`] logs under it.
const BACKEND_OUTPUT_TARGET: &str = "nativehtmlkit_host::logging";

/// An explicit `RUST_LOG` stays the expert override, but unless it names the
/// backend-output target itself, backend output is still relayed.
fn with_backend_output(
    filter: tracing_subscriber::EnvFilter,
    raw: &str,
) -> tracing_subscriber::EnvFilter {
    if raw.contains(BACKEND_OUTPUT_TARGET) {
        return filter;
    }
    match format!("{BACKEND_OUTPUT_TARGET}=info").parse() {
        Ok(directive) => filter.add_directive(directive),
        Err(_) => filter,
    }
}

/// Correlation token shared by the launcher, host and backend processes. It is
/// not a credential, but it is copied verbatim into every record, so only a
/// short identifier-like value is accepted; anything else is ignored.
fn session_id(raw: Option<&str>) -> Option<String> {
    let id = raw?.trim();
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'));
    valid.then(|| id.to_owned())
}

/// Adds `"diagnostic_session_id"` as the first member of every JSON record, so
/// records stay attributable even when neighbouring records were dropped.
#[derive(Clone)]
struct SessionTagged<M> {
    inner: M,
    member: Option<std::sync::Arc<[u8]>>,
}

impl<M> SessionTagged<M> {
    fn new(inner: M, session: Option<String>) -> Self {
        let member = session.map(|id| {
            format!("{{\"diagnostic_session_id\":\"{id}\",")
                .into_bytes()
                .into()
        });
        Self { inner, member }
    }
}

struct SessionTaggedWriter<W> {
    inner: W,
    member: Option<std::sync::Arc<[u8]>>,
}

impl<W: Write> Write for SessionTaggedWriter<W> {
    fn write(&mut self, record: &[u8]) -> std::io::Result<usize> {
        // The JSON formatter hands over one complete record per write. Anything
        // else (or an empty object) passes through unchanged.
        match (&self.member, record.strip_prefix(b"{")) {
            (Some(member), Some(rest)) if !rest.starts_with(b"}") => {
                let mut tagged = Vec::with_capacity(member.len() + rest.len());
                tagged.extend_from_slice(member);
                tagged.extend_from_slice(rest);
                self.inner.write_all(&tagged)?;
            }
            _ => self.inner.write_all(record)?,
        }
        Ok(record.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, M: tracing_subscriber::fmt::MakeWriter<'a>> tracing_subscriber::fmt::MakeWriter<'a>
    for SessionTagged<M>
{
    type Writer = SessionTaggedWriter<M::Writer>;
    fn make_writer(&'a self) -> Self::Writer {
        SessionTaggedWriter {
            inner: self.inner.make_writer(),
            member: self.member.clone(),
        }
    }
}

/// Initialize JSON diagnostics after environment setup and before the GUI.
/// Libraries embedding the host may install their own subscriber instead.
pub fn init() -> Result<LogGuard> {
    let (output, guard) = writer(std::io::stderr());
    let dropped = output.error_counter();
    let guard = LogGuard(Some(guard));
    let session = session_id(
        std::env::var("NATIVEHTMLKIT_DIAGNOSTIC_SESSION_ID")
            .ok()
            .as_deref(),
    );
    let output = SessionTagged::new(output, session);
    tracing_subscriber::fmt()
        .with_env_filter(
            // RUST_LOG is the explicit expert override. Otherwise the shared SDK/CLI
            // flag controls the host logging level without a wire protocol change.
            match tracing_subscriber::EnvFilter::try_from_default_env() {
                Ok(filter) => {
                    with_backend_output(filter, &std::env::var("RUST_LOG").unwrap_or_default())
                }
                Err(_) => tracing_subscriber::EnvFilter::new(diagnostic_filter(
                    std::env::var("NATIVEHTMLKIT_DIAGNOSTICS")
                        .as_deref()
                        .unwrap_or("normal"),
                )),
            },
        )
        .json()
        .with_ansi(false)
        .with_writer(output)
        .try_init()
        .map_err(|error| anyhow::anyhow!("initialize structured logging: {error}"))?;
    let _ = SINK_DROPPED.set(dropped);
    Ok(guard)
}

/// One complete JSON record for a thread that is about to end the process and
/// therefore writes stderr directly instead of through the lossy sink. It carries
/// the session id like every other record.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn emergency_record(message: &str) -> String {
    let mut record = serde_json::Map::new();
    if let Some(id) = session_id(
        std::env::var("NATIVEHTMLKIT_DIAGNOSTIC_SESSION_ID")
            .ok()
            .as_deref(),
    ) {
        record.insert("diagnostic_session_id".into(), id.into());
    }
    record.insert("level".into(), "ERROR".into());
    record.insert(
        "fields".into(),
        serde_json::json!({"message":message,"event":"launcher.watchdog"}),
    );
    record.insert("target".into(), "nativehtmlkit_host::platform".into());
    serde_json::Value::Object(record).to_string()
}

/// Limit formatting/queue work per pump as well as the output queue itself.
/// Backend pipes are already continuously drained by their own reader threads.
pub fn backend_logs(lines: Vec<LogLine>) {
    let omitted = lines.len().saturating_sub(BACKEND_RECORDS_PER_PUMP);
    for line in lines.into_iter().take(BACKEND_RECORDS_PER_PUMP) {
        tracing::info!(target: BACKEND_OUTPUT_TARGET, source = "backend", stream = %line.stream, text = %line.text, "backend output");
    }
    if omitted != 0 {
        BACKEND_OMITTED.fetch_add(omitted as u64, Ordering::Relaxed);
        tracing::warn!(
            target: BACKEND_OUTPUT_TARGET,
            source = "backend",
            omitted_records = omitted,
            "backend log batch exceeded the GUI formatting budget"
        );
    }
}

pub fn diagnostics() -> serde_json::Value {
    serde_json::json!({
        "sink_dropped_records": SINK_DROPPED.get().map_or(0, ErrorCounter::dropped_lines),
        "backend_omitted_records": BACKEND_OMITTED.load(Ordering::Relaxed),
        "sink_capacity_records": BUFFERED_RECORDS,
        "backend_records_per_pump": BACKEND_RECORDS_PER_PUMP,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Instant;

    #[test]
    fn diagnostic_level_mapping_is_explicit_and_safe() {
        let relay = ",nativehtmlkit_host::logging=info";
        assert_eq!(
            diagnostic_filter("normal"),
            format!("nativehtmlkit_host=info{relay}")
        );
        assert_eq!(
            diagnostic_filter("DEBUG"),
            format!("nativehtmlkit_host=debug{relay}")
        );
        assert_eq!(
            diagnostic_filter("trace"),
            format!("nativehtmlkit_host=trace{relay}")
        );
        assert_eq!(
            diagnostic_filter("off"),
            format!("nativehtmlkit_host=error{relay}")
        );
        assert_eq!(
            diagnostic_filter("nonsense=trace"),
            format!("nativehtmlkit_host=info{relay}")
        );
    }

    /// NHK-CROSS-0007: at every level (and under an unrelated RUST_LOG) the relayed
    /// backend output passes the filter, while `off` still drops host info records.
    #[test]
    fn backend_output_passes_every_filter() {
        #[derive(Clone, Default)]
        struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let filters = ["off", "normal", "debug", "trace"]
            .map(|level| {
                (
                    level,
                    tracing_subscriber::EnvFilter::new(diagnostic_filter(level)),
                )
            })
            .into_iter()
            .chain([(
                "RUST_LOG=error",
                with_backend_output(tracing_subscriber::EnvFilter::new("error"), "error"),
            )]);
        for (name, filter) in filters {
            let sink = Sink::default();
            let captured = sink.clone();
            let subscriber = tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .with_writer(move || sink.clone())
                .finish();
            tracing::subscriber::with_default(subscriber, || {
                tracing::info!(target: "nativehtmlkit_host::host", "host-info-record");
                backend_logs(vec![LogLine {
                    stream: "stdout".into(),
                    text: "app-output-line".into(),
                }]);
            });
            let text = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
            assert!(text.contains("app-output-line"), "{name}: {text}");
            assert_eq!(
                text.contains("host-info-record"),
                !matches!(name, "off" | "RUST_LOG=error"),
                "{name}: {text}"
            );
        }
    }

    #[test]
    fn session_ids_are_identifier_like_or_ignored() {
        assert_eq!(
            session_id(Some(" nhk-12:ab.c_d ")).as_deref(),
            Some("nhk-12:ab.c_d")
        );
        assert_eq!(session_id(None), None);
        assert_eq!(session_id(Some("")), None);
        assert_eq!(session_id(Some("a\"b")), None);
        assert_eq!(session_id(Some("a b")), None);
        assert_eq!(session_id(Some(&"x".repeat(129))), None);
    }

    #[test]
    fn every_json_record_carries_the_session_id() {
        use tracing_subscriber::fmt::MakeWriter;
        #[derive(Clone, Default)]
        struct Sink(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let sink = Sink::default();
        let captured = sink.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_writer(SessionTagged::new(
                move || sink.clone(),
                Some("nhk-test-1".into()),
            ))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(answer = 42, "first");
            tracing::warn!("second");
        });
        let text = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        let records: Vec<serde_json::Value> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), 2);
        for record in &records {
            assert_eq!(record["diagnostic_session_id"], "nhk-test-1");
        }
        assert_eq!(records[0]["fields"]["answer"], 42);

        // Without a session the record is byte-for-byte what the formatter wrote.
        let mut plain = SessionTagged::new(Sink::default, None).make_writer();
        plain.write_all(b"{\"a\":1}\n").unwrap();
    }

    struct BlockedOutput {
        started: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
        first: bool,
    }
    impl Write for BlockedOutput {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.first {
                self.first = false;
                self.started.send(()).unwrap();
                self.release.recv().unwrap();
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_blocked_output_drops_excess_records_without_stalling_the_caller() {
        let (started, waiting) = mpsc::sync_channel(1);
        let (release, blocked) = mpsc::sync_channel(1);
        let (mut output, guard) = writer(BlockedOutput {
            started,
            release: blocked,
            first: true,
        });
        output.write_all(b"first\n").unwrap();
        waiting.recv_timeout(Duration::from_secs(2)).unwrap();
        let began = Instant::now();
        for _ in 0..1024 {
            output.write_all(b"busy backend\n").unwrap();
        }
        let elapsed = began.elapsed();
        let dropped = output.error_counter().dropped_lines();
        release.send(()).unwrap();
        drop(output);
        drop(LogGuard(Some(guard)));
        assert!(elapsed < Duration::from_secs(1));
        assert_eq!(dropped, 1024 - BUFFERED_RECORDS);
    }
}

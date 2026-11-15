//! One managed application backend. A fixed-size log ring drains pipes even
//! when the GUI is busy; user output cannot become protocol traffic.
use anyhow::Result;
use std::collections::VecDeque;
use std::io::Read;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const LOG_LINES: usize = 512;
const LINE_BYTES: usize = 4096;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LogLine {
    pub stream: String,
    pub text: String,
}

pub struct Backend {
    child: crate::platform::NativeChild,
    logs: Arc<Mutex<VecDeque<LogLine>>>,
    dropped: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
}

impl Backend {
    pub fn spawn(
        config: &crate::config::BackendConfig,
        endpoint: &str,
        token: &str,
        app_id: &str,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self> {
        let native = crate::platform::spawn_backend(config, endpoint, token, app_id)?;
        let logs = Arc::new(Mutex::new(VecDeque::with_capacity(LOG_LINES)));
        let dropped = Arc::new(AtomicU64::new(0));
        let stopped = Arc::new(AtomicBool::new(false));
        let token: Arc<str> = Arc::from(token);
        for (name, reader) in [("stdout", native.stdout), ("stderr", native.stderr)] {
            let token = token.clone();
            let logs = logs.clone();
            let dropped = dropped.clone();
            let stopped = stopped.clone();
            let wake = wake.clone();
            thread::Builder::new()
                .name(format!("nativehtmlkit-backend-{name}"))
                .spawn(move || drain(reader, name, &token, logs, dropped, stopped, wake))?;
        }
        Ok(Self {
            child: native.child,
            logs,
            dropped,
            stopped,
        })
    }
    pub fn id(&self) -> u32 {
        self.child.id()
    }
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        Ok(self.child.try_wait()?)
    }
    pub fn terminate(&mut self) -> Result<()> {
        Ok(self.child.terminate()?)
    }
    pub fn kill(&mut self) -> Result<()> {
        Ok(self.child.kill()?)
    }
    pub fn drain_logs(&self) -> Vec<LogLine> {
        let mut result: Vec<_> = self
            .logs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain(..)
            .collect();
        let count = self.dropped.swap(0, Ordering::Relaxed);
        if count != 0 {
            result.insert(0, LogLine { stream: "host".into(), text: format!("{count} backend log records discarded because the bounded log ring was full") });
        }
        result
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.child.kill();
    }
}

const REDACTED_TOKEN: &str = "[redacted-token]";

fn push(
    logs: &Mutex<VecDeque<LogLine>>,
    dropped: &AtomicU64,
    name: &str,
    token: &str,
    bytes: &[u8],
) {
    let mut text = String::from_utf8_lossy(bytes)
        .trim_end_matches('\r')
        .to_owned();
    // CORE-A-12: the session token must never reach the host log, even if the backend prints
    // its own environment.
    if !token.is_empty() && text.contains(token) {
        text = text.replace(token, REDACTED_TOKEN);
    }
    let mut queue = logs.lock().unwrap_or_else(|e| e.into_inner());
    if queue.len() == LOG_LINES {
        queue.pop_front();
        dropped.fetch_add(1, Ordering::Relaxed);
    }
    queue.push_back(LogLine {
        stream: name.to_owned(),
        text,
    });
}

/// Move a bounded-record split point back so that a token beginning near the end of the record
/// is carried whole into the next record (where `push` can redact it) instead of being cut.
fn token_safe_split(record: &[u8], token: &[u8]) -> usize {
    for overlap in (1..token.len().min(record.len())).rev() {
        if record.ends_with(&token[..overlap]) {
            return record.len() - overlap;
        }
    }
    record.len()
}

fn drain(
    mut reader: Box<dyn Read + Send>,
    name: &str,
    token: &str,
    logs: Arc<Mutex<VecDeque<LogLine>>>,
    dropped: Arc<AtomicU64>,
    stopped: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    let mut buffer = [0_u8; 8192];
    let mut line = Vec::with_capacity(LINE_BYTES);
    let mut last_wake = Instant::now() - Duration::from_secs(1);
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) => {
                for byte in &buffer[..length] {
                    if *byte == b'\n' {
                        push(&logs, &dropped, name, token, &line);
                        line.clear();
                    } else {
                        line.push(*byte);
                        if line.len() == LINE_BYTES {
                            // Carry an incomplete UTF-8 code point into the
                            // next bounded record instead of corrupting valid
                            // output at an arbitrary byte boundary.
                            let prefix = match std::str::from_utf8(&line) {
                                Err(error) if error.error_len().is_none() => error.valid_up_to(),
                                _ => line.len(),
                            };
                            let prefix = token_safe_split(&line[..prefix], token.as_bytes());
                            push(&logs, &dropped, name, token, &line[..prefix]);
                            line.drain(..prefix);
                        }
                    }
                }
                if last_wake.elapsed() >= Duration::from_millis(50) {
                    wake();
                    last_wake = Instant::now();
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
        if stopped.load(Ordering::Acquire) {
            break;
        }
    }
    if !line.is_empty() {
        push(&logs, &dropped, name, token, &line);
    }
    wake();
}

#[cfg(test)]
mod tests {
    use super::*;
    const TEST_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn drained(input: Vec<u8>) -> Vec<String> {
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        drain(
            Box::new(std::io::Cursor::new(input)),
            "stderr",
            TEST_TOKEN,
            logs.clone(),
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|| {}),
        );
        let lines = logs
            .lock()
            .unwrap()
            .iter()
            .map(|l| l.text.clone())
            .collect();
        lines
    }

    /// CORE-A-12: a backend that prints its environment must not copy the session token into the
    /// host log, including when the token straddles a bounded-record split.
    #[test]
    fn session_token_is_redacted_from_backend_output() {
        let lines = drained(
            format!("UI4ME_TOKEN={TEST_TOKEN}\nx {TEST_TOKEN}{TEST_TOKEN} y\nclean\n").into_bytes(),
        );
        assert_eq!(
            lines,
            [
                "UI4ME_TOKEN=[redacted-token]",
                "x [redacted-token][redacted-token] y",
                "clean"
            ]
        );
        for cut in [1, 20, 63] {
            let input = format!("{}{TEST_TOKEN}tail\n", "x".repeat(LINE_BYTES - cut));
            let lines = drained(input.into_bytes());
            let joined = lines.concat();
            assert!(!joined.contains(&TEST_TOKEN[..16]), "cut {cut}: {lines:?}");
            assert!(
                joined.contains("[redacted-token]tail"),
                "cut {cut}: {lines:?}"
            );
            assert!(
                lines.iter().all(|l| l.len() <= LINE_BYTES),
                "records stay bounded"
            );
        }
    }
    #[test]
    fn log_flood_is_drained_and_bounded_without_newlines() {
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let dropped = Arc::new(AtomicU64::new(0));
        let bytes = vec![b'x'; LINE_BYTES * (LOG_LINES + 8)];
        drain(
            Box::new(std::io::Cursor::new(bytes)),
            "stdout",
            TEST_TOKEN,
            logs.clone(),
            dropped.clone(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(|| {}),
        );
        assert_eq!(logs.lock().unwrap().len(), LOG_LINES);
        assert_eq!(dropped.load(Ordering::Relaxed), 8);
    }

    #[test]
    fn unicode_survives_a_record_byte_boundary() {
        let logs = Arc::new(Mutex::new(VecDeque::new()));
        let dropped = Arc::new(AtomicU64::new(0));
        let value = format!("{}é😀\n", "x".repeat(LINE_BYTES - 1));
        drain(
            Box::new(std::io::Cursor::new(value.as_bytes().to_vec())),
            "stdout",
            TEST_TOKEN,
            logs.clone(),
            dropped,
            Arc::new(AtomicBool::new(false)),
            Arc::new(|| {}),
        );
        let combined = logs
            .lock()
            .unwrap()
            .iter()
            .map(|line| line.text.as_str())
            .collect::<String>();
        assert_eq!(combined, value.trim_end_matches('\n'));
    }
}

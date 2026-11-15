//! Real binary/process tests without a display server or scripting dependency.
//!
//! The integration-test executable is also the managed backend fixture. Its
//! stdout remains ordinary logs; the same Unix socket / Windows pipe protocol
//! used by Python carries every application command.

use nativehtmlkit_host::config::{AppConfig, AppInfo, BackendConfig, HostConfig, InstancePolicy};
use nativehtmlkit_host::platform::NativeStream;
use nativehtmlkit_host::protocol::{self, RpcError};
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_CASE: AtomicU64 = AtomicU64::new(1);

struct RunningHost(Child);

impl Drop for RunningHost {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Harness {
    directory: tempfile::TempDir,
    app_id: String,
}

impl Harness {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let runtime = directory.path().join("runtime");
        fs::create_dir(&runtime).unwrap();
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            directory,
            app_id: format!(
                "org.nativehtmlkit.Integration.p{}.c{}",
                std::process::id(),
                NEXT_CASE.fetch_add(1, Ordering::Relaxed)
            ),
        }
    }

    fn run(&self, scenario: &str, expected_exit: i32) -> (Value, String) {
        self.run_with_diagnostics(scenario, expected_exit, None)
    }

    /// `diagnostics`: the user-facing `NATIVEHTMLKIT_DIAGNOSTICS` level and session id, used
    /// instead of the harness's explicit `RUST_LOG` override (which is then removed).
    fn run_with_diagnostics(
        &self,
        scenario: &str,
        expected_exit: i32,
        diagnostics: Option<(&str, &str)>,
    ) -> (Value, String) {
        let directory = self.directory.path();
        let evidence_path = directory.join("evidence.json");
        let _ = fs::remove_file(&evidence_path);
        let config = AppConfig {
            app: AppInfo {
                id: self.app_id.clone(),
                name: "Integration fixture".into(),
                version: None,
                icon: None,
            },
            backend: BackendConfig {
                program: std::env::current_exe().unwrap(),
                args: vec![
                    "--ignored".into(),
                    "--exact".into(),
                    "backend_fixture".into(),
                    "--nocapture".into(),
                    "--test-threads=1".into(),
                ],
                cwd: directory.to_owned(),
                env: BTreeMap::from([
                    (
                        "NATIVEHTMLKIT_INTEGRATION_SCENARIO".into(),
                        scenario.to_owned(),
                    ),
                    (
                        "NATIVEHTMLKIT_INTEGRATION_EVIDENCE".into(),
                        evidence_path.to_str().unwrap().to_owned(),
                    ),
                ]),
            },
            host: HostConfig {
                startup_timeout_ms: if scenario == "never_authenticate" {
                    1000
                } else {
                    5000
                },
                shutdown_timeout_ms: 500,
                max_windows: 4,
                instance: InstancePolicy::Single,
            },
        };
        let manifest = directory.join("app.toml");
        fs::write(&manifest, toml::to_string(&config).unwrap()).unwrap();
        let output_path = directory.join("host-output.log");
        let output = File::create(&output_path).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_nativehtmlkit-host"));
        command
            .args(["run", "--headless", "--app"])
            .arg(&manifest)
            .env("XDG_RUNTIME_DIR", directory.join("runtime"));
        match diagnostics {
            Some((level, session)) => command
                .env_remove("RUST_LOG")
                .env("NATIVEHTMLKIT_DIAGNOSTICS", level)
                .env("NATIVEHTMLKIT_DIAGNOSTIC_SESSION_ID", session),
            None => command.env("RUST_LOG", "nativehtmlkit_host=info"),
        };
        let mut host = RunningHost(
            command
                .stdin(Stdio::null())
                .stdout(Stdio::from(output.try_clone().unwrap()))
                .stderr(Stdio::from(output))
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = host.0.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = host.0.kill();
                let _ = host.0.wait();
                panic!(
                    "host did not finish {scenario} within 20 seconds:\n{}",
                    fs::read_to_string(&output_path).unwrap_or_default()
                );
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let output = fs::read_to_string(&output_path).unwrap();
        assert_eq!(
            status.code(),
            Some(expected_exit),
            "scenario {scenario}:\n{output}"
        );
        let evidence: Value =
            serde_json::from_str(&fs::read_to_string(&evidence_path).unwrap_or_else(|error| {
                panic!("fixture never reached its assertion checkpoint: {error}\n{output}")
            }))
            .unwrap();
        assert_eq!(evidence["scenario"], scenario);
        assert_eq!(
            evidence["checkpoint"], "complete",
            "fixture did not complete its checks:\n{output}"
        );
        (evidence, output)
    }
}

#[test]
fn quit_veto_is_atomic_and_nested_rpc_keeps_working() {
    let harness = Harness::new();
    harness.run("transactional_quit", 0);
    // Same app ID/runtime directory must launch again after normal shutdown.
    harness.run("transactional_quit", 0);
}

#[test]
fn authentication_and_malformed_requests_do_not_grant_control() {
    Harness::new().run("authentication", 0);
}

#[test]
fn authenticated_disconnect_is_a_failure_and_releases_the_instance() {
    let harness = Harness::new();
    let (_, output) = harness.run("disconnect", 1);
    assert!(output.contains("Backend connection lost"), "{output}");
    harness.run("authentication", 0);
}

#[test]
fn backend_crash_is_reported_and_releases_the_instance() {
    let harness = Harness::new();
    let (_, output) = harness.run("crash", 1);
    assert!(output.contains("application failed"), "{output}");
    harness.run("authentication", 0);
}

#[test]
fn oversized_authenticated_frame_fails_without_waiting_for_a_body() {
    let (_, output) = Harness::new().run("oversized_frame", 1);
    assert!(
        output.contains("frame length") && output.contains("exceeds"),
        "{output}"
    );
}

#[test]
fn unauthenticated_backend_is_stopped_at_the_startup_deadline() {
    let start = Instant::now();
    let (_, output) = Harness::new().run("never_authenticate", 1);
    assert!(start.elapsed() < Duration::from_secs(10));
    assert!(output.contains("startup deadline"), "{output}");
}

#[test]
fn activation_during_primary_startup_is_forwarded_after_ready() {
    let harness = Harness::new();
    let manifest = harness.directory.path().join("app.toml");
    let runtime = harness.directory.path().join("runtime");
    let secondary = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !manifest.exists() {
            assert!(
                Instant::now() < deadline,
                "primary manifest was not written"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        // The fixture holds app.ready for two seconds; launch inside that window.
        std::thread::sleep(Duration::from_millis(500));
        let started = Instant::now();
        let status = Command::new(env!("CARGO_BIN_EXE_nativehtmlkit-host"))
            .args(["run", "--headless", "--app"])
            .arg(&manifest)
            .env("XDG_RUNTIME_DIR", &runtime)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        (status, started.elapsed())
    });
    harness.run("slow_ready_activation", 0);
    let (status, waited) = secondary.join().unwrap();
    assert!(status.success(), "secondary launch failed: {status}");
    assert!(
        waited >= Duration::from_millis(500),
        "secondary returned before the primary was ready"
    );
}

#[test]
fn startup_window_can_be_closed_before_the_backend_becomes_ready() {
    Harness::new().run("startup_close", 0);
}

/// NHK-CROSS-0002: a backend-initiated close of one of several windows before `app.ready`
/// closes only that window; startup continues and the app quits normally (exit 0).
#[test]
fn backend_close_of_a_secondary_window_before_ready_keeps_startup() {
    Harness::new().run("startup_close_secondary", 0);
}

/// NHK-CROSS-0003 (documented v1 behaviour): an allowed close of the last usable window
/// starts shutdown without a `window.event {type:"closed"}` for that window.
#[test]
fn last_window_close_shuts_down_without_a_closed_event() {
    Harness::new().run("last_window_close", 0);
}

/// Value that must never appear in host diagnostics, at any level.
const TRACE_SENTINEL: &str = "nhk-sentinel-3f9a17-do-not-log";

fn records(output: &str) -> Vec<Value> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("non-JSON host record {line:?}: {error}"))
        })
        .collect()
}

fn with_event<'a>(records: &'a [Value], event: &str) -> Vec<&'a Value> {
    records
        .iter()
        .filter(|record| record["fields"]["event"] == event)
        .collect()
}

/// NHK-CROSS-0004: at `trace` the host records RPC/notification metadata (never values);
/// at `normal` none of these records exist.
#[test]
fn trace_records_rpc_and_event_metadata_without_values() {
    let harness = Harness::new();
    let session = format!("nhk-it-trace.{}", std::process::id());
    let (evidence, output) =
        harness.run_with_diagnostics("trace_coverage", 0, Some(("trace", &session)));
    let token = evidence["token"].as_str().unwrap();
    assert!(
        !output.contains(TRACE_SENTINEL),
        "sentinel leaked:\n{output}"
    );
    assert!(!output.contains(token), "launch token leaked");
    let all = records(&output);
    for record in &all {
        assert_eq!(
            record["diagnostic_session_id"],
            session.as_str(),
            "{record}"
        );
    }
    let received = with_event(&all, "rpc.received");
    let completed = with_event(&all, "rpc.completed");
    // The sink is lossy by design; this scenario is far below its 128-record bound.
    for (id, method) in evidence["calls"].as_object().unwrap() {
        let matches = |records: &[&Value]| {
            records
                .iter()
                .filter(|r| r["fields"]["id"] == id.as_str() && r["fields"]["method"] == *method)
                .count()
        };
        assert_eq!(
            matches(&received),
            1,
            "rpc.received {id} {method}:\n{output}"
        );
        assert_eq!(
            matches(&completed),
            1,
            "rpc.completed {id} {method}:\n{output}"
        );
    }
    for record in &completed {
        assert!(record["fields"]["duration_us"].is_u64(), "{record}");
    }
    let failed = evidence["failed_call"].as_str().unwrap();
    assert!(
        completed
            .iter()
            .any(|r| r["fields"]["id"] == failed
                && r["fields"]["error_code"] == protocol::code::CLOSED),
        "error outcome not recorded:\n{output}"
    );
    let notifications = with_event(&all, "notify.sent");
    assert!(
        notifications
            .iter()
            .any(|r| r["fields"]["method"] == "document.event"
                && r["fields"]["type"] == "submit"
                && r["fields"]["window_id"].is_string()
                && r["fields"]["document_id"].is_string()),
        "submit event not visible:\n{output}"
    );
    assert!(
        notifications
            .iter()
            .any(|r| r["fields"]["method"] == "app.shutdown"),
        "{output}"
    );
    assert!(
        with_event(&all, "request.sent")
            .iter()
            .any(|r| r["fields"]["method"] == "app.before_quit"),
        "{output}"
    );
    assert!(
        !with_event(&all, "lifecycle.phase").is_empty(),
        "lifecycle transitions missing:\n{output}"
    );

    let (_, output) = harness.run_with_diagnostics("trace_coverage", 0, Some(("normal", &session)));
    assert!(!output.contains(TRACE_SENTINEL), "{output}");
    let normal = records(&output);
    for event in [
        "rpc.received",
        "rpc.completed",
        "notify.sent",
        "request.sent",
        "lifecycle.phase",
    ] {
        assert!(
            with_event(&normal, event).is_empty(),
            "{event} at normal:\n{output}"
        );
    }
}

struct Client {
    stream: NativeStream,
    next_id: u64,
    received: VecDeque<Value>,
    /// Request id -> method of every call this fixture made.
    calls: BTreeMap<String, String>,
}

impl Client {
    fn connect() -> Self {
        let endpoint = std::env::var("UI4ME_ENDPOINT").unwrap();
        Self {
            stream: NativeStream::connect(&endpoint, Duration::from_secs(5)).unwrap(),
            next_id: 1,
            received: VecDeque::new(),
            calls: BTreeMap::new(),
        }
    }

    fn send(&mut self, value: &Value) {
        self.stream
            .set_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        protocol::write_frame(&mut self.stream, value).unwrap();
    }

    fn receive(&mut self) -> Value {
        self.stream
            .set_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        protocol::read_frame(&mut self.stream)
            .unwrap()
            .expect("host disconnected prematurely")
    }

    fn call_result(&mut self, method: &str, params: Value) -> Result<Value, RpcError> {
        let id = format!("fixture:{}", self.next_id);
        self.next_id += 1;
        self.calls.insert(id.clone(), method.to_owned());
        self.send(&protocol::request(&id, method, params));
        loop {
            let value = self.receive();
            if value.get("id").and_then(Value::as_str) == Some(&id) && value.get("method").is_none()
            {
                return protocol::parse_response(value).unwrap().outcome;
            }
            assert!(
                self.received.len() < 512,
                "fixture notification queue exceeded its bound"
            );
            self.received.push_back(value);
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        self.call_result(method, params)
            .unwrap_or_else(|error| panic!("{method}: {error}"))
    }

    fn method(&mut self, name: &str) -> Value {
        loop {
            if let Some(index) = self
                .received
                .iter()
                .position(|value| value.get("method").and_then(Value::as_str) == Some(name))
            {
                return self.received.remove(index).unwrap();
            }
            let value = self.receive();
            assert!(self.received.len() < 512);
            self.received.push_back(value);
        }
    }

    fn decide(&mut self, decision: &Value, allow: bool) {
        self.send(&protocol::success(
            decision["id"].as_str().unwrap(),
            json!({"allow":allow}),
        ));
    }

    fn hello(&mut self) {
        let hello = self.call(
            "host.hello",
            json!({"protocol_version":1,"token":std::env::var("UI4ME_TOKEN").unwrap()}),
        );
        assert_eq!(hello["protocol_version"], 1);
        assert_eq!(hello["capabilities"]["headless"], true);
        assert_eq!(hello["capabilities"]["native"], false);
    }

    fn start(&mut self, count: usize) -> Vec<String> {
        self.hello();
        let windows = (0..count)
            .map(|index| {
                self.call("window.create", json!({"title":format!("Fixture {index}")}))["window_id"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        self.call("app.ready", json!({}));
        windows
    }

    fn quit(&mut self) {
        self.call("app.request_quit", json!({}));
        let decision = self.method("app.before_quit");
        self.decide(&decision, true);
        let shutdown = self.method("app.shutdown");
        assert_eq!(shutdown["params"]["deadline_ms"], 500);
    }
}

fn checkpoint(scenario: &str) {
    checkpoint_with(scenario, json!({}));
}

fn checkpoint_with(scenario: &str, mut extra: Value) {
    extra["scenario"] = json!(scenario);
    extra["checkpoint"] = json!("complete");
    extra["backend_pid"] = json!(std::process::id());
    fs::write(
        PathBuf::from(std::env::var_os("NATIVEHTMLKIT_INTEGRATION_EVIDENCE").unwrap()),
        serde_json::to_vec(&extra).unwrap(),
    )
    .unwrap();
}

/// The host starts only this ignored test, in a separate managed process.
#[test]
#[ignore = "managed subprocess fixture; exercised by the host integration tests"]
fn backend_fixture() {
    let Ok(scenario) = std::env::var("NATIVEHTMLKIT_INTEGRATION_SCENARIO") else {
        return;
    };
    let mut client = Client::connect();
    match scenario.as_str() {
        "transactional_quit" => {
            let windows = client.start(2);
            client.call("window.request_close", json!({"window_id":windows[0]}));
            let pending_close = client.method("window.before_close");
            // This RPC happens while a host -> backend close callback is
            // outstanding. A synchronous callback design would deadlock here.
            client.call(
                "window.set",
                json!({"window_id":windows[0],"title":"Saved while deciding"}),
            );
            assert_eq!(
                client
                    .call("window.list", json!({}))
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            client.call("app.request_quit", json!({}));
            let pending_quit = client.method("app.before_quit");
            let cancelled = client.method("request.cancelled");
            assert_eq!(cancelled["params"]["id"], pending_close["id"]);
            // A late answer to the superseded per-window request cannot
            // partially close the application after global quit is vetoed.
            client.decide(&pending_close, true);
            client.decide(&pending_quit, false);
            assert_eq!(
                client
                    .call("window.list", json!({}))
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            assert_eq!(
                client.call("host.diagnostics", json!({}))["phase"],
                "running"
            );
            client.call("window.request_close", json!({"window_id":windows[0]}));
            let close = client.method("window.before_close");
            client.decide(&close, false);
            assert_eq!(
                client
                    .call("window.list", json!({}))
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
            client.call("window.request_close", json!({"window_id":windows[0]}));
            let close = client.method("window.before_close");
            client.decide(&close, true);
            assert_eq!(
                client
                    .call("window.list", json!({}))
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            client.call("window.request_close", json!({"window_id":windows[1]}));
            let last_close = client.method("window.before_close");
            client.decide(&last_close, true);
            let last_quit = client.method("app.before_quit");
            client.decide(&last_quit, false);
            assert_eq!(
                client
                    .call("window.list", json!({}))
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            client.call("app.request_quit", json!({}));
            let quit = client.method("app.before_quit");
            assert_eq!(
                client
                    .call_result("window.create", json!({}))
                    .unwrap_err()
                    .code,
                protocol::code::BUSY
            );
            client.decide(&quit, true);
            client.method("app.shutdown");
            checkpoint(&scenario);
        }
        "slow_ready_activation" => {
            client.hello();
            client.call("window.create", json!({}));
            std::thread::sleep(Duration::from_secs(2));
            client.call("app.ready", json!({}));
            let activated = client.method("app.activated");
            assert!(activated["params"]["args"].is_array(), "{activated}");
            checkpoint(&scenario);
            client.quit();
        }
        "authentication" => {
            assert_eq!(
                client
                    .call_result("window.create", json!({}))
                    .unwrap_err()
                    .code,
                protocol::code::UNAUTHENTICATED
            );
            assert_eq!(
                client
                    .call_result("host.hello", json!({"protocol_version":2,"token":"wrong"}))
                    .unwrap_err()
                    .code,
                protocol::code::VERSION_MISMATCH
            );
            assert_eq!(
                client
                    .call_result("host.hello", json!({"protocol_version":1,"token":"wrong"}))
                    .unwrap_err()
                    .code,
                protocol::code::UNAUTHENTICATED
            );
            client.send(&json!({"jsonrpc":"2.0","id":42,"method":"window.create","params":{}}));
            let invalid = client.receive();
            assert!(invalid["id"].is_null());
            assert_eq!(invalid["error"]["code"], protocol::code::INVALID_REQUEST);
            client.send(
                &json!([{"jsonrpc":"2.0","id":"batch","method":"window.create","params":{}}]),
            );
            assert_eq!(
                client.receive()["error"]["code"],
                protocol::code::INVALID_REQUEST
            );
            client.start(1);
            assert_eq!(
                client
                    .call("window.list", json!({}))
                    .as_array()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                client
                    .call_result("dialog.open_files", json!({"window_id":"w1"}))
                    .unwrap_err()
                    .code,
                protocol::code::UNSUPPORTED
            );
            for _ in 0..3 {
                client.call("window.create", json!({}));
            }
            assert_eq!(
                client
                    .call_result("window.create", json!({}))
                    .unwrap_err()
                    .code,
                protocol::code::BUSY
            );
            client.quit();
            checkpoint(&scenario);
        }
        "startup_close" => {
            client.hello();
            let window = client.call("window.create", json!({}));
            client.call(
                "window.request_close",
                json!({"window_id":window["window_id"]}),
            );
            client.method("app.shutdown");
            checkpoint(&scenario);
        }
        "startup_close_secondary" => {
            client.hello();
            let main = client.call("window.create", json!({"title":"main"}))["window_id"].clone();
            let extra = client.call("window.create", json!({"title":"extra"}))["window_id"].clone();
            client.call("window.request_close", json!({"window_id":extra}));
            let closed = loop {
                let event = client.method("window.event");
                if event["params"]["window_id"] == extra {
                    break event;
                }
            };
            assert_eq!(closed["params"]["type"], "closed", "{closed}");
            let list = client.call("window.list", json!({}));
            let ids: Vec<_> = list
                .as_array()
                .unwrap()
                .iter()
                .map(|w| w["window_id"].clone())
                .collect();
            assert_eq!(ids, vec![main], "{list}");
            client.call("app.ready", json!({}));
            assert_eq!(
                client.call("host.diagnostics", json!({}))["phase"],
                "running"
            );
            checkpoint(&scenario);
            client.quit();
        }
        "last_window_close" => {
            let windows = client.start(1);
            client.call("window.request_close", json!({"window_id":windows[0]}));
            let close = client.method("window.before_close");
            client.decide(&close, true);
            let quit = client.method("app.before_quit");
            assert_eq!(quit["params"]["reason"], "last_window", "{quit}");
            client.decide(&quit, true);
            client.method("app.shutdown");
            let types: Vec<_> = client
                .received
                .iter()
                .filter(|m| m["method"] == "window.event")
                .map(|m| m["params"]["type"].clone())
                .collect();
            assert_eq!(
                types,
                vec![json!("close_requested")],
                "no closed event for the last window"
            );
            // app.shutdown implies every window is closed: handles are stale from here on.
            assert_eq!(
                client
                    .call_result("window.get", json!({"window_id":windows[0]}))
                    .unwrap_err()
                    .code,
                protocol::code::INVALID_STATE
            );
            checkpoint(&scenario);
        }
        "trace_coverage" => {
            client.hello();
            let window =
                client.call("window.create", json!({"title":"trace"}))["window_id"].clone();
            let document = client.call(
                "document.load",
                json!({"window_id":window,"html":"<form id='f'><input id='n' name='n' value='x'><button id='b' type='submit'>Go</button></form>"}),
            )["document_id"].clone();
            client.call("app.ready", json!({}));
            client.call(
                "document.command",
                json!({"window_id":window,"document_id":document,"command":{"op":"request_submit","element":"f"}}),
            );
            loop {
                let event = client.method("document.event");
                if event["params"]["event"]["type"] == "submit" {
                    break;
                }
            }
            client.call(
                "window.set",
                json!({"window_id":window,"title":TRACE_SENTINEL}),
            );
            let failed_call = format!("fixture:{}", client.next_id);
            assert_eq!(
                client
                    .call_result("window.get", json!({"window_id":"missing"}))
                    .unwrap_err()
                    .code,
                protocol::code::CLOSED
            );
            client.call("window.list", json!({}));
            client.quit();
            checkpoint_with(
                &scenario,
                json!({"calls":client.calls,"failed_call":failed_call,
                    "token":std::env::var("UI4ME_TOKEN").unwrap()}),
            );
        }
        "disconnect" => {
            client.start(1);
            checkpoint(&scenario);
            client.stream.shutdown();
            std::thread::sleep(Duration::from_secs(30));
            panic!("host failed to terminate disconnected backend");
        }
        "crash" => {
            client.start(1);
            checkpoint(&scenario);
            std::process::exit(23);
        }
        "oversized_frame" => {
            client.start(1);
            checkpoint(&scenario);
            client.stream.write_all(&u32::MAX.to_be_bytes()).unwrap();
            client.stream.flush().unwrap();
            std::thread::sleep(Duration::from_secs(30));
            panic!("host waited for the body of an oversized frame");
        }
        "never_authenticate" => {
            assert_eq!(
                client
                    .call_result("host.hello", json!({"protocol_version":1,"token":"wrong"}))
                    .unwrap_err()
                    .code,
                protocol::code::UNAUTHENTICATED
            );
            checkpoint(&scenario);
            std::thread::sleep(Duration::from_secs(30));
            panic!("startup deadline did not stop an unauthenticated backend");
        }
        other => panic!("unknown fixture scenario: {other}"),
    }
}

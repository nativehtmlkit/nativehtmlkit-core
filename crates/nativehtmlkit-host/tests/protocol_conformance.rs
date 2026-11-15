//! Protocol v1 runtime conformance against the compiled host binary.
//!
//! The test executable is also the backend: the real `nativehtmlkit-host run --headless`
//! spawns it, it authenticates over the real Unix socket / Windows named pipe, and drives
//! every client method, every `document.command` op, and the envelope edge cases
//! (unknown members, duplicate keys, `id:null`, batches, notifications, malformed frames).
//!
//! Observations are reduced to stable facts (ok + result member names, or the error code)
//! and compared with the committed golden file `protocol/v1/conformance/golden.json`.
//! A difference fails the test. To intentionally change the contract, regenerate with
//! `NATIVEHTMLKIT_CONFORMANCE_BLESS=1 cargo test -p nativehtmlkit-host --test protocol_conformance`
//! and review the diff. Headless results never qualify native GUI behavior.

use nativehtmlkit_host::config::{AppConfig, AppInfo, BackendConfig, HostConfig, InstancePolicy};
use nativehtmlkit_host::platform::NativeStream;
use nativehtmlkit_host::protocol;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, VecDeque};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SCENARIO_ENV: &str = "NATIVEHTMLKIT_CONFORMANCE_EVIDENCE";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn protocol_v1_conformance_matches_golden() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = directory.path().join("runtime");
    fs::create_dir(&runtime).unwrap();
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let evidence = directory.path().join("observed.json");
    let config = AppConfig {
        app: AppInfo {
            id: format!("org.nativehtmlkit.Conformance.p{}", std::process::id()),
            name: "Conformance fixture".into(),
            version: None,
            icon: None,
        },
        backend: BackendConfig {
            program: std::env::current_exe().unwrap(),
            args: vec![
                "--ignored".into(),
                "--exact".into(),
                "conformance_backend".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            cwd: directory.path().to_owned(),
            env: BTreeMap::from([(SCENARIO_ENV.into(), evidence.to_str().unwrap().to_owned())]),
        },
        host: HostConfig {
            startup_timeout_ms: 10_000,
            shutdown_timeout_ms: 2_000,
            max_windows: 4,
            instance: InstancePolicy::Single,
        },
    };
    let manifest = directory.path().join("app.toml");
    fs::write(&manifest, toml::to_string(&config).unwrap()).unwrap();
    let log_path = directory.path().join("host.log");
    let log = File::create(&log_path).unwrap();
    let mut host = Command::new(env!("CARGO_BIN_EXE_nativehtmlkit-host"))
        .args(["run", "--headless", "--app"])
        .arg(&manifest)
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("RUST_LOG", "nativehtmlkit_host=info")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        if let Some(status) = host.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            let _ = host.kill();
            panic!(
                "host did not finish:\n{}",
                fs::read_to_string(&log_path).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let output = fs::read_to_string(&log_path).unwrap_or_default();
    assert_eq!(status.code(), Some(0), "host exit status:\n{output}");
    let observed: Value = serde_json::from_slice(&fs::read(&evidence).unwrap_or_else(|e| {
        panic!("backend fixture did not record observations ({e}):\n{output}")
    }))
    .unwrap();

    let golden_path = repo_root().join("protocol/v1/conformance/golden.json");
    let rendered = serde_json::to_string_pretty(&observed).unwrap() + "\n";
    if std::env::var_os("NATIVEHTMLKIT_CONFORMANCE_BLESS").is_some() {
        fs::create_dir_all(golden_path.parent().unwrap()).unwrap();
        fs::write(&golden_path, &rendered).unwrap();
        return;
    }
    let golden: Value = serde_json::from_str(
        &fs::read_to_string(&golden_path)
            .expect("golden file missing; run with NATIVEHTMLKIT_CONFORMANCE_BLESS=1"),
    )
    .unwrap();
    let (want, got) = (
        golden["cases"].as_object().unwrap(),
        observed["cases"].as_object().unwrap(),
    );
    let mut differences = Vec::new();
    for key in want
        .keys()
        .chain(got.keys())
        .collect::<std::collections::BTreeSet<_>>()
    {
        if want.get(key) != got.get(key) {
            differences.push(format!(
                "{key}:\n    golden:   {}\n    observed: {}",
                want.get(key).map_or("<absent>".into(), Value::to_string),
                got.get(key).map_or("<absent>".into(), Value::to_string)
            ));
        }
    }
    assert!(
        differences.is_empty(),
        "{} protocol v1 conformance difference(s):\n{}",
        differences.len(),
        differences.join("\n")
    );
}

// ---------------------------------------------------------------- backend fixture

struct Client {
    stream: NativeStream,
    next: u64,
    queued: VecDeque<Value>,
    cases: Map<String, Value>,
}

/// Reduce a response to stable, contract-level facts.
fn summarize(response: &Value) -> Value {
    if let Some(error) = response.get("error") {
        // Request IDs are positional and not contract; whether an error is uncorrelated
        // (`id: null`) is.
        if response["id"].is_null() {
            json!({"error": error["code"], "id": null})
        } else {
            json!({"error": error["code"]})
        }
    } else {
        let result = &response["result"];
        match result {
            Value::Object(map) => {
                json!({"ok": "object", "members": map.keys().collect::<Vec<_>>()})
            }
            Value::Array(_) => json!({"ok": "array"}),
            other => json!({"ok": kind(other)}),
        }
    }
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

impl Client {
    fn connect() -> Self {
        let endpoint = std::env::var("UI4ME_ENDPOINT").unwrap();
        Self {
            stream: NativeStream::connect(&endpoint, Duration::from_secs(5)).unwrap(),
            next: 0,
            queued: VecDeque::new(),
            cases: Map::new(),
        }
    }
    fn raw(&mut self, body: &[u8]) {
        self.stream
            .set_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        self.stream
            .write_all(&(body.len() as u32).to_be_bytes())
            .unwrap();
        self.stream.write_all(body).unwrap();
        self.stream.flush().unwrap();
    }
    fn receive(&mut self) -> Value {
        self.stream
            .set_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        protocol::read_frame(&mut self.stream)
            .unwrap()
            .expect("host disconnected")
    }
    /// Next response (a message without `method`), answering host decision requests
    /// and queueing notifications.
    fn response(&mut self) -> Value {
        loop {
            let message = self.receive();
            if let Some(method) = message.get("method").and_then(Value::as_str) {
                if message.get("id").is_some() {
                    // Answer host->client requests so lifecycle never stalls; veto by default.
                    let allow = method == "app.before_quit"
                        && self.cases.contains_key("lifecycle.quit.requested");
                    let reply =
                        protocol::success(message["id"].as_str().unwrap(), json!({"allow": allow}));
                    protocol::write_frame(&mut self.stream, &reply).unwrap();
                }
                self.queued.push_back(message);
                continue;
            }
            return message;
        }
    }
    fn call_raw(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = format!("c{}", self.next);
        protocol::write_frame(&mut self.stream, &protocol::request(&id, method, params)).unwrap();
        let response = self.response();
        assert_eq!(response["id"], json!(id), "uncorrelated response");
        response
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        let response = self.call_raw(method, params);
        assert!(
            response.get("error").is_none(),
            "{method} failed: {response}"
        );
        response["result"].clone()
    }
    fn case(&mut self, name: &str, method: &str, params: Value) -> Value {
        let response = self.call_raw(method, params);
        self.cases.insert(name.into(), summarize(&response));
        response
    }
    /// A raw frame whose reply is correlated by position (the only request in flight).
    fn raw_case(&mut self, name: &str, body: &str) {
        self.raw(body.as_bytes());
        let response = self.response();
        self.cases.insert(name.into(), summarize(&response));
    }
    /// A notification never gets a response: prove it by a following request whose
    /// response must be the very next response frame.
    fn notification_case(&mut self, name: &str, body: &str) {
        self.raw(body.as_bytes());
        let probe = self.call_raw("host.diagnostics", json!({}));
        let quiet = probe["id"] == json!(format!("c{}", self.next));
        self.cases
            .insert(name.into(), json!({"no_response": quiet}));
    }
    fn wait_notification(&mut self, method: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(i) = self.queued.iter().position(|m| m["method"] == method) {
                return self.queued.remove(i).unwrap();
            }
            assert!(Instant::now() < deadline, "no {method}");
            let message = self.receive();
            self.queued.push_back(message);
        }
    }
}

fn token() -> String {
    std::env::var("UI4ME_TOKEN").unwrap()
}

/// Sample arguments that make each op meaningful against the fixture document. Fields the
/// generated schema marks required get values; optional fields are omitted.
fn op_sample(op: &str, field: &str, schema: &Value, node: &str, sheet: u64) -> Value {
    let element = match op {
        "show_dialog" | "close_dialog" | "get_dialog" => "dlg",
        "show_popover" | "hide_popover" | "toggle_popover" | "get_popover" => "pop",
        "get_options" | "set_selected_values" => "sel",
        "get_files" => "file",
        "check_validity" | "reset_form" | "request_submit" | "get_form_data" => "form",
        "get_checked" | "set_checked" => "chk",
        "focus" | "edit_command" => "txt",
        "get_value"
        | "set_value"
        | "get_validity"
        | "set_custom_validity"
        | "replace_control_range"
        | "set_control_selection"
        | "get_control_selection" => "txt",
        "move_element" => "moving",
        "remove_element" => "removable",
        _ => "probe",
    };
    match field {
        "element" => json!(element),
        "target" => json!("probe"),
        "node" => json!(node),
        "sheet" => json!(sheet),
        "name" if op.contains("resource_bundle") => json!("conformance"),
        "name" if op.contains("class") => json!("marked"),
        "name" => json!("title"),
        "value" => json!("value"),
        "values" => json!(["b"]),
        "text" | "replacement" => json!("text"),
        "html" => json!("<span>inserted</span>"),
        "position" => json!("beforeend"),
        "selector" => json!("div"),
        "css" => json!("#probe{color:red}"),
        "event" => json!("click"),
        "command" => json!("select_all"),
        "message" => json!(""),
        "properties" => json!(["color"]),
        "entries" => json!([{"path":"a.css","data_url":"data:text/css,a{}"}]),
        "ops" => json!([]),
        "keep" => json!(1),
        _ => match schema.get("type").and_then(Value::as_str) {
            Some("integer") => json!(0),
            Some("number") => json!(5.0),
            Some("boolean") => json!(true),
            Some("array") => json!([]),
            _ => json!("x"),
        },
    }
}

const FIXTURE_HTML: &str = r#"<!doctype html><html><body>
<div id="probe" title="t">Probe <b>text</b> for selection</div>
<form id="form"><input id="txt" name="t" required value="v"><input id="chk" type="checkbox" name="c" checked>
<select id="sel" name="s" multiple><option value="a">A</option><option value="b">B</option></select>
<input id="file" type="file" name="f"><button id="submit">Go</button></form>
<dialog id="dlg">dialog</dialog><div id="pop" popover>popover</div>
<div id="moving">m</div><div id="removable">r</div>
<div id="scroller" style="height:40px;overflow:auto"><div style="height:400px">tall</div></div>
</body></html>"#;

#[test]
#[ignore = "managed subprocess fixture; exercised by protocol_v1_conformance_matches_golden"]
fn conformance_backend() {
    let Some(evidence) = std::env::var_os(SCENARIO_ENV) else {
        return;
    };
    let mut c = Client::connect();

    // --- authentication
    c.case("auth.before_hello.window.list", "window.list", json!({}));
    c.case(
        "auth.hello.wrong_version",
        "host.hello",
        json!({"token":token(),"protocol_version":2}),
    );
    c.case(
        "auth.hello.wrong_token",
        "host.hello",
        json!({"token":"0".repeat(64),"protocol_version":1}),
    );
    c.case(
        "auth.hello.missing_token",
        "host.hello",
        json!({"protocol_version":1}),
    );
    c.case(
        "auth.hello.ok_with_unknown_member",
        "host.hello",
        json!({"token":token(),"protocol_version":1,"client":{"name":"conformance"},"extra":true}),
    );
    c.case(
        "auth.hello.again",
        "host.hello",
        json!({"token":token(),"protocol_version":1}),
    );

    // --- envelope (one frame in flight at a time; replies correlate by position)
    // Authenticated frames without a `method` member are treated as responses to host
    // requests; an unparseable one (a batch array included) is logged and dropped.
    c.notification_case(
        "envelope.batch_array",
        r#"[{"jsonrpc":"2.0","id":"b1","method":"window.list"}]"#,
    );
    c.notification_case(
        "envelope.object_without_method",
        r#"{"jsonrpc":"2.0","id":"b2"}"#,
    );
    c.raw_case(
        "envelope.id_number",
        r#"{"jsonrpc":"2.0","id":7,"method":"window.list"}"#,
    );
    c.raw_case(
        "envelope.id_null",
        r#"{"jsonrpc":"2.0","id":null,"method":"window.list"}"#,
    );
    c.raw_case(
        "envelope.id_empty",
        r#"{"jsonrpc":"2.0","id":"","method":"window.list"}"#,
    );
    c.raw_case(
        "envelope.id_control_char",
        "{\"jsonrpc\":\"2.0\",\"id\":\"a\\u0001\",\"method\":\"window.list\"}",
    );
    c.raw_case(
        "envelope.id_too_long",
        &format!(
            r#"{{"jsonrpc":"2.0","id":"{}","method":"window.list"}}"#,
            "i".repeat(129)
        ),
    );
    c.raw_case(
        "envelope.jsonrpc_missing",
        r#"{"id":"e1","method":"window.list"}"#,
    );
    c.raw_case(
        "envelope.jsonrpc_1_0",
        r#"{"jsonrpc":"1.0","id":"e2","method":"window.list"}"#,
    );
    c.raw_case(
        "envelope.unknown_member",
        r#"{"jsonrpc":"2.0","id":"e3","method":"window.list","extra":1}"#,
    );
    c.raw_case(
        "envelope.params_array",
        r#"{"jsonrpc":"2.0","id":"e4","method":"window.list","params":[]}"#,
    );
    c.raw_case(
        "envelope.params_null",
        r#"{"jsonrpc":"2.0","id":"e5","method":"window.list","params":null}"#,
    );
    c.raw_case(
        "envelope.params_omitted",
        r#"{"jsonrpc":"2.0","id":"e6","method":"window.list"}"#,
    );
    c.raw_case(
        "envelope.method_rpc_reserved",
        r#"{"jsonrpc":"2.0","id":"e7","method":"rpc.discover"}"#,
    );
    c.raw_case(
        "envelope.method_bad_chars",
        r#"{"jsonrpc":"2.0","id":"e8","method":"window list"}"#,
    );
    c.raw_case(
        "envelope.method_number",
        r#"{"jsonrpc":"2.0","id":"e9","method":5}"#,
    );
    c.raw_case(
        "envelope.method_unknown",
        r#"{"jsonrpc":"2.0","id":"e10","method":"nope.nope"}"#,
    );
    // Duplicate keys: serde_json keeps the last occurrence.
    c.raw_case(
        "envelope.duplicate_method_key_last_wins",
        r#"{"jsonrpc":"2.0","id":"e11","method":"nope.nope","method":"host.capabilities"}"#,
    );
    c.raw_case(
        "envelope.duplicate_id_key_last_wins",
        r#"{"jsonrpc":"2.0","id":7,"id":"e12","method":"window.list"}"#,
    );
    c.notification_case(
        "envelope.notification_unknown_method",
        r#"{"jsonrpc":"2.0","method":"nope.nope"}"#,
    );
    c.notification_case(
        "envelope.notification_invalid_params",
        r#"{"jsonrpc":"2.0","method":"window.get","params":{"x":1}}"#,
    );
    c.notification_case(
        "envelope.response_from_client_unsolicited",
        r#"{"jsonrpc":"2.0","id":"zz","result":{}}"#,
    );

    // A second, unauthenticated connection cannot act, and its malformed frame ends only
    // that connection, never the authenticated session.
    {
        let mut intruder = NativeStream::connect(
            &std::env::var("UI4ME_ENDPOINT").unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
        intruder.set_timeout(Some(Duration::from_secs(10))).unwrap();
        protocol::write_frame(
            &mut intruder,
            &protocol::request("x1", "window.list", json!({})),
        )
        .unwrap();
        let reply = protocol::read_frame(&mut intruder).unwrap().unwrap();
        c.cases.insert(
            "security.second_connection.window.list".into(),
            summarize(&reply),
        );
        protocol::write_frame(
            &mut intruder,
            &protocol::request(
                "x2",
                "host.hello",
                json!({"token":token(),"protocol_version":1}),
            ),
        )
        .unwrap();
        let reply = protocol::read_frame(&mut intruder).unwrap().unwrap();
        c.cases.insert(
            "security.second_connection.hello_with_valid_token".into(),
            summarize(&reply),
        );
        intruder.write_all(&[0, 0, 0, 3, b'{', b'x', b'}']).unwrap();
        intruder.flush().unwrap();
        let after = protocol::read_frame(&mut intruder);
        c.cases.insert(
            "security.second_connection.malformed_json_frame".into(),
            json!({"connection_closed": !matches!(after, Ok(Some(_)))}),
        );
    }
    let phase = c.call("host.diagnostics", json!({}))["phase"].clone();
    c.cases.insert(
        "security.session_survives_intruder".into(),
        json!({"phase": phase}),
    );

    // --- method matrix: valid, unknown member, wrong type
    c.case("app.ready.before_window", "app.ready", json!({}));
    let w = c.case(
        "window.create.ok",
        "window.create",
        json!({"title":"Conformance","width":640,"height":480}),
    );
    let window = w["result"]["window_id"].as_str().unwrap().to_owned();
    c.case("window.create.defaults", "window.create", json!({}));
    c.case(
        "window.create.unknown_member",
        "window.create",
        json!({"title":"x","bogus":1}),
    );
    c.case(
        "window.create.wrong_type",
        "window.create",
        json!({"width":"wide"}),
    );
    c.case("app.ready.ok", "app.ready", json!({}));
    c.case("app.ready.again", "app.ready", json!({}));
    c.case("host.capabilities.ok", "host.capabilities", json!({}));
    c.case(
        "host.capabilities.unknown_member_ignored",
        "host.capabilities",
        json!({"anything":1}),
    );
    c.case("host.diagnostics.ok", "host.diagnostics", json!({}));
    c.case("window.list.ok", "window.list", json!({}));
    c.case("window.get.ok", "window.get", json!({"window_id":window}));
    c.case(
        "window.get.unknown_window",
        "window.get",
        json!({"window_id":"nope"}),
    );
    c.case("window.get.missing_member", "window.get", json!({}));
    c.case(
        "window.get.unknown_member",
        "window.get",
        json!({"window_id":window,"x":1}),
    );
    c.case(
        "window.get.duplicate_member_last_wins",
        "window.get",
        json!({"window_id":window}),
    );
    c.raw_case(
        "window.get.duplicate_member_raw",
        &format!(r#"{{"jsonrpc":"2.0","id":"dup1","method":"window.get","params":{{"window_id":"nope","window_id":"{window}"}}}}"#),
    );
    c.case(
        "window.set.ok",
        "window.set",
        json!({"window_id":window,"title":"Renamed"}),
    );
    c.case(
        "window.set.unknown_member",
        "window.set",
        json!({"window_id":window,"opacity":0.5}),
    );
    c.case(
        "window.request_attention.ok",
        "window.request_attention",
        json!({"window_id":window,"kind":"informational"}),
    );
    c.case(
        "probe.set_text.ok",
        "probe.set_text",
        json!({"window_id":window,"text":"probe"}),
    );
    c.case(
        "probe.get_text.ok",
        "probe.get_text",
        json!({"window_id":window}),
    );
    c.case("desktop.paths.ok", "desktop.paths", json!({}));
    c.case("desktop.preferences.ok", "desktop.preferences", json!({}));
    c.case(
        "desktop.open_external.headless",
        "desktop.open_external",
        json!({"target":"https://example.invalid/"}),
    );
    c.case(
        "desktop.reveal_file.relative_path",
        "desktop.reveal_file",
        json!({"path":"relative.txt"}),
    );
    c.case(
        "clipboard.write_text.headless",
        "clipboard.write_text",
        json!({"text":"x"}),
    );
    c.case(
        "clipboard.write_text.unknown_member",
        "clipboard.write_text",
        json!({"text":"x","y":1}),
    );
    c.case(
        "clipboard.read_text.headless",
        "clipboard.read_text",
        json!({}),
    );
    c.case(
        "dialog.open_files.headless",
        "dialog.open_files",
        json!({"window_id":window}),
    );
    c.case(
        "dialog.save_file.headless",
        "dialog.save_file",
        json!({"window_id":window}),
    );
    c.case(
        "dialog.choose_folder.headless",
        "dialog.choose_folder",
        json!({"window_id":window}),
    );
    c.case(
        "dialog.open_files.unknown_member",
        "dialog.open_files",
        json!({"window_id":window,"z":1}),
    );
    c.case(
        "request.cancel.unknown_id",
        "request.cancel",
        json!({"id":"never"}),
    );
    c.case("request.cancel.missing_member", "request.cancel", json!({}));

    // --- documents
    c.case(
        "document.get.empty",
        "document.get",
        json!({"window_id":window}),
    );
    c.case(
        "document.load.neither_source",
        "document.load",
        json!({"window_id":window}),
    );
    c.case(
        "document.load.both_sources",
        "document.load",
        json!({"window_id":window,"html":"<p>","path":"x.html"}),
    );
    c.case(
        "document.load.unknown_member",
        "document.load",
        json!({"window_id":window,"html":"<p>","x":1}),
    );
    c.case(
        "document.load.oversize_html",
        "document.load",
        json!({"window_id":window,"html":"a".repeat(1024*1024+1)}),
    );
    let loaded = c.case(
        "document.load.ok",
        "document.load",
        json!({"window_id":window,"html":FIXTURE_HTML}),
    );
    let document = loaded["result"]["document_id"].as_str().unwrap().to_owned();
    c.case(
        "document.get.loaded",
        "document.get",
        json!({"window_id":window}),
    );
    c.case("document.command.stale_document", "document.command", json!({"window_id":window,"document_id":"stale","command":{"op":"get_text","element":"probe"}}));
    c.case(
        "document.command.unknown_op",
        "document.command",
        json!({"window_id":window,"document_id":document,"command":{"op":"no_such_op"}}),
    );
    c.case(
        "document.command.missing_op",
        "document.command",
        json!({"window_id":window,"document_id":document,"command":{}}),
    );
    c.case(
        "document.command.unknown_outer_member",
        "document.command",
        json!({"window_id":window,"document_id":document,"command":{"op":"get_environment"},"x":1}),
    );

    // --- every document op from the generated schema
    let schema: Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("protocol/v1/schemas/document-command.schema.json"))
            .unwrap(),
    )
    .unwrap();
    let node = c.call(
        "document.command",
        json!({"window_id":window,"document_id":document,"command":{"op":"node_of","element":"probe"}}),
    );
    let node = node
        .get("node")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let sheet = c.call(
        "document.command",
        json!({"window_id":window,"document_id":document,"command":{"op":"add_stylesheet","css":"p{}"}}),
    );
    let sheet = sheet.get("sheet").and_then(Value::as_u64).unwrap_or(0);
    let defs = schema["$defs"].as_object().unwrap();
    let mut ops: Vec<_> = defs
        .keys()
        .filter_map(|k| k.strip_prefix("op_"))
        .map(str::to_owned)
        .collect();
    // Destructive ops last so earlier ops still see the fixture elements.
    ops.sort_by_key(|op| {
        (
            matches!(
                op.as_str(),
                "remove_element"
                    | "replace_children_html"
                    | "remove_stylesheet"
                    | "unregister_resource_bundle"
            ),
            op.clone(),
        )
    });
    for op in ops {
        let def = &defs[&format!("op_{op}")];
        let required: Vec<&str> = def["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let mut command = Map::new();
        for field in &required {
            command.insert(
                (*field).into(),
                if *field == "op" {
                    json!(op)
                } else {
                    op_sample(&op, field, &def["properties"][*field], &node, sheet)
                },
            );
        }
        let base = json!({"window_id":window,"document_id":document});
        let mut params = base.clone();
        params["command"] = Value::Object(command.clone());
        c.case(
            &format!("op.{op}.required_only"),
            "document.command",
            params,
        );
        let mut extra = command.clone();
        extra.insert("zz_unknown".into(), json!(1));
        let mut params = base.clone();
        params["command"] = Value::Object(extra);
        c.case(
            &format!("op.{op}.unknown_member"),
            "document.command",
            params,
        );
        let optional_target =
            def["properties"].get("element").is_some() && !required.contains(&"element");
        if optional_target {
            let mut targeted = command.clone();
            targeted.insert(
                "element".into(),
                op_sample(&op, "element", &json!({}), &node, sheet),
            );
            let mut params = json!({"window_id":window,"document_id":document});
            params["command"] = Value::Object(targeted);
            c.case(&format!("op.{op}.with_element"), "document.command", params);
        }
        if required.len() > 1 {
            let mut missing = command.clone();
            missing.remove(required[1]);
            let mut params = base;
            params["command"] = Value::Object(missing);
            c.case(
                &format!("op.{op}.missing_required"),
                "document.command",
                params,
            );
        }
    }

    // --- bitmap
    c.case("bitmap.create.bad_format", "bitmap.create", json!({"window_id":window,"document_id":document,"bitmap_id":"b","width":2,"height":2,"pixel_format":"BGRA"}));
    c.case("bitmap.create.ok", "bitmap.create", json!({"window_id":window,"document_id":document,"bitmap_id":"b","width":2,"height":2,"pixel_format":"RGBA8"}));
    c.case(
        "bitmap.close.ok",
        "bitmap.close",
        json!({"window_id":window,"document_id":document,"bitmap_id":"b"}),
    );
    c.case(
        "bitmap.close.again",
        "bitmap.close",
        json!({"window_id":window,"document_id":document,"bitmap_id":"b"}),
    );
    c.case(
        "document.unload.ok",
        "document.unload",
        json!({"window_id":window,"document_id":document}),
    );
    c.case(
        "document.unload.again",
        "document.unload",
        json!({"window_id":window,"document_id":document}),
    );

    // --- lifecycle: veto, then accept; post-shutdown messages
    c.case(
        "window.request_close.ok",
        "window.request_close",
        json!({"window_id":window}),
    );
    let close = c.wait_notification("window.before_close");
    c.cases.insert(
        "lifecycle.window.before_close.params".into(),
        json!(close["params"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>()),
    );
    c.cases
        .insert("lifecycle.quit.requested".into(), json!(true));
    c.case("app.request_quit.ok", "app.request_quit", json!({}));
    let before = c.wait_notification("app.before_quit");
    c.cases.insert(
        "lifecycle.app.before_quit.params".into(),
        json!(before["params"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>()),
    );
    let shutdown = c.wait_notification("app.shutdown");
    c.cases.insert(
        "lifecycle.app.shutdown.params".into(),
        json!(shutdown["params"]
            .as_object()
            .unwrap()
            .keys()
            .collect::<Vec<_>>()),
    );
    c.case(
        "lifecycle.after_shutdown.window.list",
        "window.list",
        json!({}),
    );
    c.case(
        "lifecycle.after_shutdown.host.diagnostics",
        "host.diagnostics",
        json!({}),
    );

    let observed = json!({
        "schema": 1,
        "protocol_version": 1,
        "host_version": env!("CARGO_PKG_VERSION"),
        "mode": "headless; native GUI behavior is qualified separately",
        "cases": c.cases,
    });
    fs::write(evidence, serde_json::to_vec_pretty(&observed).unwrap()).unwrap();
}

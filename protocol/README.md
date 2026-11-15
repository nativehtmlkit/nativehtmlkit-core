# NativeHTMLKit host protocol, version 1

**Status: v1 wire contract locked as a release candidate (`1.0-rc`), not yet declared public-stable.**
The method/op matrix and envelope behavior below are generated from the Rust handlers and locked
by a runtime golden that passes identically against the compiled host on Linux (Unix socket) and
Windows (named pipe). The *stable* declaration additionally requires the open native gates in
[`CONFORMANCE-ROADMAP.md`](CONFORMANCE-ROADMAP.md) (IME, DPI scaling, screen-reader journeys and
native dialogs/clipboard/drag-drop GUI evidence). Until then SDK authors may build against v1, but
must treat the items listed there as unqualified.

| Artifact | What it is | Authority |
|---|---|---|
| [`v1/method-matrix.json`](v1/method-matrix.json) | Every message: direction, kind, Rust param type, unknown-member policy | Generated from Rust (`scripts/generate-protocol-contract.py`) |
| [`v1/schemas/methods/*.params.schema.json`](v1/schemas/methods) | JSON Schema (2020-12) per client→host method | Generated from the serde types |
| [`v1/schemas/document-command.schema.json`](v1/schemas/document-command.schema.json) | All 81 `document.command` ops (`oneOf`, tag `op`) | Generated from `DocumentCommand` |
| [`v1/conformance/golden.json`](v1/conformance/golden.json) | 327 observed outcomes (result members or error code) | Runtime: `crates/nativehtmlkit-host/tests/protocol_conformance.rs` |
| [`COMPATIBILITY.md`](COMPATIBILITY.md) | Versioning and compatibility policy | Project decision |
| [`SDK-AUTHORING.md`](SDK-AUTHORING.md) | How to write an independent client | Guide |

`cargo test -p nativehtmlkit-host --test protocol_conformance` fails on any behavioral drift;
`python3 scripts/generate-protocol-contract.py --check` (run by `scripts/check-architecture.py`)
fails when the Rust types change without regenerating the schemas.

## 1. Launch and transport

The host is started with `nativehtmlkit-host run --app <app.toml>`. It creates a private endpoint
(Linux: a Unix stream socket in a `0700` runtime directory; Windows: a named pipe whose DACL grants only the current
logon-session SID, with remote clients rejected) and spawns the manifest's backend with exactly these environment variables:

| Variable | Meaning |
|---|---|
| `UI4ME_ENDPOINT` | socket path / pipe name |
| `UI4ME_TOKEN` | 64 hex characters, single-use launch credential |
| `UI4ME_APP_ID` | manifest `app.id` |
| `UI4ME_PROTOCOL_VERSION` | `1` |

These **v1 names are permanent for protocol v1** (see `COMPATIBILITY.md`). A client must remove them
from its own environment after reading, and must never pass them to child processes or logs.
A manifest's `[backend.env]` may not set the four transport names (in either the `UI4ME_` or
`NATIVEHTMLKIT_` spelling) or any `UI4ME_HOST_*`/`NATIVEHTMLKIT_HOST_*` key; such manifests are
rejected at load (case-insensitively). On Linux the host runs under a transient `systemd --user` unit
(`KillMode=control-group`) bound to its launcher; on Windows the backend runs in a kill-on-close Job
Object. A launcher that must not be outlived by the host should bind the host to itself
(Linux `PR_SET_PDEATHSIG`, Windows Job Object) — the CLI does.

## 2. Framing

Each frame is a 4-byte **unsigned big-endian length** followed by exactly that many bytes of one
**UTF-8 JSON object**. Length 0 and lengths above **2,097,152** bytes are fatal. A frame that is not
valid UTF-8 or JSON, or a truncated header/body, **ends the connection without a response**
(no `-32700` is ever sent); for the authenticated backend this fails the application. The host never
writes a partial frame. Notification payloads that would exceed the limit are replaced by a
`document.error`.

## 3. Envelope (JSON-RPC 2.0 subset)

Requests: `{"jsonrpc":"2.0","id":<string>,"method":<string>,"params":<object>}`; notifications omit
`id`. Locked behaviour:

| Input | Outcome |
|---|---|
| `id` not a string, empty, > 128 bytes, or containing control characters (including `id:null`) | error `-32600`, response `id: null` |
| `jsonrpc` missing or not `"2.0"`; unknown top-level member; `method` not a string, empty, > 128 bytes, outside `[A-Za-z0-9._-]`, or starting with `rpc.` | error `-32600`, `id: null` |
| `params` omitted | treated as `{}` |
| `params` is an array or `null` | error `-32602`, `id: null` |
| Unknown method | error `-32601` (correlated `id`, `data.method`) |
| Batch (top-level array) or any object **without** `method` on the authenticated connection | treated as a response to a host request; if it is not one it is logged and **dropped silently** |
| Notification (no `id`) that fails | no response |
| Duplicate object keys (envelope or params) | last occurrence wins (serde_json) |
| A request ID reused while the original is pending | fatal: the session is failed |

Clients must therefore send **unique string IDs** and never batches. At most 128 requests may be pending.

## 4. Authentication

The first request on a connection must be `host.hello` with `{"token":…, "protocol_version":1}`
(other members, e.g. `client`, are ignored). Results: wrong version `-32010`, wrong/missing token
`-32011`, any other method first `-32011`, a second hello `-32005`. Only one authenticated connection
exists; other connections can only call `app.activate` (secondary-instance hand-off) and receive
`-32011` otherwise. A malformed frame on an unauthenticated connection closes only that connection.
Success returns `{protocol_version, host_version, app_id, capabilities}`.

## 5. Error codes

| Code | Name | Typical cause |
|---|---|---|
| -32600 | invalid request | envelope violation (see §3) |
| -32601 | method not found | unknown method |
| -32602 | invalid params | serde decode failure, unknown member on a decoding method/op, semantic argument error |
| -32603 | internal | unexpected host failure |
| -32001 | unsupported | feature unavailable here (e.g. headless clipboard/dialogs/external open, Wayland attention) |
| -32002 | unavailable | service missing at run time |
| -32003 | denied | policy refusal |
| -32004 | closed | window/document no longer exists (stale `document_id`) |
| -32005 | busy | limits reached, duplicate hello, frozen during quit, `document.load` HTML over 1 MiB |
| -32006 | cancelled | request cancelled |
| -32007 | disconnected | |
| -32008 | invalid state | wrong lifecycle phase (`app.ready` without a window or twice; any call after shutdown begins except `host.capabilities`, `host.diagnostics`, `request.cancel`, `app.force_quit`) |
| -32009 | timeout | |
| -32010 | version mismatch | `host.hello` protocol_version ≠ 1 |
| -32011 | unauthenticated | before/without a valid `host.hello` |

Clients must treat unknown codes as failures and must not parse `message` text.

## 6. Parameters and unknown members

Every decoding method and every `document.command` op with parameters rejects unknown members with
`-32602` (`deny_unknown_fields`). **Exceptions locked for v1:** methods that do not decode params
(`host.capabilities`, `host.diagnostics`, `app.ready`, `app.request_quit`, `app.force_quit`,
`window.list`, `desktop.paths`, `desktop.preferences`, `clipboard.read_text`) ignore params, and the
**11 parameterless document ops** (`clear_selection`, `copy_selection`, `css_profile`,
`get_accessibility_tree`, `get_diagnostics`, `get_environment`, `get_focused_element`,
`get_pointer_capture`, `get_resource_stats`, `get_selection`, `list_stylesheets`) ignore extra
members (serde does not apply `deny_unknown_fields` to unit variants of an internally tagged enum).
Ops whose `element`/`node` are both optional in the schema may still require one at run time
(`focus`, `get_form_data`, `reset_form`, `request_submit`, `explain`, `get_control_selection`,
`set_control_selection`, `replace_control_range`, `set_custom_validity`, `get_computed_style`,
`edit_command`); the golden records both forms.

## 7. Lifecycle

`host.hello` → `window.create` (≥1) → `document.load` → `app.ready` → events → quit → shutdown.

Host→client **requests** (the client must answer with `{"allow": true|false}`; anything else vetoes):
`window.before_close {window_id, reason}`, `app.before_quit {reason}`. Closing the last window asks
`app.before_quit` with reason `last_window`. Host→client **notifications**: `app.shutdown
{deadline_ms}` (exit before the deadline), `window.event`, `document.event {window_id, document_id,
event}`, `document.error`, `request.cancelled {id, reason}`, `app.activated`,
`desktop.preferences_changed`. A `document.event` carries `type`, `element` (nearest ancestor `id`),
`path` (identified bubbling route) and type-specific members; clients should match their target
against `element` *or* `path`.

## 8. Markup and resource names

`ui4me-res://`, `ui4me-bitmap://` and `data-ui4me-{drag,no-drag,resize,drop}` remain valid in v1.
`nativehtmlkit-res://`, `nativehtmlkit-bitmap://` and `data-nativehtmlkit-*` are additive aliases of
the same objects (bundle replacement invalidates both spellings). Wire-visible identifiers such as
the CSS profile (`ui4me-desktop-1.0`, `ui4me-css/1`), the diagnostics schema (`ui4me-diagnostics`)
and paint-tier ids keep their v1 values.

There is no JavaScript, HTTP loading or browser engine; markup is parsed and rendered by the host.

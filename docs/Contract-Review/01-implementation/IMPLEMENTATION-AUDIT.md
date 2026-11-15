# Part I — AS-IS contract reconstructed from actual code

**Methodology:** Search and inspect Rust dispatch, enum decoding and result paths, manifest config, lifecycle, transport, OS backends, renderer entry points, existing Python and PowerShell SDKs, tests and release workflows in the uploaded source snapshot. The machine catalogue can be regenerated using `META/rebuild-extraction.py`. A line reference is to a **file in this archive**, not the potentially changed GitHub main branch.

## A. Wire envelope: concrete implementation

`crates/ui4me-host/src/protocol.rs`:

- L13–L20: `PROTOCOL_VERSION=1`; `MAX_FRAME_BYTES=2,097,152`; max pending 128; request ID and method limit 128 bytes; outgoing budget 8 MiB.
- L52–L116: 4-byte **big-endian unsigned body length** followed by that many UTF-8 JSON bytes, one JSON value, no batch arrays; reject zero length, oversize, truncated header/body, invalid UTF-8/JSON. The parser uses `serde_json::from_str::<Value>`; it does not itself reject duplicate JSON object keys.
- L249–L284: JSON-RPC 2.0 **subset**: object, `jsonrpc:"2.0"`, optional **string** ID for notifications, method ASCII alnum/`._-` excluding `rpc.*`, params object or absent. Numeric/null request IDs and positional params rejected. Top-level unexpected members rejected.
- L290–L310: the *client-side Rust response parser* requires a nonempty string `id`, exactly one `result` or `error`, and disallows unexpected envelope keys. Host uses `protocol::error(None, ...)` for invalid inbound requests, which serializes `id:null` (see `host/mod.rs` near L707–L720). **Cross-direction inconsistency must be resolved** when defining SDK behavior.
- `RpcError` and named codes: `PARSE_ERROR=-32700`; standard `-32600` through `-32603`; internal server code range `-32001…-32011`. `-32007` represents disconnected client-side failure and is not necessarily a host on-wire error.

**Normative trap:** “JSON-RPC 2.0” by itself is insufficient. The host intentionally rejects standard JSON-RPC batching, positional parameters and numeric request IDs. The public contract must call this a **restricted JSON-RPC 2.0 profile** and must not claim full JSON-RPC 2.0 compatibility. Spec reference: https://www.jsonrpc.org/specification.

## B. Transport and authentication

Source anchors: `crates/ui4me-host/src/transport.rs` L1–L25, L108–L230 and L265–L450; `crates/ui4me-host/src/host/mod.rs` L360–L385, L700–L850; `platform/linux.rs` near L603–L610 and `platform/windows.rs` L805–L813.

- **Linux:** private AF_UNIX socket; path derived from app/session binding, with private per-user directory and restrictive file/directory modes. User session and systemd/Wayland integration are platform-specific; Core must document exact prerequisites.
- **Windows:** local byte-stream named pipe protected for the current logon session, rejecting remote pipe clients; Windows desktop instance and job-object lifecycle matter.
- Host-generated token is 32 cryptographically random bytes represented as 64 hex digits; delivered to the spawned backend via environment, not a command-line argument.
- Host accepts up to **8 transport connections**, but only **one** is admitted as authenticated application backend by `host.hello`. Nonauthenticated connections time out; read-side first-frame deadline is 15s and host-side unauthenticated connection retention is about 5s.
- No HTTP socket or TCP API; no public remote-control endpoint. Third-party SDKs must implement the **host-launched backend model**, not an undocumented external attach operation.
- Host and backend share a full-duplex channel. The host generates request IDs for reverse decisions. Separate reader and writer, nonblocking callback dispatch and prompt reverse responses are required.
- `host.hello` checks numeric `protocol_version == 1` and token. It returns `protocol_version`, `host_version`, `app_id`, `capabilities`. It currently ignores unknown param keys and even optional `client` details; no feature-range negotiation yet. A notification-form hello without an ID could change lifecycle without an acknowledgment — an invalid usage that is not currently rejected in that special handler.
- Host-to-backend environment currently uses exactly `UI4ME_ENDPOINT`, `UI4ME_TOKEN`, `UI4ME_APP_ID`, `UI4ME_PROTOCOL_VERSION`. These are part of the **real bootstrap contract**, despite the planned rebrand.

## C. Startup manifest and host executable

Source: `crates/ui4me-host/src/config.rs` L13–L72, L75–L255; `crates/ui4me-host/src/main.rs` L7–L118; `crates/ui4me-host/src/backend.rs` L1–L150.

Manifest TOML has `[app]` (`id`, `name`, optional `version`, `icon`), `[backend]` (`program`, `args`, `cwd`, `env`) and `[host]` (`startup_timeout_ms`, `shutdown_timeout_ms`, `max_windows`, `instance=single|multiple`). Manifest at most **256 KiB**, strict unknown-field rejection; args at most **256**, per argument at most **32,768 UTF-8 bytes**, backend environment entries at most **128**; deadlines validated by host. App ID validation controls desktop identity.

Executable commands: `run --app <manifest>`, `probe`, `render --html/--output/--width/--height/--scale`, and `doctor`. `probe` and `render` are host diagnostic CLI features **not** general SDK RPC methods. `render` produces diagnostic PNG; it is not a public live screenshot/export API over JSON-RPC.

Host owns the app backend process, captures stdout/stderr into bounded log rings (`backend.rs`), manages containment (Linux user service/process group and Windows job object), and exposes no direct lossless backend-to-shell stdout channel. This is why independent `nhk select` must own stdout in the separate CLI launcher.

## D. Lifecycle and reverse RPC

Source: `crates/ui4me-host/src/lifecycle.rs` L1–L120 and L148–L300; `host/mod.rs` L700–L1008; `host/requests.rs` L212–L299.

```text
starting → connecting ─(host.hello)→ initializing ─(app.ready)→ running
running ─(close/quit proposal)→ quit_pending ─(allow)→ quitting → stopped
                                          └(deny)→ running
error/disconnect/deadline → failed → stopped
```

- `app.ready` requires at least one window. The host waits for the SDK/backend to finish initialization.
- `window.request_close` or native close results in `window.before_close`, and final-window close may trigger `app.before_quit`. The SDK answers both with `{"allow": true|false}`. Stale decisions can be cancelled; host emits `request.cancelled`.
- `app.shutdown` notifies the backend and the host enforces a bounded shutdown deadline.
- `app.force_quit` bypasses cooperative decisions; Ctrl+Shift+Q also forces shutdown.
- In failed/stopped/quitting, most requests reject with invalid-state; capabilities/diagnostics/request cancellation/force-quit remain allowed by the code path.
- **No reconnect/resume protocol** exists for detached clients after a backend process disconnect. Backend disconnect is an app-level failure.

## E. Dispatchers and implemented application operations

Source: `crates/ui4me-host/src/host/requests.rs` L212–L404 and methods below; `host/documents.rs` L178–L330; `host/bitmap.rs` L198–L310; `host/document_commands.rs` L16–L335 and L694–L1114. Full names/locations: [`host-methods.md`](host-methods.md) and [`document-operations.md`](document-operations.md).

1. **Host / application:** hello, capability report, diagnostics, ready, request/force quit; secondary-launch activation as pre-auth special case.
2. **Windows:** create, get/list/set, request-close, request-attention. Host returns requested and separately observed state, allowing absent observed values. Per-window min/max dimensions; platform limitations.
3. **Documents:** load string or path, get/unload, command. One document per window; **16 live docs max per host**, 1 MiB HTML input, generation-safe document IDs, asset-root canonicalization. Rejected load preserves previous document.
4. **DOM, styles, forms and events:** nested tagged enum implements text/HTML mutation, CSS query/change, node query/access and generation-checked node handles, event defaults, selection, scroll, form validation/submission, popovers/dialogs, resource stats/bundles, a11y snapshot, diagnostics, CSS profile and explanation. **81 op variants** exist, but a simple list of names is not a complete behavior contract.
5. **Bitmaps:** `bitmap.create/present/close`, shared file with multiple slots, RGBA8, monotonically increasing presentation sequence, 4096² max per bitmap, 8 per doc, 64 MiB/doc. The user-facing markup scheme `ui4me-bitmap://` remains hard-coded in renderer pipeline resources; new schemes need compatibility work.
6. **Native services:** text clipboard, standard app paths/preferences, external open, reveal file, file/folder dialogs. Async operations can take time; do not block application event dispatch while waiting.
7. **Diagnostics:** `probe.get_text`, `probe.set_text`, `host.diagnostics`, `get_diagnostics`, `css_profile`, `explain`. Useful to developers, but not all should carry strict public compatibility forever.

**Limits and semantic facts that a schema alone does not encode:** paired min/max constraints, non-finite coordinates rejected, Windows/Wayland differences, 1..4096 query limit, 1..1024 batch operation limit, max 64 dynamic stylesheets, text and other resource budgets, raw HTML insertion safety, stale handles, async response/error timing, filesystem identity and OS service failures.

## F. Host-emitted events and fallibility

`host/document_native.rs` around L209–L245 builds `document.event`; if encoded payload exceeds the 2 MiB frame limit, it emits `document.error` and drops the original event. `host/mod.rs` around L464–L485 queues notifications and can disconnect when a peer cannot accept bounded output; transport outgoing queue is bounded to 128 messages / 8 MiB, incoming to 256 events / 8 MiB (`transport.rs`). Notification delivery is **not durable**.

The SDK must not assume every click/scroll/text event can always be retained or replayed; a robust future contract needs explicit event-loss state notification and resync semantics. For now, treat `document.error` as potentially meaning loss and allow polling authoritative state on relevant objects.

## G. Existing Python and PowerShell clients

**Python:** `python/ui4me_core/client.py` implements independent socket/named-pipe framing, request IDs, reader/writer and callback worker. `application.py` exposes `Application`, windows, documents, events and a raw command escape hatch; `launcher.py` finds a bundled native executable or explicit development override; `_bootstrap.py` implements self-launch/host/backend dual roles. `setup.py` builds the Rust executable into a Python platform wheel when an existing artifact is not supplied. Packaging and source build must be separated in NativeHTMLKit.

**PowerShell:** `powershell/UI4ME.Core/UI4ME.Core.psm1` contains the PowerShell-facing API and `private/UI4ME.Transport.cs` is a **runtime-compiled C# helper** via `Add-Type`. It is *not* a pure `.psm1` implementation. It bundles host binaries extracted from Python wheels in current `tools/release/build_psgallery.py`. Keep the tested C# helper initially; decouple distribution sources from Python wheels and separately test Windows PowerShell 5.1 / PowerShell 7. The generic raw RPC command means the wire can expose more than the set of high-level cmdlet wrappers.

**Coverage terminology:** A Rust method present in the dispatcher is *wire-exposed*. A wrapper method in an SDK is *SDK-exposed*. A renderer method not dispatched via host RPC is *internal*. Do **not** conflate these. Different language SDKs need not have identical ergonomic API names, but should implement the same stable wire semantics.

## H. Functionality that exists inside Core but is deliberately not a public RPC

Examples from `crates/ui4me-renderer/src/document.rs`:

- `Document::render` / `render_into`: invoked inside host/renderer; no general RPC returning a live frame/screenshot stream.
- `handle_input`, `set_viewport`, native/accessibility action dispatch: host/platform concerns; not safe to expose directly without an authorization/use-case design.
- `set_clipboard`, resource poll/worker handles, system preference setters and renderer pipeline internals: internal lifecycle control.
- Diagnostic `render` from host CLI is an explicitly separate interface, not permission for any language client to drive the internal renderer directly.
- Current HTML profile rejects JavaScript, network resource retrieval, full browser APIs, video/audio playback, printing and GPU canvas/WebGL; the future SDK contract must not imply those capabilities exist.

This is a **positive boundary**: a public protocol should be intentionally narrower than Core's internal Rust API. Avoid blindly exposing every public Rust method over RPC.

## I. “Current spec” versus “public stable spec” gap

The structural inventory enumerates accepted operation names and syntax but cannot, by itself, freeze each method's result and semantic guarantees. Required work before graduation:

- Response shapes for every host method and document operation, including optional/null semantics, error data and platform variants.
- Complete notification and reverse-request payload/response schemas, event ordering, cancellation and recovery rules.
- Conformance test cases for valid/invalid params, lifecycle, resource ownership and state-dependent errors.
- Formal capability IDs and host-version feature introduction/removal history.
- Explicit compatibility of legacy bootstrap names, resource schemes, manifest fields and app identity.
- Proof of real desktop behavior on supported platforms, including accessibility and IME gates.

**Source-of-truth policy:** Rust source is authoritative for AS-IS. After acceptance, the canonical contract must be source-checked as part of Rust/Core changes; do not allow prose documentation alone to define SDK guarantees.

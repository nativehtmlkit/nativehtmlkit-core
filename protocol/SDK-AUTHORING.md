# Writing an independent NativeHTMLKit client (SDK authoring guide)

A client is a **backend process launched by the host**; the host owns windows, rendering, input and
accessibility. Use only the published contract — this directory, [`README.md`](README.md) and
[`v1/`](v1/) — never Core's private Rust crates.

New to the protocol? Work through the [tutorial](../docs/tutorials/first-wire-client.md) first; its
dependency-free client [`examples/wire-v1/wire_client.py`](../examples/wire-v1/wire_client.py) is a
compact, verified implementation of the steps below.

## 1. Launcher

1. Locate a pinned host binary and verify it (the Python SDK checks the SHA-256 recorded next to its
   bundled host). `nativehtmlkit-host --version --verbose` prints the embedded source identity
   (`version`, `source_commit`, `source_tree`, `source_dirty`) as JSON.
2. Write an `app.toml` ([reference §1](README.md#manifest-apptoml)) into a private temporary
   directory. Quote every string safely (a JSON string is a valid TOML basic string, except that DEL
   must be escaped); `{app_dir}` is substituted everywhere and has no escape, so reject arguments
   that contain it.
3. Run `nativehtmlkit-host run --app <manifest>`; add `--headless` only when the user asked for test
   mode (the SDKs use `NATIVEHTMLKIT_TEST_MODE=1`). Never select headless silently because no
   desktop was found.
4. **Bind the host's lifetime to the launcher**: Linux `PR_SET_PDEATHSIG` in the child, Windows a Job
   Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (plus `SILENT_BREAKAWAY_OK` so applications the
   host opens for the user survive). Otherwise a killed launcher leaves an orphaned window.
5. Read the host's stderr continuously: it is JSON lines. Relay `fields.source == "backend"`
   records as the app's stdout/stderr (`fields.stream`), show `WARN`/`ERROR` records, and hide the
   rest unless the user asked for verbose output. Return the host's exit status.
6. Delete the manifest directory afterwards; keep any result/session files in `0700` directories.

## 2. Backend connection

1. Read and **remove** `UI4ME_ENDPOINT`, `UI4ME_TOKEN`, `UI4ME_APP_ID`,
   `UI4ME_PROTOCOL_VERSION`. Validate the token (64 hex characters) and the version (`1`). Never log
   the token or pass it to child processes.
2. Connect: Linux `AF_UNIX` `SOCK_STREAM` to the path. Windows: open the named pipe for read and
   write, retrying briefly while it is busy. **If one thread reads while another writes, open the pipe
   for overlapped I/O** (`FILE_FLAG_OVERLAPPED`, one `OVERLAPPED`/event per operation): on a
   synchronous handle a pending `ReadFile` blocks every `WriteFile`. A single-threaded client (read
   only while waiting for something, as in `wire_client.py`) can use a plain synchronous handle.
3. Framing: write a `u32` big-endian length followed by the UTF-8 JSON; read the same. Reject length 0
   or more than 2,097,152 **before allocating**. Never interleave two writers on one stream.
4. Send `host.hello {token, protocol_version: 1}` first; require `result.protocol_version == 1`.
   Keep `result.capabilities`.

## 3. Requests and the reader loop

- Use unique, non-empty string ids (≤128 bytes, no control characters); keep at most **128** requests
  pending, and refuse locally beyond that.
- Dispatch every incoming frame:
  - `method` **and** `id` → host request. Answer `window.before_close` / `app.before_quit` with
    `{"allow": bool}` (missing or non-boolean is a veto); answer any other method with error
    `-32601`. Answer promptly — the host has no deadline and waits.
  - `method` without `id` → notification. Deliver it; ignore unknown methods and members.
  - otherwise → response; correlate by `id`, never by order. An error response with `id: null`
    belongs to no request: treat it as a defect in your client (log it, fail loudly in tests).
- **Keep reading at all times.** Run the reader on its own thread or task (or read whenever you wait,
  as the example does). A backend that stops reading loses `document.event`/`window.event`
  notifications (counted in `host.diagnostics.dropped_notifications`) and, if it leaves responses
  unread, fails the session. Hand slow application work to another thread; never run user callbacks
  on the reader thread if they may block on a request.
- Give long-running requests (dialogs, clipboard reads, path loads) a cancellation path:
  `request.cancel {id}`; the original request is then answered `-32006`.
- Map error codes to typed exceptions and **never parse `message`**; treat unknown codes as failure.
  Distinguish `-32004` (stale window/document/node: retire the handle) and `-32008` (lifecycle phase).

## 4. Application model

1. `window.create` → `document.load {window_id, html|path}` → `app.ready` (within
   `startup_timeout_ms`).
2. Expose handles that carry both `window_id` and `document_id`. Retire the old document handle only
   after a successful `document.load` (a refused load keeps the old document). After `window.event
   closed` retire the window; after `app.shutdown` retire everything (no `closed` arrives for the
   last window).
3. Wrap `document.command` generically (`command(op, **fields)`) so new additive ops work without an
   SDK release, then add typed helpers. Generate them from
   [`v1/schemas/document-command.schema.json`](v1/schemas/document-command.schema.json) if your
   language allows.
4. Events: deliver `document.event` with the documented members
   ([events guide](../docs/guides/events.md)). Let users match a target on `event.element` **or**
   `event.path`. Ignore members beginning with `_`.
5. Default actions cannot be cancelled from a callback; expose `set_default_action` as a declarative
   API ("prevent Enter in #search", "prevent submit on #form").
6. Validate what `document.command` returns before trusting it (form entries are `{name, kind,
   value}`, file entries carry `path`/`filename`).
7. Make HTML insertion APIs obviously "trusted HTML" and offer text APIs (`set_text`, `set_value`)
   for user data. Provide an escaping helper.
8. Live bitmaps: map the returned `path` before the first `bitmap.present`, write straight-alpha RGBA8
   rows of `stride` bytes into a slot, present it with an increasing `sequence`, and reuse the slot
   only after the acknowledgement. On Windows open the file with delete sharing so the host can
   unlink it after the first present.
9. On `app.shutdown {deadline_ms}` release resources (close bitmaps' mappings, flush files) and exit
   before the deadline; otherwise the backend is killed and the host exits with status 1.

## 5. Verifying a new client

1. Run it against the pinned host in headless mode and compare every method and op you wrap with
   [`v1/conformance/golden.json`](v1/conformance/golden.json) (error codes, result member kinds); the
   [wire tables](reference/wire-tables.md) show the same data per method.
2. Exercise the edge cases the golden locks: stale documents (`-32004`), `app.ready` twice
   (`-32008`), unknown members (`-32602`), oversize HTML (`-32005`), a host request arriving while you
   wait for a response, quit vetoed and then allowed.
3. Check for orphans: after a normal exit, a crash of your backend and a killed launcher, no host or
   backend process may remain.
4. Then qualify real windows on Linux KDE Plasma Wayland and an interactive Windows desktop (input,
   focus, resize, close decisions, file dialogs, clipboard). Headless mode proves the protocol and
   document state, not native behaviour.

The Rust CLI (`nativehtmlkit-cli`), the Python SDK and the PowerShell SDK are independent v1 clients
qualified against the same host and are useful references.

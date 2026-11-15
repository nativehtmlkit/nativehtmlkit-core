# Writing an independent NativeHTMLKit client (SDK authoring guide)

Use only the published contract (`README.md`, `v1/`), never Core's private Rust crates. A client is a
**backend process launched by the host**; the host owns windows, rendering, input and accessibility.

## Minimum viable client

1. Read and **unset** `UI4ME_ENDPOINT`, `UI4ME_TOKEN`, `UI4ME_APP_ID`, `UI4ME_PROTOCOL_VERSION`.
   Validate the token shape (64 hex). Do not log it.
2. Connect: Linux `AF_UNIX` stream to the path; Windows open the named pipe for read/write
   (synchronous byte mode is sufficient; retry briefly while the pipe is busy).
3. Framing: write `u32` big-endian length + UTF-8 JSON; read the same. Reject length 0 or
   > 2,097,152 before allocating. Never interleave two writers on one stream.
4. `host.hello {token, protocol_version: 1}`; require `result.protocol_version == 1`.
5. Use unique string IDs (e.g. `"c1"`, `"c2"`); keep at most 128 pending.
6. A reader loop dispatches: frames with `method` **and** `id` are host requests — answer
   `window.before_close`/`app.before_quit` with `{"allow": bool}` and any other with error `-32601`;
   frames with `method` and no `id` are notifications; others are responses, correlated by `id`.
   An error response with `id: null` belongs to no pending request: treat it as a protocol defect.
7. `window.create` → `document.load {window_id, html|path}` → `app.ready`.
8. On `app.shutdown {deadline_ms}` release resources and exit before the deadline.

## Correctness rules learned from qualification

- Match event targets on `event.element` **or** `event.path` (an inner element with an `id` becomes
  `element`).
- Treat every error code as failure; never parse error messages.
- Validate everything returned by `document.command` before trusting it (e.g. form entries are
  `{name, kind, value}` objects; file entries carry `path`/`filename`).
- Escape all untrusted text before putting it in HTML; there is no script execution, but markup
  injection still changes what the user sees and submits.
- Bind the host's lifetime to your launcher if you launch it (Linux `PR_SET_PDEATHSIG`, Windows Job
  Object with kill-on-close); otherwise a killed launcher leaves an orphaned window.
- Keep result files/session data private (`0700` directories on Unix) and delete them promptly.

## Verifying a new SDK

Run the SDK against the pinned host with `run --headless` for protocol tests, compare against
`v1/conformance/golden.json` for every method you wrap, then qualify real windows on KDE Wayland and
an interactive Windows desktop. The Rust CLI (`nativehtmlkit-cli`) is the reference independent
client and its `tests/gui/` harnesses can be adapted.

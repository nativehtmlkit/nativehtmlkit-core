# Adversarial architecture review

**Interpretation:** These are risks evident from code structure or hypotheses requiring tests, *not reproduced exploits*. Each proposed countermeasure is a new requirement; the old source does not automatically satisfy it.

## A1 — No single enforceable contract today (Blocker)

Method names exist in several Rust dispatchers; document command decoding exists in a separate 81-variant enum; result JSON is assembled dynamically; platform code changes behavior. Existing SDKs and prose docs are not a generated formal protocol source of truth. A Rust change can silently invalidate a third-party SDK. **Action:** contract schema and runtime conformance must match code; maintain source-to-contract drift gates and release contract hashes.

## A2 — Too much accidental public surface (Blocker)

There are 32 top-level methods and 81 document operations. If all are declared stable without per-method verification, implementation bugs and debug features become permanent obligations. **Action:** preserve wire compatibility while classifying methods `stable-v1-candidate`, `conditional-platform`, `experimental`, `diagnostic`, `internal-special`. Freeze groups only after result schemas and tests exist; don't call “implemented” synonymous with “stable”.

## A3 — JSON-RPC interoperability is restricted (High)

Rust source accepts string request IDs only, named params only and no batch frames. Generic JSON-RPC 2.0 libraries allow more. JSON duplicate object keys are read through `serde_json::from_str::<Value>` without a duplicate-key-rejecting visitor. Python/PowerShell may be stricter. **Action:** publish an explicitly restricted profile; reject duplicates consistently after compatibility tests; include negative vectors.

## A4 — Null-ID error inconsistency (High)

The host responds to some malformed parsed requests via `protocol::error(None, ...)`, yielding `id:null`; Rust `parse_response` requires string IDs. This risks an SDK terminating or misrouting error handling. **Action:** define uncorrelated protocol-error semantics; never match such errors to a pending request ID; align all client parsers and test while another request is pending.

## A5 — Special handshake does not enforce request ID (High)

`host.hello` checks token and version but doesn't require a request ID; when sent as a notification, it can change connection state without an acknowledgement. It ignores metadata keys. **Action:** require valid ID in clients, consider enforcing it in host after reviewing existing clients, and freeze metadata extension semantics.

## A6 — Unknown params are inconsistently handled (High)

Many request structs use `#[serde(deny_unknown_fields)]`, while `host.capabilities`, `window.list`, `app.ready`, `desktop.paths` and other direct handlers don't parse params. `host.hello` and `app.activate` ignore arbitrary extra data. **Action:** document exact legacy behavior and propose strict behavior only with compatibility tests or a new version. Never claim universal strictness today.

## A7 — Activation is deliberately pre-auth (High)

`app.activate` is handled before authenticated backend checks, subject to same-user private endpoint and bounded payload. It forwards external caller params to backend and can request window focus. **Action:** treat as a separate security surface, validate allowed payload shape, rate/size limits, and backend input handling. It is not general shared controller access.

## A8 — Single backend is not general SDK attach (High)

Transport permits up to 8 connections but the host admits one authenticated backend. The host launches its backend from a manifest, and disconnect is treated as application failure. **Action:** document host-owned bootstrap/reverse callbacks as mandatory; do not advertise “connect to any existing window” or arbitrary MCP attach in v1.

## A9 — Notifications are bounded, not durable (High)

Host can replace oversized `document.event` notifications with `document.error` and may disconnect slow readers. There is no event sequence/replay or resume. **Action:** specify v1 best-effort semantics and recovery by querying authoritative state; future sequence+overflow+resync protocol only when use cases justify it.

## A10 — CLI stdout is not a data channel (High)

Rust host captures backend stdout/stderr to bounded diagnostic log rings and can drop output. **Action:** CLI launcher owns pipeline stdout; use an explicit protected result channel or atomic result file to receive selection completion from its backend. Tests cover EOF, cancel, signal, crash, long result, slow pipe, Windows cleanup and no debug text leakage.

## A11 — Capabilities conflate implementation and availability (High)

Existing capability JSON mixes top-level booleans, nested features and `available:null`. The fact that a method exists doesn't mean the desktop service currently works or has been qualified. **Action:** stable vocabulary and versions, session availability versus feature existence, documented null and explicit negative outcomes; feature-check at runtime.

## A12 — Branding is not cosmetic at the wire boundary (High)

`UI4ME_*` vars are used by both host platform launchers; `ui4me-res:`, `ui4me-bitmap://` and `data-ui4me-*` are parsed by renderer and assets code; CSS profile uses an old product name. Blind rename breaks currently working SDKs/apps. **Action:** additive aliases, no removed wire v1 identifiers, test old/new combinations, document deprecation rules.

## A13 — Runtime privilege/sandbox limitations (High)

Private socket and token auth protect one connection, not the operating system. Backend runs with caller's privileges. HTML mutations, file roots, dialogs and external URI handlers can be misused. **Action:** publish threat model; safe text APIs, asset-root checks, no remote fetch, path traversal/symlink/large file tests. No “untrusted third-party backend sandbox” claim.

## A14 — Resource/lifetime invariants require tests (High)

Document ID generation avoids targeting replaced documents; bitmap shared files use OS-specific open/delete semantics; native dialogs and clipboard are async. **Action:** stale ID tests, unload/reload races, dialog close/cancel, bitmap slot reuse, cross-language Windows memory-map tests and bounded-resource soak.

## A15 — Core/SDK packaging is still coupled (High)

Python's current source build compiles Rust host when no prebuilt override is supplied; PowerShell builds its package by extracting native binaries from Python wheels. **Action:** separate Core binary release + contract, then CLI/PyPI/PSGallery consume an immutable Core artifact pinned by version/hash. No Rust source checkout required by consumer builds.

## A16 — Current workflows violate new explicit policy (High)

`.github/workflows/ci.yml` and `test-powershell.yaml` include push/PR triggers. Publishing workflows are manual, but not all CI. **Action:** every new repo workflow uses **`workflow_dispatch` only**; verify with static check. No automatic cross-repo chaining.

## A17 — Errors and state semantics are not fully typed (Medium)

`serde_json::Value` outputs vary by method; renderer errors map through a numeric engine code; some OS services only return “accepted”. **Action:** separate permanent code/type from diagnostic message, define per-method result schemas, transient retry rules, timeouts, ownership and idempotency.

## A18 — Platform parity is limited (Medium)

Wayland cannot guarantee focus/placement/hidden window/unminimize; Windows UI Automation and Linux AT-SPI require interactive qualification. **Action:** documented capability matrix; native acceptance in qualified user sessions; headless tests are insufficient.

## A19 — Internal Rust functions should stay internal (Medium)

Renderer `Document::render_into`, raw input dispatch, accessibility action helpers and pipeline control exist but have no general host RPC. Exposing them “for completeness” would expand attack surface and reduce internal freedom. **Action:** only add a public method after defining a use case and testing/permission model; snapshot rendering remains a separate CLI diagnostic capability today.

## A20 — Published schemas can still be wrong (Blocker)

Static extraction finds names and input field types, not all runtime semantic checks or response schemas. The schemas in this report are explicitly **draft structural schemas**; they must not be published as production compatibility guarantees. **Action:** build responses/notification fixtures from runtime, compare with generated types, independent client qualification and source-drift gate. No stable label until then.

## Conclusion

The existing architecture is viable. The dangerous shortcuts are (i) auto-blessing all existing operations, (ii) renaming wire identifiers without compatibility, (iii) equating a JSON method list with a tested public protocol and (iv) confusing separate Git repositories with runtime-independent artifacts. Those are the failure modes the migration roadmap must prevent.

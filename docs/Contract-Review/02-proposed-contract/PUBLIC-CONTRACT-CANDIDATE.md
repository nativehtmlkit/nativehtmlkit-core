# Part II — Proposed NativeHTMLKit public contract

**Candidate:** `NativeHTMLKit Wire 1 / Contract Revision 0.1-draft`  
**Status:** **NOT APPROVED, NOT IMPLEMENTED IN FULL, NOT QUALIFIED**. This document is a proposed normative target, not a statement that source already complies.  
**Non-goal:** replace the existing host IPC with HTTP, FFI, a common SDK runtime, shared native libraries or a general multi-client daemon.

## 1. Public contract bundle ownership

Canonical files must live in **`nativehtmlkit-core/protocol/`**, exported as part of every immutable Core GitHub release. The `nativehtmlkit-project` repo owns the governance decision and ecosystem compatibility matrix, *not a conflicting copy of the wire contract*. Docs/Website render released Core contract artifacts and link by version and checksum.

A qualified Core release must include:

```text
nativehtmlkit-core release
├── host/linux-x86_64/...  # executable + required native loader dependencies
├── host/windows-x86_64/...  # executable
├── protocol/
│   ├── manifest.json              # wire_version, revision, stability classes, hash
│   ├── idl/                       # complete message and method types
│   ├── schemas/                   # request, response, notifications, method inputs+outputs
│   ├── semantic-rules.md          # state, errors, ordering and platform behavior
│   ├── conformance/               # fixtures, negative cases, smoke SDK
│   └── compatibility.md           # versioned change history and support policy
├── SHA256SUMS                  # ALL shipped artifacts and contract files
├── SOURCE_COMMIT.txt           # exact source identity
├── SBOM and license notices   # artifacts, fonts and dependencies
└── RELEASE_NOTES.md
```

**No SDK should clone Core or rebuild Core on installation.** Python and PowerShell distributions may *vendor* a qualified host and any required native dependencies. The CLI distribution may vendor the same host. They pin version, platform, SHA-256 and contract digest, and fail closed on mismatch.

## 2. IDs and versioning — four different variables

| Identifier | Example | Meaning | Changed by |
|---|---|---|---|
| `host_version` | `1.0.1` | Executable implementation version | Rust Core release |
| `wire_protocol_version` | `1` | Major incompatible framing/bootstrap/method semantic generation | Deliberate breaking protocol release |
| `contract_revision` | `1.0.0` when qualified | Precise compatible method/schema/capability set within wire v1 | Contract release |
| `sdk_version` | `0.2.0` | One language SDK's public classes/cmdlets/wrappers | That SDK repository |

A host upgrade with the same `wire_protocol_version` **must** preserve the behavior of all previously declared stable operations, unless clients explicitly opted into changed behavior by negotiated capability. Capability additions are additive; capability removal or changed semantics require an incompatible wire version or explicit new method/capability.

`host.hello` currently accepts only exact `protocol_version:1`, not a range. **Do not describe capability negotiation as already implemented.** First public revision can retain that handshake; later compatible extensions can include optional `contract_revision`, stable capability IDs, and negotiated optional features **without altering the required legacy `host.hello` shape**.

## 3. Wire framing and message profile

- One 4-byte unsigned big-endian UTF-8 JSON body length, followed by that exact number of body bytes; limit `1..=2,097,152` encoded bytes.
- Exactly one JSON object per frame. No JSON-RPC batch arrays or positional `params` arrays in wire v1.
- Request: `{"jsonrpc":"2.0","id":"req-1","method":"window.create","params":{...}}`. `id` is limited by the Rust implementation to **128 UTF-8 bytes**, not 128 characters; JSON Schema `maxLength` alone cannot enforce the byte limit. The candidate schemas carry `x-max-utf8-bytes` and require an explicit runtime test.
- Notification: same object but no `id`; no response expected. **SDKs should use request IDs for control calls**; no silently successful mutation notifications.
- Success: `{"jsonrpc":"2.0","id":"req-1","result":...}`.
- Error: `{"jsonrpc":"2.0","id":"req-1","error":{"code":-32602,"message":"...","data":...}}`.
- A malformed *valid-JSON request object* currently can receive an error with `id:null`; the Rust client parser currently rejects `id:null`. **Candidate rule:** invalid request envelopes should yield a distinguishable terminal protocol error / `id:null` handling, without completing any unrelated pending request. Specify the rule before freezing and add tests.
- Duplicate JSON members: **candidate rule:** reject duplicate keys consistently across host and clients. This changes permissive legacy parsing and requires a compatibility review and negative conformance fixtures; do not claim currently enforced by Rust.
- Preserve existing `-326xx` and `-32001…-32011` error definitions with stable semantics. Error text can be diagnostic but code and documented `data` fields must carry machine-readable meaning.
- All integers represented in JSON must have wire-safe bounds; define >53-bit integer guidance for JavaScript-based SDKs, even though Core has no JavaScript runtime.
- No automatically inferred HTTP/TCP, websocket, MCP or SDK-specific FFI transport.

## 4. Bootstrap is part of the API

1. SDK or CLI launcher finds a **verified** native host executable from its own package or a deliberate development override.
2. Launcher writes a bounded, private manifest that names the backend executable, args/cwd and host lifecycle policy; path and app identity are validated.
3. Core host starts, allocates private Unix socket/Windows local pipe and generates a random 256-bit token.
4. Host spawns the backend process and injects endpoint/token/app ID/wire version as **process-scoped environment**.
5. Backend opens the endpoint, sends `host.hello` with token and `protocol_version:1`, validates the answer, removes token-bearing variables from inherited environment, handles reverse requests and runs `app.ready` after opening at least one window.
6. Host and backend run until normal close/quit; host owns bounded shutdown and process-tree cleanup.

**Current required bootstrap variable names (wire-v1 legacy):** `UI4ME_ENDPOINT`, `UI4ME_TOKEN`, `UI4ME_APP_ID`, `UI4ME_PROTOCOL_VERSION`. Treat as **public compatibility aliases for wire v1** until a tested migration path is approved.

**Proposed additive NHK names:** `NATIVEHTMLKIT_ENDPOINT`, `NATIVEHTMLKIT_TOKEN`, `NATIVEHTMLKIT_APP_ID`, `NATIVEHTMLKIT_PROTOCOL_VERSION`. They are **not currently emitted by Core 1.0.1**. Adopt by:

- Host temporarily emits **both sets** with identical values, plus redacts both sets in logs and reserves them against user manifest overrides.
- SDK detects either but fails if both are set inconsistently. New SDKs prefer `NATIVEHTMLKIT_*`, retain legacy fallback until published removal window.
- Add cross-version tests `old SDK → renamed host`, `new SDK → original host`, `new SDK → new host`.
- Future breaking protocol may remove old aliases only with explicit version bump; do not reinterpret v1.

**Naming policy outside the wire:** project `NativeHTMLKit`; repos `nativehtmlkit-*`; CLI canonical executable `nativehtmlkit`, short alias `nhk`; Python `import nativehtmlkit` (or user-defined `as nhk`); PowerShell module `NativeHTMLKit`, long-form canonical `Verb-NativeHTMLKitNoun`, exported command aliases `Verb-NHKNoun`. These are **language APIs**, not changes to JSON-RPC method strings.

## 5. Existing resource names during rebranding

The code accepts `ui4me-res://<bundle>/<path>`, `ui4me-bitmap://<id>` and certain `data-ui4me-*` attributes. Those are **application document markup semantics** and must be accounted for in compatibility testing.

**Proposed eventual aliases:** `nativehtmlkit-res://`, `nativehtmlkit-bitmap://` and `data-nativehtmlkit-*`. They do **not** exist in the uploaded code. Add recognition side-by-side, ensure identical sandbox/budget/policy behavior and test conflicts, case rules and deprecations. Do not replace old strings in renderer, stylesheet URLs, image loader, examples and docs in one blind migration.

A host that advertises new aliases must report a capability ID for each, rather than relying on a version number guess. Legacy wire-v1 names remain legal until an explicit incompatible version.

## 6. Stability categories and initial publication approach

| Category | Included candidate surface | Rule |
|---|---|---|
| **Core base, `stable-v1-candidate`** | Hello/ready/quit, window create/get/set/list, document load/get/unload, basic DOM operations, form/control state and events, reverse close/quit | Freeze only when method/result schemas and headless+native tests pass |
| **Conditional platform, `conditional-platform`** | Attention, focus/visibility, native dialogs, desktop integration, clipboard, accessibility | Explicit host capabilities and OS-dependent failures, with qualification matrix |
| **Extended Core, `experimental`** | Bitmaps, complex DOM node batches, advanced CSS/profile/inspection, resources and special event control | Supported in code, but independent behavioral qualification before stable label |
| **Diagnostic, `diagnostic`** | `probe.*`, `host.diagnostics`, renderer diagnostic APIs | May change with less stringent compatibility; keep out of default SDK stable surface |
| **Special bootstrap, `internal-special`** | Pre-auth `app.activate` | Document and test security semantics; not general remote control |
| **Future, `not-implemented`** | Event recovery protocol, multi-backend attach, remote TCP, streaming binary protocol, built-in MCP, video/audio engine | Never advertise until implemented and tested |

**Compatibility with existing SDKs:** even if an operation stays experimental, deleting it immediately may break already shipped UI4ME clients. Preserve existing wire behavior through the migration where possible; label *support guarantees* separately from the implemented-method inventory.

## 7. Methods, results and notifications — required final deliverable

The candidate bundle includes all **32 method identifiers**, **81 document operations**, **22 batch variant shapes** and **9 emitted messages** by source inventory. The initial TypeScript IDL/JSON Schemas represent method names and structural command inputs; they are **not a full certified output model**.

Before a stable contract release, complete the following for every public method and operation:

- Request fields, presence/default/nullability, enum values, string/byte/resource constraints, examples.
- Result schema (`result`), field availability and nullability, including platform variations and version introduction.
- Errors: explicit codes, stable optional data keys, permission/state/transient categorization and retry/cancel policy.
- Side effects, atomicity and ordering; whether repeated calls are idempotent.
- Lifecycle prerequisites; stale document/window/node/bitmap handling; callback timing.
- Test fixtures: valid/invalid param matrix, errors and lifecycle transitions, host-published capability matrix.

Do **not** call a JSON Schema “complete” merely because it validates the top-level envelope or a list of operation names.

## 8. Capabilities and version evolution

Current `host.capabilities` is an excellent base but mixes booleans, nested feature objects, conditional `available` values and explanatory prose. Proposed minimum:

```json
{
  "wire_protocol_version": 1,
  "contract_revision": "1.0.0",
  "host_version": "1.0.1",
  "features": {
    "window.focus": {"implemented": true, "available": false},
    "window.create": {"implemented": true, "available": true}
  },
  "limits": {"frame_bytes": 2097152, "max_documents": 16}
}
```

**Illustrative future fields, not output of the uploaded host.** Distinguish “implemented in build” from “currently available on this session” and from “actually qualified on this OS/session.” Unknown/unavailable should never be silently coerced to true. SDKs must feature-check optional operations, not blindly infer from Core version.

## 9. Transport security and process ownership

- Local IPC transport and token do not make the backend an operating-system sandbox; backend code runs with its user's privileges.
- Untrusted HTML/text must not be parsed as trusted markup; differentiate `set_text`/`set_value` from insertion of raw HTML.
- File paths require canonicalization, asset-root confinement, no symlink escapes, bounded file loads and format limits.
- `desktop.open_external`, native dialogs, `app.activate` and clipboard are sensitive user-action boundaries.
- Logging must redact token/endpoint where appropriate; SDK scrubs bootstrap env after authentication.
- Core must guarantee child process containment and timeout handling on both platforms; don't assume an unattended/headless service can open a GUI.
- Public contract must describe payload limits and cumulative resource budgets and specify behavior on malformed/malicious frames.

## 10. NativeHTMLKit release discipline

Every workflow in Core, CLI, SDKs, Project, Docs and Website must be **manual `workflow_dispatch` only**; no push, PR, tag, release, schedule, workflow_run, repository_dispatch or automatic cross-repo trigger. A manually launched workflow can run automated tests and publish artifacts only after an explicit version/confirmation and all applicable gates pass. Existing UI4ME Core CI still contains `push` and `pull_request` triggers; remove them in the migration.

Release process: qualify host → publish immutable binaries/protocol/hashes → explicitly trigger per-SDK packaging workflows to consume pinned host artifacts → explicitly trigger ecosystem compatibility verification → publish supported matrix; **never auto-trigger subsequent repositories**.

## 11. Public v1 graduation checklist

All are mandatory before saying **NativeHTMLKit public protocol v1 is stable**:

1. Every advertised public request, result, notification and reverse request has a frozen machine-readable shape and semantic contract; no unresolved `unknown` result schemas.
2. End-to-end reference Rust client **outside the Core source tree** boots/opens/edits/closes correctly against release binaries.
3. Existing UI4ME Python/PowerShell clients work unchanged against compatible renamed hosts; then NativeHTMLKit clients work against both old and new host as promised.
4. Fuzz/negative transport tests include malformed/duplicate-key JSON, null IDs, oversized frames, disconnects, backpressure and unauthorized activation.
5. Native desktop qualification completed on actual Linux KDE Wayland and interactive Windows, not just headless tests; a11y/IME work tracked as separate gates.
6. Release manifest, hashes, provenance, license notices and repeatable package assembly are verified.
7. A breaking-change policy and compatibility matrix are present and respected.
8. Every GitHub Actions workflow is manual-only.

The accompanying JSON Schema and IDL are a **structured starting artifact**, not this final graduated output.

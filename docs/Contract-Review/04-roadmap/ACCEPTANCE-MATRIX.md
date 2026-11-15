# Protocol conformance & adversarial acceptance matrix

**Test classification:** S=static/schema; H=headless real Rust host; L=interactive Linux Wayland; W=interactive Windows; X=cross-client; P=packaging/release. The tests below are required future tests, **not executed as part of this analysis**.

| ID | Scope | Test / expected result |
|---|---|---|
| T001 | S/H | 4-byte BE frame length including exactly body bytes; UTF-8 Unicode roundtrip |
| T002 | H | Reject zero-sized, >2 MiB and truncated frame without excess allocation |
| T003 | H | Reject malformed UTF-8, invalid JSON and JSON array/batch |
| T004 | H | Reject request with numeric/null/empty/too-long ID and invalid method syntax |
| T005 | H | Reject positional params and unknown top-level request fields |
| T006 | S/H | Reject duplicate JSON keys after approved hardening; preserve documented legacy policy until then |
| T007 | H | Unknown method returns `-32601` with stable `data.method` |
| T008 | H | Bad request receives contract-defined uncorrelated error; never completes wrong pending future |
| T009 | H | `host.hello` with exact protocol version 1 and correct per-launch token succeeds |
| T010 | H | Wrong token and wrong protocol version yield documented errors without exposing host state |
| T011 | H | Missing request ID during `host.hello` has explicitly decided behavior |
| T012 | H | Unauthorized non-hello method cannot mutate windows/clipboard/desktop |
| T013 | H | 8-connection admission cap and one authenticated backend enforced |
| T014 | H | Unauthorized `app.activate` boundary, payload limit, no host takeover, safe forwarded data |
| T015 | H | Manifest unknown keys, invalid app ID, bad env names/args/cwd rejected |
| T016 | H | Host-provided token stays out of logs/process command line and is scrubbed by SDK after hello |
| T017 | H/L/W | app.ready before window fails; after window succeeds once; repeated ready fails |
| T018 | H/L/W | create/get/list/set window requests; requested vs observed separation respected |
| T019 | L/W | Platform focus, visibility, placement, minimize/restore behavior matches feature reports |
| T020 | H/L/W | load/unload/reload document, rejected load preserves old document, old ID stays stale |
| T021 | H | HTML >1 MiB, extra documents, invalid asset root/symlink and filesystem policy reject |
| T022 | H/L/W | document text/get/set/checked/attributes/classes and batch apply correctly |
| T023 | H | Invalid doc op, unexpected fields, bad enum, wrong types and semantic bounds return documented error |
| T024 | H | Node handles generation-checked; stale node operations do not affect replacement document |
| T025 | H/L/W | form validation, request_submit, event payload, get_form_data order, reset behavior |
| T026 | H/L/W | click, input, keyboard, focus and document events have typed structure and stable order |
| T027 | H | Overflowed document event yields explicit `document.error`; SDK resync behavior defined |
| T028 | H | Backpressure on unread notifications and pending RPC limits without GUI deadlock |
| T029 | H/L/W | window.before_close valid allow/deny, timeout, supersession and late-answer behavior |
| T030 | H/L/W | app.before_quit allow/deny, shutdown notification/deadline, force quit |
| T031 | H/L/W | Backend crash/disconnect/exit and failed startup terminate boundedly without orphaned windows |
| T032 | L/W | Clipboard read/write, delayed/native failure, request.cancel and side-effect rules |
| T033 | L/W | File/folder/save dialog with cancellation, parent close and single-dialog-per-window limit |
| T034 | L/W | External open/reveal returns accepted/denied/cancelled as OS allows, no false “presented” guarantee |
| T035 | H/L/W | Bitmap create/write/present/close; monotonic sequence, slot reuse only after ack, max-memory limits |
| T036 | W | Bitmap memory-mapped shared file open/unlink semantics with actual Windows client |
| T037 | L/W | Native screen-reader/a11y tree updates, IME editing and text selection qualified |
| T038 | H | Capability schema, available:null, feature version and platform-specific values consistent |
| T039 | X | Existing UI4ME Python SDK → original host vs renamed host (legacy variable scheme) |
| T040 | X | Existing UI4ME PowerShell SDK → original host vs renamed host (legacy variable scheme) |
| T041 | X | New Python/PowerShell/CLI SDKs → renamed host and optional legacy host under approved policy |
| T042 | X | Document `ui4me-res://`, `ui4me-bitmap://` and `data-ui4me-*` still work; new aliases only after capability declaration |
| T043 | P | Core releases verified native ELF/PE, platform dependency set, SHA256, provenance, licenses/SBOM |
| T044 | P | A fresh client repository checkout uses pinned released host with no Core source checkout |
| T045 | P | `nhk` and `nativehtmlkit` CLI same behavior; stdout exact and stderr diagnostics separated |
| T046 | L/W | `nhk select --multi` input → GUI submit → exact output, no extra output or background process |
| T047 | L/W | `nhk select` cancellation/close/Ctrl+C/crash/kill correctly distinct and no partial success output |
| T048 | L/W | CLI input size limits, malformed CSV/JSON/invalid UTF-8 policies, OS shell encoding |
| T049 | P | Every GitHub Actions workflow has `on: workflow_dispatch` only; no hidden chains |
| T050 | P | Artifact compatibility matrix published only after manual verification evidence |

**Minimum credible first release:** T001–T018, T020–T031 (relevant cases), T038–T049. Native acceptance (L/W) cannot be replaced by H or S. Remaining platform/resource methods may stay conditional or experimental until fully qualified, but must not be reported as stable.

## Negative fixtures and core source tests

The included `02-proposed-contract/examples/schema-fixtures.json` is a **schema-only** sample with positive and negative examples. Implement a Rust conformance binary that consumes **versioned fixture arrays** and drives the actual host in authenticated headless mode, then add Linux/Windows native acceptance drivers using your existing testing lab. Do not depend on documentation examples alone.

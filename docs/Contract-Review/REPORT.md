# NativeHTMLKit — Core Protocol Extraction, Adversarial Architecture Review & Public Contract Proposal

**Status:** Research / engineering decision package — **not a verified stable protocol release**  
**Basis:** User-supplied `ui4me-core-main(6).zip`, static inspection of actual Rust host/renderer/transport and current Python/PowerShell source.  
**Source:** UI4ME Core 1.0.1 code snapshot. No Rust compilation, native desktop test, network API test or full runtime conformance suite was executed in this analysis.  
**Date:** 15 November 2026.

## Decision in one paragraph

**Keep the existing IPC design and wire method identifiers, but do not declare the entire current surface stable yet.** NativeHTMLKit Core should publish immutable platform-specific native host binaries **and** a versioned, machine-readable protocol/SDK contract generated and checked against code. The independent CLI, Python and PowerShell repositories should each package a pinned, hash-verified host artifact and implement their own clients. Use the actual UI4ME wire-v1 contract as the starting compatibility baseline, add NativeHTMLKit branding with deliberate legacy aliases, and close the specifically identified protocol ambiguities before a public 1.0 contract freeze. Development, release, compatibility testing, and publication workflows must be **manual-only GitHub Actions (`workflow_dispatch`)**.

## Separation required by this project

| Evidence state | Meaning | Can be claimed publicly now? |
|---|---|---|
| `E` — **Existing in source** | Concrete enum, dispatcher, result creation, transport, implementation behavior found in code | Can report *implemented in inspected source*, not live-qualified |
| `C` — **Candidate public contract** | Proposed normative requirements, compatible with existing wire where possible | No; requires approval, implementation gaps and qualification |
| `G` — **Gap / unexposed** | Capability missing from wire or not reliably specified | No |
| `F` — **Future** | Potential capability not required for initial SDK/CLI | No |

### Evidence-backed size of the implemented interface

- **32 host-callable RPC method identifiers** (including `host.hello` and the special secondary-instance `app.activate` path): [`01-implementation/host-methods.md`](01-implementation/host-methods.md).
- **81 implemented tagged `document.command` operations**: [`01-implementation/document-operations.md`](01-implementation/document-operations.md).
- **11 `batch` + 11 `node_batch` operation variants** available inside the corresponding document commands.
- **9 host-emitted message types**: 7 notifications and 2 reverse requests requiring client answers: [`01-implementation/emitted-messages.md`](01-implementation/emitted-messages.md).
- These counts are **static source enumeration**, not counts of user-facing SDK wrapper functions or live behavior passes.
- SDK inventory separately found **12 Python top-level exports** and **83 PowerShell exported functions**. Those are *not* comparable one-to-one with wire operations; see [`01-implementation/SDK-SURFACE-COMPARISON.md`](01-implementation/SDK-SURFACE-COMPARISON.md).

### Immediate priority findings

| ID | Priority | Finding | Required decision |
|---|---|---|---|
| P-01 | Blocker | Wire v1 is implemented but not formally a public stable contract; response schemas and semantic guarantees are incomplete | Explicit public compatibility policy, generated input/output schemas, conformance gates |
| P-02 | Blocker | The old `UI4ME_*`, `ui4me-res:` and `ui4me-bitmap://` identifiers are embedded throughout host, resource loader, renderer and SDKs | Preserve legacy v1, add new aliases compatibly; no blind search/replace |
| P-03 | Blocker | Incompatible transport/result edge cases: host's invalid-request error may have `id:null`, but client-side `parse_response` rejects null; JSON duplicate keys not explicitly rejected by host decoder | Decide and test canonical invalid-frame behavior and errors |
| P-04 | High | Only one authenticated backend per host; arbitrary-language SDK must participate in host-owned startup and reverse callbacks | Normative bootstrap + lifecycle state machine; no “attach anytime” promise |
| P-05 | High | Protocol notifications are bounded, not durable; oversize events can be dropped; slow readers may be disconnected | Define dropped-event recovery, backpressure and cancellation contracts |
| P-06 | High | Some request handlers reject unknown params via serde while others simply ignore them | Decide strictness by method and preserve legacy behavior intentionally |
| P-07 | High | `app.activate` is deliberately accepted pre-auth on the private same-user endpoint and forwards caller data to backend | Treat it as a separate trust boundary; schema, rate/payload checks, regression tests |
| P-08 | High | Existing GitHub CI workflows trigger on push / pull_request | Replace with manual-only triggers during migration |
| P-09 | High | Capabilities are currently partly descriptive and platform-conditional, without immutable capability IDs or shape guarantees | Version capability vocabulary, bounds and fallback behavior |
| P-10 | Medium | Some useful renderer functionality is intentionally internal and not host-RPC exposed | Keep internal unless concrete SDK use cases justify public surface |

Detailed evidence and reasoning: [`03-critical-review/adversarial-review.md`](03-critical-review/adversarial-review.md).

## Existing architecture — verified from code

```text
app.toml / CLI launcher / SDK launcher
       │
       ▼
Rust host process  (ui4me-host -> proposed nativehtmlkit-host)
 ├─ owns Wayland / Win32 native event loop, renderer, accessibility, services
 ├─ creates per-launch private local IPC endpoint and 256-bit random token
 ├─ starts one backend process and supervises it
 ├─ accepts up to 8 transport connections, but only one authenticated app backend
 ├─ exchanges length-prefixed JSON-RPC-like messages (no HTTP and no JS)
 └─ closes on error/timeout and enforces bounded queues
       ▲
       │  private Unix socket / Windows named pipe; authenticated host.hello
       ▼
backend process: CLI backend / Python SDK / PowerShell SDK / other language
 ├─ implements own IPC read+write, requests and event loop
 ├─ replies to window.before_close and app.before_quit
 └─ controls windows/documents through JSON-RPC
```

Source anchors: `crates/ui4me-host/src/{main.rs,config.rs,protocol.rs,transport.rs,backend.rs,lifecycle.rs}` and `src/host/{mod.rs,requests.rs,documents.rs,bitmap.rs,document_commands.rs}`.

**Do not confuse separate repository names with a different architecture.** In the new layout Core should still contain its five internal Rust crates; internal Rust crate-to-crate source dependencies are appropriate *inside* Core. It is only **external SDK/CLI releases** that should not require Core's unpublished Rust source tree.

## Proposed NativeHTMLKit architecture

```text
nativehtmlkit-project/                      # ecosystem policy / manual compatibility matrix
  repositories/
    nativehtmlkit-core/                    # Rust host & renderer; canonical protocol
       crates/ ...
       protocol/                           # contract manifest, generated IDL, JSON schemas,
                                           # conformance fixtures, compatibility, migration
       release/                            # native binary artifact builders, hashes, SBOM/notices
    nativehtmlkit-cli/                     # independent Rust client, separate release
    nativehtmlkit-python/                  # pure Python client; packages qualified host binary
    nativehtmlkit-powershell/              # PowerShell + existing C# helper initially; host package
    nativehtmlkit-docs/                    # rendered developer reference, not canonical IDL
    nativehtmlkit-website/                 # presents documentation; no second canonical protocol
```

Release layering:

```text
Core release (native host binary + protocol contract + hashes + provenance)
   │
   ├── CLI release: nhk / nativehtmlkit (Rust CLI + pinned Core host binary)
   ├── PyPI release: nativehtmlkit (Python SDK + pinned Core host binary)
   ├── PowerShell Gallery release: NativeHTMLKit (PS/.NET helper + pinned host binaries)
   └── Third-party SDKs: use the same public contract; no Rust compilation
```

`CLI → Python → PowerShell` is the **development migration sequence**, not a runtime chain. None should need another language SDK. Core owns the contract; SDKs must not copy and independently redefine it.

## Key design decisions

1. **Version separately:** `core_host_version`, `wire_protocol_version`, `contract_revision` and `sdk_version`. Core and SDK package versions need not match.
2. **Promote wire v1 conservatively:** keep `window.*`, `document.*`, etc. unchanged. Introduce stable/new naming in packages, classes, commands and executable names first, and support legacy wire resources until migration is complete.
3. **A tested contract is more than schema files:** document startup and security, request/response types, host-initiated requests, ordering, cancellation, limits, resource ownership, platform variations, failure states and compatibility.
4. **No automatic inter-repo events:** all GitHub Actions use `workflow_dispatch` only. Trigger tests/releases deliberately; local agents may run any tests themselves.
5. **Stability classes:** `stable-v1-candidate`, `conditional-platform`, `diagnostic`, `experimental`, and `future`. Code existing in a dispatcher is **not** sufficient evidence to promise stability.
6. **Source-based drift gate:** Core CI should fail if protocol-relevant Rust source changes without regenerating or reconciling canonical schemas, examples, compatibility notes and conformance fixtures.
7. **One canonical source:** IDL in Core; derived docs in Docs and Website; separate SDK repositories import the artifact by pin/hash.

## What can be implemented immediately

The current host already supports core window creation, documents, text/form mutation, callbacks/events, local native dialogs, clipboard, local asset bundles, live RGBA bitmaps and native CSS rendering. CLI `select`, `view`, `confirm` and simple `form` can be built **without adding a new renderer or HTTP server**. A CLI shell pipeline still requires special stdout/result handoff: the current host routes backend stdout/stderr to bounded diagnostics, *not* a lossless data channel. Keep that handoff in CLI (via its owning launcher) rather than forcing a new Core wire method into v1.

## What is **not** ready to promise

- “Any language can attach to an already running Core” — the current model is host-launched, single authenticated backend; no general attach/reconnect/multi-client API.
- “Every document command is stable” — 81 variants are implemented, but full input *and output* schemas plus behavior verification remain.
- “All desktop features behave identically” — Wayland and Windows differ in window visibility, focus, placement, dialogs and session availability.
- “Unbounded stream of events/data” — frames and queues are bounded and notification loss is possible.
- “Pure PowerShell without compilation” — existing PowerShell module compiles a C# helper at runtime with `Add-Type`; this is not Rust recompilation.
- “Everything has been run” — this package is source analysis and schema structural validation, not runtime certification.

## Recommended next action

**Complete and adopt Gate G0 — Protocol inventory + compatibility policy — before touching the public protocol or splitting SDK code.** Then migrate Core in a behavior-preserving branch, add candidate contract fixtures and run core/native acceptance. CLI may begin against the existing wire implementation once G1 is satisfied; public stable-v1 declaration waits for the full contract and cross-platform qualification gates.

See the concrete gates and work packages in [`04-roadmap/ROADMAP.md`](04-roadmap/ROADMAP.md), the candidate contract in [`02-proposed-contract/PUBLIC-CONTRACT-CANDIDATE.md`](02-proposed-contract/PUBLIC-CONTRACT-CANDIDATE.md), and the source-backed implementation audit in [`01-implementation/IMPLEMENTATION-AUDIT.md`](01-implementation/IMPLEMENTATION-AUDIT.md).

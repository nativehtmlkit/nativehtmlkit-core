# Architectural decisions to resolve BEFORE declaring the wire stable

The following are decision records, not implemented changes. Record decisions in `nativehtmlkit-project/decisions/` and make the accepted contract canonical in `nativehtmlkit-core/protocol/`.

| ADR | Topic | Preferred decision | Code changes? | Proof required |
|---|---|---|---|---|
| ADR-NHK-001 | Contract owner | Rust Core owns canonical wire schema, examples, test vectors and hashes | Build/config change | Independent test without Core source tree |
| ADR-NHK-002 | Compatibility lineage | Preserve old wire-v1 method IDs, message framing, error codes and legacy env/resource names through v1 | Yes for new branding aliases | Old SDK against new Core; new SDK against old Core |
| ADR-NHK-003 | Protocol scope | Formal restricted JSON-RPC 2.0 profile; no batches, positional params or numeric request IDs | Primarily tests | Positive/negative vectors, fuzzing |
| ADR-NHK-004 | Request-ID semantics | IDs limited to 128 UTF-8 bytes; uncorrelated `id:null` errors are handled separately | May change parser | Concurrent inflight request + invalid frame cases |
| ADR-NHK-005 | Hello semantics | Request-ID required for supported SDK clients; token and protocol checked, metadata optional; no unauthenticated generic attach | Potentially | Existing SDK handshake + invalid hello tests |
| ADR-NHK-006 | Duplicate JSON keys | Consistently reject as future hardening after compatibility impact analysis | Yes | Host and all clients agree on duplicate-key behavior |
| ADR-NHK-007 | Param strictness | Preserve existing legacy handler semantics; move to consistent strictness only compatibly or on major wire version | Potentially | Per-method unknown-field tests |
| ADR-NHK-008 | Lifecycle and reverse calls | Document and test host-owned backend startup, close/quit decisions, deadlines, cancellation and fatal disconnect | Tests, docs | Host/backend death, close veto, CTRL-C/Windows close |
| ADR-NHK-009 | Event delivery | Define bounded notifications without durable replay; document recovery and future explicit resync feature | Initially docs | Flood/drop/backpressure test matrix |
| ADR-NHK-010 | Resource and markup aliases | Add new `nativehtmlkit-*` resource and attribute aliases without deleting v1 `ui4me-*` | Yes | CSS/bitmap/resource regression tests |
| ADR-NHK-011 | Capability vocabulary | Stable machine-readable IDs and feature/limit semantics, distinct `implemented`/`available` | Yes | Windows/Wayland matching fixtures |
| ADR-NHK-012 | Stability tiers | Base, conditional platform, advanced experimental, diagnostic and future are separate guarantees | Docs/test gates | No method marked stable without output/error/semantic schema + tests |
| ADR-NHK-013 | SDK artifact pins | Pin Linux/Windows host binary by version+SHA256 and contract hash, never unpublished Core `path` deps | CI/package change | Build each SDK from separate checkout without Rust Core sources |
| ADR-NHK-014 | Manual GitHub Actions | `workflow_dispatch` only across **all** projects, including publish/test/docs/deploy | Yes | Static linter; no automated trigger in new repos |
| ADR-NHK-015 | Reproducible audit | Store uploaded archive SHA now; after repo migration bind source commit hash and generated contract hash to release | Yes | Provenance and drift checks on exact source commit |

## Explicit non-decisions / deferred features

- No MCP, TCP/HTTP/WebSocket or automatic remote agent exposure as part of initial protocol.
- No multiple authenticated simultaneous application backends, attach/resume or event replay in wire v1 without a new design.
- No promises of GPU rendering, browser JavaScript, real-time audio/video, OS-independent focus/placement, or unlimited table/bitmap streaming.
- No requirement to remove the existing runtime C# `Add-Type` helper from PowerShell before extracting that SDK.
- No automatic triggering, automated release publishing or cross-repository workflow cascades.

## Public-stable freeze checklist

A public `stable` label requires, for every advertised operation: fully resolved request and response schemas, defaults/nulls/limits, return errors and machine-readable diagnostics, lifecycle/permissions, supported platforms, named capabilities, conformance positive/negative vectors, a compatible previous host/client test and independently recorded native desktop qualification. A successfully parsed JSON Schema or a unit-test pass alone is **not** sufficient.

# Protocol stabilization roadmap — status 2026-11-15

| Gate | Requirement | Status | Evidence |
|---|---|---|---|
| C0 | Workspace builds; original Rust tests pass on Linux and Windows | **PASS** | Linux 1098 passed/0 failed/12 ignored; Windows 1012 passed/0 failed/10 ignored (cfg-gated difference); both from committed tree `a1a78c8`; clippy `-D warnings` both |
| C1 | Authoritative request/op matrix | **PASS** | `v1/method-matrix.json` generated from handlers (31 client methods, 9 host-originated, 81 ops) |
| C2 | Typed schemas verified against Rust decoders | **PASS** | generated schemas + runtime golden (unknown/missing/wrong-type cases); unit-variant exception documented |
| C3 | Independent Rust client opens window, submits form, quits | **PASS** | `nativehtmlkit-cli` native GUI suites (packaged binaries: Linux KDE Wayland 28/28, Windows interactive 24/24) |
| C4 | Malformed framing, duplicate keys, invalid IDs, unexpected params characterized | **PASS** | golden `envelope.*`, `security.*` cases; identical on Linux and Windows |
| C5 | Interop with original Python v1 client | **PASS (Python, Linux headless)** / PowerShell **NOT RUN** | identical results from the unmodified UI4ME 1.0.1 SDK under original and migrated host |
| C6 | Native GUI qualification: input, focus, resize, DPI, accessibility, IME | **PARTIAL** | pointer/keyboard/focus/resize/scroll/close PASS on both OS; UIA tree observed on Windows. **BLOCKED:** IME (no active input method on either VM), DPI/scale >100 %, screen-reader journeys, native file dialogs/clipboard/drag-drop/external-open GUI evidence |
| C7 | Artifact provenance, pinned host, hashes, local packaging | **PASS** | `scripts/release-local.py --stage-only`, verified clean extraction |
| C8 | Public v1 stability declaration | **WITHHELD** | contract locked as `1.0-rc`; declare stable once the C6 blockers are qualified |

Do not mark the public contract stable solely because JSON Schemas validate or headless tests pass.

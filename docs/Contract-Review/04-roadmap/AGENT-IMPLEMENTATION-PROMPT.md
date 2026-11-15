# Local coding agent prompt — start at the contract, not the rename

You are responsible for the NativeHTMLKit Core public wire protocol migration and contract stabilization. Work under `nativehtmlkit-project/repositories/`. The attached contract audit ZIP contains an **AS-IS Rust source extraction** and a **TO-BE candidate**. Treat them separately. The original uploaded UI4ME source/repo is a reference baseline; do not overwrite or delete it. The new `nativehtmlkit-core` and `nativehtmlkit-cli` are independent Git repos.

## Non-negotiable invariants

1. Core owns the canonical versioned protocol. SDKs and CLI consume release artifacts, not private Rust source crates from another repo. A development override to use a local Core binary is okay; production/release always pinned by SHA256.
2. Preserve wire v1 method names, legacy `UI4ME_*` backend environment keys and existing resource schemes during migration until compatibility changes are separately implemented and tested. Introduce new NativeHTMLKit names additively, not through blind string replacement.
3. Use public long names for packages/API (`NativeHTMLKit`, `nativehtmlkit`, repos `nativehtmlkit-*`) and short command aliases only (`nhk`, `Verb-NHKNoun`).
4. All GitHub Actions workflows in every repo must be **manual `workflow_dispatch` only**; no push, pull_request, schedule, release, workflow_run, repository_dispatch or automatic chaining. Local verification runs are allowed without approvals. Never publish or tag without explicit user instruction.
5. Reject unsupported claims. Do not label a method or feature `stable` until full input/output schema, semantic constraints, tests, and Linux/Windows evidence exist. Headless != interactive desktop qualification.
6. Keep Linux Wayland/KDE as the primary real GUI platform and interactive Windows as the other qualified target. Do not add a daemon, HTTP listener, browser, embedded JS, or required Python/PowerShell runtime into Core.
7. Use source code and tests as authority for AS-IS; proposed files are *targets* and may differ from implementation.

## Work sequence

A. Validate the extraction in `01-implementation/` against the currently checked-out Rust source. Correct inventory automation if code changed. Record exact source commit and SHA256; identify source status. Enumerate every stable method input, result, notification, reverse request and semantic error. Keep separate `Implemented`, `Proposed`, `Not Implemented`, `Needs Test` labels.

B. Decide architectural ADRs: bootstrap naming, version contract, string/null/duplicate-key behavior, `host.hello` ID, `app.activate` security, event loss/cancellation, resource scheme aliases, stability tiers. Do not silently modify wire behavior without comparative old/new SDK tests.

C. Bring Core source into `nativehtmlkit-core` and establish standalone host builds. Preserve old source qualification evidence. Build host binaries and standalone contract artifacts independent of Python wheel construction. Implement versioned Rust conformance tests and independent reference client.

D. Add manual-only GitHub build/test/publish workflows. Any release workflow must have explicit user-supplied version/confirmation, pinned source SHA, source/contract drift checks, Linux/Windows binary evidence, SHA256SUMS and license/SBOM inventory. Do not create a release automatically.

E. Implement `nativehtmlkit-cli` only after the tested Core host/client contract is available. Use it as first independent consumer. CLI builds Rust itself, but it must not rebuild Core from source; it downloads the qualified host. Implement `nhk select` with shell-facing launcher ownership of stdout, correct exit/cancel semantics and no backend log leakage; qualify Linux Wayland + interactive Windows.

F. Continue with Python and PowerShell independent SDK packaging, preserving existing API behavior and PowerShell's tested C# runtime helper until proven unnecessary. Record all cross-version results centrally in `nativehtmlkit-project`.

## Deliverables

- ADRs and complete canonical contract in `nativehtmlkit-core/protocol/` after review; generated schemas and fixtures with a source-drift check.
- Source and behavior comparison report; list deviations from uploaded v1 and proof for each change.
- Green Rust/unit/protocol conformance + cross-platform native evidence with real logs/commands/artifact hashes.
- Manually triggered CI/publish workflows only and verified release manifest.
- CLI acceptance on Linux and Windows using exact text lines, proper cancel behavior and no child orphaning.
- Clear list of remaining `experimental`, `diagnostic`, and future features; never claim stable for incomplete items.

Start with G0/G1 from `04-roadmap/ROADMAP.md`. Do not jump to CLI feature expansion before the transport/bootstrap contract is tested.

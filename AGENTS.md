# NativeHTMLKit Core — agent instructions

This is an **independent Rust repository**. It is currently only a placeholder with a preserved research/audit snapshot; the original UI4ME engine has **not** been migrated into it.

If running inside the normal workspace, first read `../../AGENTS.md`, `../../agents/shared.md`, `../../docs/architecture/boundaries.md`, and `../../MIGRATION-HANDOFF.md`. If cloned alone, apply the non-negotiables below and consult the referenced public sources as available; don't assume a parent project checkout exists.

1. Audit the **current original UI4ME Core code and tests** at the exact given source revision; the research snapshot is not necessarily current. Preserve the existing repository/releases/tags, source provenance, licenses, patches and qualification evidence.
2. Core owns **Rust host/renderer, canonical public protocol/IDL, conformance tests and release artifacts**. Do not implement the Python, PowerShell or CLI client here. The CLI lives from day one in `nativehtmlkit-cli`, not as a Core subcrate.
3. Port and run the original Rust functionality before broad renames. Preserve legacy v1 wire compatibility until specifically versioned changes and tests justify transitions. Do not treat draft schemas as stable.
4. Produce qualified Linux/Windows host binaries and contract artifacts **without packaging through a Python wheel**; consumers must pin artifacts and SHA-256 hashes.
5. All GitHub Actions must have `workflow_dispatch` **only**; remove `push`, `pull_request` and other legacy automatic triggers during migration. Publishing, remote mutation, tags, force pushes and destructive changes require explicit authorization.
6. Run Rust/protocol/headless and real Linux KDE Wayland + interactive Windows acceptance tests. Keep source SHA, commands, actual results, platform/environment and logs; never claim verification you didn't run.
7. No browser/JS/HTTP/Node or mandatory Python/PowerShell runtime in Core. No secrets in code/fixtures/reports.
8. Work on Core defects here only when the task scope includes Core. Track consumer-discovered bugs and their workarounds in Project's `docs/bugs/bug_list.md`.

Primary research: `docs/Contract-Review/REPORT.md`, `01-implementation/`, `02-proposed-contract/`, `03-critical-review/` and `04-roadmap/`. These are **audit evidence and proposals**, not frozen runtime API claims.

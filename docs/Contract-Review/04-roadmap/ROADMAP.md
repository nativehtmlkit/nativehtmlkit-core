# Implementation roadmap — executable gates, not calendar promises

**Migration order approved:** `nativehtmlkit-core` → `nativehtmlkit-cli` → `nativehtmlkit-python` → `nativehtmlkit-powershell` → higher-level UI/application. Contract ownership and validation begin during **Core**, before CLI. New repositories sit under `nativehtmlkit-project/repositories/`. No reorganization into a single Git monorepo and no automatic workflow triggers.

## Gate G0 — Baseline evidence and immutable audit (first)

| Work | Owner | Deliverable | Acceptance |
|---|---|---|---|
| NHK-001 | Project | Record source SHA256 and UI4ME 1.0.1 release baseline; preserve old tag/qualification | Provenance stable, old repo untouched |
| NHK-002 | Core | Extract 32 top-level method names, 81 doc ops, 22 batch variants, 9 emitted types from Rust | Source inventory tool reproducible; audit of regex extraction against Rust syntax |
| NHK-003 | Core | Record request/response types, defaults, errors, transient OS differences, lifecycle constraints | Every stable-candidate method has typed input, output, reverse requests and at least one fixture |
| NHK-004 | Project/Core | ADR: preserve wire 1, naming aliases, unsupported features, stability tiers | Signed-off decision; source and proposal clearly separate |
| NHK-005 | All | Enforce manual `workflow_dispatch` GitHub Actions only | Static checker passes; old automatic triggers removed when porting |

**Gate G0 PASS:** current as-is inventory is reproducible; proposed methods/compatibility/stability classes reviewed. Draft is still *not* a stable contract.

## Gate G1 — Core independent release and conformance

| Work | Owner | Deliverable | Acceptance |
|---|---|---|---|
| NHK-006 | Core | Copy Rust-only engine and host into new Core repo; preserve history as practical | Internal Rust build/test parity with old release |
| NHK-007 | Core | Add renamed binary `nativehtmlkit-host`, preserving wire v1 | Existing UI4ME client can launch/handshake via legacy variables |
| NHK-008 | Core | Introduce `NATIVEHTMLKIT_*` bootstrap aliases, resource/HTML aliases as separate explicit changes | New+legacy startup names work; conflicts rejected; old scheme continues |
| NHK-009 | Core | Create source-checked JSON Schema/IDL and machine-compatible test fixtures | No drift; outputs and notifications typed; `id:null` and duplicate JSON cases resolved |
| NHK-010 | Core | Harden per-method errors and async cancellation/notification recovery semantics | Negative/fuzz suite; no hidden API or schema ambiguity |
| NHK-011 | Core | Build Linux/Windows qualified native artifacts, provenance and checksums | New release built from reproducible source, exact SHA256 and license inventory |
| NHK-012 | Core | Publish Core release manually | Release contains hosts + contract + conformance + checksums; no Python wheel needed |

**Gate G1 PASS:** an independent minimal client launched from **outside Core** can perform hello → create window → load document → change text → respond close/quit and exit against released host artifacts, headless and native desktop where applicable. Old SDK compatibility tests pass.

## Gate G2 — CLI first independent consumer

| Work | Owner | Deliverable | Acceptance |
|---|---|---|---|
| NHK-013 | CLI | Rust protocol client (separate source repo; no `path = ../nativehtmlkit-core/...`) | Repo builds with only published host artifact and public contract |
| NHK-014 | CLI | CLI launcher, stdin parser, isolated backend process and result handoff | Shell stdout contains only results; secure temp/process cleanup |
| NHK-015 | CLI | `nhk select` line input, single/multi, keyboard/mouse selection, submit/cancel | Select pipeline exact bytes/records on Linux and Windows |
| NHK-016 | CLI | `nativehtmlkit` full executable + `nhk` short alias | Both invoke same implementation; no unversioned collisions |
| NHK-017 | CLI | JSON/CSV/TSV and select/form/view/confirm incremental modes | Bounded inputs, explicit encoding and exit-code semantics |
| NHK-018 | Project | Manual Core+CLI compatibility test suite | Exact pinned host hash, native and headless checks, no GitHub workflow chaining |

**Gate G2 PASS:** a fresh checkout of CLI builds without Core sources and `printf 'alpha\nbeta\ngamma\n' | nhk select --multi` returns only confirmed rows on stdout (correct cancel/no data/failure and cleanup) on Linux KDE Wayland and interactive Windows.

## Gate G3 — Python SDK extraction

| Work | Owner | Deliverable | Acceptance |
|---|---|---|---|
| NHK-019 | Python | Extract and rename `ui4me_core` to `nativehtmlkit` with compatibility guidance | `import nativehtmlkit`; existing app smoke tests pass |
| NHK-020 | Python | Replace “build Core from source in setup.py” with pinned Core artifact packaging | Installed wheels require no Rust; correct platform wheel tags; sdist policy explicit |
| NHK-021 | Python | Full Python API protocol/conformance + native UI tests | Same stable contract behavior as CLI; no cross-language dependency |

**Gate G3 PASS:** `uv add nativehtmlkit` from a released wheel works in a clean environment without Rust; SDK uses the qualified host without source checkout.

## Gate G4 — PowerShell SDK extraction

| Work | Owner | Deliverable | Acceptance |
|---|---|---|---|
| NHK-022 | PowerShell | Extract existing `.psm1` and C# transport helper into `nativehtmlkit-powershell` | Keep working callback queue and native-pipe behavior |
| NHK-023 | PowerShell | Long module name `NativeHTMLKit`, canonical commands `Verb-NativeHTMLKitNoun`, alias `Verb-NHKNoun` | PSGallery package works on PS 7 (Linux/Windows) and PS 5.1 Windows |
| NHK-024 | PowerShell | Stage host from pinned Core release, **not Python wheels** | Independent release, SHA256 verification, no Python runtime dependency |
| NHK-025 | PowerShell | Pester + native conformance on qualified Linux/Windows | No regression from UI4ME Core behaviors |

**Gate G4 PASS:** `Install-Module NativeHTMLKit` in clean environments works without a Rust toolchain or Python. Existing C# `Add-Type` runtime behavior may remain and must be tested explicitly.

## Gate G5 — Ecosystem-level compatibility publication

| Work | Owner | Deliverable | Acceptance |
|---|---|---|---|
| NHK-026 | Project | `compatibility-matrix.json`: host artifact version, protocol, contract hash, SDK versions | Every supported combination backed by saved test evidence |
| NHK-027 | Docs/Website | Published reference imported from Core release | Same IDL/hash, no competing manually edited protocol |
| NHK-028 | All | Manual verification workflow runbooks; release attestation | All workflow YAMLs use only `workflow_dispatch`; no cascades |
| NHK-029 | Core/Project | Public stable wire v1 acceptance sign-off | G0–G5 passed; full advertised methods/results typed and tested |

## Rules agents must obey

- Work in **sibling repositories** under the Project `repositories/` folder. Never introduce unpublished cross-repo Cargo `path` dependencies.
- Keep source/behavior changes and branding changes as separately reviewable commits when practical.
- Never downgrade or remove the existing wire-v1 bootstrap or resource identifiers without explicit new protocol version/compatibility tests.
- Never use `push`, `pull_request`, `release`, `workflow_run`, `repository_dispatch`, `schedule` as GitHub Actions triggers; `workflow_dispatch` only.
- Never publish, create releases, delete old sources or migrate credentials automatically. Request explicit human command for any publication.
- Never claim “qualified” solely from unit tests, schema validation or headless runs; preserve native desktop evidence.

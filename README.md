# NativeHTMLKit Core

**A native Rust HTML/CSS desktop engine and a language-independent, authenticated local host protocol.** No browser, JavaScript runtime or HTTP server.

Migrated from the user's UI4ME Core **1.0.1** source archive (`SOURCE_PROVENANCE.json`, SHA-256
`2ec94422…2bd5`). The five first-party crates were renamed; behavior is preserved and verified:
the migrated host renders a parity corpus pixel-identically to the original and the unmodified
UI4ME 1.0.1 Python SDK produces identical results against both hosts. See `docs/MIGRATION-NOTES.md`
for the migration defects found and fixed.

## Build locally (Linux or Windows)

Rust 1.99.1 (`rust-toolchain.toml`) is required only to build Core. Restore the original font
fixtures first (not redistributed here), then run the full source gate:

```bash
python3 scripts/restore-source-assets.py /path/to/ui4me-core-main.zip   # verifies 14 SHA-256 hashes
bash scripts/build-local.sh          # Windows: scripts/build-local.ps1 (Python 3 needed by Stylo's build)
```

The gate checks assets, architecture and protocol-contract freshness (including dispatch arms vs
method list), vendored Stylo/Taffy patch integrity, `cargo deny` advisories/bans/sources (skipped
with a warning if `cargo-deny` is not installed), `cargo fmt` (first-party packages), `clippy -D warnings`, the full test suite (including the protocol v1 runtime golden),
the release build and a verified local stage. Native GUI qualification is separate.

```bash
./target/release/nativehtmlkit-host --version
./target/release/nativehtmlkit-host doctor
./target/release/nativehtmlkit-host render --html page.html --output page.png
./target/release/nativehtmlkit-host run --app app.toml
```

## Protocol

[`protocol/README.md`](protocol/README.md) is the v1 contract: method/op matrix and JSON Schemas
(parameter types generated from the Rust serde types; method list checked against the dispatch
arms), a 338-case runtime golden (error codes, result member kinds, boolean values and array
lengths; identical Linux/Windows results were last shown for 1.0.1 and are pending
re-qualification for 1.0.2), the compatibility policy and an SDK authoring guide. Status: **locked release candidate (`1.0-rc`)**; the public
stability declaration waits for the native gates listed in `protocol/CONFORMANCE-ROADMAP.md`.
Protocol v1 keeps its `UI4ME_*` launch variables and legacy markup/resource names;
`nativehtmlkit-*` spellings are additive aliases.

## Releases

`python3 scripts/release-local.py --stage-only` stages `dist/nativehtmlkit-core-vVERSION-PLATFORM.zip`
(host, protocol bundle, `LICENSE`, `THIRD_PARTY_NOTICES.md`, generated
`licenses/THIRD-PARTY-LICENSES.txt` for the host's dependency tree, MPL-2.0 and embedded-asset
notices, provenance, a manifest with source commit/dirty state/tree hash, host SHA-256, target and
toolchain, and `SHA256SUMS`) as a deterministic zip and verifies it from a clean extraction.
Re-staging the same version with a different host binary requires `--restage`. Publishing to
GitHub Releases needs an explicit user request and `--publish --confirm publish-VERSION` from a
clean commit whose tag `vVERSION` equals HEAD and from which the stage was made. No GitHub Actions
exist or may be added.

## Repositories

- `nativehtmlkit-core` (this): host, renderer, protocol contract, qualified host releases.
- `nativehtmlkit-cli`: independent Rust client and pipeline tool; consumes pinned host binaries.
- `nativehtmlkit-python`, `nativehtmlkit-powershell`: SDKs, in their own independently versioned
  repositories (not shipped here; they consume released Core host binaries).

## Local diagnostics

Run with `NATIVEHTMLKIT_DIAGNOSTICS=debug` or `trace` to enable additional
structured host logs on stderr; `RUST_LOG` takes precedence if set. This affects
logging only, not the protocol wire format. `NATIVEHTMLKIT_DIAGNOSTIC_SESSION_ID`
(identifier-like, at most 128 bytes) is copied into every record as
`diagnostic_session_id`.

| Level | Adds |
|---|---|
| `normal` | start/stop, warnings, errors, backend stdout/stderr lines |
| `debug` | lifecycle phase transitions (`event:"lifecycle.phase"`, `from`, `to`), launcher-loss acknowledgement, stale decision replies, presentation timings |
| `trace` | per request `event:"rpc.received"` (`method`, `id`, `connection`) and `event:"rpc.completed"` (`method`, `id`, `connection`, `duration_us`, `error_code` on failure); per host notification `event:"notify.sent"` (`method`; for `window.event`/`document.event` also the event `type` and `window_id`/`document_id`); per host request `event:"request.sent"` (`window.before_close`/`app.before_quit`, decision `id`) |

Trace and debug records are metadata only: never request params, results, text, values,
paths, URLs or the launch token. The stderr sink is bounded (128 records) and lossy by
design, so under load records can be dropped; `host.diagnostics` → `logging` reports
`sink_dropped_records` and `backend_omitted_records`. Never publish raw logs without
review: backend stdout/stderr lines (all levels) may contain application data.

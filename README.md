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

The gate checks assets, architecture and protocol-contract freshness, `cargo fmt` (first-party
packages), `clippy -D warnings`, the full test suite (including the protocol v1 runtime golden),
the release build and a verified local stage. Native GUI qualification is separate.

```bash
./target/release/nativehtmlkit-host --version
./target/release/nativehtmlkit-host doctor
./target/release/nativehtmlkit-host render --html page.html --output page.png
./target/release/nativehtmlkit-host run --app app.toml
```

## Protocol

[`protocol/README.md`](protocol/README.md) is the v1 contract: generated method/op matrix and JSON
Schemas, a 327-case runtime golden that passes identically on Linux and Windows, the compatibility
policy and an SDK authoring guide. Status: **locked release candidate (`1.0-rc`)**; the public
stability declaration waits for the native gates listed in `protocol/CONFORMANCE-ROADMAP.md`.
Protocol v1 keeps its `UI4ME_*` launch variables and legacy markup/resource names;
`nativehtmlkit-*` spellings are additive aliases.

## Releases

`python3 scripts/release-local.py --stage-only` stages `dist/nativehtmlkit-core-vVERSION-PLATFORM.zip`
(host, protocol bundle, licences incl. the embedded font's OFL, provenance, manifest with host
SHA-256, `SHA256SUMS`) and verifies it from a clean extraction. Publishing to GitHub Releases needs
an explicit user request and `--publish --confirm publish-VERSION` from a clean tagged commit. No
GitHub Actions exist or may be added.

## Repositories

- `nativehtmlkit-core` (this): host, renderer, protocol contract, qualified host releases.
- `nativehtmlkit-cli`: independent Rust client and pipeline tool; consumes pinned host binaries.
- `nativehtmlkit-python`, `nativehtmlkit-powershell`: planned SDK repositories (not shipped here).

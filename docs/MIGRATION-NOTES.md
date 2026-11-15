# Core code migration record

- Source archive: `ui4me-core-main(7).zip` (SHA-256 in `SOURCE_PROVENANCE.json`). Source Rust workspace package version 1.0.1.
- First-party crate directories and packages renamed:
  - ui4me-host → nativehtmlkit-host
  - ui4me-renderer → nativehtmlkit-renderer
  - ui4me-document → nativehtmlkit-document
  - ui4me-assets → nativehtmlkit-assets
  - ui4me-engine-types → nativehtmlkit-engine-types
- Rust module paths and Cargo workspace/dependency references renamed consistently. Cargo.lock package identities updated.
- Vendor trees under `patches/` and `third_party/` are preserved and must remain independently attributed. Do not globally rewrite upstream vendor source.
- `nativehtmlkit-host` is the release binary. Its `run`, `probe`, `render`, `doctor` subcommands are retained.
- Protocol v1 wire authentication environment keys remain `UI4ME_*` until an explicit versioned transition. Do not pass both old and new token keys to the backend because the existing legacy clients scrub only the old keys.
- Original `ui4me-res:` and `ui4me-bitmap:` schemes and `data-ui4me-*` attributes remain valid and now have NativeHTMLKit aliases. URI cache invalidation includes both scheme names.
- Python/PowerShell product SDK packages, packaging toolchain and GitHub publish workflows have been removed from the Core product repository. They are still present in the original source ZIP, for later SDK-specific migrations.
- Existing Rust code/tests and test fixtures are preserved except licensed binary fonts that are restored locally from the original ZIP.
- The default PNG, ICO and SVG icon files use NativeHTMLKit filenames. They retain the original artwork until a later visual rebrand; the Windows resource script and manifest have been renamed consistently.
- Local build scripts and candidate public protocol surface added.

## Migration defects found and fixed during qualification (2026-11-15)

Found by diffing the migrated tree against the original archive (rename-normalised) and by running
the gates. Each is fixed in Core with a regression guard where one was missing.

| # | Defect introduced by the mechanical rename | Effect | Fix / guard |
|---|---|---|---|
| M1 | `nativehtmlkit-assets/src/lib.rs`: alias edit deleted the `prefix` binding | Core did not compile | `bundles::url_prefixes()` covers both schemes for cache, font **and image** invalidation; test `replacement_restarts_images_requested_through_the_alias_scheme` (mutation-verified) |
| M2 | Three `OFL.txt` licence texts edited | build-time pinned SHA-256 failed; licence texts must stay byte-exact | restored originals |
| M3 | `forms.css` used `:-nativehtmlkit-single-row-select`, but vendored Stylo parses only `:-ui4me-single-row-select` | UA `<select>` overflow rules silently dropped (no compile error) | reverted; new tests `ua_sheets_produce_no_diagnostics` (the existing `check_ua_sheet()` was never called) and `renamed_vendor_pseudo_class_is_detected` |
| M4 | 49 debug/test env names became mixed case `NativeHTMLKit_*` | inconsistent, case-sensitive on Linux | uppercase `NATIVEHTMLKIT_*` |
| M5 | Test font URL renamed to `NativeHTMLKitBitmapTest.ttf` while the file keeps its original name | test fixture not found | reverted URL |
| M6 | Wire-visible identifiers renamed: CSS profile `ui4me-css/1`, digest header, diagnostics schema `ui4me-diagnostics`, producer name, `ui4me_version`, `ui4me-paint-tiers/1` | silent protocol v1 change for existing clients | reverted to v1 values (documented in `protocol/README.md` §8) |
| M7 | `cargo fmt --all` in the build scripts traverses the vendored taffy workspace (pre-existing upstream layout) | gate could not run | format check limited to first-party packages, as the original gate did |
| M8 | Contract inventory listed the test-only `app.changed` and missed `probe.set_text`, `probe.get_text`, `desktop.preferences_changed` | inaccurate contract | replaced by `scripts/generate-protocol-contract.py` + runtime golden |

Intentional product renames (not wire-visible) remain: binary/crate names, window class and mutex
names, systemd unit and runtime-directory names, AT-SPI toolkit name, manifest identity.

## Verification summary

- Linux (Debian 13 workstation, Rust 1.99.1) and Windows Server 2022 (winDEV, MSVC): fmt, clippy
  `-D warnings`, full test suite and release build pass; protocol golden passes on both.
- Render parity: 7-document corpus at 1x/2x rendered pixel-identically by the original
  `ui4me-host` 1.0.1 and the migrated host (only difference: the new `nativehtmlkit-bitmap://`
  alias no longer reports `RESOURCE_DENIED`).
- Legacy client: the unmodified UI4ME 1.0.1 Python SDK yields identical results against both hosts.
- Native GUI: exercised through the independent CLI on KDE Plasma 6.6 Wayland and an interactive
  Windows desktop; see the CLI `docs/QUALIFICATION.md`.

## Still unverified

- IME composition, DPI scaling above 100 %, screen-reader journeys, native file dialogs, clipboard,
  drag-and-drop and external-open on real desktops (headless results only).
- Legacy PowerShell client against the migrated host.
- Default icon still uses the original UI4ME artwork.

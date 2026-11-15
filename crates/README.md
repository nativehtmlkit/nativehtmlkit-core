# Rust crates

The five first-party workspace members (renamed from UI4ME Core 1.0.1, see `SOURCE_PROVENANCE.json`):

| Crate | Role |
|---|---|
| `nativehtmlkit-host` | The `nativehtmlkit-host` binary: CLI (`run`, `render`, `doctor`, `probe`), native windows, the authenticated v1 protocol endpoint and backend supervision. Tests: `tests/host_integration.rs`, `tests/protocol_conformance.rs` (runtime golden). |
| `nativehtmlkit-renderer` | Owner-thread HTML/CSS document engine (style via the vendored Stylo, layout via the vendored Taffy, text, raster, accessibility, input). |
| `nativehtmlkit-document` | DocumentCore: arena DOM, parser sink, mutation journal. |
| `nativehtmlkit-assets` | Bounded local assets, image decoding and the font-source registry (embeds the Noto Sans last-resort font). |
| `nativehtmlkit-engine-types` | Shared identities, geometry, status and diagnostics record types. |

Vendored third-party code is outside `crates/`: `patches/{stylo,taffy}` (MPL-2.0 / MIT, patched
upstream trees, verified by their `verify.sh`) and `third_party/` (patched crates.io crates, see
`third_party/README.md`).

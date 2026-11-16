# NativeHTMLKit Core — Agent policy

1. This repo owns *only* the Rust host, renderer, document, assets, protocol specification and its tests. Do not move Python/PowerShell SDK runtimes into Core.
2. Preserve original functionality and regressions. Original user source is identified in `SOURCE_PROVENANCE.json`.
3. Protocol v1 is the public-stable wire contract since host 1.0.3 (`protocol/README.md`, `protocol/COMPATIBILITY.md`): changes within v1 must be additive; anything else needs protocol version 2. Preserve v1 wire identifiers `UI4ME_*` for credential transport, old resource schemes and attributes; NativeHTMLKit aliases are additive.
4. Test affected Rust crates first, then cargo fmt/clippy/test workspace and real desktop/CLI integration. Do not mark tests PASS without execution evidence.
5. Build, test, and stage locally. No GitHub Actions (even manual ones), PyPI or PSGallery. GitHub Releases only after explicit approval.
6. Restore the original font fixtures using scripts/restore-source-assets.py from the original ZIP. Do not generate/rebrand fonts.
7. Use qualified immutable Core binary release artifacts for dependent repos. Local development override is allowed with explicit executable path.
8. When a protocol-visible type or behavior changes: run `python3 scripts/generate-protocol-contract.py`, re-bless the golden with `NATIVEHTMLKIT_CONFORMANCE_BLESS=1 cargo test -p nativehtmlkit-host --test protocol_conformance`, and confirm the diff is additive per `protocol/COMPATIBILITY.md`. Never rename wire-visible `ui4me` identifiers or vendored-Stylo names in a product rename.
9. Report blocked qualification steps and failures; do not fabricate screenshots, test logs, or success claims.
10. Keep per-repo commits/pushes independent; no submodules or private source-path dependencies.

Read `protocol/README.md`, `protocol/COMPATIBILITY.md`, `protocol/CONFORMANCE-ROADMAP.md` and the sibling CLI `AGENTS.md` before any cross-repo work. If started standalone, do not assume the sibling checkout exists.

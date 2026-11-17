# Building and verifying Core

Use the exact pinned Rust toolchain and dependency/patch tree. The public source ZIP may not include all restored font assets; **`assets-manifest.json` must match the assets available locally** before a reproducible build is claimed.

```bash
rustup show
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Scripts in `scripts/`:

- `build-local.sh` / `build-local.ps1` — local source gate on Linux / Windows (tests, clippy, architecture check).
- `check-architecture.py` — static workspace boundary and release-policy check (no Rust toolchain needed).
- `generate-protocol-contract.py` — regenerates or `--check`s the `protocol/v1/` matrix and schemas from the Rust source.
- `restore-source-assets.py` — restores the licensed test and fallback fonts from a locally owned source archive.
- `release-local.py` — stages a locally built host and protocol contract into a verified archive.
- `docs/check-docs.py`, `docs/generate-wire-tables.py`, `docs/generate-html-css-reference.py` — documentation checks and generated references (below).

These commands do not replace native KDE Wayland or Windows tests. A qualified release should carry source commit and binary SHA-256 and be exercised with native desktop journeys (input, IME, scaling, screen readers), which the maintainers run on lab desktops with the exact packaged host on each target platform. Do not present source-only unit results as GUI acceptance.

## Documentation checks

The generated references and the wire-v1 examples are verified against the contract and a real host
binary (headless, no desktop needed):

```bash
python3 scripts/docs/check-docs.py --host target/release/nativehtmlkit-host
```

After a protocol change regenerate `protocol/reference/wire-tables.md` with
`scripts/docs/generate-wire-tables.py`; after a CSS profile or engine change regenerate
`docs/reference/html-css-profile.md` with `scripts/docs/generate-html-css-reference.py --host …`.

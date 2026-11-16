# Building and verifying Core

Use the exact pinned Rust toolchain and dependency/patch tree. The public source ZIP may not include all restored font assets; **`assets-manifest.json` must match the assets available locally** before a reproducible build is claimed.

```bash
rustup show
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
# Also run the repository's specific source-asset and release scripts described under scripts/.
```

These commands do not replace native KDE Wayland or Windows tests. A qualified release should carry source commit and binary SHA-256 and run the central `qualification/native-journeys/` suite using the exact packaged host on each target platform. Do not present source-only unit results as GUI acceptance.

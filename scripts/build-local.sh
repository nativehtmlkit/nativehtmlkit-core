#!/usr/bin/env bash
# Local Core source gate (Linux). Mirrors the original UI4ME tools/release/check_linux.sh.
# Native KDE Wayland GUI qualification is a separate requirement (see docs/BUILD-AND-RELEASE.md).
set -euo pipefail
cd "$(dirname "$0")/.."
# If fonts were not restored yet, provide the original source archive.
if [[ -n "${NHK_LEGACY_SOURCE_ZIP:-}" ]]; then
  python3 scripts/restore-source-assets.py "$NHK_LEGACY_SOURCE_ZIP"
fi
python3 scripts/restore-source-assets.py --check
python3 scripts/check-architecture.py   # includes generate-protocol-contract.py --check
# `cargo fmt --all` would traverse the vendored taffy workspace (whose gentest member is not vendored),
# so format only the first-party packages, exactly as the original gate did.
PKGS=(-p nativehtmlkit-host -p nativehtmlkit-renderer -p nativehtmlkit-document -p nativehtmlkit-assets -p nativehtmlkit-engine-types)
cargo fmt "${PKGS[@]}" -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads="${RUST_TEST_THREADS:-4}"
cargo build --locked --release -p nativehtmlkit-host
./target/release/nativehtmlkit-host --version
python3 scripts/release-local.py --stage-only
printf 'Source gate passed. Native KDE Wayland and interactive Windows GUI tests are separate requirements.\n'

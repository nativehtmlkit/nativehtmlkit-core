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
python3 scripts/check-architecture.py   # includes generate-protocol-contract.py --check (dispatch arms vs method list)
# Vendored Stylo/Taffy trees must equal upstream + carried patches. Exit 2 = no pristine checkout.
for tree in stylo taffy; do
  set +e; bash "patches/$tree/verify.sh"; rc=$?; set -e
  if [[ $rc -eq 2 ]]; then
    printf 'WARNING: patches/%s/verify.sh SKIPPED: pristine upstream checkout not found (set UI4ME_UPSTREAM_SOURCE)\n' "$tree" >&2
  elif [[ $rc -ne 0 ]]; then
    printf 'patches/%s/verify.sh failed (%s): vendored tree and patches drifted\n' "$tree" "$rc" >&2; exit "$rc"
  fi
done
# Dependency policy (deny.toml). NHK_DENY_OFFLINE=1 uses the cached advisory database.
if cargo deny --version >/dev/null 2>&1; then
  cargo deny --locked ${NHK_DENY_OFFLINE:+--offline} check advisories bans sources
else
  printf '\nWARNING: cargo-deny is not installed: dependency advisories/bans/sources were NOT checked.\n         Install it with `cargo install --locked cargo-deny`.\n\n' >&2
fi
# `cargo fmt --all` would traverse the vendored taffy workspace (whose gentest member is not vendored),
# so format only the first-party packages, exactly as the original gate did.
PKGS=(-p nativehtmlkit-host -p nativehtmlkit-renderer -p nativehtmlkit-document -p nativehtmlkit-assets -p nativehtmlkit-engine-types)
cargo fmt "${PKGS[@]}" -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace -- --test-threads="${RUST_TEST_THREADS:-4}"
# Release build with --remap-path-prefix (no local home/workspace path in the binary, FR-R-06);
# the host embeds its source identity, which staging compares with this checkout (FR-R-04).
python3 scripts/release-local.py --build
./target/release/nativehtmlkit-host --version
./target/release/nativehtmlkit-host --version --verbose
# An existing same-version stage with a different host is refused; NHK_RESTAGE=1 replaces it.
python3 scripts/release-local.py --stage-only ${NHK_RESTAGE:+--restage}
printf 'Source gate passed. Native KDE Wayland and interactive Windows GUI tests are separate requirements.\n'

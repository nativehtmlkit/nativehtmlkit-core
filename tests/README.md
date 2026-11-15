# Core tests

Tests live with their crates (`crates/*/tests/` and unit tests in `crates/*/src/`); this directory
holds no test code. The local gate (`scripts/build-local.sh`, `scripts/build-local.ps1`) runs all
of them with `cargo test --locked --workspace`.

- Protocol v1 runtime golden: `crates/nativehtmlkit-host/tests/protocol_conformance.rs` against
  `protocol/v1/conformance/golden.json` (what it compares is stated in `protocol/README.md`).
- Host process, transport and lifecycle integration: `crates/nativehtmlkit-host/tests/host_integration.rs`.
- Headless tests never qualify native GUI behavior. Native Linux KDE Wayland and interactive
  Windows desktop qualification is a separate requirement (`protocol/CONFORMANCE-ROADMAP.md`).
- Ignored tests are subprocess fixtures, environment-driven fuzz harnesses, measurements and
  long stress runs; they are not run by the gate.

Evidence (`verification/`) is not tracked in git; record the source commit, dirty-diff hash,
commands, platform/session and failures with every qualification run.

#!/usr/bin/env python3
"""Static boundary and release-policy check; no Rust toolchain or network needed."""
from pathlib import Path
import tomllib
root=Path(__file__).resolve().parents[1]
c=tomllib.loads((root/'Cargo.toml').read_text())
assert len(c['workspace']['members']) == 5
assert all(Path(root/x).exists() for x in c['workspace']['members'])
assert not (root/'.github/workflows').exists(), 'GitHub Actions are intentionally disabled'
assert not (root/'pyproject.toml').exists(), 'Core must not publish Python SDK'
assert not (root/'powershell').exists(), 'Core must not publish PowerShell SDK'
assert (root/'crates/nativehtmlkit-host/src/protocol.rs').exists()
assert (root/'crates/nativehtmlkit-host/assets/nativehtmlkit.ico').is_file()
assert 'assets/nativehtmlkit.ico' in (root/'crates/nativehtmlkit-host/host.rc').read_text()
assert 'NativeHTMLKit.Host' in (root/'crates/nativehtmlkit-host/host.manifest').read_text()
lock=tomllib.loads((root/'Cargo.lock').read_text())
packages={pkg['name'] for pkg in lock['package']}
assert all(Path(member).name in packages for member in c['workspace']['members']), 'Cargo.lock must match the five renamed crate identities'
assert (root/'protocol/v1/method-matrix.json').is_file()
assert (root/'protocol/v1/conformance/golden.json').is_file()
assert not (root/'protocol/current').exists() and not (root/'protocol/public-v1-candidate').exists(), 'superseded drafts must not compete with protocol/v1'
import subprocess, sys
subprocess.run([sys.executable, str(root/'scripts/generate-protocol-contract.py'), '--check'], check=True)
print('PASS: Core boundaries, Rust workspace and no workflow automation')

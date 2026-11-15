# Local Core source gate (Windows). Mirrors the original UI4ME tools/release/build_windows.ps1 source checks.
# Interactive-desktop GUI qualification is a separate requirement (see docs/BUILD-AND-RELEASE.md).
$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..'))
function Step([scriptblock]$b) { & $b; if ($LASTEXITCODE) { exit $LASTEXITCODE } }
if ($env:NHK_LEGACY_SOURCE_ZIP) { Step { python scripts/restore-source-assets.py $env:NHK_LEGACY_SOURCE_ZIP } }
Step { python scripts/restore-source-assets.py --check }
Step { python scripts/check-architecture.py }
$pkgs = @('-p','nativehtmlkit-host','-p','nativehtmlkit-renderer','-p','nativehtmlkit-document','-p','nativehtmlkit-assets','-p','nativehtmlkit-engine-types')
Step { cargo fmt @pkgs -- --check }
Step { cargo clippy --locked --workspace --all-targets -- -D warnings }
$threads = if ($env:RUST_TEST_THREADS) { $env:RUST_TEST_THREADS } else { '4' }
Step { cargo test --locked --workspace -- "--test-threads=$threads" }
Step { cargo build --locked --release -p nativehtmlkit-host }
Step { .\target\release\nativehtmlkit-host.exe --version }
Step { python scripts/release-local.py --stage-only }
'Source gate passed. Interactive Windows GUI tests are a separate requirement.'

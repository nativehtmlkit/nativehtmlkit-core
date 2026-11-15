# Local Core source gate (Windows). Mirrors the original UI4ME tools/release/build_windows.ps1 source checks.
# Interactive-desktop GUI qualification is a separate requirement (see docs/BUILD-AND-RELEASE.md).
$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path (Join-Path $PSScriptRoot '..'))
function Step([scriptblock]$b) { & $b; if ($LASTEXITCODE) { exit $LASTEXITCODE } }
# `python.exe` may be only the Microsoft Store alias stub (prints a hint, exits non-zero).
# Accept a working python/py, otherwise a uv-managed interpreter; fail with a clear message.
# (Not named `Python`: a function of that name would shadow python.exe and recurse.)
function Invoke-Python {
    if (-not $script:PythonCommand) {
        foreach ($candidate in @(@('python'), @('py', '-3'), @('uv', 'run', '--no-project', 'python'))) {
            $exe = $candidate[0]; $pre = @($candidate | Select-Object -Skip 1)
            if (-not (Get-Command $exe -ErrorAction SilentlyContinue)) { continue }
            $ErrorActionPreference = 'Continue'
            try { & $exe @pre --version *> $null } catch { $global:LASTEXITCODE = 1 }
            $ErrorActionPreference = 'Stop'
            if ($LASTEXITCODE -eq 0) { $script:PythonCommand = $candidate; break }
        }
        if (-not $script:PythonCommand) { Write-Error 'No working Python 3 found (tried python, py -3, uv run python).'; exit 1 }
    }
    $exe = $script:PythonCommand[0]; $pre = @($script:PythonCommand | Select-Object -Skip 1)
    & $exe @pre @args
}
# Stylo's build script needs a real interpreter; hand it the one resolved above unless PYTHON3 is set.
if (-not $env:PYTHON3) {
    $resolved = Invoke-Python -c 'import sys; print(sys.executable)'
    if ($LASTEXITCODE -or -not $resolved) { Write-Error 'Cannot resolve the Python executable path for PYTHON3.'; exit 1 }
    $env:PYTHON3 = "$resolved".Trim()
}
if ($env:NHK_LEGACY_SOURCE_ZIP) { Step { Invoke-Python scripts/restore-source-assets.py $env:NHK_LEGACY_SOURCE_ZIP } }
Step { Invoke-Python scripts/restore-source-assets.py --check }
Step { Invoke-Python scripts/check-architecture.py }
# Vendored Stylo/Taffy trees must equal upstream + carried patches (exit 2 = no pristine checkout).
if (Get-Command bash -ErrorAction SilentlyContinue) {
    foreach ($tree in @('stylo', 'taffy')) {
        & bash "patches/$tree/verify.sh"
        if ($LASTEXITCODE -eq 2) { Write-Warning "patches/$tree/verify.sh SKIPPED: pristine upstream checkout not found (set UI4ME_UPSTREAM_SOURCE)"; $global:LASTEXITCODE = 0 }
        elseif ($LASTEXITCODE) { Write-Error "patches/$tree/verify.sh failed ($LASTEXITCODE): vendored tree and patches drifted"; exit $LASTEXITCODE }
    }
} else {
    Write-Warning 'bash not found: patches/*/verify.sh SKIPPED (vendored patch integrity NOT checked; run it on Linux or install Git Bash).'
}
# Dependency policy (deny.toml). NHK_DENY_OFFLINE=1 uses the cached advisory database.
$ErrorActionPreference = 'Continue'
try { & cargo deny --version *> $null } catch { $global:LASTEXITCODE = 1 }
$ErrorActionPreference = 'Stop'
if ($LASTEXITCODE -eq 0) {
    $denyArgs = @('deny', '--locked'); if ($env:NHK_DENY_OFFLINE) { $denyArgs += '--offline' }
    Step { cargo @denyArgs check advisories bans sources }
} else {
    $global:LASTEXITCODE = 0
    Write-Warning 'cargo-deny is not installed: dependency advisories/bans/sources were NOT checked. Install it with: cargo install --locked cargo-deny'
}
$pkgs = @('-p','nativehtmlkit-host','-p','nativehtmlkit-renderer','-p','nativehtmlkit-document','-p','nativehtmlkit-assets','-p','nativehtmlkit-engine-types')
Step { cargo fmt @pkgs -- --check }
Step { cargo clippy --locked --workspace --all-targets -- -D warnings }
$threads = if ($env:RUST_TEST_THREADS) { $env:RUST_TEST_THREADS } else { '4' }
Step { cargo test --locked --workspace -- "--test-threads=$threads" }
Step { cargo build --locked --release -p nativehtmlkit-host }
Step { .\target\release\nativehtmlkit-host.exe --version }
# An existing same-version stage with a different host is refused; NHK_RESTAGE=1 replaces it.
$stageArgs = @('--stage-only'); if ($env:NHK_RESTAGE) { $stageArgs += '--restage' }
Step { Invoke-Python scripts/release-local.py @stageArgs }
'Source gate passed. Interactive Windows GUI tests are a separate requirement.'

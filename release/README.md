# Core release artifacts

Produced locally by `scripts/release-local.py --stage-only` into `dist/` (gitignored):
`nativehtmlkit-core-vVERSION-{linux,windows}-x64.zip` + `.zip.sha256`, containing the host
executable, `protocol/` (v1 contract, schemas, golden), licences (including the embedded Noto Sans
OFL), `SOURCE_PROVENANCE.json`, `release-manifest.json` (host SHA-256, `--version`, source commit,
dirty flag, toolchain) and `SHA256SUMS`. The stager verifies every file and the host `--version`
from a clean extraction. Upload to GitHub Releases only on explicit request; no GitHub Actions.

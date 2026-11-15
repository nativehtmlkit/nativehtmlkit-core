# NativeHTMLKit Core local build and release discipline

Migrated from the original UI4ME Core Rust 1.0.1 source (`SOURCE_PROVENANCE.json`). No hosted GitHub Actions, external registry publication or automatic workflows.

1. Restore licensed fallback and test fonts locally from the owner's source archive: `python3 scripts/restore-source-assets.py /path/to/ui4me-core-main.zip`.
2. Run `bash scripts/build-local.sh` or `scripts/build-local.ps1` on the actual OS. That performs asset and architecture checks (including protocol contract freshness and dispatch arms vs method list), vendored patch verification (`patches/*/verify.sh`; reported as SKIPPED if the pristine upstream checkout is absent), `cargo deny check advisories bans sources` (`deny.toml`; skipped with a WARNING if cargo-deny is not installed), formatting, Clippy, all Rust tests, a release binary build and a verified local stage.
3. Run host `doctor`, `render` and interactive tests from `protocol/CONFORMANCE-ROADMAP.md`. The repository's Rust tests are necessary but not a complete native qualification.
4. Use the project-local verification strategy to capture actual screenshots, protocol traces, logs, binary hashes, and environment details on Linux Wayland and Windows interactive desktop.
5. Use `python3 scripts/release-local.py --stage-only` locally to produce a Core binary + protocol candidate archive. It includes `SHA256SUMS`, generated third-party licence texts and a manifest with source commit, dirty flag, a reproducible source-tree hash (temporary index, the real index is untouched), the SHA-256 of `git diff HEAD` when dirty, host SHA-256, target triple and rustc/cargo versions. A same-version stage with a different host hash is refused unless `--restage` is given. The zip is deterministic (sorted entries, fixed timestamps and permissions); the host binary itself is **not** bit-reproducible (no `--remap-path-prefix`/`SOURCE_DATE_EPOCH` discipline), so compare host hashes only between stages of the same build.
6. Promote/release only after explicit user instruction: push signed/annotated exact source tag, stage artifacts from the exact commit, `python3 scripts/release-local.py --publish --confirm publish-VERSION`.
7. Never state that a candidate contract is stable without a complete conformance and compatibility matrix.

**Important:** the repository does not contain the binary font fixtures. Core cannot compile until the original fonts have been restored. The source archive is the only intended fixture source; the restore script hashes every file.

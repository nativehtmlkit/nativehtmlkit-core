# NativeHTMLKit Core local build and release discipline

Source: original UI4ME Core Rust 1.0.1 checkout. No hosted GitHub Actions, external registry publication or automatic workflows.

1. Restore licensed fallback and test fonts locally from the owner's source archive: `python3 scripts/restore-source-assets.py /path/to/ui4me-core-main.zip`.
2. Run `bash scripts/build-local.sh` or `scripts/build-local.ps1` on the actual OS. That performs formatting, Clippy, all Rust tests and a release binary build.
3. Run host `doctor`, `render` and interactive tests from `protocol/CONFORMANCE-ROADMAP.md`. The repository's Rust tests are necessary but not a complete native qualification.
4. Use the project-local verification strategy to capture actual screenshots, protocol traces, logs, binary hashes, and environment details on Linux Wayland and Windows interactive desktop.
5. Use `python3 scripts/release-local.py --stage-only` locally to produce a Core binary + protocol candidate archive. It includes SHA-256 manifest and source identity.
6. Promote/release only after explicit user instruction: push signed/annotated exact source tag, stage artifacts from the exact commit, `python3 scripts/release-local.py --publish --confirm publish-VERSION`.
7. Never state that a candidate contract is stable without a complete conformance and compatibility matrix.

**Important:** the handoff package does not contain binary font files. Core cannot compile until the original fonts have been restored. The source archive is the only intended fixture source; the restore script hashes every file.

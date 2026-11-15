# Core release artifact manifest — proposed shape, not yet implemented

**Purpose:** ensure CLI and every SDK can obtain a pinned, qualified Core host and SDK protocol contract without accessing Core's Rust source. Packaging remains the responsibility of each downstream repo.

## Proposed release content

```text
nativehtmlkit-core-<host-version>/
  linux-x64/nativehtmlkit-host
  windows-x64/nativehtmlkit-host.exe
  protocol/manifest.json
  protocol/idl/nativehtmlkit-wire-v1.d.ts
  protocol/schemas/*.schema.json
  protocol/conformance/fixtures.json
  protocol/compatibility.md
  SHA256SUMS
  PROVENANCE.json
  THIRD_PARTY_NOTICES.md
  RELEASE_NOTES.md
```

Names and OS packages are proposed. Published content must be immutable per version; if a release already exists, do not overwrite a binary and claim the same hash. A separate platform-specific release archive can be used if its manifest is unequivocal.

## Contract manifest fields

| Field | Purpose |
|---|---|
| `core_host_version` | Native binary implementation version |
| `wire_protocol_version` | Incompatible wire generation, initially 1 |
| `contract_revision` | Exact compatible API/capability snapshot |
| `contract_sha256` | Canonical IDL/schemas digest (define deterministic ordering/canonical encoding) |
| `source_commit` | Complete Git commit hash of compiled host source |
| `artifacts[]` | OS/architecture/file, sha256, binary file type, optional runtime dependency list |
| `required_runtime` | Supported Linux glibc/Wayland or interactive Windows requirements |
| `supported_platforms` | Qualified OS/arch/session combinations, with evidence reference |
| `capabilities` | Stable typed feature IDs and limits; not just marketing labels |
| `qualification` | Commands, status, live GUI results, source provenance and exact binary hash |

## SDK consumption and version pins

- SDK/release pipelines download a **specific immutable Core release**; verify archive and binary SHA256 and contract digest before packaging.
- Core does not depend on Python, PowerShell or the CLI to produce its release.
- Each SDK can vendor a host executable into its package, including required shared native dependencies and license notices.
- The CLI's Rust implementation can compile, but **must never compile the Core host from the external source checkout** for a release.
- A developer may explicitly override the host path while working locally, with a visible warning that this is **not qualified release mode**.
- SDK release versions are independent of host version, but every distributed SDK package records its bundled host version and digest.
- For future SDKs in any language, host launch, local IPC, hello authentication, reverse callbacks and lifecycle are the only required interoperability surfaces; language-specific FFI is not needed.

## Manual execution policy

Build, tests, packaging, publishing and manual compatibility checks can run a full automatic sequence once explicitly triggered, **but every GitHub Actions workflow must only have `workflow_dispatch`**. No events from other repos or new releases can automatically start a run. An explicit user command and confirmation input are required for release publication.
